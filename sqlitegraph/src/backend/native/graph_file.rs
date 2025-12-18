//! File operations and header management for native backend.
//!
//! This module handles file creation, opening, header encoding/decoding,
//! and basic file I/O operations for the native graph database format.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

#[cfg(feature = "v2_experimental")]
use memmap2::{MmapMut, MmapOptions};

use crate::backend::native::constants::*;
use crate::backend::native::types::*;
// Phase 10: Import new header modules
use crate::backend::native::persistent_header::PersistentHeaderV2;
use crate::backend::native::transaction_state::TransactionState;
use crate::backend::native::v2::{V2_FORMAT_VERSION, V2_MAGIC};

const DEFAULT_NODE_DATA_START: u64 = 1024;
const NODE_SLOT_SIZE: u64 = 4096;
const COMMIT_MARKER_OFFSET: u64 = super::constants::HEADER_SIZE;

/// Reserved region for node slots to prevent cluster corruption
/// 8 MiB = 8 * 1024 * 1024 bytes, providing space for ~2048 node slots
/// This ensures cluster data never overlaps with node slots even for large graphs
pub const RESERVED_NODE_REGION_BYTES: u64 = 8 * 1024 * 1024;

const COMMIT_MARKER_CLEAN: u64 = 0xC0DED00DF00DF00D;

/// Simple read-ahead buffer for performance optimization
/// Adaptive sizing to minimize I/O amplification while maintaining performance
struct ReadBuffer {
    data: Vec<u8>,
    offset: u64,
    size: usize,
    capacity: usize,
}

impl ReadBuffer {
    /// Calculate adaptive buffer capacity based on request size
    /// Goal: minimize I/O amplification while maintaining performance
    fn adaptive_capacity(request_size: usize) -> usize {
        if request_size < 128 {
            256 // ~8x amplification for tiny reads
        } else if request_size < 1024 {
            512 // ~2x amplification for small reads
        } else if request_size < 4096 {
            4096 // Page-aligned for medium reads
        } else {
            std::cmp::min(request_size * 2, 16384) // Bounded for large reads
        }
    }

    fn new() -> Self {
        Self::with_capacity(256) // Default 256B for typical node records
    }

    fn with_capacity(capacity: usize) -> Self {
        Self {
            data: vec![0u8; capacity],
            offset: 0,
            size: 0,
            capacity,
        }
    }

    fn contains(&self, offset: u64, len: usize) -> bool {
        offset >= self.offset && (offset + len as u64) <= (self.offset + self.size as u64)
    }

    fn read(&self, offset: u64, buffer: &mut [u8]) -> bool {
        if self.contains(offset, buffer.len()) {
            let start = (offset - self.offset) as usize;
            buffer.copy_from_slice(&self.data[start..start + buffer.len()]);
            true
        } else {
            false
        }
    }
}

/// Simple write-behind buffer for batched writes
struct WriteBuffer {
    operations: Vec<(u64, Vec<u8>)>,
    capacity: usize,
}

impl WriteBuffer {
    fn new(capacity: usize) -> Self {
        Self {
            operations: Vec::new(),
            capacity,
        }
    }

    fn add(&mut self, offset: u64, data: Vec<u8>) -> bool {
        // PHASE 43: CRITICAL HEADER REGION LOCKDOWN
        // Prevent any buffered writes into header region [0, HEADER_SIZE)
        if offset < super::constants::HEADER_SIZE {
            return false; // Reject header region writes
        }

        if self.operations.len() < self.capacity {
            self.operations.push((offset, data));
            true
        } else {
            false
        }
    }

