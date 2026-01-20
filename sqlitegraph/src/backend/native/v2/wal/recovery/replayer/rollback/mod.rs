//! V2 WAL Recovery Rollback System
//!
//! This module provides rollback functionality for transaction recovery
//! operations, allowing the system to undo changes when replay fails.
//!
//! ## Module Organization
//!
//! - **mod.rs**: RollbackSystem struct and coordination
//! - **node_ops**: Node rollback operations (insert, update, delete)
//! - **edge_ops**: Edge rollback operations (insert, update, delete)
//! - **cluster_ops**: Cluster rollback operations (create)
//! - **string_ops**: String rollback operations (insert)
//! - **header_ops**: Header rollback operations (update)
//! - **free_space_ops**: Free space rollback operations (allocate, deallocate)

use crate::backend::native::{GraphFile, NodeStore, NativeNodeId};
use crate::backend::native::v2::{StringTable, FreeSpaceManager};
use super::types::RollbackOperation;
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;
use crate::debug::{debug_log, warn_log, error_log};
use std::sync::{Arc, Mutex, RwLock};

// Re-export rollback operation modules
pub mod node_ops;
pub mod edge_ops;
pub mod cluster_ops;
pub mod string_ops;
pub mod header_ops;
pub mod free_space_ops;

// Re-export summary type
pub use self::node_ops::RollbackSummary;

/// Rollback system for transaction recovery
///
/// This system manages rollback operations and can execute them
/// when a transaction replay fails and needs to be undone.
pub struct RollbackSystem {
    operations: Vec<RollbackOperation>,
    graph_file: Arc<RwLock<GraphFile>>,
    node_store: Arc<Mutex<Option<NodeStore<'static>>>>,
    string_table: Arc<Mutex<StringTable>>,
    free_space_manager: Arc<Mutex<Option<FreeSpaceManager>>>,
}

impl RollbackSystem {
    /// Create a new rollback system
    pub fn new(
        graph_file: Arc<RwLock<GraphFile>>,
        node_store: Arc<Mutex<Option<NodeStore<'static>>>>,
        string_table: Arc<Mutex<StringTable>>,
        free_space_manager: Arc<Mutex<Option<FreeSpaceManager>>>,
    ) -> Self {
        Self {
            operations: Vec::new(),
            graph_file,
            node_store,
            string_table,
            free_space_manager,
        }
    }

    /// Add a rollback operation to the system
    pub fn add_operation(&mut self, operation: RollbackOperation) {
        debug_log!("Adding rollback operation: {}", operation.operation_name());
        self.operations.push(operation);
    }

    /// Get the number of pending rollback operations
    pub fn len(&self) -> usize {
        self.operations.len()
    }

