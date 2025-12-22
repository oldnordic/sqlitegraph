//! Checkpoint Validation Reporting
//!
//! This module provides comprehensive reporting functionality for V2 WAL checkpoint validation.
//! It generates detailed validation reports, performance reports, and analysis summaries
//! for checkpoint operations and consistency checks.

use super::consistency::{
    ConsistencyResult, ConsistencySeverity,
};
use super::invariants::V2InvariantResult;
use crate::backend::native::v2::wal::V2WALConfig;
use crate::backend::native::v2::wal::checkpoint::constants::performance;
use crate::backend::native::v2::wal::checkpoint::errors::{CheckpointError, CheckpointResult};
use std::time::SystemTime;

/// Checkpoint validation report
#[derive(Debug, Clone, serde::Serialize)]
pub struct CheckpointValidationReport {
    /// Report generation timestamp
    pub report_timestamp: SystemTime,
    /// Checkpoint file path
    pub checkpoint_path: String,
    /// Consistency validation results
    pub consistency_result: Option<ConsistencyResult>,
    /// V2 invariant validation results
    pub v2_invariant_result: Option<V2InvariantResult>,
    /// Overall validation status
    pub validation_status: ValidationStatus,
    /// Performance metrics
    pub performance_metrics: Option<PerformanceMetrics>,
    /// Validation summary
    pub summary: ValidationSummary,
}

/// Overall validation status
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub enum ValidationStatus {
    /// All validations passed
    Passed,
    /// Validations passed with warnings
    PassedWithWarnings,
    /// Validations failed with errors
    Failed,
    /// Validations had critical failures
    CriticalFailure,
    /// Validation not completed
    Incomplete,
}

/// Performance metrics for checkpoint validation
#[derive(Debug, Clone, serde::Serialize)]
pub struct PerformanceMetrics {
    /// Total checkpoints processed
    pub total_checkpoints: u64,
    /// Average checkpoint duration (milliseconds)
    pub avg_checkpoint_duration_ms: u64,
    /// Checkpoint throughput (MB/s)
    pub checkpoint_throughput_mbps: f64,
    /// Average blocks per checkpoint
    pub avg_blocks_per_checkpoint: u64,
    /// Average records per checkpoint
    pub avg_records_per_checkpoint: u64,
    /// Anomaly detection results
    pub anomaly_summary: AnomalySummary,
}

/// Anomaly detection summary
#[derive(Debug, Clone, serde::Serialize)]
pub struct AnomalySummary {
    /// Duration anomalies detected
    pub duration_anomalies: u64,
    /// Throughput anomalies detected
    pub throughput_anomalies: u64,
    /// Block count anomalies detected
    pub block_count_anomalies: u64,
    /// Total anomaly percentage
    pub anomaly_percentage: f64,
}

/// Validation summary statistics
#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationSummary {
    /// Total violations found
    pub total_violations: usize,
    /// Critical violations
    pub critical_violations: usize,
    /// Error violations
    pub error_violations: usize,
    /// Warning violations
    pub warning_violations: usize,
    /// Info violations
    pub info_violations: usize,
    /// Validation score (0.0 to 1.0)
    pub validation_score: f64,
    /// Validation duration (milliseconds)
    pub validation_duration_ms: u64,
}

/// Checkpoint validation report generator
pub struct CheckpointValidationReporter {
    config: V2WALConfig,
}

impl CheckpointValidationReporter {
    /// Create a new checkpoint validation reporter
    pub fn new(config: V2WALConfig) -> Self {
        Self { config }
    }

    /// Generate comprehensive validation report
    pub fn generate_validation_report(
        &self,
        checkpoint_path: &std::path::Path,
        consistency_result: Option<ConsistencyResult>,
        v2_invariant_result: Option<V2InvariantResult>,
        performance_metrics: Option<PerformanceMetrics>,
        validation_duration: Option<Duration>,
    ) -> CheckpointValidationReport {
        let report_timestamp = SystemTime::now();
        let validation_duration_ms = validation_duration
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        // Determine overall validation status
        let validation_status =
            self.determine_validation_status(&consistency_result, &v2_invariant_result);

        // Generate validation summary
        let summary = self.generate_validation_summary(
            &consistency_result,
            &v2_invariant_result,
            validation_duration_ms,
        );

        CheckpointValidationReport {
            report_timestamp,
            checkpoint_path: checkpoint_path.to_string_lossy().to_string(),
            consistency_result,
            v2_invariant_result,
            validation_status,
            performance_metrics,
            summary,
        }
    }

