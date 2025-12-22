//! V2 Snapshot Export for SQLiteGraph
//!
//! This module implements instant snapshot export functionality that bypasses WAL complexity
//! and provides atomic database state snapshots through direct file operations.
//!
//! ## Architecture
//!
//! Snapshot export diverges from WAL-based exports by:
//! 1. NO WAL reads or writes
//! 2. Direct atomic graph file copy
//! 3. Strict invariant validation before export
//! 4. Immediate consistency checks
//! 5. No recovery orchestration required

use super::{ExportManifest, ManifestSerializer};
use crate::backend::native::{
    graph_file::GraphFile,
    types::{NativeBackendError, NativeResult},
    v2::wal::recovery::states::{Authority, RecoveryState as ExplicitRecoveryState},
};
use crate::backend::native::v2::snapshot::AtomicFileOperations;
use std::path::{Path, PathBuf};
use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Configuration for snapshot export operations
#[derive(Debug, Clone)]
pub struct SnapshotExportConfig {
    /// Export directory path
    pub export_path: PathBuf,

    /// Snapshot identifier (human-readable)
    pub snapshot_id: String,

    /// Whether to include database statistics in manifest
    pub include_statistics: bool,

    /// Minimum stable state duration before allowing snapshot
    pub min_stable_duration: Duration,

    /// Whether to perform checksum validation
    pub checksum_validation: bool,
}

impl Default for SnapshotExportConfig {
    fn default() -> Self {
        Self {
            export_path: PathBuf::from("snapshot_export"),
            snapshot_id: format!("snapshot_{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs()),
            include_statistics: true,
            min_stable_duration: Duration::from_secs(0),
            checksum_validation: true,
        }
    }
}

/// Snapshot validation report
#[derive(Debug, Clone)]
pub struct SnapshotValidationReport {
    /// Whether graph is in stable state
    pub is_stable: bool,

    /// Whether WAL is clean (no active WAL files)
    pub wal_clean: bool,

    /// Whether file consistency checks pass
    pub file_consistent: bool,

    /// Whether commit marker is valid
    pub commit_marker_valid: bool,

    /// Validation errors encountered
    pub errors: Vec<String>,

    /// Validation warnings encountered
    pub warnings: Vec<String>,
}

/// Snapshot export result
#[derive(Debug, Clone)]
pub struct SnapshotExportResult {
    /// Path to exported snapshot file
    pub snapshot_path: PathBuf,

    /// Path to exported manifest file
    pub manifest_path: PathBuf,

    /// Export duration
    pub export_duration: Duration,

    /// Snapshot size in bytes
    pub snapshot_size_bytes: u64,

    /// Export checksum
    pub checksum: u64,

    /// Number of records in snapshot
    pub record_count: u64,

    /// Export timestamp
    pub export_timestamp: u64,
}

/// Snapshot exporter for instant database state exports
pub struct SnapshotExporter {
    /// Graph file being exported
    graph_file: GraphFile,

    /// Export configuration
    config: SnapshotExportConfig,

    /// Source graph file path
    source_path: PathBuf,
}

impl SnapshotExporter {
    /// Create a new snapshot exporter
    pub fn new(
        graph_path: &Path,
        config: SnapshotExportConfig,
    ) -> NativeResult<Self> {
        // Validate graph file exists and is accessible
        if !graph_path.exists() {
            return Err(NativeBackendError::InvalidParameter {
                context: format!("Graph file does not exist: {:?}", graph_path),
                source: None,
            });
        }

        // Open graph file for validation
        let graph_file = GraphFile::open(graph_path)?;

        // Perform initial invariant validation
        Self::validate_initial_invariants(&graph_file, graph_path)?;

        Ok(Self {
            graph_file,
            config,
            source_path: graph_path.to_path_buf(),
        })
    }