    /// Check if there are any pending rollback operations
    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }

    /// Clear all rollback operations
    pub fn clear(&mut self) {
        debug_log!("Clearing {} rollback operations", self.operations.len());
        self.operations.clear();
    }

    /// Execute rollback by applying all operations in reverse order
    pub fn execute_rollback(&self) -> Result<(), RecoveryError> {
        if self.operations.is_empty() {
            debug_log!("No rollback operations to execute");
            return Ok(());
        }

        debug_log!("Persisting rollback state: {} operations in memory", self.operations.len());
        debug_log!("Executing rollback with {} operations", self.operations.len());

        // Apply rollback operations in reverse order (LIFO)
        for (index, operation) in self.operations.iter().rev().enumerate() {
            debug_log!("Applying rollback operation {}/{}: {}",
                   index + 1, self.operations.len(), operation.operation_name());

            if let Err(e) = self.apply_rollback_operation(operation) {
                error_log!("Failed to apply rollback operation {}: {}", operation.operation_name(), e);
                // Continue with remaining operations even if one fails
            }
        }

        debug_log!("Rollback completed successfully");
        Ok(())
    }

    /// Apply a single rollback operation
    fn apply_rollback_operation(&self, operation: &RollbackOperation) -> Result<(), RecoveryError> {
        match operation {
            RollbackOperation::NodeInsert { node_id, node_data } => {
                node_ops::rollback_node_insert(self, *node_id, node_data)?;
            }
            RollbackOperation::NodeUpdate { node_id, old_data } => {
                node_ops::rollback_node_update(self, *node_id, old_data)?;
            }
            RollbackOperation::NodeDelete { node_id, slot_offset, old_data, outgoing_edges, incoming_edges } => {
                node_ops::rollback_node_delete(self, *node_id, *slot_offset, old_data.clone(), outgoing_edges.clone(), incoming_edges.clone())?;
            }
            RollbackOperation::StringInsert { string_id, string_value } => {
                string_ops::rollback_string_insert(self, *string_id, string_value)?;
            }
            RollbackOperation::HeaderUpdate { header_offset, new_data: _new_data, old_data } => {
                header_ops::rollback_header_update(self, *header_offset, old_data)?;
            }
            RollbackOperation::EdgeInsert { cluster_key, insertion_point, edge_record, cluster_offset, cluster_size } => {
                edge_ops::rollback_edge_insert(self, *cluster_key, *insertion_point, edge_record, *cluster_offset, *cluster_size)?;
            }
            RollbackOperation::EdgeUpdate { cluster_key, position, old_edge, new_edge: _new_edge } => {
                edge_ops::rollback_edge_update(self, *cluster_key, *position, old_edge)?;
            }
            RollbackOperation::EdgeDelete { cluster_key, position, old_edge } => {
                edge_ops::rollback_edge_delete(self, *cluster_key, *position, old_edge)?;
            }
            RollbackOperation::ClusterCreate { node_id, direction, cluster_offset, cluster_size, cluster_data } => {
                cluster_ops::rollback_cluster_create(self, *node_id, *direction, *cluster_offset, *cluster_size, cluster_data.clone())?;
            }
            RollbackOperation::FreeSpaceAllocate { block_offset, block_size, block_type } => {
                free_space_ops::rollback_free_space_allocate(self, *block_offset, *block_size, *block_type)?;
            }
            RollbackOperation::FreeSpaceDeallocate { block_offset, block_size, block_type } => {
                free_space_ops::rollback_free_space_deallocate(self, *block_offset, *block_size, *block_type)?;
            }
        }
        Ok(())
    }

    /// Get a summary of pending rollback operations
    pub fn get_summary(&self) -> RollbackSummary {
        let mut node_insert_count = 0;
        let mut node_update_count = 0;
        let mut node_delete_count = 0;
        let mut string_insert_count = 0;
        let mut header_update_count = 0;
        let mut edge_insert_count = 0;
        let mut edge_update_count = 0;
        let mut edge_delete_count = 0;
        let mut cluster_create_count = 0;
        let mut free_space_allocate_count = 0;
        let mut free_space_deallocate_count = 0;

        for operation in &self.operations {
            match operation {
                RollbackOperation::NodeInsert { .. } => node_insert_count += 1,
                RollbackOperation::NodeUpdate { .. } => node_update_count += 1,
                RollbackOperation::NodeDelete { .. } => node_delete_count += 1,
                RollbackOperation::StringInsert { .. } => string_insert_count += 1,
                RollbackOperation::HeaderUpdate { .. } => header_update_count += 1,
                RollbackOperation::EdgeInsert { .. } => edge_insert_count += 1,
                RollbackOperation::EdgeUpdate { .. } => edge_update_count += 1,
                RollbackOperation::EdgeDelete { .. } => edge_delete_count += 1,
                RollbackOperation::ClusterCreate { .. } => cluster_create_count += 1,
                RollbackOperation::FreeSpaceAllocate { .. } => free_space_allocate_count += 1,
                RollbackOperation::FreeSpaceDeallocate { .. } => free_space_deallocate_count += 1,
            }
        }

        RollbackSummary {
            total_operations: self.operations.len(),
            node_insert_count,
            node_update_count,
            node_delete_count,
            string_insert_count,
            header_update_count,
            edge_insert_count,
            edge_update_count,
            edge_delete_count,
            cluster_create_count,
            free_space_allocate_count,
            free_space_deallocate_count,
        }
    }

    // Getters for operation modules to access internal state
    pub(crate) fn graph_file(&self) -> &Arc<RwLock<GraphFile>> {
        &self.graph_file
    }

    pub(crate) fn node_store(&self) -> &Arc<Mutex<Option<NodeStore<'static>>>> {
        &self.node_store
    }

    pub(crate) fn string_table(&self) -> &Arc<Mutex<StringTable>> {
        &self.string_table
    }

    pub(crate) fn free_space_manager(&self) -> &Arc<Mutex<Option<FreeSpaceManager>>> {
        &self.free_space_manager
    }
}
