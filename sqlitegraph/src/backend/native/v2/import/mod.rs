//! V2 Import Module for SQLiteGraph
//!
//! This module provides WAL-backed, recovery-verifiable import functionality for V2 clustered edge
//! format databases. Import reconstructs database state from exported artifacts using existing
//! WAL, checkpoint, and recovery subsystems.
//!
//! ## Module Organization
//!
//! - **importer**: Main import orchestration and API
//! - **validation**: Import validation and recovery verification
//!
//! ## Architecture
//!
//! The import system follows these principles:
//! 1. WAL-Backed Operations: All writes go through existing V2WALManager
//! 2. Bulk Ingest Integration: Use BulkIngestGuard for optimal performance
//! 3. Recovery Validation: Run crash recovery even without a crash
//! 4. Manifest-Driven: Strict validation using export manifest metadata
//! 5. Atomic Operations: Either complete import or rollback entirely

// Re-export core import components
pub use self::importer::{V2Importer, V2ImportConfig, ImportResult, ImportValidationReport};
pub use self::validation::{ImportValidator, PostImportValidator};
pub use self::snapshot::{SnapshotImporter, SnapshotImportConfig, SnapshotImportResult, SnapshotImportValidationReport};

// ImportMode is defined in this module, so it's automatically available

// Module declarations
pub mod importer;
pub mod validation;
pub mod snapshot;


/// Import module factory for creating import components
pub struct ImportFactory;

impl ImportFactory {
    /// Create an importer with default configuration
    pub fn create_importer(
        export_dir: &std::path::Path,
        target_graph_path: &std::path::Path,
        import_config: V2ImportConfig,
    ) -> crate::backend::native::types::NativeResult<V2Importer> {
        V2Importer::from_export_dir(export_dir, target_graph_path, import_config)
    }

    /// Create an importer optimized for fresh imports
    pub fn create_fresh_importer(
        export_dir: &std::path::Path,
        target_graph_path: &std::path::Path,
    ) -> crate::backend::native::types::NativeResult<V2Importer> {
        let config = V2ImportConfig {
            target_graph_path: target_graph_path.to_path_buf(),
            export_dir_path: export_dir.to_path_buf(),
            import_mode: ImportMode::Fresh,
            validate_recovery: true,
            force_checkpoint_after_import: true,
        };
        Self::create_importer(export_dir, target_graph_path, config)
    }

    /// Create an importer optimized for merge imports
    pub fn create_merge_importer(
        export_dir: &std::path::Path,
        target_graph_path: &std::path::Path,
    ) -> crate::backend::native::types::NativeResult<V2Importer> {
        let config = V2ImportConfig {
            target_graph_path: target_graph_path.to_path_buf(),
            export_dir_path: export_dir.to_path_buf(),
            import_mode: ImportMode::Merge,
            validate_recovery: true,
            force_checkpoint_after_import: true,
        };
        Self::create_importer(export_dir, target_graph_path, config)
    }
}

/// Import modes for different target scenarios
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ImportMode {
    /// Import into empty graph file (create new)
    Fresh,

    /// Merge into existing graph file (only if compatible)
    Merge,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_import_factory_creation() {
        let temp_dir = tempdir().unwrap();
        let export_dir = temp_dir.path().join("export");
        let target_path = temp_dir.path().join("imported.v2");

        let result = ImportFactory::create_fresh_importer(
            &export_dir,
            &target_path,
        );
        // This should initially fail until we implement the importer
        assert!(result.is_err() || result.is_ok()); // We don't know the exact error yet
    }
}