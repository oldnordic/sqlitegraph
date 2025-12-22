# WAL Implementation Examples for SQLiteGraph V2

## 1. Core WAL Data Structures

### 1.1 WAL File Format

```rust
use std::path::PathBuf;
use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};
use memmap2::{MmapMut, MmapOptions};
use parking_lot::{Mutex, RwLock};
use crossbeam::atomic::AtomicCell;

/// WAL file with append-only writes
pub struct WalFile {
    file: File,
    mmap: MmapMut,
    header: AtomicCell<WalHeader>,
    sync_mode: WalSyncMode,
    buffer: Vec<u8>,
    buffer_threshold: usize,
}

/// WAL file header (64 bytes, cache line aligned)
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct WalHeader {
    /// Magic number: "SQLTGWAL"
    pub magic: [u8; 8],
    /// WAL format version
    pub version: u32,
    /// WAL page size (power of 2)
    pub page_size: u32,
    /// Monotonically increasing sequence number
    pub sequence_number: u64,
    /// Position of last successful checkpoint
    pub checkpoint_position: u64,
    /// Bitmask of currently active transactions
    pub active_transactions: u64,
    /// Total number of committed transactions
    pub committed_tx_count: u64,
    /// CRC64 checksum of header
    pub checksum: u64,
}

impl WalHeader {
    const MAGIC: &'static [u8; 8] = b"SQLTGWAL";
    const CURRENT_VERSION: u32 = 1;
    const DEFAULT_PAGE_SIZE: u32 = 4096;

    pub fn new() -> Self {
        Self {
            magic: *Self::MAGIC,
            version: Self::CURRENT_VERSION,
            page_size: Self::DEFAULT_PAGE_SIZE,
            sequence_number: 0,
            checkpoint_position: 64, // Start after header
            active_transactions: 0,
            committed_tx_count: 0,
            checksum: 0,
        }
    }

    pub fn validate(&self) -> Result<(), WalError> {
        if self.magic != *Self::MAGIC {
            return Err(WalError::InvalidMagic);
        }
        if self.version != Self::CURRENT_VERSION {
            return Err(WalError::UnsupportedVersion(self.version));
        }
        if !self.page_size.is_power_of_two() {
            return Err(WalError::InvalidPageSize);
        }

        // Verify checksum
        let expected_checksum = self.calculate_checksum();
        if self.checksum != expected_checksum {
            return Err(WalError::ChecksumMismatch);
        }

        Ok(())
    }

    fn calculate_checksum(&self) -> u64 {
        // Use CRC64 for header integrity
        let mut hasher = crc64fast::Digest::new();
        unsafe {
            let slice = std::slice::from_raw_parts(
                self as *const _ as *const u8,
                std::mem::size_of::<Self>() - 8, // Exclude checksum field
            );
            hasher.update(slice);
        }
        hasher.finalize()
    }
}

/// Synchronization modes for WAL
#[derive(Debug, Clone, Copy)]
pub enum WalSyncMode {
    /// No fsync (fastest, least safe)
    Off,
    /// Fsync on transaction commit
    Normal,
    /// Fsync after every write (slowest, safest)
    Full,
}
```

### 1.2 WAL Record Types

