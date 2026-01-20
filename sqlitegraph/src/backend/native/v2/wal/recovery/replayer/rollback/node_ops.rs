//! Node Rollback Operations
//!
//! This module provides rollback operations for node-related WAL records:
//! - NodeInsert: Delete the inserted node
//! - NodeUpdate: Restore the old node data
//! - NodeDelete: Reinsert the deleted node with all edges

use crate::backend::native::{GraphFile, NodeStore, NativeNodeId, NodeRecordV2};
use crate::backend::native::v2::{EdgeCluster, Direction, FreeSpaceManager};
use super::super::RollbackSystem;
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;
use crate::backend::native::v2::wal::recovery::store_helpers;
use crate::debug::{debug_log, warn_log, error_log};
use std::sync::{Arc, Mutex, RwLock};

/// Summary of pending rollback operations
#[derive(Debug, Clone, Default)]
pub struct RollbackSummary {
    pub total_operations: usize,
    pub node_insert_count: u32,
    pub node_update_count: u32,
    pub node_delete_count: u32,
    pub string_insert_count: u32,
    pub header_update_count: u32,
    pub edge_insert_count: u32,
    pub edge_update_count: u32,
    pub edge_delete_count: u32,
    pub cluster_create_count: u32,
    pub free_space_allocate_count: u32,
    pub free_space_deallocate_count: u32,
}

impl RollbackSummary {
    /// Check if there are any node operations to rollback
    pub fn has_node_operations(&self) -> bool {
        self.node_insert_count + self.node_update_count + self.node_delete_count > 0
    }

    /// Check if there are any string operations to rollback
    pub fn has_string_operations(&self) -> bool {
        self.string_insert_count > 0
    }

    /// Check if there are any free space operations to rollback
    pub fn has_free_space_operations(&self) -> bool {
        self.free_space_allocate_count + self.free_space_deallocate_count > 0
    }

    /// Check if there are any edge operations to rollback
    pub fn has_edge_operations(&self) -> bool {
        self.edge_insert_count + self.edge_update_count + self.edge_delete_count > 0
    }

    /// Get the total number of data operations (node + string)
    pub fn data_operations_count(&self) -> usize {
        (self.node_insert_count + self.node_update_count + self.node_delete_count + self.string_insert_count) as usize
    }
}

/// Rollback node insertion by deleting the node
pub fn rollback_node_insert(
    system: &RollbackSystem,
    node_id: NativeNodeId,
    _node_data: &[u8],
) -> Result<(), RecoveryError> {
    debug_log!("Rolling back node insert: node_id={}", node_id);

    // Ensure node store is initialized
    {
        let mut node_store_guard = system.node_store().lock()
            .map_err(|e| RecoveryError::replay_failure(format!("Failed to lock node store: {}", e)))?;

        if node_store_guard.is_none() {
            let mut graph_file = system.graph_file().write()
                .map_err(|e| RecoveryError::io_error(format!("Failed to lock graph file: {}", e)))?;
            *node_store_guard = Some(unsafe {
                store_helpers::create_node_store(&mut *graph_file)
            });
        }
    }

    // Delete the node
    {
        let mut node_store_guard = system.node_store().lock()
            .map_err(|e| RecoveryError::replay_failure(format!("Failed to lock node store: {}", e)))?;
        let node_store = node_store_guard.as_mut()
            .ok_or_else(|| RecoveryError::replay_failure("Node store not initialized".to_string()))?;

        node_store.delete_node(node_id)
            .map_err(|e| RecoveryError::io_error(format!("Failed to delete node during rollback: {}", e)))?;
    }

    debug_log!("Successfully rolled back node insert: node_id={}", node_id);
    Ok(())
}

/// Rollback node update by restoring old data
pub fn rollback_node_update(
    system: &RollbackSystem,
    node_id: NativeNodeId,
    old_data: &[u8],
) -> Result<(), RecoveryError> {
    debug_log!("Rolling back node update: node_id={}, data_size={}", node_id, old_data.len());

    // Restore old node data
    let node_record = NodeRecordV2::deserialize(old_data)
        .map_err(|e| RecoveryError::io_error(format!("Failed to deserialize old node data: {}", e)))?;

    // Ensure node store is initialized
    {
        let mut node_store_guard = system.node_store().lock()
            .map_err(|e| RecoveryError::replay_failure(format!("Failed to lock node store: {}", e)))?;

        if node_store_guard.is_none() {
            let mut graph_file = system.graph_file().write()
                .map_err(|e| RecoveryError::io_error(format!("Failed to lock graph file: {}", e)))?;
            *node_store_guard = Some(unsafe {
                store_helpers::create_node_store(&mut *graph_file)
            });
        }
    }

    // Write old node data
    {
        let mut node_store_guard = system.node_store().lock()
            .map_err(|e| RecoveryError::replay_failure(format!("Failed to lock node store: {}", e)))?;
        let node_store = node_store_guard.as_mut()
            .ok_or_else(|| RecoveryError::replay_failure("Node store not initialized".to_string()))?;

        node_store.write_node_v2(&node_record)
            .map_err(|e| RecoveryError::io_error(format!("Failed to restore old node data: {}", e)))?;
    }

    debug_log!("Successfully rolled back node update: node_id={}", node_id);
    Ok(())
}

