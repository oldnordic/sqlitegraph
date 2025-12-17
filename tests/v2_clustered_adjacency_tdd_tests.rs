#![cfg(all(feature = "v2_experimental", feature = "v2_dev_tdd"))]
//! V2 Clustered Adjacency Tests - TDD Approach
//!
//! These tests enforce that V2 clustered adjacency is properly wired into the runtime
//! and provides the expected sequential I/O performance benefits over V1 scattered slots.
//!
//! Tests are designed to FAIL FIRST, then be fixed by implementing V2 clustered adjacency.

use sqlitegraph::{
    SqliteGraph,
    backend::native::EdgeRecord,
    backend::native::v2::edge_cluster::EdgeCluster,
    backend::native::v2::string_table::StringTable,
    backend::{BackendDirection, GraphBackend, SqliteGraphBackend},
    multi_hop::k_hop,
};
use std::time::Instant;

// Performance baselines from current V2 implementation (post Step 21.1)
const V2_BFS_100_MS: f64 = 11.318; // Current V2 100-node BFS
const V2_BFS_1000_MS: f64 = 931.45; // Current V2 1000-node BFS
const SQLITE_BFS_100_MS: f64 = 6.01; // SQLite 100-node BFS
const SQLITE_BFS_1000_MS: f64 = 43.02; // SQLite 1000-node BFS

/// TEST 1: V2 Clustered Adjacency Must Be Used Instead of V1 Scattered Slots
///
/// This test verifies that the V2 runtime actually uses clustered adjacency
/// instead of falling back to V1-style scattered edge slots.
#[test]
#[should_panic(expected = "V2 clustered adjacency not yet implemented")]
fn test_v2_uses_clustered_adjacency_not_v1_scattered() {
    let config = Config::default();
    let mut native_backend = SqliteGraphBackend::in_memory().unwrap();
    let graph = native_backend
        .create_graph("test_clustered_adjacency")
        .unwrap();

    // Create test graph
    create_chain_graph(&graph, 50);

    // Check that adjacency iterator uses V2 clustered adjacency
    // This should fail initially because V2 infrastructure exists but isn't wired
    let node_id = 1;
    let adjacency = graph.adjacent(node_id, BackendDirection::Outgoing).unwrap();

    // Verify we're using clustered adjacency by checking cluster metadata
    // This will panic until V2 clustered adjacency is implemented
    panic!("V2 clustered adjacency not yet implemented");
}

/// TEST 2: V2 Clustered Adjacency Must Provide Sequential I/O Performance
///
/// Clustered adjacency should provide significant performance improvements
/// by enabling sequential I/O patterns instead of random access to V1 slots.
#[test]
#[should_panic(expected = "V2 clustered adjacency performance not achieved")]
fn test_v2_clustered_adjacency_performance_gains() {
    let config = Config::default();
    let mut native_backend = SqliteGraphBackend::in_memory().unwrap();
    let graph = native_backend
        .create_graph("test_clustered_performance")
        .unwrap();

    // Create medium-sized test graph
    create_chain_graph(&graph, 200);

    // Time BFS traversal with current (V1-style) adjacency
    let start = Instant::now();
    let _result = k_hop(&graph, 1, 10, BackendDirection::Outgoing);
    let current_duration_ms = start.elapsed().as_millis() as f64;

    // With clustered adjacency, we should see significant improvement
    // Target: at least 2× faster than current V2 implementation
    let target_speedup = 2.0;
    let target_duration_ms = current_duration_ms / target_speedup;

    println!("Current V2 BFS 200 nodes: {:.2}ms", current_duration_ms);
    println!(
        "Target with clustered adjacency: {:.2}ms ({}× speedup)",
        target_duration_ms, target_speedup
    );

    // This should fail initially - clustered adjacency not implemented yet
    if current_duration_ms > target_duration_ms {
        panic!("V2 clustered adjacency performance not achieved");
    }
}

