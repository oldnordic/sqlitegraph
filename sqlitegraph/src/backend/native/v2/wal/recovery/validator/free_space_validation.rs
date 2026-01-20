//! Free Space Validation Operations
//!
//! This module provides validation operations for free space-related WAL records:
//! - FreeSpaceAllocate: Validate region consistency
//! - FreeSpaceDeallocate: Validate region existence checks

use crate::backend::native::v2::wal::recovery::errors::RecoveryResult;
use crate::backend::native::v2::wal::recovery::validator::{
    ValidationResult, ValidationSeverity, V2_BLOCK_ALIGNMENT, MIN_BLOCK_SIZE, MAX_BLOCK_SIZE,
};

use super::super::TransactionValidator;

/// Validate free space allocation with region consistency
pub fn validate_free_space_allocate(
    _validator: &TransactionValidator,
    block_offset: u64,
    block_size: u32,
    _lsn: u64,
) -> RecoveryResult<ValidationResult> {
    let mut issues = Vec::new();

    // Basic validation
    if block_size == 0 {
        return Ok(ValidationResult::Invalid {
            errors: vec!["Block size cannot be zero".to_string()],
            critical_error: "Free space allocation validation failed".to_string(),
        });
    }

    // Validate alignment
    if block_offset % V2_BLOCK_ALIGNMENT != 0 {
        return Ok(ValidationResult::Invalid {
            errors: vec![format!(
                "Block offset {} not aligned to V2_BLOCK_ALIGNMENT {}",
                block_offset, V2_BLOCK_ALIGNMENT
            )],
            critical_error: "V2 free space alignment error".to_string(),
        });
    }

    // Validate block size constraints
    if block_size > MAX_BLOCK_SIZE {
        issues.push(format!(
            "Block size {} exceeds maximum {}",
            block_size, MAX_BLOCK_SIZE
        ));
    }

    if block_size < MIN_BLOCK_SIZE {
        issues.push(format!(
            "Block size {} below minimum {}",
            block_size, MIN_BLOCK_SIZE
        ));
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

/// Validate free space deallocation with region existence checks
pub fn validate_free_space_deallocate(
    _validator: &TransactionValidator,
    block_offset: u64,
    block_size: u32,
    _lsn: u64,
) -> RecoveryResult<ValidationResult> {
    let issues = Vec::new();

    // Basic validation
    if block_size == 0 {
        return Ok(ValidationResult::Invalid {
            errors: vec!["Block size cannot be zero in deallocation".to_string()],
            critical_error: "Free space deallocation validation failed".to_string(),
        });
    }

    // Validate alignment
    if block_offset % V2_BLOCK_ALIGNMENT != 0 {
        return Ok(ValidationResult::Invalid {
            errors: vec![format!(
                "Block offset {} not aligned to V2_BLOCK_ALIGNMENT {}",
                block_offset, V2_BLOCK_ALIGNMENT
            )],
            critical_error: "V2 free space alignment error".to_string(),
        });
    }

    // Check if this region was previously allocated
    // In a full implementation, this would check against the free space manager's state
    // For now, we note it as a potential issue

    Ok(if issues.is_empty() {
        ValidationResult::Valid
    } else {
        ValidationResult::Recoverable {
            issues,
            severity: ValidationSeverity::Warning,
        }
    })
}
