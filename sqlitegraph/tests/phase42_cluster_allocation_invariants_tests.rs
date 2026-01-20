//! Phase 42: V2 Cluster Allocation Invariants Tests
//!
//! TDD tests that enforce critical cluster allocation invariants to eliminate
//! multi-cluster corruption by ensuring proper layout and ordering guarantees.

#[cfg(feature = "v2_experimental")]
use sqlitegraph::{
    EdgeSpec, NodeSpec,
    backend::native::v2::V2_MAGIC,
    backend::native::{GraphFile, NodeStore},
    config::GraphConfig,
    open_graph,
};
use tempfile::TempDir;

/// TEST 1: Multi-cluster offsets must be distinct and non-overlapping
/// This test proves that cluster allocation ensures no overlap between clusters
#[cfg(feature = "v2_experimental")]
#[test]
fn test_multi_cluster_offsets_must_be_distinct_and_non_overlapping() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.v2");

    // Create graph and force creation of multiple clusters
    let config = GraphConfig::native();
    let graph = open_graph(&db_path, &config).expect("Failed to create graph");

    // Create nodes for multi-cluster test
    let node1_id = graph
        .insert_node(NodeSpec {
            kind: "Node".to_string(),
            name: "node1".to_string(),
            file_path: None,
            data: serde_json::json!({"id": 1}),
        })
        .expect("Failed to insert node1");

    let node2_id = graph
        .insert_node(NodeSpec {
            kind: "Node".to_string(),
            name: "node2".to_string(),
            file_path: None,
            data: serde_json::json!({"id": 2}),
        })
        .expect("Failed to insert node2");

    let node3_id = graph
        .insert_node(NodeSpec {
            kind: "Node".to_string(),
            name: "node3".to_string(),
            file_path: None,
            data: serde_json::json!({"id": 3}),
        })
        .expect("Failed to insert node3");

    // Force multiple outgoing clusters by creating edges from different nodes
    let _edge1 = graph
        .insert_edge(EdgeSpec {
            from: node1_id,
            to: node2_id,
            edge_type: "outgoing_cluster_1".to_string(),
            data: serde_json::json!({"cluster": "node1_outgoing"}),
        })
        .expect("Failed to create edge 1");

    let _edge2 = graph
        .insert_edge(EdgeSpec {
            from: node2_id,
            to: node3_id,
            edge_type: "outgoing_cluster_2".to_string(),
            data: serde_json::json!({"cluster": "node2_outgoing"}),
        })
        .expect("Failed to create edge 2");

    let _edge3 = graph
        .insert_edge(EdgeSpec {
            from: node3_id,
            to: node1_id,
            edge_type: "outgoing_cluster_3".to_string(),
            data: serde_json::json!({"cluster": "node3_outgoing"}),
        })
        .expect("Failed to create edge 3");

    // Verify cluster metadata was set correctly
    // This tests that the cluster allocation prevents overlap during normal operation
    let mut cluster_regions = Vec::new();

    // Read each node's V2 metadata to get cluster information
    for node_id in [node1_id, node2_id, node3_id] {
        let mut graph_file = GraphFile::open(&db_path).expect("Failed to open graph file");
        let mut node_store = NodeStore::new(&mut graph_file);

        if let Ok(node_v2) = node_store.read_node_v2(node_id) {
            if node_v2.has_outgoing_edges() {
                let offset = node_v2.outgoing_cluster_offset;
                let size = node_v2.outgoing_cluster_size;
                let edge_count = node_v2.outgoing_edge_count;

                // Validate cluster header invariants
                assert!(offset > 0, "Cluster offset must be > 0, got {}", offset);
                assert!(size > 0, "Cluster size must be > 0, got {}", size);
                assert!(edge_count > 0, "Edge count must be > 0, got {}", edge_count);
                assert!(
                    edge_count < 1_000_000,
                    "Edge count unreasonably large: {}",
                    edge_count
                );

                cluster_regions.push((offset, size, edge_count, node_id));
            }
        }
    }

    // CRITICAL INVARIANT: All cluster regions must be distinct and non-overlapping
    assert!(
        cluster_regions.len() >= 2,
        "Expected at least 2 clusters, got {}",
        cluster_regions.len()
    );

    for i in 0..cluster_regions.len() {
        for j in (i + 1)..cluster_regions.len() {
            let (offset1, size1, _count1, id1) = cluster_regions[i];
            let (offset2, size2, _count2, id2) = cluster_regions[j];

            let region1_end = offset1 + size1 as u64;
            let region2_end = offset2 + size2 as u64;

            // Check for non-overlap
            let overlaps = (offset1 < region2_end) && (offset2 < region1_end);
            assert!(
                !overlaps,
                "Clusters overlap: node {} [{}, {}) and node {} [{}, {}) overlap!",
                id1, offset1, region1_end, id2, offset2, region2_end
            );

            // Check for distinct offsets
            assert_ne!(
                offset1, offset2,
                "Clusters have identical offsets: node {} and node {} both at offset {}",
                id1, id2, offset1
            );
        }
    }

    // Validate layout invariants: clusters must be after node region
    let graph_file = GraphFile::open(&db_path).expect("Failed to open graph file");
    let header = graph_file.header();
    let node_region_end = header.node_data_offset + (header.node_count as u64 * 4096);
    let file_size = graph_file.file_size().unwrap_or(u64::MAX);

    for (offset, size, _count, _node_id) in &cluster_regions {
        assert!(
            offset >= &node_region_end,
            "Cluster offset {} must be >= node_region_end {}",
            offset,
            node_region_end
        );
        assert!(
            offset + (*size as u64) <= file_size,
            "Cluster [{}, {}) exceeds file size",
            offset,
            offset + (*size as u64)
        );
    }
}