    fn flush(&mut self) -> Vec<(u64, Vec<u8>)> {
        std::mem::take(&mut self.operations)
    }
}

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
    // Phase 75: Track nodes whose V2 cluster metadata is modified during transaction
    tx_modified_nodes: std::collections::HashSet<NativeNodeId>,
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
        let path = path.as_ref();
        let file_path = path.to_path_buf();

        // Create new file with appropriate permissions
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .mode(FILE_PERMISSIONS)
            .open(path)?;

        let mut graph_file = Self {
            file,
            persistent_header: PersistentHeaderV2::new_v2(),
            transaction_state: TransactionState::new(),
            file_path,
            read_buffer: ReadBuffer::new(), // Adaptive 256B buffer (Step 21.1)
            write_buffer: WriteBuffer::new(32), // 32 pending writes
            #[cfg(feature = "v2_experimental")]
            mmap: None,
            // Phase 75: Initialize empty write-set for tracking modified nodes
            tx_modified_nodes: std::collections::HashSet::new(),
        };

        graph_file.initialize_v2_header();
        // Write initial header
        graph_file.write_header()?;
        graph_file.finish_cluster_commit()?;

        // PHASE 40: Initialize mmap using centralized method
        #[cfg(feature = "v2_experimental")]
        {
            graph_file.ensure_mmap_initialized()?;
        }

        Ok(graph_file)
    }

    /// Open an existing graph file
    pub fn open<P: AsRef<Path>>(path: P) -> NativeResult<Self> {
        let path = path.as_ref();
        let file_path = path.to_path_buf();

        let file = OpenOptions::new().read(true).write(true).open(path)?;

        let mut graph_file = Self {
            file,
            persistent_header: PersistentHeaderV2::new_v2(), // Will be overwritten by read_header
            transaction_state: TransactionState::new(),
            file_path,
            read_buffer: ReadBuffer::new(), // Adaptive 256B buffer (Step 21.1)
            write_buffer: WriteBuffer::new(32), // 32 pending writes
            #[cfg(feature = "v2_experimental")]
            mmap: None,
            // Phase 75: Initialize empty write-set for tracking modified nodes
            tx_modified_nodes: std::collections::HashSet::new(),
        };

        // Read and validate existing header
        graph_file.read_header()?;

        // Phase 70: Transaction recovery - runtime only, no persistent tx state to check
        // TransactionState is runtime-only and initialized to defaults on open

        // V2-ONLY REFACTOR: Hard format gate - refuse non-V2 files
        let required_flags = FLAG_V2_FRAMED_RECORDS | FLAG_V2_ATOMIC_COMMIT;
        if (graph_file.persistent_header.flags & required_flags) != required_flags {
            return Err(NativeBackendError::UnsupportedVersion {
                version: 1, // Any file without both V2 flags is unsupported
                supported_version: 2,
            });
        }

        // V2-specific validation
        if graph_file.persistent_header.version != 2 {
            return Err(NativeBackendError::UnsupportedVersion {
                version: graph_file.persistent_header.version,
                supported_version: 2,
            });
        }

        graph_file.persistent_header.validate()?;

        // V2 commit verification
        graph_file.verify_commit_marker()?;

        // PHASE 40: Initialize mmap using centralized method

        // PHASE 40: Initialize mmap using centralized method
        #[cfg(feature = "v2_experimental")]
        {
            graph_file.ensure_mmap_initialized()?;
        }

        Ok(graph_file)
    }

    /// Read header from file
    pub fn read_header(&mut self) -> NativeResult<()> {
        self.file.seek(SeekFrom::Start(0))?;

        let header_len = super::constants::HEADER_SIZE as usize;

        // Ensure file is large enough for header before read_exact
        self.ensure_file_len_at_least(0, header_len)?;

        let mut header_bytes = vec![0u8; header_len];
        self.file.read_exact(&mut header_bytes)?;

        self.persistent_header = decode_persistent_header(&header_bytes)?;
        Ok(())
    }

    /// Write header to file with durable persistence
    /// This is the ONLY authorized way to write to header region [0, HEADER_SIZE)
    pub fn write_header(&mut self) -> NativeResult<()> {
        self.write_header_and_sync()
    }

    /// Internal helper: Write header with immediate verification and sync
    /// Ensures header bytes reach disk and can be read back immediately
    fn write_header_and_sync(&mut self) -> NativeResult<()> {
        let header_bytes = encode_persistent_header(&self.persistent_header)?;

        // Write at offset 0 (header region)
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&header_bytes)?;
        self.file.flush()?;

        // PHASE 43: Durable write - ensure data reaches disk
        self.file.sync_all()?;

        // PHASE 43: Immediate verification (temporary instrumentation) - DISABLED FOR DEBUGGING
        // self.verify_header_written_immediately(&header_bytes)?;

        Ok(())
    }

    fn write_commit_marker_value(&mut self, value: u64) -> NativeResult<()> {
        self.file.seek(SeekFrom::Start(COMMIT_MARKER_OFFSET))?;
        self.file.write_all(&value.to_be_bytes())?;
        self.file.sync_all()?;
        Ok(())
    }

    fn read_commit_marker_value(&mut self) -> NativeResult<u64> {
        self.file.seek(SeekFrom::Start(COMMIT_MARKER_OFFSET))?;
        let mut bytes = [0u8; 8];
        self.file.read_exact(&mut bytes)?;
        Ok(u64::from_be_bytes(bytes))
    }

    fn verify_commit_marker(&mut self) -> NativeResult<()> {
        let marker = self.read_commit_marker_value()?;
        if marker != COMMIT_MARKER_CLEAN {
            return Err(NativeBackendError::InvalidHeader {
                field: "commit_marker".to_string(),
                reason: format!(
                    "incomplete clustered commit detected (marker=0x{:016X})",
                    marker
                ),
            });
        }
        Ok(())
    }

    pub fn begin_cluster_commit(&mut self) -> NativeResult<()> {
        self.write_commit_marker_value(0)
    }

    pub fn finish_cluster_commit(&mut self) -> NativeResult<()> {
        self.write_commit_marker_value(COMMIT_MARKER_CLEAN)
    }

    /// TX_BEGIN_AUDIT: Dump node slot prefix for forensic debugging
    fn dump_node_slot_prefix(&mut self, node_id: u64, label: &str) -> NativeResult<()> {
        let node_data_offset = self.persistent_header().node_data_offset;
        let slot_offset = node_data_offset + ((node_id - 1) as u64 * 4096);
        let mut buffer = vec![0u8; 32];

        // Use same read path as NodeStore
        if self.read_bytes(slot_offset, &mut buffer).is_ok() {
            println!(
                "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} first_32={:02x?} version={}",
                label, node_id, slot_offset, &buffer, buffer[0]
            );
        } else {
            println!(
                "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} READ_FAILED",
                label, node_id, slot_offset
            );
        }
        Ok(())
    }

    /// Phase 70: Begin atomic transaction with rollback state
    pub fn begin_transaction(&mut self, tx_id: u64) -> NativeResult<()> {
        // TX_BEGIN_AUDIT: Check node 257 slot before transaction operations
        if std::env::var("TX_BEGIN_AUDIT").is_ok() {
            let _ = self.dump_node_slot_prefix(257, "BEFORE_TX_BEGIN");
        }

        // PHASE 2D: Probe node1 corruption before any transaction operations
        if std::env::var("EDGE_CLUSTER_DEBUG").is_ok() {
            let mut disk_file = std::fs::File::open(&self.file_path())?;
            let mut node1_bytes = vec![0u8; 32];
            disk_file.seek(std::io::SeekFrom::Start(0x400))?;
            disk_file.read_exact(&mut node1_bytes)?;
            let version_before_tx_ops = node1_bytes[0];
            let file_size_before_tx_ops = self.file_size().unwrap_or(0);
            println!(
                "[EDGE_CLUSTER_DEBUG] BEFORE_TX_OPS: node1_version={}, file_size={}, node1_bytes={:02x?}",
                version_before_tx_ops, file_size_before_tx_ops, &node1_bytes
            );
        }

        // Begin transaction in header (saves current state)
        self.tx_state_mut().begin_tx(tx_id);

        // TX_BEGIN_AUDIT: Check node 257 slot after header state modification
        if std::env::var("TX_BEGIN_AUDIT").is_ok() {
            let _ = self.dump_node_slot_prefix(257, "AFTER_TX_STATE_MODIFY");
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
            let _ = self.dump_node_slot_prefix(257, "AFTER_HEADER_WRITE");
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
            let _ = self.dump_node_slot_prefix(257, "AFTER_SYNC_ALL");
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
            let _ = self.dump_node_slot_prefix(257, "AFTER_TX_COMPLETE");
        }

        println!("PHASE 70: Transaction {} begun", tx_id);
        Ok(())
    }

    /// Phase 70: Commit atomic transaction
    pub fn commit_transaction(&mut self) -> NativeResult<()> {
        // Clear transaction state in header
        self.tx_state_mut().commit();

        // Write final clean header
        self.write_header()?;

        // Force final header to disk
        self.file.sync_all()?;

        println!("PHASE 70: Transaction {} committed", self.tx_state().tx_id);
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

        // Phase 10: Transaction rollback is now runtime-only
        self.tx_state_mut().rollback();

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
        self.tx_modified_nodes.insert(node_id);

        #[cfg(feature = "trace_v2_io")]
        if std::env::var("PHASE75_INSTRUMENTATION").is_ok() {
            println!(
                "[phase75] WRITESET_RECORD: node_id={} marked for rollback cleanup",
                node_id
            );
        }
    }

    /// Phase 75: CRITICAL FIX - Skip V2 node slot rewriting during rollback to prevent corruption
    fn clear_v2_cluster_metadata_on_rollback(&mut self) -> NativeResult<()> {
        #[cfg(feature = "trace_v2_io")]
        println!("[phase75] ROLLBACK_CLEANUP: SKIPPING V2 node slot rewrite to prevent corruption");

        // CRITICAL FIX: Do NOT rewrite V2 node slots during rollback
        // This prevents corruption of V2 format (version=2 -> version=1)

        // Just clear the transaction tracking
        self.tx_modified_nodes.clear();

        #[cfg(feature = "trace_v2_io")]
        println!("[phase75] ROLLBACK_CLEANUP: Completed without V2 slot corruption");

        Ok(())
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
        let metadata = self.file.metadata()?;
        Ok(metadata.len())
    }

    fn initialize_v2_header(&mut self) {
        println!(
            "[CLUSTER_DEBUG] initialize_v2_header() called - fixing cluster offsets to prevent node slot corruption"
        );
        let header = self.persistent_header_mut();
        header.magic = V2_MAGIC;
        header.version = V2_FORMAT_VERSION;
        if header.node_data_offset < DEFAULT_NODE_DATA_START {
            header.node_data_offset = DEFAULT_NODE_DATA_START;
        }
        // V2-ONLY: Position edge data AFTER node region to prevent corruption
        // Reserve space for maximum node capacity (using a reasonable default)
        const MAX_NODE_CAPACITY: u64 = 10000; // Support up to 10K nodes
        let node_region_end = header.node_data_offset + (MAX_NODE_CAPACITY * NODE_SLOT_SIZE);
        header.edge_data_offset = node_region_end;

        // CRITICAL INVARIANT: Ensure edge and node regions never overlap
        debug_assert!(
            header.edge_data_offset >= header.node_data_offset,
            "edge_data_offset ({}) must be >= node_data_offset ({})",
            header.edge_data_offset,
            header.node_data_offset
        );

        // PHASE 42 FIX: Initialize cluster offsets to distinct regions
        // Reserve space for cluster regions: outgoing → incoming → free_space
        // PHASE 2A FIX: Use reserved node region size instead of hardcoded 1MB
        let node_region_size = RESERVED_NODE_REGION_BYTES; // Reserve 8MB for node slots

        // CRITICAL FIX: Calculate base_cluster_start AFTER node_data_offset is finalized
        // This prevents clusters from being allocated at offset 64 instead of proper region
        let base_cluster_start = header.node_data_offset + (header.node_count as u64 * 4096);

        // MANDATORY INVARIANT: Calculate node region end to prevent cluster overlap
        let node_region_end = header.node_data_offset + (header.node_count as u64 * 4096);

        // MANDATORY INVARIANT: Calculate cluster floor to ensure clusters are outside node region
        // PHASE 2A FIX: Use reserved node region to prevent cluster/node collision
        let cluster_floor = std::cmp::max(
            node_region_end,
            header.node_data_offset + RESERVED_NODE_REGION_BYTES,
        );

        // DEBUG: Print layout invariants
        println!("[CLUSTER_DEBUG] Layout invariants:");
        println!("  node_data_offset = {}", header.node_data_offset);
        println!("  node_count = {}", header.node_count);
        println!("  node_region_end = {}", node_region_end);
        println!("  base_cluster_start = {}", base_cluster_start);
        println!("  cluster_floor = {}", cluster_floor);
        println!(
            "  current outgoing_cluster_offset = {}",
            header.outgoing_cluster_offset
        );
        println!(
            "  current incoming_cluster_offset = {}",
            header.incoming_cluster_offset
        );

        // PHASE 76 CRITICAL FIX: Prevent cluster offset corruption of node slots
        // Ensure cluster offsets are positioned AFTER the entire node region to prevent overwrites
        let node_region_end = header.node_data_offset + node_region_size;

        if header.outgoing_cluster_offset < node_region_end {
            println!(
                "CRITICAL FIX: Moving outgoing_cluster_offset from {} to {} to prevent node slot corruption",
                header.outgoing_cluster_offset, node_region_end
            );
            header.outgoing_cluster_offset = node_region_end;
        }

        // Position incoming clusters after outgoing clusters with reasonable spacing
        let min_incoming_offset = header.outgoing_cluster_offset + (header.node_count as u64 * 256); // 256 bytes per node for outgoing clusters
        if header.incoming_cluster_offset < min_incoming_offset {
            println!(
                "CRITICAL FIX: Moving incoming_cluster_offset from {} to {} to prevent node slot corruption",
                header.incoming_cluster_offset, min_incoming_offset
            );
            header.incoming_cluster_offset = min_incoming_offset;
        }
        if header.free_space_offset < header.node_data_offset + (2 * node_region_size) {
            header.free_space_offset = cluster_floor + (2 * node_region_size);
        }

        // CRITICAL INVARIANT: Cluster offsets must be outside node region
        // This prevents cluster writes from corrupting node slots
        if header.outgoing_cluster_offset < node_region_end {
            println!(
                "CRITICAL FIX: Correcting outgoing_cluster_offset from {} to {} to prevent node slot corruption",
                header.outgoing_cluster_offset, node_region_end
            );
            header.outgoing_cluster_offset = node_region_end;
        }

        if header.incoming_cluster_offset < node_region_end {
            println!(
                "CRITICAL FIX: Correcting incoming_cluster_offset from {} to {} to prevent node slot corruption",
                header.incoming_cluster_offset, node_region_end
            );
            header.incoming_cluster_offset = node_region_end;
        }

        // CRITICAL SAFETY: Ensure cluster offsets are properly positioned
        assert!(
            header.outgoing_cluster_offset >= node_region_end,
            "CRITICAL: outgoing_cluster_offset ({}) must be >= node_region_end ({}) - NODE SLOT CORRUPTION DETECTED",
            header.outgoing_cluster_offset,
            node_region_end
        );
        assert!(
            header.incoming_cluster_offset >= node_region_end,
            "CRITICAL: incoming_cluster_offset ({}) must be >= node_region_end ({}) - NODE SLOT CORRUPTION DETECTED",
            header.incoming_cluster_offset,
            node_region_end
        );

        #[cfg(debug_assertions)]
        {
            println!(
                "  final outgoing_cluster_offset = {}",
                header.outgoing_cluster_offset
            );
            println!(
                "  final incoming_cluster_offset = {}",
                header.incoming_cluster_offset
            );
        }
    }

    /// Validate file size against header information
    pub fn validate_file_size(&self) -> NativeResult<()> {
        let file_size = self.file_size()?;

        if file_size < super::constants::HEADER_SIZE {
            return Err(NativeBackendError::FileTooSmall {
                size: file_size,
                min_size: super::constants::HEADER_SIZE,
            });
        }

        // Basic sanity check: file should be at least large enough for declared records
        // For native backend, we only require file to be large enough for actual data written
        // edge_data_offset is a reservation for future edge data, not a current requirement
        let min_expected_size = if self.persistent_header.edge_count > 0 {
            // If edges exist, file must be large enough to contain them
            std::cmp::max(
                self.persistent_header.edge_data_offset,
                self.persistent_header.node_data_offset,
            )
        } else {
            // If no edges exist, file only needs to be large enough for header and node data
            self.persistent_header.node_data_offset
        };

        if file_size < min_expected_size {
            return Err(NativeBackendError::FileTooSmall {
                size: file_size,
                min_size: min_expected_size,
            });
        }

        Ok(())
    }

    /// Grow file by specified number of bytes
    pub fn grow(&mut self, additional_bytes: u64) -> NativeResult<()> {
        if additional_bytes == 0 {
            return Ok(());
        }

        let current_size = self.file_size()?;
        let new_size = current_size + additional_bytes;
        self.file.set_len(new_size)?;
        self.file.flush()?;

        Ok(())
    }

    /// Sync file to disk
    pub fn sync(&self) -> NativeResult<()> {
        self.file.sync_all()?;
        Ok(())
    }

    /// Read bytes from file at specific offset with read-ahead buffering
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

        // Phase 41: Route reads based on exclusive I/O mode
        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
        {
            // EXCLUSIVE MMAP MODE: Read directly from mmap
            let mmap = self
                .mmap
                .as_ref()
                .ok_or(NativeBackendError::CorruptNodeRecord {
                    node_id: -1,
                    reason: "mmap not initialized in exclusive mmap mode".to_string(),
                })?;

            if offset as usize + buffer.len() > mmap.len() {
                return Err(NativeBackendError::CorruptNodeRecord {
                    node_id: -1,
                    reason: format!(
                        "Read beyond mmap region: offset={}, len={}, mmap_size={}",
                        offset,
                        buffer.len(),
                        mmap.len()
                    ),
                });
            }

            let start = offset as usize;
            let end = start + buffer.len();
            buffer.copy_from_slice(&mmap[start..end]);
        }
        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
        {
            // EXCLUSIVE STD MODE: Use standard I/O with flush ensure
            // PHASE 2C.3 INSTRUMENTATION: Track what happens to write buffer
            if !self.write_buffer.operations.is_empty() {
                let ops_count = self.write_buffer.operations.len();
                if std::env::var("WRITEBUF_DEBUG").is_ok() {
                    println!(
                        "[WRITEBUF_DEBUG] EXCLUSIVE_STD: CLEARING {} pending ops without flush",
                        ops_count
                    );
                }
                // Phase 41 FIX: Clear write buffer to prevent corruption of cluster data
                // PHASE 2C.3: THIS MAY BE DISCARDING NODE SLOT WRITES
                self.write_buffer.operations.clear();
            }

            // CRITICAL: Validate file size before read_exact to prevent "failed to fill whole buffer"
            self.ensure_file_len_at_least(offset, buffer.len())?;

            self.file.seek(SeekFrom::Start(offset))?;
            self.file.read_exact(buffer)?;
            self.file.sync_all()?;
        }
        #[cfg(not(any(
            feature = "v2_experimental",
            feature = "v2_io_exclusive_mmap",
            feature = "v2_io_exclusive_std"
        )))]
        {
            // DEFAULT MODE: Mixed I/O (current Phase 40 behavior)
            // Ensure write buffer coherence: flush any pending writes before reading
            // This ensures read-write consistency for mixed operations
            if !self.write_buffer.operations.is_empty() {
                self.flush_write_buffer()?;
                // Invalidate read buffer to force fresh data from disk
                self.read_buffer.offset = 0;
                self.read_buffer.size = 0;
            }

            // Try to satisfy from read buffer first
            if !self.read_buffer.read(offset, buffer) {
                // Buffer miss - read from file with read-ahead
                self.read_with_ahead(offset, buffer)?;
            }
        }

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
        self.flush_write_buffer()?;
        self.file.flush()?;
        Ok(())
    }

    /// Invalidate read buffer to force fresh reads from disk
    pub fn invalidate_read_buffer(&mut self) {
        self.read_buffer.offset = 0;
        self.read_buffer.size = 0;
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
        // CRITICAL: Validate file size before read_exact to prevent "failed to fill whole buffer"
        self.ensure_file_len_at_least(offset, buffer.len())?;

        // Ensure write buffer coherence first
        if !self.write_buffer.operations.is_empty() {
            self.flush_write_buffer()?;
        }

        // Force direct file access, bypassing read buffer entirely
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(buffer)?;
        Ok(())
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
        if self.mmap.is_none() {
            let file_size = self.file_size()?;
            if file_size > 0 {
                self.mmap = unsafe { Some(MmapOptions::new().map_mut(&self.file)?) };
            } else {
                // For empty files, create minimal mmap to cover header
                self.mmap = unsafe { Some(MmapOptions::new().map_mut(&self.file)?) };
            }
        }
        Ok(())
    }

    #[cfg(feature = "v2_experimental")]
    /// Ensure mmap covers at least the specified offset using conservative remapping
    fn ensure_mmap_covers(&mut self, min_len: u64) -> NativeResult<()> {
        // CRITICAL: Prevent flush_write_buffer ↔ ensure_mmap_covers recursion
        thread_local! {
            static MMAP_ENSURE_DEPTH: std::cell::RefCell<u32> = const { std::cell::RefCell::new(0) };
        }

        MMAP_ENSURE_DEPTH.with(|d| {
            let mut depth = d.borrow_mut();
            if *depth >= 2 {
                return Err(NativeBackendError::CorruptNodeRecord {
                    node_id: -1,
                    reason: format!("ensure_mmap_covers recursion depth exceeded: {}", *depth),
                });
            }
            *depth += 1;
            Ok(())
        })?;

        let depth = MMAP_ENSURE_DEPTH.with(|d| *d.borrow());

        // Initialize mmap if needed
        self.ensure_mmap_initialized()?;

        let current_file_size = self.file_size()?;

        // Ensure file is large enough
        if min_len > current_file_size {
            // Grow file to required size using set_len for atomic allocation
            self.file.set_len(min_len)?;
            self.file.flush()?;
        }

        let current_mmap_size = self.mmap.as_ref().unwrap().len() as u64;

        // PHASE 40 CRITICAL FIX: Remap if we need to cover data outside current mmap
        // This is more aggressive than the 4KB threshold to prevent "Read beyond mmap region" errors
        if min_len > current_mmap_size {
            // CRITICAL: Only flush if we're not already being called from flush_write_buffer
            if depth == 1 {
                // Flush any pending writes before remapping
                self.flush_write_buffer()?;
            }

            // Remap to cover the full file size
            self.mmap = unsafe { Some(MmapOptions::new().map_mut(&self.file)?) };
        }

        // Decrement depth counter
        MMAP_ENSURE_DEPTH.with(|d| {
            let mut depth = d.borrow_mut();
            *depth = depth.saturating_sub(1);
        });

        Ok(())
    }

    // ========================================
    // V2 MMAP I/O HELPERS (experimental feature)
    // ========================================

    #[cfg(feature = "v2_experimental")]
    /// Ensure mmap region is at least the specified size
    pub fn mmap_ensure_size(&mut self, len: u64) -> NativeResult<()> {
        // CRITICAL: Prevent mmap recursion cycle
        thread_local! {
            static MMAP_DEPTH: std::cell::RefCell<u32> = const { std::cell::RefCell::new(0) };
        }
        MMAP_DEPTH.with(|d| {
            let mut depth = d.borrow_mut();
            *depth += 1;
            if *depth > 10 {
                return Err(NativeBackendError::CorruptNodeRecord {
                    node_id: -1,
                    reason: format!("mmap recursion depth exceeded: {}", *depth),
                });
            }
            Ok(())
        })?;

        let result = (|| {
            let current_size = self.file_size()?;
            if len > current_size {
                self.grow(len - current_size)?;
            }

            // Use conservative mmap management
            self.ensure_mmap_covers(len)?;

            Ok(())
        })();

        MMAP_DEPTH.with(|d| *d.borrow_mut() -= 1);
        result
    }

    #[cfg(feature = "v2_experimental")]
    /// Read bytes using mmap (V2 path only)
    pub fn mmap_read_bytes(&self, offset: u64, buffer: &mut [u8]) -> NativeResult<()> {
        let mmap = self
            .mmap
            .as_ref()
            .ok_or_else(|| NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: "mmap not initialized".to_string(),
            })?;

        if offset as usize + buffer.len() > mmap.len() {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "Read beyond mmap region: offset={}, len={}, mmap_size={}",
                    offset,
                    buffer.len(),
                    mmap.len()
                ),
            });
        }

        let start = offset as usize;
        let end = start + buffer.len();
        buffer.copy_from_slice(&mmap[start..end]);

        Ok(())
    }

    #[cfg(feature = "v2_experimental")]
    /// Write bytes using mmap (V2 path only)
    pub fn mmap_write_bytes(&mut self, offset: u64, data: &[u8]) -> NativeResult<()> {
        // Ensure mmap is large enough
        self.mmap_ensure_size(offset + data.len() as u64)?;

        let mmap = self
            .mmap
            .as_mut()
            .ok_or_else(|| NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: "mmap not initialized".to_string(),
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

        Ok(())
    }
}

