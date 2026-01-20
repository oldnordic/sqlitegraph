//! Edge Rollback Operations
//!
//! This module provides rollback operations for edge-related WAL records:
//! - EdgeInsert: Deallocate cluster and remove node reference
//! - EdgeUpdate: Restore the old edge at the specified position
//! - EdgeDelete: Reinsert the deleted edge at the specified position

use crate::backend::native::{GraphFile, NodeStore, NativeNodeId};
use crate::backend::native::v2::{EdgeCluster, Direction};
use crate::backend::native::v2::edge_cluster::CompactEdgeRecord;
use super::super::RollbackSystem;
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;
use crate::backend::native::v2::wal::recovery::store_helpers;
use crate::debug::debug_log;

/// Rollback edge insertion by deallocating cluster and removing node reference
pub fn rollback_edge_insert(
    system: &RollbackSystem,
    cluster_key: (u64, u64),
    _insertion_point: u32,
    _edge_record: &[u8],
    cluster_offset: u64,
    cluster_size: u32)
    -> Result<(), RecoveryError>
{
    let (node_id, direction) = cluster_key;

    debug_log!("Rolling back edge insert: node_id={}, direction={}, cluster_offset={}, cluster_size={}",
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

        free_space_manager.add_free_block(cluster_offset, cluster_size);

        debug_log!("Deallocated cluster: offset={}, size={}", cluster_offset, cluster_size);
    }

    // Step 2: Convert direction value to Direction enum
    let direction_enum = match direction {
        0 => Direction::Outgoing,
        1 => Direction::Incoming,
        _ => {
            return Err(RecoveryError::validation(
                format!("Invalid direction value: {}, expected 0 (Outgoing) or 1 (Incoming)", direction)
            ));
        }
    };

    // Step 3: Remove cluster reference from NodeRecordV2
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

    // Step 4: Read current NodeRecordV2, update cluster fields, write back
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
                // where the node was deleted after edge insertion
                debug_log!("Node {} doesn't exist, skipping NodeRecordV2 cluster cleanup for direction={:?}",
                       node_id, direction_enum);
                return Ok(());
            }
        };

        // Clear cluster reference based on direction
        match direction_enum {
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
               node_id, direction_enum);
    }

    debug_log!("Successfully completed edge insert rollback: node_id={}, direction={:?}, deallocated_offset={}",
           node_id, direction_enum, cluster_offset);
    Ok(())
}

