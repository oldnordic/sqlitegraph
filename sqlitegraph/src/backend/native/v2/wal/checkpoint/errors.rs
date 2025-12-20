//! V2 WAL Checkpoint Error Handling
//!
//! This module provides comprehensive error handling and diagnostics for the checkpoint system,
//! including detailed error types, recovery strategies, and diagnostic information.

use crate::backend::native::NativeBackendError;
use std::fmt;
use std::io;

/// Enhanced checkpoint error with detailed diagnostic information
#[derive(Debug, Clone)]
pub struct CheckpointError {
    /// Error kind for categorization
    pub kind: CheckpointErrorKind,

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
}

/// Categorized checkpoint error types
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointErrorKind {
    /// Configuration-related errors
    Configuration,

    /// File system I/O errors
    Io,

    /// V2 graph integration errors
    V2Integration,

    /// Checkpoint strategy errors
    Strategy,

    /// State management errors
    State,

    /// Validation errors
    Validation,

    /// Resource exhaustion errors
    Resource,

    /// Timeout errors
    Timeout,

    /// Concurrency errors
    Concurrency,

    /// Data corruption errors
    Corruption,

    /// Unknown/unexpected errors
    Unknown,
}

/// Error context information for debugging
#[derive(Debug, Clone, Default)]
pub struct ErrorContext {
    /// Checkpoint LSN range when error occurred
    pub lsn_range: Option<(u64, u64)>,

    /// Number of records processed when error occurred
    pub records_processed: Option<u64>,

    /// Number of dirty blocks when error occurred
    pub dirty_blocks: Option<u64>,

    /// Checkpoint file path
    pub checkpoint_path: Option<String>,

    /// WAL file path
    pub wal_path: Option<String>,

    /// Operation being performed
    pub operation: Option<String>,

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

    /// Restart checkpoint process
    Restart,

    /// Check disk space and permissions
    CheckDiskSpace,

    /// Validate V2 graph file integrity
    ValidateV2File,

    /// Reduce checkpoint batch size
    ReduceBatchSize,

    /// Increase timeout values
    IncreaseTimeout,

    /// Manual intervention required
    ManualIntervention(String),

    /// Custom recovery message
    Custom(String),
}

impl CheckpointError {
    /// Create a new checkpoint error
    pub fn new(kind: CheckpointErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
            context: ErrorContext::default(),
            recovery: RecoverySuggestion::None,
            timestamp: std::time::SystemTime::now(),
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

    /// Add source error information
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Create configuration error
    pub fn configuration(message: impl Into<String>) -> Self {
        Self::new(CheckpointErrorKind::Configuration, message)
            .with_recovery(RecoverySuggestion::CheckDiskSpace)
    }

    /// Create I/O error
    pub fn io(message: impl Into<String>) -> Self {
        Self::new(CheckpointErrorKind::Io, message)
            .with_recovery(RecoverySuggestion::Retry { max_attempts: 3, backoff_ms: 100 })
    }

    /// Create V2 integration error
    pub fn v2_integration(message: impl Into<String>) -> Self {
        Self::new(CheckpointErrorKind::V2Integration, message)
            .with_recovery(RecoverySuggestion::ValidateV2File)
    }

    /// Create strategy error
    pub fn strategy(message: impl Into<String>) -> Self {
        Self::new(CheckpointErrorKind::Strategy, message)
            .with_recovery(RecoverySuggestion::Custom("Review checkpoint strategy configuration".to_string()))
    }

    /// Create state error
    pub fn state(message: impl Into<String>) -> Self {
        Self::new(CheckpointErrorKind::State, message)
            .with_recovery(RecoverySuggestion::Restart)
    }

    /// Create validation error
    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(CheckpointErrorKind::Validation, message)
            .with_recovery(RecoverySuggestion::ValidateV2File)
    }

    /// Create resource error
    pub fn resource(message: impl Into<String>) -> Self {
        Self::new(CheckpointErrorKind::Resource, message)
            .with_recovery(RecoverySuggestion::ReduceBatchSize)
    }

    /// Create timeout error
    pub fn timeout(message: impl Into<String>) -> Self {
        Self::new(CheckpointErrorKind::Timeout, message)
            .with_recovery(RecoverySuggestion::IncreaseTimeout)
    }

