//! V2 WAL Recovery Constants
//!
//! This module defines all constants used throughout the recovery system,
//! including performance targets, validation thresholds, and configuration limits.

/// Recovery file magic number for integrity validation
pub const RECOVERY_MAGIC: &[u8; 4] = b"RECV";

/// Recovery file version
pub const RECOVERY_VERSION: u32 = 1;

/// Progress record marker for recovery operations
pub const RECOVERY_PROGRESS_MAGIC: &[u8; 4] = b"RPRO";

/// Completion record marker for recovery operations
pub const RECOVERY_COMPLETION_MAGIC: &[u8; 4] = b"RCMP";

/// Minimum recovery file size (bytes)
pub const MIN_RECOVERY_SIZE: u64 = 1024;

/// Maximum recovery file size (bytes) - 500MB
pub const MAX_RECOVERY_SIZE: u64 = 500 * 1024 * 1024;

/// Default batch size for transaction replay
pub const DEFAULT_BATCH_SIZE: usize = 1000;

/// Progress reporting interval during recovery (transactions)
pub const RECOVERY_PROGRESS_INTERVAL: usize = 100;

/// Maximum allowed incomplete transactions in WAL
pub const MAX_INCOMPLETE_TRANSACTIONS: usize = 100;

/// Maximum WAL file size for automatic recovery (bytes)
pub const MAX_WAL_SIZE_FOR_AUTO_RECOVERY: u64 = 1024 * 1024 * 1024; // 1GB

/// Maximum number of recovery attempts before giving up
pub const MAX_RECOVERY_ATTEMPTS: u32 = 5;

/// Recovery timeout in seconds
pub const DEFAULT_RECOVERY_TIMEOUT_SECONDS: u64 = 300; // 5 minutes

/// Fast recovery timeout in seconds
pub const FAST_RECOVERY_TIMEOUT_SECONDS: u64 = 60; // 1 minute

/// Emergency recovery timeout in seconds
pub const EMERGENCY_RECOVERY_TIMEOUT_SECONDS: u64 = 30; // 30 seconds

/// Recovery retry backoff multiplier
pub const RECOVERY_RETRY_BACKOFF_MULTIPLIER: f64 = 2.0;

/// Maximum delay between recovery attempts (seconds)
pub const MAX_RETRY_DELAY_SECONDS: u64 = 60;

/// Minimum delay between recovery attempts (seconds)
pub const MIN_RETRY_DELAY_SECONDS: u64 = 5;

/// Transaction validation constants
pub mod transaction {
    /// Maximum transaction size in bytes
    pub const MAX_TRANSACTION_SIZE: u64 = 10 * 1024 * 1024; // 10MB

    /// Maximum records per transaction
    pub const MAX_RECORDS_PER_TRANSACTION: usize = 10000;

    /// Maximum transaction age in seconds
    pub const MAX_TRANSACTION_AGE_SECONDS: u64 = 3600; // 1 hour

    /// Transaction consistency check timeout (milliseconds)
    pub const CONSISTENCY_CHECK_TIMEOUT_MS: u64 = 5000; // 5 seconds

    /// Maximum orphaned records allowed
    pub const MAX_ORPHANED_RECORDS: usize = 100;

    /// Duplicate transaction detection window size
    pub const DUPLICATE_DETECTION_WINDOW: u64 = 1000; // LSN range
}

/// WAL scanning constants
pub mod scanning {
    /// WAL scan chunk size for memory efficiency
    pub const WAL_SCAN_CHUNK_SIZE: usize = 64 * 1024; // 64KB

    /// Maximum WAL read buffer size
    pub const MAX_READ_BUFFER_SIZE: usize = 1024 * 1024; // 1MB

    /// WAL scanner queue size
    pub const WAL_SCANNER_QUEUE_SIZE: usize = 1000;

    /// Progress reporting during WAL scan (percentage)
    pub const WAL_SCAN_PROGRESS_INTERVAL: u8 = 10;

    /// Maximum WAL file size to scan in memory (bytes)
    pub const MAX_IN_MEMORY_SCAN_SIZE: u64 = 100 * 1024 * 1024; // 100MB

