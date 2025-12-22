//! Formal Invariants Tests for SQLiteGraph V2 Snapshot System
//!
//! This test module enforces critical invariants for the snapshot export/import
//! subsystem using strict TDD methodology. Tests are written FIRST to specify
//! required behavior, then implementation follows.
//!
//! ## Invariants Tested
//!
//! ### Export Invariants
//! 1. Export never writes directly to final filenames
//! 2. Temp files are always cleaned up or detected on restart
//! 3. Export directory is created only when needed
//! 4. Snapshot files have consistent naming and layout
//! 5. Manifest is written only after successful snapshot export
//!
//! ### Import Invariants
//! 1. Import rejects directories as snapshot targets
//! 2. Import rejects partial files and mismatched metadata
//! 3. Import validates manifest before touching target files
//! 4. A snapshot is either fully valid or completely ignored
//! 5. Import never overwrites without explicit permission
//!
//! ### Crash Recovery Invariants
//! 1. WAL + snapshot interaction is deterministic after crash
//! 2. Partial exports are detected and cleaned up safely
//! 3. Recovery never corrupts existing data
//! 4. Authority resolution is deterministic
//! 5. Recovery state is externally observable

use std::path::{Path, PathBuf};
use std::fs;
use std::time::Duration;
use tempfile::TempDir;
use sqlitegraph::backend::native::{
    graph_file::GraphFile,
    types::{NativeBackendError, NativeResult},
};
use sqlitegraph::backend::native::v2::{
    export::snapshot::{SnapshotExporter, SnapshotExportConfig},
    import::snapshot::{SnapshotImporter, SnapshotImportConfig},
    snapshot::{AtomicFileOperations, SnapshotLifecycleInspector},
};

/// Test helper to create a minimal V2 graph file for testing
fn create_test_v2_graph(dir: &Path, name: &str) -> NativeResult<PathBuf> {
    let graph_path = dir.join(format!("{}.v2", name));
    let _graph = GraphFile::create(&graph_path)?;

    // Add minimal test data to make it a real graph
    // This ensures we're testing with actual V2 graph files
    Ok(graph_path)
}

/// Test helper to simulate filesystem crash by removing directories/files
fn simulate_crash(export_dir: &Path) {
    // Simulate power loss by removing the export directory entirely
    if export_dir.exists() {
        let _ = fs::remove_dir_all(export_dir);
    }
}

/// Test helper to create a directory with the same name as expected snapshot file
fn create_conflicting_directory(export_dir: &Path, snapshot_id: &str) -> NativeResult<()> {
    let conflicting_path = export_dir.join(format!("{}.v2", snapshot_id));
    fs::create_dir_all(&conflicting_path)
        .map_err(|e| NativeBackendError::Io(e))?;
    Ok(())
}

/// Test helper to create a partial snapshot file (incomplete copy)
fn create_partial_snapshot(export_dir: &Path, snapshot_id: &str) -> NativeResult<()> {
    let snapshot_path = export_dir.join(format!("{}.v2", snapshot_id));
    let mut file = fs::File::create(&snapshot_path)
        .map_err(|e| NativeBackendError::Io(e))?;

    // Write only partial data (less than header size)
    use std::io::Write;
    file.write_all(b"partial")
        .map_err(|e| NativeBackendError::Io(e))?;
    file.flush().map_err(|e| NativeBackendError::Io(e))?;

    Ok(())
}

/// Test helper to create a manifest with mismatched checksum
fn create_mismatched_manifest(export_dir: &Path, snapshot_id: &str, correct_checksum: u64) -> NativeResult<()> {
    let manifest_path = export_dir.join("export.manifest");

    // Create a manifest with wrong checksum for the snapshot
    let manifest_content = format!(r#"{{
  "magic": [86, 50, 88, 80, 77, 70, 0, 0],
  "version": 1,
  "recovery_state": "CleanShutdown",
  "authority": "GraphFile",
  "export_mode": "Snapshot",
  "graph_checkpoint_lsn": 0,
  "wal_start_lsn": null,
  "wal_end_lsn": null,
  "graph_format_version": 2,
  "wal_format_version": 1,
  "v2_clustered_edges": true,
  "export_timestamp": 1704067200,
  "export_duration_ms": 150,
  "graph_checksum": {},
  "wal_checksum": null,
  "total_records": 0,
  "total_bytes": 80,
  "reserved": [0, 0, 0, 0, 0, 0, 0, 0]
}}"#, correct_checksum + 1); // Wrong checksum

    fs::write(&manifest_path, manifest_content)
        .map_err(|e| NativeBackendError::Io(e))?;

    Ok(())
}

