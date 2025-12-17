#![cfg(feature = "v2_experimental")]

//! Tests for mmap I/O invariants in GraphFile
//! These tests will fail initially and should pass after mmap implementation

use sqlitegraph::backend::native::{GraphFile, NodeRecord, EdgeRecord};
use sqlitegraph::backend::native::node_store::NodeStore;
use sqlitegraph::backend::native::edge_store::EdgeStore;
use tempfile::NamedTempFile;
use serde_json::json;

#[test]
fn test_v2_mmap_direct_access_consistency() {
    // Test that direct mmap access produces consistent results
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path()).expect("Failed to create graph file");

    // Write a node
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let node = NodeRecord::new(1, "TestNode".to_string(), "test".to_string(), json!({"data": 42}));
        node_store.write_node(&node).expect("Should write node");
    }

    // Force flush to ensure data is written
    graph_file.flush().expect("Should flush");

    // TODO: After mmap implementation, test direct memory access
    // let mmap_slice = graph_file.get_mmap_slice();
    // let node_slot_offset = graph_file.header().node_data_offset + ((1 - 1) as u64 * 4096);
    //
    // Verify we can read the node data directly from mmap
    // let version_byte = mmap_slice[node_slot_offset as usize];
    // assert_eq!(version_byte, 2, "Should read V2 version byte from mmap");

    // For now, verify through regular API that data exists
    let header = graph_file.header();
    assert_eq!(header.node_count, 1, "Node count should be 1");
    assert!(header.node_data_offset >= 1024, "Node data offset should be valid");
}

#[test]
fn test_v2_mmap_write_immediate_visibility() {
    // Test that writes through mmap are immediately visible
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path()).expect("Failed to create graph file");

    // TODO: After mmap implementation, write directly through mmap
    // let mmap_slice = graph_file.get_mmap_slice_mut();
    // let node_slot_offset = graph_file.header().node_data_offset;
    //
    // Write V2 node header directly to mmap
    // mmap_slice[node_slot_offset as usize] = 2; // Version
    // mmap_slice[node_slot_offset as usize + 1] = 0; // Start of flags
    // ... continue writing full node record

    // For now, ensure current behavior works
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let node = NodeRecord::new(2, "DirectWrite".to_string(), "direct".to_string(), json!({"test": true}));
        node_store.write_node(&node).expect("Should write node");
    }

    // Verify we can read it back immediately
    let header = graph_file.header();
    assert_eq!(header.node_count, 1, "Should have written 1 node");

    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let read_node = node_store.read_node(2).expect("Should read node");
        assert_eq!(read_node.name, "direct", "Should read correct node name");
    }
}

#[test]
fn test_v2_mmap_no_buffer_corruption() {
    // Test that mmap eliminates buffer corruption issues
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path()).expect("Failed to create graph file");

    // Create scenario that previously caused corruption: write node then edge
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let node = NodeRecord::new(1, "CorruptionTest".to_string(), "corruption".to_string(), json!({"vulnerable": true}));
        node_store.write_node(&node).expect("Should write node");
    }

    {
        let mut edge_store = EdgeStore::new(&mut graph_file);
        let edge = EdgeRecord::new(1, 1, 2, "SAFE_EDGE".to_string(), json!({"protected": true}));
        edge_store.write_edge(&edge).expect("Should write edge");
    }

    // Verify node data is still intact (this would have failed before corruption fix)
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let read_node = node_store.read_node(1).expect("Should read node without corruption");
        assert_eq!(read_node.name, "CorruptionTest", "Node name should be intact");
        assert_eq!(read_node.data["vulnerable"], true, "Node data should be intact");
    }

    // TODO: After mmap implementation, verify no internal buffers
    // assert!(!graph_file.has_read_buffer(), "Should not have read buffer after mmap");
    // assert!(!graph_file.has_write_buffer(), "Should not have write buffer after mmap");
}

#[test]
fn test_v2_mmap_concurrent_access_safety() {
    // Test that mmap allows safe concurrent access patterns
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path()).expect("Failed to create graph file");

    // Write initial data
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let node = NodeRecord::new(1, "Concurrent".to_string(), "test".to_string(), json!({"access": "safe"}));
        node_store.write_node(&node).expect("Should write node");
    }

    // TODO: After mmap implementation, test concurrent read/write
    // let mmap_read = graph_file.get_mmap_slice();
    // let mmap_write = graph_file.get_mmap_slice_mut();
    //
    // Verify both slices point to same memory region
    // assert_eq!(mmap_read.as_ptr(), mmap_write.as_ptr(), "Read and write slices should point to same memory");

    // For now, verify current API works correctly
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let read_node = node_store.read_node(1).expect("Should read node");
        assert_eq!(read_node.name, "Concurrent", "Should read correct node");
    }
}