    /// WAL scanner timeout (seconds)
    pub const WAL_SCANNER_TIMEOUT_SECONDS: u64 = 120; // 2 minutes
}

/// V2-specific recovery constants
pub mod v2 {
    /// Maximum V2 cluster operations per recovery batch
    pub const MAX_CLUSTER_OPERATIONS_PER_RECOVERY: usize = 250;

    /// Maximum V2 edge cluster operations per batch
    pub const MAX_EDGE_CLUSTER_OPERATIONS_PER_RECOVERY: usize = 500;

    /// Maximum V2 node record operations per batch
    pub const MAX_NODE_RECORD_OPERATIONS_PER_RECOVERY: usize = 375;

    /// Maximum V2 string table operations per batch
    pub const MAX_STRING_TABLE_OPERATIONS_PER_RECOVERY: usize = 125;

    /// Maximum V2 free space operations per batch
    pub const MAX_FREE_SPACE_OPERATIONS_PER_RECOVERY: usize = 50;

    /// V2 recovery metadata size (bytes)
    pub const V2_RECOVERY_METADATA_SIZE: usize = 1024;

    /// Cluster-aware recovery priority boost factor
    pub const CLUSTER_AWARE_PRIORITY_BOOST: f64 = 1.5;

    /// V2 graph file block size for recovery
    pub const V2_GRAPH_RECOVERY_BLOCK_SIZE: u64 = 4096; // 4KB

    /// V2 cluster alignment for recovery operations
    pub const V2_RECOVERY_CLUSTER_ALIGNMENT: u64 = 64 * 1024; // 64KB
}

/// Recovery performance constants
pub mod performance {
    /// Target recovery throughput (records per second)
    pub const TARGET_RECOVERY_THROUGHPUT_RPS: u64 = 10000;

    /// Maximum recovery duration (seconds)
    pub const MAX_RECOVERY_DURATION_SECONDS: u64 = 600; // 10 minutes

    /// Recovery I/O bandwidth utilization target (percentage)
    pub const RECOVERY_IO_UTILIZATION_TARGET: f64 = 0.8; // 80%

    /// Recovery memory utilization target (percentage)
    pub const RECOVERY_MEMORY_UTILIZATION_TARGET: f64 = 0.7; // 70%

    /// Recovery CPU utilization target (percentage)
    pub const RECOVERY_CPU_UTILIZATION_TARGET: f64 = 0.6; // 60%

    /// Performance baseline for comparison
    pub const BASELINE_RECOVERY_MB_PER_SECOND: f64 = 50.0;
}

/// Recovery strategy constants
pub mod strategies {
    /// Conservative strategy parameters
    pub const CONSERVATIVE_BATCH_SIZE: usize = 250;
    pub const CONSERVATIVE_TIMEOUT_SECONDS: u64 = 600;
    pub const CONSERVATIVE_MAX_ATTEMPTS: u32 = 3;

    /// Balanced strategy parameters
    pub const BALANCED_BATCH_SIZE: usize = 1000;
    pub const BALANCED_TIMEOUT_SECONDS: u64 = 300;
    pub const BALANCED_MAX_ATTEMPTS: u32 = 5;

    /// Aggressive strategy parameters
    pub const AGGRESSIVE_BATCH_SIZE: usize = 2000;
    pub const AGGRESSIVE_TIMEOUT_SECONDS: u64 = 120;
    pub const AGGRESSIVE_MAX_ATTEMPTS: u32 = 7;

    /// Emergency strategy parameters
    pub const EMERGENCY_BATCH_SIZE: usize = 5000;
    pub const EMERGENCY_TIMEOUT_SECONDS: u64 = 60;
    pub const EMERGENCY_MAX_ATTEMPTS: u32 = 1;
}

/// Recovery validation constants
pub mod validation {
    /// Maximum allowed database size variance (percentage)
    pub const MAX_SIZE_VARIANCE_PERCENT: f64 = 0.1; // 10%

    /// Maximum allowed transaction count variance (percentage)
    pub const MAX_TRANSACTION_VARIANCE_PERCENT: f64 = 0.05; // 5%

    /// Maximum recovery duration variance (percentage)
    pub const MAX_DURATION_VARIANCE_PERCENT: f64 = 0.3; // 30%

