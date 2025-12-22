//! TDD Tests for Formal Crash Recovery System
//!
//! These tests validate the explicit recovery state model and authority resolution.
//! All tests must initially FAIL until the recovery coordinator is implemented.
//!
//! Tests create real graph files + WALs, perform real writes, simulate crashes,
//! and assert correct recovery decisions and replay behavior.

use sqlitegraph::backend::native::graph_file::GraphFile;
use sqlitegraph::backend::native::v2::wal::{
    V2WALConfig, V2WALRecord, V2WALManager, V2WALHeader, V2WALRecordType,
};
use sqlitegraph::backend::native::v2::{Direction};
use sqlitegraph::backend::native::v2::wal::recovery::{
    Authority, RecoveryContext, RecoverySeverity,
};
use sqlitegraph::backend::native::v2::wal::recovery::states::RecoveryState as ExplicitRecoveryState;
use sqlitegraph::backend::native::{NativeBackendError, NativeResult};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

/// Test 1: Clean shutdown detection - no replay required
#[test]
fn test_recovery_clean_shutdown_no_replay() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let graph_path = temp_dir.path().join("test.v2");
    let wal_path = temp_dir.path().join("test.wal");
    let checkpoint_path = temp_dir.path().join("test.checkpoint");

    // Create a clean graph file
    let _graph_file = GraphFile::create(&graph_path)?;

    // Create WAL with clean shutdown state
    let config = V2WALConfig {
        wal_path: wal_path.clone(),
        checkpoint_path: checkpoint_path.clone(),
        ..Default::default()
    };
    let manager = V2WALManager::create(config)?;

    // Write and commit a transaction cleanly
    let tx_id = manager.begin_transaction(sqlitegraph::backend::native::v2::wal::TransactionIsolation::ReadCommitted)?;
    manager.write_transaction_record(tx_id, V2WALRecord::NodeInsert {
        node_id: 1,
        slot_offset: 0,
        node_data: vec![1, 2, 3],
    })?;
    manager.commit_transaction(tx_id)?;

    // Force checkpoint to ensure clean state
    manager.force_checkpoint()?;

    // Analyze recovery context
    let context = RecoveryContext::analyze_files(&wal_path, &graph_path, &checkpoint_path)?;

    // Assert: Should detect clean shutdown
    assert_eq!(context.state, ExplicitRecoveryState::CleanShutdown);
    assert!(!context.state.requires_recovery());
    assert_eq!(context.authority, Authority::GraphFile);
    assert!(!context.authority.should_recover());

    // Assert: No recovery should be needed
    assert_eq!(context.state.severity(), sqlitegraph::backend::native::v2::wal::recovery::RecoverySeverity::Minimal);

    Ok(())
}

/// Test 2: Dirty WAL with uncommitted transactions - requires replay
#[test]
fn test_recovery_dirty_wal_replay() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let graph_path = temp_dir.path().join("test.v2");
    let wal_path = temp_dir.path().join("test.wal");
    let checkpoint_path = temp_dir.path().join("test.checkpoint");

    // Create a clean graph file
    let _graph_file = GraphFile::create(&graph_path)?;

    // Create WAL with dirty shutdown state
    let config = V2WALConfig {
        wal_path: wal_path.clone(),
        checkpoint_path: checkpoint_path.clone(),
        ..Default::default()
    };
    let manager = V2WALManager::create(config)?;

    // Write transaction but DO NOT commit (simulates crash)
    let tx_id = manager.begin_transaction(sqlitegraph::backend::native::v2::wal::TransactionIsolation::ReadCommitted)?;
    manager.write_transaction_record(tx_id, V2WALRecord::NodeInsert {
        node_id: 2,
        slot_offset: 0,
        node_data: vec![4, 5, 6],
    })?;

    // Simulate crash by dropping manager without commit
    drop(manager);

    // Analyze recovery context
    let context = RecoveryContext::analyze_files(&wal_path, &graph_path, &checkpoint_path)?;

    // Assert: Should detect dirty shutdown with uncommitted transaction
    assert_eq!(context.state, ExplicitRecoveryState::DirtyShutdown);
    assert!(context.state.requires_recovery());
    assert!(context.state.is_recoverable());
    assert_eq!(context.authority, Authority::WAL);
    assert!(context.authority.should_recover());

    // Assert: Recovery should be possible
    assert_eq!(context.state.severity(), sqlitegraph::backend::native::v2::wal::recovery::RecoverySeverity::Low);

    Ok(())
}

