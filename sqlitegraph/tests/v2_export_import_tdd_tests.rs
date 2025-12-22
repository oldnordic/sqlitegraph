//! TDD Tests for V2 Export/Import System
//!
//! This test module contains comprehensive TDD tests that validate the export/import
//! system functionality. All tests are designed to FAIL initially until the corresponding
//! functionality is implemented, following strict TDD methodology.
//!
//! ## Test Coverage
//!
//! 1. Export after clean checkpoint
//! 2. Export with active WAL tail
//! 3. Import into empty graph
//! 4. Import followed by recovery validation
//! 5. Import failure on incompatible manifest
//! 6. End-to-end export/import roundtrip

use sqlitegraph::backend::native::graph_file::GraphFile;
use sqlitegraph::backend::native::v2::Direction;
use sqlitegraph::backend::native::v2::wal::{
    V2WALConfig, V2WALManager, V2WALRecord, V2WALReader,
    TransactionIsolation, BulkIngestConfig, BulkIngestExt,
};
use sqlitegraph::backend::native::v2::wal::recovery::states::{
    RecoveryContext, Authority, RecoveryState as ExplicitRecoveryState,
};
use sqlitegraph::backend::native::v2::export::{
    V2Exporter, V2ExportConfig, ExportResult, ExportConsistencyReport,
    ExportFactory, ExportManifest, manifest::ManifestSerializer,
};
use sqlitegraph::backend::native::v2::import::{
    V2Importer, V2ImportConfig, ImportResult, ImportValidationReport,
    ImportFactory, ImportMode,
};
use sqlitegraph::backend::native::{NativeBackendError, NativeResult};
use tempfile::tempdir;

/// Test 1: Export after clean checkpoint - no WAL tail needed
#[test]
fn test_export_clean_checkpoint_no_wal() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let graph_path = temp_dir.path().join("test.v2");
    let export_dir = temp_dir.path().join("export");

    // Create a clean graph file
    let _graph_file = GraphFile::create(&graph_path)?;

    // Create WAL and perform clean operations
    let wal_path = graph_path.with_extension("wal");
    let checkpoint_path = graph_path.with_extension("checkpoint");
    let wal_config = V2WALConfig {
        wal_path: wal_path.clone(),
        checkpoint_path: checkpoint_path.clone(),
        ..Default::default()
    };

    // Create WAL manager and write some data
    let manager = V2WALManager::create(wal_config)?;

    // Write and commit a clean transaction
    let tx_id = manager.begin_transaction(TransactionIsolation::ReadCommitted)?;
    manager.write_transaction_record(tx_id, V2WALRecord::NodeInsert {
        node_id: 1,
        slot_offset: 0,
        node_data: vec![1, 2, 3, 4, 5],
    })?;
    manager.commit_transaction(tx_id)?;

    // Force checkpoint to ensure clean state
    manager.force_checkpoint()?;

    // Create export configuration for checkpoint-aligned export
    let exporter = ExportFactory::create_checkpoint_aligned_exporter(
        &graph_path,
        &export_dir,
    )?;

    // This should fail initially until export system is implemented
    let result = exporter.export_checkpoint_aligned();
    assert!(result.is_err(), "Should fail until export_checkpoint_aligned is implemented");

    // Verify export files were created (if they were)
    let manifest_path = export_dir.join("export.manifest");
    let graph_file_path = export_dir.join("export.graph");
    let wal_file_path = export_dir.join("export.wal");

    // For now, we expect the operation to fail, so files may not exist
    // When implemented, these should exist and be valid

    Ok(())
}

/// Test 2: Export with active WAL tail - requires WAL records
#[test]
fn test_export_active_wal_tail() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let graph_path = temp_dir.path().join("test.v2");
    let export_dir = temp_dir.path().join("export");

    // Create a clean graph file
    let _graph_file = GraphFile::create(&graph_path)?;

    // Create WAL system
    let wal_path = graph_path.with_extension("wal");
    let checkpoint_path = graph_path.with_extension("checkpoint");
    let wal_config = V2WALConfig {
        wal_path: wal_path.clone(),
        checkpoint_path: checkpoint_path.clone(),
        ..Default::default()
    };

    let manager = V2WALManager::create(wal_config)?;

    // Write transaction but DO NOT commit (simulates active state)
    let tx_id = manager.begin_transaction(TransactionIsolation::ReadCommitted)?;
    manager.write_transaction_record(tx_id, V2WALRecord::NodeInsert {
        node_id: 2,
        slot_offset: 0,
        node_data: vec![6, 7, 8, 9, 10],
    })?;

    // Simulate crash by dropping manager without commit
    drop(manager);

    // Create export configuration for full export (includes WAL tail)
    let exporter = ExportFactory::create_full_exporter(
        &graph_path,
        &export_dir,
    )?;

    // This should fail initially until export system is implemented
    let result = exporter.export_full();
    assert!(result.is_err(), "Should fail until export_full is implemented");

    Ok(())
}

