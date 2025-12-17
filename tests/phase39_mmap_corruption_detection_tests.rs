//! Phase 39: Mmap Corruption Detection Tests
//!
//! These tests validate the specific corruption patterns identified in Phase 39
//! and will be used to verify the conservative mmap fix in Phase 40.

#[cfg(feature = "v2_experimental")]
use sqlitegraph::{
    EdgeSpec, NodeSpec, config::GraphConfig, open_graph,
    backend::native::{GraphFile, EdgeStore, NodeStore},
    GraphBackend, BackendDirection, NeighborQuery,
};
use tempfile::TempDir;

/// Test 1: Detect magic number corruption during GraphFile reopen
/// This reproduces the exact corruption pattern from Phase 39
#[cfg(feature = "v2_experimental")]
#[test]
fn test_detect_magic_number_corruption_on_reopen() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");

    // Create graph and write cluster data (this works)
    {
        let config = GraphConfig::native();
        let mut graph = open_graph(&db_path, &config).expect("Failed to create graph");

        let source_id = graph.insert_node(NodeSpec {
            kind: "Node".to_string(),
            name: "source".to_string(),
            file_path: None,
            data: serde_json::json!({"type": "source"}),
        }).expect("Failed to insert source node");

        let target_id = graph.insert_node(NodeSpec {
            kind: "Node".to_string(),
            name: "target".to_string(),
            file_path: None,
            data: serde_json::json!({"type": "target"}),
        }).expect("Failed to insert target node");

        // This creates cluster data and triggers mmap operations
        let _edge_id = graph.insert_edge(EdgeSpec {
            from: source_id,
            to: target_id,
            edge_type: "test_edge".to_string(),
            data: serde_json::json!({"weight": 1.0}),
        }).expect("Failed to insert edge");

        // Explicit close to flush all data
        drop(graph);
    }

    // Reopen graph - this is where magic number corruption occurs
    let config = GraphConfig::native();
    let graph_result = open_graph(&db_path, &config);

    // This should NOT fail with magic number corruption after conservative mmap fix
    match graph_result {
        Ok(_) => {
            // Test passes - no magic number corruption
        }
        Err(e) => {
            // Test fails - magic number corruption detected
            panic!("GraphFile reopen failed with magic number corruption: {:?}", e);
        }
    }
}

/// Test 2: Validate cluster header integrity after mixed I/O operations
/// This reproduces the cluster header corruption pattern
#[cfg(feature = "v2_experimental")]
#[test]
fn test_cluster_header_integrity_mixed_io() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");

    let config = GraphConfig::native();
    let mut graph = open_graph(&db_path, &config).expect("Failed to create graph");

    // Create nodes and edges that trigger cluster creation
    let node1_id = graph.insert_node(NodeSpec {
        kind: "Node".to_string(),
        name: "node1".to_string(),
        file_path: None,
        data: serde_json::json!({"type": "node1"}),
    }).expect("Failed to insert node1");

    let node2_id = graph.insert_node(NodeSpec {
        kind: "Node".to_string(),
        name: "node2".to_string(),
        file_path: None,
        data: serde_json::json!({"type": "node2"}),
    }).expect("Failed to insert node2");

    // Insert edge (this triggers cluster creation with mixed I/O paths)
    let _edge_id = graph.insert_edge(EdgeSpec {
        from: node1_id,
        to: node2_id,
        edge_type: "test_edge".to_string(),
        data: serde_json::json!({"weight": 1.0}),
    }).expect("Failed to insert edge");

    // Verify neighbors using V2 clustered adjacency
    let neighbors = graph.neighbors(node1_id, BackendDirection::Outgoing)
        .expect("Failed to get neighbors");

    // Should contain exactly one neighbor
    assert_eq!(neighbors.len(), 1, "Should have exactly one neighbor");
    assert_eq!(neighbors[0], node2_id, "Neighbor should be node2");
}

