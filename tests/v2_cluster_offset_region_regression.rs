//! Phase 58 — V2 Cluster Offset Region Regression Test
//!
//! This test proves the cluster_floor layout bug where cluster_floor advances
//! beyond existing cluster offsets when node_count grows, making previously
//! written clusters "illegal" and causing read corruption.

use sqlitegraph::{
    open_graph, GraphConfig, BackendDirection, NeighborQuery, NodeSpec, EdgeSpec
};
use tempfile::TempDir;

/// Generate deterministic edges similar to Phase 55
fn generate_deterministic_edges(node_count: usize, edge_count: usize, node_ids: &[i64]) -> Vec<(usize, usize)> {
    let seed = 0xCAFE_BABE;
    let mut rng_state: u32 = seed;
    let mut edges = Vec::new();

    for _ in 0..edge_count {
        rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        let from_idx = rng_state as usize % node_count;

        rng_state = rng_state.wrapping_mul(1103515245).wrapping_add(12345);
        let mut to_idx = rng_state as usize % node_count;

        if to_idx == from_idx {
            to_idx = (to_idx + 1) % node_count;
        }

        edges.push((from_idx, to_idx));
    }

    edges
}

/// Test the cluster_floor layout invariant violation
#[test]
fn test_v2_cluster_floor_layout_violation() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("v2_layout_regression.db");

    // Use V2 NativeGraphBackend (same as Phase 55)
    let config = GraphConfig::native();
    let graph = open_graph(&db_path, &config)?;

    println!("=== STEP 1: Insert 1000 nodes ===");

    // Insert 1000 nodes to advance cluster_floor
    let mut node_ids = Vec::with_capacity(1000);
    for i in 0..1000 {
        let node_id = graph.insert_node(NodeSpec {
            kind: "TestNode".to_string(),
            name: format!("node_{}", i),
            file_path: None,
            data: serde_json::json!({"index": i}),
        })?;
        node_ids.push(node_id);
    }

    println!("✅ Inserted 1000 nodes successfully");

    // Get internal backend access to capture cluster_floor before edge insertion
    // We need to capture the cluster allocation state before writing edges
    println!("=== STEP 2: Check cluster allocation state ===");

    // Insert a single edge to trigger cluster allocation
    let first_edge_id = graph.insert_edge(EdgeSpec {
        from: node_ids[0],
        to: node_ids[1],
        edge_type: "test_edge".to_string(),
        data: serde_json::json!({"edge_index": 0}),
    })?;

    println!("✅ Inserted first edge, edge_id={}", first_edge_id);

    // Insert many more edges to trigger cluster growth and potential layout issues
    println!("=== STEP 3: Insert 3999 more edges (total: 4000) ===");

    let edges = generate_deterministic_edges(1000, 3999, &node_ids);
    for (i, &(from_idx, to_idx)) in edges.iter().enumerate() {
        let _edge_id = graph.insert_edge(EdgeSpec {
            from: node_ids[from_idx],
            to: node_ids[to_idx],
            edge_type: "test_edge".to_string(),
            data: serde_json::json!({"edge_index": i + 1}),
        })?;
    }

    println!("✅ Inserted all 4000 edges successfully");

    // STEP 4: Verify neighbor query triggers the corruption
    println!("=== STEP 4: Test neighbor query (corruption trigger) ===");

    // This should trigger the cluster reading corruption
    let neighbors = graph.neighbors(node_ids[0], NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    })?;

    println!("✅ Neighbor query succeeded, found {} neighbors", neighbors.len());

    // Verify we have the expected number of neighbors (should be > 0)
    assert!(!neighbors.is_empty(), "Node 0 should have outgoing neighbors after 4000 edge insertions");

    println!("=== TEST PASSED: No corruption detected ===");
    Ok(())
}

/// Test smaller scale to isolate the exact failure point
#[test]
fn test_v2_cluster_floor_small_scale() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("v2_small_regression.db");

    let config = GraphConfig::native();
    let graph = open_graph(&db_path, &config)?;

    // Insert 100 nodes (smaller scale)
    let mut node_ids = Vec::with_capacity(100);
    for i in 0..100 {
        let node_id = graph.insert_node(NodeSpec {
            kind: "TestNode".to_string(),
            name: format!("node_{}", i),
            file_path: None,
            data: serde_json::json!({"index": i}),
        })?;
        node_ids.push(node_id);
    }

    println!("✅ Inserted 100 nodes");

    // Insert 400 edges (4 per node average)
    let edges = generate_deterministic_edges(100, 400, &node_ids);
    for (i, &(from_idx, to_idx)) in edges.iter().enumerate() {
        let _edge_id = graph.insert_edge(EdgeSpec {
            from: node_ids[from_idx],
            to: node_ids[to_idx],
            edge_type: "test_edge".to_string(),
            data: serde_json::json!({"edge_index": i}),
        })?;
    }

    println!("✅ Inserted 400 edges");

    // Test neighbor query
    let neighbors = graph.neighbors(node_ids[0], NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    })?;

    println!("✅ Neighbor query succeeded, {} neighbors", neighbors.len());
    assert!(!neighbors.is_empty());

    Ok(())
}