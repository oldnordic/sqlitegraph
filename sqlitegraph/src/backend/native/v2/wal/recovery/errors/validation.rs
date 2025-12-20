//! V2 WAL Recovery Validation Errors
//!
//! This module provides validation-specific error handling for the V2 WAL recovery system,
//! including error factories for validation failures, context builders for validation scenarios,
//! and specialized validation error types.

use super::core::{ErrorContext, RecoveryError, RecoveryErrorKind, RecoverySuggestion, ErrorSeverity};

/// Validation-specific error context builders
pub struct ValidationErrorContext;

impl ValidationErrorContext {
    /// Create context for V2 format validation errors
    pub fn v2_format(
        lsn_range: Option<(u64, u64)>,
        expected_format: &str,
        actual_format: Option<&str>,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.lsn_range = lsn_range;
        context.recovery_state = Some("V2 Format Validation".to_string());

        if let Some(format) = actual_format {
            context.metadata.insert("expected_format".to_string(), expected_format.to_string());
            context.metadata.insert("actual_format".to_string(), format.to_string());
        } else {
            context.metadata.insert("expected_format".to_string(), expected_format.to_string());
            context.metadata.insert("actual_format".to_string(), "None".to_string());
        }

        context
    }

    /// Create context for checksum validation errors
    pub fn checksum_validation(
        lsn: u64,
        expected_checksum: u64,
        actual_checksum: u64,
        algorithm: &str,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.lsn_range = Some((lsn, lsn));
        context.recovery_state = Some("Checksum Validation".to_string());

        context.metadata.insert("lsn".to_string(), lsn.to_string());
        context.metadata.insert("expected_checksum".to_string(), format!("{:x}", expected_checksum));
        context.metadata.insert("actual_checksum".to_string(), format!("{:x}", actual_checksum));
        context.metadata.insert("checksum_algorithm".to_string(), algorithm.to_string());

        context
    }

    /// Create context for consistency validation errors
    pub fn consistency_validation(
        node_count_mismatch: Option<(u64, u64)>,
        edge_count_mismatch: Option<(u64, u64)>,
        cluster_inconsistency: Option<(u64, u64)>,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.recovery_state = Some("Consistency Validation".to_string());

        if let Some((expected, actual)) = node_count_mismatch {
            context.metadata.insert("expected_node_count".to_string(), expected.to_string());
            context.metadata.insert("actual_node_count".to_string(), actual.to_string());
        }

        if let Some((expected, actual)) = edge_count_mismatch {
            context.metadata.insert("expected_edge_count".to_string(), expected.to_string());
            context.metadata.insert("actual_edge_count".to_string(), actual.to_string());
        }

        if let Some((cluster_id, error_type)) = cluster_inconsistency {
            context.metadata.insert("cluster_id".to_string(), cluster_id.to_string());
            context.metadata.insert("cluster_error_type".to_string(), error_type.to_string());
        }

        context
    }

    /// Create context for structural validation errors
    pub fn structural_validation(
        record_type: &str,
        field_name: &str,
        expected_type: &str,
        actual_value: Option<&str>,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.recovery_state = Some("Structural Validation".to_string());

        context.metadata.insert("record_type".to_string(), record_type.to_string());
        context.metadata.insert("field_name".to_string(), field_name.to_string());
        context.metadata.insert("expected_type".to_string(), expected_type.to_string());

        if let Some(value) = actual_value {
            context.metadata.insert("actual_value".to_string(), value.to_string());
        } else {
            context.metadata.insert("actual_value".to_string(), "None".to_string());
        }

        context
    }

    /// Create context for range validation errors
    pub fn range_validation(
        field_name: &str,
        value: u64,
        min_allowed: u64,
        max_allowed: u64,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.recovery_state = Some("Range Validation".to_string());

        context.metadata.insert("field_name".to_string(), field_name.to_string());
        context.metadata.insert("value".to_string(), value.to_string());
        context.metadata.insert("min_allowed".to_string(), min_allowed.to_string());
        context.metadata.insert("max_allowed".to_string(), max_allowed.to_string());

        context
    }

