//! Cluster Validation Operations
//!
//! This module provides validation operations for cluster-related WAL records:
//! - ClusterCreate: Validate EdgeCluster deserialization and integrity

use crate::backend::native::v2::{Direction, EdgeCluster};
use crate::backend::native::v2::wal::recovery::errors::RecoveryResult;
use crate::backend::native::v2::wal::recovery::validator::{
    ValidationResult, ValidationSeverity, V2_CLUSTER_ALIGNMENT, MAX_CLUSTER_SIZE,
    MAX_EDGES_PER_CLUSTER,
};

use super::super::TransactionValidator;

/// Validate cluster creation with real EdgeCluster checks
pub fn validate_cluster_create(
    _validator: &TransactionValidator,
    node_id: i64,
    _direction: Direction,
    cluster_offset: u64,
    cluster_size: u32,
    edge_data: &[u8],
    _lsn: u64,
) -> RecoveryResult<ValidationResult> {
    let mut issues = Vec::new();

    // Basic validation
    if node_id <= 0 {
        return Ok(ValidationResult::Invalid {
            errors: vec!["Invalid node ID in cluster create".to_string()],
            critical_error: "Cluster create validation failed".to_string(),
        });
    }

    if cluster_size == 0 {
        return Ok(ValidationResult::Invalid {
            errors: vec!["Cluster size cannot be zero".to_string()],
            critical_error: "Cluster create validation failed".to_string(),
        });
    }

    if cluster_offset % V2_CLUSTER_ALIGNMENT != 0 {
        return Ok(ValidationResult::Invalid {
            errors: vec![format!(
                "Cluster offset {} not aligned to V2_CLUSTER_ALIGNMENT {}",
                cluster_offset, V2_CLUSTER_ALIGNMENT
            )],
            critical_error: "V2 cluster alignment error".to_string(),
        });
    }

    // Validate edge data by attempting to deserialize cluster
    let cluster = match EdgeCluster::deserialize(edge_data) {
        Ok(cluster) => cluster,
        Err(e) => {
            return Ok(ValidationResult::Invalid {
                errors: vec![format!("EdgeCluster deserialization failed: {}", e)],
                critical_error: "V2 cluster format error".to_string(),
            });
        }
    };

    // Validate cluster integrity
    if let Err(e) = cluster.validate() {
        return Ok(ValidationResult::Invalid {
            errors: vec![format!("Cluster validation failed: {}", e)],
            critical_error: "V2 cluster integrity error".to_string(),
        });
    }

    // Check cluster size consistency
    let actual_size = edge_data.len() as u32;
    if actual_size != cluster_size {
        issues.push(format!(
            "Cluster size mismatch: expected {}, actual {}",
            cluster_size, actual_size
        ));
    }

    // Validate cluster size constraints
    if cluster_size > MAX_CLUSTER_SIZE {
        issues.push(format!(
            "Cluster exceeds maximum size: {} > {}",
            cluster_size, MAX_CLUSTER_SIZE
        ));
    }

    // Validate edge count
    let edge_count = cluster.edge_count();
    if edge_count == 0 {
        issues.push("Empty cluster created - may indicate inefficient operation".to_string());
    }

    if edge_count > MAX_EDGES_PER_CLUSTER {
        issues.push(format!(
            "Cluster exceeds maximum edge count: {} > {}",
            edge_count, MAX_EDGES_PER_CLUSTER
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
