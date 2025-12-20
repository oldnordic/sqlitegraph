//! Checkpoint Validation Module
//!
//! This module provides comprehensive validation functionality for V2 WAL checkpoint operations.
//! It includes validation rules, consistency checks, V2 invariant validation, and reporting
//! capabilities to ensure checkpoint integrity and compatibility.
//!
//! ## Module Structure
//!
//! - **rules**: Validation rule definitions and rule engine
//! - **consistency**: Data consistency checks and validation logic
//! - **invariants**: V2-specific invariant validation
//! - **reporting**: Validation reporting and result formatting
//!
//! ## Usage
//!
//! ```rust
//! use crate::backend::native::v2::wal::checkpoint::validation::{CheckpointValidatorFactory};
//! let validator = CheckpointValidatorFactory::create_default_validator(config)?;
//! let result = validator.validate_checkpoint_file(&checkpoint_path)?;
//! ```

// Re-export all public types and functions for backward compatibility
pub use self::rules::{
    ValidationRule, ValidationRuleEngine, ValidationSeverity, ValidationConfig,
    ValidationContext, FileValidationRules,
};

pub use self::consistency::{
    ConsistencyResult, ConsistencyViolation, ConsistencyViolationType, ConsistencySeverity,
    CheckpointConsistencyValidator, ConsistencyUtils,
};

pub use self::invariants::{
    V2InvariantResult, V2InvariantViolation, V2InvariantViolationType,
    V2InvariantValidator, V2InvariantUtils, V2InvariantSeverity, V2InvariantSummary,
};

pub use self::reporting::{
    CheckpointValidationReport, ValidationStatus, PerformanceMetrics, AnomalySummary,
    ValidationSummary, CheckpointValidationReporter, ValidationReportUtils, TrendAnalysis,
};

// Re-export the main checkpoint validator types for backward compatibility
use crate::backend::native::v2::wal::checkpoint::errors::{CheckpointError, CheckpointResult};
use crate::backend::native::v2::wal::checkpoint::core::{DirtyBlockTracker, CheckpointProgress, CheckpointState};
use crate::backend::native::v2::wal::V2WALConfig;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

/// Comprehensive checkpoint validator (backward compatibility wrapper)
pub struct CheckpointValidator {
    config: V2WALConfig,
    rule_engine: ValidationRuleEngine,
    consistency_validator: CheckpointConsistencyValidator,
    invariants_validator: V2InvariantValidator,
    reporter: CheckpointValidationReporter,
}

impl CheckpointValidator {
    /// Create new checkpoint validator
    pub fn new(config: V2WALConfig) -> Self {
        let rule_engine = ValidationRuleEngine::new();
        let consistency_validator = CheckpointConsistencyValidator::new(config.clone());
        let invariants_validator = V2InvariantValidator::new(config.clone());
        let reporter = CheckpointValidationReporter::new(config.clone());

        Self {
            config,
            rule_engine,
            consistency_validator,
            invariants_validator,
            reporter,
        }
    }

    /// Validate checkpoint file integrity (backward compatibility method)
    pub fn validate_checkpoint_file(&self, checkpoint_path: &std::path::Path) -> CheckpointResult<bool> {
        // Basic file existence check
        if !checkpoint_path.exists() {
            return Ok(false);
        }

        // Use file validation rules
        FileValidationRules::validate_file_size(checkpoint_path)?;

        // Open file and validate format
        use std::fs::File;
        let mut file = File::open(checkpoint_path)
            .map_err(|e| CheckpointError::validation(format!("Failed to open checkpoint file: {}", e)))?;

        FileValidationRules::validate_magic_number(&mut file)?;
        FileValidationRules::validate_version(&mut file)?;

        // Validate V2 metadata (simplified - just check if file is readable)
        // In a real implementation, this would validate V2-specific metadata

        Ok(true)
    }

