//! V2 WAL Recovery Scanner Errors
//!
//! This module provides scanner-specific error handling for the V2 WAL recovery system,
//! including error factories for WAL scanning failures, I/O error handling, and
//! specialized scanner error types for different scanning scenarios.

use super::core::{ErrorContext, RecoveryError, RecoveryErrorKind, RecoverySuggestion, ErrorSeverity};
use std::path::PathBuf;

/// Scanner-specific error context builders
pub struct ScannerErrorContext;

impl ScannerErrorContext {
    /// Create context for WAL file reading errors
    pub fn wal_file_read(
        file_path: &str,
        offset: u64,
        bytes_requested: u64,
        bytes_read: Option<u64>,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.wal_path = Some(file_path.to_string());
        context.recovery_state = Some("WAL File Reading".to_string());

        context.metadata.insert("offset".to_string(), offset.to_string());
        context.metadata.insert("bytes_requested".to_string(), bytes_requested.to_string());

        if let Some(read) = bytes_read {
            context.metadata.insert("bytes_read".to_string(), read.to_string());
        }

        context
    }

    /// Create context for WAL file parsing errors
    pub fn wal_file_parse(
        file_path: &str,
        offset: u64,
        record_type: &str,
        expected_format: &str,
        actual_data: Option<&[u8]>,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.wal_path = Some(file_path.to_string());
        context.lsn_range = Some((offset, offset));
        context.recovery_state = Some("WAL File Parsing".to_string());

        context.metadata.insert("offset".to_string(), offset.to_string());
        context.metadata.insert("record_type".to_string(), record_type.to_string());
        context.metadata.insert("expected_format".to_string(), expected_format.to_string());

        if let Some(data) = actual_data {
            context.metadata.insert("data_length".to_string(), data.len().to_string());
            if data.len() <= 16 {
                context.metadata.insert("data_preview".to_string(), format!("{:?}", data));
            }
        }

        context
    }

    /// Create context for WAL header validation errors
    pub fn wal_header_validation(
        file_path: &str,
        header_size: u64,
        expected_version: &str,
        actual_version: Option<&str>,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.wal_path = Some(file_path.to_string());
        context.recovery_state = Some("WAL Header Validation".to_string());

        context.metadata.insert("header_size".to_string(), header_size.to_string());
        context.metadata.insert("expected_version".to_string(), expected_version.to_string());

        if let Some(version) = actual_version {
            context.metadata.insert("actual_version".to_string(), version.to_string());
        }

        context
    }

    /// Create context for WAL index scanning errors
    pub fn wal_index_scan(
        file_path: &str,
        index_offset: u64,
        entry_count: u64,
        failed_at_entry: Option<u64>,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.wal_path = Some(file_path.to_string());
        context.recovery_state = Some("WAL Index Scanning".to_string());

        context.metadata.insert("index_offset".to_string(), index_offset.to_string());
        context.metadata.insert("entry_count".to_string(), entry_count.to_string());

        if let Some(entry) = failed_at_entry {
            context.metadata.insert("failed_at_entry".to_string(), entry.to_string());
            context.records_processed = Some(entry);
        }

        context
    }

    /// Create context for WAL sequence scanning errors
    pub fn wal_sequence_scan(
        file_path: &str,
        start_lsn: u64,
        end_lsn: u64,
        current_lsn: Option<u64>,
        records_scanned: u64,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.wal_path = Some(file_path.to_string());
        context.lsn_range = Some((start_lsn, end_lsn));
        context.recovery_state = Some("WAL Sequence Scanning".to_string());

        context.metadata.insert("start_lsn".to_string(), start_lsn.to_string());
        context.metadata.insert("end_lsn".to_string(), end_lsn.to_string());
        context.records_processed = Some(records_scanned);

        if let Some(lsn) = current_lsn {
            context.metadata.insert("current_lsn".to_string(), lsn.to_string());
        }

        if end_lsn > start_lsn {
            let progress = (records_scanned as f64 / (end_lsn - start_lsn) as f64) * 100.0;
            context.recovery_progress_percentage = Some(progress.min(100.0));
        }

        context
    }