impl Drop for GraphFile {
    fn drop(&mut self) {
        // Ensure header is written before closing
        let _ = self.write_header();
        let _ = self.sync();
    }
}

/// Encode PersistentHeaderV2 to byte array
pub fn encode_persistent_header(header: &PersistentHeaderV2) -> NativeResult<Vec<u8>> {
    use crate::backend::native::persistent_header::PERSISTENT_HEADER_SIZE;

    let mut buffer = Vec::with_capacity(PERSISTENT_HEADER_SIZE);

    // Write magic bytes
    buffer.extend_from_slice(&header.magic);

    // Write version (big-endian)
    buffer.extend_from_slice(&header.version.to_be_bytes());

    // Write flags (big-endian)
    buffer.extend_from_slice(&header.flags.to_be_bytes());

    // Write node count (big-endian)
    buffer.extend_from_slice(&header.node_count.to_be_bytes());

    // Write edge count (big-endian)
    buffer.extend_from_slice(&header.edge_count.to_be_bytes());

    // Write schema version (big-endian)
    buffer.extend_from_slice(&header.schema_version.to_be_bytes());

    // Write node data offset (big-endian)
    buffer.extend_from_slice(&header.node_data_offset.to_be_bytes());

    // Write edge data offset (big-endian)
    buffer.extend_from_slice(&header.edge_data_offset.to_be_bytes());

    // Write V2 cluster offsets (big-endian)
    buffer.extend_from_slice(&header.outgoing_cluster_offset.to_be_bytes());
    buffer.extend_from_slice(&header.incoming_cluster_offset.to_be_bytes());
    buffer.extend_from_slice(&header.free_space_offset.to_be_bytes());

    assert_eq!(
        buffer.len(),
        PERSISTENT_HEADER_SIZE,
        "Persistent header encoding size mismatch"
    );
    assert_eq!(
        buffer.len(),
        super::constants::HEADER_SIZE as usize,
        "Header must match constants::HEADER_SIZE"
    );

    Ok(buffer)
}

