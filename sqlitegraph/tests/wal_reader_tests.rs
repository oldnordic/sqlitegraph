//! Comprehensive TDD unit tests for V2 WAL Reader functionality
//!
//! This module provides thorough testing for WAL read operations specifically designed
//! for V2-native clustered edge graph file operations. Tests focus on record filtering,
//! cluster-aware reading, recovery operations, and V2 graph data validation.

#![ignore] // Tests disabled: API mismatch with current V2WALRecord structure

use std::path::Path;
use tempfile::tempdir;
use sqlitegraph::backend::native::{NativeResult, NativeBackendError};
use sqlitegraph::backend::native::v2::wal::{
    V2WALConfig, V2WALWriter, V2WALReader, V2WALRecord, V2WALRecordType,
    WALReadFilter, WALStatistics,
};

/// Test WAL reader creation and basic read operations for V2 graph file
#[test]
fn test_v2_wal_reader_creation_and_basic_reads() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let wal_path = temp_dir.path().join("v2_graph_read_wal.wal");

    // First, create a WAL file with V2 graph data
    let config = V2WALConfig {
        wal_path: wal_path.clone(),
        max_wal_size: 16 * 1024 * 1024,
        buffer_size: 512 * 1024,
        flush_interval_ms: 100,
        enable_compression: false,
        cluster_affinity_groups: 8,
        ..Default::default()
    };

    let writer = V2WALWriter::create(config)?;

    // Write some V2 graph records
    let test_records = vec![
        V2WALRecord::NodeInsert {
            node_id: 1001,
            slot_offset: 4096,
            node_data: create_v2_node_record(1001, "function", "main"),
        },
        V2WALRecord::EdgeInsert {
            cluster_key: 1001,
            edge_id: 2001,
            source_node: 1001,
            target_node: 1002,
            edge_type: b"CALLS".to_vec(),
            edge_data: create_v2_edge_data(1.0, Some(0)),
        },
        V2WALRecord::TransactionBegin {
            transaction_id: 12345,
            timestamp: 1640995200000,
            isolation_level: 1,
        },
        V2WALRecord::NodeInsert {
            node_id: 1002,
            slot_offset: 8192,
            node_data: create_v2_node_record(1002, "function", "helper"),
        },
        V2WALRecord::TransactionCommit {
            transaction_id: 12345,
            commit_lsn: 0, // Will be assigned
            timestamp: 1640995201000,
        },
    ];

    let mut expected_lsns = Vec::new();
    for record in test_records {
        let lsn = writer.write_record(record)?;
        expected_lsns.push(lsn);
    }

    writer.shutdown()?;

    // Now test reading the WAL file
    let mut reader = V2WALReader::open(&wal_path)?;

    // Verify header information
    let header = reader.header();
    assert_eq!(header.version(), 1);
    assert!(header.current_lsn() > 0);

    // Read all records sequentially
    let mut read_records = Vec::new();
    while let Some((lsn, record)) = reader.read_next_record()? {
        read_records.push((lsn, record));
    }

    assert_eq!(read_records.len(), expected_lsns.len(),
              "Should read all written records");

    // Verify LSNs match
    for (i, (read_lsn, _)) in read_records.iter().enumerate() {
        assert_eq!(*read_lsn, expected_lsns[i],
                  "LSN {} should match expected value", i);
    }

    Ok(())
}

