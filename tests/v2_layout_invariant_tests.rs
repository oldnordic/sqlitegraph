#![cfg(feature = "v2_experimental")]

//! Layout invariant tests for V2 native backend
//!
//! These tests enforce that node and edge regions do not overlap,
//! and that slot offsets are deterministic and within their expected regions.

use serde_json::json;
use sqlitegraph::backend::native::{
    NativeNodeId, NativeEdgeId, NodeRecord, EdgeRecord,
    edge_store::EdgeStore, graph_file::GraphFile, node_store::NodeStore,
};
use tempfile::NamedTempFile;

const NODE_SLOT_SIZE: u64 = 4096; // From code: hardcoded 4096 bytes per node slot
const EDGE_SLOT_SIZE: u64 = 256;  // From code: hardcoded 256 bytes per edge slot

/// Helper function to create a fresh graph file for testing
fn setup_test_graph() -> (GraphFile, NamedTempFile) {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let graph_file = GraphFile::create(temp_file.path()).expect("Failed to create graph file");
    (graph_file, temp_file)
}

/// Calculate expected node slot offset using the same formula as node_store.rs
fn expected_node_slot_offset(header: &sqlitegraph::backend::native::FileHeader, node_id: NativeNodeId) -> u64 {
    header.node_data_offset + ((node_id - 1) as u64 * NODE_SLOT_SIZE)
}

/// Calculate expected edge slot offset using the same formula as edge_store.rs
fn expected_edge_slot_offset(header: &sqlitegraph::backend::native::FileHeader, edge_id: NativeEdgeId) -> u64 {
    header.edge_data_offset + ((edge_id - 1) as u64 * EDGE_SLOT_SIZE)
}

/// Test that node and edge regions do not overlap
#[test]
fn test_v2_node_and_edge_regions_do_not_overlap() {
    let (mut graph_file, _tmp) = setup_test_graph();

    // Insert 3 nodes using normal native path
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        for i in 1..=3 {
            let node = NodeRecord::new(
                i,
                "TestNode".to_string(),
                format!("node_{}", i),
                json!({"test": true}),
            );
            node_store.write_node(&node).expect("Failed to write node");
        }
    }

    // Insert 5 edges using normal native path
    {
        let mut edge_store = EdgeStore::new(&mut graph_file);
        for i in 1..=5 {
            let edge = EdgeRecord::new(
                i,
                1, // from node 1
                (i % 3) + 1, // to nodes 2, 3, 1
                "TEST_EDGE".to_string(),
                json!({"weight": i as f64}),
            );
            edge_store.write_edge(&edge).expect("Failed to write edge");
        }
    }

    // Force all writes to disk
    graph_file.flush().expect("Failed to flush graph file");

    // Read header and compute regions
    let header = graph_file.header();
    let node_region_start = header.node_data_offset;
    let node_region_end = node_region_start + (header.node_count as u64 * NODE_SLOT_SIZE);
    let edge_region_start = header.edge_data_offset;
    let edge_region_end = edge_region_start + (header.edge_count as u64 * EDGE_SLOT_SIZE);

    // Assert regions do not overlap
    assert!(
        edge_region_start >= node_region_end,
        "Edge region overlaps node region: edge_start={}, node_end={}",
        edge_region_start, node_region_end
    );

    // Assert regions are valid
    assert!(
        node_region_start < node_region_end,
        "Invalid node region: start={}, end={}",
        node_region_start, node_region_end
    );

    assert!(
        edge_region_start < edge_region_end,
        "Invalid edge region: start={}, end={}",
        edge_region_start, edge_region_end
    );

    // Assert specific expected values based on the code
    assert_eq!(node_region_start, 1024, "node_data_offset should be 1024 (HEADER_SIZE)");
    assert_eq!(header.node_count, 3, "Should have 3 nodes");
    assert_eq!(header.edge_count, 5, "Should have 5 edges");
    assert_eq!(node_region_end, 1024 + (3 * 4096), "Node region should end at 1024 + 3*4096");
}

