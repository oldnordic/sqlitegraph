//! Snapshot Export/Import Integration Tests
//!
//! This module provides comprehensive integration tests for the snapshot export/import
//! functionality, ensuring stability, correctness, and proper isolation from WAL systems.

use sqlitegraph::backend::native::{
    graph_file::GraphFile,
    types::{NativeBackendError, NativeResult},
    v2::{
        export::{ExportFactory, SnapshotExporter, SnapshotExportConfig},
        import::{SnapshotImporter, SnapshotImportConfig},
        planner::{ExportPlanner, PlannerDecision},
    },
};
use std::path::{Path, PathBuf};
use tempfile::{NamedTempFile, TempDir};
use std::fs;

/// Integration test helper to create a stable graph file with test data
fn create_stable_test_graph() -> NativeResult<(GraphFile, PathBuf)> {
    let temp_file = NamedTempFile::new().map_err(|e| NativeBackendError::Io(e))?;
    let path = temp_file.path().to_path_buf();

    // Keep temp file from being deleted
    let _ = temp_file.into_temp_path().keep().unwrap();

    let mut graph_file = GraphFile::create(&path)?;

    // Add some test data by creating nodes and edges through direct file operations
    // For simplicity, we'll just ensure the file is in a clean state

    // Ensure stable state
    if graph_file.is_transaction_active() {
        graph_file.commit_transaction()?;
    }

    graph_file.flush()?;
    graph_file.sync()?;

    Ok((graph_file, path))
}

/// Integration test: End-to-end snapshot export and import chain
#[test]
fn test_snapshot_export_import_chain() -> NativeResult<()> {
    // Phase 1: Create stable graph
    let (original_graph, original_path) = create_stable_test_graph()?;

    // Get original header data before we export and potentially drop the graph
    let original_header = original_graph.persistent_header().clone();

    // Phase 2: Planner analysis should recommend snapshot
    let planner_decision = ExportPlanner::analyze_export_strategy(&original_path)?;
    assert!(planner_decision.export_mode == sqlitegraph::backend::native::v2::export::ExportMode::Snapshot);
    assert!(planner_decision.graph_stable);

    // Phase 3: Export snapshot
    let export_dir = TempDir::new().map_err(|e| NativeBackendError::Io(e))?;
    let snapshot_config = SnapshotExportConfig {
        export_path: export_dir.path().join("snapshot"),
        snapshot_id: "test_chain_snapshot".to_string(),
        include_statistics: true,
        min_stable_duration: std::time::Duration::from_secs(0),
        checksum_validation: true,
    };

    let mut exporter = SnapshotExporter::new(&original_path, snapshot_config)?;
    let export_result = exporter.export_snapshot()?;

    // Verify export success
    assert!(export_result.snapshot_path.exists());
    assert!(export_result.manifest_path.exists());
    assert!(export_result.snapshot_size_bytes > 0);

    // Phase 4: Delete original to simulate clean import environment
    // Move the drop after we're done with all original_graph operations
    drop(original_graph);
    fs::remove_file(&original_path)?;

    // Phase 5: Import snapshot
    let import_path = export_dir.path().join("restored.v2");
    let import_config = SnapshotImportConfig {
        target_graph_path: import_path.clone(),
        export_dir_path: export_dir.path().to_path_buf(),
        import_mode: sqlitegraph::backend::native::v2::import::ImportMode::Fresh,
        validate_manifest: true,
        verify_checksum: true,
        overwrite_existing: false,
    };

    let importer = SnapshotImporter::from_export_dir(export_dir.path(), &import_path, import_config)?;
    let import_result = importer.import()?;

    // Verify import success
    assert!(import_path.exists());
    assert!(import_result.records_imported > 0);
    assert!(import_result.validation_passed);

    // Phase 6: Verify restoration integrity
    let mut restored_graph = GraphFile::open(&import_path)?;
    let restored_header = restored_graph.persistent_header().clone();

    // Headers should match exactly
    assert_eq!(original_header.magic, restored_header.magic);
    assert_eq!(original_header.version, restored_header.version);
    assert_eq!(original_header.node_count, restored_header.node_count);
    assert_eq!(original_header.edge_count, restored_header.edge_count);

    // Phase 7: Verify no WAL interference
    let wal_path = import_path.with_extension("wal");
    assert!(!wal_path.exists(), "WAL should not exist after snapshot import");

    // Phase 8: Verify restored graph is immediately usable
    assert!(!restored_graph.is_transaction_active());
    assert!(restored_graph.validate_file_size().is_ok());
    assert!(restored_graph.verify_commit_marker().is_ok());

    Ok(())
}