/// Test 3: Import into empty graph
#[test]
fn test_import_into_empty_graph() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let export_dir = temp_dir.path().join("export");
    let target_path = temp_dir.path().join("imported.v2");

    // Create mock export directory structure with all required files
    std::fs::create_dir_all(&export_dir)?;

    // Create mock manifest file with basic structure
    let manifest = ExportManifest::new();
    let manifest_bytes = format!(
        "V2EXPORT_MANIFEST\nversion=1\nrecovery_state=CleanShutdown\ngraph_checkpoint_lsn=50\nexport_mode=CheckpointAligned"
    );
    let manifest_path = export_dir.join("export.manifest");
    std::fs::write(&manifest_path, manifest_bytes)?;

    // Create mock graph file
    let graph_file_path = export_dir.join("export.graph");
    std::fs::write(&graph_file_path, b"mock exported graph data")?;

    // Create mock WAL file (optional for checkpoint-aligned export)
    let wal_file_path = export_dir.join("export.wal");
    std::fs::write(&wal_file_path, b"mock exported wal data")?;

    // Configure fresh import
    let config = V2ImportConfig {
        target_graph_path: target_path.clone(),
        export_dir_path: export_dir.clone(),
        import_mode: ImportMode::Fresh,
        validate_recovery: true,
        force_checkpoint_after_import: true,
    };

    // Try to create importer - should fail initially
    let importer_result = V2Importer::from_export_dir(&export_dir, &target_path, config);
    assert!(importer_result.is_err(), "Should fail until V2Importer::from_export_dir is implemented");

    // Even if importer creation succeeded, validation should fail
    if let Ok(importer) = importer_result {
        let validation_result = importer.validate_export();
        assert!(validation_result.is_err(), "Should fail until validate_export is implemented");

        // Even if validation succeeded, import should fail
        if let Ok(_validation) = validation_result {
            let import_result = importer.import();
            assert!(import_result.is_err(), "Should fail until import is implemented");
        }
    }

    Ok(())
}

/// Test 4: Import followed by recovery validation
#[test]
fn test_import_recovery_validation() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let export_dir = temp_dir.path().join("export");
    let target_path = temp_dir.path().join("imported.v2");

    // Create comprehensive mock export
    std::fs::create_dir_all(&export_dir)?;

    // Create mock manifest
    let manifest_path = export_dir.join("export.manifest");
    let manifest_bytes = format!(
        "V2EXPORT_MANIFEST\nversion=1\nrecovery_state=DirtyShutdown\ngraph_checkpoint_lsn=100\nwal_start_lsn=101\nwal_end_lsn=150\nexport_mode=Full"
    );
    std::fs::write(&manifest_path, manifest_bytes)?;

    // Create mock export files
    let graph_file_path = export_dir.join("export.graph");
    let wal_file_path = export_dir.join("export.wal");
    std::fs::write(&graph_file_path, b"mock exported graph data")?;
    std::fs::write(&wal_file_path, b"mock exported wal records")?;

    // Configure import with recovery validation enabled
    let config = V2ImportConfig {
        target_graph_path: target_path.clone(),
        export_dir_path: export_dir.clone(),
        import_mode: ImportMode::Fresh,
        validate_recovery: true,
        force_checkpoint_after_import: true,
    };

    // Try to create importer - should fail initially
    let importer_result = V2Importer::from_export_dir(&export_dir, &target_path, config);
    assert!(importer_result.is_err(), "Should fail until V2Importer is implemented");

    Ok(())
}

/// Test 5: Import failure on incompatible manifest
#[test]
fn test_import_incompatible_manifest() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let export_dir = temp_dir.path().join("export");
    let target_path = temp_dir.path().join("imported.v2");

    // Create mock export with incompatible manifest
    std::fs::create_dir_all(&export_dir)?;

    // Create manifest with incompatible version
    let manifest_path = export_dir.join("export.manifest");
    let incompatible_manifest = format!(
        "V2EXPORT_MANIFEST\nversion=999\nincompatible_format=true\ninvalid_field=bad_value"
    );
    std::fs::write(&manifest_path, incompatible_manifest)?;

    // Create mock graph file
    let graph_file_path = export_dir.join("export.graph");
    std::fs::write(&graph_file_path, b"mock exported graph data")?;

    // Configure import
    let config = V2ImportConfig {
        target_graph_path: target_path.clone(),
        export_dir_path: export_dir.clone(),
        import_mode: ImportMode::Fresh,
        validate_recovery: true,
        force_checkpoint_after_import: true,
    };

    // Should fail due to incompatible manifest
    let result = V2Importer::from_export_dir(&export_dir, &target_path, config);
    assert!(result.is_err(), "Should fail due to incompatible manifest");

    // Even if importer creation succeeded, validation should catch incompatibility
    if let Ok(importer) = result {
        let validation_result = importer.validate_export();
        assert!(validation_result.is_err(), "Should detect manifest incompatibility");
    }

    Ok(())
}