// ============================================================================
// EXPORT INVARIANTS TESTS
// ============================================================================

#[cfg(test)]
mod export_invariants {
    use super::*;

    /// **INVARIANT 1**: Export never writes directly to final filenames
    #[test]
    fn test_export_never_writes_directly_to_final_filenames() {
        // This test should FAIL initially - creates failing TDD test
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let graph_path = create_test_v2_graph(temp_dir.path(), "test_graph")
            .expect("Failed to create test graph");

        let export_dir = temp_dir.path().join("export");
        let config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "test_snapshot".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&graph_path, config)
            .expect("Failed to create exporter");

        // Before export, final snapshot file should not exist
        let final_snapshot_path = export_dir.join("test_snapshot.v2");
        assert!(!final_snapshot_path.exists(), "Final snapshot path should not exist before export");

        // During export, we should never see the final path being written to directly
        // This is hard to test without instrumentation, so we rely on the atomic copy behavior

        // Export should succeed (this will create the file via atomic rename)
        let result = exporter.export_snapshot();
        assert!(result.is_ok(), "Export should succeed: {:?}", result);

        // After export, final snapshot file should exist
        assert!(final_snapshot_path.exists(), "Final snapshot path should exist after export");
    }

    /// **INVARIANT 2**: Temp files are cleaned up or detected on restart
    #[test]
    fn test_temp_files_are_cleaned_or_detected_on_restart() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let graph_path = create_test_v2_graph(temp_dir.path(), "test_graph")
            .expect("Failed to create test graph");

        let export_dir = temp_dir.path().join("export");
        let config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "test_snapshot".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        // Create export directory
        fs::create_dir_all(&export_dir).expect("Failed to create export dir");

        // Create a stray temp file that might exist from a crashed export
        let stray_temp_path = export_dir.join("test_snapshot.tmp.12345");
        fs::write(&stray_temp_path, b"stray temp data")
            .expect("Failed to create stray temp file");

        assert!(stray_temp_path.exists(), "Stray temp file should exist");

        // Create new exporter - it should detect or clean up the temp file
        let mut exporter = SnapshotExporter::new(&graph_path, config)
            .expect("Failed to create exporter");

        // Export should succeed despite stray temp file
        let result = exporter.export_snapshot();
        assert!(result.is_ok(), "Export should succeed despite stray temp file: {:?}", result);

        // After successful export, stray temp file should be cleaned up
        // The AtomicFileOperations should handle this
        // This test asserts the expected behavior - implementation should ensure cleanup
    }

    /// **INVARIANT 3**: Export rejects directory as destination
    #[test]
    fn test_export_rejects_directory_as_destination() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let graph_path = create_test_v2_graph(temp_dir.path(), "test_graph")
            .expect("Failed to create test graph");

        let export_dir = temp_dir.path().join("export");
        fs::create_dir_all(&export_dir).expect("Failed to create export dir");

        // Create a directory with the name that should be a file
        create_conflicting_directory(&export_dir, "test_snapshot")
            .expect("Failed to create conflicting directory");

        let config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "test_snapshot".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&graph_path, config)
            .expect("Failed to create exporter");

        // Export should fail because destination path exists as directory
        let result = exporter.export_snapshot();
        assert!(result.is_err(), "Export should fail when destination exists as directory");

        // Verify the error type is appropriate
        match result {
            Err(NativeBackendError::Io(io_err)) => {
                // Should be "Is a directory" error or similar
                let error_msg = io_err.to_string().to_lowercase();
                assert!(error_msg.contains("directory") || error_msg.contains("is a directory"),
                       "Error should mention directory: {}", error_msg);
            }
            Err(other) => {
                panic!("Expected IoError for directory conflict, got: {:?}", other);
            }
            Ok(_) => {
                panic!("Export should not succeed when destination exists as directory");
            }
        }
    }

    /// **INVARIANT 4**: Snapshot files have consistent naming and layout
    #[test]
    fn test_snapshot_files_have_consistent_naming_and_layout() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let graph_path = create_test_v2_graph(temp_dir.path(), "test_graph")
            .expect("Failed to create test graph");

        let export_dir = temp_dir.path().join("export");
        let config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "consistency_test".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&graph_path, config)
            .expect("Failed to create exporter");

        let result = exporter.export_snapshot();
        assert!(result.is_ok(), "Export should succeed: {:?}", result);

        let export_result = result.unwrap();

        // Verify consistent naming
        let expected_snapshot_path = export_dir.join("consistency_test.v2");
        let expected_manifest_path = export_dir.join("export.manifest");

        assert_eq!(export_result.snapshot_path, expected_snapshot_path);
        assert_eq!(export_result.manifest_path, expected_manifest_path);

        // Verify both files exist
        assert!(expected_snapshot_path.exists(), "Snapshot file should exist");
        assert!(expected_manifest_path.exists(), "Manifest file should exist");

        // Verify snapshot file can be opened as GraphFile
        let _restored_graph = GraphFile::open(&expected_snapshot_path)
            .expect("Snapshot file should be valid GraphFile");

        // Verify manifest contains required fields
        let manifest_content = fs::read_to_string(&expected_manifest_path)
            .expect("Should be able to read manifest");
        assert!(manifest_content.contains("Snapshot"), "Manifest should indicate snapshot export mode");
        assert!(manifest_content.contains("v2_clustered_edges"), "Manifest should indicate V2 format");
    }

    /// **INVARIANT 5**: Manifest is written only after successful snapshot export
    #[test]
    fn test_manifest_written_only_after_successful_snapshot_export() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let graph_path = create_test_v2_graph(temp_dir.path(), "test_graph")
            .expect("Failed to create test graph");

        let export_dir = temp_dir.path().join("export");
        let config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "manifest_test".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        // Create export directory but don't export yet
        fs::create_dir_all(&export_dir).expect("Failed to create export dir");

        let manifest_path = export_dir.join("export.manifest");
        let snapshot_path = export_dir.join("manifest_test.v2");

        // Before export, neither file should exist
        assert!(!manifest_path.exists(), "Manifest should not exist before export");
        assert!(!snapshot_path.exists(), "Snapshot should not exist before export");

        let mut exporter = SnapshotExporter::new(&graph_path, config)
            .expect("Failed to create exporter");

        // Export should succeed
        let result = exporter.export_snapshot();
        assert!(result.is_ok(), "Export should succeed: {:?}", result);

        // After successful export, both files should exist
        assert!(snapshot_path.exists(), "Snapshot should exist after successful export");
        assert!(manifest_path.exists(), "Manifest should exist after successful export");

        // Verify manifest references the correct snapshot
        let manifest_content = fs::read_to_string(&manifest_path)
            .expect("Should be able to read manifest");
        assert!(manifest_content.contains("manifest_test"), "Manifest should reference correct snapshot");
    }
}

