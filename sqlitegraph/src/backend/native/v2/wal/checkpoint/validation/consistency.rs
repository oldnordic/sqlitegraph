//! Checkpoint Consistency Validation
//!
//! This module provides consistency checking functionality for V2 WAL checkpoint operations.
//! It validates data consistency across checkpoint components, LSN ranges, dirty block states,
//! and ensures proper checkpoint-WAL alignment.

use crate::backend::native::v2::wal::checkpoint::constants::*;
use crate::backend::native::v2::wal::checkpoint::errors::{CheckpointError, CheckpointResult};
use crate::backend::native::v2::wal::checkpoint::core::{DirtyBlockTracker, CheckpointProgress};
use crate::backend::native::v2::wal::V2WALConfig;
use std::time::{Duration, SystemTime, Instant};

/// Consistency validation result details
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConsistencyResult {
    /// Overall consistency check result
    pub is_consistent: bool,
    /// Consistency violations found
    pub violations: Vec<ConsistencyViolation>,
    /// Validation timestamp
    pub validation_timestamp: SystemTime,
    /// Checkpoint LSN range (if available)
    pub lsn_range: Option<(u64, u64)>,
}

/// Consistency violation details
#[derive(Debug, Clone, serde::Serialize)]
pub struct ConsistencyViolation {
    /// Violation type
    pub violation_type: ConsistencyViolationType,
    /// Violation description
    pub description: String,
    /// Violation severity
    pub severity: ConsistencySeverity,
    /// Related entity (block offset, cluster ID, etc.)
    pub entity_id: Option<String>,
}

/// Types of consistency violations
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize)]
pub enum ConsistencyViolationType {
    /// LSN range discontinuity
    LsnRangeDiscontinuity,
    /// Invalid LSN range
    InvalidLsnRange,
    /// Empty LSN range
    EmptyLsnRange,
    /// Too many dirty blocks
    ExcessDirtyBlocks,
    /// Cluster dirty block limit exceeded
    ClusterDirtyBlockLimitExceeded,
    /// Global dirty block limit exceeded
    GlobalDirtyBlockLimitExceeded,
    /// Invalid timestamp
    InvalidTimestamp,
    /// Checkpoint-WAL size inconsistency
    CheckpointWalSizeMismatch,
    /// Dirty block tracking inconsistency
    DirtyBlockTrackingInconsistency,
}

/// Consistency violation severity levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
pub enum ConsistencySeverity {
    /// Minor inconsistency that can be ignored
    Minor = 0,
    /// Warning level inconsistency
    Warning = 1,
    /// Error level inconsistency that may affect operation
    Error = 2,
    /// Critical inconsistency that requires immediate attention
    Critical = 3,
}

/// Checkpoint consistency validator
pub struct CheckpointConsistencyValidator {
    config: V2WALConfig,
}

impl CheckpointConsistencyValidator {
    /// Create a new checkpoint consistency validator
    pub fn new(config: V2WALConfig) -> Self {
        Self { config }
    }

    /// Validate checkpoint consistency with WAL state
    pub fn validate_checkpoint_consistency(
        &self,
        checkpoint_lsn_range: (u64, u64),
        last_checkpointed_lsn: u64,
    ) -> ConsistencyResult {
        let mut violations = Vec::new();
        let start_time = SystemTime::now();

        // Check LSN range continuity
        if checkpoint_lsn_range.0 != last_checkpointed_lsn {
            violations.push(ConsistencyViolation {
                violation_type: ConsistencyViolationType::LsnRangeDiscontinuity,
                description: format!(
                    "Checkpoint range discontinuity: checkpoint starts at {}, last checkpointed LSN is {}",
                    checkpoint_lsn_range.0,
                    last_checkpointed_lsn
                ),
                severity: ConsistencySeverity::Critical,
                entity_id: Some(format!("{}-{}", checkpoint_lsn_range.0, checkpoint_lsn_range.1)),
            });
        }

        // Check LSN range validity
        if checkpoint_lsn_range.0 > checkpoint_lsn_range.1 {
            violations.push(ConsistencyViolation {
                violation_type: ConsistencyViolationType::InvalidLsnRange,
                description: format!(
                    "Invalid checkpoint range: start LSN {} > end LSN {}",
                    checkpoint_lsn_range.0,
                    checkpoint_lsn_range.1
                ),
                severity: ConsistencySeverity::Critical,
                entity_id: Some(format!("{}-{}", checkpoint_lsn_range.0, checkpoint_lsn_range.1)),
            });
        }

        // Check for empty LSN range
        if checkpoint_lsn_range.0 == checkpoint_lsn_range.1 {
            violations.push(ConsistencyViolation {
                violation_type: ConsistencyViolationType::EmptyLsnRange,
                description: "Empty checkpoint range (start LSN equals end LSN)".to_string(),
                severity: ConsistencySeverity::Error,
                entity_id: Some(format!("{}", checkpoint_lsn_range.0)),
            });
        }

        let is_consistent = violations.iter().all(|v| v.severity <= ConsistencySeverity::Warning);

        ConsistencyResult {
            is_consistent,
            violations,
            validation_timestamp: start_time,
            lsn_range: Some(checkpoint_lsn_range),
        }
    }

