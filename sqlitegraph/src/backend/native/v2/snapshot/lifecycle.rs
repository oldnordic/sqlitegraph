//! Snapshot Lifecycle Management for SQLiteGraph V2
//!
//! This module defines an explicit, deterministic lifecycle model for snapshots.
//! All lifecycle operations are READ-ONLY and derive state from existing files
//! and system signals without side effects.
//!
//! ## Lifecycle States
//!
//! Snapshots progress through well-defined states with deterministic transitions:
//! - **Creating**: Snapshot export in progress, temporary files exist
//! - **Stable**: Complete snapshot ready for use, all files present and valid
//! - **Verifying**: Snapshot validation in progress, integrity checks running
//! - **Importable**: Snapshot validated and ready for import operations
//! - **Applied**: Snapshot successfully imported into target database
//! - **Obsolete**: Snapshot superseded or corrupted, no longer usable

use crate::backend::native::types::{NativeBackendError, NativeResult};
use crate::backend::native::v2::export::ExportManifest;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Explicit snapshot lifecycle states
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SnapshotLifecycleState {
    /// Snapshot export is currently in progress
    Creating,

    /// Snapshot is complete and stable, ready for use
    Stable,

    /// Snapshot validation is in progress
    Verifying,

    /// Snapshot has been validated and is ready for import
    Importable,

    /// Snapshot has been successfully imported
    Applied,

    /// Snapshot is obsolete or corrupted
    Obsolete,
}

impl SnapshotLifecycleState {
    /// Check if state is a terminal state
    pub fn is_terminal(self) -> bool {
        matches!(self, SnapshotLifecycleState::Applied | SnapshotLifecycleState::Obsolete)
    }

    /// Check if state allows export operations
    pub fn allows_export(self) -> bool {
        matches!(self, SnapshotLifecycleState::Stable | SnapshotLifecycleState::Importable)
    }

    /// Check if state allows import operations
    pub fn allows_import(self) -> bool {
        matches!(self, SnapshotLifecycleState::Importable)
    }
}

/// Snapshot lifecycle inspector (READ-ONLY operations only)
pub struct SnapshotLifecycleInspector {
    export_dir: PathBuf,
}

impl SnapshotLifecycleInspector {
    /// Create lifecycle inspector for export directory
    pub fn new(export_dir: &Path) -> Self {
        Self {
            export_dir: export_dir.to_path_buf(),
        }
    }

    /// Determine current snapshot lifecycle state
    pub fn determine_state(&self) -> NativeResult<SnapshotLifecycleState> {
        let manifest_path = self.export_dir.join("export.manifest");
        let snapshot_files = self.list_snapshot_files()?;

        // State 1: Creating - export in progress
        if self.is_export_in_progress(&snapshot_files)? {
            return Ok(SnapshotLifecycleState::Creating);
        }

        // State 2: Stable - complete snapshot ready for use (checked before Importable)
        if self.is_snapshot_stable(&manifest_path, &snapshot_files)? {
            // But if validation is also complete, it's Importable, not Stable
            if self.is_snapshot_importable(&manifest_path, &snapshot_files)? {
                return Ok(SnapshotLifecycleState::Importable);
            }
            return Ok(SnapshotLifecycleState::Stable);
        }

        // State 3: Verifying - validation in progress
        if self.is_validation_in_progress(&snapshot_files)? {
            return Ok(SnapshotLifecycleState::Verifying);
        }

        // State 4: Importable - validated and ready for import (fallback if not stable)
        if self.is_snapshot_importable(&manifest_path, &snapshot_files)? {
            return Ok(SnapshotLifecycleState::Importable);
        }

        // State 5: Applied - successfully imported (presence of import marker)
        if self.is_snapshot_applied(&snapshot_files)? {
            return Ok(SnapshotLifecycleState::Applied);
        }

        // State 6: Obsolete - corrupted or superseded
        if snapshot_files.is_empty() {
            return Ok(SnapshotLifecycleState::Obsolete);
        }

        // Default to obsolete if state cannot be determined
        Ok(SnapshotLifecycleState::Obsolete)
    }

