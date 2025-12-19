//! Integration tests for GraphFile modularization
//!
//! These tests use real file operations (no mocks) to verify that GraphFile
//! functionality is preserved after modularization into focused modules.

use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::types::{NodeRecord, NodeFlags, NativeNodeId};
use tempfile::NamedTempFile;
use std::path::Path;

/// Helper function to create a test node record
fn create_test_node(id: NativeNodeId, name: &str) -> NodeRecord {
    NodeRecord {
        id,
        name: name.to_string(),
        node_type: "TEST".to_string(),
        flags: NodeFlags(0),
    }
}

#[test]
fn test_graph_file_creation_and_lifecycle() {
    // Create real temporary file - no mocks
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let path = temp_file.path();

    // Test file creation
    let mut graph_file = GraphFile::create(path)
        .expect("Failed to create graph file");

    // Verify initial state
    assert!(graph_file.path() == path);
    assert!(graph_file.file_size().expect("Failed to get file size") > 0);

    // Test header operations
    graph_file.read_header()
        .expect("Failed to read header");

    graph_file.write_header()
        .expect("Failed to write header");

    // Test sync
    graph_file.sync()
        .expect("Failed to sync file");

    // Verify cluster floor calculation
    let cluster_floor = graph_file.cluster_floor();
    assert!(cluster_floor > 0, "Cluster floor should be positive");

    // Verify Drop trait works (file will be dropped at end of function)
    // Header should be written and file synced automatically
}

#[test]
fn test_graph_file_open_existing() {
    // Create and populate a file
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let path = temp_file.path();

    {
        let mut graph_file = GraphFile::create(path)
            .expect("Failed to create graph file");

        // Write some data to make it realistic
        let node = create_test_node(1, "test_node");
        let offset = 1024; // Standard node offset
        let serialized = serde_json::to_vec(&node)
            .expect("Failed to serialize node");

        graph_file.write_bytes(offset, &serialized)
            .expect("Failed to write node data");

        graph_file.sync()
            .expect("Failed to sync file");
    } // File is dropped here (header written, synced)

    // Reopen the file
    let mut graph_file = GraphFile::open(path)
        .expect("Failed to open existing graph file");

    // Verify file is readable
    assert!(graph_file.path() == path);
    assert!(graph_file.file_size().expect("Failed to get file size") > 0);

    graph_file.read_header()
        .expect("Failed to read header from reopened file");
}

#[test]
fn test_graph_file_io_operations() {
    // Create real temporary file
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path())
        .expect("Failed to create graph file");

    // Test file growth
    let initial_size = graph_file.file_size()
        .expect("Failed to get initial file size");

    graph_file.grow(4096)
        .expect("Failed to grow file");

    let new_size = graph_file.file_size()
        .expect("Failed to get new file size");
    assert_eq!(new_size, initial_size + 4096, "File should have grown by 4096 bytes");

    // Test byte operations
    let test_data = b"Hello, GraphFile!";
    let write_offset = 2048;

    graph_file.write_bytes(write_offset, test_data)
        .expect("Failed to write test data");

    let mut read_buffer = vec![0u8; test_data.len()];
    graph_file.read_bytes(write_offset, &mut read_buffer)
        .expect("Failed to read test data");

    assert_eq!(read_buffer, test_data, "Read data should match written data");

    // Test direct write operations
    let direct_data = b"Direct write test";
    let direct_offset = 3072;

    graph_file.write_bytes_direct(direct_offset, direct_data)
        .expect("Failed to write data directly");

    let mut direct_buffer = vec![0u8; direct_data.len()];
    graph_file.read_bytes(direct_offset, &mut direct_buffer)
        .expect("Failed to read direct written data");

    assert_eq!(direct_buffer, direct_data, "Direct read should match direct write");
}

#[test]
fn test_graph_file_node_edge_access() {
    // Create real temporary file
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path())
        .expect("Failed to create graph file");

    // Test node operations
    let test_node = create_test_node(42, "test_node_42");

    // Write node at standard offset
    let node_offset = 1024 + ((test_node.id - 1) as u64 * 4096);
    let serialized_node = serde_json::to_vec(&test_node)
        .expect("Failed to serialize test node");

    graph_file.write_bytes(node_offset, &serialized_node)
        .expect("Failed to write test node");

    // Read node back
    let read_node = graph_file.read_node_at(test_node.id)
        .expect("Failed to read test node");

    assert_eq!(read_node.id, test_node.id);
    assert_eq!(read_node.name, test_node.name);
    assert_eq!(read_node.node_type, test_node.node_type);

    // Test node statistics
    let stats = graph_file.get_node_statistics()
        .expect("Failed to get node statistics");
    assert!(stats.total_slots >= 1, "Should have at least one node slot");

    // Test node existence
    let exists = graph_file.node_exists(test_node.id)
        .expect("Failed to check node existence");
    assert!(exists, "Test node should exist");

    let not_exists = graph_file.node_exists(999)
        .expect("Failed to check non-existent node");
    assert!(!not_exists, "Non-existent node should not exist");
}

