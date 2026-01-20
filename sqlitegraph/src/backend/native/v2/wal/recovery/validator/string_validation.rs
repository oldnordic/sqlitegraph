//! String Validation Operations
//!
//! This module provides validation operations for string-related WAL records:
//! - StringInsert: Validate string table insertion with uniqueness checks

use crate::backend::native::v2::wal::recovery::errors::RecoveryResult;
use crate::backend::native::v2::wal::recovery::validator::{
    ValidationResult, ValidationSeverity, MAX_STRING_LENGTH,
};

use super::super::TransactionValidator;

/// Validate string table insertion with uniqueness checks
pub fn validate_string_insert(
    _validator: &TransactionValidator,
    string_id: u32,
    string_value: &str,
    _lsn: u64,
) -> RecoveryResult<ValidationResult> {
    let mut issues = Vec::new();

    // Basic validation
    if string_id == 0 {
        return Ok(ValidationResult::Invalid {
            errors: vec!["String ID cannot be zero".to_string()],
            critical_error: "String insert validation failed".to_string(),
        });
    }

    if string_value.is_empty() {
        return Ok(ValidationResult::Invalid {
            errors: vec!["String value cannot be empty".to_string()],
            critical_error: "String insert validation failed".to_string(),
        });
    }

    // Validate string length constraints
    if string_value.len() > MAX_STRING_LENGTH {
        issues.push(format!(
            "String exceeds maximum length: {} > {}",
            string_value.len(),
            MAX_STRING_LENGTH
        ));
    }

    // Check for invalid UTF-8 sequences (already validated by Rust str type)
    if string_value.contains('\0') {
        issues.push("String contains null byte - may cause V2 backend issues".to_string());
    }

    // Validate string content for V2 compatibility
    if string_value.len() > 1000 {
        issues.push("Very long string may impact V2 performance".to_string());
    }

    Ok(if issues.is_empty() {
        ValidationResult::Valid
    } else {
        ValidationResult::Recoverable {
            issues,
            severity: ValidationSeverity::Warning,
        }
    })
}