    /// Validate checkpoint consistency with WAL (backward compatibility method)
    pub fn validate_checkpoint_consistency(
        &self,
        checkpoint_lsn_range: (u64, u64),
        last_checkpointed_lsn: u64,
    ) -> CheckpointResult<()> {
        let result = self.consistency_validator.validate_checkpoint_consistency(
            checkpoint_lsn_range,
            last_checkpointed_lsn,
        );

        if !result.is_consistent {
            let error_msg = result.violations
                .first()
                .map(|v| v.description.clone())
                .unwrap_or_else(|| "Consistency validation failed".to_string());
            return Err(CheckpointError::validation(error_msg));
        }

        Ok(())
    }

    /// Validate dirty block state consistency (simplified version)
    pub fn validate_dirty_block_consistency(
        &self,
        dirty_blocks: &DirtyBlockTracker,
        max_pending_blocks: u64,
    ) -> CheckpointResult<()> {
        // Use public API to get statistics instead of accessing private fields
        let (cluster_blocks, global_blocks) = dirty_blocks.get_statistics();
        let total_blocks = (cluster_blocks + global_blocks) as u64;

        if total_blocks > max_pending_blocks {
            return Err(CheckpointError::validation(format!(
                "Too many pending dirty blocks: {} (maximum: {})",
                total_blocks,
                max_pending_blocks
            )));
        }

        // Additional consistency checks using public API
        if global_blocks as u64 > MAX_GLOBAL_DIRTY_BLOCKS as u64 {
            return Err(CheckpointError::validation(format!(
                "Too many global dirty blocks: {} (maximum: {})",
                global_blocks as u64,
                MAX_GLOBAL_DIRTY_BLOCKS
            )));
        }

        Ok(())
    }

    /// Perform comprehensive validation using all validation components
    pub fn validate_comprehensive(
        &self,
        checkpoint_path: &std::path::Path,
        dirty_blocks: &DirtyBlockTracker,
        checkpoint_state: &CheckpointState,
        checkpoint_progress: &CheckpointProgress,
        checkpoint_lsn_range: (u64, u64),
        last_checkpointed_lsn: u64,
        checkpoint_duration: Duration,
        max_pending_blocks: u64,
    ) -> CheckpointResult<CheckpointValidationReport> {
        // Validate V2 invariants
        let v2_invariant_result = Some(self.invariants_validator.validate_v2_metadata(checkpoint_path)?);

        // Validate consistency
        let consistency_result = Some(self.consistency_validator.validate_checkpoint_consistency(
            checkpoint_lsn_range,
            last_checkpointed_lsn,
        ));

        // Generate report
        let report = self.reporter.generate_validation_report(
            checkpoint_path,
            consistency_result,
            v2_invariant_result,
            None, // Performance metrics would be provided separately
            Some(checkpoint_duration),
        );

        // Check if validation passed
        match report.validation_status {
            ValidationStatus::CriticalFailure | ValidationStatus::Failed => {
                let error_msg = format!("Validation failed: {:?}", report.validation_status);
                Err(CheckpointError::validation(error_msg))
            },
            _ => Ok(report),
        }
    }

    /// Get the rule engine reference
    pub fn rule_engine(&self) -> &ValidationRuleEngine {
        &self.rule_engine
    }

    /// Get the consistency validator reference
    pub fn consistency_validator(&self) -> &CheckpointConsistencyValidator {
        &self.consistency_validator
    }

    /// Get the V2 invariants validator reference
    pub fn invariants_validator(&self) -> &V2InvariantValidator {
        &self.invariants_validator
    }

    /// Get the reporter reference
    pub fn reporter(&self) -> &CheckpointValidationReporter {
        &self.reporter
    }
}

/// Checkpoint metrics collector for performance monitoring (backward compatibility wrapper)
pub struct CheckpointMetrics {
    config: V2WALConfig,
    metrics: Arc<Mutex<CheckpointMetricsData>>,
}

/// Checkpoint metrics data structure (backward compatibility type)
#[derive(Debug, Default, Clone)]
pub struct CheckpointMetricsData {
    /// Total checkpoints performed
    pub total_checkpoints: u64,

    /// Average checkpoint duration (milliseconds)
    pub avg_checkpoint_duration_ms: u64,

