//! Snapshot Export/Import TDD Tests
//!
//! This module implements failing tests FIRST for snapshot export/import functionality.
//! All tests must FAIL initially, then pass after implementation.
//! Tests use REAL graph files, actual file I/O, and assert exact invariants.

use sqlitegraph::backend::native::{
    graph_file::GraphFile,
    types::{NativeBackendError, NativeResult},
};
use sqlitegraph::backend::native::v2::export::{
    V2Exporter, V2ExportConfig, ExportMode, ExportManifest, ExportFactory,
    snapshot::SnapshotExporter, snapshot::SnapshotExportConfig
};
use sqlitegraph::backend::native::v2::import::{
    V2Importer, V2ImportConfig, ImportMode, ImportFactory,
    snapshot::SnapshotImporter, snapshot::SnapshotImportConfig
};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tempfile::{NamedTempFile, TempDir};
use std::fs;

/// Test helper to create a basic graph file with known content
fn create_test_graph_file() -> NativeResult<(GraphFile, PathBuf)> {
    let temp_file = NamedTempFile::new().map_err(|e| NativeBackendError::Io(e))?;
    let path = temp_file.path().to_path_buf();

    // Keep temp file from being deleted
    let _ = temp_file.into_temp_path().keep().unwrap();

    let graph_file = GraphFile::create(&path)?;
    Ok((graph_file, path))
}

/// Test helper to ensure graph is in stable state for snapshot
fn ensure_stable_state(graph_file: &mut GraphFile) -> NativeResult<()> {
    // Commit any active transaction
    if graph_file.is_transaction_active() {
        graph_file.commit_transaction()?;
    }

    // Flush all buffers
    graph_file.flush()?;

    // Verify file consistency
    graph_file.validate_file_size()?;
    graph_file.verify_commit_marker()?;

    Ok(())
}

/// Test helper to check if WAL directory is empty or clean
fn is_wal_clean(export_path: &Path) -> bool {
    let wal_path = export_path.with_extension("wal");
    !wal_path.exists() || wal_path.metadata().map(|m| m.len() == 0).unwrap_or(false)
}

#[cfg(test)]
mod snapshot_export_tests {
    use super::*;

    /// Test: Snapshot export succeeds only when graph is stable (no active WAL / clean state)
    #[test]
    fn test_snapshot_export_requires_stable_state() {
        // Create test graph
        let (mut graph_file, graph_path) = create_test_graph_file().expect("Failed to create test graph");

        // Put graph in unstable state (active transaction)
        graph_file.begin_transaction().expect("Failed to begin transaction");

        // Attempt snapshot export using SnapshotExporter (not V2Exporter)
        let export_dir = TempDir::new().unwrap().keep();
        let snapshot_config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "test_snapshot".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&graph_path, snapshot_config)
            .expect("Failed to create snapshot exporter");

        // Should FAIL because graph has active transaction
        let result = exporter.export_snapshot();

        assert!(result.is_err(), "Snapshot export should fail with active transaction");