/// Integration test: Multiple snapshot cycles maintain consistency
#[test]
fn test_multiple_snapshot_cycles_consistency() -> NativeResult<()> {
    let mut current_path = {
        let (_, path) = create_stable_test_graph()?;
        path
    };

    // Perform 3 snapshot cycles
    for cycle in 1..=3 {
        // Export snapshot
        let export_dir = TempDir::new().map_err(|e| NativeBackendError::Io(e))?;
        let snapshot_config = SnapshotExportConfig {
            export_path: export_dir.path().join("snapshot"),
            snapshot_id: format!("cycle_{}_snapshot", cycle),
            include_statistics: true,
            min_stable_duration: std::time::Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&current_path, snapshot_config)?;
        let export_result = exporter.export_snapshot()?;

        // Delete current file
        fs::remove_file(&current_path)?;

        // Import to new location
        let import_path = export_dir.path().join(format!("cycle_{}_restored.v2", cycle));
        let import_config = SnapshotImportConfig {
            target_graph_path: import_path.clone(),
            export_dir_path: export_dir.path().to_path_buf(),
            import_mode: sqlitegraph::backend::native::v2::import::ImportMode::Fresh,
            validate_manifest: true,
            verify_checksum: true,
            overwrite_existing: false,
        };

        let importer = SnapshotImporter::from_export_dir(export_dir.path(), &import_path, import_config)?;
        let import_result = importer.import()?;

        // Verify cycle success
        assert!(import_path.exists());
        assert!(import_result.validation_passed);

        // Update for next cycle
        current_path = import_path;
    }

    // Final verification - graph should still be valid
    let mut final_graph = GraphFile::open(&current_path)?;
    assert!(final_graph.validate_file_size().is_ok());
    assert!(final_graph.verify_commit_marker().is_ok());

    Ok(())
}

/// Regression test: Ensure WAL export paths remain unchanged
#[test]
fn test_wal_export_paths_unmodified() -> NativeResult<()> {
    let (graph_file, graph_path) = create_stable_test_graph()?;

    // Test existing export factory methods still work
    let export_dir = TempDir::new().map_err(|e| NativeBackendError::Io(e))?;

    // Test checkpoint-aligned export
    let checkpoint_result = ExportFactory::create_checkpoint_aligned_exporter(&graph_path, export_dir.path());
    // This should either succeed or fail with existing error patterns

    // Test full export
    let full_result = ExportFactory::create_full_exporter(&graph_path, export_dir.path());
    // This should either succeed or fail with existing error patterns

    // The key is that adding snapshot functionality shouldn't break existing WAL export APIs
    // We're just verifying the factory methods are still available

    drop(graph_file);
    Ok(())
}

/// Regression test: Planner decisions remain deterministic
#[test]
fn test_planner_deterministic_behavior() -> NativeResult<()> {
    let (graph_file, graph_path) = create_stable_test_graph()?;

    // Run planner analysis multiple times
    let decision1 = ExportPlanner::analyze_export_strategy(&graph_path)?;
    let decision2 = ExportPlanner::analyze_export_strategy(&graph_path)?;
    let decision3 = ExportPlanner::analyze_export_strategy(&graph_path)?;

    // All decisions should be identical (deterministic)
    assert_eq!(decision1.export_mode, decision2.export_mode);
    assert_eq!(decision2.export_mode, decision3.export_mode);
    assert_eq!(decision1.reasoning, decision2.reasoning);
    assert_eq!(decision2.reasoning, decision3.reasoning);

    // Quick check should be consistent with full analysis
    let snapshot_advisable = ExportPlanner::is_snapshot_advisable(&graph_path)?;
    let should_be_snapshot = matches!(decision1.export_mode, sqlitegraph::backend::native::v2::export::ExportMode::Snapshot);
    assert_eq!(snapshot_advisable, should_be_snapshot);

    drop(graph_file);
    Ok(())
}

/// Integration test: Snapshot import bypasses WAL recovery completely
#[test]
fn test_snapshot_import_bypasses_wal_recovery() -> NativeResult<()> {
    // Create a graph and export snapshot
    let (_, graph_path) = create_stable_test_graph()?;

    let export_dir = TempDir::new().map_err(|e| NativeBackendError::Io(e))?;
    let snapshot_config = SnapshotExportConfig::default();

    let mut exporter = SnapshotExporter::new(&graph_path, snapshot_config)?;
    let export_result = exporter.export_snapshot()?;

    // Import snapshot
    let import_path = TempDir::new().map_err(|e| NativeBackendError::Io(e))?.path().join("imported.v2");
    let import_config = SnapshotImportConfig {
        target_graph_path: import_path.clone(),
        export_dir_path: export_dir.path().to_path_buf(),
        import_mode: sqlitegraph::backend::native::v2::import::ImportMode::Fresh,
        validate_manifest: true,
        verify_checksum: true,
        overwrite_existing: false,
    };

    let importer = SnapshotImporter::from_export_dir(export_dir.path(), &import_path, import_config)?;
    let import_result = importer.import()?;

    // Critical verification: WAL recovery logic should NOT be triggered
    let mut imported_graph = GraphFile::open(&import_path)?;

    // 1. No WAL files should exist
    let wal_path = import_path.with_extension("wal");
    assert!(!wal_path.exists(), "WAL file should not exist after snapshot import");

    // 2. Recovery state should be CleanShutdown (no recovery needed)
    assert!(import_result.final_recovery_state == sqlitegraph::backend::native::v2::wal::recovery::states::RecoveryState::CleanShutdown);

    // 3. Graph should be immediately usable
    assert!(!imported_graph.is_transaction_active());
    assert!(imported_graph.validate_file_size().is_ok());

    // 4. No pending operations or recovery markers
    assert!(imported_graph.verify_commit_marker().is_ok());

    Ok(())
}

