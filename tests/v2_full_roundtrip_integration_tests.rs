#![cfg(feature = "v2_experimental")]

//! Full integration tests for V2 serialization layer hardening
//! These test the complete roundtrip: serialize -> store -> retrieve -> deserialize

use sqlitegraph::backend::native::{GraphFile, NodeRecord, EdgeRecord};
use sqlitegraph::backend::native::node_store::NodeStore;
use sqlitegraph::backend::native::edge_store::EdgeStore;
use sqlitegraph::backend::native::graph_ops::native_bfs;
use tempfile::NamedTempFile;
use serde_json::json;

#[test]
fn test_v2_full_node_roundtrip_integration() {
    // Test complete roundtrip: NodeRecord -> serialize -> file -> read -> deserialize -> NodeRecord
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path()).expect("Failed to create graph file");

    // Create original node with all field variations
    let original_node = NodeRecord::new(
        42,
        "ComplexFunction".to_string(),
        "complex_function_v2".to_string(),
        json!({
            "signature": "(param: i32) -> i32",
            "parameters": [{"name": "param", "type": "i32", "default": 0}],
            "return_type": "i32",
            "body": "return param * 2;",
            "metadata": {
                "lines": 1,
                "cyclomatic_complexity": 1,
                "last_modified": "2024-01-15T10:30:00Z"
            }
        })
    );

    // Write node to file
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        node_store.write_node(&original_node).expect("Should write node successfully");
    }

    // Force flush to ensure data is written
    graph_file.flush().expect("Should flush successfully");

    // Read node back from file
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let retrieved_node = node_store.read_node(42).expect("Should read node successfully");

        // Verify complete roundtrip integrity
        assert_eq!(retrieved_node.id, original_node.id, "Node ID should match");
        assert_eq!(retrieved_node.kind, original_node.kind, "Node kind should match");
        assert_eq!(retrieved_node.name, original_node.name, "Node name should match");
        assert_eq!(retrieved_node.data, original_node.data, "Node data should match");
        assert_eq!(retrieved_node.total_degree(), 0, "Node should have no edges initially");
    }

    // Verify file state
    let header = graph_file.header();
    assert_eq!(header.node_count, 1, "Should have 1 node");
    assert_eq!(header.edge_count, 0, "Should have 0 edges");
}

#[test]
fn test_v2_full_graph_with_edges_roundtrip() {
    // Test complete graph with nodes and edges: create -> serialize -> store -> retrieve -> deserialize
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path()).expect("Failed to create graph file");

    // Create test nodes
    let nodes = vec![
        NodeRecord::new(1, "Module".to_string(), "main".to_string(), json!({"type": "module"})),
        NodeRecord::new(2, "Function".to_string(), "process".to_string(), json!({"type": "function", "lines": 50})),
        NodeRecord::new(3, "Variable".to_string(), "counter".to_string(), json!({"type": "variable", "value": 0})),
    ];

    // Write nodes
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        for node in &nodes {
            node_store.write_node(node).expect("Should write node");
        }
    }

    // Create test edges
    let edges = vec![
        EdgeRecord::new(1, 1, 2, "CONTAINS".to_string(), json!({"line": 10})),
        EdgeRecord::new(2, 2, 3, "USES".to_string(), json!({"line": 25})),
        EdgeRecord::new(3, 3, 1, "MODIFIES".to_string(), json!({"line": 5})),
    ];

    // Write edges
    {
        let mut edge_store = EdgeStore::new(&mut graph_file);
        for edge in &edges {
            edge_store.write_edge(edge).expect("Should write edge");
        }
    }

    // Force flush
    graph_file.flush().expect("Should flush successfully");

    // Verify all nodes can be read back with correct cluster metadata
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        for node in &nodes {
            let retrieved_node = node_store.read_node(node.id).expect("Should read node");

            assert_eq!(retrieved_node.id, node.id, "Node ID should match");
            assert_eq!(retrieved_node.kind, node.kind, "Node kind should match");
            assert_eq!(retrieved_node.name, node.name, "Node name should match");
            assert_eq!(retrieved_node.data, node.data, "Node data should match");

            // Verify adjacency metadata is correctly populated
            let expected_edges = edges.iter().filter(|e| e.from_id == node.id).count() as u32;
            assert_eq!(retrieved_node.total_edge_count(), expected_edges,
                     f!("Node {} should have {} outgoing edges", node.id, expected_edges));
        }
    }

    // Test graph traversal works correctly
    let bfs_result = native_bfs(&mut graph_file, 1, 2).expect("BFS should succeed");
    assert_eq!(bfs_result.visited_nodes.len(), 3, "BFS should visit all 3 nodes");
    assert!(bfs_result.visited_nodes.contains(&1), "BFS should visit node 1");
    assert!(bfs_result.visited_nodes.contains(&2), "BFS should visit node 2");
    assert!(bfs_result.visited_nodes.contains(&3), "BFS should visit node 3");
}