```rust
/// Types of operations that can be logged
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GraphOperation {
    /// Node insertion with complete record
    InsertNode {
        id: u64,
        offset: u64,
        record: NodeRecordV2,
    },
    /// Node deletion (for rollback)
    DeleteNode {
        id: u64,
        offset: u64,
        record: NodeRecordV2, // Previous state for recovery
    },
    /// Edge insertion
    InsertEdge {
        edge_id: u64,
        from_id: u64,
        to_id: u64,
        record: CompactEdgeRecord,
    },
    /// Edge deletion
    DeleteEdge {
        edge_id: u64,
        from_id: u64,
        to_id: u64,
        record: CompactEdgeRecord, // Previous state
    },
    /// Cluster update (atomic)
    UpdateCluster {
        node_id: u64,
        direction: Direction,
        old_offset: u64,
        new_offset: u64,
        cluster: EdgeCluster,
    },
    /// String table mutation
    UpdateStringTable {
        additions: Vec<(String, u32)>,
    },
    /// Free space management
    FreeSpaceOp {
        offset: u64,
        size: u64,
        operation: FreeSpaceOperation,
    },
    /// Header update
    UpdateHeader {
        field: HeaderField,
        value: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FreeSpaceOperation {
    Allocate,
    Deallocate,
    Merge,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HeaderField {
    NodeCount,
    EdgeCount,
    NodeDataOffset,
    EdgeDataOffset,
    OutgoingClusterOffset,
    IncomingClusterOffset,
    FreeSpaceOffset,
}

/// Individual WAL record with metadata
#[derive(Debug, Clone)]
pub struct WalRecord {
    /// Unique ID for this record
    pub record_id: u64,
    /// Transaction ID this belongs to
    pub tx_id: u64,
    /// Sequence number within transaction
    pub sequence: u32,
    /// Timestamp (nanoseconds since epoch)
    pub timestamp: u64,
    /// Actual operation
    pub operation: GraphOperation,
    /// CRC32 checksum of operation data
    pub checksum: u32,
}

impl WalRecord {
    pub const MAX_SIZE: usize = 4096; // One page

    pub fn new(tx_id: u64, sequence: u32, operation: GraphOperation) -> Self {
        let record_id = generate_record_id();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as u64;

        Self {
            record_id,
            tx_id,
            sequence,
            timestamp,
            operation,
            checksum: 0, // Will be calculated
        }
    }

    pub fn serialize(&self) -> Result<Vec<u8>, WalError> {
        // Fixed header + variable operation data
        let mut buf = Vec::with_capacity(64 + 1024);

        // Write fixed header
        buf.extend_from_slice(&self.record_id.to_le_bytes());
        buf.extend_from_slice(&self.tx_id.to_le_bytes());
        buf.extend_from_slice(&self.sequence.to_le_bytes());
        buf.extend_from_slice(&self.timestamp.to_le_bytes());

        // Serialize operation
        let op_data = bincode::serialize(&self.operation)
            .map_err(WalError::SerializationError)?;
        buf.extend_from_slice(&(op_data.len() as u32).to_le_bytes());
        buf.extend_from_slice(&op_data);

        // Calculate and write checksum
        let checksum = crc32fast::hash(&buf);
        buf.extend_from_slice(&checksum.to_le_bytes());

        Ok(buf)
    }

    pub fn deserialize(data: &[u8]) -> Result<Self, WalError> {
        if data.len() < 32 {
            return Err(WalError::RecordTooSmall);
        }

        let mut cursor = 0;

        // Read fixed header
        let record_id = u64::from_le_bytes(data[cursor..cursor+8].try_into().unwrap());
        cursor += 8;
        let tx_id = u64::from_le_bytes(data[cursor..cursor+8].try_into().unwrap());
        cursor += 8;
        let sequence = u32::from_le_bytes(data[cursor..cursor+4].try_into().unwrap());
        cursor += 4;
        let timestamp = u64::from_le_bytes(data[cursor..cursor+8].try_into().unwrap());
        cursor += 8;

        // Read operation data
        let op_len = u32::from_le_bytes(data[cursor..cursor+4].try_into().unwrap()) as usize;
        cursor += 4;

        if cursor + op_len + 4 > data.len() {
            return Err(WalError::RecordTooSmall);
        }

        let operation: GraphOperation = bincode::deserialize(&data[cursor..cursor+op_len])
            .map_err(WalError::SerializationError)?;
        cursor += op_len;

        // Verify checksum
        let stored_checksum = u32::from_le_bytes(data[cursor..cursor+4].try_into().unwrap());
        let calculated_checksum = crc32fast::hash(&data[..cursor]);

        if stored_checksum != calculated_checksum {
            return Err(WalError::ChecksumMismatch);
        }

        Ok(Self {
            record_id,
            tx_id,
            sequence,
            timestamp,
            operation,
            checksum: stored_checksum,
        })
    }
}
```