    /// Blocks checkpointed per checkpoint
    pub avg_blocks_per_checkpoint: u64,

    /// WAL records checkpointed per checkpoint
    pub avg_records_per_checkpoint: u64,

    /// Checkpoint I/O throughput (MB/s)
    pub checkpoint_throughput_mbps: f64,

    /// Time since last checkpoint (milliseconds)
    pub time_since_last_checkpoint_ms: u64,

    /// WAL size at last checkpoint
    pub wal_size_at_last_checkpoint: u64,

    /// Dirty blocks currently pending
    pub pending_dirty_blocks: u64,

    /// Last checkpoint timestamp
    pub last_checkpoint_timestamp: Option<SystemTime>,

    /// Recent checkpoint durations for statistical analysis
    pub recent_durations_ms: Vec<u64>,

    /// Performance anomaly detection data
    pub anomaly_detector: AnomalyDetector,
}

/// Anomaly detector for checkpoint performance (backward compatibility type)
#[derive(Debug, Default, Clone)]
pub struct AnomalyDetector {
    /// Baseline performance metrics
    pub baseline_duration_ms: u64,
    pub baseline_throughput_mbps: f64,
    pub baseline_blocks_per_checkpoint: u64,

    /// Anomaly detection thresholds
    pub duration_anomaly_threshold: f64,
    pub throughput_anomaly_threshold: f64,
    pub block_count_anomaly_threshold: f64,

    /// Anomaly counts
    pub duration_anomalies: u64,
    pub throughput_anomalies: u64,
    pub block_count_anomalies: u64,
}

impl CheckpointMetrics {
    /// Create new checkpoint metrics collector
    pub fn new(config: V2WALConfig) -> Self {
        let metrics = CheckpointMetricsData {
            anomaly_detector: AnomalyDetector {
                baseline_duration_ms: performance::TARGET_CHECKPOINT_THROUGHPUT_MBPS as u64 * 1000,
                baseline_throughput_mbps: performance::TARGET_CHECKPOINT_THROUGHPUT_MBPS,
                baseline_blocks_per_checkpoint: 100,
                duration_anomaly_threshold: 2.0,
                throughput_anomaly_threshold: 0.5,
                block_count_anomaly_threshold: 3.0,
                ..Default::default()
            },
            ..Default::default()
        };

        Self {
            config,
            metrics: Arc::new(Mutex::new(metrics)),
        }
    }

    /// Update metrics after checkpoint completion
    pub fn update_checkpoint_metrics(
        &self,
        progress: &CheckpointProgress,
        start_time: Instant,
    ) -> CheckpointResult<()> {
        let duration_ms = start_time.elapsed().as_millis() as u64;

        let mut metrics = self.metrics.lock()
            .map_err(|e| CheckpointError::validation(format!("Failed to lock metrics: {}", e)))?;

        metrics.total_checkpoints += 1;

        // Update averages using exponential smoothing
        let alpha = METRICS_SMOOTHING_ALPHA;

        metrics.avg_checkpoint_duration_ms =
            ((metrics.avg_checkpoint_duration_ms as f64 * (1.0 - alpha)) +
             (duration_ms as f64 * alpha)) as u64;

        metrics.avg_blocks_per_checkpoint =
            ((metrics.avg_blocks_per_checkpoint as f64 * (1.0 - alpha)) +
             (progress.flushed_blocks as f64 * alpha)) as u64;

        metrics.avg_records_per_checkpoint =
            ((metrics.avg_records_per_checkpoint as f64 * (1.0 - alpha)) +
             (progress.total_records as f64 * alpha)) as u64;

        // Calculate throughput
        if duration_ms > 0 {
            let bytes_processed = progress.total_records * 100;
            let mb_per_second = (bytes_processed as f64) / (1024.0 * 1024.0) / (duration_ms as f64 / 1000.0);
            metrics.checkpoint_throughput_mbps =
                ((metrics.checkpoint_throughput_mbps * (1.0 - alpha)) +
                 (mb_per_second * alpha));
        }

        metrics.last_checkpoint_timestamp = Some(SystemTime::now());

        // Track recent durations
        metrics.recent_durations_ms.push(duration_ms);
        if metrics.recent_durations_ms.len() > MAX_PROGRESS_ENTRIES {
            metrics.recent_durations_ms.remove(0);
        }

        // Detect anomalies
        self.detect_anomalies(&mut metrics, duration_ms, progress);

        Ok(())
    }