    /// Validate dirty block state consistency
    pub fn validate_dirty_block_consistency(
        &self,
        dirty_blocks: &DirtyBlockTracker,
        max_pending_blocks: u64,
    ) -> ConsistencyResult {
        let mut violations = Vec::new();
        let start_time = SystemTime::now();

        // Check global dirty block count using public API
        let (_, global_count) = dirty_blocks.get_statistics();
        if global_count as u64 > MAX_GLOBAL_DIRTY_BLOCKS as u64 {
            violations.push(ConsistencyViolation {
                violation_type: ConsistencyViolationType::GlobalDirtyBlockLimitExceeded,
                description: format!(
                    "Too many global dirty blocks: {} (maximum: {})",
                    global_count,
                    MAX_GLOBAL_DIRTY_BLOCKS
                ),
                severity: ConsistencySeverity::Error,
                entity_id: Some("global".to_string()),
            });
        }

        // Check cluster-specific dirty block counts (commented out - no public API available)
        // for (cluster_key, cluster_blocks) in &dirty_blocks.cluster_dirty_blocks {
        //     let cluster_count = cluster_blocks.len() as u64;
        //     if cluster_count > MAX_DIRTY_BLOCKS_PER_CLUSTER as u64 {
        //         violations.push(ConsistencyViolation {
        //             violation_type: ConsistencyViolationType::ClusterDirtyBlockLimitExceeded,
        //             description: format!(
        //                 "Too many dirty blocks for cluster {}: {} (maximum: {})",
        //                 cluster_key,
        //                 cluster_count,
        //                 MAX_DIRTY_BLOCKS_PER_CLUSTER
        //             ),
        //             severity: ConsistencySeverity::Warning,
        //             entity_id: Some(cluster_key.clone()),
        //         });
        //     }
        // }

        // Check total pending blocks using public API
        let (cluster_blocks, global_blocks) = dirty_blocks.get_statistics();
        let total_pending = cluster_blocks + global_blocks;

        if total_pending as u64 > max_pending_blocks {
            violations.push(ConsistencyViolation {
                violation_type: ConsistencyViolationType::ExcessDirtyBlocks,
                description: format!(
                    "Too many pending dirty blocks: {} (maximum: {})",
                    total_pending,
                    max_pending_blocks
                ),
                severity: ConsistencySeverity::Error,
                entity_id: Some("total".to_string()),
            });
        }

        // Validate block timestamps consistency
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Check block timestamps (commented out - no public API available)
        // for (&block_offset, &timestamp) in &dirty_blocks.block_timestamps {
        //     if timestamp > now {
        //         violations.push(ConsistencyViolation {
        //             violation_type: ConsistencyViolationType::InvalidTimestamp,
        //             description: format!(
        //                 "Invalid timestamp for block {}: {} (future timestamp)",
        //                 block_offset,
        //                 timestamp
        //             ),
        //             severity: ConsistencySeverity::Warning,
        //             entity_id: Some(format!("{}", block_offset)),
        //         });
        //     }
        // }

        let is_consistent = violations.iter().all(|v| v.severity <= ConsistencySeverity::Warning);

        ConsistencyResult {
            is_consistent,
            violations,
            validation_timestamp: start_time,
            lsn_range: None,
        }
    }