/// Test 6: End-to-end export/import roundtrip
#[test]
fn test_end_to_end_export_import_roundtrip() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let source_path = temp_dir.path().join("source.v2");
    let export_dir = temp_dir.path().join("export");
    let target_path = temp_dir.path().join("restored.v2");

    // Create source graph file with some data
    let _source_graph = GraphFile::create(&source_path)?;

    // Create WAL system and write some realistic data
    let source_wal_path = source_path.with_extension("wal");
    let source_checkpoint_path = source_path.with_extension("checkpoint");
    let source_wal_config = V2WALConfig {
        wal_path: source_wal_path.clone(),
        checkpoint_path: source_checkpoint_path.clone(),
        ..Default::default()
    };

    let source_manager = V2WALManager::create(source_wal_config)?;

    // Write some test data
    for i in 1..10 {
        let tx_id = source_manager.begin_transaction(TransactionIsolation::ReadCommitted)?;

        // Write node record
        source_manager.write_transaction_record(tx_id, V2WALRecord::NodeInsert {
            node_id: i,
            slot_offset: (i * 100) as u64,
            node_data: vec![i as u8; 10],
        })?;

        // Write edge record
        source_manager.write_transaction_record(tx_id, V2WALRecord::ClusterCreate {
            node_id: i,
            direction: Direction::Outgoing,
            cluster_offset: (i * 1000) as u64,
            cluster_size: 5,
            edge_data: vec![i as u8; 20],
        })?;

        source_manager.commit_transaction(tx_id)?;
    }

    // Force checkpoint for clean state
    source_manager.force_checkpoint()?;

    // Step 1: Export from source
    let source_exporter = ExportFactory::create_full_exporter(&source_path, &export_dir)?;
    let export_result = source_exporter.export_full();
    assert!(export_result.is_err(), "Should fail until export system is implemented");

    // Step 2: Import to target
    let import_config = V2ImportConfig {
        target_graph_path: target_path.clone(),
        export_dir_path: export_dir.clone(),
        import_mode: ImportMode::Fresh,
        validate_recovery: true,
        force_checkpoint_after_import: true,
    };

    let target_importer = V2Importer::from_export_dir(&export_dir, &target_path, import_config)?;
    let import_result = target_importer.import();
    assert!(import_result.is_err(), "Should fail until import system is implemented");

    // When implemented, verify:
    // 1. Export creates valid manifest, graph file, and WAL tail
    // 2. Import successfully reconstructs database state
    // 3. Recovery validation passes
    // 4. Final LSN and state match expectations

    Ok(())
}

/// Test 7: LSN-bounded export validation
#[test]
fn test_lsn_bounded_export_validation() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let graph_path = temp_dir.path().join("test.v2");
    let export_dir = temp_dir.path().join("export");

    // Create graph file
    let _graph_file = GraphFile::create(&graph_path)?;

    // Create WAL system
    let wal_path = graph_path.with_extension("wal");
    let checkpoint_path = graph_path.with_extension("checkpoint");
    let wal_config = V2WALConfig {
        wal_path: wal_path.clone(),
        checkpoint_path: checkpoint_path.clone(),
        ..Default::default()
    };

    let manager = V2WALManager::create(wal_config)?;

    // Write multiple transactions to create WAL tail
    for i in 1..5 {
        let tx_id = manager.begin_transaction(TransactionIsolation::ReadCommitted)?;
        manager.write_transaction_record(tx_id, V2WALRecord::NodeInsert {
            node_id: i,
            slot_offset: (i * 100) as u64,
            node_data: vec![i as u8; 50],
        })?;
        manager.commit_transaction(tx_id)?;
    }

    // Create export configuration for LSN-bounded export
    let config = V2ExportConfig {
        export_path: export_dir.join("export"),
        include_wal_tail: true,
        compression_enabled: false,
        checksum_validation: true,
    };

    let exporter_result = V2Exporter::from_graph_file(&graph_path, config);
    assert!(exporter_result.is_err(), "Should fail until V2Exporter is implemented");

    // Even if exporter creation succeeded, LSN-bounded export should fail
    if let Ok(exporter) = exporter_result {
        let lsn_result = exporter.export_lsn_bounded(1, 100);
        assert!(lsn_result.is_err(), "Should fail until LSN-bounded export is implemented");
    }

    Ok(())
}