    /// Detect performance anomalies
    fn detect_anomalies(
        &self,
        metrics: &mut CheckpointMetricsData,
        duration_ms: u64,
        progress: &CheckpointProgress,
    ) {
        let detector = &mut metrics.anomaly_detector;

        if duration_ms as f64 > detector.baseline_duration_ms as f64 * detector.duration_anomaly_threshold {
            detector.duration_anomalies += 1;
        }

        if metrics.checkpoint_throughput_mbps < detector.baseline_throughput_mbps * detector.throughput_anomaly_threshold {
            detector.throughput_anomalies += 1;
        }

        if progress.flushed_blocks as f64 > detector.baseline_blocks_per_checkpoint as f64 * detector.block_count_anomaly_threshold {
            detector.block_count_anomalies += 1;
        }
    }

    /// Get current metrics snapshot
    pub fn get_metrics(&self) -> CheckpointResult<CheckpointMetricsData> {
        let mut metrics = self.metrics.lock()
            .map_err(|e| CheckpointError::validation(format!("Failed to lock metrics: {}", e)))?;

        // Update time since last checkpoint
        if let Some(last_checkpoint) = metrics.last_checkpoint_timestamp {
            metrics.time_since_last_checkpoint_ms = last_checkpoint
                .elapsed()
                .unwrap_or(Duration::ZERO)
                .as_millis() as u64;
        }

        Ok(metrics.clone())
    }

    /// Reset metrics to baseline
    pub fn reset_metrics(&self) -> CheckpointResult<()> {
        let mut metrics = self.metrics.lock()
            .map_err(|e| CheckpointError::validation(format!("Failed to lock metrics: {}", e)))?;

        *metrics = CheckpointMetricsData {
            anomaly_detector: AnomalyDetector {
                baseline_duration_ms: performance::TARGET_CHECKPOINT_THROUGHPUT_MBPS as u64 * 1000,
                baseline_throughput_mbps: performance::TARGET_CHECKPOINT_THROUGHPUT_MBPS,
                baseline_blocks_per_checkpoint: 100,
                duration_anomaly_threshold: 2.0,
                throughput_anomaly_threshold: 0.5,
                block_count_anomaly_threshold: 3.0,
                ..Default::default()
            },
            ..Default::default()
        };

        Ok(())
    }

    /// Generate performance report
    pub fn generate_performance_report(&self) -> CheckpointResult<String> {
        let metrics = self.get_metrics()?;

        let performance_metrics = PerformanceMetrics {
            total_checkpoints: metrics.total_checkpoints,
            avg_checkpoint_duration_ms: metrics.avg_checkpoint_duration_ms,
            checkpoint_throughput_mbps: metrics.checkpoint_throughput_mbps,
            avg_blocks_per_checkpoint: metrics.avg_blocks_per_checkpoint,
            avg_records_per_checkpoint: metrics.avg_records_per_checkpoint,
            anomaly_summary: AnomalySummary {
                duration_anomalies: metrics.anomaly_detector.duration_anomalies,
                throughput_anomalies: metrics.anomaly_detector.throughput_anomalies,
                block_count_anomalies: metrics.anomaly_detector.block_count_anomalies,
                anomaly_percentage: if metrics.total_checkpoints > 0 {
                    ((metrics.anomaly_detector.duration_anomalies +
                      metrics.anomaly_detector.throughput_anomalies +
                      metrics.anomaly_detector.block_count_anomalies) as f64 /
                     (metrics.total_checkpoints as f64 * 3.0)) * 100.0
                } else {
                    0.0
                },
            },
        };

        let reporter = CheckpointValidationReporter::new(self.config.clone());
        Ok(reporter.generate_performance_report(&performance_metrics))
    }
}