## 2. WAL Implementation

### 2.1 Core WAL Operations

```rust
impl WalFile {
    /// Create or open a WAL file
    pub fn create_or_open(path: PathBuf, sync_mode: WalSyncMode) -> Result<Self, WalError> {
        let file = if path.exists() {
            File::options()
                .read(true)
                .write(true)
                .open(&path)?
        } else {
            Self::initialize_new_file(&path)?
        };

        // Create memory mapping
        let mmap = unsafe {
            MmapOptions::new()
                .map_mut(&file)?
        };

        // Read and validate header
        let header_data = &mmap[..std::mem::size_of::<WalHeader>()];
        let header = WalHeader::deserialize(header_data)?;
        header.validate()?;

        Ok(Self {
            file,
            mmap,
            header: AtomicCell::new(header),
            sync_mode,
            buffer: Vec::with_capacity(64 * 1024), // 64KB buffer
            buffer_threshold: 32 * 1024, // Flush at 32KB
        })
    }

    fn initialize_new_file(path: &PathBuf) -> Result<File, WalError> {
        let mut file = File::create(path)?;
        file.set_len(64 * 1024 * 1024)?; // Pre-allocate 64MB

        // Write initial header
        let header = WalHeader::new();
        let header_bytes = header.serialize();
        file.write_all(&header_bytes)?;
        file.sync_all()?;

        Ok(file)
    }

    /// Begin a new transaction
    pub fn begin_transaction(&mut self) -> Result<u64, WalError> {
        let tx_id = self.generate_transaction_id()?;

        // Mark transaction as active in header
        let mut header = self.header.load();
        header.active_transactions |= 1u64 << (tx_id % 64);
        header.sequence_number += 1;
        self.write_header(&header)?;

        // Log transaction begin marker
        let record = WalRecord::new(tx_id, 0, GraphOperation::TxBegin);
        self.append_record(&record)?;

        Ok(tx_id)
    }

    /// Commit a transaction
    pub fn commit_transaction(&mut self, tx_id: u64) -> Result<(), WalError> {
        // Log transaction commit marker
        let record = WalRecord::new(tx_id, u32::MAX, GraphOperation::TxCommit);
        self.append_record(&record)?;

        // Update header
        let mut header = self.header.load();
        header.active_transactions &= !(1u64 << (tx_id % 64));
        header.committed_tx_count += 1;
        self.write_header(&header)?;

        // Sync based on mode
        match self.sync_mode {
            WalSyncMode::Off => {},
            WalSyncMode::Normal => self.file.sync_all()?,
            WalSyncMode::Full => self.full_sync()?,
        }

        Ok(())
    }

    /// Abort a transaction
    pub fn abort_transaction(&mut self, tx_id: u64) -> Result<(), WalError> {
        // Log transaction abort marker
        let record = WalRecord::new(tx_id, u32::MAX, GraphOperation::TxAbort);
        self.append_record(&record)?;

        // Update header
        let mut header = self.header.load();
        header.active_transactions &= !(1u64 << (tx_id % 64));
        self.write_header(&header)?;

        Ok(())
    }

    /// Append a record to WAL
    fn append_record(&mut self, record: &WalRecord) -> Result<u64, WalError> {
        let serialized = record.serialize()?;
        let record_size = serialized.len();

        // Check if we need to expand the file
        let header = self.header.load();
        let write_position = self.get_write_position();
        if write_position + record_size as u64 > self.mmap.len() as u64 {
            self.expand_file()?;
        }

        // Write to buffer for batching
        self.buffer.extend_from_slice(&serialized);

        // Flush if threshold reached
        if self.buffer.len() >= self.buffer_threshold {
            self.flush_buffer()?;
        }

        Ok(write_position)
    }

    /// Flush buffered writes to disk
    fn flush_buffer(&mut self) -> Result<(), WalError> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let write_position = self.get_write_position();

        // Ensure file is large enough
        if write_position + self.buffer.len() as u64 > self.mmap.len() as u64 {
            self.expand_file()?;
        }

        // Write using memory map for speed
        unsafe {
            let dst = self.mmap.as_mut_ptr().add(write_position as usize);
            std::ptr::copy_nonoverlapping(
                self.buffer.as_ptr(),
                dst,
                self.buffer.len(),
            );
        }

        // Update write position
        self.set_write_position(write_position + self.buffer.len() as u64);
        self.buffer.clear();

        Ok(())
    }
}
```

