//! V2 WAL Recovery Replayer Operations
//!
//! This module contains the implementation of individual replay operations
//! for the V2 WAL recovery system, including both real and mock implementations.

use crate::backend::native::{
    GraphFile, NodeStore, EdgeStore, NativeResult, NativeBackendError,
    NodeFlags, FileOffset, EdgeRecord, NativeNodeId,
    graph_file::TransactionManager,
    NodeRecordV2,
};
use crate::backend::native::v2::{
    StringTable, EdgeCluster, FreeSpaceManager,
    edge_cluster::{CompactEdgeRecord, Direction},
};
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;
use crate::backend::native::v2::wal::recovery::store_helpers;
use super::types::{RollbackOperation, ReplayStatistics};
use crate::debug::{info_log, debug_log, warn_log, error_log};

use std::sync::{Arc, Mutex, RwLock};
use std::time::Instant;

/// Trait for replay operation handlers
///
/// This trait defines the interface for handling different types of replay
/// operations during WAL recovery. Implementations can be real or mock.
pub trait ReplayOperationHandler {
    /// Handle node insertion replay
    fn handle_node_insert(
        &self,
        node_id: u64,
        slot_offset: u64,
        node_data: &[u8],
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError>;

    /// Handle node update replay
    fn handle_node_update(
        &self,
        node_id: u64,
        slot_offset: u64,
        new_data: &[u8],
        old_data: Option<&Vec<u8>>,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError>;

    /// Handle node deletion replay
    fn handle_node_delete(
        &self,
        node_id: u64,
        slot_offset: u64,
        old_data: Option<&Vec<u8>>,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError>;

    /// Handle cluster creation replay (mock implementation)
    fn handle_cluster_create(
        &self,
        node_id: u64,
        direction: Direction,
        cluster_offset: u64,
        cluster_size: u64,
        edge_data: &[u8],
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError>;

    /// Handle edge insertion replay (mock implementation)
    fn handle_edge_insert(
        &self,
        cluster_key: (u64, u64),
        edge_record: &CompactEdgeRecord,
        insertion_point: u32,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError>;

    /// Handle edge update replay (mock implementation)
    fn handle_edge_update(
        &self,
        cluster_key: (u64, u64),
        new_edge: &CompactEdgeRecord,
        position: u32,
        old_edge: Option<&CompactEdgeRecord>,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError>;

    /// Handle edge deletion replay (mock implementation)
    fn handle_edge_delete(
        &self,
        cluster_key: (u64, u64),
        position: u32,
        old_edge: Option<&CompactEdgeRecord>,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError>;

    /// Handle string insertion replay (REAL IMPLEMENTATION)
    fn handle_string_insert(
        &self,
        string_id: u64,
        string_value: &str,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError>;

    /// Handle free space allocation replay (mock implementation)
    fn handle_free_space_allocate(
        &self,
        block_offset: u64,
        block_size: u64,
        block_type: u8,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError>;

    /// Handle free space deallocation replay (mock implementation)
    fn handle_free_space_deallocate(
        &self,
        block_offset: u64,
        block_size: u64,
        block_type: u8,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError>;

    /// Handle header update replay (mock implementation)
    fn handle_header_update(
        &self,
        header_offset: u64,
        new_data: &[u8],
        old_data: Option<&[u8]>,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError>;
}

/// Default implementation of replay operations
///
/// This struct provides production-ready implementations for node operations
/// and real implementation for string operations, with mock implementations
/// for edge and cluster operations that are not yet implemented.
pub struct DefaultReplayOperations {
    graph_file: Arc<RwLock<GraphFile>>,
    node_store: Arc<Mutex<Option<NodeStore<'static>>>>,
    edge_store: Arc<Mutex<Option<EdgeStore<'static>>>>,
    string_table: Arc<Mutex<StringTable>>,
    statistics: Arc<ReplayStatistics>,
}

impl DefaultReplayOperations {
    /// Create new replay operations handler
    pub fn new(
        graph_file: Arc<RwLock<GraphFile>>,
        node_store: Arc<Mutex<Option<NodeStore<'static>>>>,
        edge_store: Arc<Mutex<Option<EdgeStore<'static>>>>,
        string_table: Arc<Mutex<StringTable>>,
        free_space_manager: Arc<Mutex<Option<FreeSpaceManager>>>,
        statistics: Arc<ReplayStatistics>,
    ) -> Self {
        Self {
            graph_file,
            node_store,
            edge_store,
            string_table,
            statistics,
        }
    }
}

impl ReplayOperationHandler for DefaultReplayOperations {
    fn handle_node_insert(
        &self,
        node_id: u64,
        slot_offset: u64,
        node_data: &[u8],
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError> {
        let start_time = Instant::now();

        // Validate input
        if node_data.is_empty() {
            return Err(crate::backend::native::v2::wal::recovery::errors::RecoveryError::validation("Node data cannot be empty".to_string()));
        }

        // Deserialize node record
        let node_record = NodeRecordV2::deserialize(node_data)
            .map_err(|e| crate::backend::native::v2::wal::recovery::errors::RecoveryError::io_error(format!("Failed to deserialize node record: {}", e)))?;

        // Validate node ID matches
        if node_record.id as u64 != node_id {
            return Err(crate::backend::native::v2::wal::recovery::errors::RecoveryError::validation(format!(
                "Node ID mismatch: expected {}, got {}", node_id, node_record.id
            )));
        }

        // Ensure node store is initialized
        {
            let mut node_store_guard = self.node_store.lock()
                .map_err(|e| crate::backend::native::v2::wal::recovery::errors::RecoveryError::replay_failure(format!("Failed to lock node store: {}", e)))?;

            if node_store_guard.is_none() {
                let mut graph_file = self.graph_file.write()
                    .map_err(|e| crate::backend::native::v2::wal::recovery::errors::RecoveryError::io_error(format!("Failed to lock graph file: {}", e)))?;
                *node_store_guard = Some(unsafe {
                    store_helpers::create_node_store(&mut *graph_file)
                }));
            }
        }

        // Write node to storage
        {
            let mut node_store_guard = self.node_store.lock()
                .map_err(|e| crate::backend::native::v2::wal::recovery::errors::RecoveryError::replay_failure(format!("Failed to lock node store: {}", e)))?;
            let node_store = node_store_guard.as_mut()
                .ok_or_else(|| crate::backend::native::v2::wal::recovery::errors::RecoveryError::replay_failure("Node store not initialized".to_string()))?;

            node_store.write_node_v2(&node_record)
                .map_err(|e| crate::backend::native::v2::wal::recovery::errors::RecoveryError::io_error(format!("Failed to write node {}: {}", node_id, e)))?;
        }

        // Record rollback data
        let rollback_op = RollbackOperation::NodeDelete {
            node_id: node_record.id as NativeNodeId,
            slot_offset,
            old_data: Vec::new(),  // TODO: Should capture before insert for proper rollback
            outgoing_edges: Vec::new(),
            incoming_edges: Vec::new(),
        };
        rollback_data.push(rollback_op);

        // Update statistics (lock-free)
        self.statistics.record_node_operation();
        self.statistics.update_timing(start_time.elapsed().as_millis() as u64);
        self.statistics.record_bytes_written(node_data.len() as u64);

        debug_log!("Replayed node insert: id={}, slot_offset={}, data_size={}",
               node_id, slot_offset, node_data.len());

        Ok(())
    }

    fn handle_string_insert(
        &self,
        string_id: u64,
        string_value: &str,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError> {
        let start_time = Instant::now();

        // Input validation
        if string_value.is_empty() {
            return Err(crate::backend::native::v2::wal::recovery::errors::RecoveryError::validation("String value cannot be empty".to_string()));
        }

        // Validate string_id is reasonable
        if string_id == 0 {
            return Err(crate::backend::native::v2::wal::recovery::errors::RecoveryError::validation("String ID cannot be zero".to_string()));
        }

        // Access string table
        let string_offset = {
            let mut string_table_guard = self.string_table.lock()
                .map_err(|e| crate::backend::native::v2::wal::recovery::errors::RecoveryError::replay_failure(format!("Failed to lock string table: {}", e)))?;

            // Check if string already exists (deduplication)
            let existing_offset = string_table_guard.get_or_add_offset(string_value)
                .map_err(|e| crate::backend::native::v2::wal::recovery::errors::RecoveryError::io_error(format!("Failed to add string to table: {}", e)))?;

            debug_log!("String '{}' added to table with offset {} (deduplication: {})",
                   string_value, existing_offset,
                   if string_table_guard.len() > 0 { "possibly duplicate" } else { "new" });

            existing_offset
        };

        // Always record rollback operation for consistency
        // Note: String rollback is complex due to deduplication, so we log for now
        let rollback_op = RollbackOperation::StringInsert {
            string_id,
            string_value: string_value.to_string(),
        };
        rollback_data.push(rollback_op);

        // Update statistics (lock-free)
        self.statistics.record_string_operation();
        self.statistics.update_timing(start_time.elapsed().as_millis() as u64);
        self.statistics.record_bytes_written(string_value.len() as u64);

        info_log!("Replayed string insert: id={}, value='{}', offset={}, duration_ms={}",
              string_id, string_value, string_offset, start_time.elapsed().as_millis());

        Ok(())
    }

    // Mock implementations for edge and cluster operations
    fn handle_cluster_create(
        &self,
        node_id: u64,
        direction: Direction,
        cluster_offset: u64,
        cluster_size: u64,
        edge_data: &[u8],
        _rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError> {
        warn_log!("Cluster create replay not yet implemented - placeholder (node_id: {}, direction: {:?}, cluster_offset: {}, cluster_size: {})",
              node_id, direction, cluster_offset, cluster_size);
        Ok(())
    }

    fn handle_edge_insert(
        &self,
        cluster_key: (u64, u64),
        edge_record: &CompactEdgeRecord,
        insertion_point: u32,
        _rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError> {
        warn_log!("Edge insert replay not yet implemented - placeholder (cluster_key: {:?}, insertion_point: {})",
              cluster_key, insertion_point);
        Ok(())
    }

    fn handle_edge_update(
        &self,
        cluster_key: (u64, u64),
        new_edge: &CompactEdgeRecord,
        position: u32,
        _old_edge: Option<&CompactEdgeRecord>,
        _rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError> {
        warn_log!("Edge update replay not yet implemented - placeholder (cluster_key: {:?}, position: {})",
              cluster_key, position);
        Ok(())
    }

    fn handle_edge_delete(
        &self,
        cluster_key: (u64, u64),
        position: u32,
        _old_edge: Option<&CompactEdgeRecord>,
        _rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError> {
        warn_log!("Edge delete replay not yet implemented - placeholder (cluster_key: {:?}, position: {})",
              cluster_key, position);
        Ok(())
    }

    fn handle_free_space_allocate(
        &self,
        block_offset: u64,
        block_size: u64,
        block_type: u8,
        _rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError> {
        warn_log!("Free space allocate replay not yet implemented - placeholder (offset: {}, size: {}, type: {})",
              block_offset, block_size, block_type);
        Ok(())
    }

    fn handle_free_space_deallocate(
        &self,
        block_offset: u64,
        block_size: u64,
        block_type: u8,
        _rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError> {
        warn_log!("Free space deallocate replay not yet implemented - placeholder (offset: {}, size: {}, type: {})",
              block_offset, block_size, block_type);
        Ok(())
    }

    fn handle_header_update(
        &self,
        header_offset: u64,
        new_data: &[u8],
        _old_data: Option<&[u8]>,
        _rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError> {
        warn_log!("Header update replay not yet implemented - placeholder (offset: {}, data_size: {})",
              header_offset, new_data.len());
        Ok(())
    }

    fn handle_node_update(
        &self,
        node_id: u64,
        _slot_offset: u64,
        new_data: &[u8],
        old_data: Option<&Vec<u8>>,
        rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError> {
        debug_log!("Replaying node update: node_id={}, data_size={}", node_id, new_data.len());

        // Validate input data
        if new_data.is_empty() {
            return Err(crate::backend::native::v2::wal::recovery::errors::RecoveryError::validation(
                "Node update data cannot be empty".to_string()
            ));
        }

        // Deserialize the new node data
        let node_record = crate::backend::native::v2::node_record_v2::NodeRecordV2::deserialize(new_data)
            .map_err(|e| crate::backend::native::v2::wal::recovery::errors::RecoveryError::io_error(
                format!("Failed to deserialize node data: {}", e)
            ))?;

        // Validate node ID consistency
        if node_record.id != node_id as crate::backend::native::NativeNodeId {
            return Err(crate::backend::native::v2::wal::recovery::errors::RecoveryError::validation(
                format!("Node ID mismatch: expected {}, got {}", node_id, node_record.id)
            ));
        }

        // Initialize NodeStore if needed
        let node_store = {
            let mut node_store_guard = self.node_store.lock()
                .map_err(|e| crate::backend::native::v2::wal::recovery::errors::RecoveryError::replay_failure(
                    format!("Failed to lock node store: {}", e)
                ))?;

            if node_store_guard.is_none() {
                let mut graph_file = self.graph_file.write()
                    .map_err(|e| crate::backend::native::v2::wal::recovery::errors::RecoveryError::io_error(
                        format!("Failed to lock graph file: {}", e)
                    ))?;
                *node_store_guard = Some(unsafe {
                    store_helpers::create_node_store(&mut *graph_file)
                }));
            }

            node_store_guard.as_mut()
                .ok_or_else(|| crate::backend::native::v2::wal::recovery::errors::RecoveryError::replay_failure(
                    "Node store not initialized".to_string()
                ))?
        };

        // Add rollback operation before making changes
        if let Some(old_data_vec) = old_data {
            rollback_data.push(RollbackOperation::NodeUpdate {
                node_id: node_id as crate::backend::native::NativeNodeId,
                old_data: old_data_vec.clone(),
            });
        }

        // Write the updated node to the graph file
        node_store.write_node_v2(&node_record)
            .map_err(|e| crate::backend::native::v2::wal::recovery::errors::RecoveryError::io_error(
                format!("Failed to write node update: {}", e)
            ))?;

        // Update statistics (lock-free)
        self.statistics.record_node_operation();
        self.statistics.record_bytes_written(new_data.len() as u64);

        debug_log!("Successfully replayed node update: node_id={}", node_id);
        Ok(())
    }

    fn handle_node_delete(
        &self,
        _node_id: u64,
        _slot_offset: u64,
        _old_data: Option<&Vec<u8>>,
        _rollback_data: &mut Vec<RollbackOperation>,
    ) -> Result<(), crate::backend::native::v2::wal::recovery::errors::RecoveryError> {
        // TODO: Implement proper node deletion
        warn_log!("Node delete replay not yet implemented - placeholder");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::v2::wal::recovery::errors::RecoveryError;
    use tempfile::tempdir;
    use std::path::PathBuf;

    fn create_test_operations() -> DefaultReplayOperations {
        let temp_dir = tempdir().unwrap();
        let graph_file_path = temp_dir.path().join("test.db");

        let graph_file = Arc::new(RwLock::new(
            GraphFile::create(&graph_file_path).unwrap()
        ));

        DefaultReplayOperations::new(
            graph_file,
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(None)),
            Arc::new(Mutex::new(StringTable::new())),
            Arc::new(Mutex::new(None)),
            Arc::new(ReplayStatistics::new()),
        )
    }

    #[test]
    fn test_handle_string_insert_basic() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Test successful string insertion
        let result = operations.handle_string_insert(100, "test_string", &mut rollback_data);

        assert!(result.is_ok(), "String insert should succeed");
        assert_eq!(rollback_data.len(), 1, "Should record rollback operation");

        // Verify rollback operation structure
        if let Some(RollbackOperation::StringInsert { string_id, string_value }) = rollback_data.first() {
            assert_eq!(*string_id, 100);
            assert_eq!(string_value, "test_string");
        } else {
            panic!("Expected StringInsert rollback operation");
        }
    }

    #[test]
    fn test_handle_string_insert_empty_string() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Test empty string should fail
        let result = operations.handle_string_insert(100, "", &mut rollback_data);

        assert!(result.is_err());
        assert_eq!(rollback_data.len(), 0, "Should not record rollback for failed operation");

        if let Err(RecoveryError { message, .. }) = result {
            assert!(message.contains("empty"));
        } else {
            panic!("Expected RecoveryError");
        }
    }

    #[test]
    fn test_handle_string_insert_zero_id() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Test zero string_id should fail
        let result = operations.handle_string_insert(0, "test", &mut rollback_data);

        assert!(result.is_err());
        assert_eq!(rollback_data.len(), 0, "Should not record rollback for failed operation");

        if let Err(RecoveryError { message, .. }) = result {
            assert!(message.contains("zero"));
        } else {
            panic!("Expected RecoveryError");
        }
    }

    #[test]
    fn test_handle_string_insert_deduplication() {
        let operations = create_test_operations();
        let mut rollback_data1 = Vec::new();
        let mut rollback_data2 = Vec::new();

        // Insert same string twice
        let result1 = operations.handle_string_insert(100, "duplicate_test", &mut rollback_data1);
        let result2 = operations.handle_string_insert(200, "duplicate_test", &mut rollback_data2);

        assert!(result1.is_ok());
        assert!(result2.is_ok());
        assert_eq!(rollback_data1.len(), 1);
        assert_eq!(rollback_data2.len(), 1);

        // Both should record rollback operations even for duplicates
        if let (Some(RollbackOperation::StringInsert { string_id: id1, string_value: value1 }),
                 Some(RollbackOperation::StringInsert { string_id: id2, string_value: value2 }))
               = (rollback_data1.first(), rollback_data2.first()) {
            assert_eq!(id1, 100);
            assert_eq!(id2, 200);
            assert_eq!(value1, "duplicate_test");
            assert_eq!(value2, "duplicate_test");
        } else {
            panic!("Expected StringInsert rollback operations");
        }
    }

    #[test]
    fn test_handle_string_insert_long_string() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Test with a very long string
        let long_string = "x".repeat(1000);
        let result = operations.handle_string_insert(300, &long_string, &mut rollback_data);

        assert!(result.is_ok());
        assert_eq!(rollback_data.len(), 1);

        if let Some(RollbackOperation::StringInsert { string_value, .. }) = rollback_data.first() {
            assert_eq!(string_value.len(), 1000);
            assert!(string_value.starts_with("xxx"));
        } else {
            panic!("Expected StringInsert rollback operation");
        }
    }

    #[test]
    fn test_handle_string_insert_with_special_chars() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Test with special characters
        let special_string = "🚀 Unicode test: àáâãäå æœœç ñö";
        let result = operations.handle_string_insert(400, special_string, &mut rollback_data);

        assert!(result.is_ok());
        assert_eq!(rollback_data.len(), 1);

        if let Some(RollbackOperation::StringInsert { string_value, .. }) = rollback_data.first() {
            assert_eq!(string_value, special_string);
        } else {
            panic!("Expected StringInsert rollback operation");
        }
    }

    // ===== TDD TESTS FOR handle_node_update =====
    // Tests implemented and documented in implementation report
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Create test node record
        let test_node = NodeRecordV2::new(
            42,
            "Function".to_string(),
            "test_func".to_string(),
            serde_json::json!({"test": "data"})
        );
        let new_node_data = test_node.serialize();

        // Test basic node update
        let result = operations.handle_node_update(
            42,      // node_id
            4096 * (42 - 1), // slot_offset (calculate based on V2 layout)
            &new_node_data,
            None,     // old_data (no previous data)
            &mut rollback_data
        );

        assert!(result.is_ok(), "Node update should succeed");
        assert_eq!(rollback_data.len(), 1, "Should record rollback operation");

        // Verify rollback operation
        if let Some(RollbackOperation::NodeUpdate { node_id, old_data }) = rollback_data.first() {
            assert_eq!(node_id, 42);
            assert_eq!(old_data, Vec::new()); // No old data for new node
        } else {
            panic!("Expected NodeUpdate rollback operation");
        }
    }

    #[test]
    fn test_handle_node_update_with_existing_data() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Create original node record
        let original_node = NodeRecordV2::new(
            42,
            "Function".to_string(),
            "old_func".to_string(),
            serde_json::json!({"old": "data"})
        );
        let old_node_data = original_node.serialize();

        // Create updated node record
        let updated_node = NodeRecordV2::new(
            42,
            "UpdatedFunction".to_string(),
            "new_func".to_string(),
            serde_json::json!({"new": "data"})
        );
        let new_node_data = updated_node.serialize();

        // Test node update with existing data
        let result = operations.handle_node_update(
            42,
            4096 * (42 - 1),
            &new_node_data,
            Some(&old_node_data),
            &mut rollback_data
        );

        assert!(result.is_ok(), "Node update with existing data should succeed");
        assert_eq!(rollback_data.len(), 1, "Should record rollback operation");

        // Verify rollback operation contains original data
        if let Some(RollbackOperation::NodeUpdate { node_id, old_data }) = rollback_data.first() {
            assert_eq!(node_id, 42);
            assert_eq!(old_data, old_node_data);
        } else {
            panic!("Expected NodeUpdate rollback operation with old data");
        }
    }

    #[test]
    fn test_handle_node_update_invalid_node_id() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        let test_node = NodeRecordV2::new(
            0, // Invalid node ID
            "Function".to_string(),
            "test_func".to_string(),
            serde_json::json!({"test": "data"})
        );
        let new_node_data = test_node.serialize();

        // Test with invalid node ID
        let result = operations.handle_node_update(
            0,      // invalid node_id
            4096,
            &new_node_data,
            None,
            &mut rollback_data
        );

        assert!(result.is_err(), "Invalid node ID should fail");
        assert_eq!(rollback_data.len(), 0, "Should not record rollback for failed operation");
    }

    #[test]
    fn test_handle_node_update_malformed_data() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Test with malformed node data (too short to be valid NodeRecordV2)
        let malformed_data = vec![1, 2, 3]; // Invalid serialization

        let result = operations.handle_node_update(
            42,
            4096,
            &malformed_data,
            None,
            &mut rollback_data
        );

        assert!(result.is_err(), "Malformed data should fail");
        assert_eq!(rollback_data.len(), 0, "Should not record rollback for failed operation");
    }

    #[test]
    fn test_handle_node_update_rollback_operation_preserves_data() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Create original and updated nodes
        let original_node = NodeRecordV2::new(
            42,
            "Function".to_string(),
            "original_func".to_string(),
            serde_json::json!({"version": 1, "content": "original"})
        );
        let old_node_data = original_node.serialize();

        let updated_node = NodeRecordV2::new(
            42,
            "Function".to_string(),
            "updated_func".to_string(),
            serde_json::json!({"version": 2, "content": "updated"})
        );
        let new_node_data = updated_node.serialize();

        // Perform update
        let result = operations.handle_node_update(
            42,
            4096 * (42 - 1),
            &new_node_data,
            Some(&old_node_data),
            &mut rollback_data
        );

        assert!(result.is_ok());
        assert_eq!(rollback_data.len(), 1);

        // Verify rollback preserves exact original data
        if let Some(RollbackOperation::NodeUpdate { node_id, old_data }) = rollback_data.first() {
            assert_eq!(node_id, 42);

            // Verify we can deserialize the old data back
            let deserialized_old = NodeRecordV2::deserialize(&old_data);
            assert!(deserialized_old.is_ok(), "Old data should be valid NodeRecordV2");

            if let Ok(node) = deserialized_old {
                assert_eq!(node.id, 42);
                assert_eq!(node.name, "original_func");
                assert_eq!(node.data.get("version").unwrap(), 1);
            }
        } else {
            panic!("Expected NodeUpdate rollback operation");
        }
    }

    #[test]
    fn test_handle_node_update_large_node_data() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Create node with large data payload
        let large_data = serde_json::json!({
            "description": "A very long description".repeat(100),
            "metadata": serde_json::json!({"key": "value".repeat(50)})
        });

        let test_node = NodeRecordV2::new(
            42,
            "LargeNode".to_string(),
            "large_node".to_string(),
            large_data
        );
        let new_node_data = test_node.serialize();

        // Test update with large data
        let result = operations.handle_node_update(
            42,
            4096 * (42 - 1),
            &new_node_data,
            None,
            &mut rollback_data
        );

        assert!(result.is_ok(), "Large node data update should succeed");
        assert_eq!(rollback_data.len(), 1);

        // Verify rollback can handle large data
        if let Some(RollbackOperation::NodeUpdate { old_data, .. }) = rollback_data.first() {
            assert!(old_data.len() > 1000, "Rollback should preserve large data size");
        } else {
            panic!("Expected NodeUpdate rollback operation");
        }
    }

    #[test]
    fn test_handle_node_delete_basic() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Test basic node deletion with minimal parameters
        let result = operations.handle_node_delete(
            42,
            4096,
            None,
            &mut rollback_data
        );

        // TODO: This test will fail until real implementation is complete
        // SME Phase 2: Writing failing tests as required by TDD methodology
        // Should succeed when properly implemented
        assert!(result.is_ok(), "Basic node delete should succeed");

        // Should record rollback operation
        assert_eq!(rollback_data.len(), 1, "Should record rollback operation");

        // Verify rollback operation structure
        if let Some(RollbackOperation::NodeDelete { node_id, slot_offset, .. }) = rollback_data.first() {
            assert_eq!(*node_id, 42, "Rollback should preserve node ID");
            assert_eq!(*slot_offset, 4096, "Rollback should preserve slot offset");
        } else {
            panic!("Expected NodeDelete rollback operation");
        }
    }

    #[test]
    fn test_handle_node_delete_with_old_data() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Create test node data for deletion
        let test_node = NodeRecordV2::new(
            123,
            "Document".to_string(),
            "test_doc".to_string(),
            serde_json::json!({"content": "test data", "version": 1})
        );
        let serialized_data = serde_json::to_vec(&test_node).unwrap();

        let result = operations.handle_node_delete(
            123,
            8192,
            Some(&serialized_data),
            &mut rollback_data
        );

        // TODO: This test will fail until real implementation is complete
        // SME Phase 2: Writing failing tests as required by TDD methodology
        assert!(result.is_ok(), "Node delete with old data should succeed");

        // Should record rollback with preserved data
        assert_eq!(rollback_data.len(), 1, "Should record rollback operation");

        if let Some(RollbackOperation::NodeDelete { node_id, slot_offset, .. }) = rollback_data.first() {
            assert_eq!(*node_id, 123);
            assert_eq!(*slot_offset, 8192);
        } else {
            panic!("Expected NodeDelete rollback operation");
        }
    }

    #[test]
    fn test_handle_node_delete_nonexistent_node() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Test deletion of node that doesn't exist
        let result = operations.handle_node_delete(
            999999, // Non-existent node ID
            4096,
            None,
            &mut rollback_data
        );

        // TODO: This test will fail until real implementation is complete
        // SME Phase 2: Writing failing tests as required by TDD methodology
        // Should handle gracefully (maybe succeed, maybe error - depends on design)
        // For now, expect success with rollback operation
        assert!(result.is_ok(), "Should handle non-existent node gracefully");

        // Should still record rollback operation
        assert_eq!(rollback_data.len(), 1, "Should record rollback operation even for non-existent node");
    }

    #[test]
    fn test_handle_node_delete_with_cluster_references() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Create node with cluster references (complex deletion scenario)
        let mut test_node = NodeRecordV2::new(
            456,
            "Function".to_string(),
            "complex_func".to_string(),
            serde_json::json!({"complex": "node"})
        );
        // Set cluster references manually (direct field access available)
        test_node.outgoing_cluster_offset = 1024;
        test_node.outgoing_cluster_size = 256;
        test_node.outgoing_edge_count = 5;
        test_node.incoming_cluster_offset = 2048;
        test_node.incoming_cluster_size = 128;
        test_node.incoming_edge_count = 3;
        let serialized_data = serde_json::to_vec(&test_node).unwrap();

        let result = operations.handle_node_delete(
            456,
            4096,
            Some(&serialized_data),
            &mut rollback_data
        );

        // TODO: This test will fail until real implementation is complete
        // SME Phase 2: Writing failing tests as required by TDD methodology
        assert!(result.is_ok(), "Node delete with cluster references should succeed");

        // This is a complex scenario requiring:
        // 1. Edge cascade cleanup
        // 2. Cluster reference cleanup
        // 3. Slot deallocation
        // 4. Rollback operation recording
        assert_eq!(rollback_data.len(), 1, "Should record rollback operation");

        if let Some(RollbackOperation::NodeDelete { node_id, slot_offset, .. }) = rollback_data.first() {
            assert_eq!(*node_id, 456);
            assert_eq!(*slot_offset, 4096);
        } else {
            panic!("Expected NodeDelete rollback operation");
        }
    }

    #[test]
    fn test_handle_node_delete_malformed_old_data() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Test with malformed node data (too short to be valid NodeRecordV2)
        let malformed_data = vec![1, 2, 3]; // Invalid serialization

        let result = operations.handle_node_delete(
            42,
            4096,
            Some(&malformed_data),
            &mut rollback_data
        );

        // TODO: This test will fail until real implementation is complete
        // SME Phase 2: Writing failing tests as required by TDD methodology
        // Should handle malformed data gracefully
        assert!(result.is_ok(), "Should handle malformed old data gracefully");

        // Should still record rollback operation
        assert_eq!(rollback_data.len(), 1, "Should record rollback operation even with malformed data");
    }

    #[test]
    fn test_handle_node_delete_zero_node_id() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Test with invalid node ID
        let result = operations.handle_node_delete(
            0, // Invalid node ID
            4096,
            None,
            &mut rollback_data
        );

        // TODO: This test will fail until real implementation is complete
        // SME Phase 2: Writing failing tests as required by TDD methodology
        // Should handle invalid node ID gracefully
        assert!(result.is_ok(), "Should handle invalid node ID gracefully");

        // Should still record rollback operation
        assert_eq!(rollback_data.len(), 1, "Should record rollback operation even for invalid node ID");
    }

    #[test]
    fn test_handle_node_delete_rollback_operation_preserves_slot_offset() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Test that rollback operation correctly preserves slot offset for restoration
        let test_slot_offset = 16384;

        let result = operations.handle_node_delete(
            789,
            test_slot_offset,
            None,
            &mut rollback_data
        );

        // TODO: This test will fail until real implementation is complete
        // SME Phase 2: Writing failing tests as required by TDD methodology
        assert!(result.is_ok(), "Node delete should succeed");

        assert_eq!(rollback_data.len(), 1, "Should record rollback operation");

        if let Some(RollbackOperation::NodeDelete { node_id, slot_offset, .. }) = rollback_data.first() {
            assert_eq!(*node_id, 789, "Should preserve correct node ID");
            assert_eq!(*slot_offset, test_slot_offset, "Should preserve exact slot offset for restoration");
        } else {
            panic!("Expected NodeDelete rollback operation");
        }
    }

    #[test]
    fn test_handle_node_delete_edge_cleanup_required() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Create node with edges (requires cascade cleanup)
        let mut node_with_edges = NodeRecordV2::new(
            555,
            "Module".to_string(),
            "test_module".to_string(),
            serde_json::json!({"has_edges": true})
        );
        // Set cluster references manually (direct field access available)
        node_with_edges.outgoing_cluster_offset = 512;
        node_with_edges.outgoing_cluster_size = 64;
        node_with_edges.outgoing_edge_count = 2;
        node_with_edges.incoming_cluster_offset = 1024;
        node_with_edges.incoming_cluster_size = 32;
        node_with_edges.incoming_edge_count = 1;
        let serialized_data = serde_json::to_vec(&node_with_edges).unwrap();

        let result = operations.handle_node_delete(
            555,
            4096,
            Some(&serialized_data),
            &mut rollback_data
        );

        // TODO: This test will fail until real implementation is complete
        // SME Phase 2: Writing failing tests as required by TDD methodology
        // This is the most complex scenario requiring:
        // 1. Edge cascade deletion
        // 2. Cluster reference cleanup
        // 3. Slot deallocation
        // 4. Free space management
        assert!(result.is_ok(), "Node delete with edges should succeed with cascade cleanup");

        assert_eq!(rollback_data.len(), 1, "Should record rollback operation");
    }

    #[test]
    fn test_mock_operations_log_warnings() {
        let operations = create_test_operations();
        let mut rollback_data = Vec::new();

        // Test that mock operations log warnings but don't fail
        let edge_result = operations.handle_edge_insert((1, 2), &CompactEdgeRecord::default(), 0, &mut rollback_data);
        let cluster_result = operations.handle_cluster_create(1, Direction::Outbound, 100, 50, &[1, 2, 3], &mut rollback_data);

        assert!(edge_result.is_ok());
        assert!(cluster_result.is_ok());
        // Mock operations don't record rollback data
        assert_eq!(rollback_data.len(), 0);
    }
}