    /// Maximum integrity errors allowed
    pub const MAX_INTEGRITY_ERRORS: usize = 5;

    /// Consistency check timeout (seconds)
    pub const CONSISTENCY_CHECK_TIMEOUT_SECONDS: u64 = 30;

    /// Consistency check timeout (milliseconds)
    pub const CONSISTENCY_CHECK_TIMEOUT_MS: u64 = 5000; // 5 seconds

    /// Data validation sample rate (percentage)
    pub const DATA_VALIDATION_SAMPLE_RATE: f64 = 0.1; // 10%

    /// Checkpoint validation timeout (seconds)
    pub const CHECKPOINT_VALIDATION_TIMEOUT_SECONDS: u64 = 15;
}

/// Recovery file format constants
pub mod format {
    /// Recovery header size (bytes)
    pub const RECOVERY_HEADER_SIZE: usize = 32; // 4 + 4 + 8 + 8 + 8

    /// Progress record size (bytes)
    pub const PROGRESS_RECORD_SIZE: usize = 16; // 4 + 4 + 8

    /// Completion record size (bytes)
    pub const COMPLETION_RECORD_SIZE: usize = 20; // 4 + 4 + 8 + 4

    /// Metadata record size (bytes)
    pub const METADATA_RECORD_SIZE: usize = 64; // Variable size

    /// Maximum number of metadata records
    pub const MAX_METADATA_RECORDS: usize = 100;

    /// Recovery file alignment (bytes)
    pub const RECOVERY_FILE_ALIGNMENT: u64 = 4096; // 4KB alignment

    /// Checksum algorithm identifier
    pub const CHECKSUM_ALGORITHM: u8 = 1; // CRC32
}

/// Recovery state machine constants
pub mod state_machine {
    /// Maximum state transitions per second
    pub const MAX_STATE_TRANSITIONS_PER_SECOND: u32 = 100;

    /// State transition timeout (seconds)
    pub const STATE_TRANSITION_TIMEOUT_SECONDS: u64 = 60;

    /// Maximum state machine depth (nested recoveries)
    pub const MAX_RECOVERY_DEPTH: usize = 3;

    /// State persistence interval (transitions)
    pub const STATE_PERSISTENCE_INTERVAL: u32 = 10;

    /// State recovery validation window
    pub const STATE_RECOVERY_VALIDATION_WINDOW: u64 = 1000; // LSN range
}

/// Recovery monitoring constants
pub mod monitoring {
    /// Metrics collection interval (milliseconds)
    pub const METRICS_COLLECTION_INTERVAL_MS: u64 = 1000; // 1 second

    /// Performance reporting interval (milliseconds)
    pub const PERFORMANCE_REPORTING_INTERVAL_MS: u64 = 5000; // 5 seconds

    /// Health check interval (seconds)
    pub const HEALTH_CHECK_INTERVAL_SECONDS: u64 = 30;

    /// Anomaly detection window (seconds)
    pub const ANOMALY_DETECTION_WINDOW_SECONDS: u64 = 300; // 5 minutes

    /// Resource usage reporting interval (seconds)
    pub const RESOURCE_REPORTING_INTERVAL_SECONDS: u64 = 10;

    /// Maximum recovery logs to retain
    pub const MAX_RECOVERY_LOGS_RETAINED: usize = 1000;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recovery_magic_constants() {
        assert_eq!(RECOVERY_MAGIC, b"RECV");
        assert_eq!(RECOVERY_VERSION, 1);
        assert_eq!(RECOVERY_PROGRESS_MAGIC, b"RPRO");
        assert_eq!(RECOVERY_COMPLETION_MAGIC, b"RCMP");
    }

    #[test]
    fn test_size_constants() {
        assert_eq!(MIN_RECOVERY_SIZE, 1024);
        assert_eq!(MAX_RECOVERY_SIZE, 500 * 1024 * 1024);
        assert!(DEFAULT_BATCH_SIZE > 0);
        assert!(DEFAULT_BATCH_SIZE <= transaction::MAX_RECORDS_PER_TRANSACTION);
    }