    /// Determine overall validation status
    fn determine_validation_status(
        &self,
        consistency_result: &Option<ConsistencyResult>,
        v2_invariant_result: &Option<V2InvariantResult>,
    ) -> ValidationStatus {
        let mut has_critical = false;
        let mut has_errors = false;
        let mut has_warnings = false;

        // Check consistency results
        if let Some(consistency) = consistency_result {
            for violation in &consistency.violations {
                match violation.severity {
                    ConsistencySeverity::Critical => has_critical = true,
                    ConsistencySeverity::Error => has_errors = true,
                    ConsistencySeverity::Warning => has_warnings = true,
                    ConsistencySeverity::Minor => has_warnings = true,
                }
            }
        }

        // Check V2 invariant results
        if let Some(invariants) = v2_invariant_result {
            for violation in &invariants.violations {
                if violation.critical {
                    has_critical = true;
                } else {
                    has_errors = true;
                }
            }
        }

        // Determine status based on violations
        if has_critical {
            ValidationStatus::CriticalFailure
        } else if has_errors {
            ValidationStatus::Failed
        } else if has_warnings {
            ValidationStatus::PassedWithWarnings
        } else {
            ValidationStatus::Passed
        }
    }

    /// Generate validation summary
    fn generate_validation_summary(
        &self,
        consistency_result: &Option<ConsistencyResult>,
        v2_invariant_result: &Option<V2InvariantResult>,
        validation_duration_ms: u64,
    ) -> ValidationSummary {
        let mut summary = ValidationSummary {
            total_violations: 0,
            critical_violations: 0,
            error_violations: 0,
            warning_violations: 0,
            info_violations: 0,
            validation_score: 1.0,
            validation_duration_ms,
        };

        // Count consistency violations
        if let Some(consistency) = consistency_result {
            for violation in &consistency.violations {
                summary.total_violations += 1;
                match violation.severity {
                    ConsistencySeverity::Critical => summary.critical_violations += 1,
                    ConsistencySeverity::Error => summary.error_violations += 1,
                    ConsistencySeverity::Warning => summary.warning_violations += 1,
                    ConsistencySeverity::Minor => summary.info_violations += 1,
                }
            }
        }

        // Count V2 invariant violations
        if let Some(invariants) = v2_invariant_result {
            for violation in &invariants.violations {
                summary.total_violations += 1;
                if violation.critical {
                    summary.critical_violations += 1;
                } else {
                    summary.error_violations += 1;
                }
            }
        }

        // Calculate validation score
        if summary.total_violations > 0 {
            let critical_weight = 10.0;
            let error_weight = 5.0;
            let warning_weight = 1.0;
            let info_weight = 0.1;

            let total_score = summary.critical_violations as f64 * critical_weight
                + summary.error_violations as f64 * error_weight
                + summary.warning_violations as f64 * warning_weight
                + summary.info_violations as f64 * info_weight;

            let max_possible_score = summary.total_violations as f64 * critical_weight;
            summary.validation_score = (1.0 - (total_score / max_possible_score)).max(0.0);
        }

        summary
    }