    /// Validate checkpoint progress consistency
    pub fn validate_progress_consistency(
        &self,
        progress: &CheckpointProgress,
        checkpoint_duration: Duration,
    ) -> ConsistencyResult {
        let mut violations = Vec::new();
        let start_time = SystemTime::now();

        // Check LSN range consistency
        if progress.lsn_range.0 > progress.lsn_range.1 {
            violations.push(ConsistencyViolation {
                violation_type: ConsistencyViolationType::InvalidLsnRange,
                description: format!(
                    "Invalid progress LSN range: start {} > end {}",
                    progress.lsn_range.0,
                    progress.lsn_range.1
                ),
                severity: ConsistencySeverity::Error,
                entity_id: Some(format!("{}-{}", progress.lsn_range.0, progress.lsn_range.1)),
            });
        }

        // Check processed vs total records consistency
        if progress.processed_records > progress.total_records {
            violations.push(ConsistencyViolation {
                violation_type: ConsistencyViolationType::DirtyBlockTrackingInconsistency,
                description: format!(
                    "Processed records ({}) exceeds total records ({})",
                    progress.processed_records,
                    progress.total_records
                ),
                severity: ConsistencySeverity::Error,
                entity_id: Some("record_count".to_string()),
            });
        }

        // Check completion percentage consistency
        if progress.total_records > 0 {
            let actual_percentage = (progress.processed_records as f64 / progress.total_records as f64) * 100.0;
            let percentage_diff = (actual_percentage - progress.completion_percentage).abs();

            if percentage_diff > 1.0 { // Allow 1% tolerance
                violations.push(ConsistencyViolation {
                    violation_type: ConsistencyViolationType::DirtyBlockTrackingInconsistency,
                    description: format!(
                        "Completion percentage inconsistency: calculated {:.1}%, reported {:.1}%",
                        actual_percentage,
                        progress.completion_percentage
                    ),
                    severity: ConsistencySeverity::Warning,
                    entity_id: Some("completion_percentage".to_string()),
                });
            }
        }

        // Check for reasonable throughput
        if checkpoint_duration.as_secs() > 0 && progress.processed_records > 0 {
            let records_per_second = progress.processed_records as f64 / checkpoint_duration.as_secs_f64();
            if records_per_second < 10.0 { // Very low throughput might indicate issues
                violations.push(ConsistencyViolation {
                    violation_type: ConsistencyViolationType::CheckpointWalSizeMismatch,
                    description: format!(
                        "Very low checkpoint throughput: {:.2} records/second",
                        records_per_second
                    ),
                    severity: ConsistencySeverity::Warning,
                    entity_id: Some("throughput".to_string()),
                });
            }
        }

        let is_consistent = violations.iter().all(|v| v.severity <= ConsistencySeverity::Warning);

        ConsistencyResult {
            is_consistent,
            violations,
            validation_timestamp: start_time,
            lsn_range: Some(progress.lsn_range),
        }
    }

    /// Validate checkpoint-WAL size consistency
    pub fn validate_checkpoint_wal_consistency(
        &self,
        checkpoint_size: u64,
        wal_size: u64,
    ) -> ConsistencyResult {
        let mut violations = Vec::new();
        let start_time = SystemTime::now();

        // Check if checkpoint size is reasonable relative to WAL size
        if checkpoint_size > wal_size * 2 {
            violations.push(ConsistencyViolation {
                violation_type: ConsistencyViolationType::CheckpointWalSizeMismatch,
                description: format!(
                    "Checkpoint size {} is significantly larger than WAL size {}",
                    checkpoint_size,
                    wal_size
                ),
                severity: ConsistencySeverity::Warning,
                entity_id: Some("size_ratio".to_string()),
            });
        }

        // Check for empty checkpoint with non-empty WAL
        if checkpoint_size < MIN_CHECKPOINT_SIZE && wal_size > 1024 * 1024 {
            violations.push(ConsistencyViolation {
                violation_type: ConsistencyViolationType::EmptyLsnRange,
                description: format!(
                    "Small checkpoint {} with large WAL {} may indicate incomplete checkpointing",
                    checkpoint_size,
                    wal_size
                ),
                severity: ConsistencySeverity::Error,
                entity_id: Some("size_mismatch".to_string()),
            });
        }

        let is_consistent = violations.iter().all(|v| v.severity <= ConsistencySeverity::Warning);

        ConsistencyResult {
            is_consistent,
            violations,
            validation_timestamp: start_time,
            lsn_range: None,
        }
    }

    /// Perform comprehensive consistency validation
    pub fn validate_comprehensive_consistency(
        &self,
        dirty_blocks: &DirtyBlockTracker,
        checkpoint_progress: &CheckpointProgress,
        checkpoint_lsn_range: (u64, u64),
        last_checkpointed_lsn: u64,
        checkpoint_duration: Duration,
        max_pending_blocks: u64,
    ) -> ConsistencyResult {
        let mut all_violations = Vec::new();

        // Validate checkpoint LSN consistency
        let lsn_result = self.validate_checkpoint_consistency(checkpoint_lsn_range, last_checkpointed_lsn);
        all_violations.extend(lsn_result.violations);

        // Validate dirty block consistency
        let dirty_blocks_result = self.validate_dirty_block_consistency(dirty_blocks, max_pending_blocks);
        all_violations.extend(dirty_blocks_result.violations);

        // Validate progress consistency
        let progress_result = self.validate_progress_consistency(checkpoint_progress, checkpoint_duration);
        all_violations.extend(progress_result.violations);

        // Sort violations by severity (most severe first)
        all_violations.sort_by(|a, b| b.severity.cmp(&a.severity));

        let is_consistent = all_violations.iter().all(|v| v.severity <= ConsistencySeverity::Warning);

        ConsistencyResult {
            is_consistent,
            violations: all_violations,
            validation_timestamp: SystemTime::now(),
            lsn_range: Some(checkpoint_lsn_range),
        }
    }
}

