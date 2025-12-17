#![cfg(feature = "v2_experimental")]

//! Tests for V2 edge cluster serialization using binrw

use sqlitegraph::backend::native::{EdgeRecord, NativeResult};
use sqlitegraph::backend::native::v2::edge_cluster::{EdgeCluster, Direction};
use sqlitegraph::backend::native::v2::string_table::StringTable;
use serde_json::json;

fn create_test_string_table() -> StringTable {
    let mut table = StringTable::new();
    // Pre-populate with common edge types
    table.get_or_add_offset("CALLS").unwrap();
    table.get_or_add_offset("DEFINES").unwrap();
    table.get_or_add_offset("USES").unwrap();
    table.get_or_add_offset("CONTAINS").unwrap();
    table
}

#[test]
fn test_v2_edge_cluster_binrw_roundtrip() {
    // Test that binrw edge cluster serialization produces identical results
    let mut string_table = create_test_string_table();

    // Create test edges
    let edges = vec![
        EdgeRecord::new(1, 100, 200, "CALLS".to_string(), json!({"weight": 1.5})),
        EdgeRecord::new(2, 100, 300, "DEFINES".to_string(), json!({"line": 42})),
        EdgeRecord::new(3, 400, 100, "USES".to_string(), json!({"import": true})),
    ];

    // Create cluster from edges
    let original_cluster = EdgeCluster::create_from_edges(&edges, 100, Direction::Outgoing, &mut string_table)
        .expect("Should create cluster successfully");

    // Serialize using current implementation (reference)
    let reference_bytes = original_cluster.serialize();

    // TODO: Replace with binrw serialization once implemented
    // let binrw_bytes = binrw_serialize_edge_cluster(&original_cluster);

    // TODO: Test for byte-for-byte equivalence
    // assert_eq!(binrw_bytes, reference_bytes, "binrw should produce identical bytes");

    // For now, ensure we can roundtrip through current implementation
    let deserialized_cluster = EdgeCluster::deserialize(&reference_bytes)
        .expect("Should deserialize cluster successfully");

    assert_eq!(deserialized_cluster.edge_count(), original_cluster.edge_count(), "Edge count should match");
    assert_eq!(deserialized_cluster.size_bytes(), original_cluster.size_bytes(), "Size should match");
}

#[test]
fn test_v2_edge_cluster_header_layout() {
    // Test the exact 8-byte header layout of edge clusters
    let mut string_table = create_test_string_table();

    let edges = vec![
        EdgeRecord::new(1, 1, 2, "CONTAINS".to_string(), json!({"data": "test"})),
        EdgeRecord::new(2, 1, 3, "CALLS".to_string(), json!({"freq": 100})),
    ];

    let cluster = EdgeCluster::create_from_edges(&edges, 1, Direction::Outgoing, &mut string_table)
        .expect("Should create cluster");

    let bytes = cluster.serialize();

    // Verify 8-byte header
    assert_eq!(bytes.len(), 8 + cluster.size_bytes(), "Should have 8-byte header plus payload");

    // Edge count (first 4 bytes) - should be 2
    assert_eq!(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]), 2);

    // Payload size (next 4 bytes)
    let expected_payload_size = cluster.size_bytes();
    assert_eq!(u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]), expected_payload_size as u32);
}

#[test]
fn test_v2_compact_edge_record_layout() {
    // Test the exact layout of compact edge records within clusters
    let mut string_table = create_test_string_table();

    let edge = EdgeRecord::new(42, 1000, 2000, "USES".to_string(), json!({"metadata": {"type": "dependency"}}));

    let cluster = EdgeCluster::create_from_edges(&[edge], 1000, Direction::Outgoing, &mut string_table)
        .expect("Should create cluster");

    let bytes = cluster.serialize();

    // Verify compact edge record layout within payload
    // CompactEdgeRecord layout: [neighbor_id: i64][edge_type_offset: u16][edge_data: bytes...]

    // Skip 8-byte header, start of payload
    let payload_start = 8;

    // neighbor_id (8 bytes, big-endian) - should be 2000
    assert_eq!(i64::from_be_bytes([
        bytes[payload_start],
        bytes[payload_start + 1],
        bytes[payload_start + 2],
        bytes[payload_start + 3],
        bytes[payload_start + 4],
        bytes[payload_start + 5],
        bytes[payload_start + 6],
        bytes[payload_start + 7],
    ]), 2000);

    // edge_type_offset (2 bytes, big-endian) - should match "USES" offset in string table
    let edge_type_offset = u16::from_be_bytes([
        bytes[payload_start + 8],
        bytes[payload_start + 9],
    ]);
    assert!(edge_type_offset > 0, "Edge type offset should be positive");

    // edge_data starts at offset 10 and contains JSON bytes
    let edge_data_start = payload_start + 10;
    let edge_data_end = payload_start + 10 + json!({"metadata": {"type": "dependency"}}).to_string().len();
    assert!(edge_data_end <= bytes.len(), "Edge data should fit within cluster bytes");

    // Verify edge data can be parsed as JSON
    let edge_data_slice = &bytes[edge_data_start..edge_data_end];
    let parsed_json: serde_json::Value = serde_json::from_slice(edge_data_slice)
        .expect("Edge data should be valid JSON");

    assert!(parsed_json.is_object(), "Parsed data should be a JSON object");
}