    /// Export snapshot with atomic operations
    pub fn export_snapshot(&mut self) -> NativeResult<SnapshotExportResult> {
        let start_time = SystemTime::now();

        // Step 1: Final invariant validation before export
        let validation_report = self.validate_snapshot_conditions()?;
        if !validation_report.is_stable {
            return Err(NativeBackendError::InvalidState {
                context: "Graph is not in stable state for snapshot export".to_string(),
                source: None,
            });
        }

        // Step 2: Ensure export directory exists
        fs::create_dir_all(&self.config.export_path)
            .map_err(|e| NativeBackendError::Io(e))?;

        // Step 3: Generate snapshot paths
        let snapshot_filename = format!("{}.v2", self.config.snapshot_id);
        let snapshot_path = self.config.export_path.join(snapshot_filename);
        let manifest_path = self.config.export_path.join("export.manifest");

        // Step 4: Perform atomic graph file copy using proper AtomicFileOperations
        let atomic_ops = AtomicFileOperations::new();
        atomic_ops.atomic_copy_file(&self.source_path, &snapshot_path)?;

        // Step 5: Calculate snapshot metadata
        let snapshot_size = fs::metadata(&snapshot_path)
            .map_err(|e| NativeBackendError::Io(e))?
            .len();

        let checksum = if self.config.checksum_validation {
            self.calculate_snapshot_checksum(&snapshot_path)?
        } else {
            0
        };

        // Step 6: Generate export manifest
        let manifest = self.generate_export_manifest(
            snapshot_size,
            checksum,
            SystemTime::now(),
        )?;

        // Step 7: Write manifest file
        ManifestSerializer::write_to_file(&manifest, &manifest_path)?;

        // Step 8: Final sync to ensure all files are durable
        self.sync_export_files(&snapshot_path, &manifest_path)?;

        let export_duration = start_time.elapsed().unwrap_or_default();

        Ok(SnapshotExportResult {
            snapshot_path,
            manifest_path,
            export_duration,
            snapshot_size_bytes: snapshot_size,
            checksum,
            record_count: self.graph_file.persistent_header().node_count as u64 +
                           self.graph_file.persistent_header().edge_count as u64,
            export_timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        })
    }

    /// Validate snapshot preconditions
    pub fn validate_snapshot_conditions(&mut self) -> NativeResult<SnapshotValidationReport> {
        let mut report = SnapshotValidationReport {
            is_stable: true,
            wal_clean: true,
            file_consistent: true,
            commit_marker_valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        };

        // Check for active transactions
        if self.graph_file.is_transaction_active() {
            report.is_stable = false;
            report.errors.push(
                "Active transaction detected - snapshot requires stable state".to_string()
            );
        }

        // Check WAL directory is clean
        let wal_path = self.source_path.with_extension("wal");
        if wal_path.exists() {
            let wal_size = fs::metadata(&wal_path)
                .map_err(|e| NativeBackendError::Io(e))?
                .len();

            if wal_size > 0 {
                report.wal_clean = false;
                report.warnings.push(
                    format!("WAL file exists with size {} bytes - snapshot may not represent clean state", wal_size)
                );
            }
        }

        // Validate file consistency
        match self.graph_file.validate_file_size() {
            Ok(_) => {},
            Err(e) => {
                report.file_consistent = false;
                report.errors.push(format!("File size validation failed: {}", e));
            }
        }

        // Validate commit marker
        match self.graph_file.verify_commit_marker() {
            Ok(_) => {},
            Err(e) => {
                report.commit_marker_valid = false;
                report.errors.push(format!("Commit marker validation failed: {}", e));
            }
        }

        // Additional header consistency checks
        let header = self.graph_file.persistent_header();
        if header.node_count < 0 || header.edge_count < 0 {
            report.file_consistent = false;
            report.errors.push("Negative node or edge counts in header".to_string());
        }

        if header.node_count > 1_000_000 || header.edge_count > 10_000_000 {
            report.warnings.push(
                "Large node/edge counts detected - verify export size".to_string()
            );
        }

        Ok(report)
    }