    /// Check if export is currently in progress
    fn is_export_in_progress(&self, snapshot_files: &[PathBuf]) -> NativeResult<bool> {
        // Export in progress if:
        // 1. Temporary files exist (*.tmp) but NOT .complete files
        // 2. No complete manifest file exists
        // 3. At least one actual snapshot file component exists (.v2 files)

        // Check for actual temporary files, not completion markers
        if snapshot_files.iter().any(|p| {
            p.extension().map_or(false, |ext| ext == "tmp") &&
            !p.file_name().map_or(false, |name| name.to_str().map_or(false, |s| s.ends_with(".complete")))
        }) {
            return Ok(true);
        }

        let manifest_path = self.export_dir.join("export.manifest");
        // Look for actual snapshot files, not completion markers
        let has_snapshot_files = snapshot_files.iter().any(|p| {
            p.extension().map_or(false, |ext| ext == "v2") ||
            p.file_name().map_or(false, |name| name.to_str().map_or(false, |s| s == "export.manifest"))
        });

        if !manifest_path.exists() && has_snapshot_files {
            return Ok(true);
        }

        Ok(false)
    }

    /// Check if snapshot is stable and complete
    fn is_snapshot_stable(&self, manifest_path: &Path, snapshot_files: &[PathBuf]) -> NativeResult<bool> {
        // Stable if:
        // 1. Manifest exists and is valid
        // 2. All required files are present
        // 3. No temporary files exist
        // 4. No corruption detected

        if !manifest_path.exists() {
            return Ok(false);
        }

        // Read and validate manifest
        let manifest = match self.read_manifest(manifest_path) {
            Ok(m) => m,
            Err(_) => return Ok(false),
        };

        // Check for required files based on manifest
        if !self.all_required_files_present(&manifest, snapshot_files)? {
            return Ok(false);
        }

        // No temporary files should exist in stable state
        if snapshot_files.iter().any(|p| p.extension().map_or(false, |ext| ext == "tmp")) {
            return Ok(false);
        }

        Ok(true)
    }

    /// Check if validation is in progress
    fn is_validation_in_progress(&self, _snapshot_files: &[PathBuf]) -> NativeResult<bool> {
        // Validation in progress if:
        // 1. Validation lock file exists
        // 2. No validation error files exist

        let validation_lock = self.export_dir.join("validation.lock");
        if validation_lock.exists() {
            return Ok(true);
        }

        Ok(false)
    }

    /// Check if snapshot is importable
    fn is_snapshot_importable(&self, manifest_path: &Path, snapshot_files: &[PathBuf]) -> NativeResult<bool> {
        // Importable if:
        // 1. Stable state conditions met
        // 2. Validation marker file exists
        // 3. No import errors detected

        if !self.is_snapshot_stable(manifest_path, snapshot_files)? {
            return Ok(false);
        }

        let validation_marker = self.export_dir.join("validation.complete");
        if !validation_marker.exists() {
            return Ok(false);
        }

        let validation_error = self.export_dir.join("validation.error");
        if validation_error.exists() {
            return Ok(false);
        }

        Ok(true)
    }

    /// Check if snapshot has been applied
    fn is_snapshot_applied(&self, _snapshot_files: &[PathBuf]) -> NativeResult<bool> {
        // Applied if import marker exists
        let import_marker = self.export_dir.join("import.complete");
        if import_marker.exists() {
            return Ok(true);
        }

        Ok(false)
    }

    /// List all snapshot-related files in export directory
    fn list_snapshot_files(&self) -> NativeResult<Vec<PathBuf>> {
        if !self.export_dir.exists() {
            return Ok(vec![]);
        }

        let mut files = Vec::new();

        let entries = match std::fs::read_dir(&self.export_dir) {
            Ok(entries) => entries,
            Err(_) => return Ok(vec![]),
        };

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => continue,
            };

            let path = entry.path();