/// Rollback node deletion by reinserting the node with all edges
pub fn rollback_node_delete(
    system: &RollbackSystem,
    node_id: NativeNodeId,
    _slot_offset: u64,
    old_data: Vec<u8>,
    outgoing_edges: Vec<crate::backend::native::v2::edge_cluster::CompactEdgeRecord>,
    incoming_edges: Vec<crate::backend::native::v2::edge_cluster::CompactEdgeRecord>,
) -> Result<(), RecoveryError> {
    debug_log!("Rolling back node delete: node_id={}, slot_offset={}, old_data_size={}", node_id, _slot_offset, old_data.len());

    // Step 1: Deserialize old node data
    let node_record = NodeRecordV2::deserialize(&old_data)
        .map_err(|e| RecoveryError::io_error(
            format!("Failed to deserialize old node data: {}", e)
        ))?;

    debug_log!("Deserialized node record: id={}, kind={}, name={}", node_record.id, node_record.kind, node_record.name);

    // Step 2: Ensure NodeStore is initialized
    {
        let mut node_store_guard = system.node_store().lock()
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to lock node store: {}", e)
            ))?;

        if node_store_guard.is_none() {
            let mut graph_file = system.graph_file().write()
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to lock graph file: {}", e)
                ))?;
            *node_store_guard = Some(unsafe {
                store_helpers::create_node_store(&mut *graph_file)
            });
        }
    }

    // Step 3: Write node back to storage using NodeStore
    {
        let mut node_store_guard = system.node_store().lock()
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to lock node store for node restoration: {}", e)
            ))?;

        let node_store = node_store_guard.as_mut()
            .ok_or_else(|| RecoveryError::replay_failure(
                "NodeStore initialization failed".to_string()
            ))?;

        // Write the restored node record back to the node store
        node_store.write_node_v2(&node_record)
            .map_err(|e| RecoveryError::io_error(
                format!("Failed to restore deleted node: {}", e)
            ))?;

        debug_log!("Successfully wrote restored node record to NodeStore");
    }

    // Step 4: Restore outgoing cluster if edges were captured
    if !outgoing_edges.is_empty() {
        debug_log!("Restoring {} outgoing edges for node {}", outgoing_edges.len(), node_id);

        let cluster_data = {
            // Create cluster from captured edges
            let cluster = EdgeCluster::create_from_compact_edges(
                outgoing_edges.clone(),
                node_id as i64,
                Direction::Outgoing,
            ).map_err(|e| RecoveryError::io_error(
                format!("Failed to create outgoing cluster: {:?}", e)
            ))?;

            cluster.serialize()
        };

        let cluster_offset = {
            let mut free_space_guard = system.free_space_manager().lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock free space manager: {}", e)
                ))?;

            let free_space_manager = free_space_guard.as_mut()
                .ok_or_else(|| RecoveryError::replay_failure(
                    "Free space manager not initialized".to_string()
                ))?;

            // Calculate required size and validate against cluster_floor
            let cluster_floor = {
                let graph_file = system.graph_file().read()
                    .map_err(|e| RecoveryError::io_error(
                        format!("Failed to lock graph file for cluster_floor check: {}", e)
                    ))?;

                graph_file.cluster_floor()
            };

            // Use regular allocate since allocate_with_floor doesn't exist
            // The cluster will be allocated at or above cluster_floor naturally
            let allocated_offset = free_space_manager.allocate(
                cluster_data.len() as u32
            ).map_err(|e| RecoveryError::io_error(
                format!("Failed to allocate space for outgoing cluster: {:?}", e)
            ))?;

            // Verify allocation is above cluster_floor
            if allocated_offset < cluster_floor {
                return Err(RecoveryError::validation(
                    format!("Allocated offset {} is below cluster_floor {}", allocated_offset, cluster_floor)
                ));
            }

            allocated_offset
        };

        // Write cluster data to allocated offset
        {
            let mut graph_file = system.graph_file().write()
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to lock graph file for cluster write: {}", e)
                ))?;

            graph_file.write_bytes(cluster_offset, &cluster_data)
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to write outgoing cluster at offset {}: {:?}", cluster_offset, e)
                ))?;

            debug_log!("Wrote outgoing cluster: offset={}, size={}", cluster_offset, cluster_data.len());
        }

        // Update node record with cluster reference
        {
            let mut node_store_guard = system.node_store().lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock node store for node update: {}", e)
                ))?;

            let node_store = node_store_guard.as_mut()
                .ok_or_else(|| RecoveryError::replay_failure(
                    "NodeStore initialization failed".to_string()
                ))?;

            let mut updated_node = node_store.read_node_v2(node_id)
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to read node for cluster reference update: {}", e)
                ))?;

            updated_node.outgoing_cluster_offset = cluster_offset;
            updated_node.outgoing_cluster_size = cluster_data.len() as u32;
            updated_node.outgoing_edge_count = outgoing_edges.len() as u32;

            node_store.write_node_v2(&updated_node)
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to update node with outgoing cluster reference: {}", e)
                ))?;

            debug_log!("Updated node {} with outgoing cluster: offset={}, size={}, count={}",
                       node_id, cluster_offset, cluster_data.len(), outgoing_edges.len());
        }
    }

    // Step 5: Restore incoming cluster if edges were captured
    if !incoming_edges.is_empty() {
        debug_log!("Restoring {} incoming edges for node {}", incoming_edges.len(), node_id);

        let cluster_data = {
            // Create cluster from captured edges
            let cluster = EdgeCluster::create_from_compact_edges(
                incoming_edges.clone(),
                node_id as i64,
                Direction::Incoming,
            ).map_err(|e| RecoveryError::io_error(
                format!("Failed to create incoming cluster: {:?}", e)
            ))?;

            cluster.serialize()
        };

        let cluster_offset = {
            let mut free_space_guard = system.free_space_manager().lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock free space manager: {}", e)
                ))?;

            let free_space_manager = free_space_guard.as_mut()
                .ok_or_else(|| RecoveryError::replay_failure(
                    "Free space manager not initialized".to_string()
                ))?;

            // Calculate required size and validate against cluster_floor
            let cluster_floor = {
                let graph_file = system.graph_file().read()
                    .map_err(|e| RecoveryError::io_error(
                        format!("Failed to lock graph file for cluster_floor check: {}", e)
                    ))?;

                graph_file.cluster_floor()
            };

            // Use regular allocate since allocate_with_floor doesn't exist
            let allocated_offset = free_space_manager.allocate(
                cluster_data.len() as u32
            ).map_err(|e| RecoveryError::io_error(
                format!("Failed to allocate space for incoming cluster: {:?}", e)
            ))?;

            // Verify allocation is above cluster_floor
            if allocated_offset < cluster_floor {
                return Err(RecoveryError::validation(
                    format!("Allocated offset {} is below cluster_floor {}", allocated_offset, cluster_floor)
                ));
            }

            allocated_offset
        };

        // Write cluster data to allocated offset
        {
            let mut graph_file = system.graph_file().write()
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to lock graph file for cluster write: {}", e)
                ))?;

            graph_file.write_bytes(cluster_offset, &cluster_data)
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to write incoming cluster at offset {}: {:?}", cluster_offset, e)
                ))?;

            debug_log!("Wrote incoming cluster: offset={}, size={}", cluster_offset, cluster_data.len());
        }

        // Update node record with cluster reference
        {
            let mut node_store_guard = system.node_store().lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock node store for node update: {}", e)
                ))?;

            let node_store = node_store_guard.as_mut()
                .ok_or_else(|| RecoveryError::replay_failure(
                    "NodeStore initialization failed".to_string()
                ))?;

            let mut updated_node = node_store.read_node_v2(node_id)
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to read node for cluster reference update: {}", e)
                ))?;

            updated_node.incoming_cluster_offset = cluster_offset;
            updated_node.incoming_cluster_size = cluster_data.len() as u32;
            updated_node.incoming_edge_count = incoming_edges.len() as u32;

            node_store.write_node_v2(&updated_node)
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to update node with incoming cluster reference: {}", e)
                ))?;

            debug_log!("Updated node {} with incoming cluster: offset={}, size={}, count={}",
                       node_id, cluster_offset, cluster_data.len(), incoming_edges.len());
        }
    }

    // Step 6: Reclaim slot - remove from free list to prevent reuse
    if _slot_offset != 0 {
        debug_log!("Reclaiming slot at offset {} for node {}", _slot_offset, node_id);

        // Get the estimated node size (same as used during deallocation)
        let estimated_node_size = std::mem::size_of::<NodeRecordV2>() as u32;

        // Remove the block from free list
        {
            let mut free_space_guard = system.free_space_manager().lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock free space manager for slot reclamation: {}", e)
                ))?;

            let free_space_manager = free_space_guard.as_mut()
                .ok_or_else(|| RecoveryError::replay_failure(
                    "Free space manager not initialized".to_string()
                ))?;

            // Try to remove from free list if method exists
            match free_space_manager.remove_from_free_list(_slot_offset, estimated_node_size) {
                Ok(()) => {
                    debug_log!("Successfully reclaimed slot at offset {} (size {})", _slot_offset, estimated_node_size);
                }
                Err(_) => {
                    // Slot not found in free list - this is acceptable since the slot may have
                    // already been reused or was never added to the free list
                    debug_log!("Slot at offset {} not found in free list - may have been reused", _slot_offset);
                }
            }
        }
    }

    debug_log!("Successfully rolled back node delete: node_id={}, restored kind={}, name={}, edge_counts=(outgoing={}, incoming={})",
           node_id, node_record.kind, node_record.name, node_record.outgoing_edge_count, node_record.incoming_edge_count);

    Ok(())
}
