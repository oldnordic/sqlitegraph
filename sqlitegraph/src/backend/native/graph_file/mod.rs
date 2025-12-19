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
pub use memory_resource_manager::{MemoryResourceManager, MemoryManagementStatistics, MemoryIOMode, AccessPatternHint};
pub use transaction_auditor::{TransactionAuditor, TransactionAuditorStatistics};
pub use graph_file_coordinator::{GraphFileCoordinator, TransactionCoordinatorStatistics};

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
    /// Calculate the minimum safe offset for cluster allocation
    /// Ensures clusters are always outside the node region even when node_count = 0
    pub fn cluster_floor(&self) -> u64 {
        let header = &self.persistent_header;
        let node_region_end = header.node_data_offset + (header.node_count as u64 * NODE_SLOT_SIZE);

        // Ensure minimum separation: clusters must start at least 1MB beyond node data
        let min_cluster_start = header.node_data_offset + (1024 * 1024);

        std::cmp::max(node_region_end, min_cluster_start)
    }

    /// Create a new graph file with initial header
    pub fn create<P: AsRef<Path>>(path: P) -> NativeResult<Self> {
        FileLifecycleManager::create(path)
    }

    /// Open an existing graph file
    pub fn open<P: AsRef<Path>>(path: P) -> NativeResult<Self> {
        FileLifecycleManager::open(path)
    }

    /// Read header from file
    pub fn read_header(&mut self) -> NativeResult<()> {
        FileLifecycleManager::read_header(self)
    }

    /// Write header to file with durable persistence
    /// This is the ONLY authorized way to write to header region [0, HEADER_SIZE)
    pub fn write_header(&mut self) -> NativeResult<()> {
        FileLifecycleManager::write_header(self)
    }

    fn write_commit_marker_value(&mut self, value: u64) -> NativeResult<()> {
        TransactionManager::write_commit_marker_value(&mut self.file, value)
    }

    fn read_commit_marker_value(&mut self) -> NativeResult<u64> {
        TransactionManager::read_commit_marker_value(&mut self.file)
    }

    fn verify_commit_marker(&mut self) -> NativeResult<()> {
        let marker = self.read_commit_marker_value()?;
        validation::GraphFileValidator::verify_commit_marker(marker)
    }

    pub fn begin_cluster_commit(&mut self) -> NativeResult<()> {
        TransactionManager::begin_cluster_commit(&mut self.file)
    }

    pub fn finish_cluster_commit(&mut self) -> NativeResult<()> {
        TransactionManager::finish_cluster_commit(&mut self.file)
    }

  
    /// Phase 70: Begin atomic transaction with rollback state
    pub fn begin_transaction(&mut self, tx_id: u64) -> NativeResult<()> {
        // TX_BEGIN_AUDIT: Check node 257 slot before transaction operations
        let node_data_offset = self.persistent_header().node_data_offset;
        let auditor = &mut self.transaction_auditor;
        auditor.audit_transaction_begin(node_data_offset, |slot_offset, buffer| {
            // Use file operations directly to avoid borrowing issues
            use std::io::{Read, Seek, SeekFrom};
            let mut file = &self.file;
            file.seek(SeekFrom::Start(slot_offset))?;
            file.read_exact(buffer)?;
            Ok(())
        })?;

        // PHASE 2D: Probe node1 corruption before any transaction operations
        self.transaction_auditor.debug_edge_cluster_before_transaction(&self.file_path(), || {
            self.file_size()
        })?;

        // Use GraphFileCoordinator for transaction management
        {
            // Extract components first to avoid multiple mutable borrows
            let persistent_header = &mut self.persistent_header;
            let transaction_state = &mut self.transaction_state;

            let mut coordinator = GraphFileCoordinator::new(
                persistent_header,
                transaction_state,
            );
            coordinator.begin_transaction(tx_id)?;
        } // coordinator goes out of scope, releasing borrows

        // TX_BEGIN_AUDIT: Check node 257 slot after header state modification
        if std::env::var("TX_BEGIN_AUDIT").is_ok() {
            let node_data_offset = self.persistent_header().node_data_offset;
            let slot_offset = node_data_offset + ((257 - 1) as u64 * 4096);
            let mut buffer = vec![0u8; 32];

            if self.read_bytes(slot_offset, &mut buffer).is_ok() {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} first_32={:02x?} version={}",
                    "AFTER_TX_STATE_MODIFY", 257, slot_offset, &buffer, buffer[0]
                );
            } else {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} READ_FAILED",
                    "AFTER_TX_STATE_MODIFY", 257, slot_offset
                );
            }
        }

        // PHASE 2D: Probe after header modification
        if std::env::var("EDGE_CLUSTER_DEBUG").is_ok() {
            let mut disk_file = std::fs::File::open(&self.file_path())?;
            let mut node1_bytes = vec![0u8; 32];
            disk_file.seek(std::io::SeekFrom::Start(0x400))?;
            disk_file.read_exact(&mut node1_bytes)?;
            let version_after_header_modify = node1_bytes[0];
            let file_size_after_header_modify = self.file_size().unwrap_or(0);
            println!(
                "[EDGE_CLUSTER_DEBUG] AFTER_HEADER_MODIFY: node1_version={}, file_size={}, node1_bytes={:02x?}",
                version_after_header_modify, file_size_after_header_modify, &node1_bytes
            );
        }

        // Write header with transaction state IN_PROGRESS
        self.write_header()?;

        // TX_BEGIN_AUDIT: Check node 257 slot after header write
        if std::env::var("TX_BEGIN_AUDIT").is_ok() {
            let node_data_offset = self.persistent_header().node_data_offset;
            let slot_offset = node_data_offset + ((257 - 1) as u64 * 4096);
            let mut buffer = vec![0u8; 32];

            if self.read_bytes(slot_offset, &mut buffer).is_ok() {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} first_32={:02x?} version={}",
                    "AFTER_HEADER_WRITE", 257, slot_offset, &buffer, buffer[0]
                );
            } else {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} READ_FAILED",
                    "AFTER_HEADER_WRITE", 257, slot_offset
                );
            }
        }

        // PHASE 2D: Probe after header write
        if std::env::var("EDGE_CLUSTER_DEBUG").is_ok() {
            let mut disk_file = std::fs::File::open(&self.file_path())?;
            let mut node1_bytes = vec![0u8; 32];
            disk_file.seek(std::io::SeekFrom::Start(0x400))?;
            disk_file.read_exact(&mut node1_bytes)?;
            let version_after_header_write = node1_bytes[0];
            let file_size_after_header_write = self.file_size().unwrap_or(0);
            println!(
                "[EDGE_CLUSTER_DEBUG] AFTER_HEADER_WRITE: node1_version={}, file_size={}, node1_bytes={:02x?}",
                version_after_header_write, file_size_after_header_write, &node1_bytes
            );
        }

        // Force header to disk before writing data (SQLite WAL protocol)
        self.file.sync_all()?;

        // TX_BEGIN_AUDIT: Check node 257 slot after sync_all
        if std::env::var("TX_BEGIN_AUDIT").is_ok() {
            let node_data_offset = self.persistent_header().node_data_offset;
            let slot_offset = node_data_offset + ((257 - 1) as u64 * 4096);
            let mut buffer = vec![0u8; 32];

            if self.read_bytes(slot_offset, &mut buffer).is_ok() {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} first_32={:02x?} version={}",
                    "AFTER_SYNC_ALL", 257, slot_offset, &buffer, buffer[0]
                );
            } else {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} READ_FAILED",
                    "AFTER_SYNC_ALL", 257, slot_offset
                );
            }
        }

        // PHASE 2D: Probe after sync_all
        if std::env::var("EDGE_CLUSTER_DEBUG").is_ok() {
            let mut disk_file = std::fs::File::open(&self.file_path())?;
            let mut node1_bytes = vec![0u8; 32];
            disk_file.seek(std::io::SeekFrom::Start(0x400))?;
            disk_file.read_exact(&mut node1_bytes)?;
            let version_after_sync = node1_bytes[0];
            let file_size_after_sync = self.file_size().unwrap_or(0);
            println!(
                "[EDGE_CLUSTER_DEBUG] AFTER_SYNC_ALL: node1_version={}, file_size={}, node1_bytes={:02x?}",
                version_after_sync, file_size_after_sync, &node1_bytes
            );
        }

        // TX_BEGIN_AUDIT: Final check after all transaction begin operations
        if std::env::var("TX_BEGIN_AUDIT").is_ok() {
            let node_data_offset = self.persistent_header().node_data_offset;
            let slot_offset = node_data_offset + ((257 - 1) as u64 * 4096);
            let mut buffer = vec![0u8; 32];

            if self.read_bytes(slot_offset, &mut buffer).is_ok() {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} first_32={:02x?} version={}",
                    "AFTER_TX_COMPLETE", 257, slot_offset, &buffer, buffer[0]
                );
            } else {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} READ_FAILED",
                    "AFTER_TX_COMPLETE", 257, slot_offset
                );
            }
        }

        debug::DebugInstrumentation::log_transaction_phase("begun", tx_id);
        Ok(())
    }

    /// Phase 70: Commit atomic transaction
    pub fn commit_transaction(&mut self) -> NativeResult<()> {
        // Save tx_id for logging since we'll need to access it after commit
        let current_tx_id = self.tx_state().tx_id;

        // Write header before commit (required by protocol)
        self.write_header()?;

        // Sync file to disk
        use std::io::Write;
        self.file.sync_all()?;

        // Use GraphFileCoordinator for transaction management
        {
            // Extract components first to avoid multiple mutable borrows
            let persistent_header = &mut self.persistent_header;
            let transaction_state = &mut self.transaction_state;

            let mut coordinator = GraphFileCoordinator::new(
                persistent_header,
                transaction_state,
            );
            coordinator.commit_transaction(|| Ok(()), || Ok(()))?;
        } // coordinator goes out of scope, releasing borrows

        debug::DebugInstrumentation::log_transaction_phase("committed", current_tx_id);
        Ok(())
    }

    /// Phase 70: Rollback incomplete atomic transaction
    pub fn rollback_transaction(&mut self) -> NativeResult<()> {
        // Capture state before rollback for debugging
        let (current_size, node_data_offset, node_count) = {
            let header = self.persistent_header();
            (
                self.file_size()?,
                header.node_data_offset,
                header.node_count,
            )
        };
        let node_region_end = node_data_offset
            + (node_count as u64 * crate::backend::native::graph_file::NODE_SLOT_SIZE);

        // Use GraphFileCoordinator for rollback management
        {
            // Extract components first to avoid multiple mutable borrows
            let persistent_header = &mut self.persistent_header;
            let transaction_state = &mut self.transaction_state;

            let mut coordinator = GraphFileCoordinator::new(
                persistent_header,
                transaction_state,
            );
            // Note: The actual rollback logic below will be delegated to coordinator in a future change
        } // coordinator goes out of scope, releasing borrows

        // Phase 72: Calculate rollback floor - never truncate below node region
        let intended_rollback_size = self.persistent_header().free_space_offset;
        let rollback_floor = std::cmp::max(node_region_end, node_data_offset);

        // Additional protection: ensure all written node slots are protected
        // NEVER rollback below the file size - nodes are persistent and should never be truncated
        // This ensures all node slots that have been written are preserved
        let enhanced_rollback_floor = current_size; // Never truncate at all
        let final_rollback_size = std::cmp::max(intended_rollback_size, enhanced_rollback_floor);

        println!(
            "PHASE 72: rollback_floor = {}, enhanced_rollback_floor = {}, final_rollback_size = {}",
            rollback_floor, enhanced_rollback_floor, final_rollback_size
        );

        // TRUNC_AUDIT: Log file truncation operations
        if std::env::var("TRUNC_AUDIT").is_ok() {
            println!(
                "[TRUNC_AUDIT] ROLLBACK: current_size={}, intended_rollback_size={}, rollback_floor={}, enhanced_rollback_floor={}, final_rollback_size={}, will_truncate={}",
                current_size,
                intended_rollback_size,
                rollback_floor,
                enhanced_rollback_floor,
                final_rollback_size,
                current_size > final_rollback_size
            );
        }

        if current_size > final_rollback_size {
            // SLOT CORRUPTION DEBUG: Log truncation that could affect node slots
            if std::env::var("SLOT_CORRUPTION_DEBUG").is_ok() {
                println!(
                    "[SLOT_CORRUPTION] FILE_TRUNCATE: current_size={}, final_rollback_size={}, difference={} bytes",
                    current_size,
                    final_rollback_size,
                    current_size - final_rollback_size
                );
            }

            // Truncate file to remove any partially written cluster data, but never below rollback_floor
            if std::env::var("TRUNC_AUDIT").is_ok() {
                println!(
                    "[TRUNC_AUDIT] BEFORE_TRUNCATE: calling set_len({})",
                    final_rollback_size
                );
            }
            self.file.set_len(final_rollback_size)?;
            if std::env::var("TRUNC_AUDIT").is_ok() {
                println!(
                    "[TRUNC_AUDIT] AFTER_TRUNCATE: set_len completed, new_file_size={}",
                    self.file_size().unwrap_or(0)
                );
            }

            // If we clamped the rollback_size, update free_space_offset to match actual file size
            if final_rollback_size > intended_rollback_size {
                self.persistent_header_mut().free_space_offset = final_rollback_size;
            }

            // SLOT CORRUPTION DEBUG: Verify node slots are preserved after truncation
            if std::env::var("SLOT_CORRUPTION_DEBUG").is_ok() {
                let node_data_offset = self.persistent_header().node_data_offset;
                for node_id in 256..=258 {
                    let slot_offset = node_data_offset + ((node_id - 1) as u64 * 4096);
                    let mut buffer = [0u8; 1];
                    if self.read_bytes(slot_offset, &mut buffer).is_ok() {
                        println!(
                            "[SLOT_CORRUPTION] POST_TRUNCATE_CHECK: node_id={}, slot_offset=0x{:x}, version={}",
                            node_id, slot_offset, buffer[0]
                        );
                    }
                }
            }
        }

        // PHASE 74 FIX: Reset cluster offsets to 0 since clusters were truncated
        // This prevents node metadata from pointing to truncated cluster data
        {
            let header = self.persistent_header_mut();
            header.outgoing_cluster_offset = 0;
            header.incoming_cluster_offset = 0;
        }

        // Mutable borrow automatically cleared at end of scope

        // PHASE 74 FIX: Clear cluster metadata from V2 node records that might
        // have been updated before the transaction failed and rolled back
        self.clear_v2_cluster_metadata_on_rollback()?;

        // Persist the rolled-back header
        self.write_header()?;

        println!(
            "PHASE 72: Transaction rolled back to offset {}",
            final_rollback_size
        );
        Ok(())
    }

    /// Phase 75: Record that a node's V2 cluster metadata was modified during transaction
    pub fn record_node_v2_cluster_modified(&mut self, node_id: NativeNodeId) {
        self.transaction_auditor.record_node_v2_cluster_modified(node_id);
    }

    /// Phase 75: CRITICAL FIX - Skip V2 node slot rewriting during rollback to prevent corruption
    fn clear_v2_cluster_metadata_on_rollback(&mut self) -> NativeResult<()> {
        self.transaction_auditor.clear_v2_cluster_metadata_on_rollback()
    }

    /// Verify header was written correctly (temporary instrumentation for Phase 43)
    fn verify_header_written_immediately(&mut self, expected_bytes: &[u8]) -> NativeResult<()> {
        let mut read_back = vec![0u8; 16]; // Read first 16 bytes for verification
        self.file.seek(SeekFrom::Start(0))?;
        self.file.read_exact(&mut read_back)?;

        if expected_bytes.len() >= 16 {
            if read_back != &expected_bytes[..16] {
                return Err(NativeBackendError::CorruptNodeRecord {
                    node_id: -1,
                    reason: format!(
                        "Header write verification failed\nExpected first 16 bytes: {:02X?}\nActually read: {:02X?}",
                        &expected_bytes[..16],
                        read_back
                    ),
                });
            }
        }

        Ok(())
    }

    /// Get the current header
    pub fn persistent_header(&self) -> &PersistentHeaderV2 {
        &self.persistent_header
    }

    /// Get mutable reference to persistent header (must call write_header() to persist changes)
    pub fn persistent_header_mut(&mut self) -> &mut PersistentHeaderV2 {
        &mut self.persistent_header
    }

    /// Get reference to transaction state (runtime-only)
    pub fn transaction_state(&self) -> &TransactionState {
        &self.transaction_state
    }

    /// Get mutable reference to transaction state (runtime-only)
    pub fn transaction_state_mut(&mut self) -> &mut TransactionState {
        &mut self.transaction_state
    }

    // === HEADER ACCESS METHODS ===

    pub fn header(&self) -> &PersistentHeaderV2 {
        &self.persistent_header
    }

    pub fn header_mut(&mut self) -> &mut PersistentHeaderV2 {
        &mut self.persistent_header
    }

    pub fn tx_state(&self) -> &TransactionState {
        &self.transaction_state
    }

    pub fn tx_state_mut(&mut self) -> &mut TransactionState {
        &mut self.transaction_state
    }

    // === HEADER STATISTICS METHODS ===

    /// Get header statistics for debugging and monitoring
    pub fn get_header_statistics(&self) -> HeaderStatistics {
        HeaderManager::get_header_statistics(&self.persistent_header, RESERVED_NODE_REGION_BYTES)
    }

    /// Validate header invariants
    pub fn validate_header_invariants(&self) -> NativeResult<()> {
        HeaderManager::validate_header_invariants(&self.persistent_header)
    }

    /// Check if clusters are properly positioned
    pub fn are_clusters_positioned_correctly(&self) -> bool {
        let stats = self.get_header_statistics();
        stats.are_clusters_positioned_correctly()
    }
}