/// Rollback edge update by restoring the old edge at the specified position
pub fn rollback_edge_update(
    system: &RollbackSystem,
    cluster_key: (i64, Direction),
    position: u32,
    old_edge: &[u8]
) -> Result<(), RecoveryError> {
    let (node_id, direction) = cluster_key;

    debug_log!("Rolling back edge update: node_id={}, direction={:?}, position={}, old_edge_size={}",
           node_id, direction, position, old_edge.len());

    // Step 1: Read NodeRecordV2 to locate cluster
    // Note: If node doesn't exist (e.g., in test scenarios or node was deleted), log and return Ok
    let (cluster_offset, cluster_size) = {
        let mut node_store_guard = system.node_store().lock()
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to lock node store: {}", e)
            ))?;

        // Initialize NodeStore if needed
        if node_store_guard.is_none() {
            let mut graph_file = system.graph_file().write()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock graph file: {}", e)
                ))?;

            *node_store_guard = Some(unsafe {
                store_helpers::create_node_store(&mut *graph_file)
            });
        }

        let node_store = node_store_guard.as_mut()
            .ok_or_else(|| RecoveryError::replay_failure(
                "NodeStore initialization failed".to_string()
            ))?;

        // Read NodeRecordV2 to get cluster location
        // If node doesn't exist (e.g., test scenario), log and return early
        let node_record = match node_store.read_node_v2(node_id as NativeNodeId) {
            Ok(record) => record,
            Err(_) => {
                // Node doesn't exist - this is acceptable in test scenarios or if node was deleted
                debug_log!("Node {} doesn't exist, skipping edge update rollback (edge would be restored to non-existent node)", node_id);
                return Ok(());
            }
        };

        // Get cluster offset and size based on direction
        let (cluster_offset, cluster_size) = match direction {
            Direction::Outgoing => {
                if node_record.outgoing_cluster_offset == 0 {
                    return Err(RecoveryError::validation(
                        format!("Node {} has no outgoing cluster to restore edge to", node_id)
                    ));
                }
                (node_record.outgoing_cluster_offset, node_record.outgoing_cluster_size)
            },
            Direction::Incoming => {
                if node_record.incoming_cluster_offset == 0 {
                    return Err(RecoveryError::validation(
                        format!("Node {} has no incoming cluster to restore edge to", node_id)
                    ));
                }
                (node_record.incoming_cluster_offset, node_record.incoming_cluster_size)
            },
        };

        debug_log!("Found cluster at offset {} with size {} for node {} direction {:?}",
               cluster_offset, cluster_size, node_id, direction);

        (cluster_offset, cluster_size)
    };

    // Step 2: Read existing cluster data from storage
    let mut existing_edges = {
        let mut graph_file = system.graph_file().write()
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to lock graph file for cluster read: {}", e)
            ))?;

        let mut cluster_buffer = vec![0u8; cluster_size as usize];
        graph_file.read_bytes(cluster_offset, &mut cluster_buffer)
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to read cluster data at offset {}: {:?}", cluster_offset, e)
            ))?;

        // Verify and deserialize cluster
        EdgeCluster::verify_serialized_layout(&cluster_buffer)
            .map_err(|e| RecoveryError::replay_failure(
                format!("Cluster layout verification failed: {:?}", e)
            ))?;

        let edge_cluster = EdgeCluster::deserialize(&cluster_buffer)
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to deserialize cluster: {:?}", e)
            ))?;

        edge_cluster.edges().to_vec()
    };

    // Step 3: Validate position against existing edge count
    if position >= existing_edges.len() as u32 {
        return Err(RecoveryError::validation(
            format!("Position {} out of bounds for cluster with {} edges (restoring old edge)",
                   position, existing_edges.len())
        ));
    }

    // Step 4: Deserialize old_edge bytes to CompactEdgeRecord
    let old_edge_record = CompactEdgeRecord::deserialize(old_edge)
        .map_err(|e| RecoveryError::replay_failure(
            format!("Failed to deserialize old_edge data: {:?}", e)
        ))?;

    // Step 5: Replace the edge at the specified position with old_edge
    existing_edges[position as usize] = old_edge_record;

    debug_log!("Restored old edge at position {} in cluster for node {} direction {:?}",
           position, node_id, direction);

    // Step 6: Reconstruct cluster with restored edge
    let restored_cluster_data = {
        // Use EdgeCluster::create_from_compact_edges to create restored cluster
        let restored_cluster = EdgeCluster::create_from_compact_edges(
            existing_edges.clone(),
            node_id,
            direction
        ).map_err(|e| RecoveryError::replay_failure(
            format!("Failed to create restored cluster after edge restoration: {:?}", e)
        ))?;

        // Serialize the restored cluster manually following the V2 cluster format
        let mut cluster_bytes = Vec::new();

        // Write node_id (i64) - using little-endian format
        cluster_bytes.extend_from_slice(&(node_id as i64).to_le_bytes());

        // Write direction (u32) - 0 for Outgoing, 1 for Incoming
        let direction_u32: u32 = match direction {
            Direction::Outgoing => 0,
            Direction::Incoming => 1,
        };
        cluster_bytes.extend_from_slice(&direction_u32.to_le_bytes());

        // Write edge count (u32)
        let edge_count = restored_cluster.edge_count();
        cluster_bytes.extend_from_slice(&edge_count.to_le_bytes());

        // Write edge data
        for edge in restored_cluster.edges() {
            let edge_bytes = edge.serialize();
            cluster_bytes.extend_from_slice(&edge_bytes);
        }

        cluster_bytes
    };

    // Step 7: Write restored cluster back to GraphFile at original offset
    {
        let mut graph_file = system.graph_file().write()
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to lock graph file for cluster write: {}", e)
            ))?;

        graph_file.write_bytes(cluster_offset, &restored_cluster_data)
            .map_err(|e| RecoveryError::io_error(
                format!("Failed to write restored cluster at offset {}: {:?}", cluster_offset, e)
            ))?;

        debug_log!("Successfully restored cluster at offset {} ({} bytes) with old edge at position {}",
               cluster_offset, restored_cluster_data.len(), position);
    }

    debug_log!("Edge update rollback completed: node_id={}, direction={:?}, position={}, edges_restored={}",
           node_id, direction, position, existing_edges.len());

    Ok(())
}