    /// Generate performance report from checkpoint metrics
    pub fn generate_performance_report(&self, metrics: &PerformanceMetrics) -> String {
        let mut report = String::new();
        report.push_str("V2 WAL Checkpoint Performance Report\n");
        report.push_str("=====================================\n\n");

        report.push_str("Performance Metrics:\n");
        report.push_str(&format!(
            "  Total Checkpoints: {}\n",
            metrics.total_checkpoints
        ));
        report.push_str(&format!(
            "  Average Duration: {} ms\n",
            metrics.avg_checkpoint_duration_ms
        ));
        report.push_str(&format!(
            "  Average Throughput: {:.2} MB/s\n",
            metrics.checkpoint_throughput_mbps
        ));
        report.push_str(&format!(
            "  Average Blocks per Checkpoint: {}\n",
            metrics.avg_blocks_per_checkpoint
        ));
        report.push_str(&format!(
            "  Average Records per Checkpoint: {}\n",
            metrics.avg_records_per_checkpoint
        ));

        if metrics.total_checkpoints > 0 {
            report.push_str("\nAnomaly Detection:\n");
            report.push_str(&format!(
                "  Duration Anomalies: {} ({:.1}%)\n",
                metrics.anomaly_summary.duration_anomalies,
                metrics.anomaly_summary.anomaly_percentage
            ));
            report.push_str(&format!(
                "  Throughput Anomalies: {} ({:.1}%)\n",
                metrics.anomaly_summary.throughput_anomalies,
                metrics.anomaly_summary.anomaly_percentage
            ));
            report.push_str(&format!(
                "  Block Count Anomalies: {} ({:.1}%)\n",
                metrics.anomaly_summary.block_count_anomalies,
                metrics.anomaly_summary.anomaly_percentage
            ));
        }

        // Performance analysis
        report.push_str("\nPerformance Analysis:\n");
        if metrics.checkpoint_throughput_mbps >= performance::TARGET_CHECKPOINT_THROUGHPUT_MBPS {
            report.push_str("  ✅ Throughput meets target\n");
        } else {
            report.push_str("  ⚠️  Throughput below target\n");
        }

        if metrics.avg_checkpoint_duration_ms <= performance::MAX_CHECKPOINT_DURATION_MS {
            report.push_str("  ✅ Duration within acceptable range\n");
        } else {
            report.push_str("  ⚠️  Duration exceeds acceptable range\n");
        }

        report.push_str("\n");

        report
    }

    /// Generate detailed validation report in text format
    pub fn generate_detailed_text_report(&self, report: &CheckpointValidationReport) -> String {
        let mut output = String::new();

        output.push_str("V2 WAL Checkpoint Validation Report\n");
        output.push_str("===================================\n\n");

        output.push_str(&format!("Checkpoint File: {}\n", report.checkpoint_path));
        output.push_str(&format!(
            "Report Generated: {:?}\n",
            report.report_timestamp
        ));
        output.push_str(&format!(
            "Validation Status: {:?}\n\n",
            report.validation_status
        ));

        // Validation Summary
        output.push_str("Validation Summary:\n");
        output.push_str(&format!(
            "  Total Violations: {}\n",
            report.summary.total_violations
        ));
        output.push_str(&format!(
            "  Critical Violations: {}\n",
            report.summary.critical_violations
        ));
        output.push_str(&format!(
            "  Error Violations: {}\n",
            report.summary.error_violations
        ));
        output.push_str(&format!(
            "  Warning Violations: {}\n",
            report.summary.warning_violations
        ));
        output.push_str(&format!(
            "  Info Violations: {}\n",
            report.summary.info_violations
        ));
        output.push_str(&format!(
            "  Validation Score: {:.2}/1.00\n",
            report.summary.validation_score
        ));
        output.push_str(&format!(
            "  Validation Duration: {} ms\n\n",
            report.summary.validation_duration_ms
        ));

        // Consistency Results
        if let Some(consistency) = &report.consistency_result {
            output.push_str("Consistency Validation:\n");
            output.push_str(&format!(
                "  Consistency Status: {}\n",
                if consistency.is_consistent {
                    "PASS"
                } else {
                    "FAIL"
                }
            ));
            output.push_str(&format!("  LSN Range: {:?}\n", consistency.lsn_range));
            output.push_str(&format!(
                "  Validation Time: {:?}\n",
                consistency.validation_timestamp
            ));

            if !consistency.violations.is_empty() {
                output.push_str("  Violations:\n");
                for (i, violation) in consistency.violations.iter().enumerate() {
                    output.push_str(&format!(
                        "    {}. [{:?}] {}\n",
                        i + 1,
                        violation.severity,
                        violation.description
                    ));
                }
            }
            output.push_str("\n");
        }

        // V2 Invariant Results
        if let Some(invariants) = &report.v2_invariant_result {
            output.push_str("V2 Invariant Validation:\n");
            output.push_str(&format!(
                "  Invariant Status: {}\n",
                if invariants.invariants_held {
                    "PASS"
                } else {
                    "FAIL"
                }
            ));
            output.push_str(&format!("  V2 Version: {:?}\n", invariants.v2_version));
            output.push_str(&format!(
                "  Validation Time: {:?}\n",
                invariants.validation_timestamp
            ));

            if !invariants.violations.is_empty() {
                output.push_str("  Violations:\n");
                for (i, violation) in invariants.violations.iter().enumerate() {
                    output.push_str(&format!(
                        "    {}. [{}] {}\n",
                        i + 1,
                        if violation.critical {
                            "CRITICAL"
                        } else {
                            "ERROR"
                        },
                        violation.description
                    ));
                }
            }
            output.push_str("\n");
        }

        // Performance Metrics
        if let Some(metrics) = &report.performance_metrics {
            output.push_str("Performance Metrics:\n");
            output.push_str(&format!(
                "  Total Checkpoints: {}\n",
                metrics.total_checkpoints
            ));
            output.push_str(&format!(
                "  Average Duration: {} ms\n",
                metrics.avg_checkpoint_duration_ms
            ));
            output.push_str(&format!(
                "  Throughput: {:.2} MB/s\n",
                metrics.checkpoint_throughput_mbps
            ));
            output.push_str(&format!(
                "  Average Blocks: {}\n",
                metrics.avg_blocks_per_checkpoint
            ));
            output.push_str(&format!(
                "  Average Records: {}\n",
                metrics.avg_records_per_checkpoint
            ));
            output.push_str("\n");
        }

        // Recommendations
        output.push_str("Recommendations:\n");
        output.push_str(&self.generate_recommendations(report));

        output
    }