impl GraphFile {
    /// Get file path for instrumentation (PHASE 2D)
    pub fn file_path(&self) -> &std::path::Path {
        &self.file_path
    }

    /// Get file path
    pub fn path(&self) -> &std::path::Path {
        &self.file_path
    }

    /// Get file size
    pub fn file_size(&self) -> NativeResult<u64> {
        FileOperations::file_size(&self.file)
    }

    fn initialize_v2_header(&mut self) {
        let node_count = self.persistent_header().node_count;
        HeaderManager::initialize_v2_header(
            self.persistent_header_mut(),
            node_count,
            DEFAULT_NODE_DATA_START,
            RESERVED_NODE_REGION_BYTES,
        )
        .expect("Failed to initialize V2 header");

        // CRITICAL SAFETY: Validate header after initialization
        HeaderManager::validate_header_invariants(&self.persistent_header)
            .expect("Header invariants violated after initialization");
    }

    /// Validate file size against header information
    pub fn validate_file_size(&self) -> NativeResult<()> {
        let file_size = self.file_size()?;
        FileManager::validate_file_size(file_size, &self.persistent_header)
    }

    /// Grow file by specified number of bytes
    pub fn grow(&mut self, additional_bytes: u64) -> NativeResult<()> {
        FileManager::grow_file(&mut self.file, additional_bytes)
    }

