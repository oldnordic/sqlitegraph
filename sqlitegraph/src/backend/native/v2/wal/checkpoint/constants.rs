//! V2 WAL Checkpoint Constants
//!
//! This module defines all constants used throughout the checkpoint system,
//! including performance targets, file format constants, and configuration limits.

/// Checkpoint file magic number for integrity validation
pub const CHECKPOINT_MAGIC: &[u8; 4] = b"CKPT";

/// Checkpoint file version
pub const CHECKPOINT_VERSION: u32 = 1;

/// Progress record marker
pub const PROGRESS_MAGIC: &[u8; 4] = b"PROG";

/// Completion record marker
pub const COMPLETION_MAGIC: &[u8; 4] = b"DONE";

/// Minimum checkpoint file size (bytes)
pub const MIN_CHECKPOINT_SIZE: u64 = 1024;

/// Maximum checkpoint file size (bytes) - 1GB
pub const MAX_CHECKPOINT_SIZE: u64 = 1024 * 1024 * 1024;

/// Default checkpoint batch size for processing records
pub const DEFAULT_BATCH_SIZE: usize = 1000;

/// Progress reporting interval (records)
pub const PROGRESS_REPORT_INTERVAL: usize = DEFAULT_BATCH_SIZE * 10;

/// Dirty block tracking maximum entries per cluster
pub const MAX_DIRTY_BLOCKS_PER_CLUSTER: usize = 10_000;

/// Global dirty block tracking maximum entries
pub const MAX_GLOBAL_DIRTY_BLOCKS: usize = 50_000;

/// Block access count tracking maximum
pub const MAX_BLOCK_ACCESS_COUNTS: usize = 100_000;

/// Checkpoint metrics smoothing factor (alpha for exponential moving average)
pub const METRICS_SMOOTHING_ALPHA: f64 = 0.1;

/// Default checkpoint timeout in milliseconds
pub const DEFAULT_CHECKPOINT_TIMEOUT_MS: u64 = 300_000; // 5 minutes

/// Checkpoint coordination wait timeout
pub const CHECKPOINT_WAIT_TIMEOUT_MS: u64 = 60_000; // 1 minute

/// Maximum checkpoint retry attempts
pub const MAX_CHECKPOINT_RETRIES: u32 = 3;

/// Checkpoint retry backoff multiplier
pub const CHECKPOINT_RETRY_BACKOFF: f64 = 2.0;

/// Minimum dirty block size for checkpointing (bytes)
pub const MIN_DIRTY_BLOCK_SIZE: u64 = 512;

/// Maximum dirty block size for checkpointing (bytes)
pub const MAX_DIRTY_BLOCK_SIZE: u64 = 64 * 1024; // 64KB

/// Default checkpoint file buffer size
pub const DEFAULT_CHECKPOINT_BUFFER_SIZE: usize = 64 * 1024; // 64KB

/// Checkpoint header size (bytes)
pub const CHECKPOINT_HEADER_SIZE: usize = 28; // 4 + 8 + 8 + 4 + 4

/// Progress record size (bytes)
pub const PROGRESS_RECORD_SIZE: usize = 16; // 4 + 8 + 4

/// Completion record size (bytes)
pub const COMPLETION_RECORD_SIZE: usize = 16; // 4 + 8 + 4

/// Checkpoint metadata reserved space (bytes)
pub const CHECKPOINT_METADATA_RESERVED: usize = 1024;

/// Default time-based checkpoint interval (seconds)
pub const DEFAULT_TIME_INTERVAL_SECONDS: u64 = 300; // 5 minutes

/// Default transaction-based checkpoint threshold
pub const DEFAULT_TRANSACTION_THRESHOLD: u64 = 1000;

/// Default size-based checkpoint threshold (bytes)
pub const DEFAULT_SIZE_THRESHOLD: u64 = 16 * 1024 * 1024; // 16MB

/// Maximum checkpoint entries in progress tracking
pub const MAX_PROGRESS_ENTRIES: usize = 10_000;

/// Dirty block timestamp resolution (milliseconds)
pub const DIRTY_BLOCK_TIMESTAMP_RESOLUTION_MS: u64 = 1000; // 1 second

/// Checkpoint file sync interval (records)
pub const CHECKPOINT_SYNC_INTERVAL: usize = 100;

/// Maximum concurrent checkpoint operations
pub const MAX_CONCURRENT_CHECKPOINTS: usize = 1; // Single-threaded for consistency

/// Checkpoint lock timeout (milliseconds)
pub const CHECKPOINT_LOCK_TIMEOUT_MS: u64 = 30_000; // 30 seconds

