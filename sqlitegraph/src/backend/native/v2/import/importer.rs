//! V2 Import Engine for SQLiteGraph
//!
//! This module implements the main import orchestration that reconstructs database
//! state from exported artifacts using existing WAL, checkpoint, and recovery components.

use crate::backend::native::v2::{ExportManifest, ImportMode};
use crate::backend::native::v2::export::ManifestSerializer;
use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::types::{NativeBackendError, NativeResult};
use crate::backend::native::v2::wal::recovery::states::{
    RecoveryState as ExplicitRecoveryState,
};
use crate::backend::native::v2::wal::{V2WALConfig, V2WALRecord};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Configuration for V2 import operations
#[derive(Debug, Clone)]
pub struct V2ImportConfig {
    /// Target graph file path
    pub target_graph_path: PathBuf,

    /// Export directory path
    pub export_dir_path: PathBuf,

    /// Import mode (Fresh or Merge)
    pub import_mode: ImportMode,

    /// Whether to validate recovery after import
    pub validate_recovery: bool,

    /// Whether to force checkpoint after import
    pub force_checkpoint_after_import: bool,
}

/// Import validation report
#[derive(Debug, Clone)]
pub struct ImportValidationReport {
    /// Whether manifest is valid
    pub manifest_valid: bool,

    /// Whether all required files exist
    pub files_exist: bool,

    /// Whether formats are compatible
    pub format_compatible: bool,

    /// Whether target is compatible for merge
    pub target_compatible: bool,

    /// Validation warnings
    pub warnings: Vec<String>,

    /// Validation errors
    pub errors: Vec<String>,
}

/// Import operation result
#[derive(Debug, Clone)]
pub struct ImportResult {
    /// Number of records imported
    pub records_imported: u64,

    /// Number of WAL records replayed
    pub wal_records_replayed: u64,

    /// Import duration
    pub import_duration: Duration,

    /// Final LSN after import
    pub final_lsn: u64,

    /// Recovery state after import
    pub recovery_state: ExplicitRecoveryState,

    /// Whether validation passed
    pub validation_passed: bool,
}

/// Main V2 importer that orchestrates import operations
pub struct V2Importer {
    /// Import configuration
    config: V2ImportConfig,

    /// Export manifest
    manifest: ExportManifest,

    /// WAL configuration for target
    wal_config: V2WALConfig,
}

impl V2Importer {
    /// Create importer from export directory
    pub fn from_export_dir(
        export_dir: &Path,
        target_graph_path: &Path,
        import_config: V2ImportConfig,
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

        // Use existing ManifestSerializer to read manifest
        let manifest = ManifestSerializer::read_from_file(&manifest_path)
            .map_err(|_| NativeBackendError::InvalidParameter {
                context: format!("Failed to read manifest from {:?}", manifest_path),
                source: None,
            })?;

        // Set up WAL configuration for target
        let mut wal_config = V2WALConfig::for_graph_file(target_graph_path);
        wal_config.enable_compression = import_config.import_mode == ImportMode::Fresh;
        wal_config.validate()?;

        Ok(V2Importer {
            config: import_config,
            manifest,
            wal_config,
        })
    }

    /// Validate export before import
    pub fn validate_export(&self) -> NativeResult<ImportValidationReport> {
        let mut report = ImportValidationReport {
            manifest_valid: false,
            files_exist: false,
            format_compatible: false,
            target_compatible: false,
            warnings: Vec::new(),
            errors: Vec::new(),
        };

        // Validate manifest integrity
        report.manifest_valid = self.validate_manifest_integrity(&mut report.warnings, &mut report.errors);

        // Validate required files exist
        report.files_exist = self.validate_export_files(&mut report.warnings, &mut report.errors);

        // Validate format compatibility
        report.format_compatible = self.validate_format_compatibility(&mut report.warnings, &mut report.errors);

        // Validate target compatibility for merge imports
        if self.config.import_mode == ImportMode::Merge {
            report.target_compatible = self.validate_target_compatibility(&mut report.warnings, &mut report.errors);
        } else {
            report.target_compatible = true; // Fresh mode doesn't need target compatibility
        }

        Ok(report)
    }

    /// Perform import into target graph
    pub fn import(&self) -> NativeResult<ImportResult> {
        // This will fail initially until we implement the functionality
        Err(NativeBackendError::CorruptStringTable {
            reason: "V2Importer::import not yet implemented".to_string(),
        })
    }