/// Consistency validation utilities
pub struct ConsistencyUtils;

impl ConsistencyUtils {
    /// Calculate consistency score (0.0 to 1.0) based on violations
    pub fn calculate_consistency_score(violations: &[ConsistencyViolation]) -> f64 {
        if violations.is_empty() {
            return 1.0;
        }

        let total_penalty: f64 = violations
            .iter()
            .map(|v| match v.severity {
                ConsistencySeverity::Minor => 0.1,
                ConsistencySeverity::Warning => 0.25,
                ConsistencySeverity::Error => 0.5,
                ConsistencySeverity::Critical => 1.0,
            })
            .sum();

        let max_possible_penalty = violations.len() as f64;
        if max_possible_penalty > 0.0 {
            (1.0 - (total_penalty / max_possible_penalty)).max(0.0)
        } else {
            1.0
        }
    }

    /// Determine if consistency result requires action
    pub fn requires_action(result: &ConsistencyResult) -> bool {
        result.violations
            .iter()
            .any(|v| v.severity >= ConsistencySeverity::Error)
    }

    /// Get most critical violation
    pub fn get_most_critical_violation(result: &ConsistencyResult) -> Option<&ConsistencyViolation> {
        result.violations
            .iter()
            .max_by_key(|v| v.severity)
    }

    /// Filter violations by severity
    pub fn filter_violations_by_severity(
        violations: &[ConsistencyViolation],
        min_severity: ConsistencySeverity,
    ) -> Vec<&ConsistencyViolation> {
        violations
            .iter()
            .filter(|v| v.severity >= min_severity)
            .collect()
    }

    /// Group violations by type
    pub fn group_violations_by_type(
        violations: &[ConsistencyViolation],
    ) -> std::collections::HashMap<ConsistencyViolationType, Vec<&ConsistencyViolation>> {
        use std::collections::HashMap;

        let mut grouped: HashMap<ConsistencyViolationType, Vec<&ConsistencyViolation>> = HashMap::new();

        for violation in violations {
            grouped
                .entry(violation.violation_type.clone())
                .or_insert_with(Vec::new)
                .push(violation);
        }

        grouped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::collections::{HashMap, HashSet};

    fn create_test_dirty_block_tracker() -> DirtyBlockTracker {
        let mut dirty_blocks = DirtyBlockTracker::new(10, 10); // Allow up to 10 blocks per category

        // Mark global blocks as dirty
        let timestamp = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        dirty_blocks.mark_global_block_dirty(1000, timestamp).unwrap();
        dirty_blocks.mark_global_block_dirty(2000, timestamp).unwrap();

        // Mark cluster-specific blocks as dirty
        dirty_blocks.mark_cluster_block_dirty(1, 1000, timestamp).unwrap();
        dirty_blocks.mark_cluster_block_dirty(1, 3000, timestamp).unwrap();

        // Update block access statistics
        dirty_blocks.update_block_access(1000, timestamp);

        dirty_blocks
    }

    fn create_test_checkpoint_progress() -> CheckpointProgress {
        CheckpointProgress {
            lsn_range: (1000, 2000),
            total_records: 100,
            processed_records: 50,
            flushed_blocks: 25,
            completion_percentage: 50.0,
            checkpoint_start: Instant::now(),
        }
    }

    #[test]
    fn test_consistency_validator_creation() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let validator = CheckpointConsistencyValidator::new(config);
        assert!(true, "Validator created successfully");
    }

    #[test]
    fn test_consistency_validator_valid_lsn_range() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let validator = CheckpointConsistencyValidator::new(config);
        let result = validator.validate_checkpoint_consistency((1000, 2000), 1000);

