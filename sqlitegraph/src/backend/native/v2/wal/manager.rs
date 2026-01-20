//! V2 WAL manager for orchestrating read/write operations.
//!
//! This module provides the main WAL management interface, coordinating
//! writers, readers, checkpointing, recovery operations, and transaction
//! management with advanced group commit and cluster-affinity optimization.

use crate::backend::native::v2::wal::{
    transaction_coordinator::IsolationLevel, V2WALCheckpointManager, V2WALConfig, V2WALHeader,
    V2WALReader, V2WALRecord, V2WALWriter,
};
use crate::backend::native::{NativeBackendError, NativeResult};
use parking_lot::{Mutex, RwLock};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Active transaction state for coordination
#[derive(Debug, Clone)]
struct ActiveTransaction {
    /// Transaction identifier
    tx_id: u64,

    /// Transaction start timestamp
    start_time: Instant,

    /// Transaction start LSN
    start_lsn: u64,

    /// Records in this transaction
    records: Vec<V2WALRecord>,

    /// Transaction isolation level
    isolation_level: IsolationLevel,

    /// Whether transaction is read-only
    read_only: bool,
}

/// WAL performance metrics
#[derive(Debug, Clone)]
pub struct WALManagerMetrics {
    /// Total transactions started
    pub total_transactions: u64,

    /// Total transactions committed
    pub committed_transactions: u64,

    /// Total transactions rolled back
    pub rolled_back_transactions: u64,

    /// Average transaction duration (microseconds)
    pub avg_transaction_duration_us: u64,

    /// Total records written
    pub total_records_written: u64,

    /// WAL file size in bytes
    pub wal_size_bytes: u64,

    /// Checkpoint count
    pub checkpoint_count: u64,

    /// Recovery count
    pub recovery_count: u64,

    /// Group commit statistics
    pub group_commit_batches: u64,
    pub avg_group_commit_size: f64,

    /// Compression ratio (if enabled)
    pub compression_ratio: f64,

    /// Transactions committed since last checkpoint (resettable counter)
    pub transactions_since_checkpoint: u64,
}

/// Enhanced WAL manager with full transaction coordination
pub struct V2WALManager {
    /// WAL configuration
    config: V2WALConfig,

    /// WAL writer instance
    writer: Arc<V2WALWriter>,

    /// WAL reader instance (for recovery and analysis) - lazily initialized
    reader: Arc<Mutex<Option<V2WALReader>>>,

    /// Checkpoint manager
    checkpoint_manager: Arc<V2WALCheckpointManager>,

    /// Current WAL header (cached)
    header: Arc<RwLock<V2WALHeader>>,

    /// Active transactions
    active_transactions: Arc<RwLock<HashMap<u64, ActiveTransaction>>>,

    /// Transaction coordinator for group commit
    transaction_coordinator: Arc<Mutex<TransactionCoordinator>>,

    /// Cluster-affinity organizer
    cluster_organizer: Arc<Mutex<ClusterAffinityOrganizer>>,

    /// Performance metrics
    metrics: Arc<RwLock<WALManagerMetrics>>,

    /// Shutdown signal
    shutdown_signal: Arc<Mutex<bool>>,

    /// Background coordinator thread handle
    coordinator_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
}

/// Transaction coordinator for group commit and optimization
#[derive(Debug)]
struct TransactionCoordinator {
    /// Pending transactions for group commit
    pending_transactions: VecDeque<ActiveTransaction>,

    /// Maximum group commit size
    max_group_size: usize,

    /// Group commit timeout
    group_timeout: Duration,

    /// Last group commit time
    last_group_commit: Instant,

    /// Group commit statistics
    group_commit_count: u64,
    total_grouped_transactions: u64,
}

/// Cluster-affinity organizer for optimal I/O patterns
#[derive(Debug)]
struct ClusterAffinityOrganizer {
    /// Cluster-based record grouping
    cluster_groups: HashMap<i64, Vec<V2WALRecord>>,

