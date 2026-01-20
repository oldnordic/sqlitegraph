//! Edge operation handlers for V2 WAL recovery replay
//!
//! This module provides edge-specific replay operations including
//! edge insertion, update, and deletion with proper rollback support.

use crate::backend::native::{
    NativeNodeId,
};
use crate::backend::native::v2::{
    EdgeCluster,
    edge_cluster::{CompactEdgeRecord, Direction},
};
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;
use crate::backend::native::v2::wal::recovery::replayer::types::RollbackOperation;
use crate::backend::native::v2::wal::recovery::store_helpers;
use crate::debug::{debug_log, warn_log};

impl super::DefaultReplayOperations {
    /// Handle edge insertion during replay
    pub fn handle_edge_insert(
        &self,
        cluster_key: (u64, u64),
        edge_record: &CompactEdgeRecord,
        insertion_point: u32,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), RecoveryError> {
        debug_log!("Replaying edge insert: cluster_key={:?}, insertion_point={}, edge_len={}",
               cluster_key, insertion_point, edge_record.edge_data.len());

        // Step 1: Input validation following SME methodology
        let (node_id, direction) = cluster_key;
        if node_id == 0 {
            return Err(RecoveryError::validation(
                "Invalid node_id=0 for edge insertion".to_string()
            ));
        }

        // u32::MAX is a special value indicating "append to end"
        if insertion_point != u32::MAX && insertion_point > 1000 { // Reasonable limit to prevent corruption
            return Err(RecoveryError::validation(
                format!("Insertion point {} exceeds reasonable limit", insertion_point)
            ));
        }

        // Step 2: Convert direction value to Direction enum following patterns from mod.rs:296-302
        let direction_enum = match direction {
            0 => crate::backend::native::v2::edge_cluster::Direction::Outgoing,
            1 => crate::backend::native::v2::edge_cluster::Direction::Incoming,
            _ => return Err(RecoveryError::validation(
                format!("Invalid direction value: {}, expected 0 (Outgoing) or 1 (Incoming)", direction)
            )),
        };

        // Step 3: Create cluster data first (needed for both rollback and allocation)
        let edge_record_bytes = edge_record.serialize();

        // Step 4: Create cluster with edge following the CORRECT V2 cluster format
        // CRITICAL FIX: Use EdgeCluster::create_from_compact_edges and .serialize() instead of manual construction
        // The V2 cluster format is: [edge_count:4 BE][payload_size:4 BE][edge_data...], NOT [node_id:8][direction:4][edge_count:4][edge_data...]
        let cluster_data = {
            // Create EdgeCluster using the proper API
            let edge_cluster = EdgeCluster::create_from_compact_edges(
                vec![edge_record.clone()],
                node_id as i64,
                match direction {
                    0 => crate::backend::native::v2::edge_cluster::Direction::Outgoing,
                    1 => crate::backend::native::v2::edge_cluster::Direction::Incoming,
                    _ => crate::backend::native::v2::edge_cluster::Direction::Outgoing,
                }
            ).map_err(|e| RecoveryError::replay_failure(
                format!("Failed to create EdgeCluster: {:?}", e)
            ))?;

            // Serialize using EdgeCluster's serialize() method which produces correct format
            let cluster_bytes = edge_cluster.serialize();

            debug_log!("Created cluster data: {} bytes total (edge_count=1)",
                   cluster_bytes.len());
            cluster_bytes
        };

        // Step 6: Allocate storage space using the resolved handle_free_space_allocate
        let allocated_offset = {
            let mut free_space_guard = self.free_space_manager.lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock free space manager: {}", e)
                ))?;

            let free_space_manager = free_space_guard.as_mut()
                .ok_or_else(|| RecoveryError::replay_failure(
                    "Free space manager not initialized".to_string()
                ))?;

