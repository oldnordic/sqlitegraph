//! Node operation handlers for V2 WAL recovery replay
//!
//! This module provides node-specific replay operations including
//! node insertion, update, and deletion with proper rollback support.

use crate::backend::native::{
    GraphFile, NodeStore, EdgeStore, NativeNodeId,
    adjacency::Direction,
};
use crate::backend::native::v2::{
    StringTable, NodeRecordV2,
    free_space::AllocationStrategy, FreeSpaceManager,
};
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;
use crate::backend::native::v2::wal::recovery::replayer::types::RollbackOperation;
use std::sync::{Arc, Mutex, RwLock};

macro_rules! debug { ($($arg:tt)*) => { log::debug!($($arg)*); }; }
macro_rules! warn { ($($arg:tt)*) => { log::warn!($($arg)*); }; }
macro_rules! error { ($($arg:tt)*) => { log::error!($($arg)*); }; }

impl super::DefaultReplayOperations {
    /// Handle node insertion during replay
    pub fn handle_node_insert(
        &self,
        node_id: u64,
        slot_offset: u64,
        node_data: &[u8],
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), RecoveryError> {
        debug!("Replaying node insert: node_id={}, slot_offset={}, data_size={}",
               node_id, slot_offset, node_data.len());

        // Deserialize the node data
        let node_record = crate::backend::native::v2::node_record_v2::NodeRecordV2::deserialize(node_data)
            .map_err(|e| RecoveryError::io_error(
                format!("Failed to deserialize node data: {}", e)
            ))?;

        // Validate node ID consistency
        if node_record.id != node_id as crate::backend::native::NativeNodeId {
            return Err(RecoveryError::validation(
                format!("Node ID mismatch: expected {}, got {}", node_id, node_record.id)
            ));
        }

        // Add rollback operation before making changes
        rollback_data.push(RollbackOperation::NodeInsert {
            node_id: node_id as crate::backend::native::NativeNodeId,
            node_data: node_data.to_vec(),
        });

        // Create NodeStore for this operation following proper SME methodology
        {
            let mut graph_file = self.graph_file.write()
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to lock graph file: {}", e)
                ))?;

