//! Comprehensive TDD unit tests for V2 WAL Writer functionality
//!
//! This module provides thorough testing for WAL write operations specifically designed
//! for V2-native clustered edge graph file operations. Tests focus on cluster-affinity
//! logging, sequential write patterns, and V2 graph data integrity.


use sqlitegraph::backend::native::v2::wal::{
    V2WALConfig, V2WALRecord, V2WALRecordType, V2WALWriter,
};
use sqlitegraph::backend::native::v2::edge_cluster::{CompactEdgeRecord, Direction};
use sqlitegraph::backend::native::{NativeBackendError, NativeResult};
use std::path::Path;
use tempfile::tempdir;

/// Test WAL writer creation and basic write operations for V2 graph file
#[test]
fn test_v2_wal_writer_creation_and_basic_writes() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let wal_path = temp_dir.path().join("v2_graph_wal.wal");

    let config = V2WALConfig {
        wal_path: wal_path.clone(),
        checkpoint_path: temp_dir.path().join("checkpoint.tracker"),
        max_wal_size: 32 * 1024 * 1024, // 32MB
        buffer_size: 1024 * 1024,       // 1MB
        checkpoint_interval: 1000,
        group_commit_timeout_ms: 100,
        max_group_commit_size: 8, // 8 records in group commit batch
        enable_compression: false,
        compression_level: 3,
    };

    let writer = V2WALWriter::create(config)?;

    // Verify WAL file is created
    assert!(wal_path.exists(), "V2 WAL file should be created");

    // Test writing a V2 NodeRecordV2 insert
    let node_record = V2WALRecord::NodeInsert {
        node_id: 1001,
        slot_offset: 4096,
        node_data: vec![
            0x01, 0x00, // Version
            0x02, 0x00, // Flags (used, active)
            0x10, 0x00, 0x00, 0x00, // Degree: 16
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // First edge: 0
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, // Last edge: 0
                  // Node data follows...
        ],
    };

    let lsn = writer.write_record(node_record)?;
    assert!(lsn > 0, "LSN should be positive after writing record");

    // Test writing a V2 EdgeCluster insert with cluster affinity
    let edge_record = CompactEdgeRecord::new(
        1002 as i64, // neighbor_id
        0, // edge_type_offset
        vec![
            0x01, // Edge version
            0x04, // Edge flags (used, active, directed, weighted)
            0x80, 0x00, 0x00, 0x00, // Weight: 128
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Timestamp
        ]
    );

    let edge_cluster_record = V2WALRecord::EdgeInsert {
        cluster_key: (1001, Direction::Outgoing), // (node_id, direction)
        edge_record: edge_record,
        insertion_point: 0, // Insert at beginning of cluster
    };

    let edge_lsn = writer.write_record(edge_cluster_record)?;
    assert!(edge_lsn > lsn, "Edge LSN should be greater than node LSN");

    writer.shutdown()?;

    Ok(())
}

