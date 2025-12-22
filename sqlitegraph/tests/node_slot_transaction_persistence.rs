//! Node Slot Transaction Persistence Tests
//!
//! Tests that verify node slots remain version=2 across transaction boundaries.
//! Transactions are triggered internally by edge insertion operations.
//!
//! Critical invariant: Once a node slot is successfully written with version=2,
//! it must never be truncated, zeroed, or overwritten with version=0.

use sqlitegraph::{EdgeSpec, GraphConfig, NodeSpec, open_graph};
use std::fs;
use std::io::Read;

/// Helper to directly read node slot version from disk
fn read_node_slot_version(
    path: &std::path::Path,
    node_id: i64,
) -> Result<u8, Box<dyn std::error::Error>> {
    let mut file = fs::File::open(path)?;

    const NODE_SLOT_SIZE: u64 = 4096;
    const DEFAULT_NODE_DATA_START: u64 = 1024;

    let slot_offset = DEFAULT_NODE_DATA_START + ((node_id - 1) as u64 * NODE_SLOT_SIZE);

    use std::io::Seek;
    use std::io::SeekFrom;

    file.seek(SeekFrom::Start(slot_offset))?;
    let mut buffer = [0u8; 1];
    file.read_exact(&mut buffer)?;

    Ok(buffer[0])
}

#[test]
fn test_node_slots_persist_across_edge_transactions() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("node_edge_test.db");

    println!("=== Testing Node Slot Persistence Across Edge Transactions ===");

    // Phase 1: Create nodes and verify they have version=2
    let mut node_ids = Vec::new();
    {
        let graph = open_graph(&db_path, &GraphConfig::native()).expect("Failed to create graph");

        // Create 300 nodes to cross the 256 boundary
        for i in 1..=300 {
            let node_id = graph
                .insert_node(NodeSpec {
                    kind: "TestNode".to_string(),
                    name: format!("node_{}", i),
                    file_path: None,
                    data: serde_json::json!({"phase": 1, "id": i}),
                })
                .expect(&format!("Failed to insert node {}", i));
            node_ids.push(node_id);

            // Every 50 nodes, verify version=2
            if i % 50 == 0 {
                let version = read_node_slot_version(&db_path, node_id)
                    .expect(&format!("Failed to read version for node {}", node_id));
                assert_eq!(
                    version, 2,
                    "Node {} should have version=2, got {}",
                    node_id, version
                );
                println!("Node {} has version={}", node_id, version);
            }
        }

        // Create many edges to trigger internal transaction boundaries
        println!("Creating edges to trigger internal transactions...");
        for i in 0..1000 {
            let from_idx = (i % node_ids.len()) as usize;
            let to_idx = ((i + 1) % node_ids.len()) as usize;

            // This will internally call begin_transaction()
            graph
                .insert_edge(EdgeSpec {
                    from: node_ids[from_idx],
                    to: node_ids[to_idx],
                    edge_type: "test_edge".to_string(),
                    data: serde_json::json!({"edge_index": i}),
                })
                .expect(&format!("Failed to insert edge {}", i));
        }

        println!("✅ Created 300 nodes and 1000 edges, verifying slot versions");
        drop(graph);
    }

    // Phase 2: Reopen and verify all node slots still have version=2
    {
        let graph = open_graph(&db_path, &GraphConfig::native()).expect("Failed to reopen graph");

        // Verify critical boundary nodes (256, 257, 258)
        for &node_id in &[256i64, 257i64, 258i64] {
            let version = read_node_slot_version(&db_path, node_id).expect(&format!(
                "Failed to read version for critical node {}",
                node_id
            ));
            assert_eq!(
                version, 2,
                "Critical node {} should have version=2 after reopen, got {}",
                node_id, version
            );
            println!(
                "Critical node {} has version={} after reopen",
                node_id, version
            );
        }

        // Verify sample nodes across the range
        for &node_id in [1, 50, 100, 150, 200, 250, 300].iter() {
            if node_id <= 300 {
                let version = read_node_slot_version(&db_path, node_id).expect(&format!(
                    "Failed to read version for sample node {}",
                    node_id
                ));
                assert_eq!(
                    version, 2,
                    "Sample node {} should have version=2 after reopen, got {}",
                    node_id, version
                );

                // Also verify the node data is correct
                let node = graph
                    .get_node(node_id)
                    .expect(&format!("Node {} missing after reopen", node_id));
                assert_eq!(node.id, node_id, "Node ID mismatch after reopen");
                assert_eq!(
                    node.name,
                    format!("node_{}", node_id),
                    "Node name corrupted after reopen"
                );
            }
        }

        println!(
            "✅ All critical and sample nodes verified with version=2 after transaction/reopen"
        );
    }
}