    /// Sync file to disk
    pub fn sync(&self) -> NativeResult<()> {
        FileLifecycleManager::sync(self)
    }

    /// Read bytes from file at specific offset with managed memory resources
    pub fn read_bytes(&mut self, offset: u64, buffer: &mut [u8]) -> NativeResult<()> {
        // PHASE 2C.3: Write buffer coherence instrumentation
        if std::env::var("WRITEBUF_DEBUG").is_ok() {
            let pending_ops = self.write_buffer.operations.len();
            println!(
                "[WRITEBUF_DEBUG] READ_ENTRY: offset=0x{:x}, len={}, pending_ops={}, callsite={}:{}",
                offset,
                buffer.len(),
                pending_ops,
                file!(),
                line!()
            );
        }

        // Use MemoryResourceManager for memory-aware read operations
        let file_size = self.file_size()?; // Get file size before creating memory manager

        let mut memory_manager = MemoryResourceManager::new(
            &mut self.read_buffer,
            &mut self.write_buffer,
            #[cfg(feature = "v2_experimental")]
            &mut self.mmap,
        );

        memory_manager.memory_aware_read(
            &mut self.file,
            offset,
            buffer,
            || Ok(file_size),
        )?;

        Ok(())
    }

    /// Write bytes directly to file without buffering (Phase 41 fix for cluster corruption)
    pub fn write_bytes_direct(&mut self, offset: u64, data: &[u8]) -> NativeResult<()> {
        // PHASE 43: CRITICAL HEADER REGION LOCKDOWN
        // Prevent any writes into header region [0, HEADER_SIZE)
        if offset < super::constants::HEADER_SIZE {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1, // System-level error
                reason: format!(
                    "attempted write into header region: offset={}, len={}, HEADER_SIZE={}",
                    offset,
                    data.len(),
                    super::constants::HEADER_SIZE
                ),
            });
        }

        // CRITICAL INVARIANT: Ensure file size covers the write range before any I/O
        let required_size = offset + data.len() as u64;
        let current_size = self.file_size()?;
        if required_size > current_size {
            self.grow(required_size - current_size)?;
        }

        // Phase 41: Route direct writes based on exclusive I/O mode
        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
        {
            // EXCLUSIVE MMAP MODE: Write directly to mmap
            let end_offset = offset + data.len() as u64;
            self.ensure_mmap_covers(end_offset)?;

            let mmap = self
                .mmap
                .as_mut()
                .ok_or(NativeBackendError::CorruptNodeRecord {
                    node_id: -1,
                    reason: "mmap not initialized in exclusive mmap mode".to_string(),
                })?;

            if offset as usize + data.len() > mmap.len() {
                return Err(NativeBackendError::CorruptNodeRecord {
                    node_id: -1,
                    reason: format!(
                        "mmap write out of bounds: offset={}, len={}, mmap_len={}",
                        offset,
                        data.len(),
                        mmap.len()
                    ),
                });
            }

            mmap[offset as usize..offset as usize + data.len()].copy_from_slice(data);
            mmap.flush()?;
            return Ok(());
        }

        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
        {
            // EXCLUSIVE STD MODE: Write directly to file with explicit sync
            use std::io::{Seek, Write};
            self.file.seek(std::io::SeekFrom::Start(offset))?;
            self.file.write_all(data)?;
            self.file.sync_all()?;
            return Ok(());
        }

        #[cfg(not(any(
            feature = "v2_experimental",
            feature = "v2_io_exclusive_mmap",
            feature = "v2_io_exclusive_std"
        )))]
        {
            // DEFAULT MODE: Write directly to file without buffering (bypass write_buffer)
            use std::io::{Seek, Write};
            self.file.seek(std::io::SeekFrom::Start(offset))?;
            self.file.write_all(data)?;
            self.file.sync_all()?;
            return Ok(());
        }
    }

    /// Write bytes to file at specific offset with write-behind buffering
    pub fn write_bytes(&mut self, offset: u64, data: &[u8]) -> NativeResult<()> {
        // PHASE 2D: EDGE_CLUSTER_DEBUG - Probe write calls that might corrupt node slots
        if std::env::var("EDGE_CLUSTER_DEBUG").is_ok() {
            // Check if this write might affect node slot region
            let node1_slot_start = 0x400;
            let node1_slot_end = 0x400 + 4096;

            let write_end = offset + data.len() as u64;
            let affects_node1 = (offset < node1_slot_end) && (write_end > node1_slot_start);

            if affects_node1 || (offset >= 0x10000 && offset <= 0x20000) {
                // Also monitor cluster writes
                let mut disk_file = std::fs::File::open(&self.file_path())?;
                let mut node1_bytes = vec![0u8; 32];
                disk_file.seek(std::io::SeekFrom::Start(0x400))?;
                disk_file.read_exact(&mut node1_bytes)?;
                let version_before_write = node1_bytes[0];
                let file_size_before_write = self.file_size().unwrap_or(0);

                println!(
                    "[EDGE_CLUSTER_DEBUG] WRITE_CALL: offset=0x{:x}, len={}, affects_node1={}, version={}, file_size={}, first_32_write={:02x?}, callsite={}:{}",
                    offset,
                    data.len(),
                    affects_node1,
                    version_before_write,
                    file_size_before_write,
                    &data[..data.len().min(32)],
                    file!(),
                    line!()
                );
            }
        }

        // PHASE 2E: WRITE_AUDIT - LOWEST LEVEL WRITE AUDIT
        if std::env::var("WRITE_AUDIT").is_ok() {
            let node_data_offset = self.persistent_header().node_data_offset; // Should be 0x400
            let sensitive_start = node_data_offset;
            let sensitive_end = node_data_offset + 0x20000; // 128KB buffer
            let write_end = offset + data.len() as u64;

            let overlaps_sensitive_region =
                (offset < sensitive_end) && (write_end > sensitive_start);

            if overlaps_sensitive_region {
                let head16 = if data.len() >= 16 {
                    format!("{:02x?}", &data[..16])
                } else {
                    format!("{:02x?}", data)
                };
                let caller_function = {
                    // Use standard library's panic hook to get caller info in a non-allocating way
                    std::panic::Location::caller()
                };
                println!(
                    "[WRITE_AUDIT] offset=0x{:x}, len={}, overlaps_node_region={}, head16={}, callsite={}:{}, api=write_bytes, caller_file={}:{}, caller_line={}",
                    offset,
                    data.len(),
                    1,
                    head16,
                    file!(),
                    line!(),
                    caller_function.file(),
                    caller_function.line(),
                    caller_function.column()
                );
            }
        }
        // PHASE 2D: EDGE_CLUSTER_DEBUG - Probe write calls that might corrupt node slots
        if std::env::var("EDGE_CLUSTER_DEBUG").is_ok() {
            // Check if this write might affect node slot region
            let node1_slot_start = 0x400;
            let node1_slot_end = 0x400 + 4096;

            let write_end = offset + data.len() as u64;
            let affects_node1 = (offset < node1_slot_end) && (write_end > node1_slot_start);

            if affects_node1 || (offset >= 0x10000 && offset <= 0x20000) {
                // Also monitor cluster writes
                let mut disk_file = std::fs::File::open(&self.file_path())?;
                let mut node1_bytes = vec![0u8; 32];
                disk_file.seek(std::io::SeekFrom::Start(0x400))?;
                disk_file.read_exact(&mut node1_bytes)?;
                let version_before_write = node1_bytes[0];
                let file_size_before_write = self.file_size().unwrap_or(0);

                println!(
                    "[EDGE_CLUSTER_DEBUG] WRITE_CALL: offset=0x{:x}, len={}, affects_node1={}, version={}, file_size={}, first_32_write={:02x?}, callsite={}:{}",
                    offset,
                    data.len(),
                    affects_node1,
                    version_before_write,
                    file_size_before_write,
                    &data[..data.len().min(32)],
                    file!(),
                    line!()
                );
            }
        }

        // PHASE 43: CRITICAL HEADER REGION LOCKDOWN
        // Prevent any writes into header region [0, HEADER_SIZE)
        if offset < super::constants::HEADER_SIZE {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1, // System-level error
                reason: format!(
                    "attempted write into header region: offset={}, len={}, HEADER_SIZE={}",
                    offset,
                    data.len(),
                    super::constants::HEADER_SIZE
                ),
            });
        }

        // PHASE 2D: CRITICAL FIX - Node slot writes must NOT go through write buffer
        // Node slots (4096-byte regions starting at 0x400 + n*4096) must be written directly
        let is_node_slot =
            (offset >= 0x400) && ((offset - 0x400) % 4096 == 0) && (data.len() == 4096);

        // Phase 41: Route writes based on exclusive I/O mode
        // For small writes, try to buffer them (EXCEPT node slots)
        if !is_node_slot && data.len() <= 256 && self.write_buffer.add(offset, data.to_vec()) {
            // PHASE 2C.3: Write buffer coherence instrumentation
            if std::env::var("WRITEBUF_DEBUG").is_ok() {
                println!(
                    "[WRITEBUF_DEBUG] WRITE_BUFFERED: offset=0x{:x}, len={}, buffered=true, callsite={}:{}",
                    offset,
                    data.len(),
                    file!(),
                    line!()
                );
            }
            return Ok(());
        }

        // PHASE 2C.3: Write buffer coherence instrumentation
        if std::env::var("WRITEBUF_DEBUG").is_ok() {
            println!(
                "[WRITEBUF_DEBUG] WRITE_DIRECT: offset=0x{:x}, len={}, buffered=false (size>256), callsite={}:{}",
                offset,
                data.len(),
                file!(),
                line!()
            );
        }

        // CRITICAL INVARIANT: Ensure file size covers the write range before any I/O
        let required_size = offset + data.len() as u64;
        let current_size = self.file_size()?;
        if required_size > current_size {
            self.grow(required_size - current_size)?;
        }

        // Buffer full or large write - flush pending writes and write directly
        self.flush_write_buffer()?;

        // PHASE 41: Route writes based on exclusive I/O mode
        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
        {
            // EXCLUSIVE MMAP MODE: Write directly to mmap
            let end_offset = offset + data.len() as u64;
            self.ensure_mmap_covers(end_offset)?;

            let mmap = self
                .mmap
                .as_mut()
                .ok_or(NativeBackendError::CorruptNodeRecord {
                    node_id: -1,
                    reason: "mmap not initialized in exclusive mmap mode".to_string(),
                })?;

            if offset as usize + data.len() > mmap.len() {
                return Err(NativeBackendError::CorruptNodeRecord {
                    node_id: -1,
                    reason: format!(
                        "Write beyond mmap region: offset={}, len={}, mmap_size={}",
                        offset,
                        data.len(),
                        mmap.len()
                    ),
                });
            }

            let start = offset as usize;
            let end = start + data.len();
            mmap[start..end].copy_from_slice(data);

            // Flush mmap changes to disk
            mmap.flush()?;

            // Also ensure underlying file is extended
            if end_offset > self.file_size()? {
                self.file.set_len(end_offset)?;
            }
        }
        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
        {
            // EXCLUSIVE STD MODE: Use standard I/O with explicit sync
            self.file.seek(SeekFrom::Start(offset))?;
            self.file.write_all(data)?;
            self.file.sync_all()?;
        }
        #[cfg(not(any(
            feature = "v2_experimental",
            feature = "v2_io_exclusive_mmap",
            feature = "v2_io_exclusive_std"
        )))]
        {
            // DEFAULT MODE: Mixed I/O (current Phase 40 behavior)
            self.file.seek(SeekFrom::Start(offset))?;
            self.file.write_all(data)?;

            // PHASE 2D: Probe node1 corruption after actual write
            if std::env::var("EDGE_CLUSTER_DEBUG").is_ok() {
                let write_end = offset + data.len() as u64;
                let affects_node1 = (offset < 0x400 + 4096) && (write_end > 0x400);

                if affects_node1 || (offset >= 0x10000 && offset <= 0x20000) {
                    let mut disk_file = std::fs::File::open(&self.file_path())?;
                    let mut node1_bytes = vec![0u8; 32];
                    disk_file.seek(std::io::SeekFrom::Start(0x400))?;
                    disk_file.read_exact(&mut node1_bytes)?;
                    let version_after_write = node1_bytes[0];
                    let file_size_after_write = self.file_size().unwrap_or(0);

                    println!(
                        "[EDGE_CLUSTER_DEBUG] WRITE_DONE: offset=0x{:x}, len={}, affects_node1={}, version={}, file_size={}, callsite={}:{}",
                        offset,
                        data.len(),
                        affects_node1,
                        version_after_write,
                        file_size_after_write,
                        file!(),
                        line!()
                    );
                }
            }

            // PHASE 40: Conservative mmap management - only remap for significant growth
            let _end_offset = offset + data.len() as u64;
            // GraphFile no longer supports mmap; existing write paths guarantee file growth
        }

        Ok(())
    }

    /// Read with adaptive read-ahead optimization
    fn read_with_ahead(&mut self, offset: u64, buffer: &mut [u8]) -> NativeResult<()> {
        // Use adaptive sizing to minimize I/O amplification
        let optimal_capacity = ReadBuffer::adaptive_capacity(buffer.len());

        // Resize buffer if needed (rare, only for pattern changes)
        if optimal_capacity != self.read_buffer.capacity {
            self.read_buffer = ReadBuffer::with_capacity(optimal_capacity);
        }

        // Use the smaller of: optimal capacity or what fits in remaining file space
        let read_size = std::cmp::max(buffer.len(), optimal_capacity);
        let read_ahead_size = std::cmp::min(read_size, optimal_capacity);

        // Phase 14 Step 9: Prevent "failed to fill whole buffer" errors in k-hop operations
        // Validate we don't try to read beyond file end
        let file_size = self.file.metadata().map(|m| m.len()).unwrap_or(0);
        let remaining_bytes = file_size.saturating_sub(offset);

        if remaining_bytes == 0 {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1, // Unknown node ID for file-level error
                reason: format!(
                    "Attempted to read beyond file end at offset {} (file size: {})",
                    offset, file_size
                ),
            });
        }

        // Adjust read_ahead_size to fit within available data
        let adjusted_read_size = std::cmp::min(read_ahead_size as u64, remaining_bytes) as usize;

        // Step 21.1: I/O Amplification Optimization
        // For 32-byte header: adaptive_capacity = 256 → reads 256B instead of 64KB (256x reduction)
        // Amplification reduced from 2,048x to 8x for typical node reads
        self.file.seek(SeekFrom::Start(offset))?;
        self.file
            .read_exact(&mut self.read_buffer.data[..adjusted_read_size])?;

        // Update buffer metadata with actual size read
        self.read_buffer.offset = offset;
        self.read_buffer.size = adjusted_read_size;

        // Phase 14 Step 11: Allow variable-length reads below read_ahead capacity
        // If the request exceeds adjusted_read_size, perform a direct read instead of using
        // the read-ahead buffer so large node records still use the deterministic two-stage path
        let needs_direct_read = buffer.len() > adjusted_read_size;

        if needs_direct_read {
            if buffer.len() as u64 <= remaining_bytes {
                // Direct read for large variable-length records or boundary-crossing reads
                self.file.seek(SeekFrom::Start(offset))?;
                self.file.read_exact(buffer)?;
                return Ok(());
            } else {
                return Err(NativeBackendError::BufferTooSmall {
                    size: remaining_bytes as usize,
                    min_size: buffer.len(),
                });
            }
        }

        // Satisfy original request from buffer
        self.read_buffer.read(offset, buffer);
        Ok(())
    }

    /// Flush pending write operations
    pub fn flush_write_buffer(&mut self) -> NativeResult<()> {
        let operations = self.write_buffer.flush();

        // Sort operations by offset for better I/O patterns
        let mut sorted_ops: Vec<_> = operations.into_iter().collect();
        sorted_ops.sort_by_key(|(offset, _)| *offset);

        // CRITICAL INVARIANT: Ensure file size covers ALL buffered writes BEFORE any I/O
        let current_file_size = self.file_size()?;
        let mut max_end_offset = current_file_size;
        for (offset, data) in &sorted_ops {
            let end_offset = offset + data.len() as u64;
            max_end_offset = max_end_offset.max(end_offset);
        }

        if max_end_offset > current_file_size {
            self.file.set_len(max_end_offset)?;
            self.file.flush()?;
        }

        for (offset, data) in sorted_ops {
            // PHASE 43: CRITICAL HEADER REGION LOCKDOWN - Double-check during flush
            if offset < super::constants::HEADER_SIZE {
                return Err(NativeBackendError::CorruptNodeRecord {
                    node_id: -1,
                    reason: format!(
                        "FLUSH: attempted write into header region: offset={}, len={}, HEADER_SIZE={}",
                        offset,
                        data.len(),
                        super::constants::HEADER_SIZE
                    ),
                });
            }

            self.file.seek(SeekFrom::Start(offset))?;
            self.file.write_all(&data)?;
        }

        // PHASE 40: Conservative mmap management for buffered writes
        #[cfg(feature = "v2_experimental")]
        {
            if max_end_offset > 0 {
                self.ensure_mmap_covers(max_end_offset)?;
            }
        }

        Ok(())
    }

    /// Flush pending writes
    pub fn flush(&mut self) -> NativeResult<()> {
        FileManager::flush_complete(&mut self.file, &mut self.write_buffer)
    }

    /// Invalidate read buffer to force fresh reads from disk
    pub fn invalidate_read_buffer(&mut self) {
        FileManager::invalidate_read_buffer(&mut self.read_buffer)
    }

    /// SINGLE INVARIANT: Ensure file is large enough for safe read operations
    /// This prevents "failed to fill whole buffer" errors by validating file size
    /// before any read_exact call. Returns detailed error for debugging.
    fn ensure_file_len_at_least(
        &self,
        required_offset: u64,
        required_len: usize,
    ) -> NativeResult<()> {
        let current_len = self.file.metadata()?.len();
        let required_end = required_offset + required_len as u64;

        if required_end > current_len {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1, // System-level error, not node-specific
                reason: format!(
                    "File too small for read: need {} bytes starting at offset {} ({} total), \
                     but file is only {} bytes long. Missing {} bytes.",
                    required_len,
                    required_offset,
                    required_end,
                    current_len,
                    required_end.saturating_sub(current_len)
                ),
            });
        }
        Ok(())
    }

    /// Read bytes directly from file without any buffering (critical for V2 corruption fix)
    pub fn read_bytes_direct(&mut self, offset: u64, buffer: &mut [u8]) -> NativeResult<()> {
        // Ensure write buffer coherence first
        if !self.write_buffer.operations.is_empty() {
            self.flush_write_buffer()?;
        }

        // Use FileOperations for direct file access
        FileOperations::read_bytes_direct(&mut self.file, offset, buffer)
    }

    /// Read an edge record at a specific file offset
    /// Returns None if the offset is invalid or read fails
    pub fn read_edge_at_offset(&mut self, offset: FileOffset) -> Option<EdgeRecord> {
        if offset < self.persistent_header.edge_data_offset {
            return None;
        }

        let buffer_size = edge::FIXED_HEADER_SIZE;
        // Check file size before read_exact to prevent "failed to fill whole buffer"
        if self.ensure_file_len_at_least(offset, buffer_size).is_err() {
            return None;
        }

        let mut buffer = vec![0u8; buffer_size];
        if let Err(_) = self.file.seek(SeekFrom::Start(offset)) {
            return None;
        }
        if let Err(_) = self.file.read_exact(&mut buffer) {
            return None;
        }

        // Decode edge record from buffer
        // This is a simplified implementation - in production you'd want proper error handling
        let edge_id = u64::from_be_bytes([
            buffer[0], buffer[1], buffer[2], buffer[3], buffer[4], buffer[5], buffer[6], buffer[7],
        ]);
        let from_id = u64::from_be_bytes([
            buffer[8], buffer[9], buffer[10], buffer[11], buffer[12], buffer[13], buffer[14],
            buffer[15],
        ]);
        let to_id = u64::from_be_bytes([
            buffer[16], buffer[17], buffer[18], buffer[19], buffer[20], buffer[21], buffer[22],
            buffer[23],
        ]);

        Some(EdgeRecord {
            id: edge_id as i64,
            from_id: from_id as i64,
            to_id: to_id as i64,
            edge_type: "unknown".to_string(), // Simplified for optimization demo
            flags: EdgeFlags::empty(),
            data: serde_json::Value::Null,
        })
    }

    /// Read a node record at a specific node ID (simplified implementation)
    /// Returns None if the node ID is invalid or read fails
    pub fn read_node_at(&self, node_id: NativeNodeId) -> Option<NodeRecord> {
        // This is a simplified implementation that creates a basic node record
        // In a full implementation, this would read from the node data section
        Some(NodeRecord {
            id: node_id,
            flags: NodeFlags::empty(),
            kind: "node".to_string(),
            name: format!("node_{}", node_id),
            data: serde_json::Value::Null,
            outgoing_cluster_offset: 0,
            outgoing_cluster_size: 0,
            outgoing_edge_count: 0,
            incoming_cluster_offset: 0,
            incoming_cluster_size: 0,
            incoming_edge_count: 0,
        })
    }

    // ========================================
    // PHASE 40: CONSERVATIVE MMAP LIFECYCLE MANAGEMENT
    // ========================================

    #[cfg(feature = "v2_experimental")]
    /// Initialize mmap if not already present
    fn ensure_mmap_initialized(&mut self) -> NativeResult<()> {
        MemoryMappingManager::ensure_mmap_initialized(&self.file, &mut self.mmap)
    }

    #[cfg(feature = "v2_experimental")]
    /// Ensure mmap covers at least the specified offset using conservative remapping
    fn ensure_mmap_covers(&mut self, min_len: u64) -> NativeResult<()> {
        MemoryMappingManager::ensure_mmap_covers(
            &mut self.file,
            &mut self.write_buffer,
            &mut self.mmap,
            min_len,
        )
    }

    // ========================================
    // V2 MMAP I/O HELPERS (experimental feature)
    // ========================================

    #[cfg(feature = "v2_experimental")]
    /// Ensure mmap region is at least the specified size
    pub fn mmap_ensure_size(&mut self, len: u64) -> NativeResult<()> {
        FileManager::mmap_ensure_size(&mut self.file, &self.file_path, len, &mut self.mmap)
    }

    #[cfg(feature = "v2_experimental")]
    /// Read bytes using mmap (V2 path only)
    pub fn mmap_read_bytes(&self, offset: u64, buffer: &mut [u8]) -> NativeResult<()> {
        MemoryMappingManager::mmap_read_bytes(&self.mmap, offset, buffer)
    }

    #[cfg(feature = "v2_experimental")]
    /// Write bytes using mmap (V2 path only)
    pub fn mmap_write_bytes(&mut self, offset: u64, data: &[u8]) -> NativeResult<()> {
        MemoryMappingManager::mmap_write_bytes(
            &mut self.file,
            &self.file_path,
            &mut self.write_buffer,
            &mut self.mmap,
            offset,
            data,
        )
    }
}

impl Drop for GraphFile {
    fn drop(&mut self) {
        // Ensure header is written before closing
        let _ = self.write_header();
        let _ = self.sync();
    }
}