#[test]
fn test_v2_mmap_large_file_handling() {
    // Test mmap behavior with larger files
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path()).expect("Failed to create graph file");

    // Create many nodes to test file size handling
    let node_count = 100;
    for i in 1..=node_count {
        {
            let mut node_store = NodeStore::new(&mut graph_file);
            let node = NodeRecord::new(
                i,
                format!("NodeType{}", i),
                format!("node_{}", i),
                json!({"index": i, "large_data": "x".repeat(i * 10)})
            );
            node_store.write_node(&node).expect("Should write node");
        }
    }

    // Verify all nodes can be read back
    let header = graph_file.header();
    assert_eq!(header.node_count, node_count, f!("Should have written {} nodes", node_count));

    for i in 1..=node_count {
        let mut node_store = NodeStore::new(&mut graph_file);
        let read_node = node_store.read_node(i).expect(f!("Should read node {}", i));
        assert_eq!(read_node.name, format!("node_{}", i), f!("Node {} name mismatch", i));
        assert_eq!(read_node.data["index"], i, f!("Node {} data mismatch", i));
    }

    // TODO: After mmap implementation, verify memory mapping works for large files
    // let mmap_slice = graph_file.get_mmap_slice();
    // assert!(mmap_slice.len() > (node_count * 4096) as usize, "Mmap should cover entire file");
}

#[test]
fn test_v2_mmap_offset_calculation_accuracy() {
    // Test that offset calculations are accurate with mmap
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path()).expect("Failed to create graph file");

    // Write nodes at specific IDs to test offset calculations
    let test_node_ids = vec![1, 10, 100, 1000];

    for &node_id in &test_node_ids {
        {
            let mut node_store = NodeStore::new(&mut graph_file);
            let node = NodeRecord::new(node_id, "OffsetTest".to_string(), format!("offset_{}", node_id), json!({"id": node_id}));
            node_store.write_node(&node).expect("Should write node");
        }
    }

    // Verify offset calculations
    let header = graph_file.header();
    let node_slot_size = 4096;

    for &node_id in &test_node_ids {
        let expected_offset = header.node_data_offset + ((node_id - 1) as u64 * node_slot_size);

        // TODO: After mmap implementation, verify through direct memory access
        // let mmap_slice = graph_file.get_mmap_slice();
        // let node_start = expected_offset as usize;
        // let node_end = node_start + 4096;
        // assert!(node_end <= mmap_slice.len(), "Node {} should fit within mmap", node_id);
        // assert_eq!(mmap_slice[node_start], 2, "Node {} should have V2 version", node_id);

        // For now, verify through current API
        {
            let mut node_store = NodeStore::new(&mut graph_file);
            let read_node = node_store.read_node(node_id).expect("Should read node");
            assert_eq!(read_node.id, node_id, "Should read correct node ID");
            assert_eq!(read_node.name, format!("offset_{}", node_id), "Should read correct node name");
        }
    }
}

#[test]
fn test_v2_mmap_edge_node_region_boundaries() {
    // Test that mmap correctly handles edge/node region boundaries
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path()).expect("Failed to create graph file");

    // Create several nodes to establish node region
    let node_count = 5;
    for i in 1..=node_count {
        {
            let mut node_store = NodeStore::new(&mut graph_file);
            let node = NodeRecord::new(i, "BoundaryTest".to_string(), format!("boundary_{}", i), json!({"node": i}));
            node_store.write_node(&node).expect("Should write node");
        }
    }

    // Create edges to establish edge region
    let edge_count = 3;
    for i in 1..=edge_count {
        {
            let mut edge_store = EdgeStore::new(&mut graph_file);
            let edge = EdgeRecord::new(i, 1, (i % node_count) + 1, "BOUNDARY_EDGE".to_string(), json!({"edge": i}));
            edge_store.write_edge(&edge).expect("Should write edge");
        }
    }

    let header = graph_file.header();

    // TODO: After mmap implementation, verify region boundaries through mmap
    // let mmap_slice = graph_file.get_mmap_slice();
    // let node_region_end = header.node_data_offset + (node_count as u64 * 4096);
    // let edge_region_start = header.edge_data_offset;
    //
    // assert!(node_region_end <= edge_region_start, "Node region should not overlap edge region");
    // assert!(node_region_end as usize <= mmap_slice.len(), "Node region should fit in mmap");
    // assert!(edge_region_start as usize < mmap_slice.len(), "Edge region should fit in mmap");

    // For now, verify boundaries through current API
    assert!(header.node_count == node_count, "Should have correct node count");
    assert!(header.edge_count == edge_count, "Should have correct edge count");
    assert!(header.edge_data_offset >= header.node_data_offset + (node_count as u64 * 4096), "Edge region should start after node region");
}