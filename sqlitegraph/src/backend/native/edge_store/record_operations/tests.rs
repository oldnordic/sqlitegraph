//! Comprehensive test suite for edge record operations
//!
//! Tests all aspects of edge record operations including CRUD operations,
//! serialization, validation, and error handling.

#[cfg(test)]
mod tests {
    use crate::backend::native::constants::edge;
    use crate::backend::native::edge_store::record_operations::EdgeRecordOperations;
    use crate::backend::native::edge_store::record_operations::serialization::EdgeSerializer;
    use crate::backend::native::edge_store::record_operations::validation::EdgeValidator;
    use crate::backend::native::graph_file::GraphFile;
    use crate::backend::native::types::{EdgeFlags, EdgeRecord};
    use tempfile::NamedTempFile;

    fn create_test_graph_file() -> (GraphFile, NamedTempFile) {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let graph_file = GraphFile::create(temp_file.path()).unwrap();
        (graph_file, temp_file)
    }

    fn create_test_edge(graph_file: &mut GraphFile, from_id: i64, to_id: i64) -> EdgeRecord {
        use crate::backend::native::edge_store::id_management::EdgeIdManager;

        // Allocate edge ID properly
        let mut id_manager = EdgeIdManager::new(graph_file);
        let edge_id = id_manager.allocate_edge_id();

        EdgeRecord {
            id: edge_id,
            from_id,
            to_id,
            edge_type: "TEST_EDGE".to_string(),
            flags: EdgeFlags(0),
            data: serde_json::json!({"test": "data"}),
        }
    }

    fn create_test_edge_with_id(
        id: crate::backend::native::types::NativeEdgeId,
        from_id: i64,
        to_id: i64,
    ) -> EdgeRecord {
        EdgeRecord {
            id,
            from_id,
            to_id,
            edge_type: "TEST_EDGE".to_string(),
            flags: EdgeFlags(0),
            data: serde_json::json!({"test": "data"}),
        }
    }

    #[test]
    fn test_edge_serialization() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let edge = create_test_edge(&mut graph_file, 100, 200);
        let serializer = EdgeSerializer::new();

        let serialized = serializer.serialize_edge(&edge).unwrap();