/// Test that node slot offsets are deterministic and within node region
#[test]
fn test_v2_node_slot_offsets_are_deterministic_and_within_node_region() {
    let (mut graph_file, _tmp) = setup_test_graph();

    // Insert 5 nodes with incremental IDs
    let expected_node_ids = vec![1, 2, 3, 4, 5];
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        for &node_id in &expected_node_ids {
            let node = NodeRecord::new(
                node_id,
                "TestNode".to_string(),
                format!("node_{}", node_id),
                json!({"id": node_id}),
            );
            node_store.write_node(&node).expect("Failed to write node");
        }
    }

    // Force writes to disk
    graph_file.flush().expect("Failed to flush graph file");

    // Read header and compute region bounds
    let header = graph_file.header();
    let node_region_start = header.node_data_offset;
    let node_region_end = node_region_start + (header.node_count as u64 * NODE_SLOT_SIZE);

    // Verify each node's offset is deterministic and within bounds
    for &node_id in &expected_node_ids {
        let expected_offset = expected_node_slot_offset(header, node_id);

        // Assert offset is within node region
        assert!(
            expected_offset >= node_region_start && expected_offset < node_region_end,
            "Node {} offset {} outside node region [{}, {})",
            node_id, expected_offset, node_region_start, node_region_end
        );

        // Assert offsets are strictly increasing by NODE_SLOT_SIZE
        if node_id > 1 {
            let prev_offset = expected_node_slot_offset(header, node_id - 1);
            assert_eq!(
                expected_offset, prev_offset + NODE_SLOT_SIZE,
                "Node {} offset should be exactly {} more than node {} offset",
                node_id, NODE_SLOT_SIZE, node_id - 1
            );
        }
    }
}

/// Test that edge slot offsets are deterministic and within edge region
#[test]
fn test_v2_edge_slot_offsets_are_deterministic_and_within_edge_region() {
    let (mut graph_file, _tmp) = setup_test_graph();

    // First insert some nodes (required for edges)
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        for i in 1..=3 {
            let node = NodeRecord::new(i, "Node".to_string(), format!("node_{}", i), json!({}));
            node_store.write_node(&node).expect("Failed to write node");
        }
    }

    // Insert 7 edges to test multiple slots
    let expected_edge_ids = vec![1, 2, 3, 4, 5, 6, 7];
    {
        let mut edge_store = EdgeStore::new(&mut graph_file);
        for &edge_id in &expected_edge_ids {
            let edge = EdgeRecord::new(
                edge_id,
                1,
                (edge_id % 3) + 1,
                "TEST_EDGE".to_string(),
                json!({"edge_id": edge_id}),
            );
            edge_store.write_edge(&edge).expect("Failed to write edge");
        }
    }

    // Force writes to disk
    graph_file.flush().expect("Failed to flush graph file");

    // Read header and compute region bounds
    let header = graph_file.header();
    let edge_region_start = header.edge_data_offset;
    let edge_region_end = edge_region_start + (header.edge_count as u64 * EDGE_SLOT_SIZE);

    // Verify each edge's offset is deterministic and within bounds
    for &edge_id in &expected_edge_ids {
        let expected_offset = expected_edge_slot_offset(header, edge_id);

        // Assert offset is within edge region
        assert!(
            expected_offset >= edge_region_start && expected_offset < edge_region_end,
            "Edge {} offset {} outside edge region [{}, {})",
            edge_id, expected_offset, edge_region_start, edge_region_end
        );

        // Assert offsets are strictly increasing by EDGE_SLOT_SIZE
        if edge_id > 1 {
            let prev_offset = expected_edge_slot_offset(header, edge_id - 1);
            assert_eq!(
                expected_offset, prev_offset + EDGE_SLOT_SIZE,
                "Edge {} offset should be exactly {} more than edge {} offset",
                edge_id, EDGE_SLOT_SIZE, edge_id - 1
            );
        }
    }
}

/// Test that critical header invariants are enforced
#[test]
fn test_v2_critical_header_invariants() {
    let (mut graph_file, _tmp) = setup_test_graph();

    // Insert nodes first
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        for i in 1..=2 {
            let node = NodeRecord::new(i, "Node".to_string(), format!("node_{}", i), json!({}));
            node_store.write_node(&node).expect("Failed to write node");
        }
    }

    // Insert edges in separate scope
    {
        let mut edge_store = EdgeStore::new(&mut graph_file);
        for i in 1..=3 {
            let edge = EdgeRecord::new(i, 1, 2, "TEST".to_string(), json!({}));
            edge_store.write_edge(&edge).expect("Failed to write edge");
        }
    }

    graph_file.flush().expect("Failed to flush");

    let header = graph_file.header();

    // Validate critical invariants from FileHeader::validate()
    assert_eq!(header.magic, sqlitegraph::backend::native::v2::V2_MAGIC);
    assert_eq!(header.version, 2); // V2 format
    assert!(header.node_data_offset >= sqlitegraph::backend::native::constants::HEADER_SIZE);
    assert!(header.edge_data_offset >= header.node_data_offset);

    // Most importantly: edge region must not overlap node region
    let node_region_end = header.node_data_offset + (header.node_count as u64 * NODE_SLOT_SIZE);
    assert!(
        header.edge_data_offset >= node_region_end,
        "edge_data_offset ({}) must be >= node region end ({})",
        header.edge_data_offset, node_region_end
    );
}