/// Checkpoint state transition timeout (milliseconds)
pub const CHECKPOINT_STATE_TIMEOUT_MS: u64 = 60_000; // 1 minute

/// Checkpoint validation timeout (milliseconds)
pub const CHECKPOINT_VALIDATION_TIMEOUT_MS: u64 = 10_000; // 10 seconds

/// Checkpoint I/O error retry count
pub const CHECKPOINT_IO_RETRY_COUNT: u32 = 3;

/// Checkpoint I/O retry delay (milliseconds)
pub const CHECKPOINT_IO_RETRY_DELAY_MS: u64 = 100;

/// V2 graph operation specific constants
pub mod v2 {
    /// Maximum number of cluster operations per checkpoint batch
    pub const MAX_CLUSTER_OPERATIONS_PER_BATCH: usize = 500;

    /// Maximum edge cluster operations per checkpoint batch
    pub const MAX_EDGE_CLUSTER_OPERATIONS_PER_BATCH: usize = 1000;

    /// Maximum node record operations per checkpoint batch
    pub const MAX_NODE_RECORD_OPERATIONS_PER_BATCH: usize = 750;

    /// Maximum string table operations per checkpoint batch
    pub const MAX_STRING_TABLE_OPERATIONS_PER_BATCH: usize = 250;

    /// Maximum free space operations per checkpoint batch
    pub const MAX_FREE_SPACE_OPERATIONS_PER_BATCH: usize = 100;

    /// V2 graph file block size for checkpointing
    pub const V2_GRAPH_BLOCK_SIZE: u64 = 4096; // 4KB blocks

    /// V2 graph file cluster alignment
    pub const V2_CLUSTER_ALIGNMENT: u64 = 64 * 1024; // 64KB alignment

    /// Maximum V2 dirty blocks per checkpoint cycle
    pub const MAX_V2_DIRTY_BLOCKS_PER_CYCLE: u64 = 10_000;

    /// V2 checkpoint metadata size (bytes)
    pub const V2_CHECKPOINT_METADATA_SIZE: usize = 2048;
}

/// Checkpoint performance constants
pub mod performance {
    /// Target checkpoint throughput (MB/s)
    pub const TARGET_CHECKPOINT_THROUGHPUT_MBPS: f64 = 100.0;

    /// Maximum checkpoint duration (milliseconds)
    pub const MAX_CHECKPOINT_DURATION_MS: u64 = 600_000; // 10 minutes

    /// Checkpoint I/O bandwidth utilization target (percentage)
    pub const CHECKPOINT_IO_UTILIZATION_TARGET: f64 = 0.7; // 70%

    /// Checkpoint memory utilization target (percentage)
    pub const CHECKPOINT_MEMORY_UTILIZATION_TARGET: f64 = 0.5; // 50%

    /// Checkpoint CPU utilization target (percentage)
    pub const CHECKPOINT_CPU_UTILIZATION_TARGET: f64 = 0.6; // 60%
}

/// Checkpoint strategy constants
pub mod strategies {
    /// Minimum time interval for time-based strategy (seconds)
    pub const MIN_TIME_INTERVAL_SECONDS: u64 = 10;

    /// Maximum time interval for time-based strategy (seconds)
    pub const MAX_TIME_INTERVAL_SECONDS: u64 = 3600; // 1 hour

    /// Minimum size threshold for size-based strategy (bytes)
    pub const MIN_SIZE_THRESHOLD: u64 = 1024 * 1024; // 1MB

    /// Maximum size threshold for size-based strategy (bytes)
    pub const MAX_SIZE_THRESHOLD: u64 = 1024 * 1024 * 1024; // 1GB

    /// Minimum transaction count for transaction-based strategy
    pub const MIN_TRANSACTION_THRESHOLD: u64 = 1;

    /// Maximum transaction count for transaction-based strategy
    pub const MAX_TRANSACTION_THRESHOLD: u64 = 1_000_000;

    /// Adaptive strategy minimum interval (seconds)
    pub const ADAPTIVE_MIN_INTERVAL_SECONDS: u64 = 30;

    /// Adaptive strategy maximum WAL size multiplier
    pub const ADAPTIVE_MAX_WAL_SIZE_MULTIPLIER: f64 = 4.0;

    /// Adaptive strategy maximum transaction multiplier
    pub const ADAPTIVE_MAX_TX_MULTIPLIER: f64 = 2.0;
}

/// Checkpoint validation constants
pub mod validation {
    /// Maximum allowed checkpoint size variance (percentage)
    pub const MAX_SIZE_VARIANCE_PERCENT: f64 = 0.2; // 20%

