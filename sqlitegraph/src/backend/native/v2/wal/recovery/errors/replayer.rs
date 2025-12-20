//! V2 WAL Recovery Replayer Errors
//!
//! This module provides replayer-specific error handling for the V2 WAL recovery system,
//! including error factories for transaction replay failures, context builders for
//! replayer scenarios, and specialized replayer error types.

use super::core::{ErrorContext, RecoveryError, RecoveryErrorKind, RecoverySuggestion, ErrorSeverity};

/// Replayer-specific error context builders
pub struct ReplayerErrorContext;

impl ReplayerErrorContext {
    /// Create context for transaction replay errors
    pub fn transaction_replay(
        transaction_id: u64,
        start_lsn: u64,
        end_lsn: u64,
        operation_count: u64,
        failed_at_step: Option<u64>,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.transaction_id = Some(transaction_id);
        context.lsn_range = Some((start_lsn, end_lsn));
        context.recovery_state = Some("Transaction Replay".to_string());

        context.metadata.insert("operation_count".to_string(), operation_count.to_string());

        if let Some(step) = failed_at_step {
            context.metadata.insert("failed_at_step".to_string(), step.to_string());
            context.records_processed = Some(step);
        }

        context
    }

    /// Create context for operation replay errors
    pub fn operation_replay(
        transaction_id: u64,
        lsn: u64,
        operation_type: &str,
        operation_index: u64,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.transaction_id = Some(transaction_id);
        context.lsn_range = Some((lsn, lsn));
        context.recovery_state = Some("Operation Replay".to_string());

        context.metadata.insert("operation_type".to_string(), operation_type.to_string());
        context.metadata.insert("operation_index".to_string(), operation_index.to_string());
        context.records_processed = Some(operation_index);

        context
    }

    /// Create context for batch replay errors
    pub fn batch_replay(
        batch_size: u64,
        processed_count: u64,
        failed_count: u64,
        batch_start_lsn: u64,
        batch_end_lsn: u64,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.lsn_range = Some((batch_start_lsn, batch_end_lsn));
        context.recovery_state = Some("Batch Replay".to_string());

        context.metadata.insert("batch_size".to_string(), batch_size.to_string());
        context.metadata.insert("processed_count".to_string(), processed_count.to_string());
        context.metadata.insert("failed_count".to_string(), failed_count.to_string());
        context.records_processed = Some(processed_count);

        if processed_count > 0 {
            let progress = (processed_count as f64 / batch_size as f64) * 100.0;
            context.recovery_progress_percentage = Some(progress);
        }

        context
    }

    /// Create context for rollback errors
    pub fn rollback(
        transaction_id: u64,
        rollback_lsn: u64,
        affected_operations: u64,
        rollback_reason: &str,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.transaction_id = Some(transaction_id);
        context.lsn_range = Some((rollback_lsn, rollback_lsn));
        context.recovery_state = Some("Transaction Rollback".to_string());

        context.metadata.insert("affected_operations".to_string(), affected_operations.to_string());
        context.metadata.insert("rollback_reason".to_string(), rollback_reason.to_string());

        context
    }

    /// Create context for dependency resolution errors
    pub fn dependency_resolution(
        transaction_id: u64,
        dependent_operation: &str,
        missing_dependency: &str,
        dependency_id: u64,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.transaction_id = Some(transaction_id);
        context.recovery_state = Some("Dependency Resolution".to_string());

        context.metadata.insert("dependent_operation".to_string(), dependent_operation.to_string());
        context.metadata.insert("missing_dependency".to_string(), missing_dependency.to_string());
        context.metadata.insert("dependency_id".to_string(), dependency_id.to_string());

        context
    }

    /// Create context for state consistency errors
    pub fn state_consistency(
        transaction_id: u64,
        expected_state: &str,
        actual_state: &str,
        checkpoint_lsn: Option<u64>,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.transaction_id = Some(transaction_id);
        context.recovery_state = Some("State Consistency Check".to_string());

        context.metadata.insert("expected_state".to_string(), expected_state.to_string());
        context.metadata.insert("actual_state".to_string(), actual_state.to_string());

        if let Some(lsn) = checkpoint_lsn {
            context.metadata.insert("checkpoint_lsn".to_string(), lsn.to_string());
            context.lsn_range = Some((lsn, lsn));
        }

        context
    }