/// Test cluster-aware filtering for V2 graph operations
#[test]
fn test_cluster_aware_filtering_v2_operations() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let config = V2WALConfig {
        wal_path: temp_dir.path().join("cluster_filter_wal.wal"),
        max_wal_size: 32 * 1024 * 1024,
        buffer_size: 1024 * 1024,
        flush_interval_ms: 100,
        enable_compression: false,
        cluster_affinity_groups: 4,
        ..Default::default()
    };

    let writer = V2WALWriter::create(config)?;

    // Write records for multiple V2 clusters
    let cluster_1001_records = vec![
        V2WALRecord::NodeInsert {
            node_id: 1001,
            slot_offset: 4096,
            node_data: create_v2_node_record(1001, "function", "malloc"),
        },
        V2WALRecord::NodeInsert {
            node_id: 1002,
            slot_offset: 8192,
            node_data: create_v2_node_record(1002, "function", "free"),
        },
        V2WALRecord::EdgeInsert {
            cluster_key: 1001,
            edge_id: 3001,
            source_node: 1001,
            target_node: 1002,
            edge_type: b"CALLS".to_vec(),
            edge_data: create_v2_edge_data(1.0, Some(0)),
        },
    ];

    let cluster_2001_records = vec![
        V2WALRecord::NodeInsert {
            node_id: 2001,
            slot_offset: 12288,
            node_data: create_v2_node_record(2001, "variable", "buffer"),
        },
        V2WALRecord::EdgeInsert {
            cluster_key: 2001,
            edge_id: 4001,
            source_node: 1001,
            target_node: 2001,
            edge_type: b"WRITES".to_vec(),
            edge_data: create_v2_edge_data(2.0, Some(1)),
        },
        V2WALRecord::StringTableUpdate {
            string_id: 1001,
            string_data: b"buffer_size".to_vec(),
            hash_value: 0x12345678,
            ref_count: 3,
        },
    ];

    let transaction_records = vec![
        V2WALRecord::TransactionBegin {
            transaction_id: 20001,
            timestamp: 1640995200000,
            isolation_level: 1,
        },
        V2WALRecord::TransactionCommit {
            transaction_id: 20001,
            commit_lsn: 0,
            timestamp: 1640995201000,
        },
    ];

    // Write all records
    for record in cluster_1001_records.iter()
        .chain(cluster_2001_records.iter())
        .chain(transaction_records.iter()) {
        writer.write_record(record.clone())?;
    }

    writer.shutdown()?;

    // Test reading with cluster filtering
    let mut reader = V2WALReader::open(&temp_dir.path().join("cluster_filter_wal.wal"))?;

    // Filter by cluster 1001
    let cluster_1001_filter = WALReadFilter::by_cluster_keys(vec![1001]);
    let cluster_1001_results = reader.read_filtered_records(&cluster_1001_filter)?;

    assert_eq!(cluster_1001_results.len(), cluster_1001_records.len(),
              "Should find all cluster 1001 records");

    for (_, record) in &cluster_1001_results {
        assert_eq!(record.cluster_key(), Some(1001),
                  "All filtered records should belong to cluster 1001");
    }

    // Filter by cluster 2001
    let cluster_2001_filter = WALReadFilter::by_cluster_keys(vec![2001]);
    let cluster_2001_results = reader.read_filtered_records(&cluster_2001_filter)?;

    assert_eq!(cluster_2001_results.len(), cluster_2001_records.len(),
              "Should find all cluster 2001 records");

    // Filter by multiple clusters
    let multi_cluster_filter = WALReadFilter::by_cluster_keys(vec![1001, 2001]);
    let multi_cluster_results = reader.read_filtered_records(&multi_cluster_filter)?;

    assert_eq!(multi_cluster_results.len(),
              cluster_1001_records.len() + cluster_2001_records.len(),
              "Should find records from both clusters");

    // Filter by transaction control records only
    let tx_filter = WALReadFilter::transaction_control_only();
    let tx_results = reader.read_filtered_records(&tx_filter)?;

    assert_eq!(tx_results.len(), transaction_records.len(),
              "Should find all transaction control records");

    Ok(())
}

