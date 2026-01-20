//! V2 WAL Checkpoint Core Module
//!
//! This module provides the core checkpoint management functionality for the V2 clustered edge format,
//! including the main checkpoint manager, state management, and dirty block tracking. The core module
//! focuses on checkpoint lifecycle management, coordination, and basic operations while delegating
//! strategy decisions to the strategies module and execution details to the operations module.

use crate::backend::native::v2::wal::checkpoint::{
    constants::*, errors::*, operations::CheckpointExecutor, strategies::CheckpointStrategy,
};
use crate::backend::native::v2::wal::V2WALConfig;
use parking_lot::{Condvar, Mutex};
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Current checkpoint state machine
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointState {
    /// Idle - no checkpoint in progress
    Idle,
    /// Initializing - setting up checkpoint resources
    Initializing,
    /// Collecting - gathering dirty blocks and WAL records
    Collecting,
    /// Processing - applying records to main database
    Processing,
    /// Flushing - writing dirty blocks to disk
    Flushing,
    /// Validating - verifying checkpoint integrity
    Validating,
    /// Complete - checkpoint finished successfully
    Complete,
    /// Failed - checkpoint encountered unrecoverable error
    Failed,
}

impl Default for CheckpointState {
    fn default() -> Self {
        CheckpointState::Idle
    }
}

/// V2 WAL checkpoint manager for incremental checkpointing
///
/// This is the main orchestrator for checkpoint operations, managing the checkpoint
/// lifecycle and coordinating with other checkpoint modules. The manager maintains
/// state, coordinates with strategy evaluation, and delegates execution details.
pub struct V2WALCheckpointManager {
    /// WAL configuration
    config: V2WALConfig,

    /// Checkpoint file handle for writing checkpoint data
    checkpoint_file: Arc<Mutex<BufWriter<File>>>,

    /// Current checkpoint state machine
    state: Arc<Mutex<CheckpointManagerState>>,

    /// Checkpoint strategy for determining when to checkpoint
    strategy: Arc<Mutex<CheckpointStrategy>>,

    /// Dirty block tracking for V2 clustered edge format
    dirty_blocks: Arc<Mutex<DirtyBlockTracker>>,

    /// Checkpoint execution engine for performing actual work
    executor: Arc<Mutex<CheckpointExecutor>>,

    /// Condition variable for coordinating concurrent checkpoint requests
    checkpoint_cv: Arc<Condvar>,

    /// Shutdown flag for graceful termination
    shutdown_flag: Arc<Mutex<bool>>,
}

/// Internal state management for checkpoint manager
///
/// This struct contains the checkpoint manager's state including the current
/// state machine position, LSN tracking, and checkpoint statistics. It is made
/// public to allow validation modules to verify checkpoint state invariants.
#[derive(Debug)]
pub struct CheckpointManagerState {
    /// Current state in the checkpoint state machine
    pub current_state: CheckpointState,

    /// Last checkpointed LSN (Log Sequence Number)
    pub checkpointed_lsn: u64,

    /// Checkpoint currently in progress flag
    pub in_progress: bool,

    /// Last successful checkpoint timestamp
    pub last_checkpoint: Option<Instant>,

    /// Current checkpoint operation ID
    pub current_operation_id: u64,

    /// Total checkpoints completed since creation
    pub completed_checkpoints: u64,

    /// Failed checkpoint attempts
    pub failed_attempts: u64,

    /// Checkpoint start time for current operation
    pub checkpoint_start_time: Option<Instant>,

    /// Transactions committed since last checkpoint (resettable counter)
    pub transactions_since_checkpoint: u64,

    /// WAL file size at last checkpoint (for size-based delta calculations)
    pub checkpointed_wal_size: u64,
}

impl Default for CheckpointManagerState {
    fn default() -> Self {
        Self {
            current_state: CheckpointState::Idle,
            checkpointed_lsn: 0,
            in_progress: false,
            last_checkpoint: None,
            current_operation_id: 0,
            completed_checkpoints: 0,
            failed_attempts: 0,
            checkpoint_start_time: None,
            transactions_since_checkpoint: 0,
            checkpointed_wal_size: 0,
        }
    }
}