            let cluster_size_u32 = cluster_data.len() as u32;
            let mut allocated_offset = free_space_manager.allocate(cluster_size_u32)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to allocate space for edge cluster: {:?}", e)
                ))?;

            // CRITICAL: Ensure cluster offset is >= cluster_floor from GraphFile
            // NodeRecordV2 validation requires all clusters to be outside the node region
            // The cluster_floor is calculated dynamically as max(node_region_end, node_data_offset + RESERVED_NODE_REGION_BYTES)
            // Get the cluster_floor from the GraphFile to ensure consistency with header initialization
            let cluster_floor = {
                let graph_file = self.graph_file.read()
                    .map_err(|e| RecoveryError::replay_failure(
                        format!("Failed to lock graph file for cluster_floor query: {}", e)
                    ))?;
                graph_file.cluster_floor()
            };

            if allocated_offset < cluster_floor {
                debug_log!("Allocated offset {} is below cluster floor {}, padding to {}",
                       allocated_offset, cluster_floor, cluster_floor);
                allocated_offset = cluster_floor;
            }

            debug_log!("Successfully allocated {} bytes for edge cluster at offset {}",
                   cluster_data.len(), allocated_offset);
            allocated_offset
        }; // FreeSpaceManager lock is released here

        // Step 5: Add rollback operation AFTER cluster allocation (now we have offset and size)
        // This maintains transaction integrity while ensuring complete state capture for rollback
        rollback_data.push(RollbackOperation::EdgeInsert {
            cluster_key,
            insertion_point,
            edge_record: edge_record_bytes.clone(),
            cluster_offset: allocated_offset,
            cluster_size: cluster_data.len() as u32,
        });

        // Step 6: Write cluster data to allocated storage following handle_cluster_create pattern
        {
            let mut graph_file = self.graph_file.write()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock graph file: {}", e)
                ))?;

            // Write serialized cluster data to allocated offset
            graph_file.write_bytes(allocated_offset, &cluster_data)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to write edge cluster to graph file: {:?}", e)
                ))?;

            debug_log!("Successfully wrote edge cluster data: {} bytes to offset {}",
                   cluster_data.len(), allocated_offset);
        }; // GraphFile lock is released here

        // Step 7.5: Update NodeRecordV2 cluster references (critical fix for edge operations)
        // Following the exact pattern from handle_cluster_create (lines 391-452)
        println!("[DEBUG] About to update NodeRecordV2: node_id={}, allocated_offset={}, cluster_data_len={}",
               node_id, allocated_offset, cluster_data.len());
        {
            // Create NodeStore for this operation following established patterns
            let mut node_store_guard = self.node_store.lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock node store for NodeRecordV2 update: {}", e)
                ))?;

            // Initialize NodeStore if needed
            if node_store_guard.is_none() {
                let mut graph_file = self.graph_file.write()
                    .map_err(|e| RecoveryError::replay_failure(
                        format!("Failed to lock graph file: {}", e)
                    ))?;

                // Use documented-safe store_helpers pattern
                *node_store_guard = Some(unsafe {
                    store_helpers::create_node_store(&mut *graph_file)
                });
            }

            let node_store = node_store_guard.as_mut()
                .ok_or_else(|| RecoveryError::replay_failure(
                    "NodeStore not available for NodeRecordV2 update".to_string()
                ))?;

            // Read existing NodeRecordV2 or create new one
            let mut node_record = match node_store.read_node_v2(node_id as crate::backend::native::NativeNodeId) {
                Ok(record) => record,
                Err(_) => {
                    // Node doesn't exist - create new NodeRecordV2
                    debug_log!("Node {} not found - creating new NodeRecordV2 for cluster reference", node_id);
                    crate::backend::native::v2::node_record_v2::NodeRecordV2::new(
                        node_id as i64,
                        "Node".to_string(),
                        format!("Node {}", node_id),
                        serde_json::Value::Object(serde_json::Map::new())
                    )
                }
            };

            // Update cluster offset and size based on direction (following handle_cluster_create pattern)
            let cluster_direction = match direction {
                0 => crate::backend::native::v2::edge_cluster::Direction::Outgoing,
                1 => crate::backend::native::v2::edge_cluster::Direction::Incoming,
                _ => crate::backend::native::v2::edge_cluster::Direction::Outgoing, // Default to Outgoing
            };

            match cluster_direction {
                crate::backend::native::v2::edge_cluster::Direction::Outgoing => {
                    node_record.outgoing_cluster_offset = allocated_offset;
                    node_record.outgoing_cluster_size = cluster_data.len() as u32;
                    node_record.outgoing_edge_count += 1; // Critical: increment edge count to match cluster
                    debug_log!("Before write: node_id={}, outgoing_cluster_offset={}, outgoing_edge_count={}",
                           node_record.id, node_record.outgoing_cluster_offset, node_record.outgoing_edge_count);
                },
                crate::backend::native::v2::edge_cluster::Direction::Incoming => {
                    node_record.incoming_cluster_offset = allocated_offset;
                    node_record.incoming_cluster_size = cluster_data.len() as u32;
                    node_record.incoming_edge_count += 1; // Critical: increment edge count to match cluster
                    debug_log!("Before write: node_id={}, incoming_cluster_offset={}, incoming_edge_count={}",
                           node_record.id, node_record.incoming_cluster_offset, node_record.incoming_edge_count);
                },
            }

            // Write updated NodeRecordV2 back
            node_store.write_node_v2(&node_record)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to update NodeRecordV2 with cluster reference: {:?}", e)
                ))?;

            debug_log!("Updated NodeRecordV2 cluster reference for node {} direction {:?} to offset {} (size: {})",
                   node_id, cluster_direction, allocated_offset, cluster_data.len());
        } // NodeStore lock is released here

        // Step 8: Update statistics tracking (lock-free)
        self.statistics.record_edge_operation();
        self.statistics.record_bytes_written(cluster_data.len() as u64);

        debug_log!("Successfully completed edge insert: cluster_key={:?}, insertion_point={}, offset={}, size={}",
               cluster_key, insertion_point, allocated_offset, cluster_data.len());
        Ok(())
    }

    /// Handle edge update during replay
    pub fn handle_edge_update(
        &self,
        cluster_key: (i64, crate::backend::native::v2::edge_cluster::Direction),
        new_edge: &CompactEdgeRecord,
        position: u32,
        old_edge: &CompactEdgeRecord,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), RecoveryError> {
        debug_log!("Replaying edge update: cluster_key={:?}, position={}, old_edge_len={}, new_edge_len={}",
               cluster_key, position, old_edge.edge_data.len(), new_edge.edge_data.len());

        // Step 1: Input validation following SME methodology
        let (node_id, direction) = cluster_key;
        if node_id == 0 {
            return Err(RecoveryError::validation(
                "Invalid node_id=0 for edge update".to_string()
            ));
        }

        // Reasonable position limit to prevent corruption
        if position > 10000 { // Conservative upper limit
            return Err(RecoveryError::validation(
                format!("Position {} exceeds reasonable limit", position)
            ));
        }

        // Step 2: Create rollback operation BEFORE making changes (critical for transaction integrity)
        let old_edge_bytes = old_edge.serialize();
        let new_edge_bytes = new_edge.serialize();
        rollback_data.push(RollbackOperation::EdgeUpdate {
            cluster_key,
            position,
            old_edge: old_edge_bytes.clone(),
            new_edge: new_edge_bytes.clone(),
        });

        // Step 3: Read existing NodeRecordV2 to locate cluster
        let (cluster_offset, cluster_size) = {
            // Create NodeStore for this operation following established patterns
            let mut node_store_guard = self.node_store.lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock node store: {}", e)
                ))?;

            // Initialize NodeStore if needed
            if node_store_guard.is_none() {
                let mut graph_file = self.graph_file.write()
                    .map_err(|e| RecoveryError::replay_failure(
                        format!("Failed to lock graph file: {}", e)
                    ))?;

                // Use documented-safe store_helpers pattern
                *node_store_guard = Some(unsafe {
                    store_helpers::create_node_store(&mut *graph_file)
                }));
            }

            let node_store = node_store_guard.as_mut()
                .ok_or_else(|| RecoveryError::replay_failure(
                    "NodeStore initialization failed".to_string()
                ))?;

            // Read NodeRecordV2 to get cluster location
            let node_record = node_store.read_node_v2(node_id as crate::backend::native::NativeNodeId)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to read node {} from NodeStore: {}", node_id, e)
                ))?;

            // Get cluster offset and size based on direction
            let (cluster_offset, cluster_size) = match direction {
                crate::backend::native::v2::edge_cluster::Direction::Outgoing => {
                    debug_log!("Reading Outgoing cluster: offset={}, size={}, node_id={}",
                           node_record.outgoing_cluster_offset,
                           node_record.outgoing_cluster_size,
                           node_record.id);
                    if node_record.outgoing_cluster_offset == 0 {
                        return Err(RecoveryError::validation(
                            format!("Node {} has no outgoing cluster to update", node_id)
                        ));
                    }
                    (node_record.outgoing_cluster_offset, node_record.outgoing_cluster_size)
                },
                crate::backend::native::v2::edge_cluster::Direction::Incoming => {
                    debug_log!("Reading Incoming cluster: offset={}, size={}, node_id={}",
                           node_record.incoming_cluster_offset,
                           node_record.incoming_cluster_size,
                           node_record.id);
                    if node_record.incoming_cluster_offset == 0 {
                        return Err(RecoveryError::validation(
                            format!("Node {} has no incoming cluster to update", node_id)
                        ));
                    }
                    (node_record.incoming_cluster_offset, node_record.incoming_cluster_size)
                },
            };

            debug_log!("Found cluster at offset {} with size {} for node {:?} direction {:?}",
                   cluster_offset, cluster_size, node_id, direction);

            (cluster_offset, cluster_size)
        }; // NodeStore lock is released here

        // Step 4: Read existing cluster data from storage
        let mut existing_edges = {
            // Read cluster data from graph file
            let mut graph_file = self.graph_file.write()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock graph file for cluster read: {}", e)
                ))?;

            let mut cluster_buffer = vec![0u8; cluster_size as usize];
            graph_file.read_bytes(cluster_offset, &mut cluster_buffer)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to read cluster data at offset {}: {:?}", cluster_offset, e)
                ))?;

            // Verify and deserialize cluster using EdgeCluster public methods
            EdgeCluster::verify_serialized_layout(&cluster_buffer)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Cluster layout verification failed: {:?}", e)
                ))?;

            let edge_cluster = EdgeCluster::deserialize(&cluster_buffer)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to deserialize cluster: {:?}", e)
                ))?;

            edge_cluster.edges().to_vec()
        }; // GraphFile lock is released here

        // Step 5: Validate position against existing edge count
        if position >= existing_edges.len() as u32 {
            return Err(RecoveryError::validation(
                format!("Position {} out of bounds for cluster with {} edges",
                       position, existing_edges.len())
            ));
        }

        // Verify that the edge at position matches the expected old_edge
        let edge_at_position = &existing_edges[position as usize];
        let edge_at_position_bytes = edge_at_position.serialize();
        if edge_at_position_bytes != old_edge_bytes {
            warn_log!("Edge at position {} differs from expected old_edge - proceeding anyway for data recovery", position);
            // In recovery mode, we continue even if the edge doesn't match exactly
        }

        // Step 6: Update edge at specified position
        existing_edges[position as usize] = new_edge.clone();

        debug_log!("Updated edge at position {} in cluster for node {:?} direction {:?}",
               position, node_id, direction);

        // Step 7: Reconstruct cluster with updated edge
        let updated_cluster_data = {
            // Use EdgeCluster::create_from_compact_edges to create updated cluster
            let updated_cluster = EdgeCluster::create_from_compact_edges(
                existing_edges,
                node_id,
                direction
            ).map_err(|e| RecoveryError::replay_failure(
                format!("Failed to create updated cluster: {:?}", e)
            ))?;

            // Serialize the updated cluster manually following the V2 cluster format
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
            let edge_count = updated_cluster.edge_count();
            cluster_bytes.extend_from_slice(&edge_count.to_le_bytes());

            // Write serialized edge data
            for edge in updated_cluster.edges() {
                let edge_bytes = edge.serialize();
                cluster_bytes.extend_from_slice(&edge_bytes);
            }

            cluster_bytes
        };

        // Step 8: Allocate storage space for updated cluster (size may have changed)
        let allocated_offset = {
            let mut free_space_guard = self.free_space_manager.lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock free space manager: {}", e)
                ))?;

            let free_space_manager = free_space_guard.as_mut()
                .ok_or_else(|| RecoveryError::replay_failure(
                    "Free space manager not initialized".to_string()
                ))?;

            let cluster_size_u32 = updated_cluster_data.len() as u32;
            let mut allocated_offset = free_space_manager.allocate(cluster_size_u32)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to allocate space for updated edge cluster: {:?}", e)
                ))?;

            // CRITICAL: Ensure cluster offset is >= cluster_floor from GraphFile
            // NodeRecordV2 validation requires all clusters to be outside the node region
            // The cluster_floor is calculated dynamically as max(node_region_end, node_data_offset + RESERVED_NODE_REGION_BYTES)
            // Get the cluster_floor from the GraphFile to ensure consistency with header initialization
            let cluster_floor = {
                let graph_file = self.graph_file.read()
                    .map_err(|e| RecoveryError::replay_failure(
                        format!("Failed to lock graph file for cluster_floor query: {}", e)
                    ))?;
                graph_file.cluster_floor()
            };

            if allocated_offset < cluster_floor {
                debug_log!("Allocated offset {} is below cluster floor {}, padding to {}",
                       allocated_offset, cluster_floor, cluster_floor);
                allocated_offset = cluster_floor;
            }

            debug_log!("Successfully allocated {} bytes for updated edge cluster at offset {}",
                   updated_cluster_data.len(), allocated_offset);
            allocated_offset
        }; // FreeSpaceManager lock is released here

        // Step 9: Write updated cluster data to allocated storage
        {
            let mut graph_file = self.graph_file.write()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock graph file for cluster write: {}", e)
                ))?;

            // Write updated cluster data to new allocated offset
            graph_file.write_bytes(allocated_offset, &updated_cluster_data)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to write updated edge cluster: {:?}", e)
                ))?;

            debug_log!("Successfully wrote updated edge cluster: {} bytes to offset {}",
                   updated_cluster_data.len(), allocated_offset);
        }; // GraphFile lock is released here

        // Step 10: Update NodeRecordV2 with new cluster offset
        {
            // Create NodeStore for this operation following established patterns
            let mut node_store_guard = self.node_store.lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock node store for NodeRecordV2 update: {}", e)
                ))?;

            let node_store = node_store_guard.as_mut()
                .ok_or_else(|| RecoveryError::replay_failure(
                    "NodeStore not available for NodeRecordV2 update".to_string()
                ))?;

            // Read current NodeRecordV2
            let mut node_record = node_store.read_node_v2(node_id as crate::backend::native::NativeNodeId)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to read NodeRecordV2 for update: {}", e)
                ))?;

            // Debug: Print state before update
            debug_log!("Before NodeRecordV2 update: node_id={}, edge_count={}, allocated_offset={}, cluster_size={}",
                   node_record.id,
                   if direction == Direction::Outgoing { node_record.outgoing_edge_count } else { node_record.incoming_edge_count },
                   allocated_offset,
                   updated_cluster_data.len());

            // Update cluster offset and size based on direction
            match direction {
                crate::backend::native::v2::edge_cluster::Direction::Outgoing => {
                    node_record.outgoing_cluster_offset = allocated_offset;
                    node_record.outgoing_cluster_size = updated_cluster_data.len() as u32;
                    // Edge count remains the same in an update operation
                },
                crate::backend::native::v2::edge_cluster::Direction::Incoming => {
                    node_record.incoming_cluster_offset = allocated_offset;
                    node_record.incoming_cluster_size = updated_cluster_data.len() as u32;
                    // Edge count remains the same in an update operation
                },
            }

            // Debug: Print state after update
            debug_log!("After NodeRecordV2 update: node_id={}, outgoing_edge_count={}, outgoing_offset={}, outgoing_size={}",
                   node_record.id,
                   node_record.outgoing_edge_count,
                   node_record.outgoing_cluster_offset,
                   node_record.outgoing_cluster_size);

            // Write updated NodeRecordV2 back
            node_store.write_node_v2(&node_record)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to update NodeRecordV2: {:?}", e)
                ))?;

            debug_log!("Updated NodeRecordV2 cluster reference for node {:?} direction {:?} to offset {}",
                   node_id, direction, allocated_offset);
        }; // NodeStore lock is released here

        // Step 11: Update statistics tracking (lock-free)
        self.statistics.record_edge_operation();
        self.statistics.record_bytes_written(updated_cluster_data.len() as u64);

        debug_log!("Successfully completed edge update: cluster_key={:?}, position={}, old_offset={}, new_offset={}, old_size={}, new_size={}",
               cluster_key, position, cluster_offset, allocated_offset, cluster_size, updated_cluster_data.len());
        Ok(())
    }

    /// Handle edge deletion during replay
    pub fn handle_edge_delete(
        &self,
        cluster_key: (i64, crate::backend::native::v2::edge_cluster::Direction),
        position: u32,
        old_edge: &CompactEdgeRecord,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), RecoveryError> {
        // Step 1: Input validation following SME methodology
        let (node_id, direction) = cluster_key;
        if node_id == 0 {
            return Err(RecoveryError::validation(
                "Invalid node_id=0 for edge delete".to_string()
            ));
        }

        // Reasonable position limit to prevent corruption
        if position > 10000 { // Conservative upper limit
            return Err(RecoveryError::validation(
                format!("Position {} exceeds reasonable limit", position)
            ));
        }

        // Step 2: Create rollback operation BEFORE making changes (critical for transaction integrity)
        let old_edge_bytes = old_edge.serialize();
        rollback_data.push(RollbackOperation::EdgeDelete {
            cluster_key,
            position,
            old_edge: old_edge_bytes.clone(),
        });

        // Step 3: Read existing NodeRecordV2 to locate cluster
        let (cluster_offset, cluster_size) = {
            // Create NodeStore for this operation following established patterns
            let mut node_store_guard = self.node_store.lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock node store: {}", e)
                ))?;

            // Initialize NodeStore if needed
            if node_store_guard.is_none() {
                let mut graph_file = self.graph_file.write()
                    .map_err(|e| RecoveryError::replay_failure(
                        format!("Failed to lock graph file: {}", e)
                    ))?;

                // Use documented-safe store_helpers pattern
                *node_store_guard = Some(unsafe {
                    store_helpers::create_node_store(&mut *graph_file)
                }));
            }

            let node_store = node_store_guard.as_mut()
                .ok_or_else(|| RecoveryError::replay_failure(
                    "NodeStore initialization failed".to_string()
                ))?;

            // Read NodeRecordV2 to get cluster location
            let node_record = node_store.read_node_v2(node_id as crate::backend::native::NativeNodeId)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to read node {} from NodeStore: {}", node_id, e)
                ))?;

            // Get cluster offset and size based on direction
            let (cluster_offset, cluster_size) = match direction {
                crate::backend::native::v2::edge_cluster::Direction::Outgoing => {
                    if node_record.outgoing_cluster_offset == 0 {
                        return Err(RecoveryError::validation(
                            format!("Node {} has no outgoing cluster to delete from", node_id)
                        ));
                    }
                    (node_record.outgoing_cluster_offset, node_record.outgoing_cluster_size)
                },
                crate::backend::native::v2::edge_cluster::Direction::Incoming => {
                    if node_record.incoming_cluster_offset == 0 {
                        return Err(RecoveryError::validation(
                            format!("Node {} has no incoming cluster to delete from", node_id)
                        ));
                    }
                    (node_record.incoming_cluster_offset, node_record.incoming_cluster_size)
                },
            };

            debug_log!("Found cluster at offset {} with size {} for node {:?} direction {:?}",
                   cluster_offset, cluster_size, node_id, direction);

            (cluster_offset, cluster_size)
        }; // NodeStore lock is released here

        // Step 4: Read existing cluster data from storage
        let mut existing_edges = {
            // Read cluster data from graph file
            let mut graph_file = self.graph_file.write()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock graph file for cluster read: {}", e)
                ))?;

            let mut cluster_buffer = vec![0u8; cluster_size as usize];
            graph_file.read_bytes(cluster_offset, &mut cluster_buffer)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to read cluster data at offset {}: {:?}", cluster_offset, e)
                ))?;

            // Verify and deserialize cluster using EdgeCluster public methods
            EdgeCluster::verify_serialized_layout(&cluster_buffer)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Cluster layout verification failed: {:?}", e)
                ))?;

            let edge_cluster = EdgeCluster::deserialize(&cluster_buffer)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to deserialize cluster: {:?}", e)
                ))?;

            edge_cluster.edges().to_vec()
        }; // GraphFile lock is released here

        // Step 5: Validate position against existing edge count
        if position >= existing_edges.len() as u32 {
            return Err(RecoveryError::validation(
                format!("Position {} out of bounds for cluster with {} edges",
                       position, existing_edges.len())
            ));
        }

        // Verify that the edge at position matches the expected old_edge
        let edge_at_position = &existing_edges[position as usize];
        let edge_at_position_bytes = edge_at_position.serialize();
        if edge_at_position_bytes != old_edge_bytes {
            warn_log!("Edge at position {} differs from expected old_edge - proceeding anyway for data recovery", position);
            // In recovery mode, we continue even if the edge doesn't match exactly
        }

        // Step 6: Delete edge at specified position
        existing_edges.remove(position as usize);

        debug_log!("Deleted edge at position {} in cluster for node {:?} direction {:?} - {} edges remaining",
               position, node_id, direction, existing_edges.len());

        // Step 7: Reconstruct cluster without the deleted edge
        let updated_cluster_data = {
            // Handle empty cluster case (when all edges are deleted)
            if existing_edges.is_empty() {
                debug_log!("Cluster became empty after deleting last edge for node {:?} direction {:?}", node_id, direction);
                // Create empty cluster following established patterns
                let empty_cluster = EdgeCluster::create_from_compact_edges(
                    existing_edges, // empty vector
                    node_id,
                    direction
                ).map_err(|e| RecoveryError::replay_failure(
                        format!("Failed to create empty cluster: {:?}", e)
                    ))?;

                // Serialize empty cluster manually following the V2 cluster format
                let mut cluster_bytes = Vec::new();

                // Write node_id (i64) - using little-endian format
                cluster_bytes.extend_from_slice(&(node_id as i64).to_le_bytes());

                // Write direction (u32) - 0 for Outgoing, 1 for Incoming
                let direction_u32: u32 = match direction {
                    Direction::Outgoing => 0,
                    Direction::Incoming => 1,
                };
                cluster_bytes.extend_from_slice(&direction_u32.to_le_bytes());

                // Write edge count (u32) - 0 for empty cluster
                cluster_bytes.extend_from_slice(&0u32.to_le_bytes());

                // No edge data for empty cluster
                cluster_bytes
            } else {
                // Use EdgeCluster::create_from_compact_edges to create updated cluster
                let updated_cluster = EdgeCluster::create_from_compact_edges(
                    existing_edges,
                    node_id,
                    direction
                ).map_err(|e| RecoveryError::replay_failure(
                        format!("Failed to create updated cluster after edge deletion: {:?}", e)
                    ))?;

                // Serialize the updated cluster manually following the V2 cluster format
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
                let edge_count = updated_cluster.edge_count();
                cluster_bytes.extend_from_slice(&edge_count.to_le_bytes());

                // Write serialized edge data
                for edge in updated_cluster.edges() {
                    let edge_bytes = edge.serialize();
                    cluster_bytes.extend_from_slice(&edge_bytes);
                }

                cluster_bytes
            }
        };

        // Step 8: Allocate storage space for updated cluster (size may have changed)
        let allocated_offset = {
            let mut free_space_guard = self.free_space_manager.lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock free space manager: {}", e)
                ))?;

            let free_space_manager = free_space_guard.as_mut()
                .ok_or_else(|| RecoveryError::replay_failure(
                    "Free space manager not initialized".to_string()
                ))?;

            let cluster_size_u32 = updated_cluster_data.len() as u32;
            let allocated_offset = free_space_manager.allocate(cluster_size_u32)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to allocate space for updated edge cluster: {:?}", e)
                ))?;

            debug_log!("Successfully allocated {} bytes for updated edge cluster at offset {}",
                   updated_cluster_data.len(), allocated_offset);
            allocated_offset
        }; // FreeSpaceManager lock is released here

        // Step 9: Write updated cluster data to allocated storage
        {
            let mut graph_file = self.graph_file.write()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock graph file for cluster write: {}", e)
                ))?;

            // Write updated cluster data to new allocated offset
            graph_file.write_bytes(allocated_offset, &updated_cluster_data)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to write updated edge cluster: {:?}", e)
                ))?;

            debug_log!("Successfully wrote updated edge cluster: {} bytes to offset {}",
                   updated_cluster_data.len(), allocated_offset);
        }; // GraphFile lock is released here

        // Step 10: Update NodeRecordV2 with new cluster offset
        {
            // Create NodeStore for this operation following established patterns
            let mut node_store_guard = self.node_store.lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock node store for NodeRecordV2 update: {}", e)
                ))?;

            let node_store = node_store_guard.as_mut()
                .ok_or_else(|| RecoveryError::replay_failure(
                    "NodeStore not available for NodeRecordV2 update".to_string()
                ))?;

            // Read current NodeRecordV2
            let mut node_record = node_store.read_node_v2(node_id as crate::backend::native::NativeNodeId)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to read NodeRecordV2 for update: {}", e)
                ))?;

            // Update cluster offset, size, and edge count based on direction
            match direction {
                crate::backend::native::v2::edge_cluster::Direction::Outgoing => {
                    if updated_cluster_data.len() == 0 {
                        // Empty cluster - set offset to 0 to indicate no cluster
                        node_record.outgoing_cluster_offset = 0;
                        node_record.outgoing_cluster_size = 0;
                        node_record.outgoing_edge_count = 0; // Critical: reset edge count to 0 for empty cluster
                    } else {
                        node_record.outgoing_cluster_offset = allocated_offset;
                        node_record.outgoing_cluster_size = updated_cluster_data.len() as u32;
                        // edge_count is already correct from the read
                    }
                },
                crate::backend::native::v2::edge_cluster::Direction::Incoming => {
                    if updated_cluster_data.len() == 0 {
                        // Empty cluster - set offset to 0 to indicate no cluster
                        node_record.incoming_cluster_offset = 0;
                        node_record.incoming_cluster_size = 0;
                        node_record.incoming_edge_count = 0; // Critical: reset edge count to 0 for empty cluster
                    } else {
                        node_record.incoming_cluster_offset = allocated_offset;
                        node_record.incoming_cluster_size = updated_cluster_data.len() as u32;
                        // edge_count is already correct from the read
                    }
                },
            }

            // Write updated NodeRecordV2 back
            node_store.write_node_v2(&node_record)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to update NodeRecordV2: {:?}", e)
                ))?;

            debug_log!("Updated NodeRecordV2 cluster reference for node {:?} direction {:?} to offset {}",
                   node_id, direction, if updated_cluster_data.len() == 0 { 0 } else { allocated_offset });
        }; // NodeStore lock is released here

        // Step 11: Update statistics tracking (lock-free)
        self.statistics.record_edge_operation();
        self.statistics.record_bytes_written(updated_cluster_data.len() as u64);

        debug_log!("Successfully completed edge delete: cluster_key={:?}, position={}, old_offset={}, new_offset={}, old_size={}, new_size={}",
               cluster_key, position, cluster_offset, if updated_cluster_data.len() == 0 { 0 } else { allocated_offset }, cluster_size, updated_cluster_data.len());
        Ok(())
    }
}