/// Test cluster affinity grouping for V2 edge operations
#[test]
fn test_cluster_affinity_grouping_v2_operations() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let config = V2WALConfig {
        wal_path: temp_dir.path().join("cluster_affinity_wal.wal"),
        checkpoint_path: temp_dir.path().join("checkpoint.tracker"),
        max_wal_size: 16 * 1024 * 1024,
        buffer_size: 512 * 1024,
        checkpoint_interval: 1000,
        group_commit_timeout_ms: 50,
        max_group_commit_size: 4,
        enable_compression: false,
        compression_level: 3,
    };

    let writer = V2WALWriter::create(config)?;

    // Create operations for different V2 clusters to test locality
    let cluster_operations = vec![
        // Cluster 1001 - Software functions
        V2WALRecord::NodeInsert {
            node_id: 1001,
            slot_offset: 8192,
            node_data: create_v2_node_record(1001, "function", "malloc"),
        },
        V2WALRecord::EdgeInsert {
            cluster_key: (1001, Direction::Outgoing),
            edge_record: CompactEdgeRecord::new(1002 as i64, 0, create_v2_edge_data(1.0, Some(0))),
            insertion_point: 0,
        },
        V2WALRecord::EdgeInsert {
            cluster_key: (1002, Direction::Outgoing),
            edge_record: CompactEdgeRecord::new(1001 as i64, 1, create_v2_edge_data(2.0, Some(1))),
            insertion_point: 0,
        },
        // Cluster 2001 - Variables
        V2WALRecord::NodeInsert {
            node_id: 2001,
            slot_offset: 12288,
            node_data: create_v2_node_record(2001, "variable", "buffer"),
        },
        V2WALRecord::EdgeInsert {
            cluster_key: (2001, Direction::Outgoing),
            edge_record: CompactEdgeRecord::new(1001 as i64, 0, create_v2_edge_data(1.0, Some(0))),
            insertion_point: 0,
        },
        // Cluster 3001 - Memory operations
        V2WALRecord::NodeInsert {
            node_id: 3001,
            slot_offset: 16384,
            node_data: create_v2_node_record(3001, "function", "free"),
        },
        V2WALRecord::EdgeInsert {
            cluster_key: (1002, Direction::Outgoing), // Cross-cluster reference
            edge_record: CompactEdgeRecord::new(1001 as i64, 2, create_v2_edge_data(1.0, Some(2))),
            insertion_point: 0,
        },
    ];

    let mut cluster_1001_count = 0;
    let mut cluster_2001_count = 0;
    let mut cluster_3001_count = 0;

    // Write operations and track cluster affinity
    for operation in cluster_operations {
        let lsn = writer.write_record(operation.clone())?;
        assert!(lsn > 0, "LSN should be positive for V2 operation");

        // Count operations by cluster key
        if let V2WALRecord::NodeInsert { node_id, .. } = operation.clone() {
            match node_id {
                1001..=1999 => cluster_1001_count += 1,
                2001..=2999 => cluster_2001_count += 1,
                3001..=3999 => cluster_3001_count += 1,
                _ => {}
            }
        }
    }

    // Verify cluster distribution
    assert!(
        cluster_1001_count >= 2,
        "Cluster 1001 should have multiple operations"
    );
    assert!(
        cluster_2001_count >= 1,
        "Cluster 2001 should have operations"
    );
    assert!(
        cluster_3001_count >= 1,
        "Cluster 3001 should have operations"
    );

    writer.shutdown()?;

    Ok(())
}