    /// Perform initial invariants validation
    fn validate_initial_invariants(graph_file: &GraphFile, graph_path: &Path) -> NativeResult<()> {
        // Validate magic bytes using the proper V2 constants
        let header = graph_file.persistent_header();
        let expected_magic = crate::backend::native::constants::MAGIC_BYTES;
        if header.magic != expected_magic {
            return Err(NativeBackendError::InvalidMagicBytes {
                found: header.magic,
            });
        }

        // Validate V2 format
        if header.version != 2 {
            return Err(NativeBackendError::UnsupportedVersion {
                version: header.version,
                supported_version: 2,
            });
        }

        // Validate file meets minimum V2 header size
        let metadata = fs::metadata(graph_path)
            .map_err(|e| NativeBackendError::Io(e))?;
        let min_size = crate::backend::native::constants::HEADER_SIZE;
        if metadata.len() < min_size {
            return Err(NativeBackendError::FileTooSmall {
                size: metadata.len(),
                min_size,
            });
        }

        Ok(())
    }

    
    /// Calculate checksum of snapshot file
    fn calculate_snapshot_checksum(&self, snapshot_path: &Path) -> NativeResult<u64> {
        use std::io::Read;

        let mut file = fs::File::open(snapshot_path)
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

    /// Generate export manifest for snapshot
    fn generate_export_manifest(
        &self,
        snapshot_size: u64,
        checksum: u64,
        export_time: SystemTime,
    ) -> NativeResult<ExportManifest> {
        let header = self.graph_file.persistent_header();

        let mut manifest = ExportManifest::new();
        manifest.recovery_state = ExplicitRecoveryState::CleanShutdown;
        manifest.authority = Authority::GraphFile; // Snapshots use GraphFile authority
        manifest.export_mode = super::ExportMode::Snapshot;
        manifest.graph_checkpoint_lsn = 0; // No LSN for snapshots
        manifest.wal_start_lsn = None;
        manifest.wal_end_lsn = None;
        manifest.graph_format_version = header.version;
        manifest.wal_format_version = 1; // Default for snapshots
        manifest.v2_clustered_edges = true; // V2 format always uses clustered edges
        manifest.export_timestamp = export_time
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        manifest.export_duration_ms = 0; // Will be updated after export completes
        manifest.graph_checksum = checksum;
        manifest.wal_checksum = None; // No WAL for snapshots
        manifest.total_records = header.node_count as u64 + header.edge_count as u64;
        manifest.total_bytes = snapshot_size;

        Ok(manifest)
    }

    /// Sync exported files to ensure durability
    fn sync_export_files(&self, snapshot_path: &Path, manifest_path: &Path) -> NativeResult<()> {

        // Sync snapshot file
        {
            let file = fs::OpenOptions::new()
                .write(true)
                .open(snapshot_path)
                .map_err(|e| NativeBackendError::Io(e))?;
            file.sync_all().map_err(|e| NativeBackendError::Io(e))?;
        }

        // Sync manifest file
        {
            let file = fs::OpenOptions::new()
                .write(true)
                .open(manifest_path)
                .map_err(|e| NativeBackendError::Io(e))?;
            file.sync_all().map_err(|e| NativeBackendError::Io(e))?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{NamedTempFile, TempDir};

    fn create_test_graph_file() -> NativeResult<(GraphFile, PathBuf)> {
        let temp_file = NamedTempFile::new().map_err(|e| NativeBackendError::Io(e))?;
        let path = temp_file.path().to_path_buf();

        // Keep temp file from being deleted
        let _ = temp_file.into_temp_path().keep().unwrap();

        let graph_file = GraphFile::create(&path)?;
        Ok((graph_file, path))
    }

    #[test]
    fn test_snapshot_exporter_creation() {
        let (graph_file, graph_path) = create_test_graph_file().expect("Failed to create test graph");

        let config = SnapshotExportConfig::default();
        let result = SnapshotExporter::new(&graph_path, config);

        match result {
            Ok(_) => println!("Snapshot exporter created successfully"),
            Err(e) => {
                println!("Snapshot exporter creation failed: {:?}", e);
                panic!("Snapshot exporter creation should succeed, but got error: {:?}", e);
            }
        }
    }

    #[test]
    fn test_snapshot_exporter_fails_with_missing_file() {
        let missing_path = PathBuf::from("/nonexistent/graph.v2");
        let config = SnapshotExportConfig::default();

        let result = SnapshotExporter::new(&missing_path, config);

        assert!(result.is_err(), "Snapshot exporter creation should fail with missing file");
    }
}