/// Dirty block tracker for V2 clustered edge format
///
/// Tracks modified blocks by cluster affinity to optimize I/O patterns
/// for V2's clustered edge storage architecture.
#[derive(Debug, Default)]
pub struct DirtyBlockTracker {
    /// Dirty blocks organized by cluster key (node_id for V2 clustering)
    cluster_dirty_blocks: HashMap<i64, HashSet<u64>>,

    /// Global dirty blocks not associated with specific clusters
    global_dirty_blocks: HashSet<u64>,

    /// Last modified timestamp per block (for LRU and ordering decisions)
    block_timestamps: HashMap<u64, u64>,

    /// Access frequency per block (for optimization heuristics)
    block_access_counts: HashMap<u64, u64>,

    /// Block metadata for V2-specific optimization
    block_metadata: HashMap<u64, BlockMetadata>,

    /// Maximum dirty blocks allowed per cluster (prevents memory exhaustion)
    max_blocks_per_cluster: usize,

    /// Maximum global dirty blocks allowed
    max_global_blocks: usize,
}

/// V2-specific block metadata for optimization
#[derive(Debug, Clone)]
pub struct BlockMetadata {
    /// Block size in bytes
    pub size: u64,

    /// Cluster affinity for this block
    pub cluster_key: Option<i64>,

    /// Block type classification
    pub block_type: V2BlockType,

    /// Access pattern statistics
    pub access_pattern: AccessPattern,

    /// Priority for checkpointing (higher = more important)
    pub priority: u8,
}

/// V2 block type classification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum V2BlockType {
    /// Node record block containing V2 NodeRecordV2 entries
    NodeRecord,

    /// Edge cluster block containing V2 compact edge records
    EdgeCluster,

    /// String table block for V2 string storage
    StringTable,

    /// Free space block for V2 free list management
    FreeSpace,

    /// Metadata block for V2 file headers and indexes
    Metadata,

    /// Unknown block type
    Unknown,
}

/// Access pattern classification for dirty blocks
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessPattern {
    /// Sequential access (good for I/O optimization)
    Sequential,

    /// Random access (requires different optimization)
    Random,

    /// Burst access (concentrated in time windows)
    Burst,

    /// Sparse access (infrequent access)
    Sparse,
}

/// Checkpoint progress information for tracking checkpoint execution
#[derive(Debug, Clone)]
pub struct CheckpointProgress {
    /// Current checkpoint LSN range
    pub lsn_range: (u64, u64),

    /// Total records to checkpoint
    pub total_records: u64,

    /// Records processed so far
    pub processed_records: u64,

    /// Blocks flushed so far
    pub flushed_blocks: u64,

    /// Estimated completion percentage
    pub completion_percentage: f64,

    /// Checkpoint start time
    pub checkpoint_start: Instant,
}

impl V2WALCheckpointManager {
    /// Create a new checkpoint manager with the given configuration and strategy
    ///
    /// # Arguments
    ///
    /// * `config` - WAL configuration including file paths and limits
    /// * `strategy` - Checkpoint strategy for determining when to checkpoint
    ///
    /// # Returns
    ///
    /// * `CheckpointResult<V2WALCheckpointManager>` - Result containing the manager or error
    ///
    /// # Examples
    ///
    /// ```rust
    /// use std::time::Duration;
    /// use sqlitegraph::backend::native::v2::wal::checkpoint::{
    ///     V2WALCheckpointManager, CheckpointStrategy
    /// };
    ///
    /// let manager = V2WALCheckpointManager::create(config, CheckpointStrategy::TimeInterval(Duration::from_secs(300)))?;
    /// ```
    pub fn create(config: V2WALConfig, strategy: CheckpointStrategy) -> CheckpointResult<Self> {
        // Validate configuration first
        Self::validate_configuration(&config)?;

        // Create checkpoint directory structure
        Self::ensure_checkpoint_directory(&config)?;

        // Initialize checkpoint file
        let checkpoint_file = Self::create_checkpoint_file(&config)?;

        // Initialize checkpoint state
        let state = CheckpointManagerState::default();
        let strategy_arc = Arc::new(Mutex::new(strategy));
        let dirty_blocks = Arc::new(Mutex::new(DirtyBlockTracker::new(
            MAX_DIRTY_BLOCKS_PER_CLUSTER,
            MAX_GLOBAL_DIRTY_BLOCKS,
        )));

        // Create checkpoint executor
        let executor = CheckpointExecutor::new(config.clone())?;

        Ok(Self {
            config,
            checkpoint_file: Arc::new(Mutex::new(BufWriter::new(checkpoint_file))),
            state: Arc::new(Mutex::new(state)),
            strategy: strategy_arc,
            dirty_blocks,
            executor: Arc::new(Mutex::new(executor)),
            checkpoint_cv: Arc::new(Condvar::new()),
            shutdown_flag: Arc::new(Mutex::new(false)),
        })
    }

