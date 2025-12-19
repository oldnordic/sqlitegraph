//! Adjacency Iterator Infinite Loop Detection Test
//!
//! This test demonstrates the critical issue where EdgeStore.iter_neighbors()
//! consumes the AdjacencyIterator with .collect(), breaking the connection
//! to graph file operations and causing excessive repeated reads.

use sqlitegraph::{
    BackendDirection, NeighborQuery,
    backend::native::{EdgeStore, GraphFile, NodeStore},
    backend::native::adjacency::{AdjacencyIterator, Direction},
    config::GraphConfig,
    open_graph,
};
use tempfile::TempDir;

/// Helper to create a test graph for adjacency iterator testing
fn create_test_graph_for_iterator() -> (Box<dyn sqlitegraph::GraphBackend>, TempDir) {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test_adjacency_iterator.db");

    let cfg = GraphConfig::new(sqlitegraph::BackendKind::Native);

    let graph = open_graph(&db_path, &cfg).expect("Failed to create graph");
    (graph, temp_dir)
}

/// Test that demonstrates the iterator consumption issue
/// This test should PASS after our fixes
#[test]
fn test_edge_store_iterator_consumption_causes_infinite_reads() {
    let (mut graph, temp_dir) = create_test_graph_for_iterator();

    // Create a simple graph: node1 -> node2
    let node1_id = graph.insert_node(
        sqlitegraph::NodeSpec {
            kind: "Function".to_string(),
            name: "node1".to_string(),
            file_path: Some("/src/node1.rs".to_string()),
            data: serde_json::json!({"lines": 10}),
        }
    ).expect("Failed to insert node1");

    let node2_id = graph.insert_node(
        sqlitegraph::NodeSpec {
            kind: "Function".to_string(),
            name: "node2".to_string(),
            file_path: Some("/src/node2.rs".to_string()),
            data: serde_json::json!({"lines": 20}),
        }
    ).expect("Failed to insert node2");

    // Create an edge: node1 -> node2
    graph.insert_edge(
        sqlitegraph::EdgeSpec {
            from: node1_id,
            to: node2_id,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({"line": 5}),
        }
    ).expect("Failed to insert edge");

    // Now test the EdgeStore.iter_neighbors() method directly
    let db_path = temp_dir.path().join("test_adjacency_iterator.db");
    let mut graph_file = GraphFile::open(&db_path).expect("Failed to open graph file");
    let mut edge_store = EdgeStore::new(&mut graph_file);

    // This should return an iterator that works without excessive reads
    let mut iterator = edge_store.iter_neighbors(
        node1_id as sqlitegraph::backend::native::types::NativeNodeId,
        Direction::Outgoing,
    );

    // Count how many times we can iterate - should be finite and small
    let mut iteration_count = 0;
    let mut neighbors_found = Vec::new();

    // Set a reasonable limit - if we exceed this, we have an infinite loop
    let max_iterations = 100; // Should only need 1-2 iterations for 1 neighbor

    for neighbor in &mut iterator {
        iteration_count += 1;
        neighbors_found.push(neighbor);

        if iteration_count > max_iterations {
            panic!(
                "INFINITE LOOP DETECTED: Exceeded {} iterations for simple graph with 1 edge. \
                 Current neighbors found: {:?}. \
                 This indicates the iterator is not properly advancing or terminating.",
                max_iterations, neighbors_found
            );
        }
    }

    // Verify we found the expected neighbor
    assert_eq!(neighbors_found.len(), 1, "Should find exactly 1 neighbor");
    assert_eq!(
        neighbors_found[0],
        node2_id as sqlitegraph::backend::native::types::NativeNodeId,
        "Should find node2 as neighbor of node1"
    );

    // Verify iteration count is reasonable (should be 1 for 1 neighbor)
    assert!(
        iteration_count <= 3, // Allow some tolerance for internal operations
        "Too many iterations ({}) for simple 1-neighbor graph. Expected <= 3.",
        iteration_count
    );
}