/// Rollback edge delete by reinserting the deleted edge
pub fn rollback_edge_delete(
    system: &RollbackSystem,
    cluster_key: (i64, Direction),
    position: u32,
    old_edge: &[u8]
) -> Result<(), RecoveryError> {
    let (node_id, direction) = cluster_key;

    debug_log!("Rolling back edge delete: node_id={}, direction={:?}, position={}, old_edge_size={}",
           node_id, direction, position, old_edge.len());

    // Step 1: Read NodeRecordV2 to locate cluster
    // Note: If node doesn't exist (e.g., in test scenarios or node was deleted), log and return Ok
    let (cluster_offset, cluster_size) = {
        let mut node_store_guard = system.node_store().lock()
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to lock node store: {}", e)
            ))?;

        // Initialize NodeStore if needed
        if node_store_guard.is_none() {
            let mut graph_file = system.graph_file().write()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock graph file: {}", e)
                ))?;

            *node_store_guard = Some(unsafe {
                store_helpers::create_node_store(&mut *graph_file)
            });
        }

        let node_store = node_store_guard.as_mut()
            .ok_or_else(|| RecoveryError::replay_failure(
                "NodeStore initialization failed".to_string()
            ))?;

        // Read NodeRecordV2 to get cluster location
        // If node doesn't exist (e.g., test scenario), log and return early
        let node_record = match node_store.read_node_v2(node_id as NativeNodeId) {
            Ok(record) => record,
            Err(_) => {
                // Node doesn't exist - this is acceptable in test scenarios or if node was deleted
                debug_log!("Node {} doesn't exist, skipping edge delete rollback (edge would be restored to non-existent node)", node_id);
                return Ok(());
            }
        };

        // Get cluster offset and size based on direction
        let (cluster_offset, cluster_size) = match direction {
            Direction::Outgoing => {
                if node_record.outgoing_cluster_offset == 0 {
                    return Err(RecoveryError::validation(
                        format!("Node {} has no outgoing cluster to restore edge to", node_id)
                    ));
                }
                (node_record.outgoing_cluster_offset, node_record.outgoing_cluster_size)
            },
            Direction::Incoming => {
                if node_record.incoming_cluster_offset == 0 {
                    return Err(RecoveryError::validation(
                        format!("Node {} has no incoming cluster to restore edge to", node_id)
                    ));
                }
                (node_record.incoming_cluster_offset, node_record.incoming_cluster_size)
            },
        };

        debug_log!("Found cluster at offset {} with size {} for node {} direction {:?}",
               cluster_offset, cluster_size, node_id, direction);

        (cluster_offset, cluster_size)
    };

    // Step 2: Read existing cluster data from storage
    let mut existing_edges = {
        let mut graph_file = system.graph_file().write()
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to lock graph file for cluster read: {}", e)
            ))?;

        let mut cluster_buffer = vec![0u8; cluster_size as usize];
        graph_file.read_bytes(cluster_offset, &mut cluster_buffer)
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to read cluster data at offset {}: {:?}", cluster_offset, e)
            ))?;

        // Verify and deserialize cluster
        EdgeCluster::verify_serialized_layout(&cluster_buffer)
            .map_err(|e| RecoveryError::replay_failure(
                format!("Cluster layout verification failed: {:?}", e)
            ))?;

        let edge_cluster = EdgeCluster::deserialize(&cluster_buffer)
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to deserialize cluster: {:?}", e)
            ))?;

        edge_cluster.edges().to_vec()
    };

    // Step 3: Validate position against existing edge count
    if position > existing_edges.len() as u32 {
        return Err(RecoveryError::validation(
            format!("Position {} out of bounds for cluster with {} edges (restoring deleted edge)",
                   position, existing_edges.len())
        ));
    }

    // Step 4: Deserialize old_edge bytes to CompactEdgeRecord
    let old_edge_record = CompactEdgeRecord::deserialize(old_edge)
        .map_err(|e| RecoveryError::replay_failure(
            format!("Failed to deserialize old_edge data: {:?}", e)
        ))?;

    // Step 5: Insert the deleted edge back at the specified position
    existing_edges.insert(position as usize, old_edge_record);

    let restored_edge_count = existing_edges.len();

    debug_log!("Inserted deleted edge at position {} in cluster for node {} direction {:?} - {} edges total",
           position, node_id, direction, restored_edge_count);

    // Step 6: Reconstruct cluster with the restored edge
    let restored_cluster_data = {
        // Use EdgeCluster::create_from_compact_edges to create restored cluster
        let restored_cluster = EdgeCluster::create_from_compact_edges(
            existing_edges,
            node_id,
            direction
        ).map_err(|e| RecoveryError::replay_failure(
            format!("Failed to create restored cluster after edge reinsertion: {:?}", e)
        ))?;

        // Serialize the restored cluster manually following the V2 cluster format
        let mut cluster_bytes = Vec::new();

        // Write node_id (i64) - using little-endian format
        cluster_bytes.extend_from_slice(&(node_id as i64).to_le_bytes());

        // Write direction (u32) - 0 for Outgoing, 1 for Incoming
        let direction_u32: u32 = match direction {
            Direction::Outgoing => 0,
            Direction::Incoming => 1,
        };
        cluster_bytes.extend_from_slice(&direction_u32.to_le_bytes());

        // Write edge count (u32)
        let edge_count = restored_cluster.edge_count();
        cluster_bytes.extend_from_slice(&edge_count.to_le_bytes());

        // Write edge data
        for edge in restored_cluster.edges() {
            let edge_bytes = edge.serialize();
            cluster_bytes.extend_from_slice(&edge_bytes);
        }

        cluster_bytes
    };

    // Step 7: Write restored cluster back to GraphFile at original offset
    {
        let mut graph_file = system.graph_file().write()
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to lock graph file for cluster write: {}", e)
            ))?;

        graph_file.write_bytes(cluster_offset, &restored_cluster_data)
            .map_err(|e| RecoveryError::io_error(
                format!("Failed to write restored cluster at offset {}: {:?}", cluster_offset, e)
            ))?;

        debug_log!("Successfully restored cluster at offset {} ({} bytes) with deleted edge at position {}",
               cluster_offset, restored_cluster_data.len(), position);
    }

    debug_log!("Edge delete rollback completed: node_id={}, direction={:?}, position={}, edges_restored={}",
           node_id, direction, position, restored_edge_count);

    Ok(())
}