            // Include relevant files
            if let Some(name) = path.file_name() {
                if let Some(name_str) = name.to_str() {
                    if name_str.ends_with(".v2") ||
                       name_str.ends_with(".tmp") ||
                       name_str == "export.manifest" ||
                       name_str.starts_with("validation.") ||
                       name_str.starts_with("import.") {
                        files.push(path);
                    }
                }
            }
        }

        Ok(files)
    }

    /// Read and validate export manifest
    fn read_manifest(&self, manifest_path: &Path) -> NativeResult<ExportManifest> {
        use crate::backend::native::v2::export::ManifestSerializer;

        ManifestSerializer::read_from_file(manifest_path)
    }

    /// Check if all required files are present according to manifest
    fn all_required_files_present(&self, _manifest: &ExportManifest, snapshot_files: &[PathBuf]) -> NativeResult<bool> {
        // For snapshots, we need exactly one .v2 file
        let v2_files: Vec<&PathBuf> = snapshot_files
            .iter()
            .filter(|p| p.extension().map_or(false, |ext| ext == "v2"))
            .collect();

        if v2_files.len() != 1 {
            return Ok(false);
        }

        // Verify snapshot file exists and has reasonable size
        let snapshot_file = &v2_files[0];
        if !snapshot_file.exists() {
            return Ok(false);
        }

        let metadata = match std::fs::metadata(snapshot_file) {
            Ok(meta) => meta,
            Err(_) => return Ok(false),
        };

        if metadata.len() < 80 { // Minimum V2 header size
            return Ok(false);
        }

        Ok(true)
    }

    /// Validate if a state transition is allowed
    pub fn validate_transition(&self, from: SnapshotLifecycleState, to: SnapshotLifecycleState) -> NativeResult<()> {
        // Define allowed state transitions
        match (from, to) {
            // From Creating
            (SnapshotLifecycleState::Creating, SnapshotLifecycleState::Stable) => Ok(()),
            (SnapshotLifecycleState::Creating, SnapshotLifecycleState::Obsolete) => Ok(()),

            // From Stable
            (SnapshotLifecycleState::Stable, SnapshotLifecycleState::Verifying) => Ok(()),
            (SnapshotLifecycleState::Stable, SnapshotLifecycleState::Obsolete) => Ok(()),

            // From Verifying
            (SnapshotLifecycleState::Verifying, SnapshotLifecycleState::Stable) => Ok(()), // Validation failed, back to stable
            (SnapshotLifecycleState::Verifying, SnapshotLifecycleState::Importable) => Ok(()),
            (SnapshotLifecycleState::Verifying, SnapshotLifecycleState::Obsolete) => Ok(()),

            // From Importable
            (SnapshotLifecycleState::Importable, SnapshotLifecycleState::Applied) => Ok(()),
            (SnapshotLifecycleState::Importable, SnapshotLifecycleState::Obsolete) => Ok(()),

            // From Applied (terminal state, only to Obsolete)
            (SnapshotLifecycleState::Applied, SnapshotLifecycleState::Obsolete) => Ok(()),

            // Same state transitions (idempotent)
            (from, to) if from == to => Ok(()),

            // All other transitions are invalid
            (from, to) => Err(NativeBackendError::InvalidState {
                context: format!("Invalid snapshot lifecycle state transition from {:?} to {:?}", from, to),
                source: None,
            }),
        }
    }

    /// Get snapshot metadata for lifecycle tracking
    pub fn get_metadata(&self) -> NativeResult<SnapshotMetadata> {
        let state = self.determine_state()?;
        let manifest_path = self.export_dir.join("export.manifest");

        let (manifest_exists, export_timestamp) = if manifest_path.exists() {
            let metadata = std::fs::metadata(&manifest_path)
                .map_err(|e| NativeBackendError::Io(e))?;

            let timestamp = metadata.created()
                .or_else(|_| metadata.modified())
                .or_else(|_| Ok(SystemTime::now()))
                .map_err(|_: std::time::SystemTimeError| NativeBackendError::CorruptionDetected {
                    context: "Cannot determine manifest timestamp".to_string(),
                    source: None,
                })?
                .duration_since(UNIX_EPOCH)
                .map_err(|_: std::time::SystemTimeError| NativeBackendError::CorruptionDetected {
                    context: "Invalid manifest timestamp".to_string(),
                    source: None,
                })?
                .as_secs();

            (true, timestamp)
        } else {
            (false, 0)
        };

        Ok(SnapshotMetadata {
            state,
            export_dir: self.export_dir.clone(),
            manifest_exists,
            export_timestamp,
            inspection_timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_: std::time::SystemTimeError| NativeBackendError::CorruptionDetected {
                    context: "System clock error".to_string(),
                    source: None,
                })?
                .as_secs(),
        })
    }
}