    /// Get current checkpoint state
    pub fn get_state(&self) -> CheckpointState {
        let state = self.state.lock();
        state.current_state.clone()
    }

    /// Check if a checkpoint is currently in progress
    pub fn is_checkpoint_in_progress(&self) -> bool {
        let state = self.state.lock();
        state.in_progress
    }

    /// Get last checkpointed LSN
    pub fn get_last_checkpointed_lsn(&self) -> u64 {
        let state = self.state.lock();
        state.checkpointed_lsn
    }

    /// Get checkpoint completion statistics
    pub fn get_checkpoint_statistics(&self) -> (u64, u64, u64) {
        let state = self.state.lock();
        (
            state.completed_checkpoints,
            state.failed_attempts,
            state.current_operation_id,
        )
    }

    /// Mark a block as dirty for checkpointing
    ///
    /// # Arguments
    ///
    /// * `block_offset` - File offset of the dirty block
    /// * `cluster_key` - Optional cluster key for V2 clustering affinity
    ///
    /// # Returns
    ///
    /// * `CheckpointResult<()>` - Result indicating success or error
    pub fn mark_block_dirty(
        &self,
        block_offset: u64,
        cluster_key: Option<i64>,
    ) -> CheckpointResult<()> {
        let mut dirty_blocks = self.dirty_blocks.lock();

        // Validate block parameters
        if block_offset == 0 {
            return Err(CheckpointError::validation("Block offset cannot be zero"));
        }

        if block_offset % v2::V2_GRAPH_BLOCK_SIZE != 0 {
            return Err(CheckpointError::validation(format!(
                "Block offset {} is not aligned to V2 graph block size",
                block_offset
            )));
        }

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| CheckpointError::io(format!("Failed to get timestamp: {}", e)))?
            .as_secs();

        // Add to appropriate tracking structure
        if let Some(key) = cluster_key {
            dirty_blocks.mark_cluster_block_dirty(key, block_offset, timestamp)?;
        } else {
            dirty_blocks.mark_global_block_dirty(block_offset, timestamp)?;
        }

        // Update access statistics
        dirty_blocks.update_block_access(block_offset, timestamp);