### 2.2 Graph Operations with WAL

```rust
/// Extension trait for GraphFile to add WAL support
pub trait GraphFileWalExt {
    /// Enable WAL for this graph file
    fn enable_wal(&mut self, wal_path: PathBuf, sync_mode: WalSyncMode) -> Result<(), WalError>;

    /// Insert a node with WAL logging
    fn insert_node_with_wal(&mut self, node: NodeRecordV2) -> Result<u64, GraphError>;

    /// Delete a node with WAL logging
    fn delete_node_with_wal(&mut self, node_id: u64) -> Result<NodeRecordV2, GraphError>;

    /// Insert an edge with WAL logging
    fn insert_edge_with_wal(
        &mut self,
        from_id: u64,
        to_id: u64,
        edge_type: String,
        data: serde_json::Value,
    ) -> Result<u64, GraphError>;

    /// Begin a WAL transaction
    fn begin_transaction(&mut self) -> Result<u64, GraphError>;

    /// Commit a WAL transaction
    fn commit_transaction(&mut self, tx_id: u64) -> Result<(), GraphError>;

    /// Abort a WAL transaction
    fn abort_transaction(&mut self, tx_id: u64) -> Result<(), GraphError>;
}

impl GraphFileWalExt for GraphFile {
    fn enable_wal(&mut self, wal_path: PathBuf, sync_mode: WalSyncMode) -> Result<(), WalError> {
        let wal = WalFile::create_or_open(wal_path, sync_mode)?;
        self.wal = Some(RwLock::new(wal));

        // Update feature flags
        let mut header = self.persistent_header.clone();
        header.flags |= FLAG_WAL_ENABLED;
        self.write_header(&header)?;

        Ok(())
    }

    fn insert_node_with_wal(&mut self, node: NodeRecordV2) -> Result<u64, GraphError> {
        let tx_id = self.begin_transaction()?;

        // Allocate node slot
        let node_offset = self.allocate_node_slot(node.id)?;

        // Log node insertion
        if let Some(wal) = &self.wal {
            let mut wal = wal.write();
            wal.log_operation(tx_id, GraphOperation::InsertNode {
                id: node.id,
                offset: node_offset,
                record: node.clone(),
            })?;
        }

        // Write node to disk
        self.write_node_at_offset(node_offset, &node)?;

        // Update header
        self.increment_node_count()?;

        // Commit transaction
        self.commit_transaction(tx_id)?;

        Ok(node.id)
    }

    fn insert_edge_with_wal(
        &mut self,
        from_id: u64,
        to_id: u64,
        edge_type: String,
        data: serde_json::Value,
    ) -> Result<u64, GraphError> {
        let tx_id = self.begin_transaction()?;

        // Verify nodes exist
        let from_node = self.read_node(from_id)?;
        let to_node = self.read_node(to_id)?;

        // Create edge record
        let edge_id = self.generate_edge_id()?;
        let type_offset = self.string_table.get_or_add_offset(&edge_type)?;
        let edge_data = if data == serde_json::Value::Null {
            Vec::new()
        } else {
            serde_json::to_vec(&data)?
        };

        let compact_edge = CompactEdgeRecord::new(to_id, type_offset, edge_data);

        // Update outgoing cluster
        let mut outgoing_cluster = self.read_cluster(from_id, Direction::Outgoing)?;
        outgoing_cluster.add_compact_edge(&compact_edge)?;
        let new_outgoing_offset = self.write_cluster(&outgoing_cluster)?;

        // Update incoming cluster
        let mut incoming_cluster = self.read_cluster(to_id, Direction::Incoming)?;
        incoming_cluster.add_compact_edge(&CompactEdgeRecord::new(
            from_id, type_offset, edge_data
        ))?;
        let new_incoming_offset = self.write_cluster(&incoming_cluster)?;

        // Log all operations
        if let Some(wal) = &self.wal {
            let mut wal = wal.write();

            // Log outgoing cluster update
            wal.log_operation(tx_id, GraphOperation::UpdateCluster {
                node_id: from_id,
                direction: Direction::Outgoing,
                old_offset: from_node.outgoing_cluster_offset,
                new_offset: new_outgoing_offset,
                cluster: outgoing_cluster.clone(),
            })?;

            // Log incoming cluster update
            wal.log_operation(tx_id, GraphOperation::UpdateCluster {
                node_id: to_id,
                direction: Direction::Incoming,
                old_offset: to_node.incoming_cluster_offset,
                new_offset: new_incoming_offset,
                cluster: incoming_cluster.clone(),
            })?;

            // Log edge insertion
            wal.log_operation(tx_id, GraphOperation::InsertEdge {
                edge_id,
                from_id,
                to_id,
                record: compact_edge,
            })?;
        }

        // Update node metadata
        self.update_node_cluster_offset(from_id, Direction::Outgoing, new_outgoing_offset)?;
        self.update_node_cluster_offset(to_id, Direction::Incoming, new_incoming_offset)?;

        // Update header
        self.increment_edge_count()?;

        // Commit transaction
        self.commit_transaction(tx_id)?;

        Ok(edge_id)
    }
}
```