    /// Generate recommendations based on validation results
    fn generate_recommendations(&self, report: &CheckpointValidationReport) -> String {
        let mut recommendations = Vec::new();

        // Critical violations
        if report.summary.critical_violations > 0 {
            recommendations.push("IMMEDIATE ACTION REQUIRED: Critical violations detected. Address before proceeding.");
        }

        // Error violations
        if report.summary.error_violations > 0 {
            recommendations.push("Address error violations to ensure checkpoint integrity.");
        }

        // Performance issues
        if let Some(metrics) = &report.performance_metrics {
            if metrics.checkpoint_throughput_mbps < performance::TARGET_CHECKPOINT_THROUGHPUT_MBPS {
                recommendations
                    .push("Consider optimizing checkpoint configuration for better throughput.");
            }

            if metrics.avg_checkpoint_duration_ms > performance::MAX_CHECKPOINT_DURATION_MS {
                recommendations.push("Checkpoint duration exceeds target. Consider more frequent smaller checkpoints.");
            }

            if metrics.anomaly_summary.anomaly_percentage > 10.0 {
                recommendations
                    .push("High anomaly percentage detected. Investigate system performance.");
            }
        }

        // Low validation score
        if report.summary.validation_score < 0.8 {
            recommendations
                .push("Validation score indicates multiple issues. Review and fix violations.");
        }

        if recommendations.is_empty() {
            recommendations.push("No specific recommendations. Validation passed successfully.");
        }

        recommendations
            .iter()
            .enumerate()
            .map(|(i, rec)| format!("  {}. {}\n", i + 1, rec))
            .collect()
    }
}

/// Validation report utilities
pub struct ValidationReportUtils;

impl ValidationReportUtils {
    /// Export validation report to JSON
    pub fn export_to_json(report: &CheckpointValidationReport) -> CheckpointResult<String> {
        use serde_json;

        serde_json::to_string_pretty(report).map_err(|e| {
            CheckpointError::validation(format!("Failed to serialize report to JSON: {}", e))
        })
    }

    /// Calculate trend analysis from multiple reports
    pub fn calculate_trend_analysis(reports: &[CheckpointValidationReport]) -> TrendAnalysis {
        if reports.is_empty() {
            return TrendAnalysis::default();
        }

        let mut analysis = TrendAnalysis::default();

        // Calculate validation score trend
        let scores: Vec<f64> = reports.iter().map(|r| r.summary.validation_score).collect();

        analysis.validation_score_trend = if scores.len() >= 2 {
            let recent_avg = scores[scores.len().saturating_sub(5)..].iter().sum::<f64>()
                / scores[scores.len().saturating_sub(5)..].len() as f64;
            let overall_avg = scores.iter().sum::<f64>() / scores.len() as f64;
            recent_avg - overall_avg
        } else {
            0.0
        };

        // Calculate violation count trend
        let violations: Vec<usize> = reports.iter().map(|r| r.summary.total_violations).collect();

        analysis.violation_count_trend = if violations.len() >= 2 {
            let recent_avg = violations[violations.len().saturating_sub(5)..]
                .iter()
                .sum::<usize>() as f64
                / violations[violations.len().saturating_sub(5)..].len() as f64;
            let overall_avg = violations.iter().sum::<usize>() as f64 / violations.len() as f64;
            recent_avg - overall_avg
        } else {
            0.0
        };

        // Calculate performance trend
        let throughputs: Vec<f64> = reports
            .iter()
            .filter_map(|r| r.performance_metrics.as_ref())
            .map(|p| p.checkpoint_throughput_mbps)
            .collect();

        analysis.performance_trend = if throughputs.len() >= 2 {
            let recent_avg = throughputs[throughputs.len().saturating_sub(5)..]
                .iter()
                .sum::<f64>()
                / throughputs[throughputs.len().saturating_sub(5)..].len() as f64;
            let overall_avg = throughputs.iter().sum::<f64>() / throughputs.len() as f64;
            recent_avg - overall_avg
        } else {
            0.0
        };

        analysis
    }
}