    /// Maximum allowed duration variance (percentage)
    pub const MAX_DURATION_VARIANCE_PERCENT: f64 = 0.3; // 30%

    /// Maximum checkpoint integrity errors allowed
    pub const MAX_INTEGRITY_ERRORS: usize = 10;

    /// Checkpoint consistency check timeout (milliseconds)
    pub const CONSISTENCY_CHECK_TIMEOUT_MS: u64 = 30_000; // 30 seconds

    /// Maximum rollback validation attempts
    pub const MAX_ROLLBACK_VALIDATION_ATTEMPTS: u32 = 3;

    /// Checkpoint validation sample rate (percentage)
    pub const VALIDATION_SAMPLE_RATE: f64 = 0.1; // 10%
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_magic_constants() {
        assert_eq!(CHECKPOINT_MAGIC, b"CKPT");
        assert_eq!(CHECKPOINT_VERSION, 1);
        assert_eq!(PROGRESS_MAGIC, b"PROG");
        assert_eq!(COMPLETION_MAGIC, b"DONE");
    }

    #[test]
    fn test_size_constants() {
        assert_eq!(MIN_CHECKPOINT_SIZE, 1024);
        assert_eq!(MAX_CHECKPOINT_SIZE, 1024 * 1024 * 1024);
        assert!(DEFAULT_BATCH_SIZE > 0);
        assert!(PROGRESS_REPORT_INTERVAL > DEFAULT_BATCH_SIZE);
    }

    #[test]
    fn test_v2_constants() {
        assert!(v2::MAX_CLUSTER_OPERATIONS_PER_BATCH > 0);
        assert!(v2::MAX_EDGE_CLUSTER_OPERATIONS_PER_BATCH > v2::MAX_CLUSTER_OPERATIONS_PER_BATCH);
        assert_eq!(v2::V2_GRAPH_BLOCK_SIZE, 4096);
        assert_eq!(v2::V2_CLUSTER_ALIGNMENT, 64 * 1024);
    }

    #[test]
    fn test_performance_constants() {
        assert!(performance::TARGET_CHECKPOINT_THROUGHPUT_MBPS > 0.0);
        assert!(performance::TARGET_CHECKPOINT_THROUGHPUT_MBPS < 1000.0); // Reasonable upper bound
        assert!(performance::CHECKPOINT_IO_UTILIZATION_TARGET > 0.0);
        assert!(performance::CHECKPOINT_IO_UTILIZATION_TARGET <= 1.0);
    }

    #[test]
    fn test_strategy_constants() {
        assert!(strategies::MIN_TIME_INTERVAL_SECONDS < strategies::MAX_TIME_INTERVAL_SECONDS);
        assert!(strategies::MIN_SIZE_THRESHOLD < strategies::MAX_SIZE_THRESHOLD);
        assert!(strategies::MIN_TRANSACTION_THRESHOLD < strategies::MAX_TRANSACTION_THRESHOLD);
    }

    #[test]
    fn test_validation_constants() {
        assert!(validation::MAX_SIZE_VARIANCE_PERCENT > 0.0);
        assert!(validation::MAX_SIZE_VARIANCE_PERCENT < 1.0);
        assert!(validation::MAX_DURATION_VARIANCE_PERCENT > 0.0);
        assert!(validation::MAX_DURATION_VARIANCE_PERCENT < 1.0);
    }

    #[test]
    fn test_header_size_calculation() {
        let expected_header_size = 4 + 8 + 8 + 4 + 4; // magic + start_lsn + end_lsn + timestamp + block_count
        assert_eq!(CHECKPOINT_HEADER_SIZE, expected_header_size);

        let expected_progress_size = 4 + 8 + 4; // magic + processed_records + flushed_blocks
        assert_eq!(PROGRESS_RECORD_SIZE, expected_progress_size);

        let expected_completion_size = 4 + 8 + 4; // magic + total_records + processed_records
        assert_eq!(COMPLETION_RECORD_SIZE, expected_completion_size);
    }

    #[test]
    fn test_constants_reasonableness() {
        // Ensure constants are within reasonable bounds
        assert!(
            MAX_CONCURRENT_CHECKPOINTS == 1,
            "Only one checkpoint should run at a time for consistency"
        );
        assert!(METRICS_SMOOTHING_ALPHA > 0.0 && METRICS_SMOOTHING_ALPHA < 1.0);
        assert!(MAX_CHECKPOINT_RETRIES > 0 && MAX_CHECKPOINT_RETRIES < 10);
        assert!(CHECKPOINT_RETRY_BACKOFF > 1.0 && CHECKPOINT_RETRY_BACKOFF < 10.0);
    }
}