        Ok(())
    }

    /// Mark multiple blocks as dirty efficiently
    ///
    /// # Arguments
    ///
    /// * `block_offsets` - Iterator of block offsets to mark as dirty
    /// * `cluster_key` - Optional cluster key for all blocks
    ///
    /// # Returns
    ///
    /// * `CheckpointResult<u64>` - Number of blocks successfully marked
    pub fn mark_blocks_dirty<I>(
        &self,
        block_offsets: I,
        cluster_key: Option<i64>,
    ) -> CheckpointResult<u64>
    where
        I: IntoIterator<Item = u64>,
    {
        let mut _dirty_blocks = self.dirty_blocks.lock();
        let mut marked_count = 0;

        for block_offset in block_offsets {
            if let Err(e) = self.mark_block_dirty(block_offset, cluster_key) {
                // Log error but continue with other blocks
                eprintln!(
                    "Warning: Failed to mark block {} as dirty: {}",
                    block_offset, e
                );
                continue;
            }
            marked_count += 1;
        }

        Ok(marked_count)
    }

    /// Check if checkpointing should be performed based on current strategy
    ///
    /// # Returns
    ///
    /// * `CheckpointResult<bool>` - Result indicating if checkpoint should be performed
    pub fn should_checkpoint(&self) -> CheckpointResult<bool> {
        let strategy = self.strategy.lock();
        let state = self.state.lock();

        // Don't checkpoint if shutdown is requested
        if *self.shutdown_flag.lock() {
            return Ok(false);
        }

        // Don't checkpoint if already in progress
        if state.in_progress {
            return Ok(false);
        }

        // Don't checkpoint if in error state
        if matches!(state.current_state, CheckpointState::Failed) {
            return Ok(false);
        }

        // Delegate to strategy module for actual evaluation
        {
            let dirty_blocks = self.dirty_blocks.lock();
            self.evaluate_checkpoint_strategy(&*strategy, &*dirty_blocks, &state)
        }
    }

    /// Get current WAL file size for monitoring
    ///
    /// # Returns
    ///
    /// * `CheckpointResult<u64>` - Current WAL file size in bytes
    pub fn get_wal_size(&self) -> CheckpointResult<u64> {
        std::fs::metadata(&self.config.wal_path)
            .map(|m| m.len())
            .map_err(|e| CheckpointError::io(format!("Failed to get WAL size: {}", e)))
    }

    /// Force checkpoint regardless of strategy (emergency/manual checkpoint)
    ///
    /// # Returns
    ///
    /// * `CheckpointResult<CheckpointProgress>` - Result containing checkpoint progress
    pub fn force_checkpoint(&self) -> CheckpointResult<CheckpointProgress> {
        let start_time = Instant::now();

        // Transition to collecting state
        {
            let mut state = self.state.lock();
            if state.in_progress {
                return Err(CheckpointError::state("Checkpoint already in progress"));
            }
            state.in_progress = true;
            state.current_state = CheckpointState::Collecting;
            state.checkpoint_start_time = Some(start_time);
            state.current_operation_id += 1;
        }

        // Perform checkpoint without strategy validation
        let result = self.execute_checkpoint(start_time, true);

        // Update state regardless of outcome
        {
            let mut state = self.state.lock();
            state.in_progress = false;

            if result.is_ok() {
                state.current_state = CheckpointState::Complete;
                state.last_checkpoint = Some(start_time);
                state.completed_checkpoints += 1;
                state.checkpointed_lsn = self.get_last_checkpointed_lsn(); // Update checkpointed LSN
                state.transactions_since_checkpoint = 0; // Reset transaction counter
                state.checkpointed_wal_size = self.get_wal_size().unwrap_or(0); // Reset WAL size tracking
            } else {
                state.current_state = CheckpointState::Failed;
                state.failed_attempts += 1;
            }
        }

        // Notify waiting threads
        self.checkpoint_cv.notify_all();

        result
    }

    /// Perform incremental checkpoint with strategy validation
    ///
    /// # Returns
    ///
    /// * `CheckpointResult<CheckpointProgress>` - Result containing checkpoint progress
    pub fn checkpoint(&self) -> CheckpointResult<CheckpointProgress> {
        let start_time = Instant::now();

        // Check if checkpoint should be performed
        if !self.should_checkpoint()? {
            return Err(CheckpointError::state(
                "Checkpoint not required based on strategy",
            ));
        }

        // Transition to collecting state
        {
            let mut state = self.state.lock();
            if state.in_progress {
                return Err(CheckpointError::state("Checkpoint already in progress"));
            }
            state.in_progress = true;
            state.current_state = CheckpointState::Collecting;
            state.checkpoint_start_time = Some(start_time);
            state.current_operation_id += 1;
        }

        // Execute checkpoint with strategy validation
        let result = self.execute_checkpoint(start_time, false);

        // Update state and metrics
        {
            let mut state = self.state.lock();
            state.in_progress = false;

            if result.is_ok() {
                state.current_state = CheckpointState::Complete;
                state.last_checkpoint = Some(start_time);
                state.completed_checkpoints += 1;
            } else {
                state.current_state = CheckpointState::Failed;
                state.failed_attempts += 1;
            }
        }

        // Notify waiting threads
        self.checkpoint_cv.notify_all();

        result
    }

    /// Wait for current checkpoint to complete
    ///
    /// # Arguments
    ///
    /// * `timeout` - Maximum time to wait
    ///
    /// # Returns
    ///
    /// * `bool` - True if checkpoint completed, false if timeout
    pub fn wait_for_checkpoint(&self, timeout: Duration) -> bool {
        let state = self.state.lock();
        let mut guard = state;

        let start_time = Instant::now();

        while guard.in_progress {
            let remaining_timeout = timeout.saturating_sub(start_time.elapsed());
            if remaining_timeout.is_zero() {
                return false;
            }

            let result = self.checkpoint_cv.wait_for(&mut guard, remaining_timeout);
            if result.timed_out() {
                return false;
            }
        }

        true
    }

    /// Shutdown checkpoint manager gracefully
    ///
    /// # Returns
    ///
    /// * `CheckpointResult<()>` - Result indicating success or error
    pub fn shutdown(&self) -> CheckpointResult<()> {
        // Set shutdown flag
        {
            let mut shutdown_flag = self.shutdown_flag.lock();
            *shutdown_flag = true;
        }

        // Wait for any in-progress checkpoint to complete
        if self.is_checkpoint_in_progress() {
            if !self.wait_for_checkpoint(Duration::from_secs(30)) {
                return Err(CheckpointError::timeout(
                    "Checkpoint did not complete during shutdown",
                ));
            }
        }

        // Force final checkpoint if needed
        if self.should_checkpoint()? {
            let _ = self.force_checkpoint();
        }

        // Flush and close checkpoint file
        {
            let mut checkpoint_file = self.checkpoint_file.lock();
            checkpoint_file.flush().map_err(|e| {
                CheckpointError::io(format!("Failed to flush checkpoint file: {}", e))
            })?;
        }

        Ok(())
    }

    // Private helper methods

    /// Validate checkpoint configuration
    fn validate_configuration(config: &V2WALConfig) -> CheckpointResult<()> {
        config
            .validate()
            .map_err(|e| CheckpointError::configuration(e.to_string()))?;

        // Validate checkpoint path
        if config.checkpoint_path.as_path().parent().is_none() {
            return Err(CheckpointError::configuration(
                "Checkpoint path must have a valid parent directory",
            ));
        }

        Ok(())
    }

    /// Ensure checkpoint directory exists
    fn ensure_checkpoint_directory(config: &V2WALConfig) -> CheckpointResult<()> {
        if let Some(parent) = config.checkpoint_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CheckpointError::io(format!("Failed to create checkpoint directory: {}", e))
            })?;
        }
        Ok(())
    }

    /// Create and open checkpoint file
    fn create_checkpoint_file(config: &V2WALConfig) -> CheckpointResult<File> {
        std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(&config.checkpoint_path)
            .map_err(|e| CheckpointError::io(format!("Failed to create checkpoint file: {}", e)))
    }

    /// Execute checkpoint operation (internal implementation)
    fn execute_checkpoint(
        &self,
        _start_time: Instant,
        _force: bool,
    ) -> CheckpointResult<CheckpointProgress> {
        // Transition to processing state
        {
            let mut state = self.state.lock();
            state.current_state = CheckpointState::Processing;
        }

        // Delegate to executor for actual checkpoint work
        let executor = self.executor.lock();
        let manager_state = self.state.lock();
        let dirty_blocks = self.dirty_blocks.lock();
        let progress = executor
            .execute_incremental_checkpoint(
                &manager_state.current_state,
                &*dirty_blocks,
                0,
                u64::MAX,
            )
            .map_err(|e| {
                // Transition to failed state on error
                drop(manager_state); // Release the lock before reacquiring
                let mut state = self.state.lock();
                state.current_state = CheckpointState::Failed;
                e
            })?;

        Ok(progress)
    }

    /// Evaluate checkpoint strategy (delegated to strategies module)
    fn evaluate_checkpoint_strategy(
        &self,
        strategy: &CheckpointStrategy,
        _dirty_blocks: &DirtyBlockTracker,
        state: &CheckpointManagerState,
    ) -> CheckpointResult<bool> {
        // This method will delegate to the strategies module
        // For now, implement basic evaluation logic
        match strategy {
            CheckpointStrategy::TimeInterval(interval) => {
                if let Some(last_checkpoint) = state.last_checkpoint {
                    Ok(last_checkpoint.elapsed() >= *interval)
                } else {
                    Ok(true) // First checkpoint
                }
            }
            CheckpointStrategy::TransactionCount(threshold) => {
                // Use actual transaction counter from CheckpointManagerState
                Ok(state.transactions_since_checkpoint >= *threshold)
            }
            CheckpointStrategy::SizeThreshold(threshold) => {
                // Read actual WAL file size
                let wal_size = std::fs::metadata(&self.config.wal_path)
                    .map(|m| m.len())
                    .unwrap_or(0);

                Ok(wal_size >= *threshold)
            }
            CheckpointStrategy::Adaptive { .. } => {
                Ok(false) // TODO: Implement adaptive strategy
            }
        }
    }
}

