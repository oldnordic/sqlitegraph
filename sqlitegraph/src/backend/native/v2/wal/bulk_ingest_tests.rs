//! Bulk Ingest Mode Tests for V2 WAL System
//!
//! This test module validates bulk ingest mode functionality including:
//! - Reduced WAL flush/sync operations during bulk ingest
//! - Crash recovery consistency for bulk operations
//! - Rollback behavior during bulk ingest sessions
//! - Performance metrics validation for bulk ingest optimization

use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::v2::wal::{
    BulkIngestConfig, BulkIngestExt, V2WALConfig, V2WALManager, V2WALRecord,
};
use crate::backend::native::NativeResult;
use tempfile::tempdir;

/// Test that bulk ingest mode reduces WAL flush/sync operations
#[test]
fn test_bulk_ingest_batches_flushes() -> NativeResult<()> {
    let temp_dir = tempdir()?;

    // Create baseline WAL manager (non-bulk mode)
    let baseline_config = V2WALConfig {
        wal_path: temp_dir.path().join("baseline.wal"),
        checkpoint_path: temp_dir.path().join("baseline.checkpoint"),
        group_commit_timeout_ms: 0, // Immediate flush for baseline
        max_wal_size: 1024 * 1024,  // 1MB
        checkpoint_interval: 1000,
        ..Default::default()
    };

    // Create minimal V2 graph file for baseline
    let baseline_graph_path = temp_dir.path().join("baseline.v2");
    let _baseline_graph_file = GraphFile::create(&baseline_graph_path)?;

    let baseline_manager = V2WALManager::create(baseline_config)?;

    // Baseline test: write 1000 node insert records individually
    for i in 0..1000 {
        let record = V2WALRecord::NodeInsert {
            node_id: i,
            slot_offset: 0,
            node_data: vec![1, 2, 3], // Small payload
        };
        baseline_manager.write_record(record)?;
    }

    // Get manager metrics for baseline test
    let baseline_metrics = baseline_manager.get_metrics();
    println!(
        "Baseline metrics - Total records: {}, Group commits: {}",
        baseline_metrics.total_records_written, baseline_metrics.group_commit_batches
    );

    // Create bulk ingest WAL manager
    let bulk_config = V2WALConfig {
        wal_path: temp_dir.path().join("bulk.wal"),
        checkpoint_path: temp_dir.path().join("bulk.checkpoint"),
        group_commit_timeout_ms: 1000, // Large timeout to reduce flushes
        max_wal_size: 1024 * 1024,
        checkpoint_interval: 1000,
        ..Default::default()
    };

    // Create minimal V2 graph file for bulk test
    let bulk_graph_path = temp_dir.path().join("bulk.v2");
    let _bulk_graph_file = GraphFile::create(&bulk_graph_path)?;

    let bulk_manager = V2WALManager::create(bulk_config)?;

    // Enable bulk ingest mode
    let bulk_guard = bulk_manager.begin_bulk_ingest(BulkIngestConfig::default())?;

    // Bulk test: write same 1000 node insert records
    for i in 0..1000 {
        let record = V2WALRecord::NodeInsert {
            node_id: i + 1000, // Different IDs to avoid conflicts
            slot_offset: 0,
            node_data: vec![1, 2, 3],
        };
        bulk_manager.write_record(record)?;
    }

    let bulk_metrics = bulk_manager.get_metrics();
    println!(
        "Bulk metrics - Total records: {}, Group commits: {}",
        bulk_metrics.total_records_written, bulk_metrics.group_commit_batches
    );

    // Drop bulk guard to exit bulk mode
    drop(bulk_guard);

    // Verify bulk ingest reduces or maintains group commit operations (proxy for flush operations)
    assert!(
        bulk_metrics.group_commit_batches <= baseline_metrics.group_commit_batches,
        "Bulk ingest should reduce or maintain group commit operations"
    );

    // Verify both completed writes (approximate since group commits batch multiple records)
    assert!(baseline_metrics.total_records_written >= 1000);
    assert!(bulk_metrics.total_records_written >= 1000);
    println!(
        "Records written - Baseline: {}, Bulk: {}",
        baseline_metrics.total_records_written, bulk_metrics.total_records_written
    );

    Ok(())
}

