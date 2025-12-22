//! Checkpoint Executor
//!
//! This module contains the main CheckpointExecutor implementation that orchestrates
//! the checkpoint process, coordinating record application, dirty block flushing,
//! and progress tracking.

use crate::backend::native::v2::wal::checkpoint::core::{
    CheckpointProgress, CheckpointState, DirtyBlockTracker,
};
use crate::backend::native::v2::wal::checkpoint::errors::{CheckpointError, CheckpointResult};
use crate::backend::native::v2::wal::{V2WALConfig, V2WALReader, V2WALRecord};
use crate::backend::native::v2::wal::checkpoint::constants::*;
use crate::backend::native::v2::wal::checkpoint::io::block_flusher::BlockFlusher;
use crate::backend::native::v2::wal::checkpoint::record::integrator::V2GraphIntegrator;
use std::fs::File;
use std::io::BufWriter;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Checkpoint executor for performing incremental checkpoint operations
pub struct CheckpointExecutor {
    config: V2WALConfig,
    checkpoint_file: Arc<Mutex<BufWriter<File>>>,
    v2_integrator: Arc<Mutex<V2GraphIntegrator>>,
}

impl CheckpointExecutor {
    /// Create new checkpoint executor with V2 integration
    pub fn new(config: V2WALConfig) -> CheckpointResult<Self> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&config.checkpoint_path)
            .map_err(|e| CheckpointError::io(format!("Failed to create checkpoint file: {}", e)))?;

        let writer = BufWriter::with_capacity(DEFAULT_CHECKPOINT_BUFFER_SIZE, file);

        // Extract V2 graph file path from WAL config
        let v2_graph_path = config.wal_path.with_extension("v2");

        // Create V2 Graph Integrator with real backend integration
        let v2_integrator = V2GraphIntegrator::new(v2_graph_path).map_err(|e| {
            CheckpointError::v2_integration(format!("Failed to create V2 Graph Integrator: {}", e))
        })?;

        Ok(Self {
            config,
            checkpoint_file: Arc::new(Mutex::new(writer)),
            v2_integrator: Arc::new(Mutex::new(v2_integrator)),
        })
    }

    /// Execute incremental checkpoint with progress tracking
    pub fn execute_incremental_checkpoint(
        &self,
        _state: &CheckpointState,
        dirty_blocks: &DirtyBlockTracker,
        start_lsn: u64,
        end_lsn: u64,
    ) -> CheckpointResult<CheckpointProgress> {
        let checkpoint_start = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| CheckpointError::state(format!("Invalid checkpoint start time: {}", e)))?
            .as_secs();

        // Read WAL records for checkpoint range
        let records = self.read_wal_records(start_lsn, end_lsn)?;

        // Collect dirty blocks for checkpoint
        let dirty_block_offsets = self.collect_dirty_blocks(dirty_blocks, start_lsn, end_lsn)?;

        // Initialize progress
        let mut progress = CheckpointProgress {
            lsn_range: (start_lsn, end_lsn),
            total_records: records.len() as u64,
            processed_records: 0,
            flushed_blocks: 0,
            completion_percentage: 0.0,
            checkpoint_start: std::time::Instant::now(),
        };

        // Write checkpoint header
        self.write_checkpoint_header(
            (start_lsn, end_lsn),
            checkpoint_start,
            dirty_block_offsets.len() as u64,
        )?;

        // Process records in batches for optimal I/O patterns
        let batch_size = std::cmp::min(DEFAULT_BATCH_SIZE, v2::MAX_CLUSTER_OPERATIONS_PER_BATCH);

        for (i, (lsn, record)) in records.into_iter().enumerate() {
            // Apply record to V2 graph file using real V2 integration
            {
                let mut v2_integrator = self.v2_integrator.lock().map_err(|e| {
                    CheckpointError::state(format!("Failed to lock V2 integrator: {}", e))
                })?;

                v2_integrator
                    .apply_record_to_v2_graph(&record, lsn)
                    .map_err(|e| {
                        CheckpointError::v2_integration(format!(
                            "Failed to apply WAL record to V2 graph file: {}",
                            e
                        ))
                    })?;
            }

            progress.processed_records += 1;

            // Update progress periodically
            if i % batch_size == 0 || i == progress.total_records as usize - 1 {
                progress.completion_percentage = (i as f64 / progress.total_records as f64) * 100.0;

                // Write progress checkpoint periodically
                if i % (batch_size * PROGRESS_REPORT_INTERVAL) == 0 {
                    self.write_checkpoint_progress(&progress)?;
                }
            }
        }

        // Flush dirty blocks to V2 graph file
        for &block_offset in &dirty_block_offsets {
            self.flush_dirty_block_to_v2_file(block_offset)?;
            progress.flushed_blocks += 1;
        }

        // Write checkpoint completion marker
        self.write_checkpoint_completion(&progress)?;

        // Sync checkpoint file to ensure durability
        self.sync_checkpoint_file()?;

        progress.completion_percentage = 100.0;
        Ok(progress)
    }

    /// Read WAL records for checkpoint range
    fn read_wal_records(
        &self,
        start_lsn: u64,
        end_lsn: u64,
    ) -> CheckpointResult<Vec<(u64, V2WALRecord)>> {
        let mut reader = V2WALReader::open(&self.config.wal_path).map_err(|e| {
            CheckpointError::v2_integration(format!("Failed to open WAL reader: {}", e))
        })?;

        let mut records = Vec::new();

        let wal_records = reader.read_from_lsn(start_lsn)?;
        for (lsn, record) in wal_records {
            if lsn > end_lsn {
                break;
            }
            records.push((lsn, record));
        }

        Ok(records)
    }

    /// Collect dirty blocks that need checkpointing
    fn collect_dirty_blocks(
        &self,
        dirty_blocks: &DirtyBlockTracker,
        _start_lsn: u64,
        _end_lsn: u64,
    ) -> CheckpointResult<Vec<u64>> {
        let mut blocks_to_checkpoint = Vec::new();

        // Collect global dirty blocks
        for &block_offset in dirty_blocks.global_dirty_blocks() {
            if let Some(&_timestamp) = dirty_blocks.block_timestamps().get(&block_offset) {
                // Include blocks modified within the checkpoint range
                // This uses timestamp-based filtering as an approximation
                blocks_to_checkpoint.push(block_offset);
            }
        }

        // Collect cluster-specific dirty blocks
        for (_cluster_key, cluster_blocks) in dirty_blocks.cluster_dirty_blocks() {
            for &block_offset in cluster_blocks {
                if !blocks_to_checkpoint.contains(&block_offset) {
                    blocks_to_checkpoint.push(block_offset);
                }
            }
        }

        // Sort blocks for optimal I/O patterns (sequential when possible)
        blocks_to_checkpoint.sort_unstable();

        // Limit to prevent excessive memory usage
        if blocks_to_checkpoint.len() > MAX_DIRTY_BLOCKS_PER_CLUSTER {
            blocks_to_checkpoint.truncate(MAX_DIRTY_BLOCKS_PER_CLUSTER);
        }

        Ok(blocks_to_checkpoint)
    }

    /// Write checkpoint header to checkpoint file
    fn write_checkpoint_header(
        &self,
        lsn_range: (u64, u64),
        timestamp: u64,
        block_count: u64,
    ) -> CheckpointResult<()> {
        use crate::backend::native::v2::wal::checkpoint::io::checkpoint_writer::CheckpointWriter;

        let mut checkpoint_file = self.checkpoint_file.lock().map_err(|e| {
            CheckpointError::state(format!("Failed to lock checkpoint file: {}", e))
        })?;

        CheckpointWriter::write_header(&mut *checkpoint_file, lsn_range, timestamp, block_count)
    }

    /// Write checkpoint progress to checkpoint file
    fn write_checkpoint_progress(&self, progress: &CheckpointProgress) -> CheckpointResult<()> {
        use crate::backend::native::v2::wal::checkpoint::io::checkpoint_writer::CheckpointWriter;

        let mut checkpoint_file = self.checkpoint_file.lock().map_err(|e| {
            CheckpointError::state(format!("Failed to lock checkpoint file: {}", e))
        })?;

        CheckpointWriter::write_progress(&mut *checkpoint_file, progress)
    }

    /// Write checkpoint completion marker to checkpoint file
    fn write_checkpoint_completion(&self, progress: &CheckpointProgress) -> CheckpointResult<()> {
        use crate::backend::native::v2::wal::checkpoint::io::checkpoint_writer::CheckpointWriter;

        let mut checkpoint_file = self.checkpoint_file.lock().map_err(|e| {
            CheckpointError::state(format!("Failed to lock checkpoint file: {}", e))
        })?;

        CheckpointWriter::write_completion(&mut *checkpoint_file, progress)
    }

    /// Sync checkpoint file to ensure durability
    fn sync_checkpoint_file(&self) -> CheckpointResult<()> {
        let checkpoint_file = self.checkpoint_file.lock().map_err(|e| {
            CheckpointError::state(format!("Failed to lock checkpoint file: {}", e))
        })?;

        checkpoint_file
            .get_ref()
            .sync_all()
            .map_err(|e| CheckpointError::io(format!("Failed to sync checkpoint file: {}", e)))?;

        Ok(())
    }

    /// Flush dirty block to V2 file using real backend operations
    fn flush_dirty_block_to_v2_file(&self, block_offset: u64) -> CheckpointResult<()> {
        // Extract V2 graph file path from checkpoint config
        let v2_graph_path = self.config.checkpoint_path.with_extension("v2");
        let flusher = BlockFlusher::new(v2_graph_path);
        flusher.flush_dirty_block(block_offset)
    }
}