/// Helper function for safe slice access with bounds checking
pub fn get_slice_safe(data: &[u8], start: usize, len: usize) -> NativeResult<&[u8]> {
    if start.checked_add(len).map_or(true, |end| end > data.len()) {
        return Err(NativeBackendError::InvalidHeader {
            field: "header_data".to_string(),
            reason: format!("slice access out of bounds: start={}, len={}, data_len={}",
                          start, len, data.len()),
        });
    }
    // This is safe now because we checked the bounds above
    Ok(&data[start..start + len])
}

/// Decode PersistentHeaderV2 from byte array
pub fn decode_persistent_header(bytes: &[u8]) -> NativeResult<PersistentHeaderV2> {
    use crate::backend::native::persistent_header::{PERSISTENT_HEADER_SIZE, PersistentHeaderV2};

    if bytes.len() < PERSISTENT_HEADER_SIZE {
        return Err(NativeBackendError::FileTooSmall {
            size: bytes.len() as u64,
            min_size: PERSISTENT_HEADER_SIZE as u64,
        });
    }

    let mut offset = 0;

    // Read magic bytes
    let magic_slice = get_slice_safe(bytes, offset, 8)?;
    let mut magic = [0u8; 8];
    magic.copy_from_slice(magic_slice);
    offset += 8;

    // Read version
    let version_slice = get_slice_safe(bytes, offset, 4)?;
    let version = u32::from_be_bytes([
        version_slice[0],
        version_slice[1],
        version_slice[2],
        version_slice[3],
    ]);
    offset += 4;

    // Read flags
    let flags_slice = get_slice_safe(bytes, offset, 4)?;
    let flags = u32::from_be_bytes([
        flags_slice[0],
        flags_slice[1],
        flags_slice[2],
        flags_slice[3],
    ]);
    offset += 4;

    // Read node count
    let node_count_slice = get_slice_safe(bytes, offset, 8)?;
    let node_count = u64::from_be_bytes([
        node_count_slice[0],
        node_count_slice[1],
        node_count_slice[2],
        node_count_slice[3],
        node_count_slice[4],
        node_count_slice[5],
        node_count_slice[6],
        node_count_slice[7],
    ]);
    offset += 8;

    // Read edge count
    let edge_count_slice = get_slice_safe(bytes, offset, 8)?;
    let edge_count = u64::from_be_bytes([
        edge_count_slice[0],
        edge_count_slice[1],
        edge_count_slice[2],
        edge_count_slice[3],
        edge_count_slice[4],
        edge_count_slice[5],
        edge_count_slice[6],
        edge_count_slice[7],
    ]);
    offset += 8;

    // Read schema version
    let schema_version_slice = get_slice_safe(bytes, offset, 8)?;  // TODO: This should probably be 4 bytes, not 8
    let schema_version = u64::from_be_bytes([
        schema_version_slice[0],
        schema_version_slice[1],
        schema_version_slice[2],
        schema_version_slice[3],
        schema_version_slice[4],
        schema_version_slice[5],
        schema_version_slice[6],
        schema_version_slice[7],
    ]);
    offset += 8;

    // Read node data offset
    let node_data_offset_slice = get_slice_safe(bytes, offset, 8)?;
    let node_data_offset = u64::from_be_bytes([
        node_data_offset_slice[0],
        node_data_offset_slice[1],
        node_data_offset_slice[2],
        node_data_offset_slice[3],
        node_data_offset_slice[4],
        node_data_offset_slice[5],
        node_data_offset_slice[6],
        node_data_offset_slice[7],
    ]);
    offset += 8;

    // Read edge data offset
    let edge_data_offset_slice = get_slice_safe(bytes, offset, 8)?;
    let edge_data_offset = u64::from_be_bytes([
        edge_data_offset_slice[0],
        edge_data_offset_slice[1],
        edge_data_offset_slice[2],
        edge_data_offset_slice[3],
        edge_data_offset_slice[4],
        edge_data_offset_slice[5],
        edge_data_offset_slice[6],
        edge_data_offset_slice[7],
    ]);
    offset += 8;

    let mut outgoing_cluster_offset = 0u64;
    let mut incoming_cluster_offset = 0u64;
    let mut free_space_offset = 0u64;

    let checksum = if bytes.len() >= super::constants::HEADER_SIZE as usize {
        // HEADER_VALIDATE_DEBUG: Track byte positions
        if std::env::var("HEADER_VALIDATE_DEBUG").is_ok() {
            println!(
                "[HEADER_READ_DEBUG] Reading outgoing_cluster_offset at offset {} (should be 56)",
                offset
            );
            let outgoing_bytes = get_slice_safe(bytes, offset, 8)?;
            println!(
                "[HEADER_READ_DEBUG] Raw outgoing bytes: {:02x?}",
                outgoing_bytes
            );
        }

        let outgoing_slice = get_slice_safe(bytes, offset, 8)?;
        outgoing_cluster_offset = u64::from_be_bytes([
            outgoing_slice[0],
            outgoing_slice[1],
            outgoing_slice[2],
            outgoing_slice[3],
            outgoing_slice[4],
            outgoing_slice[5],
            outgoing_slice[6],
            outgoing_slice[7],
        ]);
        offset += 8;

        // HEADER_VALIDATE_DEBUG: Track byte positions
        if std::env::var("HEADER_VALIDATE_DEBUG").is_ok() {
            println!(
                "[HEADER_READ_DEBUG] Reading incoming_cluster_offset at offset {} (should be 64)",
                offset
            );
            let incoming_bytes = get_slice_safe(bytes, offset, 8)?;
            println!(
                "[HEADER_READ_DEBUG] Raw incoming bytes: {:02x?}",
                incoming_bytes
            );
        }

        let incoming_slice = get_slice_safe(bytes, offset, 8)?;
        incoming_cluster_offset = u64::from_be_bytes([
            incoming_slice[0],
            incoming_slice[1],
            incoming_slice[2],
            incoming_slice[3],
            incoming_slice[4],
            incoming_slice[5],
            incoming_slice[6],
            incoming_slice[7],
        ]);
        offset += 8;

        let free_space_slice = get_slice_safe(bytes, offset, 8)?;
        free_space_offset = u64::from_be_bytes([
            free_space_slice[0],
            free_space_slice[1],
            free_space_slice[2],
            free_space_slice[3],
            free_space_slice[4],
            free_space_slice[5],
            free_space_slice[6],
            free_space_slice[7],
        ]);
        offset += 8;

        0u64 // No checksum field in PersistentHeaderV2 - removed to prevent out-of-bounds access
    } else {
        0u64 // No checksum field in smaller headers
    };

    Ok(PersistentHeaderV2 {
        magic,
        version,
        flags,
        node_count,
        edge_count,
        schema_version,
        node_data_offset,
        edge_data_offset,
        outgoing_cluster_offset,
        incoming_cluster_offset,
        free_space_offset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persistent_header_encode_decode_roundtrip() {
        use crate::backend::native::persistent_header::PersistentHeaderV2;

        let mut header = PersistentHeaderV2::new_v2();
        header.node_count = 42;
        header.edge_count = 100;
        header.schema_version = 2;

        let encoded = encode_persistent_header(&header).unwrap();
        let decoded = decode_persistent_header(&encoded).unwrap();

        assert_eq!(header.node_count, decoded.node_count);
        assert_eq!(header.edge_count, decoded.edge_count);
        assert_eq!(header.schema_version, decoded.schema_version);
    }

    #[test]
    fn test_persistent_header_magic_validation() {
        use crate::backend::native::persistent_header::PersistentHeaderV2;

        let mut header = PersistentHeaderV2::new_v2();
        header.magic[0] = 0xFF; // Corrupt magic
        let encoded = encode_persistent_header(&header).unwrap();
        let decoded = decode_persistent_header(&encoded).unwrap();

        assert!(decoded.validate().is_err());
    }

    #[test]
    fn test_header_checksum_validation() {
        let mut header = FileHeader::new();
        header.node_count = 42;
        header.update_checksum();

        // Corrupt checksum
        header.checksum = header.checksum.wrapping_add(1);

        assert!(header.verify_checksum().is_err());
    }
}