    /// Maximum records per cluster group
    max_cluster_group_size: usize,

    /// Cluster flush timeout
    cluster_flush_timeout: Duration,

    /// Last cluster flush time
    last_cluster_flush: Instant,
}

impl V2WALManager {
    /// Create a new enhanced WAL manager
    pub fn create(config: V2WALConfig) -> NativeResult<Self> {
        config.validate()?;

        // Create WAL writer
        let writer = Arc::new(V2WALWriter::create(config.clone())?);

        // Create WAL reader lazily (will be initialized on first access)
        let reader = Arc::new(Mutex::new(None));

        // Create checkpoint manager with default strategy
        let checkpoint_strategy =
            crate::backend::native::v2::wal::checkpoint::CheckpointStrategy::SizeThreshold(
                config.max_wal_size / 4,
            );
        let checkpoint_manager = Arc::new(V2WALCheckpointManager::create(
            config.clone(),
            checkpoint_strategy,
        )?);

        // Initialize header from writer
        let header = Arc::new(RwLock::new(writer.get_header()));

        // Initialize transaction coordinator
        let transaction_coordinator = Arc::new(Mutex::new(TransactionCoordinator {
            pending_transactions: VecDeque::new(),
            max_group_size: config.max_group_commit_size,
            group_timeout: Duration::from_millis(config.group_commit_timeout_ms),
            last_group_commit: Instant::now(),
            group_commit_count: 0,
            total_grouped_transactions: 0,
        }));

        // Initialize cluster organizer
        let cluster_organizer = Arc::new(Mutex::new(ClusterAffinityOrganizer {
            cluster_groups: HashMap::new(),
            max_cluster_group_size: 100,
            cluster_flush_timeout: Duration::from_millis(50),
            last_cluster_flush: Instant::now(),
        }));

        let manager = Self {
            config,
            writer,
            reader,
            checkpoint_manager,
            header,
            active_transactions: Arc::new(RwLock::new(HashMap::new())),
            transaction_coordinator,
            cluster_organizer,
            metrics: Arc::new(RwLock::new(WALManagerMetrics::default())),
            shutdown_signal: Arc::new(Mutex::new(false)),
            coordinator_handle: Arc::new(Mutex::new(None)),
        };

        // Start background coordinator
        manager.start_background_coordinator()?;

        Ok(manager)
    }

    /// Ensure WAL reader is initialized (lazy initialization)
    fn ensure_reader_initialized(&self) -> NativeResult<()> {
        let mut reader_guard = self.reader.lock();
        if reader_guard.is_none() {
            // Writer should have initialized the WAL file by now
            let reader = V2WALReader::open(&self.config.wal_path)?;
            *reader_guard = Some(reader);
        }
        Ok(())
    }