impl DirtyBlockTracker {
    /// Create new dirty block tracker with capacity limits
    pub fn new(max_blocks_per_cluster: usize, max_global_blocks: usize) -> Self {
        Self {
            cluster_dirty_blocks: HashMap::new(),
            global_dirty_blocks: HashSet::new(),
            block_timestamps: HashMap::new(),
            block_access_counts: HashMap::new(),
            block_metadata: HashMap::new(),
            max_blocks_per_cluster,
            max_global_blocks,
        }
    }

    /// Mark cluster-specific block as dirty
    pub fn mark_cluster_block_dirty(
        &mut self,
        cluster_key: i64,
        block_offset: u64,
        _timestamp: u64,
    ) -> CheckpointResult<()> {
        let cluster_blocks = self
            .cluster_dirty_blocks
            .entry(cluster_key)
            .or_insert_with(HashSet::new);

        // Enforce capacity limits
        if cluster_blocks.len() >= self.max_blocks_per_cluster {
            return Err(CheckpointError::resource(format!(
                "Maximum dirty blocks per cluster exceeded for cluster {}",
                cluster_key
            )));
        }

        cluster_blocks.insert(block_offset);
        Ok(())
    }

    /// Mark global block as dirty
    pub fn mark_global_block_dirty(
        &mut self,
        block_offset: u64,
        _timestamp: u64,
    ) -> CheckpointResult<()> {
        // Enforce capacity limits
        if self.global_dirty_blocks.len() >= self.max_global_blocks {
            return Err(CheckpointError::resource(
                "Maximum global dirty blocks exceeded",
            ));
        }

        self.global_dirty_blocks.insert(block_offset);
        Ok(())
    }