    /// Create context for file system access errors
    pub fn file_system_access(
        operation: &str,
        file_path: &str,
        permissions_required: Option<&str>,
        file_size: Option<u64>,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.wal_path = Some(file_path.to_string());
        context.recovery_state = Some("File System Access".to_string());

        context.metadata.insert("operation".to_string(), operation.to_string());

        if let Some(perm) = permissions_required {
            context.metadata.insert("permissions_required".to_string(), perm.to_string());
        }

        if let Some(size) = file_size {
            context.metadata.insert("file_size".to_string(), size.to_string());
        }

        context
    }

    /// Create context for WAL format detection errors
    pub fn format_detection(
        file_path: &str,
        magic_bytes: Option<&[u8]>,
        file_size: u64,
        suspected_format: Option<&str>,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.wal_path = Some(file_path.to_string());
        context.recovery_state = Some("Format Detection".to_string());

        context.metadata.insert("file_size".to_string(), file_size.to_string());

        if let Some(bytes) = magic_bytes {
            context.metadata.insert("magic_bytes".to_string(), format!("{:?}", bytes));
        }

        if let Some(format) = suspected_format {
            context.metadata.insert("suspected_format".to_string(), format.to_string());
        }

        context
    }

    /// Create context for buffer management errors
    pub fn buffer_management(
        operation: &str,
        buffer_size: u64,
        required_size: u64,
        allocation_failed: bool,
    ) -> ErrorContext {
        let mut context = ErrorContext::default();
        context.recovery_state = Some("Buffer Management".to_string());

        context.metadata.insert("operation".to_string(), operation.to_string());
        context.metadata.insert("buffer_size".to_string(), buffer_size.to_string());
        context.metadata.insert("required_size".to_string(), required_size.to_string());
        context.metadata.insert("allocation_failed".to_string(), allocation_failed.to_string());

        context
    }
}

/// Scanner-specific error factories
pub struct ScannerErrorFactory;

impl ScannerErrorFactory {
    /// Create WAL file read error
    pub fn wal_read_error(
        file_path: &str,
        offset: u64,
        bytes_requested: u64,
        bytes_read: Option<u64>,
        message: impl Into<String>,
    ) -> RecoveryError {
        RecoveryError::new(RecoveryErrorKind::Io, message)
            .with_context(ScannerErrorContext::wal_file_read(file_path, offset, bytes_requested, bytes_read))
            .with_recovery(RecoverySuggestion::Retry { max_attempts: 3, backoff_ms: 1000 })
            .with_severity(ErrorSeverity::Error)
    }

    /// Create WAL file parse error
    pub fn wal_parse_error(
        file_path: &str,
        offset: u64,
        record_type: &str,
        expected_format: &str,
        message: impl Into<String>,
    ) -> RecoveryError {
        RecoveryError::new(RecoveryErrorKind::WalFile, message)
            .with_context(ScannerErrorContext::wal_file_parse(file_path, offset, record_type, expected_format, None))
            .with_recovery(RecoverySuggestion::ValidateWalFile)
            .with_severity(ErrorSeverity::Error)
    }

    /// Create WAL header validation error
    pub fn wal_header_error(
        file_path: &str,
        expected_version: &str,
        actual_version: Option<&str>,
    ) -> RecoveryError {
        let message = format!(
            "WAL header validation failed for {}: expected version {}, got {:?}",
            file_path, expected_version, actual_version
        );

        RecoveryError::new(RecoveryErrorKind::WalFile, message)
            .with_context(ScannerErrorContext::wal_header_validation(file_path, 0, expected_version, actual_version))
            .with_recovery(RecoverySuggestion::ValidateWalFile)
            .with_severity(ErrorSeverity::Critical)
    }

    /// Create WAL index scan error
    pub fn index_scan_error(
        file_path: &str,
        index_offset: u64,
        entry_count: u64,
        failed_at_entry: Option<u64>,
        message: impl Into<String>,
    ) -> RecoveryError {
        RecoveryError::new(RecoveryErrorKind::WalFile, message)
            .with_context(ScannerErrorContext::wal_index_scan(file_path, index_offset, entry_count, failed_at_entry))
            .with_recovery(RecoverySuggestion::ValidateWalFile)
            .with_severity(ErrorSeverity::Error)
    }

