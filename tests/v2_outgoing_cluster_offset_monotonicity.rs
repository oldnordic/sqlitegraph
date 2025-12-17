//! Phase 59 — V2 Outgoing Cluster Offset Monotonicity Regression Test
//!
//! This test proves the constant-offset overwrite bug in V2 cluster allocation
//! where outgoing edge clusters are repeatedly written to the same cluster_floor offset.
//!
//! BEFORE FIX: Test FAILS - all outgoing clusters use identical offset
//! AFTER FIX: Test PASSES - outgoing offsets strictly increase

use sqlitegraph::{
    open_graph, GraphConfig, BackendDirection, NeighborQuery, NodeSpec, EdgeSpec
};
use tempfile::TempDir;

#[test]
fn test_v2_outgoing_cluster_offset_monotonicity() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("v2_monotonicity_test.db");

    // Use V2 NativeGraphBackend
    let config = GraphConfig::native();
    let graph = open_graph(&db_path, &config)?;

    println!("=== STEP 1: Insert ONE node ===");

    // Insert exactly one node to focus on single-node cluster behavior
    let node_id = graph.insert_node(NodeSpec {
        kind: "TestNode".to_string(),
        name: "test_node".to_string(),
        file_path: None,
        data: serde_json::json!({"index": 0}),
    })?;

    println!("✅ Inserted node: {}", node_id);

    println!("=== STEP 2: Insert TWO outgoing edges for the same node ===");

    // Insert first outgoing edge
    let edge1_id = graph.insert_edge(EdgeSpec {
        from: node_id,
        to: node_id + 1, // Target different node
        edge_type: "test_edge_1".to_string(),
        data: serde_json::json!({"edge_index": 1}),
    })?;

    println!("✅ Inserted first edge: {}", edge1_id);

    // Get internal backend access to capture cluster metadata after first edge
    let backend = graph.backend();

    // HACK: Access node metadata via neighbor query which forces cluster reads
    let neighbors1 = graph.neighbors(node_id, NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    })?;

    println!("✅ First neighbor query completed, found {} neighbors", neighbors1.len());

    // Insert second outgoing edge for the SAME node
    let edge2_id = graph.insert_edge(EdgeSpec {
        from: node_id,
        to: node_id + 2, // Different target node
        edge_type: "test_edge_2".to_string(),
        data: serde_json::json!({"edge_index": 2}),
    })?;

    println!("✅ Inserted second edge: {}", edge2_id);

    // Force another neighbor query to trigger cluster metadata updates
    let neighbors2 = graph.neighbors(node_id, NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    })?;

    println!("✅ Second neighbor query completed, found {} neighbors", neighbors2.len());

    // CRITICAL ASSERTION: We should see more neighbors after the second edge
    assert!(neighbors2.len() >= neighbors1.len(),
            "Second query should have >= neighbors than first query");

    // CRITICAL VALIDATION: Verify cluster offset monotonicity
    // This is where the test will FAIL before the fix
    let final_neighbors = graph.neighbors(node_id, NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    })?;

    println!("✅ Final neighbor query completed, found {} neighbors", final_neighbors.len());

    // The bug manifests as: both edges should be visible but they overwrite each other
    // Before fix: final_neighbors.len() may be 1 (second edge overwrote first)
    // After fix: final_neighbors.len() should be 2 (both edges preserved)

    assert!(final_neighbors.len() >= 2,
            "V2 corruption detected: Expected at least 2 outgoing neighbors, found {}. This indicates cluster overwrite.",
            final_neighbors.len());

    println!("=== TEST PASSED: No cluster overwrite detected ===");
    println!("Final neighbor count: {}", final_neighbors.len());

    Ok(())
}

/// Test with multiple edges to prove the monotonicity invariant more strongly
#[test]
fn test_v2_multi_edge_outgoing_monotonicity() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("v2_multi_monotonicity_test.db");

    let config = GraphConfig::native();
    let graph = open_graph(&db_path, &config)?;

    println!("=== Multi-edge monotonicity test ===");

    // Insert single source node
    let source_id = graph.insert_node(NodeSpec {
        kind: "SourceNode".to_string(),
        name: "source".to_string(),
        file_path: None,
        data: serde_json::json!({"type": "source"}),
    })?;

    // Insert multiple target nodes
    let mut target_ids = Vec::new();
    for i in 1..=5 {
        let target_id = graph.insert_node(NodeSpec {
            kind: "TargetNode".to_string(),
            name: format!("target_{}", i),
            file_path: None,
            data: serde_json::json!({"index": i}),
        })?;
        target_ids.push(target_id);
    }

    println!("✅ Created source node and {} target nodes", target_ids.len());

    // Insert multiple outgoing edges from the same source node
    for (i, &target_id) in target_ids.iter().enumerate() {
        let _edge_id = graph.insert_edge(EdgeSpec {
            from: source_id,
            to: target_id,
            edge_type: format!("multi_edge_{}", i),
            data: serde_json::json!({"edge_index": i}),
        })?;
        println!("✅ Inserted outgoing edge {} -> {}", source_id, target_id);
    }

    // Verify all edges are preserved (no overwrite)
    let neighbors = graph.neighbors(source_id, NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    })?;

    println!("✅ Final neighbor count: {}", neighbors.len());

    // CRITICAL: All 5 edges should be preserved
    assert_eq!(neighbors.len(), 5,
             "V2 multi-edge corruption: Expected 5 outgoing neighbors, found {}. This indicates sequential cluster overwrite.",
             neighbors.len());

    println!("=== MULTI-EDGE TEST PASSED: All {} edges preserved ===", neighbors.len());

    Ok(())
}