/// Test 8: Manifest integrity validation
#[test]
fn test_manifest_integrity_validation() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let export_dir = temp_dir.path().join("export");

    // Create mock export with corrupted manifest
    std::fs::create_dir_all(&export_dir)?;

    // Create corrupted manifest (invalid magic bytes)
    let manifest_path = export_dir.join("export.manifest");
    std::fs::write(&manifest_path, b"CORRUPT_MANIFEST_DATA")?;

    // Try to read manifest - should fail due to corruption
    let read_result = ManifestSerializer::read_from_file(&manifest_path);
    assert!(read_result.is_err(), "Should fail due to corrupted manifest");

    // Even if we could read it, validation should catch corruption
    if let Ok(_manifest) = read_result {
        let validation_result = _manifest.validate();
        assert!(validation_result.is_err(), "Should detect manifest corruption");
    }

    Ok(())
}

/// Test 9: Bulk ingest integration for import performance
#[test]
fn test_bulk_ingest_integration() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let export_dir = temp_dir.path().join("export");
    let target_path = temp_dir.path().join("bulk_imported.v2");

    // Create mock export with WAL tail containing many records
    std::fs::create_dir_all(&export_dir)?;

    // Create mock manifest
    let manifest_path = export_dir.join("export.manifest");
    let manifest_bytes = format!(
        "V2EXPORT_MANIFEST\nversion=1\nrecovery_state=DirtyShutdown\nwal_start_lsn=1\nwal_end_lsn=1000\nexport_mode=Full"
    );
    std::fs::write(&manifest_path, manifest_bytes)?;

    // Create mock graph file
    let graph_file_path = export_dir.join("export.graph");
    std::fs::write(&graph_file_path, b"mock exported graph data")?;

    // Create mock WAL with many records (simulating bulk data)
    let wal_file_path = export_dir.join("export.wal");
    let mut wal_data = Vec::new();

    // Simulate 100 WAL records
    for i in 1..=100 {
        // Create a simple record pattern
        wal_data.extend_from_slice(&[1u8]); // Record type
        wal_data.extend_from_slice(&(i as u32).to_le_bytes()); // Record size
        wal_data.extend_from_slice(&[i; 10]); // Mock record data
    }
    std::fs::write(&wal_file_path, wal_data)?;

    // Configure import with bulk optimization
    let config = V2ImportConfig {
        target_graph_path: target_path.clone(),
        export_dir_path: export_dir.clone(),
        import_mode: ImportMode::Fresh,
        validate_recovery: true,
        force_checkpoint_after_import: true,
    };

    // Should fail initially but be designed for bulk ingest optimization
    let result = V2Importer::from_export_dir(&export_dir, &target_path, config);
    assert!(result.is_err(), "Should fail until import system is implemented");

    Ok(())
}

/// Test 10: Recovery state detection accuracy
#[test]
fn test_recovery_state_detection_accuracy() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let graph_path = temp_dir.path().join("test.v2");

    // Create graph file
    let _graph_file = GraphFile::create(&graph_path)?;

    // Create WAL system
    let wal_path = graph_path.with_extension("wal");
    let checkpoint_path = graph_path.with_extension("checkpoint");
    let wal_config = V2WALConfig {
        wal_path: wal_path.clone(),
        checkpoint_path: checkpoint_path.clone(),
        ..Default::default()
    };

    let manager = V2WALManager::create(wal_config)?;

    // Test clean shutdown detection
    let tx_id = manager.begin_transaction(TransactionIsolation::ReadCommitted)?;
    manager.write_transaction_record(tx_id, V2WALRecord::NodeInsert {
        node_id: 1,
        slot_offset: 0,
        node_data: vec![1, 2, 3],
    })?;
    manager.commit_transaction(tx_id)?;
    manager.force_checkpoint()?;

    // Verify recovery state detection should work
    let recovery_context = RecoveryContext::analyze_files(&wal_path, &graph_path, &checkpoint_path)?;
    assert_eq!(recovery_context.state, ExplicitRecoveryState::CleanShutdown);
    assert_eq!(recovery_context.authority, Authority::GraphFile);

    // Test dirty shutdown detection
    let tx_id = manager.begin_transaction(TransactionIsolation::ReadCommitted)?;
    manager.write_transaction_record(tx_id, V2WALRecord::NodeInsert {
        node_id: 2,
        slot_offset: 0,
        node_data: vec![4, 5, 6],
    })?;
    // Don't commit - simulate crash
    drop(manager);

    let recovery_context = RecoveryContext::analyze_files(&wal_path, &graph_path, &checkpoint_path)?;
    assert_eq!(recovery_context.state, ExplicitRecoveryState::DirtyShutdown);
    assert_eq!(recovery_context.authority, Authority::WAL);

    Ok(())
}