/// Test 3: Partial checkpoint resume
#[test]
fn test_recovery_partial_checkpoint_resume() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let graph_path = temp_dir.path().join("test.v2");
    let wal_path = temp_dir.path().join("test.wal");
    let checkpoint_path = temp_dir.path().join("test.checkpoint");

    // Create a clean graph file
    let _graph_file = GraphFile::create(&graph_path)?;

    // Create WAL
    let config = V2WALConfig {
        wal_path: wal_path.clone(),
        checkpoint_path: checkpoint_path.clone(),
        ..Default::default()
    };
    let manager = V2WALManager::create(config)?;

    // Write and commit transactions
    let tx1_id = manager.begin_transaction(sqlitegraph::backend::native::v2::wal::TransactionIsolation::ReadCommitted)?;
    manager.write_transaction_record(tx1_id, V2WALRecord::NodeInsert {
        node_id: 3,
        slot_offset: 0,
        node_data: vec![7, 8, 9],
    })?;
    manager.commit_transaction(tx1_id)?;

    // Write second transaction but don't checkpoint
    let tx2_id = manager.begin_transaction(sqlitegraph::backend::native::v2::wal::TransactionIsolation::ReadCommitted)?;
    manager.write_transaction_record(tx2_id, V2WALRecord::NodeInsert {
        node_id: 4,
        slot_offset: 0,
        node_data: vec![10, 11, 12],
    })?;
    manager.commit_transaction(tx2_id)?;

    // Create partial checkpoint (simulate interrupted checkpoint)
    // This will be simulated by creating a checkpoint file with incomplete data
    let _ = File::create(&checkpoint_path)?.write_all(b"partial_checkpoint");

    // Simulate crash by dropping manager
    drop(manager);

    // Analyze recovery context
    let context = RecoveryContext::analyze_files(&wal_path, &graph_path, &checkpoint_path)?;

    // Assert: Should detect partial checkpoint
    assert_eq!(context.state, ExplicitRecoveryState::PartialCheckpoint);
    assert!(context.state.requires_recovery());
    assert!(context.state.is_recoverable());
    assert_eq!(context.authority, Authority::WAL);
    assert!(context.authority.should_recover());

    // Assert: Medium severity recovery needed
    assert_eq!(context.state.severity(), sqlitegraph::backend::native::v2::wal::recovery::RecoverySeverity::Medium);

    Ok(())
}

/// Test 4: Uncommitted transaction rollback
#[test]
fn test_recovery_uncommitted_transaction_rollback() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let graph_path = temp_dir.path().join("test.v2");
    let wal_path = temp_dir.path().join("test.wal");
    let checkpoint_path = temp_dir.path().join("test.checkpoint");

    // Create a clean graph file
    let _graph_file = GraphFile::create(&graph_path)?;

    // Create WAL
    let config = V2WALConfig {
        wal_path: wal_path.clone(),
        checkpoint_path: checkpoint_path.clone(),
        ..Default::default()
    };
    let manager = V2WALManager::create(config)?;

    // Begin transaction and write records but ROLLBACK
    let tx_id = manager.begin_transaction(sqlitegraph::backend::native::v2::wal::TransactionIsolation::ReadCommitted)?;
    manager.write_transaction_record(tx_id, V2WALRecord::NodeInsert {
        node_id: 5,
        slot_offset: 0,
        node_data: vec![13, 14, 15],
    })?;
    manager.write_transaction_record(tx_id, V2WALRecord::ClusterCreate {
        node_id: 5,
        direction: sqlitegraph::backend::native::v2::Direction::Outgoing,
        cluster_offset: 100,
        cluster_size: 5,
        edge_data: vec![16, 17, 18],
    })?;

    // Explicit rollback
    manager.rollback_transaction(tx_id)?;

    // Simulate crash by dropping manager
    drop(manager);

    // Analyze recovery context
    let context = RecoveryContext::analyze_files(&wal_path, &graph_path, &checkpoint_path)?;

    // Assert: Should detect dirty shutdown but rollback should be handled
    assert_eq!(context.state, ExplicitRecoveryState::DirtyShutdown);
    assert!(context.state.requires_recovery());
    assert!(context.state.is_recoverable());
    assert_eq!(context.authority, Authority::WAL);
    assert!(context.authority.should_recover());

    Ok(())
}

