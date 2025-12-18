#![cfg(feature = "v2_experimental")]

//! Test isolation fixes for native backend
//! These tests enforce strict per-test isolation to prevent cross-test contamination

use serde_json::json;
use sqlitegraph::backend::native::{
    EdgeRecord, NativeNodeId, NodeRecord, edge_store::EdgeStore, graph_file::GraphFile,
    graph_ops::native_bfs, node_store::NodeStore,
};
use tempfile::{NamedTempFile, TempDir};

/// Test 1: Each test must start with a clean GraphFile header
#[test]
fn test_clean_graph_file_header_per_test() {
    // Create isolated temp directory per test
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("isolation_test_1.db");

    let mut graph_file = GraphFile::create(&db_path).expect("Failed to create graph file");

    // Verify clean header
    let header = graph_file.header();
    assert_eq!(header.node_count, 0, "New graph file should have 0 nodes");
    assert_eq!(header.edge_count, 0, "New graph file should have 0 edges");
    assert_eq!(header.version, 2, "New graph file should use V2 format");

    // Write one node and verify it doesn't affect other tests
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let node = NodeRecord::new(1, "Test".to_string(), "node1".to_string(), json!({}));
        node_store.write_node(&node).expect("Failed to write node");
    }

    // Verify node count updated only for this test instance
    assert_eq!(
        graph_file.header().node_count,
        1,
        "Node count should be 1 in this test"
    );
}

/// Test 2: Each test must enforce unique tmpfile path
#[test]
fn test_unique_tempfile_per_test() {
    // Create isolated temp directory with unique name
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("unique_test_2.db");

    // Verify the path doesn't conflict with other tests
    assert!(
        !db_path.exists(),
        "Database file should not exist before creation"
    );

    let mut graph_file = GraphFile::create(&db_path).expect("Failed to create graph file");

    // Write nodes with different IDs to prove isolation
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let node = NodeRecord::new(
            2,
            "Unique".to_string(),
            "unique_node".to_string(),
            json!({}),
        );
        node_store.write_node(&node).expect("Failed to write node");
    }

    // Verify we get back exactly what we wrote
    let mut node_store = NodeStore::new(&mut graph_file);
    let read_node = node_store.read_node(2).expect("Failed to read node");
    assert_eq!(read_node.name, "unique_node");
    assert_eq!(read_node.kind, "Unique");
}

/// Test 3: GraphFile must close before next test opens same file
#[test]
fn test_graph_file_proper_closure() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("closure_test_3.db");

    {
        // Create and use GraphFile in limited scope
        let mut graph_file = GraphFile::create(&db_path).expect("Failed to create graph file");
        {
            let mut node_store = NodeStore::new(&mut graph_file);
            let node = NodeRecord::new(
                3,
                "Closure".to_string(),
                "closure_node".to_string(),
                json!({}),
            );
            node_store.write_node(&node).expect("Failed to write node");
        }
        // Force buffer flush and sync
        graph_file.flush().expect("Failed to flush graph file");
        graph_file.sync().expect("Failed to sync graph file");
    } // GraphFile dropped here

    // Reopen and verify data persistence
    let mut graph_file = GraphFile::create(&db_path).expect("Failed to reopen graph file");
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let read_node = node_store
            .read_node(3)
            .expect("Failed to read node after reopen");
        assert_eq!(read_node.name, "closure_node");
    }
}

/// Test 4: read_buffer must not reuse bytes from previous test
#[test]
fn test_read_buffer_isolation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("buffer_test_4.db");

    let mut graph_file = GraphFile::create(&db_path).expect("Failed to create graph file");

    // Write specific test data
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let node = NodeRecord::new(
            4,
            "Buffer".to_string(),
            "buffer_node".to_string(),
            json!({"test": "buffer_isolation"}),
        );
        node_store.write_node(&node).expect("Failed to write node");
    }

    // Invalidate read buffer explicitly
    graph_file.invalidate_read_buffer();

    // Read back and verify no contamination
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let read_node = node_store.read_node(4).expect("Failed to read node");
        assert_eq!(read_node.data, json!({"test": "buffer_isolation"}));
        assert_eq!(read_node.name, "buffer_node");
    }
}

/// Test 5: Tests that write version=1 must be skipped under native backend
#[test]
fn test_v2_format_is_default_under_native_backend() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("v1_test_5.db");

    let mut graph_file = GraphFile::create(&db_path).expect("Failed to create graph file");

    // Verify V2 is the default format
    let node_slot_offset = graph_file.header().node_data_offset;
    let mut version_buf = [0u8; 1];
    graph_file
        .read_bytes(node_slot_offset, &mut version_buf)
        .expect("Failed to read version");

    assert_eq!(
        version_buf[0], 2,
        "Native backend should default to V2 format"
    );
}

/// Test 6: Edge operations should work correctly with V2 nodes
#[test]
fn test_edge_operations_with_v2_isolation() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("edge_test_6.db");

    let mut graph_file = GraphFile::create(&db_path).expect("Failed to create graph file");

    // Write V2 nodes first
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let source = NodeRecord::new(1, "EdgeTest".to_string(), "source".to_string(), json!({}));
        let target = NodeRecord::new(2, "EdgeTest".to_string(), "target".to_string(), json!({}));
        node_store
            .write_node(&source)
            .expect("Failed to write source node");
        node_store
            .write_node(&target)
            .expect("Failed to write target node");

        // Force flush before edge operations
        graph_file
            .flush_write_buffer()
            .expect("Failed to flush write buffer");
        graph_file.invalidate_read_buffer();
    }

    // Write edges
    {
        let mut edge_store = EdgeStore::new(&mut graph_file);
        let edge = EdgeRecord::new(1, 1, 2, "LINK".to_string(), json!({"weight": 1}));
        edge_store.write_edge(&edge).expect("Failed to write edge");
    }

    // Verify operations completed without corruption
    let result = native_bfs(&mut graph_file, 1, 1);
    assert!(
        result.is_ok(),
        "BFS should work correctly with isolated V2 nodes and edges"
    );
}