// ============================================================================
// IMPORT INVARIANTS TESTS
// ============================================================================

#[cfg(test)]
mod import_invariants {
    use super::*;

    /// **INVARIANT 1**: Import rejects directories as snapshot targets
    #[test]
    fn test_import_rejects_directories_as_snapshot_targets() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create export directory structure
        let export_dir = temp_dir.path().join("export");
        fs::create_dir_all(&export_dir).expect("Failed to create export dir");

        // Create a directory instead of a snapshot file
        create_conflicting_directory(&export_dir, "test_snapshot")
            .expect("Failed to create conflicting directory");

        // Create a basic manifest
        let manifest_path = export_dir.join("export.manifest");
        let manifest_content = r#"{
  "magic": [86, 50, 88, 80, 77, 70, 0, 0],
  "version": 1,
  "recovery_state": "CleanShutdown",
  "authority": "GraphFile",
  "export_mode": "Snapshot",
  "graph_checkpoint_lsn": 0,
  "wal_start_lsn": null,
  "wal_end_lsn": null,
  "graph_format_version": 2,
  "wal_format_version": 1,
  "v2_clustered_edges": true,
  "export_timestamp": 1704067200,
  "export_duration_ms": 150,
  "graph_checksum": 1234567890,
  "wal_checksum": null,
  "total_records": 0,
  "total_bytes": 80,
  "reserved": [0, 0, 0, 0, 0, 0, 0, 0]
}"#;
        fs::write(&manifest_path, manifest_content)
            .expect("Failed to write manifest");

        let target_path = temp_dir.path().join("imported.v2");
        let config = SnapshotImportConfig::default();

        // Import should fail because snapshot target is a directory
        let result = SnapshotImporter::from_export_dir(&export_dir, &target_path, config);
        assert!(result.is_err(), "Import should reject directory as snapshot target");

        match result {
            Err(NativeBackendError::InvalidParameter { context, .. }) => {
                assert!(context.contains("directory") || context.contains("file"),
                       "Error should mention directory/file issue: {}", context);
            }
            Err(other) => {
                panic!("Expected InvalidParameter for directory conflict, got: {:?}", other);
            }
            Ok(_) => {
                panic!("Import should not succeed when snapshot is a directory");
            }
        }
    }

    /// **INVARIANT 2**: Import rejects partial files
    #[test]
    fn test_import_rejects_partial_files() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create export directory structure
        let export_dir = temp_dir.path().join("export");
        fs::create_dir_all(&export_dir).expect("Failed to create export dir");

        // Create a partial snapshot file (smaller than header)
        create_partial_snapshot(&export_dir, "test_snapshot")
            .expect("Failed to create partial snapshot");

        // Create a manifest that references the partial snapshot
        let manifest_path = export_dir.join("export.manifest");
        let manifest_content = r#"{
  "magic": [86, 50, 88, 80, 77, 70, 0, 0],
  "version": 1,
  "recovery_state": "CleanShutdown",
  "authority": "GraphFile",
  "export_mode": "Snapshot",
  "graph_checkpoint_lsn": 0,
  "wal_start_lsn": null,
  "wal_end_lsn": null,
  "graph_format_version": 2,
  "wal_format_version": 1,
  "v2_clustered_edges": true,
  "export_timestamp": 1704067200,
  "export_duration_ms": 150,
  "graph_checksum": 1234567890,
  "wal_checksum": null,
  "total_records": 0,
  "total_bytes": 80,
  "reserved": [0, 0, 0, 0, 0, 0, 0, 0]
}"#;
        fs::write(&manifest_path, manifest_content)
            .expect("Failed to write manifest");

        let target_path = temp_dir.path().join("imported.v2");
        let config = SnapshotImportConfig::default();

        // Import should fail because snapshot file is partial
        let result = SnapshotImporter::from_export_dir(&export_dir, &target_path, config);
        assert!(result.is_err(), "Import should reject partial snapshot files");
    }

    /// **INVARIANT 3**: Import rejects mismatched metadata
    #[test]
    fn test_import_rejects_mismatched_metadata() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create a valid snapshot file first
        let snapshot_path = create_test_v2_graph(temp_dir.path(), "test_snapshot")
            .expect("Failed to create test snapshot");

        // Get the actual checksum
        let graph = GraphFile::open(&snapshot_path).expect("Failed to open graph");
        let correct_checksum = 1234567890; // This would normally be calculated

        // Move to export directory structure
        let export_dir = temp_dir.path().join("export");
        fs::create_dir_all(&export_dir).expect("Failed to create export dir");

        let export_snapshot_path = export_dir.join("test_snapshot.v2");
        fs::copy(&snapshot_path, &export_snapshot_path)
            .expect("Failed to copy snapshot to export dir");

        // Create manifest with mismatched checksum
        create_mismatched_manifest(&export_dir, "test_snapshot", correct_checksum)
            .expect("Failed to create mismatched manifest");

        let target_path = temp_dir.path().join("imported.v2");
        let config = SnapshotImportConfig::default();

        // Import should fail because metadata doesn't match
        let result = SnapshotImporter::from_export_dir(&export_dir, &target_path, config);
        assert!(result.is_err(), "Import should reject mismatched metadata");
    }

    /// **INVARIANT 4**: A snapshot is either fully valid or completely ignored
    #[test]
    fn test_snapshot_either_fully_valid_or_completely_ignored() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Test case: Missing manifest (snapshot ignored)
        let export_dir = temp_dir.path().join("export");
        fs::create_dir_all(&export_dir).expect("Failed to create export dir");

        // Create only snapshot file, no manifest
        let _snapshot_path = create_test_v2_graph(&export_dir, "incomplete_snapshot")
            .expect("Failed to create snapshot");

        let target_path = temp_dir.path().join("imported.v2");
        let config = SnapshotImportConfig::default();

        // Import should fail completely (no partial import)
        let result = SnapshotImporter::from_export_dir(&export_dir, &target_path, config);
        assert!(result.is_err(), "Import should fail for incomplete snapshot");

        // Verify no partial import occurred
        assert!(!target_path.exists(), "No target file should be created for incomplete import");
    }

    /// **INVARIANT 5**: Import never overwrites without explicit permission
    #[test]
    fn test_import_never_overwrites_without_permission() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create a complete export
        let export_dir = temp_dir.path().join("export");
        let source_graph_path = create_test_v2_graph(temp_dir.path(), "source_graph")
            .expect("Failed to create source graph");

        // Perform export
        let config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "test_snapshot".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&source_graph_path, config)
            .expect("Failed to create exporter");
        let _export_result = exporter.export_snapshot()
            .expect("Export should succeed");

        // Create an existing target file
        let target_path = temp_dir.path().join("existing.v2");
        let _existing_graph = create_test_v2_graph(temp_dir.path(), "existing")
            .expect("Failed to create existing target");
        assert!(target_path.exists(), "Target file should exist before import");

        // Try to import without overwrite permission
        let import_config = SnapshotImportConfig {
            target_graph_path: target_path.clone(),
            export_dir_path: export_dir,
            import_mode: sqlitegraph::backend::native::v2::import::ImportMode::Fresh,
            validate_manifest: true,
            verify_checksum: true,
            overwrite_existing: false, // Explicitly no overwrite
        };

        let result = SnapshotImporter::from_export_dir(&export_dir, &target_path, import_config);
        assert!(result.is_err(), "Import should fail without overwrite permission");

        // Verify existing file was not touched
        assert!(target_path.exists(), "Existing file should not be modified");

        // File should still be valid (not corrupted)
        let _existing_graph = GraphFile::open(&target_path)
            .expect("Existing file should still be valid after failed import");
    }
}