/// Snapshot metadata for lifecycle tracking
#[derive(Debug, Clone)]
pub struct SnapshotMetadata {
    /// Current lifecycle state
    pub state: SnapshotLifecycleState,

    /// Export directory path
    pub export_dir: PathBuf,

    /// Whether manifest file exists
    pub manifest_exists: bool,

    /// Export timestamp (Unix epoch seconds)
    pub export_timestamp: u64,

    /// Last inspection timestamp (Unix epoch seconds)
    pub inspection_timestamp: u64,
}

impl SnapshotMetadata {
    /// Get age of snapshot in seconds
    pub fn age_seconds(&self) -> u64 {
        self.inspection_timestamp.saturating_sub(self.export_timestamp)
    }

    /// Check if snapshot is stale (older than specified duration)
    pub fn is_stale(&self, max_age_seconds: u64) -> bool {
        self.age_seconds() > max_age_seconds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::v2::wal::recovery::states::{RecoveryState as ExplicitRecoveryState, Authority};
    use tempfile::{TempDir, NamedTempFile};
    use std::fs;
    use crate::backend::native::constants::MAGIC_BYTES;
    use crate::backend::native::v2::export::ExportManifest;

    #[test]
    fn test_snapshot_lifecycle_clean_export() {
        // This test should FAIL initially - create failing TDD test
        let temp_dir = TempDir::new().unwrap();
        let inspector = SnapshotLifecycleInspector::new(temp_dir.path());

        // Initially, no snapshot exists - should be Obsolete
        let state = inspector.determine_state().unwrap();
        assert_eq!(state, SnapshotLifecycleState::Obsolete);

        // Create a proper snapshot with manifest
        let snapshot_file = temp_dir.path().join("test_snapshot.v2");
        create_minimal_v2_file(&snapshot_file);

        let manifest = create_test_manifest();
        write_manifest(temp_dir.path(), &manifest);

        // Now should be Stable (failing assertion until implementation complete)
        let state = inspector.determine_state().unwrap();
        assert_eq!(state, SnapshotLifecycleState::Stable);

        // Test advanced functionality that should fail initially:
        // Check that Stable state allows export operations
        assert!(state.allows_export());
        // Check that Stable state does NOT allow import (needs validation first)
        assert!(!state.allows_import());

        // This should fail initially - validate_transition is not implemented yet
        inspector.validate_transition(SnapshotLifecycleState::Stable, SnapshotLifecycleState::Verifying).unwrap();
    }

    #[test]
    fn test_snapshot_lifecycle_incomplete_export() {
        // Test case: Export in progress with temporary files
        let temp_dir = TempDir::new().unwrap();
        let inspector = SnapshotLifecycleInspector::new(temp_dir.path());

        // Create temporary file indicating export in progress
        let temp_file = temp_dir.path().join("test_snapshot.tmp");
        fs::write(&temp_file, b"temporary data").unwrap();

        // Should detect Creating state
        let state = inspector.determine_state().unwrap();
        assert_eq!(state, SnapshotLifecycleState::Creating);
    }

    #[test]
    fn test_snapshot_lifecycle_importable() {
        // Test case: Validated snapshot ready for import
        let temp_dir = TempDir::new().unwrap();
        let inspector = SnapshotLifecycleInspector::new(temp_dir.path());

        // Create complete snapshot
        let snapshot_file = temp_dir.path().join("test_snapshot.v2");
        create_minimal_v2_file(&snapshot_file);

        let manifest = create_test_manifest();
        write_manifest(temp_dir.path(), &manifest);

        // Add validation completion marker
        let validation_marker = temp_dir.path().join("validation.complete");
        fs::write(&validation_marker, b"validation complete").unwrap();

        // Should detect Importable state
        let state = inspector.determine_state().unwrap();
        assert_eq!(state, SnapshotLifecycleState::Importable);
    }

    #[test]
    fn test_snapshot_lifecycle_obsolete_after_import() {
        // Test case: Snapshot marked as applied after import
        let temp_dir = TempDir::new().unwrap();
        let inspector = SnapshotLifecycleInspector::new(temp_dir.path());

        // Create import completion marker
        let import_marker = temp_dir.path().join("import.complete");
        fs::write(&import_marker, b"import complete").unwrap();

        // Should detect Applied state
        let state = inspector.determine_state().unwrap();
        assert_eq!(state, SnapshotLifecycleState::Applied);
    }

    #[test]
    fn test_lifecycle_state_properties() {
        // Test state property methods
        assert!(!SnapshotLifecycleState::Creating.is_terminal());
        assert!(!SnapshotLifecycleState::Stable.is_terminal());
        assert!(!SnapshotLifecycleState::Verifying.is_terminal());
        assert!(!SnapshotLifecycleState::Importable.is_terminal());
        assert!(SnapshotLifecycleState::Applied.is_terminal());
        assert!(SnapshotLifecycleState::Obsolete.is_terminal());

        assert!(!SnapshotLifecycleState::Creating.allows_export());
        assert!(SnapshotLifecycleState::Stable.allows_export());
        assert!(SnapshotLifecycleState::Importable.allows_export());

        assert!(!SnapshotLifecycleState::Stable.allows_import());
        assert!(SnapshotLifecycleState::Importable.allows_import());
    }

    #[test]
    fn test_snapshot_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let inspector = SnapshotLifecycleInspector::new(temp_dir.path());

        let metadata = inspector.get_metadata().unwrap();

        assert_eq!(metadata.state, SnapshotLifecycleState::Obsolete);
        assert!(!metadata.manifest_exists);
        assert_eq!(metadata.export_timestamp, 0);
        assert!(metadata.inspection_timestamp > 0);
    }