    /// Create corruption error
    pub fn corruption(message: impl Into<String>) -> Self {
        Self::new(CheckpointErrorKind::Corruption, message)
            .with_recovery(RecoverySuggestion::ManualIntervention("Data corruption detected".to_string()))
    }

    /// Create unknown error
    pub fn unknown(message: impl Into<String>) -> Self {
        Self::new(CheckpointErrorKind::Unknown, message)
    }

    /// Get error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self.kind {
            CheckpointErrorKind::Configuration | CheckpointErrorKind::Strategy => ErrorSeverity::Warning,
            CheckpointErrorKind::Io | CheckpointErrorKind::Resource | CheckpointErrorKind::Timeout => ErrorSeverity::Error,
            CheckpointErrorKind::State | CheckpointErrorKind::Validation => ErrorSeverity::Error,
            CheckpointErrorKind::V2Integration | CheckpointErrorKind::Corruption => ErrorSeverity::Critical,
            CheckpointErrorKind::Concurrency | CheckpointErrorKind::Unknown => ErrorSeverity::Error,
        }
    }

    /// Check if error is recoverable
    pub fn is_recoverable(&self) -> bool {
        !matches!(self.kind, CheckpointErrorKind::Corruption)
    }

    /// Get suggested retry delay in milliseconds
    pub fn retry_delay_ms(&self) -> Option<u64> {
        match &self.recovery {
            RecoverySuggestion::Retry { backoff_ms, .. } => Some(*backoff_ms),
            RecoverySuggestion::IncreaseTimeout => Some(5000), // 5 seconds
            _ => None,
        }
    }

    /// Generate diagnostic report
    pub fn diagnostic_report(&self) -> String {
        let mut report = format!("Checkpoint Error Report\n");
        report.push_str(&format!("=====================\n"));
        report.push_str(&format!("Error Kind: {:?}\n", self.kind));
        report.push_str(&format!("Severity: {:?}\n", self.severity()));
        report.push_str(&format!("Message: {}\n", self.message));

        if let Some(source) = &self.source {
            report.push_str(&format!("Source: {}\n", source));
        }

        report.push_str(&format!("Timestamp: {:?}\n", self.timestamp));
        report.push_str(&format!("Recoverable: {}\n", self.is_recoverable()));

        if let Some(lsn_range) = self.context.lsn_range {
            report.push_str(&format!("LSN Range: {}-{}\n", lsn_range.0, lsn_range.1));
        }

        if let Some(records) = self.context.records_processed {
            report.push_str(&format!("Records Processed: {}\n", records));
        }

        if let Some(blocks) = self.context.dirty_blocks {
            report.push_str(&format!("Dirty Blocks: {}\n", blocks));
        }

        if let Some(operation) = &self.context.operation {
            report.push_str(&format!("Operation: {}\n", operation));
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
}

impl fmt::Display for CheckpointError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{:?}] {}", self.kind, self.message)
    }
}

impl std::error::Error for CheckpointError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None // We store source as String for serialization compatibility
    }
}

/// Error severity levels
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum ErrorSeverity {
    Warning,
    Error,
    Critical,
}

/// Result type for checkpoint operations
pub type CheckpointResult<T> = Result<T, CheckpointError>;

/// Convert from NativeBackendError
impl From<NativeBackendError> for CheckpointError {
    fn from(error: NativeBackendError) -> Self {
        let message = error.to_string();

        match error {
            NativeBackendError::Io(_) => {
                Self::io(&message).with_source(format!("NativeBackendError: {}", message))
            }
            NativeBackendError::InvalidHeader { field, reason, .. } => {
                Self::configuration(message).with_source(format!("NativeBackendError: Invalid header {}: {}", field, reason))
            }
            NativeBackendError::CorruptNodeRecord { node_id, reason, .. } => {
                Self::corruption(message).with_source(format!("NativeBackendError: Corrupt node {}: {}", node_id, reason))
            }
            NativeBackendError::CorruptEdgeRecord { edge_id, reason, .. } => {
                Self::corruption(message).with_source(format!("NativeBackendError: Corrupt edge {}: {}", edge_id, reason))
            }
            NativeBackendError::InvalidMagic { expected, found, .. } => {
                Self::corruption(message).with_source(format!("NativeBackendError: Invalid magic expected {:x}, found {:x}", expected, found))
            }
            NativeBackendError::ValidationFailed { metric, expected, actual, .. } => {
                Self::validation(message).with_source(format!("NativeBackendError: Validation failed for {}: expected {}, found {}", metric, expected, actual))
            }
            NativeBackendError::InvalidParameter { context, .. } => {
                Self::configuration(message).with_source(format!("NativeBackendError: {}", context))
            }
            NativeBackendError::InvalidState { context, .. } => {
                Self::state(message).with_source(format!("NativeBackendError: {}", context))
            }
            NativeBackendError::CorruptionDetected { context, .. } => {
                Self::corruption(message).with_source(format!("NativeBackendError: {}", context))
            }
            _ => {
                Self::unknown(message).with_source("NativeBackendError".to_string())
            }
        }
    }
}