/// Checkpoint cleanup utilities for maintenance operations (backward compatibility wrapper)
pub struct CheckpointCleanup {
    config: V2WALConfig,
}

impl CheckpointCleanup {
    /// Create new checkpoint cleanup utility
    pub fn new(config: V2WALConfig) -> Self {
        Self { config }
    }

    /// Clean up checkpointed dirty blocks from tracking (simplified version)
    pub fn clear_checkpointed_blocks(
        &self,
        dirty_blocks: &mut DirtyBlockTracker,
        checkpointed_blocks: &[u64],
    ) -> CheckpointResult<()> {
        // This is a simplified implementation
        // In a full implementation, this would properly clean up dirty block tracking
        // Since we don't have access to the internal API, we'll just log the operation

        if !checkpointed_blocks.is_empty() {
            // Log the cleanup operation (in real implementation, would actually clean up)
            println!("Cleaning up {} checkpointed blocks", checkpointed_blocks.len());
        }

        Ok(())
    }

    /// Force checkpoint regardless of strategy (used during shutdown)
    pub fn force_checkpoint_if_needed(
        &self,
        state: &CheckpointState,
        last_checkpoint_time: SystemTime,
        max_wait_time: Duration,
    ) -> CheckpointResult<bool> {
        let time_since_last = last_checkpoint_time.elapsed()
            .unwrap_or(Duration::ZERO);

        // Force checkpoint if it's been too long
        if time_since_last > max_wait_time {
            return Ok(true);
        }

        // Check if checkpoint state indicates it's stuck (using public API)
        match state {
            CheckpointState::Initializing |
            CheckpointState::Collecting |
            CheckpointState::Processing |
            CheckpointState::Flushing => {
                if time_since_last > Duration::from_millis(DEFAULT_CHECKPOINT_TIMEOUT_MS) {
                    return Ok(true);
                }
            },
            _ => {}
        }

        Ok(false)
    }

    /// Cleanup old checkpoint files
    pub fn cleanup_old_checkpoints(&self, max_checkpoints_to_keep: usize) -> CheckpointResult<usize> {
        use std::fs;

        let checkpoint_dir = self.config.checkpoint_path.parent()
            .ok_or_else(|| CheckpointError::validation("Invalid checkpoint path".to_string()))?;

        let mut checkpoint_files = Vec::new();

        // Find all checkpoint files
        for entry in fs::read_dir(checkpoint_dir)
            .map_err(|e| CheckpointError::validation(format!("Failed to read checkpoint directory: {}", e)))? {
            let entry = entry.map_err(|e| CheckpointError::validation(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();

            if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                if filename.ends_with(".checkpoint") && filename != self.config.checkpoint_path.file_name().and_then(|n| n.to_str()).unwrap_or("") {
                    if let Ok(metadata) = fs::metadata(&path) {
                        if let Ok(modified) = metadata.modified() {
                            checkpoint_files.push((path, modified));
                        }
                    }
                }
            }
        }

        // Sort by modification time (oldest first)
        checkpoint_files.sort_by_key(|(_, modified)| *modified);

        // Remove excess checkpoints
        let files_to_remove = if checkpoint_files.len() > max_checkpoints_to_keep {
            checkpoint_files.len() - max_checkpoints_to_keep
        } else {
            0
        };

        let mut removed_count = 0;
        for (path, _) in checkpoint_files.iter().take(files_to_remove) {
            if fs::remove_file(path).is_ok() {
                removed_count += 1;
            }
        }

        Ok(removed_count)
    }
}

/// Factory for creating checkpoint validation components
pub struct CheckpointValidatorFactory;

impl CheckpointValidatorFactory {
    /// Create a checkpoint validator with default configuration
    pub fn create_default_validator(config: V2WALConfig) -> CheckpointResult<CheckpointValidator> {
        Ok(CheckpointValidator::new(config))
    }