/// TEST 2: Cluster headers must survive file reopen
/// This test proves that cluster corruption doesn't happen during reopen cycles
#[cfg(feature = "v2_experimental")]
#[test]
fn test_cluster_headers_survive_reopen() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.v2");

    // Create graph with multi-cluster scenario
    {
        let config = GraphConfig::native();
        let graph = open_graph(&db_path, &config).expect("Failed to create graph");

        let node1_id = graph
            .insert_node(NodeSpec {
                kind: "Node".to_string(),
                name: "node1".to_string(),
                file_path: None,
                data: serde_json::json!({"id": 1}),
            })
            .expect("Failed to insert node1");

        let node2_id = graph
            .insert_node(NodeSpec {
                kind: "Node".to_string(),
                name: "node2".to_string(),
                file_path: None,
                data: serde_json::json!({"id": 2}),
            })
            .expect("Failed to insert node2");

        let node3_id = graph
            .insert_node(NodeSpec {
                kind: "Node".to_string(),
                name: "node3".to_string(),
                file_path: None,
                data: serde_json::json!({"id": 3}),
            })
            .expect("Failed to insert node3");

        // Create edges to generate clusters
        let _edge1 = graph
            .insert_edge(EdgeSpec {
                from: node1_id,
                to: node2_id,
                edge_type: "reopen_test_1".to_string(),
                data: serde_json::json!({"test": "reopen"}),
            })
            .expect("Failed to create edge 1");

        let _edge2 = graph
            .insert_edge(EdgeSpec {
                from: node2_id,
                to: node3_id,
                edge_type: "reopen_test_2".to_string(),
                data: serde_json::json!({"test": "reopen"}),
            })
            .expect("Failed to create edge 2");

        // Store cluster metadata before close
        let mut stored_clusters = Vec::new();
        for node_id in [node1_id, node2_id, node3_id] {
            let mut graph_file = GraphFile::open(&db_path).expect("Failed to open graph file");
            let mut node_store = NodeStore::new(&mut graph_file);

            if let Ok(node_v2) = node_store.read_node_v2(node_id) {
                if node_v2.has_outgoing_edges() {
                    let offset = node_v2.outgoing_cluster_offset;
                    let size = node_v2.outgoing_cluster_size;
                    let edge_count = node_v2.outgoing_edge_count;

                    // Read cluster header before close
                    let mut header_before = vec![0u8; 8];
                    graph_file
                        .read_bytes(offset, &mut header_before)
                        .expect("Failed to read cluster header before close");

                    stored_clusters.push((offset, size, edge_count, header_before, node_id));
                }
            }
        }

        // Verify clusters are valid before close
        for (offset, size, edge_count, header_before, node_id) in &stored_clusters {
            let edge_count_from_header = u32::from_be_bytes([
                header_before[0],
                header_before[1],
                header_before[2],
                header_before[3],
            ]);
            let payload_size = u32::from_be_bytes([
                header_before[4],
                header_before[5],
                header_before[6],
                header_before[7],
            ]);

            println!(
                "BEFORE CLOSE - Node {} cluster: offset={}, size={}, header_edge_count={}, header_payload={}",
                node_id, offset, size, edge_count_from_header, payload_size
            );

            assert_ne!(
                edge_count_from_header, 0,
                "Header edge_count should not be zero before close"
            );
            assert_ne!(
                edge_count_from_header, 33554432,
                "Header should not be byte-swapped before close"
            );
            assert_eq!(
                edge_count_from_header, *edge_count,
                "Header edge count must match metadata before close"
            );
        }

        // Explicit close
        drop(graph);
    }

    // Reopen and verify cluster headers survive
    {
        let mut graph_file = GraphFile::open(&db_path).expect("Failed to reopen graph file");

        // Verify magic number survived reopen
        let header = graph_file.header();
        assert_eq!(
            header.magic, V2_MAGIC,
            "Magic number corrupted during reopen"
        );

        // Re-read cluster metadata after reopen and verify headers
        for node_id in 1..=3 {
            let mut node_store = NodeStore::new(&mut graph_file);
            if let Ok(node_v2) = node_store.read_node_v2(node_id) {
                if node_v2.has_outgoing_edges() {
                    let offset = node_v2.outgoing_cluster_offset;
                    let size = node_v2.outgoing_cluster_size;
                    let edge_count = node_v2.outgoing_edge_count;

                    // Read cluster header after reopen
                    let mut header_after = vec![0u8; 8];
                    drop(node_store); // Release borrow
                    graph_file
                        .read_bytes(offset, &mut header_after)
                        .expect("Failed to read cluster header after reopen");

                    let edge_count_from_header = u32::from_be_bytes([
                        header_after[0],
                        header_after[1],
                        header_after[2],
                        header_after[3],
                    ]);
                    let payload_size = u32::from_be_bytes([
                        header_after[4],
                        header_after[5],
                        header_after[6],
                        header_after[7],
                    ]);

                    println!(
                        "AFTER REOPEN - Node {} cluster: offset={}, size={}, header_edge_count={}, header_payload={}",
                        node_id, offset, size, edge_count_from_header, payload_size
                    );

                    // CRITICAL INVARIANTS after reopen
                    assert_ne!(
                        edge_count_from_header, 0,
                        "Header edge_count should not be zero after reopen for node {}",
                        node_id
                    );
                    assert_ne!(
                        edge_count_from_header, 33554432,
                        "Header should not be byte-swapped after reopen for node {} (got {})",
                        node_id, edge_count_from_header
                    );
                    assert_eq!(
                        edge_count_from_header, edge_count,
                        "Header edge count must match metadata after reopen for node {}",
                        node_id
                    );
                    assert_ne!(
                        payload_size, 0,
                        "Payload size should not be zero after reopen for node {}",
                        node_id
                    );
                }
            }
        }
    }
}