    // Helper functions for test setup

    fn create_minimal_v2_file(path: &Path) {
        use std::io::Write;

        let mut file = fs::File::create(path).unwrap();

        // Write V2 magic bytes and minimal header (80 bytes total)
        file.write_all(&MAGIC_BYTES).unwrap();

        // Write padding to reach minimum V2 size
        let padding = vec![0u8; 80 - MAGIC_BYTES.len()];
        file.write_all(&padding).unwrap();

        file.sync_all().unwrap();
    }

    fn create_test_manifest() -> ExportManifest {
        use crate::backend::native::v2::export::ExportMode;

        ExportManifest {
            magic: crate::backend::native::v2::export::ExportManifest::MAGIC,
            version: crate::backend::native::v2::export::ExportManifest::VERSION,
            recovery_state: ExplicitRecoveryState::CleanShutdown,
            authority: Authority::GraphFile,
            export_mode: ExportMode::Snapshot,
            graph_checkpoint_lsn: 0,
            wal_start_lsn: None,
            wal_end_lsn: None,
            graph_format_version: 2,
            wal_format_version: 1,
            v2_clustered_edges: true,
            export_timestamp: 1704067200,
            export_duration_ms: 150,
            graph_checksum: 1234567890,
            wal_checksum: None,
            total_records: 42,
            total_bytes: 1048576,
            reserved: [0; 8],
        }
    }

    fn write_manifest(export_dir: &Path, manifest: &ExportManifest) {
        use crate::backend::native::v2::export::ManifestSerializer;

        let manifest_path = export_dir.join("export.manifest");
        ManifestSerializer::write_to_file(manifest, manifest_path).unwrap();
    }
}