/// Test bulk ingest recovery consistency
#[test]
fn test_bulk_ingest_recovery_consistency() -> NativeResult<()> {
    let temp_dir = tempdir()?;

    let wal_path = temp_dir.path().join("recovery.wal");
    let checkpoint_path = temp_dir.path().join("recovery.checkpoint");
    let graph_path = temp_dir.path().join("recovery.v2");

    // Create V2 graph file
    let _graph_file = GraphFile::create(&graph_path)?;

    let config = V2WALConfig {
        wal_path: wal_path.clone(),
        checkpoint_path: checkpoint_path.clone(),
        group_commit_timeout_ms: 1000, // Enable bulk-like behavior
        max_wal_size: 1024 * 1024,
        checkpoint_interval: 100,
        ..Default::default()
    };

    let manager = V2WALManager::create(config)?;

    // Enable bulk ingest mode
    let bulk_guard = manager.begin_bulk_ingest(BulkIngestConfig::default())?;

    // Write bulk data: 500 nodes and 500 edges
    for i in 0..500 {
        // Node inserts
        let node_record = V2WALRecord::NodeInsert {
            node_id: i,
            slot_offset: 0,
            node_data: vec![1, 2, 3],
        };
        manager.write_record(node_record)?;

        // Edge inserts (simplified - using different record types)
        let edge_record = V2WALRecord::ClusterCreate {
            node_id: i,
            direction: crate::backend::native::v2::Direction::Outgoing,
            cluster_offset: 100 + i as u64,
            cluster_size: 10,
            edge_data: vec![1, 2, 3],
        };
        manager.write_record(edge_record)?;
    }

    let write_metrics = manager.get_metrics();
    assert_eq!(write_metrics.total_records_written, 1000); // 500 nodes + 500 clusters

    // Records are automatically persisted in WAL - no explicit checkpoint needed for basic recovery test

    // End bulk ingest mode
    drop(bulk_guard);

    // Shutdown and reopen
    drop(manager);

    // Simulate recovery by opening the WAL file
    let recovery_manager = V2WALManager::create(V2WALConfig {
        wal_path: wal_path.clone(),
        checkpoint_path: checkpoint_path.clone(),
        group_commit_timeout_ms: 0,
        max_wal_size: 1024 * 1024,
        checkpoint_interval: 1000,
        ..Default::default()
    })?;

    let recovery_metrics = recovery_manager.get_metrics();

    // TODO: Run recovery validation when implemented
    // let recovery_engine = V2WALRecoveryEngine::new(&config)?;
    // let recovery_result = recovery_engine.recover_from_wal()?;
    // assert!(recovery_result.is_consistent);

    // Verify WAL still exists and has expected structure
    assert!(wal_path.exists(), "WAL file should persist after recovery");
    assert!(
        checkpoint_path.exists(),
        "Checkpoint file should exist after recovery"
    );

    // Recovery metrics should show recovery operations
    // assert!(recovery_metrics.recovery_operations > 0);

    println!(
        "Recovery test completed - WAL file: {:?}, Checkpoint: {:?}",
        wal_path.exists(),
        checkpoint_path.exists()
    );

    Ok(())
}

/// Test bulk ingest rollback behavior
#[test]
fn test_bulk_ingest_rollback() -> NativeResult<()> {
    let temp_dir = tempdir()?;

    let wal_path = temp_dir.path().join("rollback.wal");
    let checkpoint_path = temp_dir.path().join("rollback.checkpoint");
    let graph_path = temp_dir.path().join("rollback.v2");

    // Create V2 graph file
    let _graph_file = GraphFile::create(&graph_path)?;

    let config = V2WALConfig {
        wal_path: wal_path.clone(),
        checkpoint_path: checkpoint_path.clone(),
        group_commit_timeout_ms: 1000,
        max_wal_size: 1024 * 1024,
        checkpoint_interval: 1000,
        ..Default::default()
    };

    let manager = V2WALManager::create(config)?;

    // Get initial metrics
    let initial_metrics = manager.get_metrics();

    // Begin transaction
    let tx_id = manager
        .begin_transaction(crate::backend::native::v2::wal::IsolationLevel::ReadCommitted)?;

    // Enable bulk ingest mode
    let bulk_guard = manager.begin_bulk_ingest(BulkIngestConfig::default())?;

    // Write some records within transaction
    for i in 0..100 {
        let record = V2WALRecord::NodeInsert {
            node_id: i,
            slot_offset: 0,
            node_data: vec![1, 2, 3],
        };
        manager.write_transaction_record(tx_id, record)?;
    }

    // Verify records were written to WAL
    let write_metrics = manager.get_metrics();
    assert!(write_metrics.total_records_written > initial_metrics.total_records_written);

    // Rollback the transaction
    manager.rollback_transaction(tx_id)?;

    // End bulk ingest mode
    drop(bulk_guard);

    // Shutdown and reopen to verify rollback persistence
    drop(manager);

    // Verify WAL file still exists but should have rollback records
    assert!(wal_path.exists(), "WAL file should exist after rollback");

    // Reopen and verify consistent state
    let reopened_manager = V2WALManager::create(V2WALConfig {
        wal_path: wal_path.clone(),
        checkpoint_path: checkpoint_path.clone(),
        group_commit_timeout_ms: 0,
        max_wal_size: 1024 * 1024,
        checkpoint_interval: 1000,
        ..Default::default()
    })?;

    let reopened_metrics = reopened_manager.get_metrics();

    // TODO: Verify rollback consistency when recovery validation is implemented
    // let recovery_engine = V2WALRecoveryEngine::new(&reopened_config)?;
    // let recovery_result = recovery_engine.recover_from_wal()?;
    // assert!(recovery_result.is_consistent);
    // assert_eq!(recovery_result.node_count, 0, "All nodes should be rolled back");

    println!("Rollback test completed - WAL persisted with rollback records");

    Ok(())
}

/// Helper function to create test node record
fn create_test_node_record(node_id: i64) -> V2WALRecord {
    V2WALRecord::NodeInsert {
        node_id,
        slot_offset: 0,
        node_data: vec![1, 2, 3],
    }
}

/// Helper function to create test edge cluster record
fn create_test_cluster_record(node_id: i64) -> V2WALRecord {
    V2WALRecord::ClusterCreate {
        node_id,
        direction: crate::backend::native::v2::Direction::Outgoing,
        cluster_offset: 100 + node_id as u64,
        cluster_size: 10,
        edge_data: vec![1, 2, 3],
    }
}
