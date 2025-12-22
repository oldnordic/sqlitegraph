//! V2 Snapshot Import for SQLiteGraph
//!
//! This module implements instant snapshot import functionality that bypasses WAL complexity
//! and provides direct database restoration through atomic file operations.
//!
//! ## Architecture
//!
//! Snapshot import diverges from WAL-based imports by:
//! 1. NO WAL replay or recovery orchestration
//! 2. Direct atomic snapshot file restoration
//! 3. Strict manifest validation before import
//! 4. Immediate database availability (no recovery needed)
//! 5. WAL directory must be empty or ignored

use super::ImportMode;
use crate::backend::native::{
    graph_file::GraphFile,
    types::{NativeBackendError, NativeResult},
    v2::export::{ExportManifest, ManifestSerializer},
    v2::wal::recovery::states::{Authority, RecoveryState as ExplicitRecoveryState},
};
use std::path::{Path, PathBuf};
use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Configuration for snapshot import operations
#[derive(Debug, Clone)]
pub struct SnapshotImportConfig {
    /// Target graph file path
    pub target_graph_path: PathBuf,

    /// Export directory containing snapshot
    pub export_dir_path: PathBuf,

    /// Import mode (Fresh only supported for snapshots)
    pub import_mode: ImportMode,

    /// Whether to validate manifest strictly
    pub validate_manifest: bool,

    /// Whether to perform checksum verification
    pub verify_checksum: bool,

    /// Whether to overwrite existing target file
    pub overwrite_existing: bool,
}

impl Default for SnapshotImportConfig {
    fn default() -> Self {
        Self {
            target_graph_path: PathBuf::from("imported.v2"),
            export_dir_path: PathBuf::from("snapshot_export"),
            import_mode: ImportMode::Fresh,
            validate_manifest: true,
            verify_checksum: true,
            overwrite_existing: false,
        }
    }
}

/// Snapshot import validation report
#[derive(Debug, Clone)]
pub struct SnapshotImportValidationReport {
    /// Whether manifest is valid and compatible
    pub manifest_valid: bool,

    /// Whether snapshot file exists and is readable
    pub snapshot_accessible: bool,

    /// Whether format versions are compatible
    pub format_compatible: bool,

    /// Whether target is ready for import
    pub target_ready: bool,

    /// Validation warnings
    pub warnings: Vec<String>,

    /// Validation errors
    pub errors: Vec<String>,
}

/// Snapshot import result
#[derive(Debug, Clone)]
pub struct SnapshotImportResult {
    /// Number of records imported
    pub records_imported: u64,

    /// Import duration
    pub import_duration: Duration,

    /// Size of imported snapshot in bytes
    pub snapshot_size_bytes: u64,

    /// Calculated checksum of imported file
    pub imported_checksum: u64,

    /// Whether validation passed
    pub validation_passed: bool,

    /// Final recovery state (should be CleanShutdown for snapshots)
    pub final_recovery_state: ExplicitRecoveryState,
}

/// Snapshot importer for direct database restoration
pub struct SnapshotImporter {
    /// Import configuration
    config: SnapshotImportConfig,

    /// Export manifest
    manifest: ExportManifest,

    /// Snapshot file path
    snapshot_path: PathBuf,
}

impl SnapshotImporter {
    /// Create snapshot importer from export directory
    pub fn from_export_dir(
        export_dir: &Path,
        _target_path: &Path,
        config: SnapshotImportConfig,
    ) -> NativeResult<Self> {
        // Validate export directory exists
        if !export_dir.exists() {
            return Err(NativeBackendError::InvalidParameter {
                context: format!("Export directory does not exist: {:?}", export_dir),
                source: None,
            });
        }

        // Read manifest file
        let manifest_path = export_dir.join("export.manifest");
        if !manifest_path.exists() {
            return Err(NativeBackendError::InvalidParameter {
                context: format!("Export manifest not found: {:?}", manifest_path),
                source: None,
            });
        }

        let manifest = ManifestSerializer::read_from_file(&manifest_path)?;

        // Validate this is a snapshot export
        if manifest.export_mode != crate::backend::native::v2::export::ExportMode::Snapshot {
            return Err(NativeBackendError::InvalidParameter {
                context: format!("Export is not a snapshot: {:?}", manifest.export_mode),
                source: None,
            });
        }

        // Find snapshot file (look for .v2 files)
        let snapshot_path = Self::find_snapshot_file(export_dir)?;

        Ok(Self {
            config,
            manifest,
            snapshot_path,
        })
    }