## 3. Recovery and Checkpointing

### 3.1 Recovery Implementation

```rust
/// Recovery manager for WAL-based crash recovery
pub struct WalRecoveryManager {
    wal_file: WalFile,
    graph_file: GraphFile,
}

impl WalRecoveryManager {
    /// Recover from crash using WAL
    pub fn recover(wal_path: PathBuf, graph_path: PathBuf) -> Result<RecoveryResult, WalError> {
        let wal_file = WalFile::open(wal_path)?;
        let graph_file = GraphFile::open(graph_path)?;
        let mut recovery = Self {
            wal_file,
            graph_file,
        };

        recovery.run_recovery()
    }

    fn run_recovery(&mut self) -> Result<RecoveryResult, WalError> {
        let mut result = RecoveryResult::new();

        // Check for incomplete transactions
        let header = self.wal_file.header.load();
        if header.active_transactions != 0 {
            // Scan WAL for incomplete transactions
            let incomplete_txs = self.find_incomplete_transactions()?;

            // Rollback incomplete transactions
            for tx_id in incomplete_txs {
                self.rollback_transaction(tx_id)?;
                result.rolled_back_transactions.push(tx_id);
            }
        }

        // Replay committed transactions since last checkpoint
        let committed_txs = self.find_committed_transactions_since_checkpoint()?;
        for tx_id in committed_txs {
            let ops = self.get_transaction_operations(tx_id)?;
            self.replay_operations(&ops)?;
            result.replayed_transactions.push(tx_id);
            result.replayed_operations += ops.len();
        }

        // Validate graph integrity
        self.validate_graph_integrity(&mut result)?;

        // Clear WAL after successful recovery
        self.wal_file.clear()?;

        Ok(result)
    }

    fn rollback_transaction(&mut self, tx_id: u64) -> Result<(), WalError> {
        let ops = self.get_transaction_operations(tx_id)?;

        // Process operations in reverse order for rollback
        for op in ops.iter().rev() {
            match &op.operation {
                GraphOperation::InsertNode { id, .. } => {
                    // Delete the node that was inserted
                    self.graph_file.delete_node_direct(*id)?;
                }
                GraphOperation::DeleteNode { record, offset, .. } => {
                    // Restore the deleted node
                    self.graph_file.write_node_at_offset(*offset, record)?;
                }
                GraphOperation::InsertEdge { edge_id, .. } => {
                    // Delete the edge that was inserted
                    self.graph_file.delete_edge_direct(*edge_id)?;
                }
                GraphOperation::UpdateCluster { node_id, direction, old_offset, cluster: _, .. } => {
                    // Restore previous cluster
                    self.graph_file.restore_cluster(*node_id, *direction, *old_offset)?;
                }
                GraphOperation::UpdateStringTable { additions, .. } => {
                    // Remove string table additions
                    for (string, _) in additions {
                        self.graph_file.string_table.remove_string(string)?;
                    }
                }
                _ => {} // Other operations handled differently
            }
        }

        Ok(())
    }

    fn replay_operations(&mut self, operations: &[WalRecord]) -> Result<(), WalError> {
        for op in operations {
            match &op.operation {
                GraphOperation::InsertNode { id, offset, record } => {
                    self.graph_file.write_node_at_offset(*offset, record)?;
                }
                GraphOperation::UpdateCluster { node_id, direction, new_offset, cluster, .. } => {
                    self.graph_file.write_cluster_at_offset(*new_offset, cluster)?;
                    self.graph_file.update_node_cluster_offset(*node_id, *direction, *new_offset)?;
                }
                GraphOperation::UpdateHeader { field, value } => {
                    self.graph_file.update_header_field(*field, *value)?;
                }
                // Skip operations that are already applied
                _ => {}
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
pub struct RecoveryResult {
    pub incomplete_transactions_found: Vec<u64>,
    pub rolled_back_transactions: Vec<u64>,
    pub replayed_transactions: Vec<u64>,
    pub replayed_operations: usize,
    pub integrity_errors: Vec<String>,
    pub recovery_time_ms: u64,
}
```