/// Test V2 graph-specific batch write operations
#[test]
fn test_v2_graph_batch_write_operations() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let config = V2WALConfig {
        wal_path: temp_dir.path().join("v2_batch_wal.wal"),
        checkpoint_path: temp_dir.path().join("checkpoint.tracker"),
        max_wal_size: 64 * 1024 * 1024,
        buffer_size: 2 * 1024 * 1024,
        checkpoint_interval: 1000,
        group_commit_timeout_ms: 200,
        max_group_commit_size: 50, // Batch up to 50 records
        enable_compression: true,
        compression_level: 3,
    };

    let writer = V2WALWriter::create(config)?;

    // Create a realistic V2 graph batch - representing a function call graph
    let batch_size = 100;
    let mut batch_records = Vec::with_capacity(batch_size);

    // Add transaction begin
    batch_records.push(V2WALRecord::TransactionBegin {
        tx_id: 10001,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    });

    // Create nodes for functions in a call chain
    for i in 0..20 {
        let function_id = 5000 + i;
        batch_records.push(V2WALRecord::NodeInsert {
            node_id: function_id,
            slot_offset: (i * 4096) as u64,
            node_data: create_v2_node_record(function_id, "function", &format!("func_{}", i)),
        });
    }

    // Create edges representing call relationships
    for i in 0..20 {
        if i < 19 {
            // Don't create edge from last function
            batch_records.push(V2WALRecord::EdgeInsert {
                cluster_key: ((5000 + i) as i64, Direction::Outgoing), // Group into clusters of 4
                edge_record: CompactEdgeRecord::new((5000 + i + 1) as i64, 0, create_v2_edge_data(1.0, Some(i as u64))),
                insertion_point: 0,
            });
        }
    }

    // Add variable nodes and write edges
    for i in 0..10 {
        let var_id = 7000 + i;
        batch_records.push(V2WALRecord::NodeInsert {
            node_id: var_id,
            slot_offset: ((20 + i) * 4096) as u64,
            node_data: create_v2_node_record(var_id, "variable", &format!("var_{}", i)),
        });

        // Connect some functions to variables (writes/reads)
        let func_id = 5000 + (i % 20);
        batch_records.push(V2WALRecord::EdgeInsert {
            cluster_key: (func_id as i64, Direction::Outgoing), // Affinity to the function
            edge_record: CompactEdgeRecord::new((func_id + 1) as i64, 1, create_v2_edge_data(1.0, Some((i * 2) as u64))),
            insertion_point: 0,
        });
    }

    // Add transaction commit
    batch_records.push(V2WALRecord::TransactionCommit {
        tx_id: 10001,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64,
    });

    // Write the entire batch
    let mut lsns = Vec::new();
    for record in batch_records {
        let lsn = writer.write_record(record)?;
        lsns.push(lsn);
    }

    // Verify all records got LSNs
    assert_eq!(
        lsns.len(),
        1 + 20 + 19 + 10 + 10 + 1,
        "Should have written all records"
    );

    // LSNs should be sequential
    for i in 1..lsns.len() {
        assert!(lsns[i] > lsns[i - 1], "LSNs should be strictly increasing");
    }

    writer.shutdown()?;

    Ok(())
}

/// Test V2 free space and string table operations
#[test]
fn test_v2_free_space_and_string_table_operations() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let config = V2WALConfig {
        wal_path: temp_dir.path().join("v2_metadata_wal.wal"),
        checkpoint_path: temp_dir.path().join("checkpoint.tracker"),
        max_wal_size: 32 * 1024 * 1024,
        buffer_size: 1024 * 1024,
        checkpoint_interval: 1000,
        group_commit_timeout_ms: 100,
        max_group_commit_size: 8,
        enable_compression: false,
        compression_level: 3,
    };

    let writer = V2WALWriter::create(config)?;

    // Test V2 free space operations
    let free_space_update = V2WALRecord::FreeSpaceAllocate {
        block_offset: 4096,
        block_size: 20480,
        block_type: 1,
    };

    let lsn1 = writer.write_record(free_space_update)?;
    assert!(lsn1 > 0);

    // Test V2 string table operations
    let string_table_update = V2WALRecord::StringInsert {
        string_id: 1001,
        string_value: "function_main".to_string(),
    };

    let lsn2 = writer.write_record(string_table_update)?;
    assert!(lsn2 > lsn1);

    // Test cluster operations for V2 edge clustering
    let cluster_create = V2WALRecord::ClusterCreate {
        node_id: 3001 as i64,
        direction: Direction::Outgoing,
        cluster_offset: 4096,
        cluster_size: 256,
        edge_data: vec![
            0x01, // Version
            0x00, 0x01, // Initial capacity (256)
            0x00, 0x00, 0x00, 0x00, // Current size (0)
            0x00, 0x00, 0x00, 0x00, // Next edge slot (0)
        ],
    };

    let lsn3 = writer.write_record(cluster_create)?;
    assert!(lsn3 > lsn2);

    // ClusterResize operation not available in current V2 API
    // Commented out as ClusterResize variant doesn't exist
    /*
    let cluster_resize = V2WALRecord::ClusterResize {
        cluster_key: 3001,
        old_capacity: 256,
        new_capacity: 512,
        new_location: 8192, // New cluster file offset
        migration_metadata: vec![0x02, 0x00, 0x02, 0x00], // Resize flags and info
    };

    let lsn4 = writer.write_record(cluster_resize)?;
    assert!(lsn4 > lsn3);
    */

    writer.shutdown()?;

    Ok(())
}

