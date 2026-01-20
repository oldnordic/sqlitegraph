//! Node Validation Operations
//!
//! This module provides validation operations for node-related WAL records:
//! - NodeInsert: Validate node record deserialization and consistency
//! - NodeUpdate: Validate old and new data consistency
//! - NodeDelete: Validate proper cleanup checks

use crate::backend::native::v2::node_record_v2::NodeRecordV2;
use crate::backend::native::v2::wal::recovery::errors::RecoveryResult;
use crate::backend::native::v2::wal::recovery::validator::{
    ValidationResult, ValidationSeverity, MAX_NODE_RECORD_SIZE,
};

use super::super::TransactionValidator;

/// Validate node insertion with real NodeRecordV2 deserialization and checks
pub fn validate_node_insert(
    _validator: &TransactionValidator,
    node_id: i64,
    node_data: &[u8],
    _lsn: u64,
) -> RecoveryResult<ValidationResult> {
    let mut issues = Vec::new();

    // Basic validation
    if node_id <= 0 {
        return Ok(ValidationResult::Invalid {
            errors: vec!["Invalid node ID: must be positive".to_string()],
            critical_error: "Node ID validation failed".to_string(),
        });
    }

    if node_data.is_empty() {
        return Ok(ValidationResult::Invalid {
            errors: vec!["Node data cannot be empty".to_string()],
            critical_error: "Node data validation failed".to_string(),
        });
    }

    // Deserialize and validate NodeRecordV2 with real V2 backend
    let node_record = match NodeRecordV2::deserialize(node_data) {
        Ok(record) => record,
        Err(e) => {
            return Ok(ValidationResult::Invalid {
                errors: vec![format!("NodeRecordV2 deserialization failed: {}", e)],
                critical_error: "V2 node record format error".to_string(),
            });
        }
    };

    // Validate NodeRecordV2 consistency
    if let Err(e) = node_record.validate() {
        return Ok(ValidationResult::Invalid {
            errors: vec![format!("NodeRecordV2 validation failed: {}", e)],
            critical_error: "V2 node record consistency error".to_string(),
        });
    }

    // Check if node ID matches serialized record
    if node_record.id != node_id {
        return Ok(ValidationResult::Invalid {
            errors: vec![format!(
                "Node ID mismatch: expected {}, got {}",
                node_id, node_record.id
            )],
            critical_error: "V2 node record ID inconsistency".to_string(),
        });
    }

    // Validate V2-specific invariants
    if node_record.has_outgoing_edges() {
        // Verify cluster metadata consistency
        if node_record.outgoing_cluster_offset == 0 || node_record.outgoing_cluster_size == 0 {
            issues.push("Node claims outgoing edges but has no cluster metadata".to_string());
        }

        if node_record.outgoing_cluster_size != node_record.outgoing_edge_count * 58 {
            // Estimated edge size
            issues.push(format!(
                "Outgoing cluster size inconsistency: size={}, edge_count={}",
                node_record.outgoing_cluster_size, node_record.outgoing_edge_count
            ));
        }
    }

    if node_record.has_incoming_edges() {
        // Verify incoming cluster metadata consistency
        if node_record.incoming_cluster_offset == 0 || node_record.incoming_cluster_size == 0 {
            issues.push("Node claims incoming edges but has no cluster metadata".to_string());
        }
    }

    // Validate node record size constraints
    if node_data.len() > MAX_NODE_RECORD_SIZE {
        issues.push(format!(
            "Node record exceeds maximum size: {} > {}",
            node_data.len(),
            MAX_NODE_RECORD_SIZE
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

/// Validate node update with old and new data consistency
pub fn validate_node_update(
    _validator: &TransactionValidator,
    node_id: i64,
    old_data: &[u8],
    new_data: &[u8],
    _lsn: u64,
) -> RecoveryResult<ValidationResult> {
    let mut issues = Vec::new();

    // Validate both old and new data
    let old_record = match NodeRecordV2::deserialize(old_data) {
        Ok(record) => record,
        Err(e) => {
            return Ok(ValidationResult::Invalid {
                errors: vec![format!("Old NodeRecordV2 deserialization failed: {}", e)],
                critical_error: "V2 node update format error".to_string(),
            });
        }
    };

    let new_record = match NodeRecordV2::deserialize(new_data) {
        Ok(record) => record,
        Err(e) => {
            return Ok(ValidationResult::Invalid {
                errors: vec![format!("New NodeRecordV2 deserialization failed: {}", e)],
                critical_error: "V2 node update format error".to_string(),
            });
        }
    };

    // Validate record consistency
    if old_record.id != node_id || new_record.id != node_id {
        return Ok(ValidationResult::Invalid {
            errors: vec!["Node ID mismatch in update record".to_string()],
            critical_error: "V2 node update ID inconsistency".to_string(),
        });
    }

    // Validate immutable fields haven't changed
    if old_record.kind != new_record.kind {
        issues.push("Node kind changed in update (should be immutable)".to_string());
    }

    // Validate V2 cluster metadata changes are consistent
    if old_record.has_outgoing_edges() && !new_record.has_outgoing_edges() {
        issues
            .push("Outgoing edges disappeared in update without explicit deletion".to_string());
    }

    if old_record.has_incoming_edges() && !new_record.has_incoming_edges() {
        issues
            .push("Incoming edges disappeared in update without explicit deletion".to_string());
    }

    Ok(if issues.is_empty() {
        ValidationResult::Valid
    } else {
        ValidationResult::Recoverable {
            issues,
            severity: ValidationSeverity::Error,
        }
    })
}

/// Validate node deletion with proper cleanup checks
pub fn validate_node_delete(
    _validator: &TransactionValidator,
    node_id: i64,
    old_data: &[u8],
    _lsn: u64,
) -> RecoveryResult<ValidationResult> {
    let mut issues = Vec::new();

    // Deserialize old node record for validation
    let old_record = match NodeRecordV2::deserialize(old_data) {
        Ok(record) => record,
        Err(e) => {
            return Ok(ValidationResult::Invalid {
                errors: vec![format!(
                    "Old NodeRecordV2 deserialization failed in delete: {}",
                    e
                )],
                critical_error: "V2 node delete format error".to_string(),
            });
        }
    };

    // Validate node ID consistency
    if old_record.id != node_id {
        return Ok(ValidationResult::Invalid {
            errors: vec!["Node ID mismatch in delete record".to_string()],
            critical_error: "V2 node delete ID inconsistency".to_string(),
        });
    }

    // Check if node has dependencies that need cleanup
    if old_record.has_outgoing_edges() {
        issues.push("Node with outgoing edges deleted - cluster cleanup required".to_string());
    }

    if old_record.has_incoming_edges() {
        issues.push(
            "Node with incoming edges deleted - inbound references may be orphaned".to_string(),
        );
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
