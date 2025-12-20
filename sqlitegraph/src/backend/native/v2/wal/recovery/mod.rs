//! V2 WAL Recovery Module
//!
//! This module orchestrates comprehensive crash recovery functionality for the V2 clustered edge
//! format, providing modular separation of concerns for recovery management, WAL scanning,
//! transaction validation, and replay operations. Each submodule focuses on specific aspects
//! of the recovery system while maintaining V2 graph file integration.
//!
//! ## Module Organization
//!
//! - **core**: Main recovery engine, state management, and orchestration
//! - **scanner**: WAL file scanning and transaction identification
//! - **validator**: Transaction consistency validation and integrity checking
//! - **replayer**: Transaction replay and rollback execution
//!
//! ## Architecture
//!
//! The recovery system follows a layered architecture with clear separation of concerns:
//! 1. Core management layer handles recovery lifecycle and orchestration
//! 2. Scanner layer performs WAL analysis and transaction detection
//! 3. Validator layer ensures transaction consistency and data integrity
//! 4. Replayer layer executes transaction replay and rollback operations
//!
//! ## V2 Integration
//!
//! All recovery operations are specifically designed for V2-native clustered edge format:
//! - NodeRecordV2 integration for node recovery operations
//! - EdgeCluster integration for edge recovery operations
//! - String table and free space management recovery integration
//! - Cluster-aware transaction state tracking

// Re-export core recovery components
pub use self::core::{
    V2WALRecoveryEngine, RecoveryState, RecoveryResult,
    RecoveryProgress, RecoveryOptions,
};

// Re-export recovery strategy components
pub use self::scanner::{
    WALScanner, TransactionScanner, WALScanResult,
};

// Re-export validation components
pub use self::validator::{
    TransactionValidator, RecoveryValidator, ValidationResult,
};

// Re-export replay components
pub use self::replayer::{
    V2GraphFileReplayer, ReplayConfig, ReplayResult, ReplayStatistics,
    RollbackOperation,
};

// Module declarations
pub mod core;
pub mod scanner;
pub mod validator;
pub mod replayer;

// Constants module for recovery-specific constants
pub mod constants;

// Errors module for comprehensive error handling
pub mod errors;

// Import dependencies
use crate::backend::native::v2::wal::V2WALConfig;
use std::path::{Path, PathBuf};

// Re-export error types for convenience
pub use self::errors::RecoveryError;

/// Recovery module factory for creating recovery components
pub struct RecoveryFactory;

impl RecoveryFactory {
    /// Create a recovery engine with default options
    pub fn create_engine(
        config: V2WALConfig,
        database_path: PathBuf,
    ) -> RecoveryResult<V2WALRecoveryEngine> {
        let options = RecoveryOptions::default();
        V2WALRecoveryEngine::create(config, database_path, options)
            .map_err(RecoveryError::from)
    }

    /// Create a recovery engine with custom options
    pub fn create_engine_with_options(
        config: V2WALConfig,
        database_path: PathBuf,
        options: RecoveryOptions,
    ) -> RecoveryResult<V2WALRecoveryEngine> {
        V2WALRecoveryEngine::create(config, database_path, options)
            .map_err(RecoveryError::from)
    }

    /// Create a recovery engine optimized for V2 workloads
    pub fn create_v2_optimized_engine(
        config: V2WALConfig,
        database_path: PathBuf,
    ) -> RecoveryResult<V2WALRecoveryEngine> {
        let options = RecoveryOptions {
            fast_recovery: false, // V2 workloads need thorough recovery
            max_batch_size: 500,     // Moderate batch size for V2 clustered edge data
            recovery_timeout: std::time::Duration::from_secs(600), // 10 minutes
            perform_consistency_checks: true,
            create_backup: true,
            max_recovery_attempts: 5,
            force_recovery: false,
        };
        V2WALRecoveryEngine::create(config, database_path, options)
            .map_err(RecoveryError::from)
    }

    /// Create a fast recovery engine for emergency scenarios
    pub fn create_fast_recovery_engine(
        config: V2WALConfig,
        database_path: PathBuf,
    ) -> RecoveryResult<V2WALRecoveryEngine> {
        let options = RecoveryOptions {
            fast_recovery: true,
            max_batch_size: 2000, // Larger batches for speed
            recovery_timeout: std::time::Duration::from_secs(120), // 2 minutes
            perform_consistency_checks: false, // Skip checks for speed
            create_backup: false, // Skip backup for speed
            max_recovery_attempts: 1,
            force_recovery: true,
        };
        V2WALRecoveryEngine::create(config, database_path, options)
            .map_err(RecoveryError::from)
    }