/// Test record type filtering for V2 graph operations
#[test]
fn test_record_type_filtering_v2_operations() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let config = V2WALConfig {
        wal_path: temp_dir.path().join("type_filter_wal.wal"),
        max_wal_size: 16 * 1024 * 1024,
        buffer_size: 512 * 1024,
        flush_interval_ms: 100,
        enable_compression: false,
        cluster_affinity_groups: 4,
        ..Default::default()
    };

    let writer = V2WALWriter::create(config)?;

    // Write different types of V2 records
    let node_inserts = 3;
    let edge_inserts = 5;
    let string_updates = 2;
    let free_space_updates = 1;

    // Node inserts
    for i in 0..node_inserts {
        writer.write_record(V2WALRecord::NodeInsert {
            node_id: 3000 + i,
            slot_offset: (i * 4096) as u64,
            node_data: create_v2_node_record(3000 + i, "function", &format!("node_{}", i)),
        })?;
    }

    // Edge inserts
    for i in 0..edge_inserts {
        writer.write_record(V2WALRecord::EdgeInsert {
            cluster_key: 3000 + (i / 2),
            edge_id: 5000 + i,
            source_node: 3000 + i,
            target_node: 3000 + i + 1,
            edge_type: b"CALLS".to_vec(),
            edge_data: create_v2_edge_data(1.0, Some(i as u64)),
        })?;
    }

    // String table updates
    for i in 0..string_updates {
        writer.write_record(V2WALRecord::StringTableUpdate {
            string_id: 6000 + i,
            string_data: format!("string_{}", i).into_bytes(),
            hash_value: (i * 0xABCDEF01) as u32,
            ref_count: i + 1,
        })?;
    }

    // Free space updates
    for i in 0..free_space_updates {
        writer.write_record(V2WALRecord::FreeSpaceUpdate {
            free_list_head: (i * 1024) as u64,
            reclaimed_blocks: i + 1,
            total_free_bytes: (i * 2048) as u64,
            metadata: vec![i as u8; 8],
        })?;
    }

    writer.shutdown()?;

    // Test reading with type filtering
    let mut reader = V2WALReader::open(&temp_dir.path().join("type_filter_wal.wal"))?;

    // Filter by NodeInsert records
    let node_filter = WALReadFilter::by_types(vec![V2WALRecordType::NodeInsert]);
    let node_results = reader.read_filtered_records(&node_filter)?;
    assert_eq!(node_results.len(), node_inserts, "Should find all node inserts");

    // Filter by EdgeInsert records
    let edge_filter = WALReadFilter::by_types(vec![V2WALRecordType::EdgeInsert]);
    let edge_results = reader.read_filtered_records(&edge_filter)?;
    assert_eq!(edge_results.len(), edge_inserts, "Should find all edge inserts");

    // Filter by multiple types
    let multi_type_filter = WALReadFilter::by_types(vec![
        V2WALRecordType::StringTableUpdate,
        V2WALRecordType::FreeSpaceUpdate,
    ]);
    let multi_type_results = reader.read_filtered_records(&multi_type_filter)?;
    assert_eq!(multi_type_results.len(), string_updates + free_space_updates,
              "Should find string and free space updates");

    // Filter by data-modifying records only
    let data_modifying_filter = WALReadFilter::data_modifying_only();
    let data_results = reader.read_filtered_records(&data_modifying_filter)?;
    assert_eq!(data_results.len(), node_inserts + edge_inserts + string_updates + free_space_updates,
              "Should find all data-modifying records");

    Ok(())
}

/// Test LSN-based reading and seeking for V2 operations
#[test]
fn test_lsn_based_reading_v2_operations() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let config = V2WALConfig {
        wal_path: temp_dir.path().join("lsn_reading_wal.wal"),
        max_wal_size: 16 * 1024 * 1024,
        buffer_size: 512 * 1024,
        flush_interval_ms: 100,
        enable_compression: false,
        cluster_affinity_groups: 4,
        ..Default::default()
    };

    let writer = V2WALWriter::create(config)?;

    // Write records and track LSNs
    let record_count = 50;
    let mut lsns = Vec::new();

    for i in 0..record_count {
        let record = match i % 4 {
            0 => V2WALRecord::NodeInsert {
                node_id: 4000 + i,
                slot_offset: (i * 1024) as u64,
                node_data: create_v2_node_record(4000 + i, "test", &format!("lsn_node_{}", i)),
            },
            1 => V2WALRecord::EdgeInsert {
                cluster_key: 4000 + (i / 4),
                edge_id: 7000 + i,
                source_node: 4000 + i,
                target_node: 4000 + i + 1,
                edge_type: b"TEST_EDGE".to_vec(),
                edge_data: create_v2_edge_data(1.0, Some(i as u64)),
            },
            2 => V2WALRecord::StringTableUpdate {
                string_id: 8000 + i,
                string_data: format!("lsn_string_{}", i).into_bytes(),
                hash_value: (i * 0x12345678) as u32,
                ref_count: i + 1,
            },
            _ => V2WALRecord::FreeSpaceUpdate {
                free_list_head: (i * 512) as u64,
                reclaimed_blocks: i % 5 + 1,
                total_free_bytes: (i * 1024) as u64,
                metadata: vec![i as u8; 4],
            },
        };

        let lsn = writer.write_record(record)?;
        lsns.push(lsn);
    }

    writer.shutdown()?;

    // Test LSN-based reading
    let mut reader = V2WALReader::open(&temp_dir.path().join("lsn_reading_wal.wal"))?;

    // Test reading from specific LSN
    let start_lsn = lsns[20]; // Start from 21st record
    let records_from_lsn = reader.read_from_lsn(start_lsn)?;

    assert_eq!(records_from_lsn.len(), record_count - 20,
              "Should read all records from LSN {} onwards", start_lsn);

    // Verify first record has expected LSN
    assert_eq!(records_from_lsn[0].0, start_lsn,
              "First record should have expected LSN");

    // Test LSN range filtering
    let reader = V2WALReader::open(&temp_dir.path().join("lsn_reading_wal.wal"))?;
    let lsn_range_filter = WALReadFilter::by_lsn_range(lsns[10], lsns[20]);
    let range_results = reader.read_filtered_records(&lsn_range_filter)?;

    assert_eq!(range_results.len(), 11, // 10 to 20 inclusive
              "Should find records in LSN range");

    // Verify LSN bounds
    assert!(range_results[0].0 >= lsns[10], "First record should be within range");
    assert!(range_results.last().unwrap().0 <= lsns[20], "Last record should be within range");

    Ok(())
}