    /// Create WAL sequence scan error
    pub fn sequence_scan_error(
        file_path: &str,
        start_lsn: u64,
        end_lsn: u64,
        current_lsn: Option<u64>,
        records_scanned: u64,
        message: impl Into<String>,
    ) -> RecoveryError {
        RecoveryError::new(RecoveryErrorKind::WalFile, message)
            .with_context(ScannerErrorContext::wal_sequence_scan(file_path, start_lsn, end_lsn, current_lsn, records_scanned))
            .with_recovery(RecoverySuggestion::Restart)
            .with_severity(ErrorSeverity::Error)
    }

    /// Create file permission error
    pub fn permission_error(
        file_path: &str,
        operation: &str,
        required_permissions: &str,
    ) -> RecoveryError {
        let message = format!("Permission denied for {} on file {}: {}", operation, file_path, required_permissions);

        RecoveryError::new(RecoveryErrorKind::Io, message)
            .with_context(ScannerErrorContext::file_system_access(operation, file_path, Some(required_permissions), None))
            .with_recovery(RecoverySuggestion::CheckDiskSpace)
            .with_severity(ErrorSeverity::Error)
    }

    /// Create file not found error
    pub fn file_not_found_error(file_path: &str) -> RecoveryError {
        let message = format!("WAL file not found: {}", file_path);

        RecoveryError::new(RecoveryErrorKind::WalFile, message)
            .with_context(ScannerErrorContext::file_system_access("open", file_path, None, None))
            .with_recovery(RecoverySuggestion::CheckDiskSpace)
            .with_severity(ErrorSeverity::Critical)
    }

    /// Create disk space error
    pub fn disk_space_error(
        file_path: &str,
        operation: &str,
        required_space: u64,
        available_space: Option<u64>,
    ) -> RecoveryError {
        let message = format!(
            "Insufficient disk space for {} on {}: required {} bytes, available: {:?}",
            operation, file_path, required_space, available_space
        );

        let mut context = ScannerErrorContext::file_system_access(operation, file_path, None, None);
        context.metadata.insert("required_space".to_string(), required_space.to_string());

        if let Some(space) = available_space {
            context.metadata.insert("available_space".to_string(), space.to_string());
        }

        RecoveryError::new(RecoveryErrorKind::Resource, message)
            .with_context(context)
            .with_recovery(RecoverySuggestion::CheckDiskSpace)
            .with_severity(ErrorSeverity::Critical)
    }

    /// Create format detection error
    pub fn format_detection_error(
        file_path: &str,
        magic_bytes: Option<&[u8]>,
        suspected_format: Option<&str>,
    ) -> RecoveryError {
        let message = format!(
            "Unable to detect WAL format for file: {} (suspected: {:?})",
            file_path, suspected_format
        );

        RecoveryError::new(RecoveryErrorKind::WalFile, message)
            .with_context(ScannerErrorContext::format_detection(file_path, magic_bytes, 0, suspected_format))
            .with_recovery(RecoverySuggestion::ValidateWalFile)
            .with_severity(ErrorSeverity::Error)
    }

    /// Create buffer allocation error
    pub fn buffer_allocation_error(
        operation: &str,
        buffer_size: u64,
        required_size: u64,
    ) -> RecoveryError {
        let message = format!(
            "Buffer allocation failed for {}: required {} bytes, available buffer size: {}",
            operation, required_size, buffer_size
        );

        RecoveryError::new(RecoveryErrorKind::Resource, message)
            .with_context(ScannerErrorContext::buffer_management(operation, buffer_size, required_size, true))
            .with_recovery(RecoverySuggestion::CheckDiskSpace)
            .with_severity(ErrorSeverity::Critical)
    }