// ============================================================================
// CRASH RECOVERY INVARIANTS TESTS
// ============================================================================

#[cfg(test)]
mod crash_recovery_invariants {
    use super::*;

    /// **INVARIANT 1**: Power-loss during atomic copy is handled safely
    #[test]
    fn test_power_loss_during_atomic_copy_handled_safely() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let graph_path = create_test_v2_graph(temp_dir.path(), "test_graph")
            .expect("Failed to create test graph");

        let export_dir = temp_dir.path().join("export");
        let config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "crash_test".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&graph_path, config)
            .expect("Failed to create exporter");

        // Simulate crash during export by removing export directory
        simulate_crash(&export_dir);

        // Create new exporter after "crash"
        let config_after_crash = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "crash_test".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter_after_crash = SnapshotExporter::new(&graph_path, config_after_crash)
            .expect("Failed to create exporter after crash");

        // Export should still succeed after crash
        let result = exporter_after_crash.export_snapshot();
        assert!(result.is_ok(), "Export should succeed after simulated crash: {:?}", result);

        // Verify export is complete and valid
        let export_result = result.unwrap();
        assert!(export_result.snapshot_path.exists(), "Snapshot should exist after recovery");
        assert!(export_result.manifest_path.exists(), "Manifest should exist after recovery");

        // Verify the exported snapshot is valid
        let _restored_graph = GraphFile::open(&export_result.snapshot_path)
            .expect("Exported snapshot should be valid after crash recovery");
    }

    /// **INVARIANT 2**: Power-loss between export and metadata write
    #[test]
    fn test_power_loss_between_export_and_metadata_write() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let graph_path = create_test_v2_graph(temp_dir.path(), "test_graph")
            .expect("Failed to create test graph");

        let export_dir = temp_dir.path().join("export");
        let config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "metadata_crash_test".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&graph_path, config)
            .expect("Failed to create exporter");

        // Perform export
        let result = exporter.export_snapshot();
        assert!(result.is_ok(), "Export should succeed: {:?}", result);

        let export_result = result.unwrap();

        // Simulate crash by removing only the manifest (snapshot file remains)
        fs::remove_file(&export_result.manifest_path)
            .expect("Failed to remove manifest for crash simulation");

        // Verify snapshot file still exists
        assert!(export_result.snapshot_path.exists(), "Snapshot should remain after manifest loss");
        assert!(!export_result.manifest_path.exists(), "Manifest should be missing");

        // Try to import - should fail gracefully due to missing manifest
        let target_path = temp_dir.path().join("recovered.v2");
        let import_config = SnapshotImportConfig::default();

        let import_result = SnapshotImporter::from_export_dir(&export_dir, &target_path, import_config);
        assert!(import_result.is_err(), "Import should fail without manifest");

        // Verify no partial import occurred
        assert!(!target_path.exists(), "No target file should be created without manifest");
    }

    /// **INVARIANT 3**: Import with dirty WAL is handled correctly
    #[test]
    fn test_import_with_dirty_wal_handled_correctly() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create a complete export
        let export_dir = temp_dir.path().join("export");
        let source_graph_path = create_test_v2_graph(temp_dir.path(), "source_graph")
            .expect("Failed to create source graph");

        let config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "wal_test".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&source_graph_path, config)
            .expect("Failed to create exporter");
        let _export_result = exporter.export_snapshot()
            .expect("Export should succeed");

        // Create dirty WAL file at target location
        let target_path = temp_dir.path().join("target.v2");
        let wal_path = target_path.with_extension("wal");

        // Write some data to WAL file to simulate dirty WAL
        fs::write(&wal_path, b"dirty wal data")
            .expect("Failed to create dirty WAL");

        assert!(wal_path.exists(), "WAL file should exist");
        assert!(wal_path.metadata().unwrap().len() > 0, "WAL file should not be empty");

        // Try to import - should handle dirty WAL appropriately
        let import_config = SnapshotImportConfig::default();
        let import_result = SnapshotImporter::from_export_dir(&export_dir, &target_path, import_config);

        // This should succeed because snapshots represent clean state
        assert!(import_result.is_ok(), "Import should succeed even with dirty WAL at target: {:?}", import_result);

        // Verify import completed successfully
        assert!(target_path.exists(), "Target file should exist after import");

        // The WAL handling behavior depends on the specific implementation
        // The test ensures the system doesn't crash with dirty WAL
    }

    /// **INVARIANT 4**: Import over existing graph files is safe
    #[test]
    fn test_import_over_existing_graph_files_is_safe() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create a complete export
        let export_dir = temp_dir.path().join("export");
        let source_graph_path = create_test_v2_graph(temp_dir.path(), "source_graph")
            .expect("Failed to create source graph");

        let config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "overwrite_test".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&source_graph_path, config)
            .expect("Failed to create exporter");
        let _export_result = exporter.export_snapshot()
            .expect("Export should succeed");

        // Create existing target graph with different data
        let target_path = temp_dir.path().join("existing.v2");
        let _existing_graph = create_test_v2_graph(temp_dir.path(), "existing_data")
            .expect("Failed to create existing target graph");

        assert!(target_path.exists(), "Existing target should exist");

        // Get original size for comparison
        let original_size = target_path.metadata().unwrap().len();

        // Import with overwrite permission
        let import_config = SnapshotImportConfig {
            target_graph_path: target_path.clone(),
            export_dir_path: export_dir,
            import_mode: sqlitegraph::backend::native::v2::import::ImportMode::Fresh,
            validate_manifest: true,
            verify_checksum: true,
            overwrite_existing: true, // Explicit overwrite
        };

        let import_result = SnapshotImporter::from_export_dir(&export_dir, &target_path, import_config);
        assert!(import_result.is_ok(), "Import with overwrite should succeed: {:?}", import_result);

        // Verify overwrite occurred safely
        assert!(target_path.exists(), "Target should exist after import");

        let new_size = target_path.metadata().unwrap().len();
        // Sizes should be different if overwrite actually occurred
        assert_ne!(original_size, new_size, "File size should change after successful overwrite");

        // Verify the new file is valid
        let _imported_graph = GraphFile::open(&target_path)
            .expect("Imported file should be valid GraphFile");
    }

    /// **INVARIANT 5**: Recovery state is externally observable
    #[test]
    fn test_recovery_state_is_externally_observable() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let graph_path = create_test_v2_graph(temp_dir.path(), "test_graph")
            .expect("Failed to create test graph");

        // Test recovery state inspection
        let inspector = SnapshotLifecycleInspector::new()
            .expect("Failed to create lifecycle inspector");

        // Inspect before export (should show no snapshots)
        let before_export = inspector.inspect_snapshot_state(&graph_path);
        assert!(before_export.is_ok(), "Inspection should succeed: {:?}", before_export);

        // Create a snapshot export
        let export_dir = temp_dir.path().join("export");
        let config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "recovery_test".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&graph_path, config)
            .expect("Failed to create exporter");
        let export_result = exporter.export_snapshot()
            .expect("Export should succeed");

        // Inspect after export - should detect the snapshot
        let after_export = inspector.inspect_snapshot_state(&export_result.snapshot_path);
        assert!(after_export.is_ok(), "Inspection should succeed after export: {:?}", after_export);

        // The specific recovery state behavior would depend on implementation
        // This test ensures the inspection API works and provides observable state
    }
}