    /// Validate recovery prerequisites
    pub fn validate_prerequisites(
        config: &V2WALConfig,
        database_path: &Path,
    ) -> RecoveryResult<()> {
        // Validate database file exists
        if !database_path.exists() {
            return Err(RecoveryError::configuration(format!(
                "Database file does not exist: {}",
                database_path.display()
            )));
        }

        // Validate WAL file exists
        if !config.wal_path.exists() {
            return Err(RecoveryError::configuration(format!(
                "WAL file does not exist: {}",
                config.wal_path.display()
            )));
        }

        // Validate database file is readable
        if !database_path.is_file() {
            return Err(RecoveryError::configuration(format!(
                "Database path is not a file: {}",
                database_path.display()
            )));
        }

        // Validate WAL file is readable
        if !config.wal_path.is_file() {
            return Err(RecoveryError::configuration(format!(
                "WAL path is not a file: {}",
                config.wal_path.display()
            )));
        }

        Ok(())
    }

    /// Create backup path for recovery safety
    pub fn create_backup_path(database_path: &Path, timestamp: u64) -> PathBuf {
        let database_name = database_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("database");

        let backup_name = format!("{}.recovery_backup.{}", database_name, timestamp);
        database_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("recovery_backups")
            .join(backup_name)
    }
}

/// Recovery module utilities for common operations
pub mod utils {
    use super::*;

    /// Estimate recovery duration based on database size and options
    pub fn estimate_recovery_duration(
        database_size_bytes: u64,
        wal_size_bytes: u64,
        options: &RecoveryOptions,
    ) -> std::time::Duration {
        let base_duration = std::time::Duration::from_millis(
            ((database_size_bytes + wal_size_bytes) / (1024 * 1024)) as u64 * 50 // 50ms per MB
        );

        let mut duration = base_duration;

        // Adjust based on recovery options
        if options.fast_recovery {
            duration = duration / 2; // Fast recovery is twice as fast
        }

        if options.perform_consistency_checks {
            duration = duration * 2; // Consistency checks double the time
        }

        if options.create_backup {
            duration += std::time::Duration::from_secs(10); // 10 seconds for backup
        }

        duration
    }

    /// Calculate optimal batch size based on system resources
    pub fn calculate_optimal_batch_size(database_size_bytes: u64) -> usize {
        // Larger databases benefit from larger batches
        let size_mb = database_size_bytes / (1024 * 1024);

        match size_mb {
            0..=100 => 100,      // Small databases: small batches
            101..=500 => 500,    // Medium databases: medium batches
            501..=1000 => 1000,  // Large databases: large batches
            _ => 2000,           // Very large databases: very large batches
        }
    }

    /// Validate recovery options for consistency
    pub fn validate_recovery_options(options: &RecoveryOptions) -> RecoveryResult<()> {
        // Validate batch size is reasonable
        if options.max_batch_size == 0 {
            return Err(RecoveryError::configuration(
                "Max batch size cannot be zero".to_string()
            ));
        }

        if options.max_batch_size > 10000 {
            return Err(RecoveryError::configuration(
                "Max batch size too large (>10000)".to_string()
            ));
        }

        // Validate timeout is reasonable
        if options.recovery_timeout.as_secs() == 0 {
            return Err(RecoveryError::configuration(
                "Recovery timeout cannot be zero".to_string()
            ));
        }

        if options.recovery_timeout.as_secs() > 3600 {
            return Err(RecoveryError::configuration(
                "Recovery timeout too large (>1 hour)".to_string()
            ));
        }

        // Validate recovery attempts
        if options.max_recovery_attempts == 0 {
            return Err(RecoveryError::configuration(
                "Max recovery attempts cannot be zero".to_string()
            ));
        }

        if options.max_recovery_attempts > 10 {
            return Err(RecoveryError::configuration(
                "Max recovery attempts too many (>10)".to_string()
            ));
        }

        Ok(())
    }

    /// Get recovery severity level based on error conditions
    pub fn get_recovery_severity(
        database_corrupted: bool,
        wal_corrupted: bool,
        transaction_count: u64,
    ) -> RecoverySeverity {
        if database_corrupted {
            RecoverySeverity::Critical
        } else if wal_corrupted {
            RecoverySeverity::High
        } else if transaction_count > 1000 {
            RecoverySeverity::Medium
        } else if transaction_count > 100 {
            RecoverySeverity::Low
        } else {
            RecoverySeverity::Minimal
        }
    }
}

/// Recovery severity levels for classification
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RecoverySeverity {
    /// Minimal recovery needed
    Minimal,

    /// Low severity recovery
    Low,

    /// Medium severity recovery
    Medium,

    /// High severity recovery
    High,

    /// Critical recovery required
    Critical,
}

