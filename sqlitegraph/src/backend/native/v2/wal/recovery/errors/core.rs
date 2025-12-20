//! Core V2 WAL Recovery Error Types
//!
//! This module provides the core error types and structures for the V2 WAL recovery system,
//! including the main RecoveryError type, error categorization, and foundational
//! error handling functionality.

use crate::backend::native::NativeBackendError;
use std::fmt;
use std::io;

/// Enhanced recovery error with detailed diagnostic information
#[derive(Debug, Clone)]
pub struct RecoveryError {
    /// Error kind for categorization
    pub kind: RecoveryErrorKind,

    /// Human-readable error message
    pub message: String,

    /// Source error if applicable
    pub source: Option<String>,

    /// Error context and diagnostics
    pub context: ErrorContext,

    /// Recovery suggestions
    pub recovery: RecoverySuggestion,

    /// Error timestamp
    pub timestamp: std::time::SystemTime,

    /// Error severity level
    pub severity: ErrorSeverity,
}

/// Categorized recovery error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryErrorKind {
    /// Configuration-related errors
    Configuration,

    /// File system I/O errors
    Io,

    /// V2 graph file integration errors
    V2Integration,

    /// WAL file errors
    WalFile,

    /// Transaction-related errors
    Transaction,

    /// Validation errors
    Validation,

    /// State management errors
    State,

    /// Resource exhaustion errors
    Resource,

    /// Timeout errors
    Timeout,

    /// Data corruption errors
    Corruption,

    /// Consistency errors
    Consistency,

    /// Unknown/unexpected errors
    Unknown,
}

/// Error severity levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    Warning,
    Error,
    Critical,
}

impl std::fmt::Display for ErrorSeverity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorSeverity::Warning => write!(f, "Warning"),
            ErrorSeverity::Error => write!(f, "Error"),
            ErrorSeverity::Critical => write!(f, "Critical"),
        }
    }
}

/// Error context information for debugging and diagnostics
#[derive(Debug, Clone, Default)]
pub struct ErrorContext {
    /// LSN range when error occurred
    pub lsn_range: Option<(u64, u64)>,

    /// Transaction ID if applicable
    pub transaction_id: Option<u64>,

    /// WAL file path
    pub wal_path: Option<String>,

    /// Database file path
    pub database_path: Option<String>,

    /// Recovery state when error occurred
    pub recovery_state: Option<String>,

    /// Number of processed transactions
    pub transactions_processed: Option<u64>,

    /// Number of processed records
    pub records_processed: Option<u64>,

    /// Recovery progress percentage
    pub recovery_progress_percentage: Option<f64>,

    /// Additional context key-value pairs
    pub metadata: std::collections::HashMap<String, String>,
}

/// Recovery suggestions for different error types
#[derive(Debug, Clone)]
pub enum RecoverySuggestion {
    /// No recovery needed
    None,

    /// Retry the operation
    Retry { max_attempts: u32, backoff_ms: u64 },

    /// Restart recovery process
    Restart,

    /// Check disk space and permissions
    CheckDiskSpace,

    /// Validate WAL file integrity
    ValidateWalFile,

    /// Restore from backup
    RestoreFromBackup,

    /// Force recovery with warnings
    ForceRecovery,

    /// Manual intervention required
    ManualIntervention(String),

    /// Custom recovery message
    Custom(String),
}

/// Result type for recovery operations
pub type RecoveryResult<T> = Result<T, RecoveryError>;