/// TEST 3: Header and file length consistency after multiple cluster writes
/// This test proves that file size and header fields remain consistent
#[cfg(feature = "v2_experimental")]
#[test]
fn test_header_and_file_length_consistency_after_multiple_cluster_writes() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("test.v2");

    let config = GraphConfig::native();
    let graph = open_graph(&db_path, &config).expect("Failed to create graph");

    // Track file consistency after each cluster write
    let mut max_written_offset = 0u64;
    let initial_header = {
        let graph_file = GraphFile::open(&db_path).expect("Failed to open graph file");
        graph_file.header().clone()
    };

    // Create nodes
    let node1_id = graph
        .insert_node(NodeSpec {
            kind: "Node".to_string(),
            name: "node1".to_string(),
            file_path: None,
            data: serde_json::json!({"id": 1}),
        })
        .expect("Failed to insert node1");

    let node2_id = graph
        .insert_node(NodeSpec {
            kind: "Node".to_string(),
            name: "node2".to_string(),
            file_path: None,
            data: serde_json::json!({"id": 2}),
        })
        .expect("Failed to insert node2");

    // Create first cluster and verify consistency
    let _edge1 = graph
        .insert_edge(EdgeSpec {
            from: node1_id,
            to: node2_id,
            edge_type: "consistency_test_1".to_string(),
            data: serde_json::json!({"phase": "first_cluster"}),
        })
        .expect("Failed to create edge 1");

    // Check consistency after first cluster
    {
        let mut graph_file = GraphFile::open(&db_path).expect("Failed to open graph file");

        let (header, file_size, node_region_end) = {
            let header = graph_file.header();
            let file_size = graph_file.file_size().expect("Failed to get file size");
            let node_region_end = header.node_data_offset + (header.node_count as u64 * 4096);
            (header, file_size, node_region_end)
        };

        println!(
            "After first cluster: file_size={}, edge_data_offset={}, node_data_offset={}",
            file_size, header.edge_data_offset, header.node_data_offset
        );

        // Verify magic number intact
        assert_eq!(
            header.magic, initial_header.magic,
            "Magic number corrupted after first cluster"
        );

        // Find and track cluster regions
        let mut node_store = NodeStore::new(&mut graph_file);
        if let Ok(node_v2) = node_store.read_node_v2(node1_id) {
            if node_v2.has_outgoing_edges() {
                let cluster_end =
                    node_v2.outgoing_cluster_offset + node_v2.outgoing_cluster_size as u64;
                max_written_offset = max_written_offset.max(cluster_end);

                // File must be large enough to contain all clusters
                assert!(
                    file_size >= max_written_offset,
                    "File size {} must be >= max_written_offset {} after first cluster",
                    file_size,
                    max_written_offset
                );

                // Clusters must be after node region
                assert!(
                    node_v2.outgoing_cluster_offset >= node_region_end,
                    "Cluster offset {} must be >= node_region_end {}",
                    node_v2.outgoing_cluster_offset,
                    node_region_end
                );
            }
        }
    }

    // Create second cluster and verify consistency
    let node3_id = graph
        .insert_node(NodeSpec {
            kind: "Node".to_string(),
            name: "node3".to_string(),
            file_path: None,
            data: serde_json::json!({"id": 3}),
        })
        .expect("Failed to insert node3");

    let _edge2 = graph
        .insert_edge(EdgeSpec {
            from: node2_id,
            to: node3_id,
            edge_type: "consistency_test_2".to_string(),
            data: serde_json::json!({"phase": "second_cluster"}),
        })
        .expect("Failed to create edge 2");

    // Check consistency after second cluster
    {
        let mut graph_file = GraphFile::open(&db_path).expect("Failed to open graph file");

        let (header, file_size, node_count, edge_data_offset, node_data_offset) = {
            let header = graph_file.header();
            let file_size = graph_file.file_size().expect("Failed to get file size");
            let node_count = header.node_count;
            let edge_data_offset = header.edge_data_offset;
            let node_data_offset = header.node_data_offset;
            (
                header,
                file_size,
                node_count,
                edge_data_offset,
                node_data_offset,
            )
        };

        println!(
            "After second cluster: file_size={}, edge_data_offset={}, node_data_offset={}",
            file_size, edge_data_offset, node_data_offset
        );

        // Verify magic number still intact
        assert_eq!(
            header.magic, initial_header.magic,
            "Magic number corrupted after second cluster"
        );

        // Track all cluster regions
        let mut node_store = NodeStore::new(&mut graph_file);
        for node_id in [node1_id, node2_id, node3_id] {
            if let Ok(node_v2) = node_store.read_node_v2(node_id) {
                if node_v2.has_outgoing_edges() {
                    let cluster_end =
                        node_v2.outgoing_cluster_offset + node_v2.outgoing_cluster_size as u64;
                    max_written_offset = max_written_offset.max(cluster_end);
                }
            }
        }

        // Final consistency check
        assert!(
            file_size >= max_written_offset,
            "Final file size {} must be >= max_written_offset {}",
            file_size,
            max_written_offset
        );

        // Header consistency: node_count should match actual nodes created
        assert_eq!(
            node_count, 3,
            "Node count should be 3 after creating 3 nodes"
        );

        // Node region should still be valid
        let node_region_end = node_data_offset + (node_count as u64 * 4096);
        assert!(
            edge_data_offset >= node_data_offset,
            "edge_data_offset {} must be >= node_data_offset {}",
            edge_data_offset,
            node_data_offset
        );

        println!(
            "Final consistency check passed: file_size={}, max_cluster_end={}, node_region_end={}",
            file_size, max_written_offset, node_region_end
        );
    }
}