/// Trend analysis for validation reports
#[derive(Debug, Clone, Default)]
pub struct TrendAnalysis {
    /// Validation score trend (positive = improving)
    pub validation_score_trend: f64,
    /// Violation count trend (negative = improving)
    pub violation_count_trend: f64,
    /// Performance trend (positive = improving)
    pub performance_trend: f64,
}

use std::time::Duration;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::v2::wal::checkpoint::validation::{
        ConsistencyViolation, ConsistencyViolationType, V2InvariantViolation, V2InvariantViolationType
    };
        use std::time::SystemTime;
    use tempfile::tempdir;

    fn create_test_performance_metrics() -> PerformanceMetrics {
        PerformanceMetrics {
            total_checkpoints: 10,
            avg_checkpoint_duration_ms: 1000,
            checkpoint_throughput_mbps: 50.0,
            avg_blocks_per_checkpoint: 100,
            avg_records_per_checkpoint: 1000,
            anomaly_summary: AnomalySummary {
                duration_anomalies: 1,
                throughput_anomalies: 2,
                block_count_anomalies: 0,
                anomaly_percentage: 30.0,
            },
        }
    }

    #[test]
    fn test_validation_reporter_creation() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let reporter = CheckpointValidationReporter::new(config);
        assert!(true, "Validation reporter created successfully");
    }

    #[test]
    fn test_validation_report_generation() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let reporter = CheckpointValidationReporter::new(config);
        let checkpoint_path = temp_dir.path().join("test.checkpoint");

        let consistency_result = Some(ConsistencyResult {
            is_consistent: true,
            violations: vec![],
            validation_timestamp: SystemTime::now(),
            lsn_range: Some((1000, 2000)),
        });

        let v2_invariant_result = Some(V2InvariantResult {
            invariants_held: true,
            violations: vec![],
            validation_timestamp: SystemTime::now(),
            v2_version: Some(2),
        });

        let performance_metrics = Some(create_test_performance_metrics());
        let validation_duration = Some(Duration::from_millis(500));

        let report = reporter.generate_validation_report(
            &checkpoint_path,
            consistency_result,
            v2_invariant_result,
            performance_metrics,
            validation_duration,
        );

        assert_eq!(report.validation_status, ValidationStatus::Passed);
        assert_eq!(
            report.checkpoint_path,
            checkpoint_path.to_string_lossy().to_string()
        );
        assert_eq!(report.summary.total_violations, 0);
        assert_eq!(report.summary.validation_score, 1.0);
    }

    #[test]
    fn test_validation_report_with_violations() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let reporter = CheckpointValidationReporter::new(config);
        let checkpoint_path = temp_dir.path().join("test.checkpoint");

        let consistency_result = Some(ConsistencyResult {
            is_consistent: false,
            violations: vec![ConsistencyViolation {
                violation_type: ConsistencyViolationType::InvalidLsnRange,
                description: "LSN range error".to_string(),
                severity: ConsistencySeverity::Error,
                entity_id: Some("test".to_string()),
            }],
            validation_timestamp: SystemTime::now(),
            lsn_range: Some((1000, 2000)),
        });

        let v2_invariant_result = None;
        let performance_metrics = None;
        let validation_duration = Some(Duration::from_millis(100));

        let report = reporter.generate_validation_report(
            &checkpoint_path,
            consistency_result,
            v2_invariant_result,
            performance_metrics,
            validation_duration,
        );

        assert_eq!(report.validation_status, ValidationStatus::Failed);
        assert_eq!(report.summary.total_violations, 1);
        assert_eq!(report.summary.error_violations, 1);
        assert!(report.summary.validation_score < 1.0);
    }

    #[test]
    fn test_performance_report_generation() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let reporter = CheckpointValidationReporter::new(config);
        let metrics = create_test_performance_metrics();

        let report_text = reporter.generate_performance_report(&metrics);

        assert!(report_text.contains("V2 WAL Checkpoint Performance Report"));
        assert!(report_text.contains("Total Checkpoints: 10"));
        assert!(report_text.contains("Average Duration: 1000 ms"));
        assert!(report_text.contains("Performance Analysis"));
    }

    #[test]
    fn test_detailed_text_report_generation() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let reporter = CheckpointValidationReporter::new(config);

        let consistency_result = Some(ConsistencyResult {
            is_consistent: true,
            violations: vec![],
            validation_timestamp: SystemTime::now(),
            lsn_range: Some((1000, 2000)),
        });

        let v2_invariant_result = Some(V2InvariantResult {
            invariants_held: true,
            violations: vec![],
            validation_timestamp: SystemTime::now(),
            v2_version: Some(2),
        });

        let performance_metrics = Some(create_test_performance_metrics());
        let validation_duration = Some(Duration::from_millis(500));

        let report = reporter.generate_validation_report(
            &temp_dir.path().join("test.checkpoint"),
            consistency_result,
            v2_invariant_result,
            performance_metrics,
            validation_duration,
        );

        let detailed_report = reporter.generate_detailed_text_report(&report);

        assert!(detailed_report.contains("V2 WAL Checkpoint Validation Report"));
        assert!(detailed_report.contains("Validation Summary:"));
        assert!(detailed_report.contains("Consistency Validation:"));
        assert!(detailed_report.contains("V2 Invariant Validation:"));
        assert!(detailed_report.contains("Performance Metrics:"));
        assert!(detailed_report.contains("Recommendations:"));
    }

    #[test]
    fn test_validation_status_determination() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        let reporter = CheckpointValidationReporter::new(config);

        // Test passed status
        let passed_status = reporter.determine_validation_status(&None, &None);
        assert_eq!(passed_status, ValidationStatus::Passed);

        // Test warning status
        let consistency_with_warnings = Some(ConsistencyResult {
            is_consistent: true,
            violations: vec![ConsistencyViolation {
                violation_type: ConsistencyViolationType::InvalidTimestamp,
                description: "Warning".to_string(),
                severity: ConsistencySeverity::Warning,
                entity_id: None,
            }],
            validation_timestamp: SystemTime::now(),
            lsn_range: None,
        });

        let warning_status =
            reporter.determine_validation_status(&consistency_with_warnings, &None);
        assert_eq!(warning_status, ValidationStatus::PassedWithWarnings);

        // Test critical failure status
        let critical_violations = Some(V2InvariantResult {
            invariants_held: false,
            violations: vec![V2InvariantViolation {
                violation_type: V2InvariantViolationType::InvalidV2Version,
                description: "Critical".to_string(),
                expected: None,
                actual: None,
                critical: true,
            }],
            validation_timestamp: SystemTime::now(),
            v2_version: None,
        });

        let critical_status = reporter.determine_validation_status(&None, &critical_violations);
        assert_eq!(critical_status, ValidationStatus::CriticalFailure);
    }

    #[test]
    fn test_trend_analysis() {
        let reports = vec![CheckpointValidationReport {
            report_timestamp: SystemTime::now(),
            checkpoint_path: "test1.checkpoint".to_string(),
            consistency_result: None,
            v2_invariant_result: None,
            validation_status: ValidationStatus::Passed,
            performance_metrics: Some(create_test_performance_metrics()),
            summary: ValidationSummary {
                total_violations: 0,
                critical_violations: 0,
                error_violations: 0,
                warning_violations: 0,
                info_violations: 0,
                validation_score: 1.0,
                validation_duration_ms: 100,
            },
        }];

        let analysis = ValidationReportUtils::calculate_trend_analysis(&reports);
        assert_eq!(analysis.validation_score_trend, 0.0);
        assert_eq!(analysis.violation_count_trend, 0.0);
        assert_eq!(analysis.performance_trend, 0.0);
    }
}