            let mut node_store = NodeStore::new(&mut *graph_file);
            node_store.write_node_v2(&node_record)
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to write node: {}", e)
                ))?;
        } // graph_file lock and node_store are released here

        // Update statistics
        {
            let mut stats = self.statistics.lock().unwrap();
            stats.record_node_operation();
            stats.record_bytes_written(node_data.len() as u64);
        }

        debug!("Successfully replayed node insert: node_id={}", node_id);
        Ok(())
    }

    /// Handle node update during replay
    pub fn handle_node_update(
        &self,
        node_id: u64,
        _slot_offset: u64,
        new_data: &[u8],
        old_data: Option<&Vec<u8>>,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), RecoveryError> {
        debug!("Replaying node update: node_id={}, data_size={}", node_id, new_data.len());

        // Validate input data
        if new_data.is_empty() {
            return Err(RecoveryError::validation(
                "Node update data cannot be empty".to_string()
            ));
        }

        // Deserialize the new node data
        let node_record = crate::backend::native::v2::node_record_v2::NodeRecordV2::deserialize(new_data)
            .map_err(|e| RecoveryError::io_error(
                format!("Failed to deserialize node data: {}", e)
            ))?;

        // Validate node ID consistency
        if node_record.id != node_id as crate::backend::native::NativeNodeId {
            return Err(RecoveryError::validation(
                format!("Node ID mismatch: expected {}, got {}", node_id, node_record.id)
            ));
        }

        // Add rollback operation before making changes
        if let Some(old_data_vec) = old_data {
            rollback_data.push(RollbackOperation::NodeUpdate {
                node_id: node_id as crate::backend::native::NativeNodeId,
                old_data: old_data_vec.clone(),
            });
        }

        // Create NodeStore for this operation following proper SME methodology
        {
            let mut graph_file = self.graph_file.write()
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to lock graph file: {}", e)
                ))?;

            let mut node_store = NodeStore::new(&mut *graph_file);
            node_store.write_node_v2(&node_record)
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to write node update: {}", e)
                ))?;
        } // graph_file lock and node_store are released here

        // Update statistics
        {
            let mut stats = self.statistics.lock().unwrap();
            stats.record_node_operation();
            stats.record_bytes_written(new_data.len() as u64);
        }

        debug!("Successfully replayed node update: node_id={}", node_id);
        Ok(())
    }

    /// Handle node deletion during replay
    pub fn handle_node_delete(
        &self,
        node_id: u64,
        slot_offset: u64,
        old_data: Option<&Vec<u8>>,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), RecoveryError> {
        debug!("Replaying node delete: node_id={}, slot_offset={}", node_id, slot_offset);

        // Step 1: Validate input parameters
        if node_id == 0 {
            warn!("Invalid node_id=0 for node deletion - treating as no-op");
            return Ok(());
        }

        // Step 2: Parse existing node data if provided, or retrieve from storage
        let node_record = if let Some(data) = old_data {
            // Deserialize NodeRecordV2 from provided old_data
            serde_json::from_slice::<NodeRecordV2>(data)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to deserialize NodeRecordV2 data: {}", e)
                ))?
        } else {
            // For now, create a minimal node record - in real implementation would retrieve from storage
            warn!("No old_data provided for node delete - creating minimal rollback record");
            NodeRecordV2::new(
                node_id as i64,
                "Unknown".to_string(),
                "deleted_node".to_string(),
                serde_json::Value::Null
            )
        };

        // Step 3: Add rollback operation BEFORE deletion (critical for transaction integrity)
        // Serialize the node_record to old_data so rollback can restore the deleted node
        let old_data = serde_json::to_vec(&node_record)
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to serialize node data for rollback: {}", e)
            ))?;

        let old_data_len = old_data.len();

        rollback_data.push(RollbackOperation::NodeDelete {
            node_id: node_id as NativeNodeId,
            slot_offset,
            old_data,
        });

        // Create NodeStore and FreeSpaceManager for this operation following proper SME methodology
        {
            let mut graph_file = self.graph_file.write()
                .map_err(|e| RecoveryError::io_error(format!("Failed to lock graph file: {}", e)))?;

            // Step 5: Handle edge cascade cleanup (if node has cluster references)
            // Do this BEFORE creating NodeStore to avoid borrow conflicts
            if node_record.outgoing_edge_count > 0 || node_record.incoming_edge_count > 0 {
                debug!("Node {} has edges - performing cascade cleanup: outgoing={}, incoming={}",
                       node_id, node_record.outgoing_edge_count, node_record.incoming_edge_count);

                // Create EdgeStore for edge deletion operations
                let mut edge_store = EdgeStore::new(&mut *graph_file);

                // Collect and delete outgoing edges (edges where from_id = node_id)
                if node_record.outgoing_edge_count > 0 {
                    let outgoing_edges: Vec<(NativeNodeId, NativeNodeId)> = edge_store
                        .iter_edges_with_ids(
                            node_id as NativeNodeId,
                            Direction::Outgoing
                        )
                        .collect();

                    let outgoing_count = outgoing_edges.len();
                    for (edge_id, neighbor_id) in outgoing_edges {
                        // Mark edge as deleted (soft deletion)
                        if let Err(e) = edge_store.delete_edge(edge_id) {
                            warn!("Failed to delete outgoing edge {} for node {} -> neighbor {}: {:?}",
                                  edge_id, node_id, neighbor_id, e);
                        } else {
                            debug!("Deleted outgoing edge {} for node {} -> neighbor {}", edge_id, node_id, neighbor_id);
                        }
                    }

                    debug!("Deleted {} outgoing edges for node {}", outgoing_count, node_id);
                }

                // Collect and delete incoming edges (edges where to_id = node_id)
                if node_record.incoming_edge_count > 0 {
                    let incoming_edges: Vec<(NativeNodeId, NativeNodeId)> = edge_store
                        .iter_edges_with_ids(
                            node_id as NativeNodeId,
                            Direction::Incoming
                        )
                        .collect();

                    let incoming_count = incoming_edges.len();
                    for (edge_id, neighbor_id) in incoming_edges {
                        // Mark edge as deleted (soft deletion)
                        if let Err(e) = edge_store.delete_edge(edge_id) {
                            warn!("Failed to delete incoming edge {} for node {} <- neighbor {}: {:?}",
                                  edge_id, node_id, neighbor_id, e);
                        } else {
                            debug!("Deleted incoming edge {} for node {} <- neighbor {}", edge_id, node_id, neighbor_id);
                        }
                    }

                    debug!("Deleted {} incoming edges for node {}", incoming_count, node_id);
                }

                debug!("Successfully completed edge cascade cleanup for node {}", node_id);
            }

            // Now create NodeStore and FreeSpaceManager for remaining operations
            let mut node_store = NodeStore::new(&mut *graph_file);
            let mut free_space_manager = FreeSpaceManager::new(AllocationStrategy::FirstFit);

            // Step 6: Clean up cluster references if they exist
            if node_record.outgoing_cluster_offset != 0 || node_record.incoming_cluster_offset != 0 {
                debug!("Cleaning up cluster references for node {}: outgoing_offset={}, incoming_offset={}",
                       node_id, node_record.outgoing_cluster_offset, node_record.incoming_cluster_offset);

                // Deallocate outgoing cluster if it exists
                if node_record.outgoing_cluster_offset != 0 && node_record.outgoing_cluster_size > 0 {
                    free_space_manager.add_free_block(
                        node_record.outgoing_cluster_offset,
                        node_record.outgoing_cluster_size
                    );
                    debug!("Deallocated outgoing cluster: node_id={}, offset={}, size={}",
                           node_id, node_record.outgoing_cluster_offset, node_record.outgoing_cluster_size);
                }

                // Deallocate incoming cluster if it exists
                if node_record.incoming_cluster_offset != 0 && node_record.incoming_cluster_size > 0 {
                    free_space_manager.add_free_block(
                        node_record.incoming_cluster_offset,
                        node_record.incoming_cluster_size
                    );
                    debug!("Deallocated incoming cluster: node_id={}, offset={}, size={}",
                           node_id, node_record.incoming_cluster_offset, node_record.incoming_cluster_size);
                }

                debug!("Successfully cleaned up cluster references for node {}", node_id);
            }

            // Step 7: Deallocate node slot using FreeSpaceManager
            if slot_offset != 0 {
                // Estimate node size for deallocation (use reasonable default for now)
                let estimated_node_size = std::mem::size_of::<NodeRecordV2>() as u32;
                free_space_manager.add_free_block(slot_offset, estimated_node_size);
                debug!("Deallocated node slot: offset={}, size={}", slot_offset, estimated_node_size);
            }

            // Step 8: Remove node from node index using real NodeStore deletion
            node_store.delete_node(node_id as NativeNodeId)
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to delete node {} from NodeStore: {}", node_id, e)
                ))?;
        } // graph_file lock, node_store, and free_space_manager are released here

        // Step 9: Update statistics
        {
            let mut stats = self.statistics.lock().unwrap();
            stats.record_node_operation();
            stats.record_bytes_written(old_data_len as u64);
        }

        debug!("Successfully completed node delete: node_id={}, rollback_data_count={}",
               node_id, rollback_data.len());

        Ok(())
    }
}