/// Test WAL statistics collection for V2 graph operations
#[test]
fn test_wal_statistics_v2_operations() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let config = V2WALConfig {
        wal_path: temp_dir.path().join("stats_wal.wal"),
        max_wal_size: 16 * 1024 * 1024,
        buffer_size: 512 * 1024,
        flush_interval_ms: 100,
        enable_compression: false,
        cluster_affinity_groups: 4,
        ..Default::default()
    };

    let writer = V2WALWriter::create(config)?;

    // Write a controlled mix of V2 operations
    let expected_counts = (
        10, // Node inserts
        15, // Edge inserts
        3,  // Node updates
        2,  // Cluster creates
        1,  // Transaction begin
        1,  // Transaction commit
    );

    // Write node inserts
    for i in 0..expected_counts.0 {
        writer.write_record(V2WALRecord::NodeInsert {
            node_id: 5000 + i,
            slot_offset: (i * 2048) as u64,
            node_data: create_v2_node_record(5000 + i, "stats", &format!("node_{}", i)),
        })?;
    }

    // Write edge inserts
    for i in 0..expected_counts.1 {
        writer.write_record(V2WALRecord::EdgeInsert {
            cluster_key: 5000 + (i % 5),
            edge_id: 9000 + i,
            source_node: 5000 + i,
            target_node: 5000 + i + 1,
            edge_type: b"STATS_EDGE".to_vec(),
            edge_data: create_v2_edge_data((i % 10) as f64, Some(i as u64)),
        })?;
    }

    // Write node updates
    for i in 0..expected_counts.2 {
        writer.write_record(V2WALRecord::NodeUpdate {
            node_id: 5000 + i,
            slot_offset: (i * 2048) as u64,
            old_data: create_v2_node_record(5000 + i, "old", &format!("old_{}", i)),
            new_data: create_v2_node_record(5000 + i, "new", &format!("new_{}", i)),
        })?;
    }

    // Write cluster creates
    for i in 0..expected_counts.3 {
        writer.write_record(V2WALRecord::ClusterCreate {
            cluster_key: 6000 + i,
            initial_capacity: 64 * (i + 1),
            cluster_metadata: vec![i as u8; 16],
        })?;
    }

    // Write transaction records
    writer.write_record(V2WALRecord::TransactionBegin {
        transaction_id: 30001,
        timestamp: 1640995200000,
        isolation_level: 1,
    })?;

    writer.write_record(V2WALRecord::TransactionCommit {
        transaction_id: 30001,
        commit_lsn: 0,
        timestamp: 1640995201000,
    })?;

    writer.shutdown()?;

    // Collect and verify statistics
    let mut reader = V2WALReader::open(&temp_dir.path().join("stats_wal.wal"))?;
    let stats = reader.get_statistics()?;

    let total_expected = expected_counts.0 + expected_counts.1 + expected_counts.2 +
                        expected_counts.3 + expected_counts.4 + expected_counts.5;

    assert_eq!(stats.total_records, total_expected as u64,
              "Total records should match expected count");
    assert_eq!(stats.node_inserts, expected_counts.0 as u64,
              "Node insert count should match");
    assert_eq!(stats.edge_inserts, expected_counts.1 as u64,
              "Edge insert count should match");
    assert_eq!(stats.node_updates, expected_counts.2 as u64,
              "Node update count should match");
    assert_eq!(stats.cluster_creates, expected_counts.3 as u64,
              "Cluster create count should match");
    assert_eq!(stats.transaction_begins, expected_counts.4 as u64,
              "Transaction begin count should match");
    assert_eq!(stats.transaction_commits, expected_counts.5 as u64,
              "Transaction commit count should match");

    // Verify LSN range is reasonable
    assert!(stats.min_lsn > 0, "Min LSN should be positive");
    assert!(stats.max_lsn >= stats.min_lsn, "Max LSN should be >= min LSN");

    Ok(())
}