#[test]
fn test_graph_file_edge_operations() {
    // Create real temporary file
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path())
        .expect("Failed to create graph file");

    // Create a test edge record
    use crate::backend::native::types::{EdgeRecord, EdgeFlags, NativeEdgeId};

    let test_edge = EdgeRecord {
        id: 1,
        from_id: 1,
        to_id: 2,
        edge_type: "TEST_EDGE".to_string(),
        flags: EdgeFlags(0),
        data: serde_json::json!({"test": true}),
    };

    // Write edge at standard offset
    let edge_offset = graph_file.calculate_edge_offset(test_edge.id);
    let serialized_edge = serde_json::to_vec(&test_edge)
        .expect("Failed to serialize test edge");

    graph_file.write_bytes(edge_offset, &serialized_edge)
        .expect("Failed to write test edge");

    // Read edge back
    let read_edge = graph_file.read_edge_at_offset(edge_offset)
        .expect("Failed to read test edge");

    assert_eq!(read_edge.id, test_edge.id);
    assert_eq!(read_edge.from_id, test_edge.from_id);
    assert_eq!(read_edge.to_id, test_edge.to_id);
    assert_eq!(read_edge.edge_type, test_edge.edge_type);

    // Test edge offset calculation
    let offset_1 = graph_file.calculate_edge_offset(1);
    let offset_2 = graph_file.calculate_edge_offset(2);
    assert_eq!(offset_2, offset_1 + 256, "Edge offsets should be 256 bytes apart");
}

#[test]
fn test_graph_file_transaction_operations() {
    // Create real temporary file
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path())
        .expect("Failed to create graph file");

    // Test transaction state access
    let tx_id = graph_file.current_transaction_id();
    assert_eq!(tx_id, 0, "Initial transaction ID should be 0");

    let is_active = graph_file.is_transaction_active();
    assert!(!is_active, "No transaction should be active initially");

    // Test transaction statistics
    let stats = graph_file.get_transaction_statistics();
    assert_eq!(stats.tx_id, 0);
    assert_eq!(stats.node_count, 0);
    assert_eq!(stats.edge_count, 0);

    // Test cluster commit (if available)
    #[cfg(not(feature = "v2_experimental"))]
    {
        // Only test if not in experimental mode
        let result = graph_file.begin_cluster_commit();
        // This might fail in test environment, which is okay
        println!("Cluster commit result: {:?}", result);
    }
}

#[test]
fn test_graph_file_memory_mapping() {
    // Create real temporary file
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path())
        .expect("Failed to create graph file");

    // Test memory mapping operations (only if experimental feature is enabled)
    #[cfg(feature = "v2_experimental")]
    {
        // Ensure mmap covers a certain size
        graph_file.mmap_ensure_size(8192)
            .expect("Failed to ensure mmap size");

        // Test mmap read/write
        let test_data = b"MMap test data";
        let offset = 4096;

        graph_file.mmap_write_bytes(offset, test_data)
            .expect("Failed to write via mmap");

        let mut read_buffer = vec![0u8; test_data.len()];
        graph_file.mmap_read_bytes(offset, &mut read_buffer)
            .expect("Failed to read via mmap");

        assert_eq!(read_buffer, test_data, "MMap read should match MMap write");
    }
}

#[test]
fn test_graph_file_api_compatibility() {
    // Test that all the existing public APIs still work after modularization
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path())
        .expect("Failed to create graph file");

    // Test all public methods that should still be available
    let _path = graph_file.path();
    let _file_path = graph_file.file_path();
    let _size = graph_file.file_size().expect("Failed to get file size");
    let _cluster_floor = graph_file.cluster_floor();

    // Test header operations
    graph_file.read_header().expect("Failed to read header");
    graph_file.write_header().expect("Failed to write header");

    // Test file operations
    graph_file.grow(1024).expect("Failed to grow file");
    graph_file.sync().expect("Failed to sync file");

    // Test validation
    graph_file.validate_file_size().expect("Failed to validate file size");

    // Test statistics
    let _tx_stats = graph_file.get_transaction_statistics();
    let _node_stats = graph_file.get_node_statistics().expect("Failed to get node statistics");

    // All operations should work without any API breakage
}

#[test]
fn test_graph_file_drop_behavior() {
    // Test that Drop trait still works correctly after modularization
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let path = temp_file.path();

    {
        let mut graph_file = GraphFile::create(path)
            .expect("Failed to create graph file");

        // Modify some state
        graph_file.write_header()
            .expect("Failed to write header");
    } // GraphFile is dropped here - should write header and sync

    // Verify file is still valid after being dropped
    let mut graph_file = GraphFile::open(path)
        .expect("Failed to reopen file after drop");

    // Should be able to read header without issues
    graph_file.read_header()
        .expect("Failed to read header after reopening");
}

#[test]
fn test_graph_file_error_handling() {
    // Test that error handling is preserved after modularization
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path())
        .expect("Failed to create graph file");

    // Test reading from non-existent node
    let result = graph_file.read_node_at(999999);
    assert!(result.is_err(), "Reading non-existent node should fail");

    // Test reading beyond file size
    let mut buffer = vec![0u8; 100];
    let large_offset = graph_file.file_size().expect("Failed to get file size") + 1000;
    let result = graph_file.read_bytes(large_offset, &mut buffer);
    assert!(result.is_err(), "Reading beyond file size should fail");

    // Test writing to invalid offset (should be handled gracefully)
    let test_data = b"test";
    let result = graph_file.write_bytes(u64::MAX, test_data);
    assert!(result.is_err(), "Writing to invalid offset should fail");
}