    /// Validate import before performing operation
    pub fn validate_import(&self) -> NativeResult<SnapshotImportValidationReport> {
        let mut report = SnapshotImportValidationReport {
            manifest_valid: true,
            snapshot_accessible: true,
            format_compatible: true,
            target_ready: true,
            warnings: Vec::new(),
            errors: Vec::new(),
        };

        // Validate manifest integrity and compatibility
        report.manifest_valid = self.validate_manifest(&mut report.warnings, &mut report.errors);

        // Validate snapshot file accessibility
        report.snapshot_accessible = self.validate_snapshot_file(&mut report.warnings, &mut report.errors);

        // Validate format compatibility
        report.format_compatible = self.validate_format_compatibility(&mut report.warnings, &mut report.errors);

        // Validate target readiness
        report.target_ready = self.validate_target(&mut report.warnings, &mut report.errors);

        Ok(report)
    }

    /// Perform snapshot import
    pub fn import(&self) -> NativeResult<SnapshotImportResult> {
        let start_time = SystemTime::now();

        // Step 1: Pre-import validation
        let validation_report = self.validate_import()?;
        if !validation_report.manifest_valid || !validation_report.snapshot_accessible {
            return Err(NativeBackendError::InvalidParameter {
                context: "Snapshot validation failed".to_string(),
                source: None,
            });
        }

        // Step 2: Ensure target directory exists
        if let Some(parent) = self.config.target_graph_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| NativeBackendError::Io(e))?;
        }

        // Step 3: Validate target doesn't exist (unless overwrite allowed)
        if self.config.target_graph_path.exists() && !self.config.overwrite_existing {
            return Err(NativeBackendError::InvalidParameter {
                context: format!("Target file exists and overwrite disabled: {:?}", self.config.target_graph_path),
                source: None,
            });
        }

        // Step 4: Perform atomic snapshot copy
        self.atomic_snapshot_restore()?;

        // Step 5: Verify import integrity
        let imported_checksum = if self.config.verify_checksum {
            self.calculate_imported_checksum()?
        } else {
            0
        };

        // Step 6: Validate imported file can be opened
        let imported_graph = GraphFile::open(&self.config.target_graph_path)?;
        let records_imported = imported_graph.persistent_header().node_count as u64 +
                              imported_graph.persistent_header().edge_count as u64;

        let import_duration = start_time.elapsed().unwrap_or_default();

        Ok(SnapshotImportResult {
            records_imported,
            import_duration,
            snapshot_size_bytes: self.manifest.total_bytes,
            imported_checksum,
            validation_passed: validation_report.manifest_valid && validation_report.snapshot_accessible,
            final_recovery_state: ExplicitRecoveryState::CleanShutdown,
        })
    }

    /// Validate manifest integrity and snapshot compatibility
    fn validate_manifest(&self, warnings: &mut Vec<String>, errors: &mut Vec<String>) -> bool {
        // Check magic bytes
        if self.manifest.magic != crate::backend::native::v2::export::manifest::ExportManifest::MAGIC {
            errors.push("Invalid manifest magic bytes".to_string());
            return false;
        }

        // Check version
        if self.manifest.version != crate::backend::native::v2::export::manifest::ExportManifest::VERSION {
            errors.push(format!("Unsupported manifest version: {}", self.manifest.version));
            return false;
        }

        // Validate export mode is Snapshot
        if self.manifest.export_mode != crate::backend::native::v2::export::ExportMode::Snapshot {
            errors.push(format!("Expected Snapshot export, got: {:?}", self.manifest.export_mode));
            return false;
        }

        // Validate V2 clustered edges
        if !self.manifest.v2_clustered_edges {
            errors.push("Import requires V2 clustered edge format support".to_string());
            return false;
        }

        // Validate authority is GraphFile (expected for snapshots)
        if self.manifest.authority != Authority::GraphFile {
            warnings.push(format!("Unexpected authority: {:?} (expected GraphFile for snapshots)", self.manifest.authority));
        }

        // Validate no WAL LSNs (should be None for snapshots)
        if self.manifest.wal_start_lsn.is_some() || self.manifest.wal_end_lsn.is_some() {
            errors.push("Snapshot should not contain WAL LSN information".to_string());
            return false;
        }

        true
    }

    /// Validate snapshot file exists and is readable
    fn validate_snapshot_file(&self, warnings: &mut Vec<String>, errors: &mut Vec<String>) -> bool {
        // Check snapshot file exists
        if !self.snapshot_path.exists() {
            errors.push(format!("Snapshot file not found: {:?}", self.snapshot_path));
            return false;
        }

        // Check file size matches manifest
        let file_size = fs::metadata(&self.snapshot_path)
            .map_err(|e| {
                errors.push(format!("Failed to read snapshot metadata: {}", e));
                NativeBackendError::Io(e)
            })
            .map(|m| m.len())
            .unwrap_or(0);

        if file_size != self.manifest.total_bytes {
            warnings.push(format!(
                "Snapshot size mismatch: manifest says {} bytes, file is {} bytes",
                self.manifest.total_bytes, file_size
            ));
        }

        // Try to open as GraphFile to validate format
        match GraphFile::open(&self.snapshot_path) {
            Ok(_) => true,
            Err(e) => {
                errors.push(format!("Snapshot file is not valid GraphFile: {}", e));
                false
            }
        }
    }

    /// Validate format compatibility
    fn validate_format_compatibility(&self, warnings: &mut Vec<String>, errors: &mut Vec<String>) -> bool {
        // Check V2 format support
        if self.manifest.graph_format_version != 2 {
            errors.push(format!("Unsupported graph format version: {}", self.manifest.graph_format_version));
            return false;
        }

        // Validate timestamp is reasonable (not in future, not too old)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        if self.manifest.export_timestamp > now {
            warnings.push("Export timestamp is in the future".to_string());
        }

        // Check for reasonable snapshot age (optional)
        let age_seconds = now.saturating_sub(self.manifest.export_timestamp);
        if age_seconds > 365 * 24 * 60 * 60 { // 1 year
            warnings.push("Snapshot is very old (over 1 year)".to_string());
        }

        true
    }

    /// Validate target readiness for import
    fn validate_target(&self, warnings: &mut Vec<String>, errors: &mut Vec<String>) -> bool {
        // For Fresh import, target should not exist unless overwrite is enabled
        if self.config.import_mode == ImportMode::Fresh {
            if self.config.target_graph_path.exists() {
                if self.config.overwrite_existing {
                    warnings.push("Target exists and will be overwritten".to_string());
                } else {
                    errors.push("Target exists but overwrite is disabled".to_string());
                    return false;
                }
            }
        }

        // Validate target path has proper .v2 extension
        if let Some(extension) = self.config.target_graph_path.extension() {
            if extension != "v2" {
                warnings.push("Target file should have .v2 extension".to_string());
            }
        } else {
            warnings.push("Target file missing extension (should be .v2)".to_string());
        }

        true
    }

    /// Find snapshot file in export directory
    fn find_snapshot_file(export_dir: &Path) -> NativeResult<PathBuf> {
        // Look for .v2 files in the export directory
        let entries = fs::read_dir(export_dir)
            .map_err(|e| NativeBackendError::Io(e))?;

        for entry in entries {
            let entry = entry.map_err(|e| NativeBackendError::Io(e))?;
            let path = entry.path();

            if let Some(extension) = path.extension() {
                if extension == "v2" && path.is_file() {
                    return Ok(path);
                }
            }
        }

        Err(NativeBackendError::InvalidParameter {
            context: format!("No .v2 snapshot file found in export directory: {:?}", export_dir),
            source: None,
        })
    }

    /// Perform atomic snapshot file restoration
    fn atomic_snapshot_restore(&self) -> NativeResult<()> {
        // Use temporary file during copy to ensure atomicity
        let temp_path = self.config.target_graph_path.with_extension("tmp");

        // Copy snapshot to temporary location
        fs::copy(&self.snapshot_path, &temp_path)
            .map_err(|e| NativeBackendError::Io(e))?;

        // Sync temporary file to ensure data is durable
        {
            let temp_file = fs::OpenOptions::new()
                .write(true)
                .open(&temp_path)
                .map_err(|e| NativeBackendError::Io(e))?;
            temp_file.sync_all().map_err(|e| NativeBackendError::Io(e))?;
        }

        // Atomic rename to final destination
        fs::rename(&temp_path, &self.config.target_graph_path)
            .map_err(|e| NativeBackendError::Io(e))?;

        // Sync parent directory to make rename durable
        if let Some(parent) = self.config.target_graph_path.parent() {
            let parent_dir = fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(parent)
                .map_err(|e| NativeBackendError::Io(e))?;
            parent_dir.sync_all().map_err(|e| NativeBackendError::Io(e))?;
        }

        Ok(())
    }

    /// Calculate checksum of imported file
    fn calculate_imported_checksum(&self) -> NativeResult<u64> {
        use std::io::Read;

        let mut file = fs::File::open(&self.config.target_graph_path)
            .map_err(|e| NativeBackendError::Io(e))?;

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let mut buffer = [0u8; 8192];

        loop {
            let bytes_read = file.read(&mut buffer)
                .map_err(|e| NativeBackendError::Io(e))?;

            if bytes_read == 0 {
                break;
            }

            std::hash::Hasher::write(&mut hasher, &buffer[..bytes_read]);
        }

        Ok(std::hash::Hasher::finish(&hasher))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{NamedTempFile, TempDir};
    use crate::backend::native::v2::export::{SnapshotExporter, SnapshotExportConfig};

    fn create_test_snapshot() -> NativeResult<(TempDir, ExportManifest)> {
        let export_dir = TempDir::new().map_err(|e| NativeBackendError::Io(e))?;

        // Create a test graph file with a unique name to avoid path conflicts
        let graph_path = export_dir.path().join("source_graph.v2");
        let _graph = GraphFile::create(&graph_path)?;

        // Create snapshot export with explicit export path and unique snapshot ID
        let export_path = export_dir.path().to_path_buf();
        println!("Export path: {:?}", export_path);
        println!("Graph path: {:?}", graph_path);

        let config = SnapshotExportConfig {
            export_path: export_path.clone(),
            snapshot_id: "snapshot_12345".to_string(), // Use unique ID
            include_statistics: true,
            min_stable_duration: std::time::Duration::from_secs(0),
            checksum_validation: true,
        };
        let mut exporter = SnapshotExporter::new(&graph_path, config)?;
        let result = match exporter.export_snapshot() {
            Ok(result) => {
                println!("Export succeeded: {:?}", result);
                result
            }
            Err(e) => {
                println!("Export failed: {:?}", e);
                return Err(e);
            }
        };

        // Read the generated manifest
        let manifest_path = export_dir.path().join("export.manifest");

        // Debug: Check what files were actually created
        println!("Looking for manifest at: {:?}", manifest_path);
        if manifest_path.exists() {
            if manifest_path.is_file() {
                println!("Manifest file exists and is a file");
            } else {
                println!("Manifest path exists but is a directory!");
            }
        } else {
            println!("Manifest file does not exist!");
            // List what files do exist
            match std::fs::read_dir(export_dir.path()) {
                Ok(entries) => {
                    println!("Files in export directory:");
                    for entry in entries {
                        if let Ok(entry) = entry {
                            println!("  {:?}", entry.path());
                        }
                    }
                }
                Err(e) => println!("Cannot read export directory: {:?}", e),
            }
        }

        let manifest = ManifestSerializer::read_from_file(&manifest_path)?;

        Ok((export_dir, manifest))
    }

    #[test]
    fn test_snapshot_importer_creation() {
        match create_test_snapshot() {
            Ok((export_dir, manifest)) => {
                println!("Test snapshot created successfully in: {:?}", export_dir.path());
                println!("Manifest: {:?}", manifest);

                let target_path = export_dir.path().join("imported.v2");
                let config = SnapshotImportConfig::default();

                match SnapshotImporter::from_export_dir(export_dir.path(), &target_path, config) {
                    Ok(_) => println!("Snapshot importer created successfully"),
                    Err(e) => {
                        println!("Snapshot importer creation failed: {:?}", e);
                        panic!("Snapshot importer creation should succeed, but got error: {:?}", e);
                    }
                }
            }
            Err(e) => {
                println!("Failed to create test snapshot: {:?}", e);
                panic!("Failed to create test snapshot: {:?}", e);
            }
        }
    }

    #[test]
    fn test_snapshot_importer_fails_with_missing_export_dir() {
        let missing_dir = PathBuf::from("/nonexistent/export");
        let target_path = PathBuf::from("imported.v2");
        let config = SnapshotImportConfig::default();

        let result = SnapshotImporter::from_export_dir(&missing_dir, &target_path, config);

        assert!(result.is_err(), "Snapshot importer creation should fail with missing directory");
    }
}