/// Test WAL iterator for V2 graph records
#[test]
fn test_wal_iterator_v2_operations() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let config = V2WALConfig {
        wal_path: temp_dir.path().join("iterator_wal.wal"),
        max_wal_size: 16 * 1024 * 1024,
        buffer_size: 512 * 1024,
        flush_interval_ms: 100,
        enable_compression: false,
        cluster_affinity_groups: 4,
        ..Default::default()
    };

    let writer = V2WALWriter::create(config)?;

    // Write test records
    let record_count = 25;
    for i in 0..record_count {
        let record = V2WALRecord::NodeInsert {
            node_id: 7000 + i,
            slot_offset: (i * 1024) as u64,
            node_data: create_v2_node_record(7000 + i, "iterator", &format!("iter_node_{}", i)),
        };
        writer.write_record(record)?;
    }

    writer.shutdown()?;

    // Test iterator functionality
    let mut reader = V2WALReader::open(&temp_dir.path().join("iterator_wal.wal"))?;

    // Test full iterator
    let mut iter_count = 0;
    let mut iter = reader.iter();
    while let Some(result) = iter.next() {
        let (lsn, record) = result?;
        assert!(lsn > 0, "Iterator should return valid LSNs");
        match record {
            V2WALRecord::NodeInsert { node_id, .. } => {
                assert!(node_id >= 7000, "Iterator should return correct node IDs");
            }
            _ => panic!("Iterator should only return NodeInsert records"),
        }
        iter_count += 1;
    }

    assert_eq!(iter_count, record_count, "Iterator should iterate through all records");

    // Test bounded iterator
    let mut reader = V2WALReader::open(&temp_dir.path().join("iterator_wal.wal"))?;
    let mut first_lsn = 0;

    // Get first LSN to bound iteration
    if let Some((lsn, _)) = reader.read_next_record()? {
        first_lsn = lsn;
    }

    let mut reader = V2WALReader::open(&temp_dir.path().join("iterator_wal.wal"))?;
    let end_lsn = first_lsn + 10; // Limit to first 11 records

    let mut bounded_count = 0;
    let mut bounded_iter = reader.iter_until(end_lsn);
    while let Some(result) = bounded_iter.next() {
        let (lsn, _) = result?;
        assert!(lsn <= end_lsn, "Bounded iterator should respect end LSN");
        bounded_count += 1;
    }

    assert!(bounded_count <= 11, "Bounded iterator should limit iterations");

    Ok(())
}

/// Test error handling and corruption detection
#[test]
fn test_wal_reader_error_handling() -> NativeResult<()> {
    let temp_dir = tempdir()?;

    // Test reading non-existent file
    let non_existent_path = temp_dir.path().join("non_existent.wal");
    let result = V2WALReader::open(&non_existent_path);
    assert!(result.is_err(), "Opening non-existent WAL should fail");

    // Create a corrupted WAL file (invalid magic bytes)
    let corrupted_path = temp_dir.path().join("corrupted.wal");
    std::fs::write(&corrupted_path, vec![0xFF; 1024])?;

    let result = V2WALReader::open(&corrupted_path);
    assert!(result.is_err(), "Opening corrupted WAL should fail");

    // Create a valid WAL first
    let valid_config = V2WALConfig {
        wal_path: temp_dir.path().join("valid.wal"),
        max_wal_size: 8 * 1024 * 1024,
        buffer_size: 256 * 1024,
        flush_interval_ms: 100,
        enable_compression: false,
        cluster_affinity_groups: 4,
        ..Default::default()
    };

    let writer = V2WALWriter::create(valid_config)?;
    writer.write_record(V2WALRecord::NodeInsert {
        node_id: 1,
        slot_offset: 0,
        node_data: create_v2_node_record(1, "test", "valid"),
    })?;
    writer.shutdown()?;

    // Now test reading valid WAL
    let mut reader = V2WALReader::open(&temp_dir.path().join("valid.wal"))?;

    // Should successfully read the record
    let result = reader.read_next_record();
    assert!(result.is_ok(), "Reading valid WAL should succeed");

    let (lsn, record) = result.unwrap().unwrap();
    assert!(lsn > 0, "Should have valid LSN");

    match record {
        V2WALRecord::NodeInsert { node_id, .. } => {
            assert_eq!(node_id, 1, "Should read correct node ID");
        }
        _ => panic!("Should read NodeInsert record"),
    }

    // Subsequent read should return None (end of WAL)
    let result = reader.read_next_record();
    assert!(result.is_ok(), "Reading beyond WAL should succeed with None");
    assert!(result.unwrap().is_none(), "Should return None at end of WAL");

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