    /// Update block access statistics
    pub fn update_block_access(&mut self, block_offset: u64, timestamp: u64) {
        self.block_timestamps.insert(block_offset, timestamp);
        *self.block_access_counts.entry(block_offset).or_insert(0) += 1;
    }

    /// Get dirty blocks for checkpointing (both cluster and global)
    pub fn get_dirty_blocks_for_checkpoint(&self) -> Vec<u64> {
        let mut blocks = Vec::new();

        // Add cluster dirty blocks
        for cluster_blocks in self.cluster_dirty_blocks.values() {
            blocks.extend(cluster_blocks.iter().copied());
        }

        // Add global dirty blocks
        blocks.extend(self.global_dirty_blocks.iter().copied());

        // Sort for optimal I/O patterns
        blocks.sort_unstable();
        blocks
    }

    /// Clear checkpointed blocks from tracking
    pub fn clear_checkpointed_blocks(&mut self, checkpointed_blocks: &[u64]) {
        // Remove from global dirty blocks
        for &block_offset in checkpointed_blocks {
            self.global_dirty_blocks.remove(&block_offset);
        }

        // Remove from cluster dirty blocks
        for cluster_blocks in self.cluster_dirty_blocks.values_mut() {
            for &block_offset in checkpointed_blocks {
                cluster_blocks.remove(&block_offset);
            }
        }

        // Clean up tracking metadata
        for &block_offset in checkpointed_blocks {
            self.block_timestamps.remove(&block_offset);
            self.block_access_counts.remove(&block_offset);
            self.block_metadata.remove(&block_offset);
        }

        // Remove empty cluster entries
        self.cluster_dirty_blocks
            .retain(|_, blocks| !blocks.is_empty());
    }