impl RecoveryError {
    /// Create a new recovery error
    pub fn new(kind: RecoveryErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
            context: ErrorContext::default(),
            recovery: RecoverySuggestion::None,
            timestamp: std::time::SystemTime::now(),
            severity: ErrorSeverity::Error,
        }
    }

    /// Add context information to the error
    pub fn with_context(mut self, context: ErrorContext) -> Self {
        self.context = context;
        self
    }

    /// Add recovery suggestion to the error
    pub fn with_recovery(mut self, recovery: RecoverySuggestion) -> Self {
        self.recovery = recovery;
        self
    }

    /// Set error severity
    pub fn with_severity(mut self, severity: ErrorSeverity) -> Self {
        self.severity = severity;
        self
    }

    /// Add source error information
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Set error kind (for changing error type after creation)
    pub fn with_kind(mut self, kind: RecoveryErrorKind) -> Self {
        self.kind = kind;
        self
    }

    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        self.severity.clone()
    }

    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        !matches!(self.kind, RecoveryErrorKind::Corruption | RecoveryErrorKind::V2Integration)
    }

    /// Get suggested retry delay in milliseconds
    pub fn retry_delay_ms(&self) -> Option<u64> {
        match &self.recovery {
            RecoverySuggestion::Retry { backoff_ms, .. } => Some(*backoff_ms),
            RecoverySuggestion::ForceRecovery => Some(1000),
            RecoverySuggestion::Restart => Some(5000),
            RecoverySuggestion::ValidateWalFile => Some(2000),
            _ => None,
        }
    }

    /// Check if error requires manual intervention
    pub fn requires_manual_intervention(&self) -> bool {
        matches!(&self.recovery, RecoverySuggestion::ManualIntervention(_))
            || self.severity == ErrorSeverity::Critical
    }

    /// Generate diagnostic report
    pub fn diagnostic_report(&self) -> String {
        let mut report = format!("Recovery Error Report\n");
        report.push_str("==================\n");
        report.push_str(&format!("Error Kind: {:?}\n", self.kind));
        report.push_str(&format!("Severity: {:?}\n", self.severity));
        report.push_str(&format!("Message: {}\n", self.message));

        if let Some(source) = &self.source {
            report.push_str(&format!("Source: {}\n", source));
        }

        report.push_str(&format!("Timestamp: {:?}\n", self.timestamp));
        report.push_str(&format!("Recoverable: {}\n", self.is_recoverable()));
        report.push_str(&format!("Requires Manual Intervention: {}\n", self.requires_manual_intervention()));

        if let Some((start_lsn, end_lsn)) = self.context.lsn_range {
            report.push_str(&format!("LSN Range: {}-{}\n", start_lsn, end_lsn));
        }

        if let Some(tx_id) = self.context.transaction_id {
            report.push_str(&format!("Transaction ID: {}\n", tx_id));
        }

        if let Some(progress) = self.context.recovery_progress_percentage {
            report.push_str(&format!("Recovery Progress: {:.1}%\n", progress));
        }

        if let Some(transactions) = self.context.transactions_processed {
            report.push_str(&format!("Transactions Processed: {}\n", transactions));
        }

        if let Some(records) = self.context.records_processed {
            report.push_str(&format!("Records Processed: {}\n", records));
        }

        if let Some(state) = &self.context.recovery_state {
            report.push_str(&format!("Recovery State: {}\n", state));
        }

        report.push_str(&format!("Recovery: {:?}\n", self.recovery));

        if !self.context.metadata.is_empty() {
            report.push_str("Additional Context:\n");
            for (key, value) in &self.context.metadata {
                report.push_str(&format!("  {}: {}\n", key, value));
            }
        }

        report
    }

    /// Create enhanced error with V2-specific context
    pub fn with_v2_context(
        mut self,
        transaction_id: Option<u64>,
        cluster_key: Option<(u64, u64)>,
        node_count: Option<u64>,
        edge_count: Option<u64>,
    ) -> Self {
        if let Some(tx_id) = transaction_id {
            self.context.transaction_id = Some(tx_id);
        }

        if let Some((node_a, node_b)) = cluster_key {
            self.context.metadata.insert("cluster_key".to_string(), format!("{}-{}", node_a, node_b));
        }

        if let Some(nodes) = node_count {
            self.context.metadata.insert("node_count".to_string(), nodes.to_string());
        }

        if let Some(edges) = edge_count {
            self.context.metadata.insert("edge_count".to_string(), edges.to_string());
        }

        self
    }

    /// Create error from WAL read context
    pub fn from_wal_read_error(
        error: crate::backend::native::v2::wal::WALSerializationError,
        lsn: u64,
        record_type: Option<crate::backend::native::v2::wal::V2WALRecordType>,
    ) -> Self {
        let message = format!("WAL read error at LSN {}: {}", lsn, error);
        let mut context = ErrorContext::default();
        context.lsn_range = Some((lsn, lsn));

        if let Some(record_type) = record_type {
            context.metadata.insert("record_type".to_string(), format!("{:?}", record_type));
        }

        Self::new(RecoveryErrorKind::WalFile, message)
            .with_context(context)
            .with_source(format!("WAL error: {}", error))
    }

    /// Create error from transaction context
    pub fn from_transaction_context(
        error: String,
        transaction_id: u64,
        start_lsn: u64,
        end_lsn: u64,
    ) -> Self {
        let mut context = ErrorContext::default();
        context.transaction_id = Some(transaction_id);
        context.lsn_range = Some((start_lsn, end_lsn));

        Self::new(RecoveryErrorKind::Transaction, error)
            .with_context(context)
    }

    /// Create unknown error
    pub fn unknown(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::Unknown, message)
    }

  /// Create state transition error
    pub fn state_transition(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::State, message)
            .with_recovery(RecoverySuggestion::Restart)
    }

  /// Create I/O error
    pub fn io_error(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::Io, message)
            .with_recovery(RecoverySuggestion::Retry { max_attempts: 3, backoff_ms: 100 })
    }

  /// Create replay failure error
    pub fn replay_failure(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::Transaction, message)
            .with_recovery(RecoverySuggestion::Restart)
    }

  /// Create rollback failure error
    pub fn rollback_failure(message: impl Into<String>) -> Self {
        Self::new(RecoveryErrorKind::Transaction, message)
            .with_recovery(RecoverySuggestion::RestoreFromBackup)
            .with_severity(ErrorSeverity::Critical)
    }
}