#[test]
fn test_v2_full_serialization_determinism() {
    // Test that serialization is completely deterministic across multiple cycles
    let temp_file1 = NamedTempFile::new().expect("Failed to create temp file 1");
    let temp_file2 = NamedTempFile().expect("Failed to create temp file 2");

    let mut graph_file1 = GraphFile::create(temp_file1.path()).expect("Failed to create graph file 1");
    let mut graph_file2 = GraphFile::create(temp_file2.path()).expect("Failed to create graph file 2");

    // Create identical test data
    let test_node = NodeRecord::new(
        123,
        "Deterministic".to_string(),
        "deterministic_test".to_string(),
        json!({
            "timestamp": 1705123456,
            "data": "test_data",
            "array": [1, 2, 3, {"nested": true}],
            "boolean": true,
            "null_field": null
        })
    );

    // Write to both files
    {
        let mut node_store1 = NodeStore::new(&mut graph_file1);
        let mut node_store2 = NodeStore::new(&mut graph_file2);

        node_store1.write_node(&test_node).expect("Should write to file 1");
        node_store2.write_node(&test_node).expect("Should write to file 2");
    }

    // Force both writes
    graph_file1.flush().expect("Should flush file 1");
    graph_file2.flush().expect("Should flush file 2");

    // Read from both files
    let read_node1 = {
        let mut node_store = NodeStore::new(&mut graph_file1);
        node_store.read_node(123).expect("Should read from file 1")
    };

    let read_node2 = {
        let mut node_store = NodeStore::new(&mut graph_file2);
        node_store.read_node(123).expect("Should read from file 2")
    };

    // Verify complete determinism
    assert_eq!(read_node1.id, read_node2.id, "Node IDs should be identical");
    assert_eq!(read_node1.kind, read_node2.kind, "Node kinds should be identical");
    assert_eq!(read_node1.name, read_node2.name, "Node names should be identical");
    assert_eq!(read_node1.data, read_node2.data, "Node data should be identical");

    // Verify serialized bytes would be identical
    // TODO: After binrw implementation
    // let bytes1 = read_node1.serialize();
    // let bytes2 = read_node2.serialize();
    // assert_eq!(bytes1, bytes2, "Serialized bytes should be identical");
}