#[test]
fn test_file_size_never_shrinks_during_edge_operations() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("file_size_test.db");

    println!("=== Testing File Size Never Shrinks During Edge Operations ===");

    let mut max_file_size = 0u64;
    let node_ids: Vec<i64>;

    // Phase 1: Create nodes and track file growth
    {
        let graph = open_graph(&db_path, &GraphConfig::native()).expect("Failed to create graph");

        // Create 200 nodes
        let mut temp_node_ids = Vec::new();
        for i in 1..=200 {
            let node_id = graph
                .insert_node(NodeSpec {
                    kind: "FileSizeTestNode".to_string(),
                    name: format!("file_size_node_{}", i),
                    file_path: None,
                    data: serde_json::json!({"node_index": i}),
                })
                .expect("Failed to insert file size test node");
            temp_node_ids.push(node_id);

            // Track maximum file size reached
            let current_size = std::fs::metadata(&db_path).unwrap().len();
            max_file_size = max_file_size.max(current_size);
        }
        node_ids = temp_node_ids;

        // Verify some critical nodes have version=2
        for &node_id in &[1, 50, 100, 150, 200] {
            let version = read_node_slot_version(&db_path, node_id)
                .expect(&format!("Failed to read version for node {}", node_id));
            assert_eq!(
                version, 2,
                "Node {} should have version=2, got {}",
                node_id, version
            );
        }

        println!("Created 200 nodes, max file size: {} bytes", max_file_size);
        drop(graph);
    }

    // Phase 2: Create many edges (triggers internal transactions)
    {
        let graph = open_graph(&db_path, &GraphConfig::native()).expect("Failed to reopen graph");

        // Create thousands of edges to exercise transaction logic
        for i in 0..2000 {
            let from_idx = (i % node_ids.len()) as usize;
            let to_idx = ((i + 1) % node_ids.len()) as usize;

            graph
                .insert_edge(EdgeSpec {
                    from: node_ids[from_idx],
                    to: node_ids[to_idx],
                    edge_type: "file_size_test_edge".to_string(),
                    data: serde_json::json!({"edge_index": i}),
                })
                .expect(&format!("Failed to insert edge {}", i));

            // Check file size periodically
            if i % 500 == 0 {
                let current_size = std::fs::metadata(&db_path).unwrap().len();
                assert!(
                    current_size >= max_file_size,
                    "File size shrunk during edge operations: max was {}, now {}",
                    max_file_size,
                    current_size
                );
                max_file_size = max_file_size.max(current_size);
                println!("After {} edges, file size: {} bytes", i, current_size);
            }
        }

        println!(
            "✅ Created 2000 edges, file size never shrunk below max: {}",
            max_file_size
        );
        drop(graph);
    }

    // Phase 3: Reopen and verify all nodes still exist with version=2
    {
        let graph = open_graph(&db_path, &GraphConfig::native()).expect("Failed to reopen graph");

        // Verify critical nodes still have version=2
        for &node_id in &[1, 50, 100, 150, 200] {
            let version = read_node_slot_version(&db_path, node_id).expect(&format!(
                "Failed to read version for node {} after edge operations",
                node_id
            ));
            assert_eq!(
                version, 2,
                "Node {} should have version=2 after edge operations, got {}",
                node_id, version
            );
        }

        // Verify all nodes still exist
        for &node_id in &node_ids {
            let node = graph
                .get_node(node_id)
                .expect(&format!("Node {} missing after edge operations", node_id));
            assert_eq!(node.id, node_id, "Node ID corrupted after edge operations");
        }

        let final_size = std::fs::metadata(&db_path).unwrap().len();
        assert!(
            final_size >= max_file_size,
            "Final file size {} is smaller than max size {}",
            final_size,
            max_file_size
        );

        println!(
            "✅ All {} nodes verified after edge operations, final file size: {}",
            node_ids.len(),
            final_size
        );
    }
}