    /// Create scanner initialization error
    pub fn initialization_error(
        message: impl Into<String>,
        component: &str,
        file_path: Option<&str>,
    ) -> RecoveryError {
        let mut context = ErrorContext::default();
        context.recovery_state = Some("Scanner Initialization".to_string());
        context.metadata.insert("component".to_string(), component.to_string());

        if let Some(path) = file_path {
            context.wal_path = Some(path.to_string());
        }

        RecoveryError::new(RecoveryErrorKind::Configuration, message)
            .with_context(context)
            .with_recovery(RecoverySuggestion::Restart)
            .with_severity(ErrorSeverity::Critical)
    }

    /// Create scanner timeout error
    pub fn timeout_error(
        operation: &str,
        timeout_ms: u64,
        elapsed_ms: u64,
        file_path: &str,
    ) -> RecoveryError {
        let message = format!(
            "Scanner timeout during {} on file {}: operation timed out after {}ms (elapsed: {}ms)",
            operation, file_path, timeout_ms, elapsed_ms
        );

        let mut context = ScannerErrorContext::file_system_access(operation, file_path, None, None);
        context.metadata.insert("timeout_ms".to_string(), timeout_ms.to_string());
        context.metadata.insert("elapsed_ms".to_string(), elapsed_ms.to_string());

        RecoveryError::new(RecoveryErrorKind::Timeout, message)
            .with_context(context)
            .with_recovery(RecoverySuggestion::Retry { max_attempts: 2, backoff_ms: 3000 })
            .with_severity(ErrorSeverity::Error)
    }
}

/// Extension trait for RecoveryError to provide scanner-specific methods
pub trait ScannerErrorExt {
    /// Convert to a scanner error with file context
    fn as_scanner_error(self, file_path: &str, operation: &str) -> Self;

    /// Add scanner-specific context
    fn with_scanner_context(
        self,
        file_path: &str,
        offset: Option<u64>,
        bytes_processed: Option<u64>,
    ) -> Self;

    /// Mark as recoverable scanner error
    fn as_recoverable_scanner_error(self) -> Self;

    /// Mark as critical scanner error
    fn as_critical_scanner_error(self) -> Self;

    /// Add scan progress context
    fn with_scan_progress(self, progress_percentage: f64, items_processed: u64, total_items: u64) -> Self;

    /// Add file metadata context
    fn with_file_metadata(self, file_size: u64, modification_time: Option<std::time::SystemTime>) -> Self;
}

impl ScannerErrorExt for RecoveryError {
    fn as_scanner_error(self, file_path: &str, operation: &str) -> Self {
        let mut context = self.context.clone();
        context.wal_path = Some(file_path.to_string());
        context.recovery_state = Some(format!("Scanner: {}", operation));

        self.with_context(context)
    }

    fn with_scanner_context(
        self,
        file_path: &str,
        offset: Option<u64>,
        bytes_processed: Option<u64>,
    ) -> Self {
        let mut context = self.context.clone();
        context.wal_path = Some(file_path.to_string());

        if let Some(off) = offset {
            context.metadata.insert("offset".to_string(), off.to_string());
        }

        if let Some(bytes) = bytes_processed {
            context.records_processed = Some(bytes);
        }

        self.with_context(context)
    }

    fn as_recoverable_scanner_error(self) -> Self {
        self.with_severity(ErrorSeverity::Error)
            .with_recovery(RecoverySuggestion::Retry { max_attempts: 3, backoff_ms: 1000 })
    }

    fn as_critical_scanner_error(self) -> Self {
        self.with_severity(ErrorSeverity::Critical)
            .with_recovery(RecoverySuggestion::CheckDiskSpace)
    }

    fn with_scan_progress(self, progress_percentage: f64, items_processed: u64, total_items: u64) -> Self {
        let mut context = self.context.clone();
        context.recovery_progress_percentage = Some(progress_percentage.min(100.0));
        context.records_processed = Some(items_processed);
        context.metadata.insert("total_items".to_string(), total_items.to_string());

        self.with_context(context)
    }