/// Convert from std::io::Error
impl From<io::Error> for CheckpointError {
    fn from(error: io::Error) -> Self {
        let message = error.to_string();
        let kind = error.kind();

        let mut checkpoint_error = match kind {
            io::ErrorKind::NotFound => Self::io(format!("File not found: {}", message)),
            io::ErrorKind::PermissionDenied => Self::io(format!("Permission denied: {}", message)),
            io::ErrorKind::AlreadyExists => Self::io(format!("File already exists: {}", message)),
            io::ErrorKind::InvalidInput => Self::configuration(format!("Invalid input: {}", message)),
            io::ErrorKind::InvalidData => Self::corruption(format!("Invalid data: {}", message)),
            io::ErrorKind::TimedOut => Self::timeout(format!("Operation timed out: {}", message)),
            io::ErrorKind::WriteZero => Self::io(format!("Write zero bytes: {}", message)),
            io::ErrorKind::Interrupted => Self::io(format!("Operation interrupted: {}", message)),
            io::ErrorKind::UnexpectedEof => Self::corruption(format!("Unexpected EOF: {}", message)),
            _ => Self::io(format!("I/O error: {}", message)),
        };

        checkpoint_error.source = Some(format!("io::Error kind: {:?}", kind));
        checkpoint_error
    }
}

/// Create unknown error from any error type
impl From<Box<dyn std::error::Error + Send + Sync>> for CheckpointError {
    fn from(error: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::unknown(error.to_string()).with_source("Unknown error source".to_string())
    }
}

/// Error collection for multiple checkpoint errors
#[derive(Debug, Clone)]
pub struct CheckpointErrorCollection {
    /// Collection of errors
    pub errors: Vec<CheckpointError>,

    /// Collection timestamp
    pub timestamp: std::time::SystemTime,
}

impl CheckpointErrorCollection {
    /// Create new error collection
    pub fn new() -> Self {
        Self {
            errors: Vec::new(),
            timestamp: std::time::SystemTime::now(),
        }
    }

    /// Add error to collection
    pub fn add_error(&mut self, error: CheckpointError) {
        self.errors.push(error);
    }

    /// Add multiple errors to collection
    pub fn add_errors<I>(&mut self, errors: I)
    where
        I: IntoIterator<Item = CheckpointError>,
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

    /// Generate summary report
    pub fn summary_report(&self) -> String {
        if !self.has_errors() {
            return "No checkpoint errors".to_string();
        }

        let (warning, error, critical) = self.count_by_severity();
        let total = self.errors.len();

        format!(
            "Checkpoint Error Summary\n========================\n\
             Total Errors: {}\n\
             Warnings: {}\n\
             Errors: {}\n\
             Critical: {}\n\
             Recoverable: {}\n\
             Timestamp: {:?}",
            total,
            warning,
            error,
            critical,
            total - self.errors.iter().filter(|e| !e.is_recoverable()).count(),
            self.timestamp
        )
    }