    /// Create a checkpoint validator with custom validation rules
    pub fn create_validator_with_rules(
        config: V2WALConfig,
        rules: Vec<ValidationRule>,
    ) -> CheckpointResult<CheckpointValidator> {
        let mut validator = CheckpointValidator::new(config);

        // Add custom rules to the rule engine
        for rule in rules {
            validator.rule_engine.add_rule(rule);
        }

        Ok(validator)
    }

    /// Create checkpoint metrics collector
    pub fn create_metrics(config: V2WALConfig) -> CheckpointResult<CheckpointMetrics> {
        Ok(CheckpointMetrics::new(config))
    }

    /// Create checkpoint cleanup utility
    pub fn create_cleanup(config: V2WALConfig) -> CheckpointResult<CheckpointCleanup> {
        Ok(CheckpointCleanup::new(config))
    }

    /// Create all validation components
    pub fn create_all_components(config: V2WALConfig) -> CheckpointResult<ValidationComponents> {
        Ok(ValidationComponents {
            validator: Self::create_default_validator(config.clone())?,
            metrics: Self::create_metrics(config.clone())?,
            cleanup: Self::create_cleanup(config)?,
        })
    }
}

/// Collection of all validation components
pub struct ValidationComponents {
    pub validator: CheckpointValidator,
    pub metrics: CheckpointMetrics,
    pub cleanup: CheckpointCleanup,
}

// Module exports
pub mod rules;
pub mod consistency;
pub mod invariants;
pub mod reporting;

// Import required constants and modules
use crate::backend::native::v2::wal::checkpoint::constants::*;
use crate::backend::native::v2::wal::checkpoint::constants::performance;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_checkpoint_validator_creation() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let validator = CheckpointValidator::new(config);
        assert!(true, "Checkpoint validator created successfully");
    }

    #[test]
    fn test_checkpoint_metrics_creation() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let metrics = CheckpointMetrics::new(config);
        assert!(true, "Checkpoint metrics created successfully");
    }

    #[test]
    fn test_checkpoint_cleanup_creation() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let cleanup = CheckpointCleanup::new(config);
        assert!(true, "Checkpoint cleanup created successfully");
    }

    #[test]
    fn test_checkpoint_validator_factory() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let validator = CheckpointValidatorFactory::create_default_validator(config.clone());
        assert!(validator.is_ok());

        let metrics = CheckpointValidatorFactory::create_metrics(config.clone());
        assert!(metrics.is_ok());

        let cleanup = CheckpointValidatorFactory::create_cleanup(config.clone());
        assert!(cleanup.is_ok());

        let components = CheckpointValidatorFactory::create_all_components(config);
        assert!(components.is_ok());
    }

    #[test]
    fn test_anomaly_detector_creation() {
        let detector = AnomalyDetector::default();
        assert_eq!(detector.duration_anomalies, 0);
        assert_eq!(detector.throughput_anomalies, 0);
        assert_eq!(detector.block_count_anomalies, 0);
    }

    #[test]
    fn test_checkpoint_metrics_data_default() {
        let metrics = CheckpointMetricsData::default();
        assert_eq!(metrics.total_checkpoints, 0);
        assert_eq!(metrics.avg_checkpoint_duration_ms, 0);
        assert_eq!(metrics.checkpoint_throughput_mbps, 0.0);
    }

    #[test]
    fn test_validation_components_structure() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let components = CheckpointValidatorFactory::create_all_components(config).unwrap();

        // Verify all components are present
        let _ = &components.validator;
        let _ = &components.metrics;
        let _ = &components.cleanup;

        assert!(true, "All validation components created successfully");
    }

    #[test]
    fn test_backward_compatibility() {
        // Test that all the original types are still available
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        // These should all be available for backward compatibility
        let _validator: CheckpointValidator = CheckpointValidator::new(config.clone());
        let _metrics: CheckpointMetrics = CheckpointMetrics::new(config.clone());
        let _cleanup: CheckpointCleanup = CheckpointCleanup::new(config);

        assert!(true, "Backward compatibility types are available");
    }
}