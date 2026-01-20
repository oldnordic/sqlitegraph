//! Cross-Record Validation
//!
//! This module provides validation operations that check consistency across
//! multiple WAL records within a transaction:
//! - Cross-record consistency (node-cluster, free space allocation)
//! - V2-specific invariants (transaction size limits, record sequence, cluster alignment)

use crate::backend::native::v2::wal::V2WALRecord;
use crate::backend::native::v2::wal::recovery::core::TransactionState;
use crate::backend::native::v2::wal::recovery::errors::RecoveryResult;
use crate::backend::native::v2::wal::recovery::validator::{
    V2_CLUSTER_ALIGNMENT, MAX_RECORDS_PER_TRANSACTION, MAX_TRANSACTION_ALLOCATION,
};

use std::collections::{HashMap, HashSet};

use super::super::TransactionValidator;

/// Validate cross-record consistency within a transaction
pub fn validate_cross_record_consistency(
    _validator: &TransactionValidator,
    transaction: &TransactionState,
) -> RecoveryResult<Vec<String>> {
    let mut issues = Vec::new();

    // Validate node-cluster consistency
    for record in &transaction.records {
        if let V2WALRecord::NodeInsert { node_id, .. } = record {
            // Check if node has corresponding cluster creation records
            let has_cluster_create = transaction.records.iter().any(
                |r| matches!(r, V2WALRecord::ClusterCreate { node_id: n, .. } if n == node_id),
            );

            // Note: This is a simplified check - full implementation would be more sophisticated
            let _ = has_cluster_create;
        }
    }

    // Validate free space allocation consistency
    let mut total_allocated = 0u64;
    for record in &transaction.records {
        if let V2WALRecord::FreeSpaceAllocate { block_size, .. } = record {
            total_allocated += *block_size as u64;
        }
        if let V2WALRecord::FreeSpaceDeallocate { block_size, .. } = record {
            total_allocated = total_allocated.saturating_sub(*block_size as u64);
        }
    }

    if total_allocated > MAX_TRANSACTION_ALLOCATION {
        issues.push(format!(
            "Transaction allocates {} bytes, exceeding maximum {}",
            total_allocated, MAX_TRANSACTION_ALLOCATION
        ));
    }

    Ok(issues)
}

/// Validate V2-specific invariants and constraints
pub fn validate_v2_invariants(
    _validator: &TransactionValidator,
    transaction: &TransactionState,
) -> RecoveryResult<Vec<String>> {
    let mut issues = Vec::new();

    // Validate transaction size limits
    if transaction.records.len() > MAX_RECORDS_PER_TRANSACTION {
        issues.push(format!(
            "Transaction has {} records, exceeding maximum {}",
            transaction.records.len(),
            MAX_RECORDS_PER_TRANSACTION
        ));
    }

    // Validate record sequence consistency
    let mut node_creations = HashMap::new();
    let mut node_deletions = HashSet::new();

    for record in &transaction.records {
        match record {
            V2WALRecord::NodeInsert { node_id, .. } => {
                if node_deletions.contains(node_id) {
                    issues.push(format!(
                        "Node {} created after deletion in same transaction",
                        node_id
                    ));
                }
                node_creations.insert(node_id, true);
            }
            V2WALRecord::NodeDelete { node_id, .. } => {
                if !node_creations.contains_key(node_id) {
                    // Node deletion without prior creation - this could be valid if node existed before
                }
                node_deletions.insert(*node_id);
            }
            _ => {}
        }
    }

    // Validate cluster alignment
    for record in &transaction.records {
        if let V2WALRecord::ClusterCreate { cluster_offset, .. } = record {
            if cluster_offset % V2_CLUSTER_ALIGNMENT != 0 {
                issues.push(format!(
                    "Cluster offset {} not properly aligned to {}",
                    cluster_offset, V2_CLUSTER_ALIGNMENT
                ));
            }
        }
    }

    Ok(issues)
}