impl fmt::Display for RecoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {} ({})", self.kind, self.message, self.severity)
    }
}

impl std::error::Error for RecoveryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None // We store source as String for serialization compatibility
    }
}

/// Convert from NativeBackendError for backward compatibility
impl From<NativeBackendError> for RecoveryError {
    fn from(error: NativeBackendError) -> Self {
        let message = error.to_string();

        match error {
            NativeBackendError::Io(_) => {
                Self::new(RecoveryErrorKind::Io, &message).with_source(format!("NativeBackendError: {}", message))
            }
            NativeBackendError::InvalidHeader { field, reason, .. } => {
                Self::new(RecoveryErrorKind::Configuration, message).with_source(format!("NativeBackendError: Invalid header {}: {}", field, reason))
            }
            NativeBackendError::CorruptNodeRecord { node_id, reason, .. } => {
                Self::new(RecoveryErrorKind::Corruption, message).with_source(format!("NativeBackendError: Corrupt node {}: {}", node_id, reason))
            }
            NativeBackendError::CorruptEdgeRecord { edge_id, reason, .. } => {
                Self::new(RecoveryErrorKind::Corruption, message).with_source(format!("NativeBackendError: Corrupt edge {}: {}", edge_id, reason))
            }
            NativeBackendError::InvalidMagic { expected, found, .. } => {
                Self::new(RecoveryErrorKind::Corruption, message).with_source(format!("NativeBackendError: Invalid magic expected {:x}, found {:x}", expected, found))
            }
            NativeBackendError::ValidationFailed { metric, expected, actual, .. } => {
                Self::new(RecoveryErrorKind::Validation, message).with_source(format!("NativeBackendError: Validation failed for {}: expected {}, found {}", metric, expected, actual))
            }
            NativeBackendError::InvalidParameter { context, .. } => {
                Self::new(RecoveryErrorKind::Configuration, message).with_source(format!("NativeBackendError: {}", context))
            }
            NativeBackendError::InvalidState { context, .. } => {
                Self::new(RecoveryErrorKind::State, message).with_source(format!("NativeBackendError: {}", context))
            }
            NativeBackendError::CorruptionDetected { context, .. } => {
                Self::new(RecoveryErrorKind::Corruption, message).with_source(format!("NativeBackendError: {}", context))
            }
            _ => {
                Self::unknown(message).with_source("NativeBackendError".to_string())
            }
        }
    }
}