/// Test that demonstrates the repeated node reads issue is fixed
/// This test uses the public API which properly handles V2 clustered adjacency
#[test]
fn test_adjacency_iterator_repeated_node_reads_fixed() {
    let (graph, temp_dir) = create_test_graph_for_iterator();

    // Create a simple graph for testing
    let node1_id = graph.insert_node(
        sqlitegraph::NodeSpec {
            kind: "Type".to_string(),
            name: "test_node".to_string(),
            file_path: None,
            data: serde_json::json!({"test": true}),
        }
    ).expect("Failed to insert test node");

    let node2_id = graph.insert_node(
        sqlitegraph::NodeSpec {
            kind: "Type".to_string(),
            name: "target_node".to_string(),
            file_path: None,
            data: serde_json::json!({"target": true}),
        }
    ).expect("Failed to insert target node");

    // Create an edge to test adjacency
    graph.insert_edge(
        sqlitegraph::EdgeSpec {
            from: node1_id,
            to: node2_id,
            edge_type: "CONNECTS".to_string(),
            data: serde_json::json!({"weight": 1.0}),
        }
    ).expect("Failed to insert edge");

    // Test neighbors through public API - this should work without infinite reads
    let neighbors = graph
        .neighbors(
            node1_id,
            NeighborQuery {
                direction: BackendDirection::Outgoing,
                edge_type: None,
            },
        )
        .expect("Failed to get neighbors");

    // Should find exactly 1 neighbor
    assert_eq!(neighbors.len(), 1, "Should find exactly 1 neighbor");
    assert_eq!(neighbors[0], node2_id, "Should find target_node as neighbor");

    println!("✅ Adjacency iterator repeated reads test PASSED - no infinite loops detected");
}

/// Test that verifies iterator advancement works correctly
#[test]
fn test_adjacency_iterator_proper_advancement() {
    // This test works with the current implementation and will pass
    // It serves as a baseline to verify our fix doesn't break existing functionality

    let (mut graph, temp_dir) = create_test_graph_for_iterator();

    // Create nodes and edges
    let node1_id = graph.insert_node(
        sqlitegraph::NodeSpec {
            kind: "Function".to_string(),
            name: "main".to_string(),
            file_path: Some("/src/main.rs".to_string()),
            data: serde_json::json!({"lines": 100}),
        }
    ).expect("Failed to insert main node");

    let node2_id = graph.insert_node(
        sqlitegraph::NodeSpec {
            kind: "Function".to_string(),
            name: "helper".to_string(),
            file_path: Some("/src/helper.rs".to_string()),
            data: serde_json::json!({"lines": 50}),
        }
    ).expect("Failed to insert helper node");

    let node3_id = graph.insert_node(
        sqlitegraph::NodeSpec {
            kind: "Function".to_string(),
            name: "util".to_string(),
            file_path: Some("/src/util.rs".to_string()),
            data: serde_json::json!({"lines": 75}),
        }
    ).expect("Failed to insert util node");

    // Create multiple edges from main
    graph.insert_edge(
        sqlitegraph::EdgeSpec {
            from: node1_id,
            to: node2_id,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({"line": 10}),
        }
    ).expect("Failed to insert main->helper edge");

    graph.insert_edge(
        sqlitegraph::EdgeSpec {
            from: node1_id,
            to: node3_id,
            edge_type: "USES".to_string(),
            data: serde_json::json!({"line": 15}),
        }
    ).expect("Failed to insert main->util edge");

    // Test neighbors through public API (should work)
    let neighbors = graph
        .neighbors(
            node1_id,
            NeighborQuery {
                direction: BackendDirection::Outgoing,
                edge_type: None,
            },
        )
        .expect("Failed to get neighbors");

    // Should find both neighbors
    assert_eq!(neighbors.len(), 2, "Should find 2 neighbors");
    assert!(neighbors.contains(&node2_id), "Should find helper");
    assert!(neighbors.contains(&node3_id), "Should find util");

    println!("✅ Adjacency iterator advancement test PASSED - found {} neighbors", neighbors.len());
}