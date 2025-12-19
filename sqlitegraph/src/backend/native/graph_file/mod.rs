//! Modularized graph file operations
//!
//! This directory contains the split modules from the original graph_file.rs
//! to maintain the 300 LOC limit while preserving all functionality.
//!
//! ## Module Organization
//!
//! - **buffers.rs**: Adaptive read/write buffer management
//! - **validation.rs**: File validation and corruption detection
//! - **encoding.rs**: Safe header encoding/decoding utilities
//! - **debug.rs**: Debug instrumentation and logging
//! - **file_ops.rs**: Core file I/O operations
//! - **header.rs**: Header encoding/decoding and persistent header operations (future)
//! - **transaction.rs**: Transaction lifecycle and commit management (future)
//! - **io_backend.rs**: I/O routing and backend selection (future)
//! - **mmap_ops.rs**: Memory mapping operations and management (future)

// Module exports
pub mod buffers;
pub mod validation;
pub mod encoding;
pub mod debug;
pub mod file_ops;
pub mod header;
pub mod transaction;
pub mod io_backend;
pub mod mmap_ops;
pub mod file_lifecycle;
pub mod io_operations;
pub mod node_edge_access;
pub mod file_management;
pub mod memory_mapping;
pub mod memory_resource_manager;
pub mod transaction_auditor;
pub mod graph_file_coordinator;
pub mod graph_file_core;
pub mod graph_file_io;
pub mod graph_file_accessors;
pub mod graph_file_advanced;

use std::fs::{File, OpenOptions};
use std::path::Path;
use std::io::{Read, Seek, Write, SeekFrom};

use crate::backend::native::{
    constants::{HEADER_SIZE, node::NODE_SLOT_SIZE,
               FLAG_V2_FRAMED_RECORDS, FLAG_V2_ATOMIC_COMMIT, edge},
    persistent_header::PersistentHeaderV2,
    transaction_state::TransactionState,
    types::{NativeResult, NativeNodeId, NativeBackendError, FileOffset, EdgeRecord, EdgeFlags, NodeRecord, NodeFlags, FileHeader},
};

#[cfg(feature = "v2_experimental")]
use memmap2::MmapMut;

use crate::backend::native::v2::{V2_MAGIC, V2_FORMAT_VERSION};

// Exported constants for parent module
pub const DEFAULT_NODE_DATA_START: u64 = 1024;
pub const RESERVED_NODE_REGION_BYTES: u64 = 8 * 1024 * 1024; // 8 MiB

// Re-export the main types for use by the parent module
pub use buffers::{ReadBuffer, WriteBuffer};
pub use validation::GraphFileValidator;
pub use encoding::{encode_persistent_header, decode_persistent_header, get_slice_safe};
pub use debug::DebugInstrumentation;
pub use file_ops::{FileOperations, IOMode};
pub use header::{HeaderManager, HeaderStatistics, ClusterUtilization};
pub use transaction::{TransactionManager, TransactionStatistics};
pub use io_backend::{IOBackendManager, IOBackendStatistics};
pub use mmap_ops::{MMapManager, MMapStatistics, MMapConfig};
pub use file_lifecycle::FileLifecycleManager;
pub use io_operations::IOOperationsManager;
pub use node_edge_access::NodeEdgeAccessManager;
pub use file_management::FileManager;
pub use memory_mapping::MemoryMappingManager;
pub use memory_resource_manager::{MemoryResourceManager, MemoryManagementStatistics, MemoryIOMode, AccessPatternHint, MemoryUtils};
pub use transaction_auditor::{TransactionAuditor, TransactionAuditorStatistics};
pub use graph_file_coordinator::{GraphFileCoordinator, TransactionCoordinatorStatistics};
pub use graph_file_advanced::{FileHealthStatus, OptimizationReport, DebugInfo};

/// Graph file wrapper that manages file handle and header operations
pub struct GraphFile {
    file: File,
    // Phase 10: Split header into persistent and runtime components
    persistent_header: PersistentHeaderV2,
    transaction_state: TransactionState,
    file_path: std::path::PathBuf,
    read_buffer: ReadBuffer,
    write_buffer: WriteBuffer,
    #[cfg(feature = "v2_experimental")]
    mmap: Option<MmapMut>,
    // Phase 75+: Track nodes whose V2 cluster metadata is modified during transaction
    transaction_auditor: TransactionAuditor,
}