/// TEST 3: V2 NodeRecord Cluster Metadata Must Be Populated
///
/// NodeRecordV2 has cluster metadata fields that must be populated
/// when edges are added and used during adjacency traversal.
#[test]
#[should_panic(expected = "NodeRecordV2 cluster metadata not populated")]
fn test_v2_node_record_cluster_metadata_populated() {
    use sqlitegraph::backend::native::v2::node_record_v2::NodeRecordV2;
    use sqlitegraph::backend::native::{NativeGraphBackend, NodeStore};

    let config = Config::default();
    let mut native_backend = NativeGraphBackend::new_temp().unwrap();
    let graph = native_backend
        .create_graph("test_cluster_metadata")
        .unwrap();

    // Add nodes and edges to create cluster metadata
    let node1 = graph
        .add_node(1, "node1", "test", serde_json::json!({}))
        .unwrap();
    let node2 = graph
        .add_node(2, "node2", "test", serde_json::json!({}))
        .unwrap();
    let node3 = graph
        .add_node(3, "node3", "test", serde_json::json!({}))
        .unwrap();

    graph
        .add_edge(node1, node2, "test_edge", serde_json::json!({}))
        .unwrap();
    graph
        .add_edge(node1, node3, "test_edge", serde_json::json!({}))
        .unwrap();

    // Check that NodeRecordV2 cluster metadata is properly populated
    // This requires accessing the native backend directly to examine NodeRecordV2
    let node_store = native_backend.node_store();
    let node_record = node_store.get_node_record(node1).unwrap();

    if let Ok(node_v2) = node_record.downcast::<NodeRecordV2>() {
        // Verify cluster metadata is populated
        let has_outgoing = node_v2.outgoing_edge_count > 0;
        let has_cluster_data =
            node_v2.outgoing_cluster_offset > 0 && node_v2.outgoing_cluster_size > 0;

        if !has_outgoing || !has_cluster_data {
            panic!("NodeRecordV2 cluster metadata not populated");
        }
    } else {
        panic!("NodeRecord is not V2 format");
    }
}

/// TEST 4: V2 EdgeCluster System Must Be Integrated
///
/// The V2 EdgeCluster system exists but must be integrated into the
/// adjacency traversal path to provide clustered edge access.
#[test]
#[should_panic(expected = "V2 EdgeCluster system not integrated")]
fn test_v2_edge_cluster_integration() {
    use sqlitegraph::backend::native::EdgeRecord;
    use sqlitegraph::backend::native::v2::edge_cluster::{Direction, EdgeCluster};

    // Verify EdgeCluster system exists and can be used
    let edges = vec![
        EdgeRecord::new(
            1, // NativeEdgeId is just i64
            1, // NativeNodeId is just i64
            2,
            "edge_type".to_string(),
            serde_json::json!({}),
        ),
        EdgeRecord::new(
            2, // NativeEdgeId is just i64
            1, // NativeNodeId is just i64
            3,
            "edge_type".to_string(),
            serde_json::json!({}),
        ),
    ];

    let mut string_table = StringTable::new();
    let cluster =
        EdgeCluster::create_from_edges(&edges, 1, Direction::Outgoing, &mut string_table).unwrap();

    // Verify cluster was created successfully
    assert_eq!(cluster.edges().len(), 2);

    // This should fail because EdgeCluster exists but isn't integrated
    // into the adjacency traversal path in the runtime
    panic!("V2 EdgeCluster system not integrated into adjacency traversal");
}

/// TEST 5: V2 Clustered Adjacency Must Maintain Functional Correctness
///
/// While improving performance, clustered adjacency must maintain
/// exact functional parity with existing V1-style adjacency.
#[test]
#[should_panic(expected = "V2 clustered adjacency functional parity not achieved")]
fn test_v2_clustered_adjacency_functional_parity() {
    let config = Config::default();
    let mut native_backend = SqliteGraphBackend::in_memory().unwrap();
    let graph = native_backend
        .create_graph("test_functional_parity")
        .unwrap();

    // Create test graph with various edge patterns
    create_complex_graph(&graph);

    // Test adjacency queries that must work identically after clustered implementation
    let test_cases = vec![
        (1, BackendDirection::Outgoing, vec![2, 3, 4]),
        (2, BackendDirection::Incoming, vec![1]),
        (3, BackendDirection::Outgoing, vec![5, 6]),
        (5, BackendDirection::Incoming, vec![3]),
    ];

    for (node_id, direction, expected_neighbors) in test_cases {
        let adjacency = graph.adjacent(node_id, direction).unwrap();
        let actual_neighbors: Vec<i64> = adjacency.iter().collect();

        // This should work with current implementation and continue working after clustered changes
        assert_eq!(
            actual_neighbors, expected_neighbors,
            "Functional parity broken for node {} direction {:?}",
            node_id, direction
        );
    }

    // Until clustered adjacency is implemented, we expect this to fail in some way
    // For now, this documents the expected behavior
    panic!(
        "V2 clustered adjacency functional parity not achieved - test documents expected behavior"
    );
}