/// Test WAL write performance for V2 graph operations
#[test]
fn test_v2_wal_write_performance() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let config = V2WALConfig {
        wal_path: temp_dir.path().join("perf_v2_wal.wal"),
        checkpoint_path: temp_dir.path().join("checkpoint.tracker"),
        max_wal_size: 256 * 1024 * 1024, // 256MB
        buffer_size: 8 * 1024 * 1024,    // 8MB buffer
        checkpoint_interval: 1000,
        group_commit_timeout_ms: 1000,   // Less frequent flushing for performance
        max_group_commit_size: 100,      // Large batch size
        enable_compression: true,
        compression_level: 3,
    };

    let writer = V2WALWriter::create(config)?;

    let start_time = std::time::Instant::now();
    let target_operations = 10_000;

    // Create realistic V2 graph operations
    for i in 0..target_operations {
        let record = if i % 10 == 0 {
            // 10% nodes
            V2WALRecord::NodeInsert {
                node_id: 10000 + i,
                slot_offset: (i * 512) as u64,
                node_data: create_v2_node_record(
                    10000 + i,
                    "function",
                    &format!("perf_func_{}", i),
                ),
            }
        } else if i % 10 < 7 {
            // 60% edges (majority of operations)
            let cluster_key = 10000 + ((i / 10) * 10);
            V2WALRecord::EdgeInsert {
                cluster_key: (cluster_key as i64, Direction::Outgoing),
                edge_record: CompactEdgeRecord::new((cluster_key + (i % 10) + 1) as i64, (i % 10) as u16, create_v2_edge_data((i % 10) as f64, Some((i / 3) as u64))),
                insertion_point: 0,
            }
        } else if i % 10 == 8 {
            // 10% string table updates
            V2WALRecord::StringInsert {
                string_id: (30000 + i) as u32,
                string_value: format!("perf_string_{}", i),
            }
        } else {
            // 10% free space updates
            V2WALRecord::FreeSpaceAllocate {
                block_offset: (i * 1024) as u64,
                block_size: ((i % 10) + 1) as u32 * 64,
                block_type: (i % 256) as u8,
            }
        };

        writer.write_record(record)?;
    }

    let elapsed = start_time.elapsed();
    let ops_per_second = target_operations as f64 / elapsed.as_secs_f64();

    // Should achieve high throughput for V2 operations
    assert!(
        ops_per_second >= 5_000.0,
        "V2 WAL should handle at least 5K ops/sec: {:.0} ops/sec",
        ops_per_second
    );

    writer.shutdown()?;

    Ok(())
}

/// Test WAL write buffer management for V2 operations
#[test]
fn test_v2_wal_write_buffer_management() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let config = V2WALConfig {
        wal_path: temp_dir.path().join("buffer_test_wal.wal"),
        checkpoint_path: temp_dir.path().join("checkpoint.tracker"),
        max_wal_size: 32 * 1024 * 1024,
        buffer_size: 64 * 1024, // Small buffer to test flushing
        checkpoint_interval: 1000,
        group_commit_timeout_ms: 10,   // Very frequent flushing
        max_group_commit_size: 4,
        enable_compression: false,
        compression_level: 3,
    };

    let writer = V2WALWriter::create(config)?;

    // Write records that exceed buffer size to force flushing
    let large_data = vec![0x42; 4096]; // 4KB records
    let record_count = 100;

    let mut lsns = Vec::new();
    for i in 0..record_count {
        let record = V2WALRecord::NodeInsert {
            node_id: 9000 + i,
            slot_offset: (i * 8192) as u64,
            node_data: large_data.clone(),
        };

        let lsn = writer.write_record(record)?;
        lsns.push(lsn);

        // Every few records should trigger a flush due to buffer size
        if i % 15 == 0 {
            // Force periodic flush verification
            writer.flush_buffer()?;
        }
    }

    // Verify all records were written with valid LSNs
    assert_eq!(lsns.len(), record_count as usize, "All records should be written");
    for (i, &lsn) in lsns.iter().enumerate() {
        assert!(lsn > 0, "LSN {} should be positive", i);
        if i > 0 {
            assert!(lsn > lsns[i - 1], "LSNs should be increasing");
        }
    }

    writer.shutdown()?;

    Ok(())
}

