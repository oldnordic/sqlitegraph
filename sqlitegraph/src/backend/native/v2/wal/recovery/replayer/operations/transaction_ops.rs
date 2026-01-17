//! Transaction and rollback helpers for V2 WAL recovery replay
//!
//! This module provides transaction-specific replay operations including
//! string insertion, cluster creation, free space management, and header updates.

use crate::backend::native::v2::{
    EdgeCluster,
    edge_cluster::Direction,
    free_space::MIN_BLOCK_SIZE,
};
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;
use crate::backend::native::v2::wal::recovery::replayer::types::RollbackOperation;
use crate::debug::{debug_log, warn_log};

impl super::DefaultReplayOperations {
    /// Handle string insertion during replay
    pub fn handle_string_insert(
        &self,
        string_id: u64,
        string_value: &str,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), RecoveryError> {
        debug_log!("Replaying string insert: string_id={}, value='{}'", string_id, string_value);

        // Initialize string table if needed
        {
            let mut string_table_guard = self.string_table.lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock string table: {}", e)
                ))?;

            // Insert the string into the string table using correct API
            let _offset = string_table_guard.get_or_add_offset(string_value)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to insert string into string table: {}", e)
                ))?;
        }

        // Add rollback operation
        let rollback_op = RollbackOperation::StringInsert {
            string_id,
            string_value: string_value.to_string(),
        };
        rollback_data.push(rollback_op);

        // Update statistics
        {
            let mut stats = self.statistics.lock().unwrap();
            stats.record_string_operation();
            stats.record_bytes_written(string_value.len() as u64);
        }

        debug_log!("Successfully replayed string insert: string_id={}", string_id);
        Ok(())
    }

    /// Handle cluster creation during replay
    pub fn handle_cluster_create(
        &self,
        node_id: u64,
        direction: Direction,
        cluster_offset: u64,
        cluster_size: u64,
        edge_data: &[u8],
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), RecoveryError> {
        debug_log!("Replaying cluster create: node_id={}, direction={:?}, cluster_offset={}, cluster_size={}",
               node_id, direction, cluster_offset, cluster_size);

        // Step 1: Input validation following SME methodology
        if node_id == 0 {
            warn_log!("Invalid node_id=0 for cluster creation - treating as no-op");
            return Ok(());
        }

        // Validate parameter consistency
        if cluster_size as usize != edge_data.len() {
            return Err(RecoveryError::validation(
                format!("Cluster size mismatch: expected {} bytes, got {} bytes", cluster_size, edge_data.len())
            ));
        }

        // Validate cluster offset for reasonable bounds
        if cluster_offset == 0 {
            return Err(RecoveryError::validation(
                "Cluster offset cannot be 0 for valid cluster creation".to_string()
            ));
        }

        // Step 2: Verify data integrity using EdgeCluster API (from SME research)
        EdgeCluster::verify_serialized_layout(edge_data)
            .map_err(|e| RecoveryError::replay_failure(
                format!("Cluster data integrity verification failed: {:?}", e)
            ))?;

        // Step 3: Add rollback operation BEFORE making changes (critical for transaction integrity)
        rollback_data.push(RollbackOperation::ClusterCreate {
            node_id,
            direction,
            cluster_offset,
            cluster_size,
            cluster_data: edge_data.to_vec(),
        });

        // Step 4: Atomic cluster creation with proper resource management
        {
            let mut graph_file = self.graph_file.write()
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to lock graph file: {}", e)
                ))?;

            // Step 5: Write cluster data directly to graph file
            graph_file.write_bytes(cluster_offset, edge_data)
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to write cluster data to graph file: {}", e)
                ))?;

            debug_log!("Successfully wrote cluster data for node {} at offset {} ({} bytes)",
                   node_id, cluster_offset, edge_data.len());
        } // graph_file lock is released here

        // Step 6: Update NodeRecordV2 cluster references (critical for edge operations)
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

                // Use unsafe static lifetime pattern following established approach in operations.rs
                *node_store_guard = Some(crate::backend::native::NodeStore::new(unsafe {
                    std::mem::transmute::<&mut _, &'static mut _>(&mut *graph_file)
                }));
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

            // Update cluster offset and size based on direction (following handle_edge_delete pattern)
            match direction {
                crate::backend::native::v2::edge_cluster::Direction::Outgoing => {
                    node_record.outgoing_cluster_offset = cluster_offset;
                    node_record.outgoing_cluster_size = cluster_size as u32;
                },
                crate::backend::native::v2::edge_cluster::Direction::Incoming => {
                    node_record.incoming_cluster_offset = cluster_offset;
                    node_record.incoming_cluster_size = cluster_size as u32;
                },
            }

            // Write updated NodeRecordV2 back
            node_store.write_node_v2(&node_record)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to update NodeRecordV2 with cluster reference: {:?}", e)
                ))?;

            debug_log!("Updated NodeRecordV2 cluster reference for node {} direction {:?} to offset {} (size: {})",
                   node_id, direction, cluster_offset, cluster_size);
        } // NodeStore lock is released here

        // Step 7: Update statistics tracking
        {
            let mut stats = self.statistics.lock().unwrap();
            stats.record_edge_operation();
            stats.record_bytes_written(edge_data.len() as u64);
        }

        debug_log!("Successfully completed cluster create: node_id={}, direction={:?}, offset={}, size={}",
               node_id, direction, cluster_offset, edge_data.len());
        Ok(())
    }

    /// Handle free space allocation during replay
    pub fn handle_free_space_allocate(
        &self,
        block_offset: u64,
        block_size: u64,
        block_type: u8,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), RecoveryError> {
        debug_log!("Replaying free space allocate: block_offset={}, block_size={}, block_type={}",
               block_offset, block_size, block_type);

        // Step 1: Input validation following SME methodology
        if block_size == 0 {
            return Err(RecoveryError::validation(
                "Block size cannot be 0 for free space allocation".to_string()
            ));
        }

        // Validate block_size against minimum requirements (from research doc line 74)
        if block_size < 32 {
            return Err(RecoveryError::validation(
                format!("Block size {} is below minimum required size of 32 bytes", block_size)
            ));
        }

        // Convert block_size: u64 → u32 for FreeSpaceManager API
        let block_size_u32 = block_size as u32;
        if block_size_u32 as u64 != block_size {
            return Err(RecoveryError::validation(
                format!("Block size {} exceeds u32 maximum value", block_size)
            ));
        }

        // Step 2: Add rollback operation BEFORE making changes (critical for transaction integrity)
        // Note: Following research recommendation (line 167-170), we use allocated offset for rollback
        // The actual allocation offset will be determined by FreeSpaceManager::allocate()
        rollback_data.push(RollbackOperation::FreeSpaceAllocate {
            block_offset: 0, // Placeholder - will be updated with actual allocated offset
            block_size,
            block_type,
        });

        // Step 3: Perform actual allocation using FreeSpaceManager
        let allocated_offset = {
            let mut free_space_guard = self.free_space_manager.lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock free space manager: {}", e)
                ))?;

            let free_space_manager = free_space_guard.as_mut()
                .ok_or_else(|| RecoveryError::replay_failure(
                    "Free space manager not initialized".to_string()
                ))?;

            // Use FreeSpaceManager::allocate() API (research doc line 49)
            let allocated_offset = free_space_manager.allocate(block_size_u32)
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Free space allocation failed: {:?}", e)
                ))?;

            debug_log!("Successfully allocated {} bytes at offset {} (type: {})",
                   block_size, allocated_offset, block_type);
            allocated_offset
        }; // FreeSpaceManager lock is released here

        // Step 4: Update rollback data with actual allocated offset
        if let Some(last_operation) = rollback_data.last_mut() {
            if let RollbackOperation::FreeSpaceAllocate { block_offset, .. } = last_operation {
                *block_offset = allocated_offset;
            }
        }

        // Step 5: Update statistics tracking
        {
            let mut stats = self.statistics.lock().unwrap();
            stats.record_free_space_operation();
            stats.record_bytes_written(block_size);
        }

        debug_log!("Successfully completed free space allocate: offset={}, size={}, type={}",
               allocated_offset, block_size, block_type);
        Ok(())
    }

    /// Handle free space deallocation during replay
    pub fn handle_free_space_deallocate(
        &self,
        block_offset: u64,
        block_size: u64,
        block_type: u8,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), RecoveryError> {
        // Step 1: Input validation following SME methodology
        if block_offset == 0 {
            return Err(RecoveryError::validation(
                "Invalid block_offset=0 for free space deallocate".to_string()
            ));
        }

        if block_size == 0 {
            return Err(RecoveryError::validation(
                "Invalid block_size=0 for free space deallocate".to_string()
            ));
        }

        // Check minimum block size requirement from FreeSpaceManager
        if block_size < MIN_BLOCK_SIZE as u64 {
            return Err(RecoveryError::validation(
                format!("Block size {} below MIN_BLOCK_SIZE ({})", block_size, MIN_BLOCK_SIZE)
            ));
        }

        // Validate block_type is in valid range (0-255)
        // All values are currently valid, but we document this for future type restrictions
        if block_type > 5 {
            // Future types may be reserved, for now accept all values 0-255
            debug_log!("Unusual block_type={} for deallocation (accepted but may indicate WAL corruption)", block_type);
        }

        // Step 2: Create rollback operation BEFORE making changes (critical for transaction integrity)
        rollback_data.push(RollbackOperation::FreeSpaceDeallocate {
            block_offset,
            block_size,
            block_type,
        });

        debug_log!("Creating rollback data for FreeSpaceDeallocate: offset={}, size={}, type={}",
               block_offset, block_size, block_type);

        // Step 3: Perform deallocation using FreeSpaceManager::add_free_block()
        {
            // Lock FreeSpaceManager for thread-safe access
            let mut free_space_guard = self.free_space_manager.lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock free space manager: {}", e)
                ))?;

            let free_space_manager = free_space_guard.as_mut()
                .ok_or_else(|| RecoveryError::replay_failure(
                    "Free space manager not initialized".to_string()
                ))?;

            // Add block back to free list using FreeSpaceManager API
            // Note: FreeSpaceManager::add_free_block() handles:
            // - Minimum block size validation
            // - Fragmentation management via try_merge_adjacent_blocks()
            // - Statistics tracking (total_deallocations, total_deallocated_bytes)
            free_space_manager.add_free_block(block_offset, block_size as u32);

            debug_log!("Successfully deallocated block at offset {} ({} bytes, type {})",
                   block_offset, block_size, block_type);
        } // FreeSpaceManager lock is released here

        // Step 4: Update replay statistics
        {
            let mut stats_guard = self.statistics.lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock statistics: {}", e)
                ))?;

            stats_guard.record_free_space_operation();
        }

        debug_log!("Free space deallocation replay completed: offset={}, size={}, type={}",
               block_offset, block_size, block_type);

        Ok(())
    }

    /// Handle header update during replay
    ///
    /// Updates the graph file header with new data during WAL replay.
    /// This operation ensures that header modifications (such as metadata
    /// updates, version changes, or flag modifications) are properly applied
    /// during recovery.
    ///
    /// # Arguments
    /// * `header_offset` - Byte offset in the file where the header data starts
    /// * `new_data` - New header data to write
    /// * `old_data` - Previous header data (for rollback purposes)
    /// * `rollback_data` - Accumulator for rollback operations
    ///
    /// # Returns
    /// * `Ok(())` if header update succeeds
    /// * `Err(RecoveryError)` if the update fails
    pub fn handle_header_update(
        &self,
        header_offset: u64,
        new_data: &[u8],
        old_data: Option<&[u8]>,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), RecoveryError> {
        debug_log!("Replaying header update: offset={}, data_size={}", header_offset, new_data.len());

        // Step 1: Input validation
        // File: sqlitegraph/src/backend/native/constants.rs
        // HEADER_SIZE is defined as the size of the file header region
        use crate::backend::native::constants::HEADER_SIZE;

        if header_offset >= HEADER_SIZE as u64 {
            return Err(RecoveryError::validation(
                format!("Header offset {} exceeds header region size {}", header_offset, HEADER_SIZE)
            ));
        }

        let end_offset = header_offset + new_data.len() as u64;
        if end_offset > HEADER_SIZE as u64 {
            return Err(RecoveryError::validation(
                format!("Header update exceeds header region: offset={} + size={} > {}",
                       header_offset, new_data.len(), HEADER_SIZE)
            ));
        }

        // Step 2: Create rollback operation BEFORE making changes (critical for transaction integrity)
        if let Some(old) = old_data {
            rollback_data.push(RollbackOperation::HeaderUpdate {
                header_offset,
                new_data: new_data.to_vec(),
                old_data: old.to_vec(),
            });
        }

        // Step 3: Perform atomic write to GraphFile header
        // File: sqlitegraph/src/backend/native/graph_file/mod.rs
        // Method: write_bytes(offset, data) - Writes data at specific offset
        {
            let mut graph_file = self.graph_file.write()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock graph file: {}", e)
                ))?;

            graph_file.write_bytes(header_offset, new_data)
                .map_err(|e| RecoveryError::io_error(
                    format!("Failed to write header at offset {}: {:?}", header_offset, e)
                ))?;

            debug_log!("Successfully updated header at offset {} ({} bytes)", header_offset, new_data.len());
        }

        // Step 4: Update replay statistics
        {
            let mut stats_guard = self.statistics.lock()
                .map_err(|e| RecoveryError::replay_failure(
                    format!("Failed to lock statistics: {}", e)
                ))?;

            stats_guard.record_bytes_written(new_data.len() as u64);
        }

        debug_log!("Header update replay completed: offset={}, size={}", header_offset, new_data.len());

        Ok(())
    }
}