#[test]
fn test_v2_edge_cluster_edge_cases() {
    // Test edge clusters with various edge counts and payload sizes
    let mut string_table = create_test_string_table();

    let test_cases = vec![
        // Empty cluster
        vec![],

        // Single edge
        vec![EdgeRecord::new(1, 1, 2, "CALLS".to_string(), json!({"weight": 1.0}))],

        // Multiple edges with different data sizes
        vec![
            EdgeRecord::new(1, 10, 20, "DEFINES".to_string(), json!({})),
            EdgeRecord::new(2, 10, 30, "USES".to_string(), json!({"import": "std::io"})),
            EdgeRecord::new(3, 40, 10, "CONTAINS".to_string(), json!({"nested": {"deep": true}})),
        ],

        // Edges with large JSON payloads
        vec![
            EdgeRecord::new(1, 1, 2, "CALLS".to_string(), json!({
                "call_graph": {
                    "function": "main",
                    "line": 100,
                    "file": "src/main.rs",
                    "complexity": {
                        "cyclomatic": 5,
                        "cognitive": 3
                    }
                }
            })),
        ],
    ];

    for (i, edges) in test_cases.iter().enumerate() {
        let result = EdgeCluster::create_from_edges(edges, i as i64, Direction::Outgoing, &mut string_table);

        match edges.len() {
            0 => {
                assert!(result.is_ok(), "Empty cluster should be created successfully");
                let cluster = result.unwrap();
                assert_eq!(cluster.edge_count(), 0, "Empty cluster should have 0 edges");
                assert_eq!(cluster.size_bytes(), 8, "Empty cluster should only have header");
            }
            edge_count => {
                assert!(result.is_ok(), "Test case {}: Cluster creation failed: {:?}", i, result);
                let cluster = result.unwrap();
                assert_eq!(cluster.edge_count(), edge_count as u32, "Test case {}: Edge count mismatch", i);
                assert!(cluster.size_bytes() > 8, "Test case {}: Cluster should have payload", i);
            }
        }
    }
}

#[test]
fn test_v2_edge_cluster_corruption_detection() {
    // Test corruption detection for malformed cluster data

    // Test 1: Truncated header
    let truncated_header = vec![1, 0, 0, 0]; // Only 4 bytes, need 8
    let result = EdgeCluster::deserialize(&truncated_header);
    assert!(result.is_err(), "Should reject truncated header");

    // Test 2: Size mismatch
    let mut size_mismatch = vec![2, 0, 0, 0, 100, 0, 0, 0]; // Claims 100 bytes payload
    size_mismatch.extend_from_slice(&[0; 50]); // But only provides 58 bytes total
    let result = EdgeCluster::deserialize(&size_mismatch);
    assert!(result.is_err(), "Should reject size mismatch");

    // Test 3: Valid header but invalid edge record within payload
    let mut invalid_edge = vec![1, 0, 0, 0, 10, 0, 0, 0]; // 1 edge, 10 byte payload
    invalid_edge.extend_from_slice(&[0xFF, 0xFF]); // Start with invalid neighbor_id (all 0xFF)
    invalid_edge.extend_from_slice(&[0, 0]); // Invalid edge_type_offset
    invalid_edge.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]); // Some edge data
    let result = EdgeCluster::deserialize(&invalid_edge);
    // This might pass deserialization but fail validation
    if let Ok(cluster) = result {
        let validation_result = cluster.validate();
        assert!(validation_result.is_err(), "Should fail validation for invalid edge record");
    }
}

#[test]
fn test_v2_edge_cluster_direction_consistency() {
    // Test that clusters correctly filter edges by direction
    let mut string_table = create_test_string_table();

    let edges = vec![
        EdgeRecord::new(1, 100, 200, "CALLS".to_string(), json!({})),     // 100 -> 200
        EdgeRecord::new(2, 300, 100, "DEFINES".to_string(), json!({})),   // 300 -> 100
        EdgeRecord::new(3, 100, 400, "USES".to_string(), json!({})),      // 100 -> 400
        EdgeRecord::new(4, 500, 600, "CONTAINS".to_string(), json!({})),   // 500 -> 600
    ];

    // Test outgoing cluster from node 100
    let outgoing_cluster = EdgeCluster::create_from_edges(&edges, 100, Direction::Outgoing, &mut string_table)
        .expect("Should create outgoing cluster");

    // Should contain edges where from_id = 100
    assert_eq!(outgoing_cluster.edge_count(), 2, "Outgoing cluster should have 2 edges");

    let outgoing_neighbors: Vec<_> = outgoing_cluster.iter_neighbors().collect();
    assert!(outgoing_neighbors.contains(&200), "Should contain target 200");
    assert!(outgoing_neighbors.contains(&400), "Should contain target 400");
    assert!(!outgoing_neighbors.contains(&100), "Should not contain source 100");

    // Test incoming cluster to node 100
    let incoming_cluster = EdgeCluster::create_from_edges(&edges, 100, Direction::Incoming, &mut string_table)
        .expect("Should create incoming cluster");

    // Should contain edges where to_id = 100
    assert_eq!(incoming_cluster.edge_count(), 1, "Incoming cluster should have 1 edge");

    let incoming_neighbors: Vec<_> = incoming_cluster.iter_neighbors().collect();
    assert!(incoming_neighbors.contains(&300), "Should contain source 300");
    assert!(!incoming_neighbors.contains(&100), "Should not contain target 100");
}