    /// Get dirty block statistics
    pub fn get_statistics(&self) -> (usize, usize) {
        let cluster_blocks: usize = self
            .cluster_dirty_blocks
            .values()
            .map(|set| set.len())
            .sum();
        let global_blocks = self.global_dirty_blocks.len();
        (cluster_blocks, global_blocks)
    }

    /// Get immutable reference to global dirty blocks
    pub fn global_dirty_blocks(&self) -> &HashSet<u64> {
        &self.global_dirty_blocks
    }

    /// Get immutable reference to block timestamps
    pub fn block_timestamps(&self) -> &HashMap<u64, u64> {
        &self.block_timestamps
    }

    /// Get immutable reference to cluster dirty blocks
    pub fn cluster_dirty_blocks(&self) -> &HashMap<i64, HashSet<u64>> {
        &self.cluster_dirty_blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::GraphFile;
    use std::time::Duration;
    use tempfile::tempdir;

    #[test]
    fn test_checkpoint_manager_creation() -> CheckpointResult<()> {
        let temp_dir = tempdir().unwrap();
        let v2_graph_path = temp_dir.path().join("test.v2");

        // Create a minimal V2 graph file for testing (same as working test)
        let _graph_file = GraphFile::create(&v2_graph_path).map_err(|e| {
            CheckpointError::v2_integration(format!("Failed to create test graph file: {}", e))
        })?;

        let wal_path = temp_dir.path().join("test.wal");
        let config = V2WALConfig {
            wal_path,
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            max_wal_size: 64 * 1024 * 1024, // 64MB
            buffer_size: 1024 * 1024,       // 1MB
            checkpoint_interval: 100,
            enable_compression: false,
            ..Default::default()
        };

        let strategy = CheckpointStrategy::TimeInterval(Duration::from_secs(60));

        let manager = V2WALCheckpointManager::create(config, strategy)?;
        assert_eq!(manager.get_state(), CheckpointState::Idle);
        assert!(!manager.is_checkpoint_in_progress());
        assert_eq!(manager.get_last_checkpointed_lsn(), 0);

        Ok(())
    }

    #[test]
    fn test_mark_block_dirty() -> CheckpointResult<()> {
        let temp_dir = tempdir().unwrap();
        let v2_graph_path = temp_dir.path().join("test.v2");

        // Create a minimal V2 graph file for testing (same as working test)
        let _graph_file = GraphFile::create(&v2_graph_path).map_err(|e| {
            CheckpointError::v2_integration(format!("Failed to create test graph file: {}", e))
        })?;

        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            max_wal_size: 64 * 1024 * 1024, // 64MB
            buffer_size: 1024 * 1024,       // 1MB
            checkpoint_interval: 100,
            enable_compression: false,
            ..Default::default()
        };

        let strategy = CheckpointStrategy::TimeInterval(Duration::from_secs(60));
        let manager = V2WALCheckpointManager::create(config, strategy)?;

        // Mark cluster-specific dirty block
        manager.mark_block_dirty(4096, Some(42))?;

        // Mark global dirty block
        manager.mark_block_dirty(8192, None)?;

        let (cluster_blocks, global_blocks) = manager.dirty_blocks.lock().get_statistics();
        assert_eq!(cluster_blocks, 1);
        assert_eq!(global_blocks, 1);

        Ok(())
    }

    #[test]
    fn test_mark_invalid_block() {
        let temp_dir = tempdir().unwrap();
        let v2_graph_path = temp_dir.path().join("test.v2");

        // Create a minimal V2 graph file for testing (same as working test)
        let _graph_file =
            GraphFile::create(&v2_graph_path).expect("Failed to create test V2 graph file");

        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            max_wal_size: 64 * 1024 * 1024, // 64MB
            buffer_size: 1024 * 1024,       // 1MB
            checkpoint_interval: 100,
            enable_compression: false,
            ..Default::default()
        };

        let strategy = CheckpointStrategy::TimeInterval(Duration::from_secs(60));
        let manager = V2WALCheckpointManager::create(config, strategy).unwrap();