### 3.2 Checkpoint Implementation

```rust
/// Checkpoint manager for WAL
pub struct WalCheckpointManager {
    wal_file: WalFile,
    graph_file: GraphFile,
    policy: CheckpointPolicy,
}

#[derive(Debug, Clone)]
pub struct CheckpointPolicy {
    /// Minimum WAL size to trigger checkpoint
    pub size_threshold: u64,
    /// Maximum time between checkpoints
    pub time_threshold: Duration,
    /// Maximum number of transactions before checkpoint
    pub transaction_threshold: u32,
    /// Whether to run checkpoints in background
    pub background_checkpoint: bool,
}

impl WalCheckpointManager {
    /// Run a checkpoint operation
    pub fn checkpoint(&mut self) -> Result<CheckpointResult, WalError> {
        let start_time = Instant::now();
        let mut result = CheckpointResult::new();

        // Get checkpoint position
        let header = self.wal_file.header.load();
        let checkpoint_start = header.checkpoint_position;
        let checkpoint_end = self.wal_file.get_write_position();

        if checkpoint_start >= checkpoint_end {
            return Ok(result); // Nothing to checkpoint
        }

        // Collect transactions to checkpoint
        let transactions = self.collect_transactions_for_checkpoint(checkpoint_start, checkpoint_end)?;
        result.transactions_processed = transactions.len();

        // Group operations by type for efficient processing
        let mut node_ops = Vec::new();
        let mut edge_ops = Vec::new();
        let mut cluster_ops = Vec::new();
        let mut header_ops = Vec::new();

        for tx in transactions {
            for op in tx.operations {
                match op.operation {
                    GraphOperation::InsertNode { .. } => node_ops.push(op),
                    GraphOperation::InsertEdge { .. } => edge_ops.push(op),
                    GraphOperation::UpdateCluster { .. } => cluster_ops.push(op),
                    GraphOperation::UpdateHeader { .. } => header_ops.push(op),
                    _ => {}
                }
            }
        }

        // Apply operations in optimal order
        result.nodes_checkpointed = self.apply_node_operations(&node_ops)?;
        result.edges_checkpointed = self.apply_edge_operations(&edge_ops)?;
        result.clusters_checkpointed = self.apply_cluster_operations(&cluster_ops)?;

        // Update header
        for op in header_ops {
            if let GraphOperation::UpdateHeader { field, value } = op.operation {
                self.graph_file.update_header_field(field, value)?;
            }
        }

        // Update checkpoint position
        self.wal_file.advance_checkpoint_position(checkpoint_end)?;

        // Optionally truncate WAL
        if self.should_truncate_wal() {
            self.wal_file.truncate_to_checkpoint()?;
            result.wal_truncated = true;
        }

        result.duration_ms = start_time.elapsed().as_millis() as u64;
        Ok(result)
    }

    /// Collect all transactions in the checkpoint range
    fn collect_transactions_for_checkpoint(
        &self,
        start: u64,
        end: u64,
    ) -> Result<Vec<Transaction>, WalError> {
        let mut transactions = Vec::new();
        let mut current_tx = None;
        let mut cursor = start;

        while cursor < end {
            let record = self.wal_file.read_record_at(cursor)?;

            match record.operation {
                GraphOperation::TxBegin => {
                    current_tx = Some(Transaction {
                        id: record.tx_id,
                        operations: Vec::new(),
                    });
                }
                GraphOperation::TxCommit => {
                    if let Some(mut tx) = current_tx.take() {
                        transactions.push(tx);
                    }
                }
                GraphOperation::TxAbort => {
                    // Discard aborted transaction
                    current_tx = None;
                }
                _ => {
                    if let Some(ref mut tx) = current_tx {
                        tx.operations.push(record);
                    }
                }
            }

            cursor += record.serialized_size() as u64;
        }

        Ok(transactions)
    }
}

#[derive(Debug)]
pub struct CheckpointResult {
    pub transactions_processed: usize,
    pub nodes_checkpointed: usize,
    pub edges_checkpointed: usize,
    pub clusters_checkpointed: usize,
    pub wal_truncated: bool,
    pub duration_ms: u64,
}
```