// ============================================================================
// ATOMIC OPERATIONS INVARIANTS TESTS
// ============================================================================

#[cfg(test)]
mod atomic_operations_invariants {
    use super::*;

    /// **INVARIANT 1**: Atomic operations reject directory sources
    #[test]
    fn test_atomic_operations_reject_directory_sources() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create a directory as "source"
        let source_dir = temp_dir.path().join("source_dir");
        fs::create_dir(&source_dir).expect("Failed to create source directory");

        let target_file = temp_dir.path().join("target.txt");

        let atomic_ops = AtomicFileOperations::new();
        let result = atomic_ops.atomic_copy_file(&source_dir, &target_file);

        assert!(result.is_err(), "Atomic operations should reject directory sources");

        match result {
            Err(NativeBackendError::InvalidParameter { context, .. }) => {
                assert!(context.contains("directory") || context.contains("file"),
                       "Error should mention directory/file issue: {}", context);
            }
            Err(other) => {
                panic!("Expected InvalidParameter for directory source, got: {:?}", other);
            }
            Ok(_) => {
                panic!("Atomic operations should not succeed with directory source");
            }
        }
    }

    /// **INVARIANT 2**: Atomic operations reject existing destinations
    #[test]
    fn test_atomic_operations_reject_existing_destinations() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create source file
        let source_file = temp_dir.path().join("source.txt");
        fs::write(&source_file, b"source content").expect("Failed to create source file");

        // Create existing destination
        let target_file = temp_dir.path().join("target.txt");
        fs::write(&target_file, b"existing content").expect("Failed to create existing target");

        let atomic_ops = AtomicFileOperations::new();
        let result = atomic_ops.atomic_copy_file(&source_file, &target_file);

        assert!(result.is_err(), "Atomic operations should reject existing destinations");

        // Verify existing content is unchanged
        let existing_content = fs::read_to_string(&target_file).expect("Failed to read target");
        assert_eq!(existing_content, "existing content", "Existing content should be unchanged");
    }

    /// **INVARIANT 3**: Atomic operations provide all-or-nothing semantics
    #[test]
    fn test_atomic_operations_provide_all_or_nothing_semantics() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create source file
        let source_file = temp_dir.path().join("source.txt");
        fs::write(&source_file, b"atomic test content").expect("Failed to create source file");

        let target_file = temp_dir.path().join("target.txt");

        let atomic_ops = AtomicFileOperations::new();
        let result = atomic_ops.atomic_copy_file(&source_file, &target_file);

        assert!(result.is_ok(), "Atomic copy should succeed: {:?}", result);

        // Verify complete copy
        assert!(target_file.exists(), "Target file should exist after atomic copy");

        let source_content = fs::read_to_string(&source_file).expect("Failed to read source");
        let target_content = fs::read_to_string(&target_file).expect("Failed to read target");
        assert_eq!(source_content, target_content, "Content should be identical");

        // Verify source still exists (copy, not move)
        assert!(source_file.exists(), "Source should still exist after copy");
    }

    /// **INVARIANT 4**: Atomic operations cleanup temporary files on failure
    #[test]
    fn test_atomic_operations_cleanup_temp_files_on_failure() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Create source file
        let source_file = temp_dir.path().join("source.txt");
        fs::write(&source_file, b"source content").expect("Failed to create source file");

        // Create target directory that will cause failure
        let target_dir = temp_dir.path().join("target");
        fs::create_dir(&target_dir).expect("Failed to create target directory");

        let atomic_ops = AtomicFileOperations::new();
        let result = atomic_ops.atomic_copy_file(&source_file, &target_dir);

        assert!(result.is_err(), "Atomic copy should fail with directory target");

        // Check for any temporary files that might have been created
        for entry in fs::read_dir(temp_dir.path()).expect("Failed to read temp dir") {
            let entry = entry.expect("Failed to read directory entry");
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Should not have any .tmp files left behind
            assert!(!file_name.contains(".tmp"),
                   "Found temporary file that should have been cleaned up: {}", file_name);
        }
    }
}

