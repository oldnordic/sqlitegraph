//! Cluster Allocation Collision Fix Validation Tests
//!
//! This test validates the surgical fix for the cluster allocation collision bug
//! that was causing neighbor_id==0 corruption when outgoing and incoming clusters
//! wrote to the same disk offset simultaneously.

use sqlitegraph::backend::native::{GraphFile, EdgeStore, NodeStore};
use sqlitegraph::backend::native::types::{EdgeRecord, NodeRecord};
use tempfile::NamedTempFile;

/// Test that cluster allocation prevents collisions between outgoing and incoming clusters
/// This is the core regression test for the neighbor_id==0 corruption bug.
#[test]
fn test_cluster_allocation_collision_prevention() {
    // Enable debug environment variables to trace allocation
    unsafe {
        std::env::set_var("CLUSTER_COLLISION_FIX_DEBUG", "1");
        std::env::set_var("V2_CLUSTER_AUDIT", "1");
    }

    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();

    // Create a V2 graph file (will automatically have V2 flags set)
    let mut graph_file = GraphFile::create(path).unwrap();

    // Create node 1 and node 2 using the legacy API that will be upgraded to V2
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let node1_record = NodeRecord::new(1, "node".to_string(), "node1".to_string(), serde_json::json!({}));
        let node2_record = NodeRecord::new(2, "node".to_string(), "node2".to_string(), serde_json::json!({}));

        node_store.write_node(&node1_record).unwrap();
        node_store.write_node(&node2_record).unwrap();
    } // node_store is dropped here, releasing the borrow

    // Create edge from node1 -> node2
    {
        let mut edge_store = EdgeStore::new(&mut graph_file);
        let edge = EdgeRecord::new(
            1, // edge_id
            1, // from_id (node1)
            2, // to_id (node2)
            "calls".to_string(),
            serde_json::json!({"weight": 1.0}),
        );

        // This should write both outgoing and incoming clusters without collision
        let result = edge_store.write_edge(&edge);
        assert!(result.is_ok(), "Edge insertion should succeed without cluster collision: {:?}", result.err());
    } // edge_store is dropped here, releasing the borrow

    // Verify nodes exist and have V2 cluster metadata
    let mut node_store = NodeStore::new(&mut graph_file);
    let node1_v2 = node_store.read_node_v2(1).unwrap();
    let node2_v2 = node_store.read_node_v2(2).unwrap();

    // Verify outgoing cluster for node1
    assert!(node1_v2.outgoing_cluster_offset > 0, "Node1 should have outgoing cluster");
    assert!(node1_v2.outgoing_cluster_size > 0, "Node1 should have non-zero outgoing cluster size");
    assert!(node1_v2.outgoing_edge_count >= 1, "Node1 should have at least 1 outgoing edge");

    // Verify incoming cluster for node2
    assert!(node2_v2.incoming_cluster_offset > 0, "Node2 should have incoming cluster");
    assert!(node2_v2.incoming_cluster_size > 0, "Node2 should have non-zero incoming cluster size");
    assert!(node2_v2.incoming_edge_count >= 1, "Node2 should have at least 1 incoming edge");

    // CRITICAL: Verify no overlap between outgoing and incoming clusters
    let node1_outgoing_end = node1_v2.outgoing_cluster_offset + node1_v2.outgoing_cluster_size as u64;
    let node2_incoming_start = node2_v2.incoming_cluster_offset;

    // Incoming cluster should start AFTER outgoing cluster ends (no overlap)
    assert!(
        node2_incoming_start >= node1_outgoing_end,
        "CRITICAL COLLISION TEST FAILED: Incoming cluster offset {} would overwrite outgoing cluster ending at {}",
        node2_incoming_start,
        node1_outgoing_end
    );

    println!("✅ CLUSTER COLLISION PREVENTION VALIDATION PASSED:");
    println!("   Node1 outgoing: offset={}, size={}, end={}",
        node1_v2.outgoing_cluster_offset, node1_v2.outgoing_cluster_size, node1_outgoing_end);
    println!("   Node2 incoming: offset={}, size={}, start={}",
        node2_v2.incoming_cluster_offset, node2_v2.incoming_cluster_size, node2_incoming_start);
    println!("   Separation gap: {} bytes", node2_incoming_start - node1_outgoing_end);
}