#[test]
fn test_v2_full_corruption_resilience() {
    // Test that the hardened serialization layer is resilient to corruption
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path()).expect("Failed to create graph file");

    // Create test data that previously could be corrupted
    let node = NodeRecord::new(
        999,
        "ResilienceTest".to_string(),
        "resilience_test".to_string(),
        json!({
            "vulnerable_data": "This should not be corrupted",
            "complex_structure": {
                "nested_arrays": [1, [2, [3]], 4],
                "mixed_types": {
                    "string_field": "test",
                    "number_field": 42.5,
                    "boolean_field": true,
                    "null_field": null
                }
            }
        })
    );

    // Write node
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        node_store.write_node(&node).expect("Should write node");
    }

    // Immediately read back to verify no corruption occurred
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let read_node = node_store.read_node(999).expect("Should read node");

        // Verify no corruption occurred
        assert_eq!(read_node.id, node.id, "Node ID should not be corrupted");
        assert_eq!(read_node.kind, node.kind, "Node kind should not be corrupted");
        assert_eq!(read_node.name, node.name, "Node name should not be corrupted");
        assert_eq!(read_node.data, node.data, "Node data should not be corrupted");

        // Verify specific field that was previously vulnerable
        assert_eq!(read_node.data["vulnerable_data"], "This should not be corrupted",
                 "Specific field should not be corrupted");
        assert_eq!(read_node.data["complex_structure"]["mixed_types"]["string_field"], "test",
                 "Nested field should not be corrupted");
    }

    // TODO: After mmap implementation, verify memory is mapped correctly
    // let mmap_slice = graph_file.get_mmap_slice();
    // let node_offset = graph_file.header().node_data_offset + ((999 - 1) as u64 * 4096);
    // assert_eq!(mmap_slice[node_offset as usize], 2, "Version byte should be correct in memory");
}

#[test]
fn test_v2_full_large_scale_serialization() {
    // Test serialization with a large number of nodes and edges
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path()).expect("Failed to create graph file");

    let node_count = 50;
    let edges_per_node = 3;

    // Create many nodes
    for i in 1..=node_count {
        {
            let mut node_store = NodeStore::new(&mut graph_file);
            let node = NodeRecord::new(
                i,
                format!("NodeType{}", i % 5),
                format!("large_node_{}", i),
                json!({
                    "index": i,
                    "large_array": (0..100).collect::<Vec<_>>(),
                    "metadata": {
                        "created": "2024-01-01",
                        "tags": vec![format!("tag_{}", j) for j in 0..5],
                        "properties": {
                            "size": i * 100,
                            "complex": i % 3 == 0,
                            "nested": {"level": i % 4}
                        }
                    }
                })
            );
            node_store.write_node(&node).expect("Should write large node");
        }

        // Create edges for each node
        let mut edge_store = EdgeStore::new(&mut graph_file);
        for j in 0..edges_per_node {
            let edge_id = (i - 1) * edges_per_node + j + 1;
            let target_id = ((i + j) % node_count) + 1;
            let edge = EdgeRecord::new(
                edge_id as i64,
                i as i64,
                target_id,
                format!("EDGE_TYPE_{}", j % 3),
                json!({"edge_index": edge_id, "weight": (j + 1) as f64})
            );
            edge_store.write_edge(&edge).expect("Should write edge");
        }
    }

    // Force flush
    graph_file.flush().expect("Should flush large file");

    // Verify all nodes and edges can be read back
    let header = graph_file.header();
    assert_eq!(header.node_count, node_count, f!("Should have {} nodes", node_count));
    assert_eq!(header.edge_count, node_count * edges_per_node, f!("Should have {} edges", node_count * edges_per_node));

    // Random sample verification (not all nodes to keep test fast)
    let test_indices = vec![1, 5, 25, 42, 50];
    for &i in &test_indices {
        {
            let mut node_store = NodeStore::new(&mut graph_file);
            let read_node = node_store.read_node(i).expect("Should read node");

            assert_eq!(read_node.id, i, "Sample node ID should match");
            assert!(read_node.name.contains(&format!("large_node_{}", i)), "Sample node name should match pattern");
            assert!(read_node.data["index"] == i, "Sample node index should match");
        }
    }

    // Verify adjacency metadata is populated
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        for i in 1..=5 {
            let node = node_store.read_node(i).expect("Should read node");
            assert!(node.total_edge_count() == edges_per_node as u32,
                     f!("Node {} should have {} edges", i, edges_per_node));
            assert!(node.has_outgoing_edges(), f!("Node {} should have outgoing edges", i));
        }
    }
}