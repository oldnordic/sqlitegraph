#![cfg(feature = "v2_experimental")]

//! Tests for V2 node serialization using binrw
//! These tests will fail initially and should pass after binrw implementation

use sqlitegraph::backend::native::v2::node_record_v2::NodeRecordV2;
use sqlitegraph::backend::native::{NodeFlags, FileOffset};
use serde_json::json;

#[test]
fn test_v2_node_serialization_binrw_equivalence() {
    // Test that binrw serialization produces exactly the same bytes as current implementation

    // Create test node with all field variations
    let original = NodeRecordV2 {
        id: 1001,
        flags: NodeFlags(0xABCDEF12),
        kind: "ComplexType".to_string(),
        name: "node_with_complex_name_and_data".to_string(),
        data: json!({
            "metadata": {
                "created": "2024-01-15",
                "tags": ["important", "production"],
                "metrics": {"latency": 23.5, "throughput": 1024}
            }
        }),
        outgoing_cluster_offset: 65536,
        outgoing_cluster_size: 8192,
        outgoing_edge_count: 42,
        incoming_cluster_offset: 131072,
        incoming_cluster_size: 4096,
        incoming_edge_count: 17,
    };

    // Serialize using current implementation (reference)
    let reference_bytes = original.serialize();

    // TODO: Replace with binrw serialization once implemented
    // let binrw_bytes = binrw_serialize_node_v2(&original);

    // TODO: Test for byte-for-byte equivalence
    // assert_eq!(binrw_bytes, reference_bytes, "binrw should produce identical bytes");

    // For now, ensure reference serialization works
    assert!(!reference_bytes.is_empty(), "Reference serialization should produce bytes");

    // Verify we can roundtrip through current implementation
    let deserialized = NodeRecordV2::deserialize(&reference_bytes)
        .expect("Should deserialize reference bytes successfully");

    assert_eq!(deserialized.id, original.id);
    assert_eq!(deserialized.kind, original.kind);
    assert_eq!(deserialized.name, original.name);
    assert_eq!(deserialized.data, original.data);
}

#[test]
fn test_v2_node_serialization_edge_cases() {
    // Test edge cases that could cause serialization issues

    let test_cases = vec![
        // Empty strings
        NodeRecordV2::new(1, "".to_string(), "".to_string(), json!({})),

        // Single character strings
        NodeRecordV2::new(2, "A".to_string(), "B".to_string(), json!({"c": "d"})),

        // Long strings (within reasonable limits)
        NodeRecordV2::new(3, "A".repeat(100), "B".repeat(200), json!({"data": "x".repeat(50)})),

        // Complex nested JSON
        NodeRecordV2::new(4, "Complex".to_string(), "nested".to_string(), json!({
            "array": [1, 2, 3, {"nested": true}],
            "object": {"key1": "value1", "key2": {"nested2": false}},
            "null_field": null,
            "number": 42.5,
            "boolean": true
        })),

        // Node with maximum cluster values
        {
            let mut node = NodeRecordV2::new(5, "MaxCluster".to_string(), "max".to_string(), json!({}));
            node.set_outgoing_cluster(2147483647, 4294967295, 4294967295);
            node.set_incoming_cluster(4294967296, 2147483648, 2147483648);
            node
        }
    ];

    for (i, test_node) in test_cases.iter().enumerate() {
        let serialized = test_node.serialize();

        // Should be able to roundtrip
        let deserialized = NodeRecordV2::deserialize(&serialized)
            .unwrap_or_else(|e| panic!("Test case {}: deserialization failed: {:?}", i, e));

        assert_eq!(deserialized.id, test_node.id, "Test case {}: ID mismatch", i);
        assert_eq!(deserialized.kind, test_node.kind, "Test case {}: kind mismatch", i);
        assert_eq!(deserialized.name, test_node.name, "Test case {}: name mismatch", i);

        // JSON comparison needs special handling due to potential whitespace differences
        let expected_json = serde_json::to_string(&test_node.data).unwrap();
        let actual_json = serde_json::to_string(&deserialized.data).unwrap();
        assert_eq!(actual_json, expected_json, "Test case {}: data mismatch", i);
    }
}

#[test]
fn test_v2_node_serialization_corruption_detection() {
    // Test that corruption detection works for various malformed inputs

    // Test 1: Truncated header
    let truncated = vec![2, 0, 0, 0, 0]; // Only 5 bytes, need at least 21
    let result = NodeRecordV2::deserialize(&truncated);
    assert!(result.is_err(), "Should reject truncated header");

    // Test 2: Wrong version
    let wrong_version = vec![1]; // Version 1 instead of 2
    let result = NodeRecordV2::deserialize(&wrong_version);
    assert!(result.is_err(), "Should reject wrong version");

    // Test 3: Length field corruption (claims more data than available)
    let mut corrupt = vec![0u8; 25]; // Valid size but with corrupted length fields
    corrupt[0] = 2; // version
    corrupt[13] = 0xFF; // kind_len = 65535 (way too big)
    corrupt[14] = 0xFF;
    let result = NodeRecordV2::deserialize(&corrupt);
    assert!(result.is_err(), "Should reject impossible length fields");

    // Test 4: Valid structure but invalid UTF-8 strings
    let mut invalid_utf8 = vec![0u8; 30];
    invalid_utf8[0] = 2; // version
    // Set reasonable lengths
    invalid_utf8[13] = 2; // kind_len = 2
    invalid_utf8[14] = 0;
    invalid_utf8[15] = 5; // name_len = 5
    invalid_utf8[16] = 0;
    invalid_utf8[17] = 0; // data_len = 0
    invalid_utf8[18] = 0;
    invalid_utf8[19] = 0;
    // Insert invalid UTF-8 at position 21 (kind string)
    invalid_utf8[21] = 0xFF;
    invalid_utf8[22] = 0xFE; // Invalid UTF-8 sequence

    let result = NodeRecordV2::deserialize(&invalid_utf8);
    assert!(result.is_err(), "Should reject invalid UTF-8 strings");
}

#[test]
fn test_v2_node_serialization_deterministic() {
    // Test that serialization is deterministic - same input always produces same output

    let node = NodeRecordV2 {
        id: 777,
        flags: NodeFlags(0x12345678),
        kind: "Deterministic".to_string(),
        name: "test_deterministic".to_string(),
        data: json!({"test": true, "value": 42.5}),
        outgoing_cluster_offset: 12345,
        outgoing_cluster_size: 678,
        outgoing_edge_count: 9,
        incoming_cluster_offset: 54321,
        incoming_cluster_size: 456,
        incoming_edge_count: 3,
    };

    // Serialize multiple times
    let serialized1 = node.serialize();
    let serialized2 = node.serialize();
    let serialized3 = node.serialize();

    // All should be identical
    assert_eq!(serialized1, serialized2, "Serialization should be deterministic (1 vs 2)");
    assert_eq!(serialized2, serialized3, "Serialization should be deterministic (2 vs 3)");
    assert_eq!(serialized1, serialized3, "Serialization should be deterministic (1 vs 3)");
}