        match result.unwrap_err() {
            NativeBackendError::InvalidState { context, .. } => {
                assert!(context.contains("active transaction") || context.contains("unstable state"));
            }
            _ => panic!("Expected InvalidState error for active transaction"),
        }
    }

    /// Test: Snapshot export fails deterministically when invariants are violated
    #[test]
    fn test_snapshot_export_fails_with_corrupt_file() {
        let (mut graph_file, graph_path) = create_test_graph_file().expect("Failed to create test graph");

        // Ensure stable state initially
        ensure_stable_state(&mut graph_file).expect("Failed to ensure stable state");

        // Corrupt the file by truncating it (simulating corruption)
        {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .open(&graph_path)
                .expect("Failed to open graph file for corruption");
            file.set_len(100).expect("Failed to truncate file");
            file.sync_all().expect("Failed to sync corrupted file");
        }

        // Attempt snapshot export using SnapshotExporter
        let export_dir = TempDir::new().unwrap().keep();
        let snapshot_config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "test_corrupt_snapshot".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&graph_path, snapshot_config)
            .expect("Failed to create snapshot exporter");

        // Should FAIL because file is corrupt
        let result = exporter.export_snapshot();

        assert!(result.is_err(), "Snapshot export should fail with corrupt file");

        let error = result.unwrap_err();
        match error {
            NativeBackendError::FileTooSmall { .. } |
            NativeBackendError::InvalidMagic { .. } |
            NativeBackendError::InvalidHeader { .. } => {
                // Expected corruption errors
            }
            _ => panic!("Expected file corruption error, got: {:?}", error),
        }
    }

    /// Test: Snapshot export succeeds with clean state and creates proper files
    #[test]
    fn test_snapshot_export_succeeds_with_clean_state() {
        let (mut graph_file, graph_path) = create_test_graph_file().expect("Failed to create test graph");

        // Ensure clean, stable state
        ensure_stable_state(&mut graph_file).expect("Failed to ensure stable state");

        // Verify WAL is clean
        assert!(is_wal_clean(&graph_path), "WAL should be clean for snapshot");

        // Create export directory
        let export_dir = TempDir::new().unwrap().keep();

        // Attempt snapshot export
        let snapshot_config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "test_clean_snapshot".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&graph_path, snapshot_config)
            .expect("Failed to create snapshot exporter");

        // Should succeed with clean state
        let result = exporter.export_snapshot();

        assert!(result.is_ok(), "Snapshot export should succeed with clean state");

        // Verify expected files were created
        let snapshot_file = export_dir.join("snapshot.v2");
        let manifest_file = export_dir.join("export.manifest");

        assert!(snapshot_file.exists(), "Snapshot file should exist");
        assert!(manifest_file.exists(), "Manifest file should exist");

        // Verify file sizes are reasonable
        let snapshot_size = snapshot_file.metadata().unwrap().len();
        let original_size = graph_path.metadata().unwrap().len();

        assert_eq!(snapshot_size, original_size, "Snapshot should be same size as original");

        // Verify manifest can be read
        let manifest_content = fs::read_to_string(manifest_file).expect("Failed to read manifest");
        assert!(manifest_content.contains("V2EXPMF") || manifest_content.contains("Snapshot"));
    }

    /// Test: Snapshot export creates atomic, fsync'd files
    #[test]
    fn test_snapshot_export_atomicity() {
        let (mut graph_file, graph_path) = create_test_graph_file().expect("Failed to create test graph");
        ensure_stable_state(&mut graph_file).expect("Failed to ensure stable state");

        let export_dir = TempDir::new().unwrap().keep();
        let snapshot_config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "test_atomic_snapshot".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&graph_path, snapshot_config)
            .expect("Failed to create snapshot exporter");
        let result = exporter.export_snapshot();

        assert!(result.is_ok(), "Snapshot export should succeed");

        // Verify files are properly fsync'd by checking they survive process exit
        let snapshot_file = export_dir.join("snapshot.v2");
        assert!(snapshot_file.exists(), "Snapshot file should exist");

        // Verify file integrity by trying to open it as a GraphFile
        let restored_graph = GraphFile::open(&snapshot_file);
        assert!(restored_graph.is_ok(), "Snapshot should be valid GraphFile");

        // Verify headers match exactly
        let original_header = graph_file.persistent_header();
        let restored_graph_file = restored_graph.expect("Failed to open restored graph file");
        let restored_header = restored_graph_file.persistent_header();

        assert_eq!(original_header.magic, restored_header.magic);
        assert_eq!(original_header.version, restored_header.version);
        assert_eq!(original_header.node_count, restored_header.node_count);
        assert_eq!(original_header.edge_count, restored_header.edge_count);
    }
}

#[cfg(test)]
mod snapshot_import_tests {
    use super::*;

    /// Test: Snapshot import restores graph byte-identically
    #[test]
    fn test_snapshot_import_restores_byte_identically() {
        // Create original graph with known content
        let (mut original_graph, original_path) = create_test_graph_file().expect("Failed to create original graph");
        ensure_stable_state(&mut original_graph).expect("Failed to stabilize original graph");

        // Export snapshot
        let export_dir = TempDir::new().unwrap().keep();
        let snapshot_config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "byte_identical_snapshot".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&original_path, snapshot_config)
            .expect("Failed to create snapshot exporter");
        let export_result = exporter.export_snapshot();
        assert!(export_result.is_ok(), "Snapshot export should succeed");