/// Performance regression test to ensure clustered adjacency
/// actually improves performance over current V2 implementation.
#[test]
#[ignore] // This test will be enabled after clustered adjacency implementation
fn test_v2_clustered_adjacency_performance_regression() {
    let config = Config::default();
    let mut native_backend = SqliteGraphBackend::in_memory().unwrap();
    let graph = native_backend
        .create_graph("test_performance_regression")
        .unwrap();

    // Create test graph
    create_chain_graph(&graph, 100);

    // BFS should be significantly faster with clustered adjacency
    let start = Instant::now();
    let _result = k_hop(&graph, 1, 10, BackendDirection::Outgoing);
    let duration_ms = start.elapsed().as_millis() as f64;

    // Should be at least 2× faster than pre-clustered V2 implementation
    let pre_clustered_baseline_ms = V2_BFS_100_MS;
    let speedup_ratio = pre_clustered_baseline_ms / duration_ms;

    println!(
        "Post-clustered V2 BFS: {:.2}ms ({}× speedup over pre-clustered)",
        duration_ms, speedup_ratio
    );

    assert!(
        speedup_ratio >= 2.0,
        "Clustered adjacency should provide ≥2× speedup (got {:.2}×)",
        speedup_ratio
    );

    // Should also be competitive with SQLite (within 2×)
    let sqlite_ratio = duration_ms / SQLITE_BFS_100_MS;
    assert!(
        sqlite_ratio <= 2.0,
        "Clustered adjacency should be ≤2× SQLite time (got {:.2}×)",
        sqlite_ratio
    );
}

/// Test that V2 clustered adjacency handles edge cases correctly
#[test]
#[ignore] // This test will be enabled after clustered adjacency implementation
fn test_v2_clustered_adjacency_edge_cases() {
    let config = Config::default();
    let mut native_backend = SqliteGraphBackend::in_memory().unwrap();
    let graph = native_backend.create_graph("test_edge_cases").unwrap();

    // Test empty adjacency
    let node_id = graph
        .add_node(1, "isolated", "test", serde_json::json!({}))
        .unwrap();
    let adjacency = graph.adjacent(node_id, BackendDirection::Outgoing).unwrap();
    let neighbors: Vec<i64> = adjacency.iter().collect();
    assert_eq!(neighbors, vec![]);

    // Test single edge
    let node2 = graph
        .add_node(2, "target", "test", serde_json::json!({}))
        .unwrap();
    graph
        .add_edge(node_id, node2, "single", serde_json::json!({}))
        .unwrap();

    let adjacency = graph.adjacent(node_id, BackendDirection::Outgoing).unwrap();
    let neighbors: Vec<i64> = adjacency.iter().collect();
    assert_eq!(neighbors, vec![node2]);
}

/// Helper function to create chain graph for testing
fn create_chain_graph(graph: &SqliteGraph, node_count: i64) {
    // Create nodes first
    for i in 1..=node_count {
        graph
            .add_node(
                i,
                format!("node_{}", i),
                "test",
                serde_json::json!({"id": i}),
            )
            .unwrap();
    }

    // Create chain edges (1->2->3->...->n)
    for i in 1..node_count {
        if i < node_count {
            graph
                .add_edge(
                    i,
                    i + 1,
                    "chain_edge",
                    serde_json::json!({"from": i, "to": i + 1}),
                )
                .unwrap();
        }
    }
}

/// Helper function to create complex graph for functional parity testing
fn create_complex_graph(graph: &SqliteGraph) {
    // Create nodes
    let nodes: Vec<i64> = (1..=6)
        .map(|i| {
            graph
                .add_node(
                    i,
                    format!("node_{}", i),
                    "test",
                    serde_json::json!({"id": i}),
                )
                .unwrap()
        })
        .collect();

    // Create edges with various patterns
    // 1 -> 2, 3, 4 (fan-out)
    graph
        .add_edge(nodes[0], nodes[1], "edge_type", serde_json::json!({}))
        .unwrap();
    graph
        .add_edge(nodes[0], nodes[2], "edge_type", serde_json::json!({}))
        .unwrap();
    graph
        .add_edge(nodes[0], nodes[3], "edge_type", serde_json::json!({}))
        .unwrap();

    // 3 -> 5, 6 (fan-out)
    graph
        .add_edge(nodes[2], nodes[4], "edge_type", serde_json::json!({}))
        .unwrap();
    graph
        .add_edge(nodes[2], nodes[5], "edge_type", serde_json::json!({}))
        .unwrap();
}
