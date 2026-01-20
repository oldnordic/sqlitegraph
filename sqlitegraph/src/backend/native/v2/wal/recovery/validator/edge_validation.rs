//! Edge Validation Operations
//!
//! This module provides validation operations for edge-related WAL records:
//! - EdgeInsert: Validate cluster compatibility checks
//! - EdgeUpdate: Validate compatibility checks
//! - EdgeDelete: Validate reference consistency

use crate::backend::native::v2::wal::V2WALRecord;
use crate::backend::native::v2::wal::recovery::errors::RecoveryResult;
use crate::backend::native::v2::wal::recovery::validator::{
    ValidationResult, ValidationSeverity, MAX_EDGE_RECORD_SIZE, MAX_INSERTION_POINT,
};

use super::super::TransactionValidator;

/// Validate edge insertion with cluster compatibility checks
pub fn validate_edge_insert(
    _validator: &TransactionValidator,
    record: &V2WALRecord,
    _lsn: u64,
) -> RecoveryResult<ValidationResult> {
    if let V2WALRecord::EdgeInsert {
        cluster_key,
        edge_record,
        insertion_point,
        ..
    } = record
    {
        let mut issues = Vec::new();

        // Validate cluster key
        if cluster_key.0 <= 0 {
            return Ok(ValidationResult::Invalid {
                errors: vec!["Invalid node ID in edge insert cluster key".to_string()],
                critical_error: "Edge insert validation failed".to_string(),
            });
        }

        // Validate insertion point
        if *insertion_point > MAX_INSERTION_POINT {
            issues.push(format!(
                "Insertion point {} exceeds maximum {}",
                insertion_point, MAX_INSERTION_POINT
            ));
        }

        // Validate edge record structure
        if edge_record.neighbor_id <= 0 {
            return Ok(ValidationResult::Invalid {
                errors: vec!["Invalid neighbor ID in edge record".to_string()],
                critical_error: "Edge insert validation failed".to_string(),
            });
        }

        // Validate edge record size
        if edge_record.size_bytes() > MAX_EDGE_RECORD_SIZE {
            issues.push(format!(
                "Edge record exceeds maximum size: {} > {}",
                edge_record.size_bytes(),
                MAX_EDGE_RECORD_SIZE
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
    } else {
        Ok(ValidationResult::Invalid {
            errors: vec!["Invalid record type for edge insert validation".to_string()],
            critical_error: "Validation logic error".to_string(),
        })
    }
}

/// Validate edge update with compatibility checks
pub fn validate_edge_update(
    _validator: &TransactionValidator,
    record: &V2WALRecord,
    _lsn: u64,
) -> RecoveryResult<ValidationResult> {
    if let V2WALRecord::EdgeUpdate {
        old_edge,
        new_edge,
        position,
        ..
    } = record
    {
        let mut issues = Vec::new();

        // Validate position
        if *position > MAX_INSERTION_POINT {
            issues.push(format!(
                "Update position {} exceeds maximum {}",
                position, MAX_INSERTION_POINT
            ));
        }

        // Validate edge records have same neighbor
        if old_edge.neighbor_id != new_edge.neighbor_id {
            issues.push(
                "Edge update changed neighbor ID - should use delete + insert".to_string(),
            );
        }

        // Validate size constraints
        if new_edge.size_bytes() > MAX_EDGE_RECORD_SIZE {
            issues.push(format!(
                "New edge record exceeds maximum size: {} > {}",
                new_edge.size_bytes(),
                MAX_EDGE_RECORD_SIZE
            ));
        }

        Ok(if issues.is_empty() {
            ValidationResult::Valid
        } else {
            ValidationResult::Recoverable {
                issues,
                severity: ValidationSeverity::Error,
            }
        })
    } else {
        Ok(ValidationResult::Invalid {
            errors: vec!["Invalid record type for edge update validation".to_string()],
            critical_error: "Validation logic error".to_string(),
        })
    }
}

/// Validate edge deletion with reference consistency
pub fn validate_edge_delete(
    _validator: &TransactionValidator,
    record: &V2WALRecord,
    _lsn: u64,
) -> RecoveryResult<ValidationResult> {
    if let V2WALRecord::EdgeDelete {
        old_edge, position, ..
    } = record
    {
        let mut issues = Vec::new();

        // Validate position
        if *position > MAX_INSERTION_POINT {
            issues.push(format!(
                "Delete position {} exceeds maximum {}",
                position, MAX_INSERTION_POINT
            ));
        }

        // Validate edge record structure
        if old_edge.neighbor_id <= 0 {
            return Ok(ValidationResult::Invalid {
                errors: vec!["Invalid neighbor ID in edge delete record".to_string()],
                critical_error: "Edge delete validation failed".to_string(),
            });
        }

        Ok(if issues.is_empty() {
            ValidationResult::Valid
        } else {
            ValidationResult::Recoverable {
                issues,
                severity: ValidationSeverity::Warning,
            }
        })
    } else {
        Ok(ValidationResult::Invalid {
            errors: vec!["Invalid record type for edge delete validation".to_string()],
            critical_error: "Validation logic error".to_string(),
        })
    }
}