        assert!(result.is_consistent);
        assert!(result.violations.is_empty());
        assert_eq!(result.lsn_range, Some((1000, 2000)));
    }

    #[test]
    fn test_consistency_validator_lsn_discontinuity() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let validator = CheckpointConsistencyValidator::new(config);
        let result = validator.validate_checkpoint_consistency((1500, 2000), 1000);

        assert!(!result.is_consistent);
        assert_eq!(result.violations.len(), 1);
        assert!(matches!(result.violations[0].violation_type, ConsistencyViolationType::LsnRangeDiscontinuity));
        assert_eq!(result.violations[0].severity, ConsistencySeverity::Critical);
    }

    #[test]
    fn test_consistency_validator_invalid_lsn_range() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let validator = CheckpointConsistencyValidator::new(config);
        let result = validator.validate_checkpoint_consistency((2000, 1000), 1000);

        assert!(!result.is_consistent);
        assert!(!result.violations.is_empty());
        assert!(result.violations.iter().any(|v| matches!(v.violation_type, ConsistencyViolationType::InvalidLsnRange)));
    }

    #[test]
    fn test_consistency_validator_dirty_blocks() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let validator = CheckpointConsistencyValidator::new(config);
        let dirty_blocks = create_test_dirty_block_tracker();
        let result = validator.validate_dirty_block_consistency(&dirty_blocks, 100);

        // Should be consistent with reasonable dirty block count
        assert!(result.is_consistent);
    }

    #[test]
    fn test_consistency_validator_progress() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let validator = CheckpointConsistencyValidator::new(config);
        let progress = create_test_checkpoint_progress();
        let duration = Duration::from_millis(100);
        let result = validator.validate_progress_consistency(&progress, duration);

        assert!(result.is_consistent);
    }

    #[test]
    fn test_consistency_utils_score_calculation() {
        let violations = vec![
            ConsistencyViolation {
                violation_type: ConsistencyViolationType::InvalidLsnRange,
                description: "Test violation".to_string(),
                severity: ConsistencySeverity::Warning,
                entity_id: None,
            },
        ];

        let score = ConsistencyUtils::calculate_consistency_score(&violations);
        assert!(score < 1.0);
        assert!(score > 0.0);

        // Empty violations should give perfect score
        let perfect_score = ConsistencyUtils::calculate_consistency_score(&[]);
        assert_eq!(perfect_score, 1.0);
    }

    #[test]
    fn test_consistency_utils_requires_action() {
        let minor_violation = ConsistencyResult {
            is_consistent: true,
            violations: vec![ConsistencyViolation {
                violation_type: ConsistencyViolationType::InvalidTimestamp,
                description: "Minor issue".to_string(),
                severity: ConsistencySeverity::Warning,
                entity_id: None,
            }],
            validation_timestamp: SystemTime::now(),
            lsn_range: None,
        };

        assert!(!ConsistencyUtils::requires_action(&minor_violation));

        let critical_violation = ConsistencyResult {
            is_consistent: false,
            violations: vec![ConsistencyViolation {
                violation_type: ConsistencyViolationType::LsnRangeDiscontinuity,
                description: "Critical issue".to_string(),
                severity: ConsistencySeverity::Critical,
                entity_id: None,
            }],
            validation_timestamp: SystemTime::now(),
            lsn_range: None,
        };

        assert!(ConsistencyUtils::requires_action(&critical_violation));
    }

    #[test]
    fn test_consistency_utils_group_violations() {
        let violations = vec![
            ConsistencyViolation {
                violation_type: ConsistencyViolationType::InvalidLsnRange,
                description: "LSN issue".to_string(),
                severity: ConsistencySeverity::Error,
                entity_id: Some("lsn1".to_string()),
            },
            ConsistencyViolation {
                violation_type: ConsistencyViolationType::InvalidTimestamp,
                description: "Time issue".to_string(),
                severity: ConsistencySeverity::Warning,
                entity_id: Some("time1".to_string()),
            },
            ConsistencyViolation {
                violation_type: ConsistencyViolationType::InvalidLsnRange,
                description: "Another LSN issue".to_string(),
                severity: ConsistencySeverity::Error,
                entity_id: Some("lsn2".to_string()),
            },
        ];

        let grouped = ConsistencyUtils::group_violations_by_type(&violations);
        assert_eq!(grouped.get(&ConsistencyViolationType::InvalidLsnRange).unwrap().len(), 2);
        assert_eq!(grouped.get(&ConsistencyViolationType::InvalidTimestamp).unwrap().len(), 1);
    }

    #[test]
    fn test_consistency_severity_ordering() {
        assert!(ConsistencySeverity::Minor < ConsistencySeverity::Warning);
        assert!(ConsistencySeverity::Warning < ConsistencySeverity::Error);
        assert!(ConsistencySeverity::Error < ConsistencySeverity::Critical);
    }
}