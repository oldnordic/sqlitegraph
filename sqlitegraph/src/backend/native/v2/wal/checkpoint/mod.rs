//! V2 WAL Checkpoint Module
//!
//! This module orchestrates checkpoint functionality for the V2 clustered edge format,
//! providing modular separation of concerns for checkpoint management, strategy implementation,
//! execution operations, and validation. Each submodule focuses on specific aspects of the
//! checkpointing system while maintaining V2 graph file integration.
//!
//! ## Module Organization
//!
//! - **core**: Main checkpoint manager, state management, and orchestration
//! - **strategies**: Checkpoint strategy implementations (size, time, transaction, adaptive)
//! - **operations**: Checkpoint execution, V2 integration, and file I/O operations
//! - **validation**: Metrics collection, validation, and cleanup operations
//!
//! ## Architecture
//!
//! The checkpoint system follows a layered architecture with clear separation of concerns:
//! 1. Core management layer handles lifecycle and orchestration
//! 2. Strategy layer determines when and how to checkpoint
//! 3. Operations layer performs the actual checkpointing work
//! 4. Validation layer ensures consistency and provides metrics
//!
//! ## V2 Integration
//!
//! All checkpoint operations are specifically designed for V2-native clustered edge format:
//! - NodeRecordV2 integration for node operations
//! - EdgeCluster integration for edge operations
//! - String table and free space management integration
//! - Cluster-aware dirty block tracking

pub use self::core::{
    V2WALCheckpointManager, CheckpointState, DirtyBlockTracker, CheckpointProgress,
};
pub use self::strategies::{
    CheckpointStrategy, CheckpointTrigger, StrategyValidator, StrategyEvaluator, StrategyMetrics,
};
pub use self::operations::{
    CheckpointExecutor, BlockFlusher, V2GraphIntegrator,
};
pub use self::validation::{
    CheckpointValidator, CheckpointMetrics as V2CheckpointMetrics, CheckpointCleanup,
    // Extended validation types from the new modular structure
    ValidationRule, ValidationRuleEngine, ValidationSeverity, ValidationConfig,
    ConsistencyResult, ConsistencyViolation, ConsistencySeverity,
    V2InvariantResult, V2InvariantViolation,
    CheckpointValidationReport, ValidationStatus, PerformanceMetrics,
    CheckpointValidatorFactory, ValidationComponents,
};

/// Checkpoint module re-exports for backward compatibility
pub mod core;
pub mod strategies;
pub mod operations;
pub mod validation;

/// Checkpoint module constants and utilities
pub mod constants;

/// Checkpoint module errors and diagnostics
pub mod errors;

use crate::backend::native::{NativeBackendError, NativeResult};
use crate::backend::native::v2::wal::V2WALConfig;
use std::path::Path;

// Re-export error types from the errors module
pub use self::errors::{CheckpointError, CheckpointResult, ErrorSeverity, RecoverySuggestion, CheckpointErrorKind};


/// Checkpoint module factory for creating checkpoint components
pub struct CheckpointFactory;

impl CheckpointFactory {
    /// Create a checkpoint manager with default configuration
    pub fn create_manager(config: V2WALConfig, strategy: CheckpointStrategy) -> CheckpointResult<V2WALCheckpointManager> {
        V2WALCheckpointManager::create(config, strategy).map_err(Into::into)
    }

    /// Create a checkpoint manager with adaptive strategy
    pub fn create_adaptive_manager(
        config: V2WALConfig,
        min_interval: std::time::Duration,
        max_wal_size: u64,
        max_transactions: u64,
    ) -> CheckpointResult<V2WALCheckpointManager> {
        let strategy = CheckpointStrategy::Adaptive {
            min_interval,
            max_wal_size,
            max_transactions,
        };
        Self::create_manager(config, strategy)
    }

    /// Create a checkpoint manager optimized for V2 graph workloads
    pub fn create_v2_optimized_manager(config: V2WALConfig) -> CheckpointResult<V2WALCheckpointManager> {
        // V2 graph workloads benefit from size-based checkpointing
        // due to clustered edge I/O patterns
        let strategy = CheckpointStrategy::SizeThreshold(config.max_wal_size / 4);
        Self::create_manager(config, strategy)
    }

    /// Validate checkpoint configuration
    pub fn validate_config(config: &V2WALConfig) -> CheckpointResult<()> {
        config.validate().map_err(Into::into)
    }

    /// Create checkpoint directory if it doesn't exist
    pub fn ensure_checkpoint_directory(config: &V2WALConfig) -> CheckpointResult<()> {
        if let Some(parent) = config.checkpoint_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CheckpointError::io(format!("Failed to create checkpoint directory: {}", e))
            })?;
        }
        Ok(())
    }
}

/// Checkpoint module utilities for common operations
pub mod utils {
    use super::*;

