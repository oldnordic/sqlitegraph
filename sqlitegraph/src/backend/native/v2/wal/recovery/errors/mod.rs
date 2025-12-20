//! V2 WAL Recovery Error Module
//!
//! This module provides modularized error handling for the V2 WAL recovery system.
//! It maintains full backward compatibility while organizing error types by responsibility.
//!
//! ## Module Structure
//!
//! - **core**: Core error types and foundational structures
//! - **validation**: Validation-specific error handling
//! - **replayer**: Transaction replay error handling
//! - **scanner**: WAL scanning and I/O error handling
//!
//! ## Backward Compatibility
//!
//! All original exports are re-exported from this module to ensure existing code continues to work.

// Core error types - foundational structures and main error definitions
pub mod core;

// Specialized error modules
pub mod validation;
pub mod replayer;
pub mod scanner;

// Re-export all core types for backward compatibility
pub use core::{
    RecoveryError, RecoveryErrorKind, RecoveryResult, RecoveryErrorCollection,
    RecoveryAction, RecoverySuggestion, ErrorSeverity, ErrorContext,
};

// Re-export validation-specific types and traits
pub use validation::{
    ValidationErrorContext, ValidationErrorFactory, ValidationErrorExt,
};

// Re-export replayer-specific types and traits
pub use replayer::{
    ReplayerErrorContext, ReplayerErrorFactory, ReplayerErrorExt,
};

// Re-export scanner-specific types and traits
pub use scanner::{
    ScannerErrorContext, ScannerErrorFactory, ScannerErrorExt,
};

// RecoveryError convenience methods - these were in the original file
// and need to be available for backward compatibility
impl RecoveryError {
    /// Create configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::Configuration, message)
            .with_recovery(RecoverySuggestion::CheckDiskSpace)
    }

    /// Create I/O error
    pub fn io(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::Io, message)
            .with_recovery(RecoverySuggestion::Retry { max_attempts: 3, backoff_ms: 1000 })
    }

    /// Create V2 integration error
    pub fn v2_integration(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::V2Integration, message)
            .with_recovery(RecoverySuggestion::ValidateWalFile)
            .with_severity(ErrorSeverity::Critical)
    }

    /// Create WAL file error
    pub fn wal_file(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::WalFile, message)
            .with_recovery(RecoverySuggestion::ValidateWalFile)
    }

    /// Create transaction error
    pub fn transaction(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::Transaction, message)
            .with_recovery(RecoverySuggestion::Restart)
    }

    /// Create validation error
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::Validation, message)
            .with_recovery(RecoverySuggestion::ForceRecovery)
    }

    /// Create state error
    pub fn state(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::State, message)
            .with_recovery(RecoverySuggestion::Restart)
    }

    /// Create resource error
    pub fn resource(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::Resource, message)
            .with_recovery(RecoverySuggestion::CheckDiskSpace)
            .with_severity(ErrorSeverity::Critical)
    }

    /// Create timeout error
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::Timeout, message)
            .with_recovery(RecoverySuggestion::Retry { max_attempts: 2, backoff_ms: 5000 })
    }

    /// Create corruption error
    pub fn corruption(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::Corruption, message)
            .with_recovery(RecoverySuggestion::RestoreFromBackup)
            .with_severity(ErrorSeverity::Critical)
    }

    /// Create consistency error
    pub fn consistency(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::Consistency, message)
            .with_recovery(RecoverySuggestion::ForceRecovery)
    }
}