    #[test]
    fn test_v2_constants() {
        assert!(v2::MAX_CLUSTER_OPERATIONS_PER_RECOVERY > 0);
        assert!(
            v2::MAX_EDGE_CLUSTER_OPERATIONS_PER_RECOVERY >= v2::MAX_CLUSTER_OPERATIONS_PER_RECOVERY
        );
        assert_eq!(v2::V2_GRAPH_RECOVERY_BLOCK_SIZE, 4096);
        assert_eq!(v2::V2_RECOVERY_CLUSTER_ALIGNMENT, 64 * 1024);
    }

    #[test]
    fn test_performance_constants() {
        assert!(performance::TARGET_RECOVERY_THROUGHPUT_RPS > 0);
        assert!(performance::TARGET_RECOVERY_THROUGHPUT_RPS < 100000); // Reasonable upper bound
        assert!(performance::RECOVERY_IO_UTILIZATION_TARGET > 0.0);
        assert!(performance::RECOVERY_IO_UTILIZATION_TARGET <= 1.0);
        assert!(performance::RECOVERY_MEMORY_UTILIZATION_TARGET > 0.0);
        assert!(performance::RECOVERY_MEMORY_UTILIZATION_TARGET <= 1.0);
        assert!(performance::RECOVERY_CPU_UTILIZATION_TARGET > 0.0);
        assert!(performance::RECOVERY_CPU_UTILIZATION_TARGET <= 1.0);
    }

    #[test]
    fn test_strategy_constants() {
        // Ensure strategies are ordered from conservative to aggressive
        assert!(strategies::CONSERVATIVE_BATCH_SIZE < strategies::BALANCED_BATCH_SIZE);
        assert!(strategies::BALANCED_BATCH_SIZE < strategies::AGGRESSIVE_BATCH_SIZE);
        assert!(strategies::AGGRESSIVE_BATCH_SIZE < strategies::EMERGENCY_BATCH_SIZE);

        // Ensure timeouts follow the same pattern
        assert!(strategies::CONSERVATIVE_TIMEOUT_SECONDS > strategies::BALANCED_TIMEOUT_SECONDS);
        assert!(strategies::BALANCED_TIMEOUT_SECONDS > strategies::AGGRESSIVE_TIMEOUT_SECONDS);
        assert!(strategies::AGGRESSIVE_TIMEOUT_SECONDS > strategies::EMERGENCY_TIMEOUT_SECONDS);
    }

    #[test]
    fn test_validation_constants() {
        assert!(validation::MAX_SIZE_VARIANCE_PERCENT > 0.0);
        assert!(validation::MAX_SIZE_VARIANCE_PERCENT < 1.0);
        assert!(validation::MAX_TRANSACTION_VARIANCE_PERCENT > 0.0);
        assert!(validation::MAX_TRANSACTION_VARIANCE_PERCENT < 1.0);
        assert!(validation::MAX_DURATION_VARIANCE_PERCENT > 0.0);
        assert!(validation::MAX_DURATION_VARIANCE_PERCENT < 1.0);
    }

    #[test]
    fn test_format_constants() {
        let expected_header_size = 4 + 4 + 8 + 8 + 8; // magic + version + timestamp + checksum + flags
        assert_eq!(format::RECOVERY_HEADER_SIZE, expected_header_size);

        let expected_progress_size = 4 + 4 + 8; // magic + version + timestamp
        assert_eq!(format::PROGRESS_RECORD_SIZE, expected_progress_size);

        let expected_completion_size = 4 + 4 + 8 + 4; // magic + version + timestamp + status
        assert_eq!(format::COMPLETION_RECORD_SIZE, expected_completion_size);
    }

    #[test]
    fn test_reasonableness() {
        // Ensure constants are within reasonable bounds
        assert!(MAX_RECOVERY_ATTEMPTS > 0 && MAX_RECOVERY_ATTEMPTS < 20);
        assert!(DEFAULT_RECOVERY_TIMEOUT_SECONDS > 0 && DEFAULT_RECOVERY_TIMEOUT_SECONDS < 3600);
        assert!(
            RECOVERY_RETRY_BACKOFF_MULTIPLIER > 1.0 && RECOVERY_RETRY_BACKOFF_MULTIPLIER < 10.0
        );
        assert!(MAX_RETRY_DELAY_SECONDS >= MIN_RETRY_DELAY_SECONDS);
    }
}