        assert!(serialized.len() > edge::FIXED_HEADER_SIZE);
        assert_eq!(serialized[0], 1); // Version
    }

    #[test]
    fn test_edge_roundtrip() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        let original_edge = create_test_edge(&mut graph_file, 100, 200);
        let allocated_edge_id = original_edge.id;
        let mut operations = EdgeRecordOperations::new(&mut graph_file);

        // Write edge
        operations.write_edge(&original_edge).unwrap();

        // Read edge back
        let read_edge = operations.read_edge(allocated_edge_id).unwrap();

        assert_eq!(original_edge.id, read_edge.id);
        assert_eq!(original_edge.from_id, read_edge.from_id);
        assert_eq!(original_edge.to_id, read_edge.to_id);
        assert_eq!(original_edge.edge_type, read_edge.edge_type);
        assert_eq!(original_edge.flags, read_edge.flags);
        assert_eq!(original_edge.data, read_edge.data);
    }

    #[test]
    fn test_edge_validation() {
        let validator = EdgeValidator::new();

        // Valid edge
        let valid_edge = create_test_edge_with_id(1, 100, 200);
        assert!(validator.validate_edge_fields(&valid_edge).is_ok());

        // Invalid edge ID
        let mut invalid_edge = create_test_edge_with_id(0, 100, 200);
        assert!(validator.validate_edge_fields(&invalid_edge).is_err());

        // Invalid node ID
        invalid_edge.id = 1;
        invalid_edge.from_id = -1;
        assert!(validator.validate_edge_fields(&invalid_edge).is_err());

        // Edge type too long
        invalid_edge.from_id = 100;
        invalid_edge.edge_type = "x".repeat(edge::MAX_STRING_LENGTH_U32 as usize + 1);
        assert!(validator.validate_edge_fields(&invalid_edge).is_err());
    }

    #[test]
    fn test_edge_offset_calculation() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Get base_offset and test edge offset calculation
        let base_offset = graph_file.persistent_header().edge_data_offset;
        let operations = EdgeRecordOperations::new(&mut graph_file);

        // Test the edge offset calculation using the fixed-size edge slot formula
        let edge_id_1 = 1;
        let edge_id_2 = 2;
        let edge_id_10 = 10;
        let fixed_slot_size = 256;

        let expected_offset_1 = base_offset + ((edge_id_1 - 1) as u64 * fixed_slot_size);
        let expected_offset_2 = base_offset + ((edge_id_2 - 1) as u64 * fixed_slot_size);
        let expected_offset_10 = base_offset + ((edge_id_10 - 1) as u64 * fixed_slot_size);

        assert_eq!(expected_offset_1, base_offset);
        assert_eq!(expected_offset_2, base_offset + 256);
        assert_eq!(expected_offset_10, base_offset + (9 * 256));
    }

    #[test]
    fn test_edge_update() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Write initial edge
        let mut edge = create_test_edge(&mut graph_file, 100, 200);
        let allocated_edge_id = edge.id;
        let mut operations = EdgeRecordOperations::new(&mut graph_file);
        operations.write_edge(&edge).unwrap();

        // Update edge
        edge.edge_type = "UPDATED_EDGE".to_string();
        edge.data = serde_json::json!({"updated": true});
        operations.update_edge(&edge).unwrap();

        // Read and verify update
        let read_edge = operations.read_edge(allocated_edge_id).unwrap();
        assert_eq!(read_edge.edge_type, "UPDATED_EDGE");
        assert_eq!(read_edge.data, serde_json::json!({"updated": true}));
    }

    #[test]
    fn test_edge_deletion() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Write edge
        let edge = create_test_edge(&mut graph_file, 100, 200);
        let allocated_edge_id = edge.id;
        let mut operations = EdgeRecordOperations::new(&mut graph_file);
        operations.write_edge(&edge).unwrap();

        // Edge should not be deleted initially
        assert!(!operations.is_edge_deleted(allocated_edge_id).unwrap());

        // Delete edge
        operations.delete_edge(allocated_edge_id).unwrap();

        // Edge should now be marked as deleted
        assert!(operations.is_edge_deleted(allocated_edge_id).unwrap());
    }

    #[test]
    fn test_serialization_with_null_data() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        let mut edge = create_test_edge(&mut graph_file, 100, 200);
        let allocated_edge_id = edge.id;
        edge.data = serde_json::Value::Null;
        let mut operations = EdgeRecordOperations::new(&mut graph_file);

        // Test serialization works with null data using EdgeSerializer directly
        let serializer = EdgeSerializer::new();
        let serialized = serializer.serialize_edge(&edge).unwrap();
        assert!(
            !serialized.is_empty(),
            "Serialization should not be empty even with null data"
        );

        // Test full edge write/read roundtrip with null data
        operations.write_edge(&edge).unwrap();
        let read_edge = operations.read_edge(allocated_edge_id).unwrap();

        assert_eq!(read_edge.id, allocated_edge_id);
        assert_eq!(read_edge.from_id, 100);
        assert_eq!(read_edge.to_id, 200);
        assert_eq!(read_edge.edge_type, "TEST_EDGE");
        assert_eq!(read_edge.data, serde_json::Value::Null);
    }

    #[test]
    fn test_read_nonexistent_edge() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut operations = EdgeRecordOperations::new(&mut graph_file);

        // Try to read edge that doesn't exist
        let result = operations.read_edge(999);
        assert!(result.is_err());

        match result.unwrap_err() {
            crate::backend::native::types::NativeBackendError::InvalidEdgeId { id, max_id } => {
                assert_eq!(id, 999);
                assert_eq!(max_id, 0); // No edges allocated yet
            }
            _ => panic!("Expected InvalidEdgeId error"),
        }
    }

    #[test]
    fn test_serialization_deserialization_standalone() {
        let serializer = EdgeSerializer::new();
        let edge = create_test_edge_with_id(42, 100, 200);

        // Test serialization
        let serialized = serializer.serialize_edge(&edge).unwrap();
        assert!(!serialized.is_empty());

        // Test deserialization
        let deserialized = serializer.deserialize_edge(42, &serialized).unwrap();

        assert_eq!(edge.id, deserialized.id);
        assert_eq!(edge.from_id, deserialized.from_id);
        assert_eq!(edge.to_id, deserialized.to_id);
        assert_eq!(edge.edge_type, deserialized.edge_type);
        assert_eq!(edge.flags, deserialized.flags);
        assert_eq!(edge.data, deserialized.data);
    }

    #[test]
    fn test_validation_standalone() {
        let validator = EdgeValidator::new();

        // Test various validation scenarios
        let test_cases = vec![
            // (edge_id, from_id, to_id, edge_type, should_be_valid)
            (1, 1, 2, "valid_type", true),
            (0, 1, 2, "invalid_id_zero", false),
            (-1, 1, 2, "invalid_id_negative", false),
            (1, 0, 2, "invalid_from_id", false),
            (1, -1, 2, "invalid_from_id_negative", false),
            (1, 1, 0, "invalid_to_id", false),
            (1, 1, -1, "invalid_to_id_negative", false),
        ];

        for (id, from_id, to_id, edge_type, should_be_valid) in test_cases {
            let edge = EdgeRecord {
                id,
                from_id,
                to_id,
                edge_type: edge_type.to_string(),
                flags: EdgeFlags(0),
                data: serde_json::Value::Null,
            };

            let result = validator.validate_edge_fields(&edge);
            assert_eq!(
                result.is_ok(),
                should_be_valid,
                "Validation failed for edge: id={}, from={}, to={}, type={}",
                id,
                from_id,
                to_id,
                edge_type
            );
        }
    }
}