    /// Create context for dependency validation errors
    pub fn dependency_validation(
        dependent_type: &str,
        dependency_type: &str,
        dependency_id: u64,
        missing_reference: bool,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.recovery_state = Some("Dependency Validation".to_string());

        context.metadata.insert("dependent_type".to_string(), dependent_type.to_string());
        context.metadata.insert("dependency_type".to_string(), dependency_type.to_string());
        context.metadata.insert("dependency_id".to_string(), dependency_id.to_string());
        context.metadata.insert("missing_reference".to_string(), missing_reference.to_string());

        context
    }
}

/// Validation-specific error factories
pub struct ValidationErrorFactory;

impl ValidationErrorFactory {
    /// Create V2 format validation error
    pub fn v2_format_error(
        message: impl Into<String>,
        expected_format: &str,
        actual_format: Option<&str>,
    ) -> RecoveryError {
        RecoveryError::new(RecoveryErrorKind::Validation, message)
            .with_context(ValidationErrorContext::v2_format(None, expected_format, actual_format))
            .with_recovery(RecoverySuggestion::ValidateWalFile)
            .with_severity(ErrorSeverity::Error)
    }

    /// Create checksum validation error
    pub fn checksum_error(
        lsn: u64,
        expected_checksum: u64,
        actual_checksum: u64,
        algorithm: &str,
    ) -> RecoveryError {
        let message = format!(
            "Checksum validation failed at LSN {}: expected {:x}, got {:x} ({})",
            lsn, expected_checksum, actual_checksum, algorithm
        );

        RecoveryError::new(RecoveryErrorKind::Validation, message)
            .with_context(ValidationErrorContext::checksum_validation(lsn, expected_checksum, actual_checksum, algorithm))
            .with_recovery(RecoverySuggestion::ForceRecovery)
            .with_severity(ErrorSeverity::Error)
    }

    /// Create consistency validation error
    pub fn consistency_error(
        message: impl Into<String>,
        node_count_mismatch: Option<(u64, u64)>,
        edge_count_mismatch: Option<(u64, u64)>,
    ) -> RecoveryError {
        RecoveryError::new(RecoveryErrorKind::Consistency, message)
            .with_context(ValidationErrorContext::consistency_validation(node_count_mismatch, edge_count_mismatch, None))
            .with_recovery(RecoverySuggestion::ForceRecovery)
            .with_severity(ErrorSeverity::Critical)
    }

    /// Create structural validation error
    pub fn structural_error(
        message: impl Into<String>,
        record_type: &str,
        field_name: &str,
        expected_type: &str,
    ) -> RecoveryError {
        RecoveryError::new(RecoveryErrorKind::Validation, message)
            .with_context(ValidationErrorContext::structural_validation(record_type, field_name, expected_type, None))
            .with_recovery(RecoverySuggestion::ForceRecovery)
            .with_severity(ErrorSeverity::Error)
    }

    /// Create range validation error
    pub fn range_error(
        message: impl Into<String>,
        field_name: &str,
        value: u64,
        min_allowed: u64,
        max_allowed: u64,
    ) -> RecoveryError {
        RecoveryError::new(RecoveryErrorKind::Validation, message)
            .with_context(ValidationErrorContext::range_validation(field_name, value, min_allowed, max_allowed))
            .with_recovery(RecoverySuggestion::ForceRecovery)
            .with_severity(ErrorSeverity::Error)
    }

    /// Create dependency validation error
    pub fn dependency_error(
        message: impl Into<String>,
        dependent_type: &str,
        dependency_type: &str,
        dependency_id: u64,
    ) -> RecoveryError {
        RecoveryError::new(RecoveryErrorKind::Consistency, message)
            .with_context(ValidationErrorContext::dependency_validation(dependent_type, dependency_type, dependency_id, true))
            .with_recovery(RecoverySuggestion::ForceRecovery)
            .with_severity(ErrorSeverity::Error)
    }