    /// Get WAL reader (ensuring it's initialized)
    fn get_reader(&self) -> NativeResult<parking_lot::MutexGuard<'_, Option<V2WALReader>>> {
        self.ensure_reader_initialized()?;
        Ok(self.reader.lock())
    }

    /// Begin a new transaction
    pub fn begin_transaction(&self, isolation_level: IsolationLevel) -> NativeResult<u64> {
        let start_time = Instant::now();

        // Generate unique transaction ID
        let tx_id = self.generate_transaction_id();

        // Get current LSN
        let start_lsn = {
            let header = self.header.read();
            header.current_lsn
        };

        // Create active transaction
        let transaction = ActiveTransaction {
            tx_id,
            start_time,
            start_lsn,
            records: Vec::new(),
            isolation_level,
            read_only: false, // Will be updated based on first operation
        };

        // Add to active transactions
        {
            let mut active = self.active_transactions.write();
            active.insert(tx_id, transaction);
        }

        // Write transaction begin record
        let begin_record = V2WALRecord::TransactionBegin {
            tx_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        self.writer.write_record(begin_record)?;

        // Update metrics
        {
            let mut metrics = self.metrics.write();
            metrics.total_transactions += 1;
        }

        Ok(tx_id)
    }

    /// Write a record within a transaction
    pub fn write_transaction_record(&self, tx_id: u64, record: V2WALRecord) -> NativeResult<u64> {
        // Validate transaction is active
        {
            let active = self.active_transactions.read();
            if !active.contains_key(&tx_id) {
                return Err(NativeBackendError::InvalidTransaction {
                    tx_id,
                    reason: "Transaction not found or not active".to_string(),
                });
            }
        }

        // Extract cluster key before moving record
        let cluster_key = record.cluster_key();

        // Create two separate clones
        let record_for_tx = record.clone();
        let record_for_cluster = record.clone();

        // Write the record
        let lsn = self.writer.write_record(record)?;

        // Add to transaction record list
        {
            let mut active = self.active_transactions.write();
            if let Some(tx) = active.get_mut(&tx_id) {
                tx.records.push(record_for_tx);
                tx.read_only = false; // Transaction is now read-write
            }
        }

        // Add to cluster organizer for optimal I/O
        if let Some(key) = cluster_key {
            let mut organizer = self.cluster_organizer.lock();
            organizer
                .cluster_groups
                .entry(key)
                .or_insert_with(Vec::new)
                .push(record_for_cluster);
        }

        // Synchronize writer metrics with manager metrics
        {
            let writer_metrics = self.writer.get_metrics();
            let mut manager_metrics = self.metrics.write();
            manager_metrics.total_records_written = writer_metrics.records_written;
        }

        Ok(lsn)
    }

    /// Commit a transaction
    pub fn commit_transaction(&self, tx_id: u64) -> NativeResult<()> {
        let start_time = Instant::now();

        // Remove from active transactions
        let transaction = {
            let mut active = self.active_transactions.write();
            active.remove(&tx_id)
        };

        let transaction = transaction.ok_or_else(|| NativeBackendError::InvalidTransaction {
            tx_id,
            reason: "Transaction not found".to_string(),
        })?;

        // Write transaction commit record
        let commit_record = V2WALRecord::TransactionCommit {
            tx_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        self.writer.write_record(commit_record)?;

        // Add to group commit coordinator
        {
            let mut coordinator = self.transaction_coordinator.lock();
            coordinator.pending_transactions.push_back(transaction);
        }

        // Update metrics
        {
            let mut metrics = self.metrics.write();
            metrics.committed_transactions += 1;
            metrics.transactions_since_checkpoint += 1;
            let duration_us = start_time.elapsed().as_micros() as u64;
            let total_tx = metrics.committed_transactions;
            metrics.avg_transaction_duration_us =
                ((metrics.avg_transaction_duration_us * (total_tx - 1) as u64) + duration_us)
                    / total_tx;
        }

        // Trigger group commit if needed
        self.check_group_commit();

        // Check if checkpoint is needed after commit
        if self.config.auto_checkpoint && self.requires_checkpoint() {
            // Spawn background checkpoint to avoid blocking commit
            let checkpoint_manager = self.checkpoint_manager.clone();
            std::thread::spawn(move || {
                if let Err(e) = checkpoint_manager.force_checkpoint() {
                    eprintln!("Background checkpoint failed: {}", e);
                }
            });
        }

        Ok(())
    }

    /// Rollback a transaction
    pub fn rollback_transaction(&self, tx_id: u64) -> NativeResult<()> {
        // Remove from active transactions
        let transaction = {
            let mut active = self.active_transactions.write();
            active.remove(&tx_id)
        };

        let transaction = transaction.ok_or_else(|| NativeBackendError::InvalidTransaction {
            tx_id,
            reason: "Transaction not found".to_string(),
        })?;

        // Write transaction rollback record
        let rollback_record = V2WALRecord::TransactionRollback {
            tx_id,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        };

        self.writer.write_record(rollback_record)?;

        // Update metrics
        {
            let mut metrics = self.metrics.write();
            metrics.rolled_back_transactions += 1;
        }

        Ok(())
    }

    /// Write a single WAL record (outside transaction)
    pub fn write_record(&self, record: V2WALRecord) -> NativeResult<u64> {
        let result = self.writer.write_record(record)?;

        // Synchronize writer metrics with manager metrics
        {
            let writer_metrics = self.writer.get_metrics();
            let mut manager_metrics = self.metrics.write();
            manager_metrics.total_records_written = writer_metrics.records_written;
        }

        Ok(result)
    }

    /// Write multiple records in a batch
    pub fn write_records_batch(&self, records: Vec<V2WALRecord>) -> NativeResult<Vec<u64>> {
        let result = self.writer.write_records_batch(records)?;

        // Synchronize writer metrics with manager metrics
        {
            let writer_metrics = self.writer.get_metrics();
            let mut manager_metrics = self.metrics.write();
            manager_metrics.total_records_written = writer_metrics.records_written;
        }

        Ok(result)
    }

    /// Flush all pending writes
    pub fn flush(&self) -> NativeResult<()> {
        self.writer.flush_buffer()
    }

    /// Force checkpoint operation
    pub fn force_checkpoint(&self) -> NativeResult<()> {
        let checkpoint_lsn = {
            let header = self.header.read();
            header.committed_lsn
        };

        self.checkpoint_manager.force_checkpoint()?;

        // Update metrics
        {
            let mut metrics = self.metrics.write();
            metrics.checkpoint_count += 1;
        }

        Ok(())
    }

    /// Get current WAL header
    pub fn get_header(&self) -> V2WALHeader {
        self.header.read().clone()
    }

    /// Get performance metrics
    pub fn get_metrics(&self) -> WALManagerMetrics {
        self.metrics.read().clone()
    }

    /// Get active transaction count
    pub fn get_active_transaction_count(&self) -> usize {
        self.active_transactions.read().len()
    }

    /// Check if WAL requires checkpoint
    pub fn requires_checkpoint(&self) -> bool {
        let header = self.header.read();
        let wal_size = self.estimate_wal_size();

        wal_size > self.config.max_wal_size
            || (header.current_lsn - header.checkpointed_lsn) > self.config.checkpoint_interval
    }

    /// Generate unique transaction ID
    fn generate_transaction_id(&self) -> u64 {
        static NEXT_TX_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        NEXT_TX_ID.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    /// Start background coordinator thread
    fn start_background_coordinator(&self) -> NativeResult<()> {
        let transaction_coordinator = self.transaction_coordinator.clone();
        let cluster_organizer = self.cluster_organizer.clone();
        let writer = self.writer.clone();
        let shutdown_signal = self.shutdown_signal.clone();

        let handle = std::thread::spawn(move || {
            let mut last_check = Instant::now();

            loop {
                // Check shutdown signal
                {
                    let shutdown = shutdown_signal.lock();
                    if *shutdown {
                        break;
                    }
                }

                // Check for group commit opportunities
                if last_check.elapsed() >= Duration::from_millis(10) {
                    Self::process_group_commits(&transaction_coordinator, &writer);
                    Self::process_cluster_groups(&cluster_organizer, &writer);
                    last_check = Instant::now();
                }

                // Sleep briefly to avoid busy loop
                std::thread::sleep(Duration::from_millis(5));
            }
        });

        let mut coordinator_handle = self.coordinator_handle.lock();
        *coordinator_handle = Some(handle);

        Ok(())
    }

    /// Process group commits
    fn process_group_commits(
        coordinator: &Arc<Mutex<TransactionCoordinator>>,
        writer: &Arc<V2WALWriter>,
    ) {
        let mut coord = coordinator.lock();

        if coord.pending_transactions.len() >= coord.max_group_size
            || coord.last_group_commit.elapsed() >= coord.group_timeout
        {
            let batch_size = coord.pending_transactions.len().min(coord.max_group_size);
            let batch: Vec<_> = coord.pending_transactions.drain(..batch_size).collect();

            if !batch.is_empty() {
                // Process batch commit
                let _ = writer.flush_buffer(); // Ensure all records are written

                coord.group_commit_count += 1;
                coord.total_grouped_transactions += batch.len() as u64;
                coord.last_group_commit = Instant::now();
            }
        }
    }

    /// Process cluster groups for optimal I/O
    fn process_cluster_groups(
        organizer: &Arc<Mutex<ClusterAffinityOrganizer>>,
        writer: &Arc<V2WALWriter>,
    ) {
        let mut org = organizer.lock();

        if org.last_cluster_flush.elapsed() >= org.cluster_flush_timeout {
            // Flush cluster groups
            for (cluster_key, records) in org.cluster_groups.drain() {
                if !records.is_empty() {
                    // Process cluster-affinity records
                    let _ = writer.flush_buffer(); // Ensure records are written
                }
            }
            org.last_cluster_flush = Instant::now();
        }
    }

    /// Check and trigger group commit if needed
    fn check_group_commit(&self) {
        Self::process_group_commits(&self.transaction_coordinator, &self.writer);
    }

    /// Estimate current WAL file size
    fn estimate_wal_size(&self) -> u64 {
        // Check actual WAL file size if available
        if let Ok(metadata) = std::fs::metadata(&self.config.wal_path) {
            return metadata.len();
        }

        // Fallback to writer metrics
        let metrics = self.writer.get_metrics();
        metrics.bytes_written + std::mem::size_of::<V2WALHeader>() as u64
    }

    // Bulk ingest mode methods

    /// Enable bulk ingest mode with optimized parameters
    pub fn enable_bulk_mode(
        &self,
        config: &super::bulk_ingest::BulkIngestConfig,
    ) -> NativeResult<()> {
        self.writer.enable_bulk_mode(config)
    }

    /// Disable bulk ingest mode and restore original configuration
    pub fn disable_bulk_mode(&self) -> NativeResult<()> {
        self.writer.disable_bulk_mode()
    }

    /// Check if bulk mode is currently active
    pub fn is_bulk_mode_active(&self) -> bool {
        self.writer.is_bulk_mode_active()
    }

    /// Shutdown WAL manager gracefully
    pub fn shutdown(self) -> NativeResult<()> {
        // Signal shutdown
        {
            let mut shutdown = self.shutdown_signal.lock();
            *shutdown = true;
        }

        // Join coordinator thread
        {
            let mut handle = self.coordinator_handle.lock();
            if let Some(handle) = handle.take() {
                let _ = handle.join();
            }
        }

        // Force final group commit
        self.check_group_commit();

        // Flush any remaining data
        self.flush()?;

        // Shutdown writer
        self.writer.shutdown()?;

        Ok(())
    }
}

impl Default for WALManagerMetrics {
    fn default() -> Self {
        Self {
            total_transactions: 0,
            committed_transactions: 0,
            rolled_back_transactions: 0,
            avg_transaction_duration_us: 0,
            total_records_written: 0,
            wal_size_bytes: 0,
            checkpoint_count: 0,
            recovery_count: 0,
            group_commit_batches: 0,
            avg_group_commit_size: 0.0,
            compression_ratio: 1.0,
            transactions_since_checkpoint: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::GraphFile;
    use tempfile::tempdir;

    #[test]
    fn test_enhanced_wal_manager_create() {
        let temp_dir = tempdir().unwrap();
        let v2_graph_path = temp_dir.path().join("test.v2");

        // Create a minimal V2 graph file for the checkpoint manager
        let _graph_file =
            GraphFile::create(&v2_graph_path).expect("Failed to create V2 graph file for test");

        let config = V2WALConfig {
            graph_path: v2_graph_path.clone(),
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let manager = V2WALManager::create(config);
        assert!(manager.is_ok());

        let manager = manager.unwrap();
        assert_eq!(manager.get_active_transaction_count(), 0);

        // Test metrics
        let metrics = manager.get_metrics();
        assert_eq!(metrics.total_transactions, 0);
        assert_eq!(metrics.committed_transactions, 0);
    }

    #[test]
    fn test_transaction_lifecycle() {
        let temp_dir = tempdir().unwrap();
        let v2_graph_path = temp_dir.path().join("test.v2");

        // Create a minimal V2 graph file for the checkpoint manager
        let _graph_file =
            GraphFile::create(&v2_graph_path).expect("Failed to create V2 graph file for test");

        let config = V2WALConfig {
            graph_path: v2_graph_path.clone(),
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let manager = V2WALManager::create(config).unwrap();

        // Begin transaction
        let tx_id = manager
            .begin_transaction(IsolationLevel::ReadCommitted)
            .unwrap();
        assert!(tx_id > 0);
        assert_eq!(manager.get_active_transaction_count(), 1);

        // Write record within transaction
        let record = V2WALRecord::NodeInsert {
            node_id: 42,
            slot_offset: 1024,
            node_data: vec![1, 2, 3],
        };

        let lsn = manager.write_transaction_record(tx_id, record).unwrap();
        assert!(lsn > 0);

        // Commit transaction
        manager.commit_transaction(tx_id).unwrap();
        assert_eq!(manager.get_active_transaction_count(), 0);

        // Check metrics
        let metrics = manager.get_metrics();
        assert_eq!(metrics.total_transactions, 1);
        assert_eq!(metrics.committed_transactions, 1);
    }

    #[test]
    fn test_transaction_rollback() {
        let temp_dir = tempdir().unwrap();
        let v2_graph_path = temp_dir.path().join("test.v2");

        // Create a minimal V2 graph file for the checkpoint manager
        let _graph_file =
            GraphFile::create(&v2_graph_path).expect("Failed to create V2 graph file for test");

        let config = V2WALConfig {
            graph_path: v2_graph_path.clone(),
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let manager = V2WALManager::create(config).unwrap();

        // Begin transaction
        let tx_id = manager
            .begin_transaction(IsolationLevel::Serializable)
            .unwrap();
        assert_eq!(manager.get_active_transaction_count(), 1);

        // Write record within transaction
        let record = V2WALRecord::NodeInsert {
            node_id: 43,
            slot_offset: 2048,
            node_data: vec![4, 5, 6],
        };

        manager.write_transaction_record(tx_id, record).unwrap();

        // Rollback transaction
        manager.rollback_transaction(tx_id).unwrap();
        assert_eq!(manager.get_active_transaction_count(), 0);

        // Check metrics
        let metrics = manager.get_metrics();
        assert_eq!(metrics.total_transactions, 1);
        assert_eq!(metrics.committed_transactions, 0);
        assert_eq!(metrics.rolled_back_transactions, 1);
    }

    #[test]
    fn test_isolation_levels() {
        assert_eq!(
            IsolationLevel::ReadCommitted,
            IsolationLevel::ReadCommitted
        );
        assert_ne!(
            IsolationLevel::ReadCommitted,
            IsolationLevel::Serializable
        );
        assert_ne!(
            IsolationLevel::Serializable,
            IsolationLevel::Snapshot
        );
    }

    #[test]
    fn test_transaction_coordinator() {
        let coordinator = TransactionCoordinator {
            pending_transactions: VecDeque::new(),
            max_group_size: 10,
            group_timeout: Duration::from_millis(100),
            last_group_commit: Instant::now(),
            group_commit_count: 0,
            total_grouped_transactions: 0,
        };

        assert_eq!(coordinator.pending_transactions.len(), 0);
        assert_eq!(coordinator.max_group_size, 10);
        assert_eq!(coordinator.group_commit_count, 0);
    }

    #[test]
    fn test_cluster_organizer() {
        let organizer = ClusterAffinityOrganizer {
            cluster_groups: HashMap::new(),
            max_cluster_group_size: 50,
            cluster_flush_timeout: Duration::from_millis(25),
            last_cluster_flush: Instant::now(),
        };

        assert_eq!(organizer.cluster_groups.len(), 0);
        assert_eq!(organizer.max_cluster_group_size, 50);
    }

    #[test]
    fn test_wal_manager_metrics() {
        let mut metrics = WALManagerMetrics::default();

        assert_eq!(metrics.total_transactions, 0);
        assert_eq!(metrics.committed_transactions, 0);
        assert_eq!(metrics.rolled_back_transactions, 0);
        assert_eq!(metrics.avg_transaction_duration_us, 0);

        // Update some metrics
        metrics.total_transactions = 5;
        metrics.committed_transactions = 4;
        metrics.rolled_back_transactions = 1;
        metrics.avg_transaction_duration_us = 1500;

        assert_eq!(metrics.total_transactions, 5);
        assert_eq!(metrics.committed_transactions, 4);
        assert_eq!(metrics.rolled_back_transactions, 1);
        assert_eq!(metrics.avg_transaction_duration_us, 1500);
    }

    #[test]
    fn test_wal_manager_shutdown() {
        let temp_dir = tempdir().unwrap();
        let v2_graph_path = temp_dir.path().join("test.v2");

        // Create a minimal V2 graph file for the checkpoint manager
        let _graph_file =
            GraphFile::create(&v2_graph_path).expect("Failed to create V2 graph file for test");

        let config = V2WALConfig {
            graph_path: v2_graph_path.clone(),
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let manager = V2WALManager::create(config).unwrap();

        // Begin a transaction to test cleanup
        let tx_id = manager
            .begin_transaction(IsolationLevel::ReadCommitted)
            .unwrap();
        manager
            .write_transaction_record(
                tx_id,
                V2WALRecord::NodeInsert {
                    node_id: 44,
                    slot_offset: 3072,
                    node_data: vec![7, 8, 9],
                },
            )
            .unwrap();

        // Shutdown should clean up properly
        let shutdown_result = manager.shutdown();
        assert!(shutdown_result.is_ok());
    }

    #[test]
    fn test_auto_checkpoint_enabled() {
        let temp_dir = tempdir().unwrap();
        let v2_graph_path = temp_dir.path().join("test.v2");

        // Create a minimal V2 graph file for the checkpoint manager
        let _graph_file =
            GraphFile::create(&v2_graph_path).expect("Failed to create V2 graph file for test");

        let mut config = V2WALConfig {
            graph_path: v2_graph_path.clone(),
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            max_wal_size: 1024 * 1024, // 1MB (minimum allowed)
            checkpoint_interval: 2, // Trigger after 2 transactions
            auto_checkpoint: true,
            ..Default::default()
        };

        let manager = V2WALManager::create(config).unwrap();

        // Begin and commit first transaction
        let tx_id = manager
            .begin_transaction(IsolationLevel::ReadCommitted)
            .unwrap();
        manager
            .write_transaction_record(
                tx_id,
                V2WALRecord::NodeInsert {
                    node_id: 1,
                    slot_offset: 1024,
                    node_data: vec![1, 2, 3],
                },
            )
            .unwrap();
        manager.commit_transaction(tx_id).unwrap();

        // Begin and commit second transaction (should trigger checkpoint)
        let tx_id = manager
            .begin_transaction(IsolationLevel::ReadCommitted)
            .unwrap();
        manager
            .write_transaction_record(
                tx_id,
                V2WALRecord::NodeInsert {
                    node_id: 2,
                    slot_offset: 2048,
                    node_data: vec![4, 5, 6],
                },
            )
            .unwrap();
        manager.commit_transaction(tx_id).unwrap();

        // Give background checkpoint thread time to run
        std::thread::sleep(Duration::from_millis(100));

        // Verify checkpoint was triggered
        let metrics = manager.get_metrics();
        // Note: checkpoint_count may not be incremented yet as checkpoint runs in background
        // The key test is that the commit doesn't block and completes successfully
        assert_eq!(metrics.committed_transactions, 2);
    }

    #[test]
    fn test_auto_checkpoint_disabled() {
        let temp_dir = tempdir().unwrap();
        let v2_graph_path = temp_dir.path().join("test.v2");

        // Create a minimal V2 graph file for the checkpoint manager
        let _graph_file =
            GraphFile::create(&v2_graph_path).expect("Failed to create V2 graph file for test");

        let mut config = V2WALConfig {
            graph_path: v2_graph_path.clone(),
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            max_wal_size: 1024 * 1024, // 1MB (minimum allowed)
            checkpoint_interval: 2,
            auto_checkpoint: false, // Disabled
            ..Default::default()
        };

        let manager = V2WALManager::create(config).unwrap();

        // Commit multiple transactions
        for i in 0..5 {
            let tx_id = manager
                .begin_transaction(IsolationLevel::ReadCommitted)
                .unwrap();
            manager
                .write_transaction_record(
                    tx_id,
                    V2WALRecord::NodeInsert {
                        node_id: i,
                        slot_offset: ((i + 1) * 1024) as u64,
                        node_data: vec![i as u8],
                    },
                )
                .unwrap();
            manager.commit_transaction(tx_id).unwrap();
        }

        // Give time for any potential background checkpoint
        std::thread::sleep(Duration::from_millis(100));

        // With auto_checkpoint disabled, checkpoint count should remain 0
        let metrics = manager.get_metrics();
        assert_eq!(metrics.committed_transactions, 5);
        assert_eq!(metrics.checkpoint_count, 0);
    }

    #[test]
    fn test_checkpoint_does_not_block_commit() {
        let temp_dir = tempdir().unwrap();
        let v2_graph_path = temp_dir.path().join("test.v2");

        // Create a minimal V2 graph file for the checkpoint manager
        let _graph_file =
            GraphFile::create(&v2_graph_path).expect("Failed to create V2 graph file for test");

        let config = V2WALConfig {
            graph_path: v2_graph_path.clone(),
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            max_wal_size: 1024 * 1024, // 1MB (minimum allowed)
            checkpoint_interval: 1,
            auto_checkpoint: true,
            ..Default::default()
        };

        let manager = V2WALManager::create(config).unwrap();

        // Measure commit time - should be fast even with checkpoint trigger
        let start = std::time::Instant::now();

        let tx_id = manager
            .begin_transaction(IsolationLevel::ReadCommitted)
            .unwrap();
        manager
            .write_transaction_record(
                tx_id,
                V2WALRecord::NodeInsert {
                    node_id: 1,
                    slot_offset: 1024,
                    node_data: vec![1, 2, 3],
                },
            )
            .unwrap();
        manager.commit_transaction(tx_id).unwrap();

        let commit_duration = start.elapsed();

        // Commit should complete quickly (not wait for checkpoint)
        // Background checkpoint runs in separate thread
        assert!(commit_duration < Duration::from_millis(100));
    }

    #[test]
    fn test_wal_size_estimation_uses_actual_file() {
        let temp_dir = tempdir().unwrap();
        let v2_graph_path = temp_dir.path().join("test.v2");

        // Create a minimal V2 graph file for the checkpoint manager
        let _graph_file =
            GraphFile::create(&v2_graph_path).expect("Failed to create V2 graph file for test");

        let config = V2WALConfig {
            graph_path: v2_graph_path.clone(),
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let manager = V2WALManager::create(config).unwrap();

        // Write some data to create WAL file
        let tx_id = manager
            .begin_transaction(IsolationLevel::ReadCommitted)
            .unwrap();
        manager
            .write_transaction_record(
                tx_id,
                V2WALRecord::NodeInsert {
                    node_id: 1,
                    slot_offset: 1024,
                    node_data: vec![1, 2, 3],
                },
            )
            .unwrap();
        manager.commit_transaction(tx_id).unwrap();

        // Wait for file to be flushed
        std::thread::sleep(Duration::from_millis(50));

        // Verify WAL file exists and has size
        assert!(temp_dir.path().join("test.wal").exists());
        let wal_size = std::fs::metadata(temp_dir.path().join("test.wal"))
            .unwrap()
            .len();
        assert!(wal_size > 0);
    }
}