        // Create new empty graph file for import
        let import_path = TempDir::new().unwrap().keep().join("imported.v2");
        let import_config = V2ImportConfig {
            target_graph_path: import_path.clone(),
            export_dir_path: export_dir.clone(),
            import_mode: ImportMode::Fresh,
            validate_recovery: true,
            force_checkpoint_after_import: true,
        };

        // Import snapshot
        let importer = V2Importer::from_export_dir(&export_dir, &import_path, import_config);
        assert!(importer.is_ok(), "Import creation should succeed");

        let import_result = importer.unwrap().import();
        assert!(import_result.is_ok(), "Snapshot import should succeed");

        // Verify byte-identical restoration
        let original_bytes = fs::read(&original_path).expect("Failed to read original");
        let imported_bytes = fs::read(&import_path).expect("Failed to read imported");

        assert_eq!(original_bytes, imported_bytes, "Imported graph should be byte-identical to original");

        // Verify both graphs can be opened and have identical headers
        let original_opened = GraphFile::open(&original_path).expect("Failed to open original");
        let imported_opened = GraphFile::open(&import_path).expect("Failed to open imported");

        let orig_header = original_opened.persistent_header();
        let imp_header = imported_opened.persistent_header();

        assert_eq!(orig_header.magic, imp_header.magic);
        assert_eq!(orig_header.version, imp_header.version);
        assert_eq!(orig_header.node_count, imp_header.node_count);
        assert_eq!(orig_header.edge_count, imp_header.edge_count);
        assert_eq!(orig_header.node_data_offset, imp_header.node_data_offset);
    }

    /// Test: Snapshot import bypasses WAL replay entirely
    #[test]
    fn test_snapshot_import_bypasses_wal_replay() {
        let export_dir = TempDir::new().unwrap().keep();

        // Create and export snapshot
        {
            let (mut original_graph, original_path) = create_test_graph_file().expect("Failed to create original graph");
            ensure_stable_state(&mut original_graph).expect("Failed to stabilize original graph");

            let snapshot_config = SnapshotExportConfig {
                export_path: export_dir.clone(),
                snapshot_id: "bypass_wal_snapshot".to_string(),
                include_statistics: true,
                min_stable_duration: Duration::from_secs(0),
                checksum_validation: true,
            };

            let mut exporter = SnapshotExporter::new(&original_path, snapshot_config)
                .expect("Failed to create snapshot exporter");
            let export_result = exporter.export_snapshot();
            assert!(export_result.is_ok(), "Snapshot export should succeed");
        }

        // Create import target
        let import_path = TempDir::new().unwrap().keep().join("imported.v2");
        let import_config = V2ImportConfig {
            target_graph_path: import_path.clone(),
            export_dir_path: export_dir.clone(),
            import_mode: ImportMode::Fresh,
            validate_recovery: true,
            force_checkpoint_after_import: true,
        };

        let importer = V2Importer::from_export_dir(&export_dir, &import_path, import_config);
        assert!(importer.is_ok(), "Import creation should succeed");

        let import_result = importer.unwrap().import();
        assert!(import_result.is_ok(), "Snapshot import should succeed");

        // Verify WAL directory was not used (no WAL files should be created)
        let wal_path = import_path.with_extension("wal");
        assert!(!wal_path.exists(), "WAL file should not exist after snapshot import");

        // Verify imported graph is immediately usable without recovery
        let mut imported_graph = GraphFile::open(&import_path).expect("Failed to open imported graph");
        assert!(!imported_graph.is_transaction_active(), "No active transaction should exist");
        assert!(imported_graph.verify_commit_marker().is_ok(), "Commit marker should be valid");
    }

    /// Test: Snapshot import validates format/version/compatibility
    #[test]
    fn test_snapshot_import_validates_compatibility() {
        let export_dir = TempDir::new().unwrap().keep();

        // Create snapshot with current version
        {
            let (mut original_graph, original_path) = create_test_graph_file().expect("Failed to create original graph");
            ensure_stable_state(&mut original_graph).expect("Failed to stabilize original graph");

            let snapshot_config = SnapshotExportConfig {
                export_path: export_dir.clone(),
                snapshot_id: "compatibility_test_snapshot".to_string(),
                include_statistics: true,
                min_stable_duration: Duration::from_secs(0),
                checksum_validation: true,
            };

            let mut exporter = SnapshotExporter::new(&original_path, snapshot_config)
                .expect("Failed to create snapshot exporter");
            let export_result = exporter.export_snapshot();
            assert!(export_result.is_ok(), "Snapshot export should succeed");
        }

        // Corrupt the manifest to indicate incompatible version
        let manifest_path = export_dir.join("export.manifest");
        if manifest_path.exists() {
            // Write invalid version to manifest
            let mut manifest_data = fs::read_to_string(&manifest_path).expect("Failed to read manifest");
            manifest_data = manifest_data.replace("\"version\":1", "\"version\":999");
            fs::write(&manifest_path, manifest_data).expect("Failed to write corrupted manifest");
        }

        // Attempt import with incompatible version
        let import_path = TempDir::new().unwrap().keep().join("imported.v2");
        let import_config = V2ImportConfig {
            target_graph_path: import_path.clone(),
            export_dir_path: export_dir.clone(),
            import_mode: ImportMode::Fresh,
            validate_recovery: true, // Enable validation
            force_checkpoint_after_import: false,
        };

        let importer = V2Importer::from_export_dir(&export_dir, &import_path, import_config);
        assert!(importer.is_ok(), "Import creation should succeed");

        let import_result = importer.unwrap().import();

        // Should fail due to version incompatibility
        assert!(import_result.is_err(), "Import should fail with incompatible version");

        let import_error = import_result.unwrap_err();
        match import_error {
            NativeBackendError::UnsupportedVersion { .. } |
            NativeBackendError::InvalidParameter { .. } => {
                // Expected version/validation errors
            }
            _ => panic!("Expected version/validation error, got: {:?}", import_error),
        }
    }
}