// Macro for creating recovery errors with context - preserved for backward compatibility
#[macro_export]
macro_rules! recovery_error {
    ($kind:expr, $message:expr) => {
        $crate::backend::native::v2::wal::recovery::errors::RecoveryError::new($kind, $message)
    };
    ($kind:expr, $message:expr, context: $context:expr) => {
        $crate::backend::native::v2::wal::recovery::errors::RecoveryError::new($kind, $message)
            .with_context($context)
    };
    ($kind:expr, $message:expr, context: $context:expr, recovery: $recovery:expr) => {
        $crate::backend::native::v2::wal::recovery::errors::RecoveryError::new($kind, $message)
            .with_context($context)
            .with_recovery($recovery)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn test_reexports_work() {
        // Test that all re-exports are available
        let _error: RecoveryError = RecoveryError::io("test");
        let _kind: RecoveryErrorKind = RecoveryErrorKind::Io;
        let _severity: ErrorSeverity = ErrorSeverity::Error;
        let _context: ErrorContext = ErrorContext::default();
        let _suggestion: RecoverySuggestion = RecoverySuggestion::None;
        let _result: RecoveryResult<()> = Ok(());

        // Test specialized types are re-exported
        let _validation_context = ValidationErrorContext;
        let _replayer_context = ReplayerErrorContext;
        let _scanner_context = ScannerErrorContext;
    }

    #[test]
    fn test_convenience_methods() {
        let config_error = RecoveryError::configuration("test config");
        assert_eq!(config_error.kind, RecoveryErrorKind::Configuration);
        assert!(matches!(config_error.recovery, RecoverySuggestion::CheckDiskSpace));

        let io_error = RecoveryError::io("test io");
        assert_eq!(io_error.kind, RecoveryErrorKind::Io);
        if let RecoverySuggestion::Retry { max_attempts, backoff_ms } = io_error.recovery {
            assert_eq!(max_attempts, 3);
            assert_eq!(backoff_ms, 1000);
        } else {
            panic!("Expected Retry suggestion");
        }

        let v2_error = RecoveryError::v2_integration("test v2");
        assert_eq!(v2_error.kind, RecoveryErrorKind::V2Integration);
        assert_eq!(v2_error.severity(), ErrorSeverity::Critical);

        let wal_error = RecoveryError::wal_file("test wal");
        assert_eq!(wal_error.kind, RecoveryErrorKind::WalFile);
        assert!(matches!(wal_error.recovery, RecoverySuggestion::ValidateWalFile));

        let tx_error = RecoveryError::transaction("test tx");
        assert_eq!(tx_error.kind, RecoveryErrorKind::Transaction);
        assert!(matches!(tx_error.recovery, RecoverySuggestion::Restart));

        let validation_error = RecoveryError::validation("test validation");
        assert_eq!(validation_error.kind, RecoveryErrorKind::Validation);
        assert!(matches!(validation_error.recovery, RecoverySuggestion::ForceRecovery));

        let state_error = RecoveryError::state("test state");
        assert_eq!(state_error.kind, RecoveryErrorKind::State);
        assert!(matches!(state_error.recovery, RecoverySuggestion::Restart));

        let resource_error = RecoveryError::resource("test resource");
        assert_eq!(resource_error.kind, RecoveryErrorKind::Resource);
        assert_eq!(resource_error.severity(), ErrorSeverity::Critical);
        assert!(matches!(resource_error.recovery, RecoverySuggestion::CheckDiskSpace));

        let timeout_error = RecoveryError::timeout("test timeout");
        assert_eq!(timeout_error.kind, RecoveryErrorKind::Timeout);
        if let RecoverySuggestion::Retry { max_attempts, backoff_ms } = timeout_error.recovery {
            assert_eq!(max_attempts, 2);
            assert_eq!(backoff_ms, 5000);
        } else {
            panic!("Expected Retry suggestion");
        }

        let corruption_error = RecoveryError::corruption("test corruption");
        assert_eq!(corruption_error.kind, RecoveryErrorKind::Corruption);
        assert_eq!(corruption_error.severity(), ErrorSeverity::Critical);
        assert!(matches!(corruption_error.recovery, RecoverySuggestion::RestoreFromBackup));

        let consistency_error = RecoveryError::consistency("test consistency");
        assert_eq!(consistency_error.kind, RecoveryErrorKind::Consistency);
        assert!(matches!(consistency_error.recovery, RecoverySuggestion::ForceRecovery));
    }

    #[test]
    fn test_macro_usage() {
        // Test basic macro usage
        let error1 = recovery_error!(
            RecoveryErrorKind::Io,
            "Test error"
        );
        assert_eq!(error1.kind, RecoveryErrorKind::Io);
        assert_eq!(error1.message, "Test error");

        // Test macro with context
        let context = ErrorContext {
            recovery_state: Some("test".to_string()),
            ..Default::default()
        };
        let error2 = recovery_error!(
            RecoveryErrorKind::Validation,
            "Test error",
            context: context
        );
        assert_eq!(error2.kind, RecoveryErrorKind::Validation);
        assert_eq!(error2.context.recovery_state, Some("test".to_string()));

        // Test macro with context and recovery
        let error3 = recovery_error!(
            RecoveryErrorKind::Timeout,
            "Test error",
            context: ErrorContext::default(),
            recovery: RecoverySuggestion::Retry { max_attempts: 1, backoff_ms: 50 }
        );
        assert_eq!(error3.kind, RecoveryErrorKind::Timeout);
        assert!(matches!(error3.recovery, RecoverySuggestion::Retry { max_attempts: 1, backoff_ms: 50 }));
    }

    #[test]
    fn test_specialized_error_contexts() {
        // Test validation context
        let validation_context = ValidationErrorContext::checksum_validation(
            1234, 0xABCD, 0xDCBA, "CRC32"
        );
        assert_eq!(validation_context.recovery_state, Some("Checksum Validation".to_string()));
        assert_eq!(validation_context.metadata.get("lsn"), Some(&"1234".to_string()));

        // Test replayer context
        let replayer_context = ReplayerErrorContext::transaction_replay(
            567, 2000, 3000, 10, Some(5)
        );
        assert_eq!(replayer_context.transaction_id, Some(567));
        assert_eq!(replayer_context.recovery_state, Some("Transaction Replay".to_string()));

        // Test scanner context
        let scanner_context = ScannerErrorContext::wal_file_read(
            "/test/wal.db", 1024, 512, Some(256)
        );
        assert_eq!(scanner_context.wal_path, Some("/test/wal.db".to_string()));
        assert_eq!(scanner_context.recovery_state, Some("WAL File Reading".to_string()));
    }

    #[test]
    fn test_specialized_error_factories() {
        // Test validation error factory
        let validation_error = ValidationErrorFactory::checksum_error(
            1234, 0xABCD, 0xDCBA, "CRC32"
        );
        assert_eq!(validation_error.kind, RecoveryErrorKind::Validation);
        assert!(validation_error.message.contains("1234"));

        // Test replayer error factory
        let replayer_error = ReplayerErrorFactory::transaction_replay_error(
            567, 2000, 3000, "Transaction failed"
        );
        assert_eq!(replayer_error.kind, RecoveryErrorKind::Transaction);
        assert_eq!(replayer_error.context.transaction_id, Some(567));

        // Test scanner error factory
        let scanner_error = ScannerErrorFactory::wal_read_error(
            "/test/wal.db", 1024, 512, Some(256), "Read failed"
        );
        assert_eq!(scanner_error.kind, RecoveryErrorKind::Io);
        assert_eq!(scanner_error.context.wal_path, Some("/test/wal.db".to_string()));
    }

    #[test]
    fn test_extension_traits() {
        let base_error = RecoveryError::new(RecoveryErrorKind::Io, "Test error");

        // Test validation extension
        let validation_error = base_error.clone().as_validation_error("Format Check");
        assert_eq!(validation_error.context.recovery_state, Some("Validation: Format Check".to_string()));

        // Test replayer extension
        let replayer_error = base_error.clone().as_replayer_error(123, "REPLAY");
        assert_eq!(replayer_error.context.transaction_id, Some(123));
        assert_eq!(replayer_error.context.recovery_state, Some("Replayer: REPLAY".to_string()));

        // Test scanner extension
        let scanner_error = base_error.as_scanner_error("/test/wal.db", "SCAN");
        assert_eq!(scanner_error.context.wal_path, Some("/test/wal.db".to_string()));
        assert_eq!(scanner_error.context.recovery_state, Some("Scanner: SCAN".to_string()));
    }

    #[test]
    fn test_error_collection() {
        let mut collection = RecoveryErrorCollection::new();

        collection.add_error(RecoveryError::configuration("Bad config"));
        collection.add_error(RecoveryError::io("Disk error"));
        collection.add_error(RecoveryError::corruption("Data corrupted"));

        assert!(collection.has_errors());
        assert_eq!(collection.errors.len(), 3);
        assert!(collection.has_unrecoverable_errors());

        let (warning, error, critical) = collection.count_by_severity();
        assert_eq!(warning, 0);  // none are warnings
        assert_eq!(error, 2);    // configuration and io are errors by default
        assert_eq!(critical, 1);  // corruption is critical

        let action = collection.recommended_action();
        assert_eq!(action, RecoveryAction::ManualIntervention);
    }

    #[test]
    fn test_backward_compatibility_all_types_available() {
        // This test ensures that all types that were available in the original
        // monolithic errors.rs file are still available through re-exports

        // Core types
        let _: RecoveryError = RecoveryError::new(RecoveryErrorKind::Unknown, "test");
        let _: RecoveryErrorKind = RecoveryErrorKind::Unknown;
        let _: ErrorSeverity = ErrorSeverity::Error;
        let _: ErrorContext = ErrorContext::default();
        let _: RecoverySuggestion = RecoverySuggestion::None;
        let _: RecoveryResult<()> = Ok(());

        // Collection types
        let _: RecoveryErrorCollection = RecoveryErrorCollection::new();
        let _: RecoveryAction = RecoveryAction::Continue;

        // Extension traits
        fn _use_validation_ext<E: ValidationErrorExt>(_: E) {}
        fn _use_replayer_ext<E: ReplayerErrorExt>(_: E) {}
        fn _use_scanner_ext<E: ScannerErrorExt>(_: E) {}

        let error = RecoveryError::io("test");
        _use_validation_ext(error.clone());
        _use_replayer_ext(error.clone());
        _use_scanner_ext(error.clone());
    }
}