    /// Generate detailed report
    pub fn detailed_report(&self) -> String {
        if !self.has_errors() {
            return "No checkpoint errors".to_string();
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
}

impl Default for CheckpointErrorCollection {
    fn default() -> Self {
        Self::new()
    }
}

/// Macro for creating checkpoint errors with context
#[macro_export]
macro_rules! checkpoint_error {
    ($kind:expr, $message:expr) => {
        $crate::backend::native::v2::wal::checkpoint::errors::CheckpointError::new($kind, $message)
    };
    ($kind:expr, $message:expr, context: $context:expr) => {
        $crate::backend::native::v2::wal::checkpoint::errors::CheckpointError::new($kind, $message)
            .with_context($context)
    };
    ($kind:expr, $message:expr, context: $context:expr, recovery: $recovery:expr) => {
        $crate::backend::native::v2::wal::checkpoint::errors::CheckpointError::new($kind, $message)
            .with_context($context)
            .with_recovery($recovery)
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    #[test]
    fn test_checkpoint_error_creation() {
        let error = CheckpointError::configuration("Invalid path");
        assert_eq!(error.kind, CheckpointErrorKind::Configuration);
        assert_eq!(error.message, "Invalid path");
        assert_eq!(error.severity(), ErrorSeverity::Warning);
        assert!(error.is_recoverable());
    }

    #[test]
    fn test_checkpoint_error_with_context() {
        let context = ErrorContext {
            lsn_range: Some((1000, 2000)),
            records_processed: Some(500),
            ..Default::default()
        };

        let error = CheckpointError::io("File write failed")
            .with_context(context);

        assert_eq!(error.context.lsn_range, Some((1000, 2000)));
        assert_eq!(error.context.records_processed, Some(500));
    }

    #[test]
    fn test_checkpoint_error_recovery() {
        let error = CheckpointError::timeout("Operation timed out")
            .with_recovery(RecoverySuggestion::IncreaseTimeout);

        assert!(matches!(error.recovery, RecoverySuggestion::IncreaseTimeout));
        assert_eq!(error.retry_delay_ms(), Some(5000));
    }

    #[test]
    fn test_checkpoint_error_from_native() {
        let native_error = NativeBackendError::Io(
            std::io::Error::new(std::io::ErrorKind::StorageFull, "test")
        );

        let checkpoint_error: CheckpointError = native_error.into();
        assert_eq!(checkpoint_error.kind, CheckpointErrorKind::Io);
        assert!(checkpoint_error.message.contains("I/O error"));
    }

    #[test]
    fn test_error_collection() {
        let mut collection = CheckpointErrorCollection::new();

        collection.add_error(CheckpointError::configuration("Bad config"));
        collection.add_error(CheckpointError::io("Disk error"));
        collection.add_error(CheckpointError::corruption("Data corrupted"));

        assert!(collection.has_errors());
        assert!(collection.has_unrecoverable_errors());
        assert_eq!(collection.errors.len(), 3);

        let (warning, error, critical) = collection.count_by_severity();
        assert_eq!(warning, 1);
        assert_eq!(error, 1);
        assert_eq!(critical, 1);
    }

    #[test]
    fn test_error_severity_levels() {
        let config_error = CheckpointError::configuration("test");
        let io_error = CheckpointError::io("test");
        let corruption_error = CheckpointError::corruption("test");

        assert_eq!(config_error.severity(), ErrorSeverity::Warning);
        assert_eq!(io_error.severity(), ErrorSeverity::Error);
        assert_eq!(corruption_error.severity(), ErrorSeverity::Critical);

        assert!(config_error.severity() < io_error.severity());
        assert!(io_error.severity() < corruption_error.severity());
    }

    #[test]
    fn test_diagnostic_report() {
        let context = ErrorContext {
            lsn_range: Some((1000, 2000)),
            records_processed: Some(500),
            operation: Some("test_operation".to_string()),
            ..Default::default()
        };

        let error = CheckpointError::io("Test error")
            .with_context(context)
            .with_recovery(RecoverySuggestion::Retry { max_attempts: 3, backoff_ms: 100 });

        let report = error.diagnostic_report();
        assert!(report.contains("Test error"));
        assert!(report.contains("1000-2000"));
        assert!(report.contains("Records Processed: 500"));
        assert!(report.contains("test_operation"));
    }

    #[test]
    fn test_macro_usage() {
        let error = checkpoint_error!(
            CheckpointErrorKind::Io,
            "Test error",
            context: ErrorContext {
                operation: Some("test".to_string()),
                ..Default::default()
            },
            recovery: RecoverySuggestion::Retry { max_attempts: 1, backoff_ms: 50 }
        );

        assert_eq!(error.kind, CheckpointErrorKind::Io);
        assert_eq!(error.message, "Test error");
        assert_eq!(error.context.operation, Some("test".to_string()));
    }
}