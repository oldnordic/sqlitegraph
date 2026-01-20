//! Cluster Rollback Operations
//!
//! This module provides rollback operations for cluster-related WAL records:
//! - ClusterCreate: Deallocate cluster and remove node reference

use crate::backend::native::{GraphFile, NodeStore, NativeNodeId};
use crate::backend::native::v2::{EdgeCluster, Direction};
use super::super::RollbackSystem;
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;
use crate::backend::native::v2::wal::recovery::store_helpers;
use crate::debug::debug_log;

/// Rollback cluster creation by deallocating cluster and removing node reference
pub fn rollback_cluster_create(
    system: &RollbackSystem,
    node_id: u64,
    direction: Direction,
    cluster_offset: u64,
    cluster_size: u64,
    _cluster_data: Vec<u8>)
    -> Result<(), RecoveryError>
{
    debug_log!("Rolling back cluster create: node_id={}, direction={:?}, cluster_offset={}, cluster_size={}",
           node_id, direction, cluster_offset, cluster_size);

    // Step 1: Deallocate cluster space via FreeSpaceManager
    {
        let mut free_space_guard = system.free_space_manager().lock()
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to lock free space manager: {}", e)
            ))?;

        let free_space_manager = free_space_guard.as_mut()
            .ok_or_else(|| RecoveryError::replay_failure(
                "Free space manager not initialized".to_string()
            ))?;

        free_space_manager.add_free_block(cluster_offset, cluster_size as u32);

        debug_log!("Deallocated cluster: offset={}, size={}", cluster_offset, cluster_size);
    }

    // Step 2: Remove cluster reference from NodeRecordV2
    // Initialize NodeStore if needed (lazy initialization pattern)
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

    // Step 3: Read current NodeRecordV2, update cluster fields, write back
    {
        let mut node_store_guard = system.node_store().lock()
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to lock node store for node update: {}", e)
            ))?;

        let node_store = node_store_guard.as_mut()
            .ok_or_else(|| RecoveryError::replay_failure(
                "NodeStore initialization failed".to_string()
            ))?;

        // Read current node record - gracefully handle missing node
        let mut node_record = match node_store.read_node_v2(node_id as NativeNodeId) {
            Ok(record) => record,
            Err(_) => {
                // Node doesn't exist - this is acceptable for rollback scenarios
                // where the node was deleted after cluster creation
                debug_log!("Node {} doesn't exist, skipping NodeRecordV2 cluster cleanup for direction={:?}",
                       node_id, direction);
                return Ok(());
            }
        };

        // Clear cluster reference based on direction
        match direction {
            Direction::Outgoing => {
                debug_log!("Clearing outgoing cluster reference: node_id={}, old_offset={}, old_size={}",
                       node_id, node_record.outgoing_cluster_offset, node_record.outgoing_cluster_size);
                node_record.outgoing_cluster_offset = 0;
                node_record.outgoing_cluster_size = 0;
                node_record.outgoing_edge_count = 0;
            },
            Direction::Incoming => {
                debug_log!("Clearing incoming cluster reference: node_id={}, old_offset={}, old_size={}",
                       node_id, node_record.incoming_cluster_offset, node_record.incoming_cluster_size);
                node_record.incoming_cluster_offset = 0;
                node_record.incoming_cluster_size = 0;
                node_record.incoming_edge_count = 0;
            },
        }

        // Write updated node record back to storage
        node_store.write_node_v2(&node_record)
            .map_err(|e| RecoveryError::io_error(
                format!("Failed to update node {} after cluster cleanup: {}", node_id, e)
            ))?;

        debug_log!("Successfully cleared cluster reference from node_id={}, direction={:?}",
               node_id, direction);
    }

    debug_log!("Successfully completed cluster create rollback: node_id={}, direction={:?}, deallocated_offset={}",
           node_id, direction, cluster_offset);
    Ok(())
}