    fn with_file_metadata(self, file_size: u64, modification_time: Option<std::time::SystemTime>) -> Self {
        let mut context = self.context.clone();
        context.metadata.insert("file_size".to_string(), file_size.to_string());

        if let Some(time) = modification_time {
            context.metadata.insert("modification_time".to_string(), format!("{:?}", time));
        }

        self.with_context(context)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::core::ErrorSeverity;
    use std::time::SystemTime;

    #[test]
    fn test_scanner_error_context_wal_read() {
        let context = ScannerErrorContext::wal_file_read("/test/wal.db", 1024, 512, Some(256));

        assert_eq!(context.wal_path, Some("/test/wal.db".to_string()));
        assert_eq!(context.recovery_state, Some("WAL File Reading".to_string()));
        assert_eq!(context.metadata.get("offset"), Some(&"1024".to_string()));
        assert_eq!(context.metadata.get("bytes_requested"), Some(&"512".to_string()));
        assert_eq!(context.metadata.get("bytes_read"), Some(&"256".to_string()));
    }

    #[test]
    fn test_scanner_error_context_wal_parse() {
        let context = ScannerErrorContext::wal_file_parse(
            "/test/wal.db",
            2048,
            "NodeRecord",
            "V2.0",
            Some(&[0x01, 0x02, 0x03, 0x04])
        );

        assert_eq!(context.lsn_range, Some((2048, 2048)));
        assert_eq!(context.metadata.get("offset"), Some(&"2048".to_string()));
        assert_eq!(context.metadata.get("record_type"), Some(&"NodeRecord".to_string()));
        assert_eq!(context.metadata.get("expected_format"), Some(&"V2.0".to_string()));
        assert_eq!(context.metadata.get("data_length"), Some(&"4".to_string()));
        assert_eq!(context.metadata.get("data_preview"), Some(&"[1, 2, 3, 4]".to_string()));
    }

    #[test]
    fn test_scanner_error_context_wal_sequence() {
        let context = ScannerErrorContext::wal_sequence_scan("/test/wal.db", 1000, 2000, Some(1500), 250);

        assert_eq!(context.lsn_range, Some((1000, 2000)));
        assert_eq!(context.records_processed, Some(250));
        assert_eq!(context.recovery_progress_percentage, Some(25.0));
        assert_eq!(context.metadata.get("current_lsn"), Some(&"1500".to_string()));
    }

    #[test]
    fn test_scanner_error_factory_wal_read() {
        let error = ScannerErrorFactory::wal_read_error("/test/wal.db", 1024, 512, Some(256), "Read failed");

        assert_eq!(error.kind, RecoveryErrorKind::Io);
        assert_eq!(error.context.wal_path, Some("/test/wal.db".to_string()));
        assert_eq!(error.severity(), ErrorSeverity::Error);
        assert!(matches!(error.recovery, RecoverySuggestion::Retry { max_attempts: 3, backoff_ms: 1000 }));
    }

    #[test]
    fn test_scanner_error_factory_wal_parse() {
        let error = ScannerErrorFactory::wal_parse_error("/test/wal.db", 2048, "EdgeRecord", "V2.0", "Invalid format");

        assert_eq!(error.kind, RecoveryErrorKind::WalFile);
        assert_eq!(error.context.metadata.get("record_type"), Some(&"EdgeRecord".to_string()));
        assert_eq!(error.context.metadata.get("expected_format"), Some(&"V2.0".to_string()));
    }

    #[test]
    fn test_scanner_error_factory_permission() {
        let error = ScannerErrorFactory::permission_error("/test/wal.db", "read", "r");

        assert_eq!(error.kind, RecoveryErrorKind::Io);
        assert_eq!(error.context.metadata.get("operation"), Some(&"read".to_string()));
        assert_eq!(error.context.metadata.get("permissions_required"), Some(&"r".to_string()));
        assert!(error.message.contains("Permission denied"));
    }

    #[test]
    fn test_scanner_error_factory_disk_space() {
        let error = ScannerErrorFactory::disk_space_error("/test/wal.db", "write", 1024, Some(512));

        assert_eq!(error.kind, RecoveryErrorKind::Resource);
        assert_eq!(error.context.metadata.get("required_space"), Some(&"1024".to_string()));
        assert_eq!(error.context.metadata.get("available_space"), Some(&"512".to_string()));
        assert!(error.message.contains("Insufficient disk space"));
    }

    #[test]
    fn test_scanner_error_factory_file_not_found() {
        let error = ScannerErrorFactory::file_not_found_error("/missing/wal.db");

        assert_eq!(error.kind, RecoveryErrorKind::WalFile);
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        assert_eq!(error.context.wal_path, Some("/missing/wal.db".to_string()));
        assert!(error.message.contains("not found"));
    }

    #[test]
    fn test_scanner_error_factory_buffer_allocation() {
        let error = ScannerErrorFactory::buffer_allocation_error("scan", 1024, 2048);

        assert_eq!(error.kind, RecoveryErrorKind::Resource);
        assert_eq!(error.context.metadata.get("operation"), Some(&"scan".to_string()));
        assert_eq!(error.context.metadata.get("required_size"), Some(&"2048".to_string()));
        assert_eq!(error.context.metadata.get("buffer_size"), Some(&"1024".to_string()));
    }

    #[test]
    fn test_scanner_error_extension() {
        let base_error = RecoveryError::new(RecoveryErrorKind::Io, "Test error");

        let scanner_error = base_error
            .as_scanner_error("/test/wal.db", "SCAN")
            .with_scanner_context("/test/wal.db", Some(1024), Some(512))
            .with_scan_progress(50.0, 512, 1024)
            .with_file_metadata(2048, Some(SystemTime::now()));

        assert_eq!(scanner_error.context.wal_path, Some("/test/wal.db".to_string()));
        assert_eq!(scanner_error.context.recovery_state, Some("Scanner: SCAN".to_string()));
        assert_eq!(scanner_error.context.metadata.get("offset"), Some(&"1024".to_string()));
        assert_eq!(scanner_error.context.records_processed, Some(512));
        assert_eq!(scanner_error.context.recovery_progress_percentage, Some(50.0));
        assert_eq!(scanner_error.context.metadata.get("total_items"), Some(&"1024".to_string()));
        assert_eq!(scanner_error.context.metadata.get("file_size"), Some(&"2048".to_string()));
    }

    #[test]
    fn test_scanner_error_recovery_levels() {
        let base_error = RecoveryError::new(RecoveryErrorKind::WalFile, "Test error");

        let recoverable = base_error.clone().as_recoverable_scanner_error();
        let critical = base_error.as_critical_scanner_error();

        assert_eq!(recoverable.severity(), ErrorSeverity::Error);
        assert!(matches!(recoverable.recovery, RecoverySuggestion::Retry { max_attempts: 3, backoff_ms: 1000 }));

        assert_eq!(critical.severity(), ErrorSeverity::Critical);
        assert!(matches!(critical.recovery, RecoverySuggestion::CheckDiskSpace));
    }

    #[test]
    fn test_scanner_error_factory_timeout() {
        let error = ScannerErrorFactory::timeout_error("scan", 5000, 6000, "/test/wal.db");

        assert_eq!(error.kind, RecoveryErrorKind::Timeout);
        assert_eq!(error.context.metadata.get("operation"), Some(&"scan".to_string()));
        assert_eq!(error.context.metadata.get("timeout_ms"), Some(&"5000".to_string()));
        assert_eq!(error.context.metadata.get("elapsed_ms"), Some(&"6000".to_string()));
        assert!(matches!(error.recovery, RecoverySuggestion::Retry { max_attempts: 2, backoff_ms: 3000 }));
    }

    #[test]
    fn test_scanner_error_factory_initialization() {
        let error = ScannerErrorFactory::initialization_error("Failed to init", "Scanner", Some("/test/wal.db"));

        assert_eq!(error.kind, RecoveryErrorKind::Configuration);
        assert_eq!(error.severity(), ErrorSeverity::Critical);
        assert_eq!(error.context.metadata.get("component"), Some(&"Scanner".to_string()));
        assert_eq!(error.context.wal_path, Some("/test/wal.db".to_string()));
    }
}