    /// Validate manifest integrity
    fn validate_manifest_integrity(&self, _warnings: &mut Vec<String>, errors: &mut Vec<String>) -> bool {
        // Check magic bytes
        if self.manifest.magic != ExportManifest::MAGIC {
            errors.push("Invalid manifest magic bytes".to_string());
            return false;
        }

        // Check version
        if self.manifest.version != ExportManifest::VERSION {
            errors.push(format!("Unsupported manifest version: {}", self.manifest.version));
            return false;
        }

        // Check LSN consistency
        if let (Some(wal_start), Some(wal_end)) = (self.manifest.wal_start_lsn, self.manifest.wal_end_lsn) {
            if wal_start > wal_end {
                errors.push("Invalid WAL LSN range: start > end".to_string());
                return false;
            }
        }

        true
    }

    /// Validate that all required export files exist
    fn validate_export_files(&self, warnings: &mut Vec<String>, errors: &mut Vec<String>) -> bool {
        let export_path = &self.config.export_dir_path;
        let mut all_files_exist = true;

        // Expected files based on export patterns
        let expected_graph_files = vec![
            "v2_export_checkpoint.graph",
            "v2_export_lsn.graph",
            "v2_export.graph",
        ];

        let expected_wal_files = vec![
            "v2_export.wal",
            "v2_export_lsn.wal",
        ];

        // Check for manifest file (already validated to exist)
        let manifest_path = export_path.join("export.manifest");
        if !manifest_path.exists() {
            errors.push("Export manifest file missing".to_string());
            all_files_exist = false;
        }

        // Check for graph files based on export mode
        for graph_file in expected_graph_files {
            let file_path = export_path.join(graph_file);
            if file_path.exists() {
                warnings.push(format!("Found graph file: {:?}", file_path));
                return true; // Found at least one graph file
            }
        }

        errors.push("No graph files found in export directory".to_string());
        all_files_exist = false;

        // Check for WAL files (optional)
        for wal_file in expected_wal_files {
            let file_path = export_path.join(wal_file);
            if file_path.exists() {
                warnings.push(format!("Found WAL file: {:?}", file_path));
            }
        }

        all_files_exist
    }

    /// Validate format compatibility
    fn validate_format_compatibility(&self, warnings: &mut Vec<String>, errors: &mut Vec<String>) -> bool {
        // Check if V2 format is supported
        if self.manifest.graph_format_version != 2 {
            errors.push(format!("Unsupported graph format version: {}", self.manifest.graph_format_version));
            return false;
        }

        // Check WAL format version compatibility
        if let Some(wal_end_lsn) = self.manifest.wal_end_lsn {
            if wal_end_lsn > 0 && self.manifest.wal_format_version != 1 {
                errors.push(format!("Unsupported WAL format version: {}", self.manifest.wal_format_version));
                return false;
            }
        }

        // Warn about potential incompatibilities
        if self.manifest.export_mode != crate::backend::native::v2::export::ExportMode::CheckpointAligned && self.manifest.wal_end_lsn.is_none() {
            warnings.push("Export is not checkpoint aligned and has no WAL tail".to_string());
        }

        // Check V2 clustered edge support
        if !self.manifest.v2_clustered_edges {
            errors.push("Import requires V2 clustered edge format support".to_string());
            return false;
        }

        true
    }

    /// Validate target compatibility for merge imports
    fn validate_target_compatibility(&self, warnings: &mut Vec<String>, errors: &mut Vec<String>) -> bool {
        let target_path = &self.config.target_graph_path;

        // Check if target graph exists for merge
        if !target_path.exists() {
            errors.push("Target graph file does not exist for merge import".to_string());
            return false;
        }

        // Try to open target graph to validate format
        match GraphFile::open(target_path) {
            Ok(_graph_file) => {
                // Target exists and is readable
                warnings.push("Merge import target validation successful".to_string());
                true
            }
            Err(e) => {
                errors.push(format!("Target graph file is not readable: {}", e));
                false
            }
        }
    }

    /// Replay WAL records using existing transaction patterns
    fn replay_wal_records(&self, _wal_records: &[V2WALRecord]) -> NativeResult<()> {
        // This will fail initially until we implement the functionality
        Err(NativeBackendError::CorruptStringTable {
            reason: "V2Importer::replay_wal_records not yet implemented".to_string(),
        })
    }
}