    /// Create integrity validation error
    pub fn integrity_error(
        message: impl Into<String>,
        lsn_range: Option<(u64, u64)>,
        affected_records: u64,
    ) -> RecoveryError {
        let mut context = ValidationErrorContext::v2_format(lsn_range, "V2 Integrity", Some("Corrupted"));
        context.metadata.insert("affected_records".to_string(), affected_records.to_string());

        RecoveryError::new(RecoveryErrorKind::Corruption, message)
            .with_context(context)
            .with_recovery(RecoverySuggestion::RestoreFromBackup)
            .with_severity(ErrorSeverity::Critical)
    }

    /// Create schema validation error
    pub fn schema_error(
        message: impl Into<String>,
        expected_version: &str,
        actual_version: &str,
    ) -> RecoveryError {
        let mut context = ValidationErrorContext::v2_format(None, expected_version, Some(actual_version));
        context.recovery_state = Some("Schema Validation".to_string());

        RecoveryError::new(RecoveryErrorKind::V2Integration, message)
            .with_context(context)
            .with_recovery(RecoverySuggestion::ValidateWalFile)
            .with_severity(ErrorSeverity::Critical)
    }
}

/// Extension trait for RecoveryError to provide validation-specific methods
pub trait ValidationErrorExt {
    /// Convert to a validation error with context
    fn as_validation_error(self, validation_stage: &str) -> Self;

    /// Add validation-specific context
    fn with_validation_context(
        self,
        stage: &str,
        record_count: Option<u64>,
        failure_count: Option<u64>,
    ) -> Self;

    /// Mark as recoverable validation error
    fn as_recoverable_validation(self) -> Self;

    /// Mark as critical validation error
    fn as_critical_validation(self) -> Self;
}

impl ValidationErrorExt for RecoveryError {
    fn as_validation_error(self, validation_stage: &str) -> Self {
        let mut context = self.context.clone();
        context.recovery_state = Some(format!("Validation: {}", validation_stage));

        self.with_context(context)
            .with_kind(RecoveryErrorKind::Validation)
    }

    fn with_validation_context(
        self,
        stage: &str,
        record_count: Option<u64>,
        failure_count: Option<u64>,
    ) -> Self {
        let mut context = self.context.clone();
        context.recovery_state = Some(format!("Validation: {}", stage));

        if let Some(count) = record_count {
            context.metadata.insert("records_validated".to_string(), count.to_string());
        }

        if let Some(count) = failure_count {
            context.metadata.insert("validation_failures".to_string(), count.to_string());
        }

        self.with_context(context)
    }

    fn as_recoverable_validation(self) -> Self {
        self.with_severity(ErrorSeverity::Error)
            .with_recovery(RecoverySuggestion::ForceRecovery)
    }