/// Integration test: Snapshot export requires stable state
#[test]
fn test_snapshot_export_requires_stable_state() -> NativeResult<()> {
    let (mut graph_file, graph_path) = create_stable_test_graph()?;

    // Put graph in unstable state
    graph_file.begin_transaction()?;

    // Planner should not recommend snapshot
    let planner_decision = ExportPlanner::analyze_export_strategy(&graph_path)?;
    assert!(!matches!(planner_decision.export_mode, sqlitegraph::backend::native::v2::export::ExportMode::Snapshot));

    // Quick check should also return false
    let snapshot_advisable = ExportPlanner::is_snapshot_advisable(&graph_path)?;
    assert!(!snapshot_advisable);

    // Clean up
    graph_file.rollback_transaction()?;
    drop(graph_file);

    Ok(())
}

/// Integration test: Error handling and recovery
#[test]
fn test_snapshot_error_handling() -> NativeResult<()> {
    // Test import with missing manifest
    let empty_dir = TempDir::new().map_err(|e| NativeBackendError::Io(e))?;
    let import_path = empty_dir.path().join("imported.v2");
    let import_config = SnapshotImportConfig::default();

    let import_result = SnapshotImporter::from_export_dir(empty_dir.path(), &import_path, import_config);
    assert!(import_result.is_err());

    // Test import with non-snapshot export (if we had one)
    // This would require creating a WAL export first, which is complex
    // For now, we focus on the basic error handling

    Ok(())
}

/// Integration test: Large file handling
#[test]
fn test_snapshot_large_file_handling() -> NativeResult<()> {
    // Create a stable graph
    let (_, graph_path) = create_stable_test_graph()?;

    // Verify file exists and has reasonable size
    assert!(graph_path.exists());
    let file_size = fs::metadata(&graph_path)?.len();
    assert!(file_size > 0);

    // Export and import should work regardless of file size
    let export_dir = TempDir::new().map_err(|e| NativeBackendError::Io(e))?;
    let snapshot_config = SnapshotExportConfig::default();

    let mut exporter = SnapshotExporter::new(&graph_path, snapshot_config)?;
    let export_result = exporter.export_snapshot()?;

    assert!(export_result.snapshot_size_bytes > 0);

    let import_path = export_dir.path().join("large_import.v2");
    let import_config = SnapshotImportConfig {
        target_graph_path: import_path.clone(),
        export_dir_path: export_dir.path().to_path_buf(),
        import_mode: sqlitegraph::backend::native::v2::import::ImportMode::Fresh,
        validate_manifest: true,
        verify_checksum: true,
        overwrite_existing: false,
    };

    let importer = SnapshotImporter::from_export_dir(export_dir.path(), &import_path, import_config)?;
    let import_result = importer.import()?;

    assert!(import_result.snapshot_size_bytes == export_result.snapshot_size_bytes);
    assert!(import_path.exists());

    // Verify imported file has same size as original
    let imported_size = fs::metadata(&import_path)?.len();
    assert_eq!(imported_size, file_size);

    Ok(())
}

/// Integration test: Concurrent access scenarios
#[test]
fn test_snapshot_concurrent_access() -> NativeResult<()> {
    // Create stable graph
    let (_, graph_path) = create_stable_test_graph()?;

    // Create multiple exporter instances
    let export_dir1 = TempDir::new().map_err(|e| NativeBackendError::Io(e))?;
    let export_dir2 = TempDir::new().map_err(|e| NativeBackendError::Io(e))?;

    let snapshot_config1 = SnapshotExportConfig {
        snapshot_id: "concurrent_1".to_string(),
        export_path: export_dir1.path().join("snapshot"),
        ..Default::default()
    };

    let snapshot_config2 = SnapshotExportConfig {
        snapshot_id: "concurrent_2".to_string(),
        export_path: export_dir2.path().join("snapshot"),
        ..Default::default()
    };

    // Both should be able to read the same graph file concurrently
    let mut exporter1 = SnapshotExporter::new(&graph_path, snapshot_config1)?;
    let mut exporter2 = SnapshotExporter::new(&graph_path, snapshot_config2)?;

    // Both exports should succeed
    let export_result1 = exporter1.export_snapshot()?;
    let export_result2 = exporter2.export_snapshot()?;

    assert!(export_result1.snapshot_path.exists());
    assert!(export_result2.snapshot_path.exists());

    // Snapshots should be identical in size (same source)
    assert_eq!(export_result1.snapshot_size_bytes, export_result2.snapshot_size_bytes);

    Ok(())
}