## 4. Performance Optimizations

### 4.1 Batched Operations

```rust
/// Batch WAL writer for high throughput
pub struct BatchedWalWriter {
    wal: WalFile,
    pending_operations: HashMap<u64, Vec<GraphOperation>>,
    batch_size: usize,
    batch_timeout: Duration,
    last_flush: Instant,
}

impl BatchedWalWriter {
    pub fn new(wal: WalFile) -> Self {
        Self {
            wal,
            pending_operations: HashMap::new(),
            batch_size: 1000,
            batch_timeout: Duration::from_millis(100),
            last_flush: Instant::now(),
        }
    }

    pub fn log_operation(&mut self, tx_id: u64, operation: GraphOperation) -> Result<(), WalError> {
        let ops = self.pending_operations.entry(tx_id).or_insert_with(Vec::new);
        ops.push(operation);

        // Check if we should flush
        if ops.len() >= self.batch_size ||
           self.last_flush.elapsed() >= self.batch_timeout {
            self.flush_batch()?;
        }

        Ok(())
    }

    pub fn flush_batch(&mut self) -> Result<(), WalError> {
        if self.pending_operations.is_empty() {
            return Ok(());
        }

        // Group operations by type for batch processing
        let mut node_inserts = Vec::new();
        let mut edge_inserts = Vec::new();
        let mut cluster_updates = HashMap::new();

        for (tx_id, ops) in std::mem::take(&mut self.pending_operations) {
            for op in ops {
                match op {
                    GraphOperation::InsertNode { id, offset, record } => {
                        node_inserts.push((tx_id, id, offset, record));
                    }
                    GraphOperation::InsertEdge { edge_id, from_id, to_id, record } => {
                        edge_inserts.push((tx_id, edge_id, from_id, to_id, record));
                    }
                    GraphOperation::UpdateCluster { node_id, direction, .. } => {
                        cluster_updates.insert((tx_id, node_id, direction), op);
                    }
                    _ => {
                        // Log individual operation
                        self.wal.append_record(&WalRecord::new(tx_id, 0, op))?;
                    }
                }
            }
        }

        // Batch log node inserts
        if !node_inserts.is_empty() {
            let batch_op = GraphOperation::BatchInsertNodes {
                nodes: node_inserts.into_iter()
                    .map(|(_, id, offset, record)| (id, offset, record))
                    .collect(),
            };
            // Use first tx_id for batch
            let tx_id = node_inserts[0].0;
            self.wal.append_record(&WalRecord::new(tx_id, 0, batch_op))?;
        }

        // Batch log edge inserts
        if !edge_inserts.is_empty() {
            let batch_op = GraphOperation::BatchInsertEdges {
                edges: edge_inserts.into_iter()
                    .map(|(_, edge_id, from_id, to_id, record)| (edge_id, from_id, to_id, record))
                    .collect(),
            };
            let tx_id = edge_inserts[0].0;
            self.wal.append_record(&WalRecord::new(tx_id, 0, batch_op))?;
        }

        // Log cluster updates
        for ((tx_id, _, _), op) in cluster_updates {
            self.wal.append_record(&WalRecord::new(tx_id, 0, op))?;
        }

        self.last_flush = Instant::now();
        Ok(())
    }
}
```