/// Convert from std::io::Error
impl From<io::Error> for RecoveryError {
    fn from(error: io::Error) -> Self {
        let message = error.to_string();
        let kind = error.kind();

        let mut recovery_error = match kind {
            io::ErrorKind::NotFound => Self::new(RecoveryErrorKind::WalFile, format!("File not found: {}", message)),
            io::ErrorKind::PermissionDenied => Self::new(RecoveryErrorKind::Io, format!("Permission denied: {}", message)),
            io::ErrorKind::AlreadyExists => Self::new(RecoveryErrorKind::Configuration, format!("File already exists: {}", message)),
            io::ErrorKind::InvalidInput => Self::new(RecoveryErrorKind::Configuration, format!("Invalid input: {}", message)),
            io::ErrorKind::InvalidData => Self::new(RecoveryErrorKind::Corruption, format!("Invalid data: {}", message)),
            io::ErrorKind::TimedOut => Self::new(RecoveryErrorKind::Timeout, format!("Operation timed out: {}", message)),
            io::ErrorKind::WriteZero => Self::new(RecoveryErrorKind::Io, format!("Write zero bytes: {}", message)),
            io::ErrorKind::Interrupted => Self::new(RecoveryErrorKind::Io, format!("Operation interrupted: {}", message)),
            io::ErrorKind::UnexpectedEof => Self::new(RecoveryErrorKind::Corruption, format!("Unexpected EOF: {}", message)),
            _ => Self::new(RecoveryErrorKind::Io, format!("I/O error: {}", message)),
        };

        recovery_error.source = Some(format!("io::Error kind: {:?}", kind));
        recovery_error
    }
}

/// Create unknown error from any error type
impl From<Box<dyn std::error::Error + Send + Sync>> for RecoveryError {
    fn from(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::unknown(error.to_string()).with_source("Unknown error source".to_string())
    }
}

/// Error collection for multiple recovery errors
#[derive(Debug, Clone)]
pub struct RecoveryErrorCollection {
    /// Collection of errors
    pub errors: Vec<RecoveryError>,

    /// Collection timestamp
    pub timestamp: std::time::SystemTime,

    /// Overall recovery result
    pub final_result: Option<RecoveryResult<()>>,
}

/// Recommended actions based on error collection analysis
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryAction {
    /// Continue with recovery
    Continue,

    /// Retry recovery with delay
    RetryWithDelay,

    /// Force recovery (skip validations)
    ForceRecovery,

    /// Manual intervention required
    ManualIntervention,

    /// Restore from backup
    RestoreFromBackup,

    /// Abort recovery
    Abort,
}

impl RecoveryErrorCollection {
    /// Create new error collection
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            timestamp: std::time::SystemTime::now(),
            final_result: None,
        }
    }

    /// Add error to collection
    pub fn add_error(&mut self, error: RecoveryError) {
        self.errors.push(error);
    }

    /// Add multiple errors to collection
    pub fn add_errors<I>(&mut self, errors: I)
    where
        I: IntoIterator<Item = RecoveryError>,
    {
        for error in errors {
            self.add_error(error);
        }
    }

    /// Check if collection has any errors
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get highest severity error
    pub fn highest_severity(&self) -> Option<ErrorSeverity> {
        self.errors
            .iter()
            .map(|error| error.severity())
            .max()
    }

    /// Get number of errors by severity
    pub fn count_by_severity(&self) -> (usize, usize, usize) {
        let (mut warning, mut error_count, mut critical) = (0, 0, 0);

        for error in &self.errors {
            match error.severity() {
                ErrorSeverity::Warning => warning += 1,
                ErrorSeverity::Error => error_count += 1,
                ErrorSeverity::Critical => critical += 1,
            }
        }

        (warning, error_count, critical)
    }

    /// Check if any errors are unrecoverable
    pub fn has_unrecoverable_errors(&self) -> bool {
        self.errors.iter().any(|error| !error.is_recoverable())
    }

    /// Check if any errors require manual intervention
    pub fn requires_manual_intervention(&self) -> bool {
        self.errors.iter().any(|error| error.requires_manual_intervention())
    }

    /// Generate summary report
    pub fn summary_report(&self) -> String {
        if !self.has_errors() {
            return "No recovery errors".to_string();
        }

        let (warning, error, critical) = self.count_by_severity();
        let total = self.errors.len();

        format!(
            "Recovery Error Summary\n=====================\n\
             Total Errors: {}\n\
             Warnings: {}\n\
             Errors: {}\n\
             Critical: {}\n\
             Recoverable: {}\n\
             Manual Intervention: {}\n\
             Timestamp: {:?}\n\
             Final Result: {:?}",
            total,
            warning,
            error,
            critical,
            total - self.errors.iter().filter(|e| !e.is_recoverable()).count(),
            self.errors.iter().filter(|e| e.requires_manual_intervention()).count(),
            self.timestamp,
            self.final_result
        )
    }

    /// Generate detailed report
    pub fn detailed_report(&self) -> String {
        if !self.has_errors() {
            return "No recovery errors".to_string();
        }

        let mut report = self.summary_report();
        report.push_str("\n\nDetailed Errors:\n");

        for (i, error) in self.errors.iter().enumerate() {
            report.push_str(&format!("\n{}. {}\n", i + 1, error));
            if let Some(retry_delay) = error.retry_delay_ms() {
                report.push_str(&format!("   Retry delay: {}ms\n", retry_delay));
            }
        }

        report
    }

    /// Get recommended action based on error collection
    pub fn recommended_action(&self) -> RecoveryAction {
        if !self.has_errors() {
            RecoveryAction::Continue
        } else if self.has_unrecoverable_errors() || self.highest_severity() == Some(ErrorSeverity::Critical) {
            RecoveryAction::ManualIntervention
        } else if self.requires_manual_intervention() {
            RecoveryAction::ManualIntervention
        } else if self.highest_severity() == Some(ErrorSeverity::Error) {
            RecoveryAction::RetryWithDelay
        } else {
            RecoveryAction::Continue
        }
    }
}