    /// Create context for resource constraints errors
    pub fn resource_constraints(
        transaction_id: u64,
        resource_type: &str,
        current_usage: u64,
        max_limit: u64,
        operation_being_executed: &str,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.transaction_id = Some(transaction_id);
        context.recovery_state = Some("Resource Constraints".to_string());

        context.metadata.insert("resource_type".to_string(), resource_type.to_string());
        context.metadata.insert("current_usage".to_string(), current_usage.to_string());
        context.metadata.insert("max_limit".to_string(), max_limit.to_string());
        context.metadata.insert("operation_being_executed".to_string(), operation_being_executed.to_string());

        context
    }
}

/// Replayer-specific error factories
pub struct ReplayerErrorFactory;

impl ReplayerErrorFactory {
    /// Create transaction replay error
    pub fn transaction_replay_error(
        transaction_id: u64,
        start_lsn: u64,
        end_lsn: u64,
        message: impl Into<String>,
    ) -> RecoveryError {
        RecoveryError::new(RecoveryErrorKind::Transaction, message)
            .with_context(ReplayerErrorContext::transaction_replay(transaction_id, start_lsn, end_lsn, 0, None))
            .with_recovery(RecoverySuggestion::Restart)
            .with_severity(ErrorSeverity::Error)
    }

    /// Create operation replay error
    pub fn operation_replay_error(
        transaction_id: u64,
        lsn: u64,
        operation_type: &str,
        operation_index: u64,
        message: impl Into<String>,
    ) -> RecoveryError {
        RecoveryError::new(RecoveryErrorKind::Transaction, message)
            .with_context(ReplayerErrorContext::operation_replay(transaction_id, lsn, operation_type, operation_index))
            .with_recovery(RecoverySuggestion::Restart)
            .with_severity(ErrorSeverity::Error)
    }

    /// Create batch replay error
    pub fn batch_replay_error(
        batch_size: u64,
        processed_count: u64,
        failed_count: u64,
        start_lsn: u64,
        end_lsn: u64,
        message: impl Into<String>,
    ) -> RecoveryError {
        RecoveryError::new(RecoveryErrorKind::Transaction, message)
            .with_context(ReplayerErrorContext::batch_replay(batch_size, processed_count, failed_count, start_lsn, end_lsn))
            .with_recovery(RecoverySuggestion::Retry { max_attempts: 3, backoff_ms: 2000 })
            .with_severity(ErrorSeverity::Error)
    }

    /// Create rollback error
    pub fn rollback_error(
        transaction_id: u64,
        rollback_lsn: u64,
        affected_operations: u64,
        reason: &str,
    ) -> RecoveryError {
        let message = format!("Rollback failed for transaction {}: {} ({} operations affected)",
                             transaction_id, reason, affected_operations);

        RecoveryError::new(RecoveryErrorKind::State, message)
            .with_context(ReplayerErrorContext::rollback(transaction_id, rollback_lsn, affected_operations, reason))
            .with_recovery(RecoverySuggestion::ManualIntervention("Transaction rollback failed - may require manual cleanup".to_string()))
            .with_severity(ErrorSeverity::Critical)
    }

    /// Create dependency resolution error
    pub fn dependency_error(
        transaction_id: u64,
        operation: &str,
        missing_dependency: &str,
        dependency_id: u64,
    ) -> RecoveryError {
        let message = format!("Dependency resolution failed for transaction {}: operation '{}' depends on '{}' ({})",
                             transaction_id, operation, missing_dependency, dependency_id);

        RecoveryError::new(RecoveryErrorKind::Consistency, message)
            .with_context(ReplayerErrorContext::dependency_resolution(transaction_id, operation, missing_dependency, dependency_id))
            .with_recovery(RecoverySuggestion::ForceRecovery)
            .with_severity(ErrorSeverity::Error)
    }