    fn as_critical_validation(self) -> Self {
        self.with_severity(ErrorSeverity::Critical)
            .with_recovery(RecoverySuggestion::RestoreFromBackup)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::core::ErrorSeverity;

    #[test]
    fn test_validation_error_context_v2_format() {
        let context = ValidationErrorContext::v2_format(
            Some((1000, 2000)),
            "V2.0",
            Some("V1.0")
        );

        assert_eq!(context.lsn_range, Some((1000, 2000)));
        assert_eq!(context.recovery_state, Some("V2 Format Validation".to_string()));
        assert_eq!(context.metadata.get("expected_format"), Some(&"V2.0".to_string()));
        assert_eq!(context.metadata.get("actual_format"), Some(&"V1.0".to_string()));
    }

    #[test]
    fn test_validation_error_context_checksum() {
        let context = ValidationErrorContext::checksum_validation(1234, 0xABCD, 0xDCBA, "CRC32");

        assert_eq!(context.lsn_range, Some((1234, 1234)));
        assert_eq!(context.recovery_state, Some("Checksum Validation".to_string()));
        assert_eq!(context.metadata.get("lsn"), Some(&"1234".to_string()));
        assert_eq!(context.metadata.get("expected_checksum"), Some(&"abcd".to_string()));
        assert_eq!(context.metadata.get("actual_checksum"), Some(&"dcba".to_string()));
        assert_eq!(context.metadata.get("checksum_algorithm"), Some(&"CRC32".to_string()));
    }

    #[test]
    fn test_validation_error_factory_checksum() {
        let error = ValidationErrorFactory::checksum_error(1234, 0xABCD, 0xDCBA, "CRC32");

        assert_eq!(error.kind, RecoveryErrorKind::Validation);
        assert!(error.message.contains("1234"));
        assert!(error.message.contains("abcd"));
        assert!(error.message.contains("dcba"));
        assert!(matches!(error.recovery, RecoverySuggestion::ForceRecovery));
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_validation_error_factory_consistency() {
        let error = ValidationErrorFactory::consistency_error(
            "Node count mismatch",
            Some((100, 90)),
            Some((200, 180))
        );

        assert_eq!(error.kind, RecoveryErrorKind::Consistency);
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        assert_eq!(error.context.metadata.get("expected_node_count"), Some(&"100".to_string()));
        assert_eq!(error.context.metadata.get("actual_node_count"), Some(&"90".to_string()));
        assert_eq!(error.context.metadata.get("expected_edge_count"), Some(&"200".to_string()));
        assert_eq!(error.context.metadata.get("actual_edge_count"), Some(&"180".to_string()));
    }

    #[test]
    fn test_validation_error_factory_structural() {
        let error = ValidationErrorFactory::structural_error(
            "Invalid field type",
            "NodeRecord",
            "node_id",
            "u64"
        );

        assert_eq!(error.kind, RecoveryErrorKind::Validation);
        assert_eq!(error.context.metadata.get("record_type"), Some(&"NodeRecord".to_string()));
        assert_eq!(error.context.metadata.get("field_name"), Some(&"node_id".to_string()));
        assert_eq!(error.context.metadata.get("expected_type"), Some(&"u64".to_string()));
    }

    #[test]
    fn test_validation_error_extension() {
        let base_error = RecoveryError::new(RecoveryErrorKind::Io, "Test error");

        let validation_error = base_error
            .as_validation_error("Format Check")
            .with_validation_context("V2", Some(100), Some(5));

        assert_eq!(validation_error.context.recovery_state, Some("Validation: V2".to_string()));
        assert_eq!(validation_error.context.metadata.get("records_validated"), Some(&"100".to_string()));
        assert_eq!(validation_error.context.metadata.get("validation_failures"), Some(&"5".to_string()));
    }

    #[test]
    fn test_validation_error_recovery_levels() {
        let base_error = RecoveryError::new(RecoveryErrorKind::Validation, "Test error");

        let recoverable = base_error.clone().as_recoverable_validation();
        let critical = base_error.as_critical_validation();

        assert_eq!(recoverable.severity(), ErrorSeverity::Error);
        assert!(matches!(recoverable.recovery, RecoverySuggestion::ForceRecovery));

        assert_eq!(critical.severity(), ErrorSeverity::Critical);
        assert!(matches!(critical.recovery, RecoverySuggestion::RestoreFromBackup));
    }

    #[test]
    fn test_validation_error_factory_schema() {
        let error = ValidationErrorFactory::schema_error(
            "Version mismatch",
            "2.0",
            "1.0"
        );

        assert_eq!(error.kind, RecoveryErrorKind::V2Integration);
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        assert_eq!(error.context.recovery_state, Some("Schema Validation".to_string()));
        assert_eq!(error.context.metadata.get("expected_format"), Some(&"2.0".to_string()));
        assert_eq!(error.context.metadata.get("actual_format"), Some(&"1.0".to_string()));
    }
}