impl Default for RecoveryErrorCollection {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn test_recovery_error_creation() {
        let error = RecoveryError::new(RecoveryErrorKind::Configuration, "Invalid path");
        assert_eq!(error.kind, RecoveryErrorKind::Configuration);
        assert_eq!(error.message, "Invalid path");
        assert_eq!(error.severity(), ErrorSeverity::Error);
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_recovery_error_with_context() {
        let context = ErrorContext {
            lsn_range: Some((1000, 2000)),
            transaction_id: Some(123),
            recovery_state: Some("Scanning".to_string()),
            ..Default::default()
        };

        let error = RecoveryError::new(RecoveryErrorKind::Io, "File write failed")
            .with_context(context);

        assert_eq!(error.context.lsn_range, Some((1000, 2000)));
        assert_eq!(error.context.transaction_id, Some(123));
        assert_eq!(error.context.recovery_state, Some("Scanning".to_string()));
    }

    #[test]
    fn test_error_severity_levels() {
        let config_error = RecoveryError::new(RecoveryErrorKind::Configuration, "test");
        let corruption_error = RecoveryError::new(RecoveryErrorKind::Corruption, "test");

        assert_eq!(config_error.severity(), ErrorSeverity::Error);
        assert_eq!(corruption_error.severity(), ErrorSeverity::Error);

        assert!(config_error.severity() < ErrorSeverity::Critical);
    }

    #[test]
    fn test_error_collection() {
        let mut collection = RecoveryErrorCollection::new();

        collection.add_error(RecoveryError::new(RecoveryErrorKind::Configuration, "Bad config"));
        collection.add_error(RecoveryError::new(RecoveryErrorKind::Io, "Disk error"));
        collection.add_error(RecoveryError::new(RecoveryErrorKind::Corruption, "Data corrupted"));

        assert!(collection.has_errors());
        assert!(collection.has_unrecoverable_errors());
        assert_eq!(collection.errors.len(), 3);

        let (warning, error, critical) = collection.count_by_severity();
        assert_eq!(warning, 0);
        assert_eq!(error, 3);
        assert_eq!(critical, 0);
    }

    #[test]
    fn test_diagnostic_report() {
        let context = ErrorContext {
            lsn_range: Some((1000, 2000)),
            transaction_id: Some(123),
            recovery_state: Some("test_operation".to_string()),
            ..Default::default()
        };

        let error = RecoveryError::new(RecoveryErrorKind::Io, "Test error")
            .with_context(context)
            .with_recovery(RecoverySuggestion::Retry { max_attempts: 1, backoff_ms: 50 });

        let report = error.diagnostic_report();
        assert!(report.contains("Test error"));
        assert!(report.contains("1000-2000"));
        assert!(report.contains("Transaction ID: 123"));
        assert!(report.contains("test_operation"));
    }
}