    /// Calculate optimal checkpoint size for V2 graph workloads
    pub fn calculate_optimal_checkpoint_size(wal_size: u64) -> u64 {
        // For V2 clustered edge format, optimal checkpoint size
        // is typically 1/4 to 1/3 of WAL size to balance I/O patterns
        std::cmp::max(wal_size / 4, 1024 * 1024) // Minimum 1MB
    }

    /// Estimate checkpoint duration based on WAL size and strategy
    pub fn estimate_checkpoint_duration(wal_size: u64, strategy: &CheckpointStrategy) -> std::time::Duration {
        let base_duration = std::time::Duration::from_millis(
            ((wal_size / (1024 * 1024)) as u64) * 100 // 100ms per MB baseline
        );

        match strategy {
            CheckpointStrategy::SizeThreshold(_) => base_duration,
            CheckpointStrategy::TransactionCount(_) => base_duration * 2, // Transaction counting adds overhead
            CheckpointStrategy::TimeInterval(_) => base_duration / 2, // Time-based checkpoints are usually smaller
            CheckpointStrategy::Adaptive { .. } => base_duration * 3, // Adaptive strategy has more logic
        }
    }

    /// Validate checkpoint file integrity
    pub fn validate_checkpoint_file(path: &Path) -> CheckpointResult<bool> {
        if !path.exists() {
            return Ok(false);
        }

        let metadata = std::fs::metadata(path).map_err(|e| {
            CheckpointError::io(format!("Failed to read checkpoint metadata: {}", e))
        })?;

        // Basic validation: file should not be empty
        if metadata.len() == 0 {
            return Err(CheckpointError::validation("Checkpoint file is empty".to_string()));
        }

        // File should have a reasonable size (not too small, not too large)
        let min_size = 1024; // 1KB minimum for header
        let max_size = 1024 * 1024 * 1024; // 1GB maximum for single checkpoint

        if metadata.len() < min_size {
            return Err(CheckpointError::validation("Checkpoint file too small".to_string()));
        }

        if metadata.len() > max_size {
            return Err(CheckpointError::validation("Checkpoint file too large".to_string()));
        }

        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::time::Duration;
    use crate::backend::native::GraphFile;

    #[test]
    fn test_checkpoint_factory_create_manager() -> CheckpointResult<()> {
        let temp_dir = tempdir().unwrap();
        let v2_graph_path = temp_dir.path().join("test.v2");

        // Create a minimal V2 graph file for testing
        let _graph_file = GraphFile::create(&v2_graph_path)
            .map_err(|e| CheckpointError::v2_integration(format!("Failed to create test graph file: {}", e)))?;

        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };
        let strategy = CheckpointStrategy::TimeInterval(Duration::from_secs(60));

        let manager = CheckpointFactory::create_manager(config, strategy)?;
        assert!(true, "Checkpoint manager created successfully");
        Ok(())
    }

    #[test]
    fn test_checkpoint_factory_adaptive_manager() -> CheckpointResult<()> {
        let temp_dir = tempdir().unwrap();
        let v2_graph_path = temp_dir.path().join("test.v2");

        // Create a minimal V2 graph file for testing
        let _graph_file = GraphFile::create(&v2_graph_path)
            .map_err(|e| CheckpointError::v2_integration(format!("Failed to create test graph file: {}", e)))?;

        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let manager = CheckpointFactory::create_adaptive_manager(
            config,
            Duration::from_secs(30),
            100 * 1024 * 1024, // 100MB
            1000,
        )?;
        assert!(true, "Adaptive checkpoint manager created successfully");
        Ok(())
    }

    #[test]
    fn test_checkpoint_utils_calculate_optimal_size() {
        let test_sizes = vec![
            (16 * 1024 * 1024, 4 * 1024 * 1024),   // 16MB WAL -> 4MB checkpoint
            (64 * 1024 * 1024, 16 * 1024 * 1024),  // 64MB WAL -> 16MB checkpoint
            (256 * 1024 * 1024, 64 * 1024 * 1024), // 256MB WAL -> 64MB checkpoint
            (512, 1024 * 1024),                   // Small WAL -> 1MB minimum
        ];

        for (wal_size, expected) in test_sizes {
            let result = utils::calculate_optimal_checkpoint_size(wal_size);
            assert_eq!(result, expected, "Incorrect optimal checkpoint size for WAL size {}", wal_size);
        }
    }

    #[test]
    fn test_checkpoint_error_display() {
        let error = CheckpointError::configuration("Invalid configuration");
        let display = format!("{}", error);
        assert!(display.contains("Configuration"));
        assert!(display.contains("Invalid configuration"));
    }

    #[test]
    fn test_checkpoint_error_from_native() {
        let native_error = NativeBackendError::Io(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
        let checkpoint_error: CheckpointError = native_error.into();
        assert_eq!(checkpoint_error.kind, CheckpointErrorKind::Io);
        assert!(checkpoint_error.message.contains("I/O error"));
    }
}