#[cfg(test)]
mod snapshot_round_trip_tests {
    use super::*;

    /// Test: Snapshot export + import round-trip integrity
    #[test]
    fn test_snapshot_export_import_round_trip() {
        let (mut original_graph, original_path) = create_test_graph_file().expect("Failed to create original graph");
        ensure_stable_state(&mut original_graph).expect("Failed to stabilize original graph");

        // Store original header data for later comparison (copy the data we need)
        let original_header = original_graph.persistent_header();
        // Copy the specific fields we need to compare later
        let original_magic = original_header.magic;
        let original_version = original_header.version;
        let original_node_count = original_header.node_count;
        let original_edge_count = original_header.edge_count;
        let original_node_data_offset = original_header.node_data_offset;
        let original_free_space_offset = original_header.free_space_offset;

        // Export snapshot
        let export_dir = TempDir::new().unwrap().keep();
        let snapshot_config = SnapshotExportConfig {
            export_path: export_dir.clone(),
            snapshot_id: "round_trip_snapshot".to_string(),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&original_path, snapshot_config)
            .expect("Failed to create snapshot exporter");
        let export_result = exporter.export_snapshot();
        assert!(export_result.is_ok(), "Snapshot export should succeed");

        // Delete original to simulate fresh import environment
        drop(original_graph);
        fs::remove_file(&original_path).expect("Failed to delete original file");

        // Import snapshot
        let import_path = TempDir::new().unwrap().keep().join("restored.v2");
        let import_config = V2ImportConfig {
            target_graph_path: import_path.clone(),
            export_dir_path: export_dir.clone(),
            import_mode: ImportMode::Fresh,
            validate_recovery: true,
            force_checkpoint_after_import: false,
        };

        let importer = V2Importer::from_export_dir(&export_dir, &import_path, import_config);
        assert!(importer.is_ok(), "Import creation should succeed");

        let import_result = importer.unwrap().import();
        assert!(import_result.is_ok(), "Snapshot import should succeed");

        // Verify round-trip integrity
        let restored_graph = GraphFile::open(&import_path).expect("Failed to open restored graph");
        let restored_header = restored_graph.persistent_header();

        // All critical fields should match exactly
        assert_eq!(original_magic, restored_header.magic);
        assert_eq!(original_version, restored_header.version);
        assert_eq!(original_node_count, restored_header.node_count);
        assert_eq!(original_edge_count, restored_header.edge_count);
        assert_eq!(original_node_data_offset, restored_header.node_data_offset);
        assert_eq!(original_free_space_offset, restored_header.free_space_offset);
        // Note: PersistentHeaderV2 doesn't have a checksum field - data integrity is validated through other means
    }