### 4.2 Concurrent WAL Access

```rust
/// Concurrent WAL manager using lock-free structures
pub struct ConcurrentWalManager {
    writer: Arc<Mutex<WalFile>>,
    pending_writes: Arc<SegQueue<WalRecord>>,
    write_buffer: Arc<AtomicCell<Vec<WalRecord>>>,
    worker_handle: JoinHandle<()>,
}

impl ConcurrentWalManager {
    pub fn new(wal_path: PathBuf) -> Result<Self, WalError> {
        let wal = WalFile::create_or_open(wal_path, WalSyncMode::Normal)?;
        let writer = Arc::new(Mutex::new(wal));
        let pending_writes = Arc::new(SegQueue::new());
        let write_buffer = Arc::new(AtomicCell::new(Vec::new()));

        // Start background writer thread
        let pending_clone = pending_writes.clone();
        let buffer_clone = write_buffer.clone();
        let writer_clone = writer.clone();

        let worker_handle = tokio::spawn(async move {
            let mut batch = Vec::with_capacity(1000);

            loop {
                // Collect pending records
                while let Some(record) = pending_clone.pop() {
                    batch.push(record);

                    // Flush batch when full
                    if batch.len() >= 1000 {
                        Self::flush_batch(&writer_clone, &mut batch).await;
                    }
                }

                // Flush any remaining records
                if !batch.is_empty() {
                    Self::flush_batch(&writer_clone, &mut batch).await;
                }

                // Brief pause to prevent busy-waiting
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        });

        Ok(Self {
            writer,
            pending_writes,
            write_buffer,
            worker_handle,
        })
    }

    pub fn log_operation_async(&self, record: WalRecord) -> Result<(), WalError> {
        self.pending_writes.push(record);
        Ok(())
    }

    async fn flush_batch(
        writer: &Arc<Mutex<WalFile>>,
        batch: &mut Vec<WalRecord>,
    ) {
        if batch.is_empty() {
            return;
        }

        // Sort by transaction ID and sequence number
        batch.sort_by(|a, b| {
            a.tx_id.cmp(&b.tx_id)
                .then_with(|| a.sequence.cmp(&b.sequence))
        });

        // Write batch
        let mut wal = writer.lock();
        for record in batch.drain(..) {
            let _ = wal.append_record(&record);
        }

        // Sync periodically
        let _ = wal.file.sync_all();
    }
}
```

This implementation provides a comprehensive WAL system specifically designed for SQLiteGraph's V2 clustered edge format, addressing graph-specific challenges while maintaining high performance and data integrity.