impl std::fmt::Display for RecoverySeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecoverySeverity::Minimal => write!(f, "Minimal"),
            RecoverySeverity::Low => write!(f, "Low"),
            RecoverySeverity::Medium => write!(f, "Medium"),
            RecoverySeverity::High => write!(f, "High"),
            RecoverySeverity::Critical => write!(f, "Critical"),
        }
    }
}

/// Recovery statistics for monitoring and analysis
#[derive(Debug, Clone, Default)]
pub struct RecoveryStatistics {
    /// Total recovery attempts
    pub total_attempts: u64,

    /// Successful recoveries
    pub successful_recoveries: u64,

    /// Failed recoveries
    pub failed_recoveries: u64,

    /// Average recovery duration (milliseconds)
    pub avg_duration_ms: u64,

    /// Total data recovered (bytes)
    pub total_data_recovered: u64,

    /// Total transactions recovered
    pub total_transactions_recovered: u64,

    /// Most recent recovery timestamp
    pub last_recovery_timestamp: Option<std::time::SystemTime>,
}

impl RecoveryStatistics {
    /// Calculate success rate
    pub fn success_rate(&self) -> f64 {
        if self.total_attempts == 0 {
            0.0
        } else {
            self.successful_recoveries as f64 / self.total_attempts as f64 * 100.0
        }
    }

    /// Get recovery status description
    pub fn status_description(&self) -> String {
        if self.total_attempts == 0 {
            "No recovery attempts recorded".to_string()
        } else {
            format!(
                "Recovery success rate: {:.1}% ({} of {} attempts)",
                self.success_rate(),
                self.successful_recoveries,
                self.total_attempts
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::time::Duration;

    #[test]
    fn test_recovery_factory_create_engine() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };
        let database_path = temp_dir.path().join("test.db");

        // Create empty files for validation
        std::fs::File::create(&config.wal_path).unwrap();
        std::fs::File::create(&database_path).unwrap();

        let result = RecoveryFactory::create_engine(config, database_path.clone());
        assert!(result.is_ok(), "Recovery engine creation should succeed");
    }

    #[test]
    fn test_recovery_factory_validate_prerequisites() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };
        let database_path = temp_dir.path().join("test.db");

        // Test with missing files
        let result = RecoveryFactory::validate_prerequisites(&config, &database_path);
        assert!(result.is_err(), "Should fail with missing files");

        // Create files and test again
        std::fs::File::create(&config.wal_path).unwrap();
        std::fs::File::create(&database_path).unwrap();

        let result = RecoveryFactory::validate_prerequisites(&config, &database_path);
        assert!(result.is_ok(), "Should succeed with existing files");
    }

    #[test]
    fn test_recovery_estimation() {
        let database_size = 100 * 1024 * 1024; // 100MB
        let wal_size = 50 * 1024 * 1024; // 50MB

        let options = RecoveryOptions::default();
        let duration = utils::estimate_recovery_duration(database_size, wal_size, &options);

        // Should be reasonable duration (150MB * 50ms = 7.5s + checks)
        assert!(duration.as_secs() >= 5, "Duration should be at least 5 seconds");
        assert!(duration.as_secs() <= 30, "Duration should be at most 30 seconds");
    }

    #[test]
    fn test_optimal_batch_size() {
        assert_eq!(utils::calculate_optimal_batch_size(50 * 1024 * 1024), 100);  // 50MB
        assert_eq!(utils::calculate_optimal_batch_size(250 * 1024 * 1024), 500); // 250MB
        assert_eq!(utils::calculate_optimal_batch_size(750 * 1024 * 1024), 1000); // 750MB
        assert_eq!(utils::calculate_optimal_batch_size(2 * 1024 * 1024 * 1024), 2000); // 2GB
    }

    #[test]
    fn test_recovery_severity() {
        // Minimal case
        let severity = utils::get_recovery_severity(false, false, 10);
        assert_eq!(severity, RecoverySeverity::Minimal);

        // High transaction count
        let severity = utils::get_recovery_severity(false, false, 1500);
        assert_eq!(severity, RecoverySeverity::Medium);

        // Database corruption
        let severity = utils::get_recovery_severity(true, false, 10);
        assert_eq!(severity, RecoverySeverity::Critical);

        // WAL corruption
        let severity = utils::get_recovery_severity(false, true, 10);
        assert_eq!(severity, RecoverySeverity::High);
    }

    #[test]
    fn test_recovery_statistics() {
        let mut stats = RecoveryStatistics::default();

        // Initial state
        assert_eq!(stats.success_rate(), 0.0);
        assert!(stats.status_description().contains("No recovery attempts"));

        // Add some attempts
        stats.total_attempts = 5;
        stats.successful_recoveries = 4;
        stats.failed_recoveries = 1;

        assert_eq!(stats.success_rate(), 80.0);
        assert!(stats.status_description().contains("80.0%"));
    }
}