        // Try to mark invalid block offset (not aligned)
        let result = manager.mark_block_dirty(100, Some(42));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err().kind,
            CheckpointErrorKind::Validation
        ));
    }

    #[test]
    fn test_checkpoint_statistics() -> CheckpointResult<()> {
        let temp_dir = tempdir().unwrap();
        let v2_graph_path = temp_dir.path().join("test.v2");

        // Create a minimal V2 graph file for testing (same as working test)
        let _graph_file = GraphFile::create(&v2_graph_path).map_err(|e| {
            CheckpointError::v2_integration(format!("Failed to create test graph file: {}", e))
        })?;

        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            max_wal_size: 64 * 1024 * 1024, // 64MB
            buffer_size: 1024 * 1024,       // 1MB
            checkpoint_interval: 100,
            enable_compression: false,
            ..Default::default()
        };

        let strategy = CheckpointStrategy::TimeInterval(Duration::from_secs(60));
        let manager = V2WALCheckpointManager::create(config, strategy)?;

        let (completed, failed, operation_id) = manager.get_checkpoint_statistics();
        assert_eq!(completed, 0);
        assert_eq!(failed, 0);
        assert_eq!(operation_id, 0);

        Ok(())
    }

    #[test]
    fn test_dirty_block_tracker_capacity_limits() {
        let mut tracker = DirtyBlockTracker::new(2, 5); // Small limits for testing

        // Add cluster blocks up to limit
        tracker.mark_cluster_block_dirty(1, 4096, 100).unwrap();
        tracker.mark_cluster_block_dirty(1, 8192, 101).unwrap();

        // Third block should fail
        let result = tracker.mark_cluster_block_dirty(1, 12288, 102);
        assert!(result.is_err());

        // Add global blocks up to limit
        tracker.mark_global_block_dirty(16384, 103).unwrap();
        tracker.mark_global_block_dirty(20480, 104).unwrap();
        tracker.mark_global_block_dirty(24576, 105).unwrap();
        tracker.mark_global_block_dirty(28672, 106).unwrap();
        tracker.mark_global_block_dirty(32768, 107).unwrap();

        // Sixth global block should fail (exceeds limit of 5)
        let result = tracker.mark_global_block_dirty(36864, 108);
        assert!(result.is_err());
    }

    #[test]
    fn test_dirty_block_tracker_operations() {
        let mut tracker = DirtyBlockTracker::new(100, 1000);

        // Mark blocks
        tracker.mark_cluster_block_dirty(1, 4096, 100).unwrap();
        tracker.mark_global_block_dirty(8192, 101).unwrap();
        tracker.mark_cluster_block_dirty(2, 12288, 102).unwrap();

        // Get dirty blocks for checkpointing
        let blocks = tracker.get_dirty_blocks_for_checkpoint();
        assert_eq!(blocks.len(), 3);
        assert!(blocks.contains(&4096));
        assert!(blocks.contains(&8192));
        assert!(blocks.contains(&12288));

        // Clear some blocks
        tracker.clear_checkpointed_blocks(&[4096, 8192]);

        let remaining_blocks = tracker.get_dirty_blocks_for_checkpoint();
        assert_eq!(remaining_blocks.len(), 1);
        assert!(remaining_blocks.contains(&12288));
    }

    #[test]
    fn test_checkpoint_state_transitions() {
        let temp_dir = tempdir().unwrap();
        let v2_graph_path = temp_dir.path().join("test.v2");

        // Create a minimal V2 graph file for testing (same as working test)
        let _graph_file =
            GraphFile::create(&v2_graph_path).expect("Failed to create test V2 graph file");

        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            max_wal_size: 64 * 1024 * 1024, // 64MB
            buffer_size: 1024 * 1024,       // 1MB
            checkpoint_interval: 100,
            enable_compression: false,
            ..Default::default()
        };

        let strategy = CheckpointStrategy::TimeInterval(Duration::from_secs(1)); // Very short interval
        let manager = V2WALCheckpointManager::create(config, strategy).unwrap();

        assert_eq!(manager.get_state(), CheckpointState::Idle);

        // Start checkpoint (but don't complete due to missing WAL file)
        let result = manager.checkpoint();
        assert!(result.is_err()); // Expected to fail due to missing WAL

        // State should have transitioned back from Collecting to Failed
        assert_eq!(manager.get_state(), CheckpointState::Failed);
    }
}