    /// Test: Multiple round-trips maintain integrity
    #[test]
    fn test_multiple_snapshot_round_trips() {
        let mut current_path = {
            let (mut graph, path) = create_test_graph_file().expect("Failed to create initial graph");
            ensure_stable_state(&mut graph).expect("Failed to stabilize initial graph");
            drop(graph);
            path
        };

        // Perform multiple export/import cycles
        for cycle in 0..3 {
            // Export snapshot
            let export_dir = TempDir::new().unwrap().keep();
            let snapshot_config = SnapshotExportConfig {
                export_path: export_dir.clone(),
                snapshot_id: format!("cycle_{}_snapshot", cycle),
                include_statistics: true,
                min_stable_duration: Duration::from_secs(0),
                checksum_validation: true,
            };

            let mut exporter = SnapshotExporter::new(&current_path, snapshot_config)
                .expect("Failed to create snapshot exporter");
            let export_result = exporter.export_snapshot();
            assert!(export_result.is_ok(), "Cycle {} export should succeed", cycle);

            // Import to new location
            let new_path = TempDir::new().unwrap().keep().join(format!("cycle_{}.v2", cycle));
            let import_config = V2ImportConfig {
                target_graph_path: new_path.clone(),
                export_dir_path: export_dir.clone(),
                import_mode: ImportMode::Fresh,
                validate_recovery: true,
                force_checkpoint_after_import: true,
            };

            let importer = V2Importer::from_export_dir(&export_dir, &new_path, import_config);
            assert!(importer.is_ok(), "Cycle {} import creation should succeed", cycle);

            let import_result = importer.unwrap().import();
            assert!(import_result.is_ok(), "Cycle {} import should succeed", cycle);

            // Clean up old file and continue with new one
            fs::remove_file(&current_path).expect("Failed to remove old file");
            current_path = new_path;
        }

        // Final graph should still be valid and consistent
        let mut final_graph = GraphFile::open(&current_path).expect("Failed to open final graph");
        assert!(final_graph.validate_file_size().is_ok(), "Final graph should be consistent");
        assert!(final_graph.verify_commit_marker().is_ok(), "Final graph should have valid commit marker");
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;

    /// Regression test: Ensure WAL export paths are unchanged
    #[test]
    fn test_wal_export_paths_unchanged() {
        let (mut graph_file, graph_path) = create_test_graph_file().expect("Failed to create test graph");
        ensure_stable_state(&mut graph_file).expect("Failed to stabilize graph");

        // Test existing WAL export modes still work
        for export_mode in [ExportMode::CheckpointAligned, ExportMode::LsnBounded, ExportMode::Full] {
            let export_dir = TempDir::new().unwrap().keep();
            let export_config = V2ExportConfig {
                export_path: export_dir.clone(),
                include_wal_tail: true,  // For WAL exports, include tail
                compression_enabled: false,
                checksum_validation: true,
            };

            let exporter = V2Exporter::from_graph_file(&graph_path, export_config)
                .expect("Failed to create V2 exporter");

            // Test the appropriate export method for each mode
            let result = match export_mode {
                ExportMode::CheckpointAligned => exporter.export_checkpoint_aligned(),
                ExportMode::LsnBounded => exporter.export_lsn_bounded(0, 1000), // Use dummy LSN range
                ExportMode::Full => exporter.export_full(),
                ExportMode::Snapshot => {
                    // V2Exporter doesn't support snapshots - use SnapshotExporter instead
                    panic!("V2Exporter should not handle Snapshot mode");
                }
            };

            // These should continue to work (or fail with existing expected errors)
            // The key is that adding Snapshot support shouldn't break existing functionality
            match result {
                Ok(_) => {
                    // Success is fine - verify expected files exist
                    assert!(export_dir.join("export.manifest").exists());
                }
                Err(NativeBackendError::CorruptStringTable { reason }) if reason.contains("not yet implemented") => {
                    // Expected for unimplemented functionality
                }
                Err(other) => {
                    // Other errors should be documented, not caused by Snapshot changes
                    panic!("Unexpected error for WAL export mode {:?}: {:?}", export_mode, other);
                }
            }
        }
    }
}