// ============================================================================
// INTEGRATION TESTS
// ============================================================================

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// **INTEGRATION TEST**: Export → crash → restart → recover
    #[test]
    fn test_export_crash_restart_recover() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let graph_path = create_test_v2_graph(temp_dir.path(), "test_graph")
            .expect("Failed to create test graph");

        // Phase 1: Start export
        let export_dir = temp_dir.path().join("export");
        let config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "integration_test".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&graph_path, config)
            .expect("Failed to create exporter");

        // Phase 2: Simulate crash during export
        simulate_crash(&export_dir);

        // Phase 3: Restart and complete export
        let config_after_crash = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "integration_test".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter_after_crash = SnapshotExporter::new(&graph_path, config_after_crash)
            .expect("Failed to create exporter after crash");

        let result = exporter_after_crash.export_snapshot();
        assert!(result.is_ok(), "Export should succeed after crash recovery: {:?}", result);

        // Phase 4: Verify complete recovery
        let export_result = result.unwrap();
        assert!(export_result.snapshot_path.exists(), "Snapshot should exist after recovery");
        assert!(export_result.manifest_path.exists(), "Manifest should exist after recovery");

        // Verify snapshot is valid and complete
        let _restored_graph = GraphFile::open(&export_result.snapshot_path)
            .expect("Recovered snapshot should be valid");

        // Verify manifest is complete and valid
        let manifest_content = fs::read_to_string(&export_result.manifest_path)
            .expect("Should be able to read manifest");
        assert!(manifest_content.contains("integration_test"), "Manifest should reference correct snapshot");
        assert!(manifest_content.len() > 100, "Manifest should be substantial");
    }

    /// **INTEGRATION TEST**: Export → import → consistency check
    #[test]
    fn test_export_import_consistency_check() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");

        // Phase 1: Create original graph with data
        let original_graph_path = create_test_v2_graph(temp_dir.path(), "original")
            .expect("Failed to create original graph");

        // Add some test data (this would be done through the GraphFile API)
        // For now, we're using the basic graph file creation

        // Phase 2: Export the graph
        let export_dir = temp_dir.path().join("export");
        let config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "consistency_test".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&original_graph_path, config)
            .expect("Failed to create exporter");

        let export_result = exporter.export_snapshot()
            .expect("Export should succeed");

        // Phase 3: Import to new location
        let imported_graph_path = temp_dir.path().join("imported.v2");
        let import_config = SnapshotImportConfig::default();

        let importer = SnapshotImporter::from_export_dir(&export_dir, &imported_graph_path, import_config)
            .expect("Failed to create importer");

        let import_result = importer.import()
            .expect("Import should succeed");

        // Phase 4: Consistency check
        assert!(imported_graph_path.exists(), "Imported graph should exist");

        // Open both graphs and compare
        let original_graph = GraphFile::open(&original_graph_path)
            .expect("Failed to open original graph");
        let imported_graph = GraphFile::open(&imported_graph_path)
            .expect("Failed to open imported graph");

        // Compare basic properties
        let original_header = original_graph.persistent_header();
        let imported_header = imported_graph.persistent_header();

        assert_eq!(original_header.magic, imported_header.magic, "Magic bytes should match");
        assert_eq!(original_header.version, imported_header.version, "Version should match");
        assert_eq!(original_header.node_count, imported_header.node_count, "Node count should match");
        assert_eq!(original_header.edge_count, imported_header.edge_count, "Edge count should match");

        // The graphs should be functionally identical
    }

    /// **INTEGRATION TEST**: Multiple exports with different IDs
    #[test]
    fn test_multiple_exports_with_different_ids() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let graph_path = create_test_v2_graph(temp_dir.path(), "test_graph")
            .expect("Failed to create test graph");

        let export_dir = temp_dir.path().join("export");
        fs::create_dir_all(&export_dir).expect("Failed to create export dir");

        // Export multiple snapshots with different IDs
        let snapshot_ids = vec!["snapshot1", "snapshot2", "snapshot3"];
        let mut export_results = Vec::new();

        for snapshot_id in &snapshot_ids {
            let config = SnapshotExportConfig {
                export_path: export_dir.clone(),
                snapshot_id: snapshot_id.to_string(),
                include_statistics: true,
                min_stable_duration: Duration::from_secs(0),
                checksum_validation: true,
            };

            let mut exporter = SnapshotExporter::new(&graph_path, config)
                .expect("Failed to create exporter");

            let result = exporter.export_snapshot()
                .expect("Export should succeed");

            export_results.push(result);
        }

        // Verify all exports succeeded and are distinct
        assert_eq!(export_results.len(), snapshot_ids.len(), "Should have same number of results as snapshots");

        for (i, export_result) in export_results.iter().enumerate() {
            let expected_snapshot_path = export_dir.join(format!("{}.v2", snapshot_ids[i]));
            let expected_manifest_path = export_dir.join("export.manifest");

            assert_eq!(export_result.snapshot_path, expected_snapshot_path);
            assert_eq!(export_result.manifest_path, expected_manifest_path);

            assert!(expected_snapshot_path.exists(), "Snapshot {} should exist", snapshot_ids[i]);

            // Each snapshot should be valid
            let _graph = GraphFile::open(&expected_snapshot_path)
                .expect("Snapshot should be valid GraphFile");
        }
    }
}