    /// Create state consistency error
    pub fn state_consistency_error(
        transaction_id: u64,
        expected_state: &str,
        actual_state: &str,
        message: impl Into<String>,
    ) -> RecoveryError {
        RecoveryError::new(RecoveryErrorKind::Consistency, message)
            .with_context(ReplayerErrorContext::state_consistency(transaction_id, expected_state, actual_state, None))
            .with_recovery(RecoverySuggestion::Restart)
            .with_severity(ErrorSeverity::Critical)
    }

    /// Create resource constraint error
    pub fn resource_error(
        transaction_id: u64,
        resource_type: &str,
        current_usage: u64,
        max_limit: u64,
        operation: &str,
    ) -> RecoveryError {
        let message = format!("Resource constraint exceeded in transaction {}: {} usage {}/{} during {}",
                             transaction_id, resource_type, current_usage, max_limit, operation);

        RecoveryError::new(RecoveryErrorKind::Resource, message)
            .with_context(ReplayerErrorContext::resource_constraints(transaction_id, resource_type, current_usage, max_limit, operation))
            .with_recovery(RecoverySuggestion::CheckDiskSpace)
            .with_severity(ErrorSeverity::Critical)
    }

    /// Create replayer timeout error
    pub fn timeout_error(
        transaction_id: u64,
        operation_type: &str,
        timeout_ms: u64,
        elapsed_ms: u64,
    ) -> RecoveryError {
        let message = format!("Replayer timeout in transaction {}: {} operation timed out after {}ms (elapsed: {}ms)",
                             transaction_id, operation_type, timeout_ms, elapsed_ms);

        let mut context = ErrorContext::default();
        context.transaction_id = Some(transaction_id);
        context.recovery_state = Some("Replayer Timeout".to_string());
        context.metadata.insert("operation_type".to_string(), operation_type.to_string());
        context.metadata.insert("timeout_ms".to_string(), timeout_ms.to_string());
        context.metadata.insert("elapsed_ms".to_string(), elapsed_ms.to_string());

        RecoveryError::new(RecoveryErrorKind::Timeout, message)
            .with_context(context)
            .with_recovery(RecoverySuggestion::Retry { max_attempts: 2, backoff_ms: 5000 })
            .with_severity(ErrorSeverity::Error)
    }

    /// Create replayer initialization error
    pub fn initialization_error(
        message: impl Into<String>,
        component: &str,
        config_details: Option<std::collections::HashMap<String, String>>,
    ) -> RecoveryError {
        let mut context = ErrorContext::default();
        context.recovery_state = Some("Replayer Initialization".to_string());
        context.metadata.insert("component".to_string(), component.to_string());

        if let Some(config) = config_details {
            for (key, value) in config {
                context.metadata.insert(format!("config_{}", key), value);
            }
        }

        RecoveryError::new(RecoveryErrorKind::Configuration, message)
            .with_context(context)
            .with_recovery(RecoverySuggestion::Restart)
            .with_severity(ErrorSeverity::Critical)
    }
}

/// Extension trait for RecoveryError to provide replayer-specific methods
pub trait ReplayerErrorExt {
    /// Convert to a replayer error with transaction context
    fn as_replayer_error(self, transaction_id: u64, operation: &str) -> Self;

    /// Add replayer-specific context
    fn with_replayer_context(
        self,
        transaction_id: u64,
        start_lsn: u64,
        end_lsn: u64,
        operations_processed: u64,
    ) -> Self;

    /// Mark as recoverable replayer error
    fn as_recoverable_replayer_error(self) -> Self;

    /// Mark as critical replayer error requiring manual intervention
    fn as_critical_replayer_error(self) -> Self;

    /// Add progress context for long-running operations
    fn with_progress_context(self, progress_percentage: f64, items_processed: u64, total_items: u64) -> Self;
}

impl ReplayerErrorExt for RecoveryError {
    fn as_replayer_error(self, transaction_id: u64, operation: &str) -> Self {
        let mut context = self.context.clone();
        context.transaction_id = Some(transaction_id);
        context.recovery_state = Some(format!("Replayer: {}", operation));

        self.with_context(context)
            .with_kind(RecoveryErrorKind::Transaction)
    }