/// Test 3: Detect node ID corruption in large cluster operations
/// This reproduces the node ID corruption pattern
#[cfg(feature = "v2_experimental")]
#[test]
fn test_detect_node_id_corruption_large_operations() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");

    let config = GraphConfig::native();
    let mut graph = open_graph(&db_path, &config).expect("Failed to create graph");

    // Create central node
    let central_id = graph.insert_node(NodeSpec {
        kind: "Central".to_string(),
        name: "central".to_string(),
        file_path: None,
        data: serde_json::json!({"type": "central"}),
    }).expect("Failed to insert central node");

    // Create multiple connected nodes (triggers large cluster operations)
    let mut node_ids = Vec::new();
    for i in 1..=5 {
        let node_id = graph.insert_node(NodeSpec {
            kind: "Node".to_string(),
            name: format!("node{}", i),
            file_path: None,
            data: serde_json::json!({"index": i}),
        }).expect("Failed to insert node");

        node_ids.push(node_id);

        // Create edge from central to this node
        let _edge_id = graph.insert_edge(EdgeSpec {
            from: central_id,
            to: node_id,
            edge_type: "connects_to".to_string(),
            data: serde_json::json!({"weight": 1.0}),
        }).expect("Failed to insert edge");
    }

    // Verify neighbors - should contain exactly 5 valid node IDs
    let neighbors = graph.neighbors(central_id, BackendDirection::Outgoing)
        .expect("Failed to get neighbors");

    assert_eq!(neighbors.len(), 5, "Should have exactly 5 neighbors");

    // Verify all neighbor IDs are reasonable (not corrupted to huge values)
    for &neighbor_id in &neighbors {
        assert!(neighbor_id > 0, "Neighbor ID should be positive: {}", neighbor_id);
        assert!(neighbor_id < 1000000, "Neighbor ID should be reasonable: {}", neighbor_id);
        assert!(node_ids.contains(&neighbor_id), "Neighbor ID {} should be in expected set", neighbor_id);
    }
}

/// Test 4: Validate GraphFile basic I/O still works after conservative mmap changes
/// This ensures our fix doesn't break the working Phase 38 functionality
#[cfg(feature = "v2_experimental")]
#[test]
fn test_basic_graphfile_io_still_works() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");

    // Create graph file directly and test basic I/O
    let mut graph_file = GraphFile::create(&db_path).expect("Failed to create graph file");

    // Write test data at specific offset
    let test_data = b"CLUSTER_HEADER\x01\x00\x00\x00\x0C";
    let write_offset = 2048;

    graph_file.write_bytes(write_offset, test_data).expect("Failed to write bytes");
    graph_file.flush().expect("Failed to flush");

    // Read back the exact bytes
    let mut read_buffer = vec![0u8; test_data.len()];
    graph_file.read_bytes(write_offset, &mut read_buffer).expect("Failed to read bytes");

    // Verify exact match
    assert_eq!(read_buffer, test_data, "Read bytes must match written bytes exactly");

    // Test mmap operations also work
    let mut mmap_buffer = vec![0u8; test_data.len()];
    graph_file.mmap_read_bytes(write_offset, &mut mmap_buffer).expect("Failed to mmap read");

    assert_eq!(mmap_buffer, test_data, "Mmap read bytes must match written data");
}

/// Test 5: Verify mmap doesn't remap too frequently after conservative fix
#[cfg(feature = "v2_experimental")]
#[test]
fn test_conservative_mmap_remapping() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.db");

    let mut graph_file = GraphFile::create(&db_path).expect("Failed to create graph file");

    // Write multiple small chunks that should NOT trigger remapping
    let initial_mmap_size = graph_file.mmap.as_ref().unwrap().len();

    for i in 0..10 {
        let small_data = format!("CHUNK{}", i).as_bytes().to_vec();
        let offset = 2048 + (i * 16);

        graph_file.write_bytes(offset as u64, &small_data).expect("Failed to write small chunk");
        graph_file.flush().expect("Failed to flush");
    }

    // Mmap size should NOT have changed (conservative remapping)
    let final_mmap_size = graph_file.mmap.as_ref().unwrap().len();
    assert_eq!(initial_mmap_size, final_mmap_size, "Mmap should not remap for small writes");

    // Write a large chunk that SHOULD trigger remapping (>4KB)
    let large_data = vec![0u8; 5000]; // 5KB
    let large_offset = 8192;

    graph_file.write_bytes(large_offset, &large_data).expect("Failed to write large chunk");
    graph_file.flush().expect("Failed to flush");

    // Mmap size should have increased now
    let new_mmap_size = graph_file.mmap.as_ref().unwrap().len();
    assert!(new_mmap_size > final_mmap_size, "Mmap should remap for large writes >4KB");
}