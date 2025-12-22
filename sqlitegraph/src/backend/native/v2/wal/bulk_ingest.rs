//! Bulk Ingest Mode for V2 WAL System
//!
//! This module provides bulk ingest optimization for large-scale data loading operations.
//! Bulk ingest mode optimizes initial load scenarios by:
//! - Batching WAL writes to reduce I/O overhead
//! - Deferring expensive maintenance operations
//! - Using checkpoints at natural bulk boundaries
//! - Ensuring crash-recovery consistency through WAL replay
//!
//! # Usage
//!
//! ```rust
//! let manager = V2WALManager::create(config)?;
//! let bulk_guard = manager.begin_bulk_ingest(BulkIngestConfig::default())?;
//!
//! // Perform bulk operations...
//! for record in bulk_records {
//!     manager.write_record(record)?;
//! }
//!
//! // Automatically exits bulk mode and flushes when guard is dropped
//! drop(bulk_guard);
//! ```

use super::{V2WALManager, WALManagerMetrics};
use crate::backend::native::NativeResult;

/// Configuration for bulk ingest mode
#[derive(Debug, Clone)]
pub struct BulkIngestConfig {
    /// Maximum batch size for bulk operations (default: 10MB)
    pub max_batch_size_bytes: usize,

    /// Buffer flush timeout during bulk mode (default: 5 seconds)
    pub flush_timeout_ms: u64,

    /// Whether to force checkpoint when exiting bulk mode (default: true)
    pub force_checkpoint_on_exit: bool,

    /// Maximum number of records to batch before auto-flush (default: 10000)
    pub max_records_per_batch: usize,
}

impl Default for BulkIngestConfig {
    fn default() -> Self {
        Self {
            max_batch_size_bytes: 10 * 1024 * 1024, // 10MB
            flush_timeout_ms: 5_000,                // 5 seconds
            force_checkpoint_on_exit: true,
            max_records_per_batch: 10_000,
        }
    }
}

/// RAII guard for bulk ingest mode
///
/// Automatically exits bulk ingest mode and flushes pending changes when dropped.
/// This ensures crash recovery consistency even if the bulk operation is interrupted.
pub struct BulkIngestGuard<'a> {
    manager: &'a V2WALManager,
    config: BulkIngestConfig,
    records_written: u64,
    start_metrics: WALManagerMetrics,
}

impl<'a> BulkIngestGuard<'a> {
    /// Create a new bulk ingest guard
    pub(crate) fn new(manager: &'a V2WALManager, config: BulkIngestConfig) -> NativeResult<Self> {
        let start_metrics = manager.get_metrics();

        // Enable bulk mode optimizations in the writer
        manager.enable_bulk_mode(&config)?;

        Ok(Self {
            manager,
            config,
            records_written: 0,
            start_metrics,
        })
    }

    /// Get the number of records written during this bulk session
    pub fn records_written(&self) -> u64 {
        self.records_written
    }

    /// Get the start metrics when bulk mode began
    pub fn start_metrics(&self) -> &WALManagerMetrics {
        &self.start_metrics
    }

    /// Force flush of pending bulk writes
    pub fn flush(&mut self) -> NativeResult<()> {
        self.manager.flush()?;
        Ok(())
    }

    /// Complete bulk ingest manually (also happens on drop)
    pub fn complete(mut self) -> NativeResult<()> {
        self.finish_bulk_session()
    }

    /// Internal method to complete the bulk session
    fn finish_bulk_session(&mut self) -> NativeResult<()> {
        // Flush any remaining buffered writes
        self.manager.flush()?;

        // Disable bulk mode optimizations
        self.manager.disable_bulk_mode()?;

        // Force checkpoint if configured
        if self.config.force_checkpoint_on_exit {
            self.manager.force_checkpoint()?;
        }

        Ok(())
    }
}

impl<'a> Drop for BulkIngestGuard<'a> {
    fn drop(&mut self) {
        // Ensure bulk session is completed even if panic occurs
        let _ = self.finish_bulk_session();
    }
}

/// Extension trait for V2WALManager to support bulk ingest
pub trait BulkIngestExt {
    /// Begin bulk ingest mode with the given configuration
    ///
    /// Returns a RAII guard that automatically exits bulk mode when dropped.
    /// While bulk mode is active:
    /// - WAL writes are batched more aggressively
    /// - Flush operations are deferred to reduce I/O overhead
    /// - Checkpoint boundaries are optimized for bulk operations
    /// - All crash recovery semantics are preserved
    fn begin_bulk_ingest(&self, config: BulkIngestConfig) -> NativeResult<BulkIngestGuard>;

    /// Check if bulk ingest mode is currently active
    fn is_bulk_ingest_active(&self) -> bool;

    /// Get bulk ingest performance metrics
    fn get_bulk_metrics(&self) -> BulkIngestMetrics;
}

/// Bulk ingest specific performance metrics
#[derive(Debug, Clone)]
pub struct BulkIngestMetrics {
    /// Number of bulk ingest sessions completed
    pub sessions_completed: u64,

    /// Total records processed in bulk mode
    pub total_bulk_records: u64,

    /// Average batch size during bulk operations
    pub avg_batch_size: f64,

    /// Total time spent in bulk mode (milliseconds)
    pub total_bulk_time_ms: u64,

    /// Performance improvement ratio (vs non-bulk)
    pub performance_improvement_ratio: f64,
}

impl BulkIngestExt for V2WALManager {
    fn begin_bulk_ingest(&self, config: BulkIngestConfig) -> NativeResult<BulkIngestGuard> {
        BulkIngestGuard::new(self, config)
    }

    fn is_bulk_ingest_active(&self) -> bool {
        // Call the manager method directly
        V2WALManager::is_bulk_mode_active(self)
    }

    fn get_bulk_metrics(&self) -> BulkIngestMetrics {
        // For now, return default metrics - will be enhanced when we track bulk stats
        BulkIngestMetrics {
            sessions_completed: 0,
            total_bulk_records: 0,
            avg_batch_size: 0.0,
            total_bulk_time_ms: 0,
            performance_improvement_ratio: 1.0,
        }
    }
}