impl GraphFile {
    // Core API methods moved to graph_file_core.rs

    pub fn finish_cluster_commit(&mut self) -> NativeResult<()> {
        TransactionManager::finish_cluster_commit(&mut self.file)
    }

    /// Phase 75: Record that a node's V2 cluster metadata was modified during transaction
    pub fn record_node_v2_cluster_modified(&mut self, node_id: NativeNodeId) {
        self.transaction_auditor.record_node_v2_cluster_modified(node_id);
    }

    /// Phase 75: CRITICAL FIX - Skip V2 node slot rewriting during rollback to prevent corruption
    fn clear_v2_cluster_metadata_on_rollback(&mut self) -> NativeResult<()> {
        self.transaction_auditor.clear_v2_cluster_metadata_on_rollback()
    }

    // === Essential API Methods Needed by Other Modules ===

    /// Get the current header (needed by edge_store and other modules)
    pub fn header(&self) -> &PersistentHeaderV2 {
        &self.persistent_header
    }

    /// Get mutable reference to persistent header (must call write_header() to persist changes)
    pub fn header_mut(&mut self) -> &mut PersistentHeaderV2 {
        &mut self.persistent_header
    }

    /// Get reference to transaction state (runtime-only)
    pub fn tx_state(&self) -> &TransactionState {
        &self.transaction_state
    }

    /// Get mutable reference to transaction state (runtime-only)
    pub fn tx_state_mut(&mut self) -> &mut TransactionState {
        &mut self.transaction_state
    }

    /// Get the current header (alias for header method)
    pub fn persistent_header(&self) -> &PersistentHeaderV2 {
        &self.persistent_header
    }

    /// Get mutable reference to persistent header (alias for header_mut)
    pub fn persistent_header_mut(&mut self) -> &mut PersistentHeaderV2 {
        &mut self.persistent_header
    }

    /// Get reference to transaction state (alias for tx_state)
    pub fn transaction_state(&self) -> &TransactionState {
        &self.transaction_state
    }

    /// Get mutable reference to transaction state (alias for tx_state_mut)
    pub fn transaction_state_mut(&mut self) -> &mut TransactionState {
        &mut self.transaction_state
    }

    /// Ensure file has at least the specified length (needed by node_edge_access)
    pub fn ensure_file_len_at_least(&mut self, min_len: u64) -> NativeResult<()> {
        use crate::backend::native::graph_file::io_operations::IOOperationsManager;
        IOOperationsManager::ensure_file_len_at_least(&mut self.file, min_len)
    }

    /// Write header to disk (needed by various operations)
    pub fn write_header(&mut self) -> NativeResult<()> {
        use crate::backend::native::graph_file::encoding::encode_persistent_header;
        use std::io::{Write, Seek, SeekFrom};

        let header_bytes = encode_persistent_header(&self.persistent_header)?;

        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&header_bytes)?;
        self.file.flush()?;
        Ok(())
    }

    /// Sync file to disk
    pub fn sync(&mut self) -> NativeResult<()> {
        self.file.sync_all().map_err(NativeBackendError::Io)
    }

    /// Get file path
    pub fn file_path(&self) -> &std::path::Path {
        &self.file_path
    }

    /// Read bytes from file at specific offset
    pub fn read_bytes(&mut self, offset: u64, buffer: &mut [u8]) -> NativeResult<()> {
        use std::io::{Read, Seek, SeekFrom};

        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(buffer)?;
        Ok(())
    }

    /// Write bytes to file at specific offset
    pub fn write_bytes(&mut self, offset: u64, data: &[u8]) -> NativeResult<()> {
        use std::io::{Write, Seek, SeekFrom};

        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(data)?;
        Ok(())
    }

    /// Get file size
    pub fn file_size(&self) -> NativeResult<u64> {
        use crate::backend::native::graph_file::file_ops::FileOperations;
        FileOperations::file_size(&self.file)
    }

    /// Get mutable reference to file for direct I/O operations
    fn file_mut(&mut self) -> &mut File {
        &mut self.file
    }
}

impl Drop for GraphFile {
    fn drop(&mut self) {
        // Ensure header is written before closing
        let _ = self.write_header();
        let _ = self.sync();
    }
}