    fn with_replayer_context(
        self,
        transaction_id: u64,
        start_lsn: u64,
        end_lsn: u64,
        operations_processed: u64,
    ) -> Self {
        let mut context = self.context.clone();
        context.transaction_id = Some(transaction_id);
        context.lsn_range = Some((start_lsn, end_lsn));
        context.records_processed = Some(operations_processed);

        self.with_context(context)
    }

    fn as_recoverable_replayer_error(self) -> Self {
        self.with_severity(ErrorSeverity::Error)
            .with_recovery(RecoverySuggestion::Restart)
    }

    fn as_critical_replayer_error(self) -> Self {
        self.with_severity(ErrorSeverity::Critical)
            .with_recovery(RecoverySuggestion::ManualIntervention("Critical replayer failure - manual intervention required".to_string()))
    }

    fn with_progress_context(self, progress_percentage: f64, items_processed: u64, total_items: u64) -> Self {
        let mut context = self.context.clone();
        context.recovery_progress_percentage = Some(progress_percentage);
        context.records_processed = Some(items_processed);
        context.metadata.insert("total_items".to_string(), total_items.to_string());

        self.with_context(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::core::ErrorSeverity;

    #[test]
    fn test_replayer_error_context_transaction() {
        let context = ReplayerErrorContext::transaction_replay(123, 1000, 2000, 5, Some(3));

        assert_eq!(context.transaction_id, Some(123));
        assert_eq!(context.lsn_range, Some((1000, 2000)));
        assert_eq!(context.recovery_state, Some("Transaction Replay".to_string()));
        assert_eq!(context.records_processed, Some(3));
        assert_eq!(context.metadata.get("operation_count"), Some(&"5".to_string()));
        assert_eq!(context.metadata.get("failed_at_step"), Some(&"3".to_string()));
    }

    #[test]
    fn test_replayer_error_context_operation() {
        let context = ReplayerErrorContext::operation_replay(123, 1500, "INSERT", 2);

        assert_eq!(context.transaction_id, Some(123));
        assert_eq!(context.lsn_range, Some((1500, 1500)));
        assert_eq!(context.recovery_state, Some("Operation Replay".to_string()));
        assert_eq!(context.metadata.get("operation_type"), Some(&"INSERT".to_string()));
        assert_eq!(context.metadata.get("operation_index"), Some(&"2".to_string()));
    }

    #[test]
    fn test_replayer_error_context_batch() {
        let context = ReplayerErrorContext::batch_replay(100, 75, 5, 1000, 1100);

        assert_eq!(context.lsn_range, Some((1000, 1100)));
        assert_eq!(context.recovery_state, Some("Batch Replay".to_string()));
        assert_eq!(context.records_processed, Some(75));
        assert_eq!(context.metadata.get("batch_size"), Some(&"100".to_string()));
        assert_eq!(context.metadata.get("processed_count"), Some(&"75".to_string()));
        assert_eq!(context.metadata.get("failed_count"), Some(&"5".to_string()));
        assert_eq!(context.recovery_progress_percentage, Some(75.0));
    }

    #[test]
    fn test_replayer_error_factory_transaction() {
        let error = ReplayerErrorFactory::transaction_replay_error(123, 1000, 2000, "Test error");

        assert_eq!(error.kind, RecoveryErrorKind::Transaction);
        assert_eq!(error.context.transaction_id, Some(123));
        assert_eq!(error.context.lsn_range, Some((1000, 2000)));
        assert_eq!(error.severity(), ErrorSeverity::Error);
        assert!(matches!(error.recovery, RecoverySuggestion::Restart));
    }

    #[test]
    fn test_replayer_error_factory_operation() {
        let error = ReplayerErrorFactory::operation_replay_error(123, 1500, "UPDATE", 1, "Update failed");

        assert_eq!(error.kind, RecoveryErrorKind::Transaction);
        assert_eq!(error.context.metadata.get("operation_type"), Some(&"UPDATE".to_string()));
        assert_eq!(error.context.metadata.get("operation_index"), Some(&"1".to_string()));
    }

    #[test]
    fn test_replayer_error_factory_rollback() {
        let error = ReplayerErrorFactory::rollback_error(123, 2000, 3, "Constraint violation");

        assert_eq!(error.kind, RecoveryErrorKind::State);
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        assert!(matches!(error.recovery, RecoverySuggestion::ManualIntervention(_)));
        assert!(error.message.contains("123"));
        assert!(error.message.contains("3"));
    }

    #[test]
    fn test_replayer_error_factory_dependency() {
        let error = ReplayerErrorFactory::dependency_error(123, "operation", "dependency", 456);

        assert_eq!(error.kind, RecoveryErrorKind::Consistency);
        assert_eq!(error.context.metadata.get("dependent_operation"), Some(&"operation".to_string()));
        assert_eq!(error.context.metadata.get("missing_dependency"), Some(&"dependency".to_string()));
        assert_eq!(error.context.metadata.get("dependency_id"), Some(&"456".to_string()));
    }

    #[test]
    fn test_replayer_error_factory_resource() {
        let error = ReplayerErrorFactory::resource_error(123, "memory", 1024, 1000, "INSERT");

        assert_eq!(error.kind, RecoveryErrorKind::Resource);
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        assert_eq!(error.context.metadata.get("resource_type"), Some(&"memory".to_string()));
        assert_eq!(error.context.metadata.get("current_usage"), Some(&"1024".to_string()));
        assert_eq!(error.context.metadata.get("max_limit"), Some(&"1000".to_string()));
    }

    #[test]
    fn test_replayer_error_factory_timeout() {
        let error = ReplayerErrorFactory::timeout_error(123, "SCAN", 5000, 6000);

        assert_eq!(error.kind, RecoveryErrorKind::Timeout);
        assert_eq!(error.context.metadata.get("operation_type"), Some(&"SCAN".to_string()));
        assert_eq!(error.context.metadata.get("timeout_ms"), Some(&"5000".to_string()));
        assert_eq!(error.context.metadata.get("elapsed_ms"), Some(&"6000".to_string()));
    }

    #[test]
    fn test_replayer_error_extension() {
        let base_error = RecoveryError::new(RecoveryErrorKind::Io, "Test error");

        let replayer_error = base_error
            .as_replayer_error(123, "REPLAY")
            .with_replayer_context(123, 1000, 2000, 5)
            .with_progress_context(50.0, 5, 10);

        assert_eq!(replayer_error.context.transaction_id, Some(123));
        assert_eq!(replayer_error.context.recovery_state, Some("Replayer: REPLAY".to_string()));
        assert_eq!(replayer_error.context.lsn_range, Some((1000, 2000)));
        assert_eq!(replayer_error.context.records_processed, Some(5));
        assert_eq!(replayer_error.context.recovery_progress_percentage, Some(50.0));
        assert_eq!(replayer_error.context.metadata.get("total_items"), Some(&"10".to_string()));
    }

    #[test]
    fn test_replayer_error_recovery_levels() {
        let base_error = RecoveryError::new(RecoveryErrorKind::Transaction, "Test error");

        let recoverable = base_error.clone().as_recoverable_replayer_error();
        let critical = base_error.as_critical_replayer_error();

        assert_eq!(recoverable.severity(), ErrorSeverity::Error);
        assert!(matches!(recoverable.recovery, RecoverySuggestion::Restart));

        assert_eq!(critical.severity(), ErrorSeverity::Critical);
        assert!(matches!(critical.recovery, RecoverySuggestion::ManualIntervention(_)));
    }

    #[test]
    fn test_replayer_error_factory_initialization() {
        let mut config = std::collections::HashMap::new();
        config.insert("max_workers".to_string(), "4".to_string());
        config.insert("buffer_size".to_string(), "1024".to_string());

        let error = ReplayerErrorFactory::initialization_error("Failed to start", "ThreadPool", Some(config));

        assert_eq!(error.kind, RecoveryErrorKind::Configuration);
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        assert_eq!(error.context.metadata.get("component"), Some(&"ThreadPool".to_string()));
        assert_eq!(error.context.metadata.get("config_max_workers"), Some(&"4".to_string()));
        assert_eq!(error.context.metadata.get("config_buffer_size"), Some(&"1024".to_string()));
    }
}