/// Test WAL write error handling and recovery
#[test]
fn test_v2_wal_write_error_handling() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let config = V2WALConfig {
        wal_path: temp_dir.path().join("error_test_wal.wal"),
        checkpoint_path: temp_dir.path().join("checkpoint.tracker"),
        max_wal_size: 1024 * 1024, // Very small WAL to test size limits
        buffer_size: 64 * 1024,
        checkpoint_interval: 1000,
        group_commit_timeout_ms: 50,
        max_group_commit_size: 2,
        enable_compression: false,
        compression_level: 3,
    };

    let writer = V2WALWriter::create(config)?;

    // Write normal records
    for i in 0..10 {
        let record = V2WALRecord::NodeInsert {
            node_id: 8000 + i,
            slot_offset: (i * 1024) as u64,
            node_data: create_v2_node_record(8000 + i, "test", &format!("node_{}", i)),
        };
        writer.write_record(record)?;
    }

    // Attempt to write extremely large records that should fail gracefully
    let oversized_data = vec![0xFF; 10 * 1024 * 1024]; // 10MB record
    let oversized_record = V2WALRecord::NodeInsert {
        node_id: 9999,
        slot_offset: 0,
        node_data: oversized_data,
    };

    // This should either succeed (if WAL size allows) or fail gracefully
    let result = writer.write_record(oversized_record);

    // If it fails, it should fail with a proper error, not panic
    if let Err(e) = result {
        match e {
            NativeBackendError::Io { .. }
            | NativeBackendError::InvalidConfiguration { .. } => {
                // These are acceptable error types
            }
            _ => {
                panic!("Unexpected error type: {:?}", e);
            }
        }
    }

    // Writer should still be functional after the error
    let recovery_record = V2WALRecord::NodeInsert {
        node_id: 8000,
        slot_offset: 10240,
        node_data: create_v2_node_record(8000, "recovery", "test_recovery"),
    };

    let result = writer.write_record(recovery_record);
    assert!(result.is_ok(), "Writer should recover and allow new writes");

    writer.shutdown()?;

    Ok(())
}

/// Helper function to create V2 node record data
fn create_v2_node_record(node_id: i64, node_type: &str, name: &str) -> Vec<u8> {
    let mut data = Vec::new();

    // V2 NodeRecordV2 header
    data.extend_from_slice(&[0x02, 0x00]); // Version 2
    data.extend_from_slice(&[0x01]); // Flags: used

    // Basic node metadata
    data.extend_from_slice(&(node_type.len() as u16).to_le_bytes());
    data.extend_from_slice(node_type.as_bytes());

    data.extend_from_slice(&(name.len() as u16).to_le_bytes());
    data.extend_from_slice(name.as_bytes());

    // Node ID and basic stats
    data.extend_from_slice(&node_id.to_le_bytes());
    data.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // Degree: 0

    // Pad to reasonable size
    data.resize(128, 0);

    data
}

/// Helper function to create V2 edge data
fn create_v2_edge_data(weight: f64, timestamp: Option<u64>) -> Vec<u8> {
    let mut data = Vec::new();

    // V2 edge header
    data.extend_from_slice(&[0x01, 0x00]); // Version 1
    data.extend_from_slice(&[0x05]); // Flags: used, active, directed, weighted, timestamped

    // Weight
    data.extend_from_slice(&weight.to_le_bytes());

    // Timestamp if provided
    if let Some(ts) = timestamp {
        data.extend_from_slice(&ts.to_le_bytes());
    } else {
        data.extend_from_slice(&[0u8; 8]);
    }

    // Pad to reasonable size
    data.resize(64, 0);

    data
}