/// Test 5: Corrupt WAL detection
#[test]
fn test_recovery_corrupt_wal_detection() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let graph_path = temp_dir.path().join("test.v2");
    let wal_path = temp_dir.path().join("test.wal");
    let checkpoint_path = temp_dir.path().join("test.checkpoint");

    // Create a clean graph file
    let _graph_file = GraphFile::create(&graph_path)?;

    // Create a corrupted WAL file (invalid magic bytes)
    let mut wal_file = File::create(&wal_path)?;
    wal_file.write_all(b"CORRUPT_WAL_DATA_NOT_VALID_HEADER")?;
    wal_file.sync_all()?;

    // Analyze recovery context
    let context = RecoveryContext::analyze_files(&wal_path, &graph_path, &checkpoint_path)?;

    // Assert: Should detect corrupt WAL
    assert_eq!(context.state, ExplicitRecoveryState::CorruptWAL);
    assert!(context.state.requires_recovery());
    assert!(!context.state.is_recoverable());
    assert_eq!(context.authority, Authority::Unrecoverable);
    assert!(!context.authority.should_recover());

    // Assert: Critical severity
    assert_eq!(context.state.severity(), sqlitegraph::backend::native::v2::wal::recovery::RecoverySeverity::Critical);

    Ok(())
}

/// Test 6: Authority resolution scenarios
#[test]
fn test_recovery_authority_resolution() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let graph_path = temp_dir.path().join("test.v2");
    let wal_path = temp_dir.path().join("test.wal");
    let checkpoint_path = temp_dir.path().join("test.checkpoint");

    // Scenario 1: Clean shutdown -> Graph file authority
    {
        let _graph_file = GraphFile::create(&graph_path)?;
        let config = V2WALConfig {
            wal_path: wal_path.clone(),
            checkpoint_path: checkpoint_path.clone(),
            ..Default::default()
        };
        let manager = V2WALManager::create(config)?;
        let tx_id = manager.begin_transaction(sqlitegraph::backend::native::v2::wal::TransactionIsolation::ReadCommitted)?;
        manager.write_transaction_record(tx_id, V2WALRecord::NodeInsert {
            node_id: 10,
            slot_offset: 0,
            node_data: vec![1, 2, 3],
        })?;
        manager.commit_transaction(tx_id)?;
        manager.force_checkpoint()?;
        drop(manager);

        let context = RecoveryContext::analyze_files(&wal_path, &graph_path, &checkpoint_path)?;
        assert_eq!(context.authority, Authority::GraphFile);
        assert!(!context.authority.should_recover());
    }

    // Scenario 2: Dirty shutdown -> WAL authority
    {
        let config = V2WALConfig {
            wal_path: wal_path.clone(),
            checkpoint_path: checkpoint_path.clone(),
            ..Default::default()
        };
        let manager = V2WALManager::create(config)?;
        let tx_id = manager.begin_transaction(sqlitegraph::backend::native::v2::wal::TransactionIsolation::ReadCommitted)?;
        manager.write_transaction_record(tx_id, V2WALRecord::NodeInsert {
            node_id: 11,
            slot_offset: 0,
            node_data: vec![4, 5, 6],
        })?;
        // Intentionally do not commit - simulate crash
        drop(manager);

        let context = RecoveryContext::analyze_files(&wal_path, &graph_path, &checkpoint_path)?;
        assert_eq!(context.authority, Authority::WAL);
        assert!(context.authority.should_recover());
    }

    // Scenario 3: Corrupt WAL -> Unrecoverable
    {
        let mut wal_file = File::create(&wal_path)?;
        wal_file.write_all(b"INVALID_MAGIC_AND_HEADER")?;
        wal_file.sync_all()?;

        let context = RecoveryContext::analyze_files(&wal_path, &graph_path, &checkpoint_path)?;
        assert_eq!(context.authority, Authority::Unrecoverable);
        assert!(!context.authority.should_recover());
    }

    Ok(())
}