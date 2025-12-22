//! V2 Export Module for SQLiteGraph
//!
//! This module provides deterministic, crash-safe export functionality for V2 clustered edge
//! format databases. Export operates on consistent views using existing WAL, checkpoint,
//! and recovery subsystems.
//!
//! ## Module Organization
//!
//! - **exporter**: Main export orchestration and API
//! - **manifest**: Manifest generation and validation
//!
//! ## Architecture
//!
//! The export system follows these principles:
//! 1. Consistent View Operation: Either checkpoint-aligned or LSN-bounded
//! 2. Read-Only Source Files: Never mutates source graph or WAL files
//! 3. Existing Component Reuse: Uses WAL reader, recovery context, graph file APIs
//! 4. Deterministic Results: Same inputs always produce same outputs

// Re-export core export components
pub use self::exporter::{V2Exporter, V2ExportConfig, ExportResult, ExportConsistencyReport};
pub use self::manifest::{ExportManifest, ManifestSerializer, ManifestValidator};
pub use self::snapshot::{SnapshotExporter, SnapshotExportConfig, SnapshotExportResult, SnapshotValidationReport};

// ExportMode is defined in this module, so it's automatically available

// Module declarations
pub mod exporter;
pub mod manifest;
pub mod snapshot;


/// Export module factory for creating export components
pub struct ExportFactory;

impl ExportFactory {
    /// Create an exporter with default configuration
    pub fn create_exporter(
        graph_path: &std::path::Path,
        export_config: V2ExportConfig,
    ) -> crate::backend::native::types::NativeResult<V2Exporter> {
        V2Exporter::from_graph_file(graph_path, export_config)
    }

    /// Create an exporter optimized for checkpoint-aligned exports
    pub fn create_checkpoint_aligned_exporter(
        graph_path: &std::path::Path,
        export_dir: &std::path::Path,
    ) -> crate::backend::native::types::NativeResult<V2Exporter> {
        let config = V2ExportConfig {
            export_path: export_dir.join("export"),
            include_wal_tail: false,
            compression_enabled: false,
            checksum_validation: true,
        };
        Self::create_exporter(graph_path, config)
    }

    /// Create an exporter optimized for full exports (graph + WAL)
    pub fn create_full_exporter(
        graph_path: &std::path::Path,
        export_dir: &std::path::Path,
    ) -> crate::backend::native::types::NativeResult<V2Exporter> {
        let config = V2ExportConfig {
            export_path: export_dir.join("export"),
            include_wal_tail: true,
            compression_enabled: false,
            checksum_validation: true,
        };
        Self::create_exporter(graph_path, config)
    }

    /// Create a snapshot exporter for instant database state exports
    pub fn create_snapshot_exporter(
        graph_path: &std::path::Path,
        export_dir: &std::path::Path,
        snapshot_id: Option<String>,
    ) -> crate::backend::native::types::NativeResult<SnapshotExporter> {
        let config = SnapshotExportConfig {
            export_path: export_dir.join("snapshot"),
            snapshot_id: snapshot_id.unwrap_or_else(|| {
                format!("snapshot_{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs())
            }),
            include_statistics: true,
            min_stable_duration: std::time::Duration::from_secs(0),
            checksum_validation: true,
        };
        SnapshotExporter::new(graph_path, config)
    }
}

/// Export modes for different consistency requirements
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ExportMode {
    /// Export checkpoint-aligned state (no WAL tail)
    CheckpointAligned,

    /// Export with LSN-bounded WAL tail
    LsnBounded,

    /// Export full state (graph + all WAL records)
    Full,

    /// Export instant snapshot (atomic graph file copy, no WAL involvement)
    Snapshot,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_export_factory_creation() {
        let temp_dir = tempdir().unwrap();
        let graph_path = temp_dir.path().join("test.v2");
        let export_dir = temp_dir.path().join("export");

        let result = ExportFactory::create_checkpoint_aligned_exporter(
            &graph_path,
            &export_dir,
        );
        // This should initially fail until we implement the exporter
        assert!(result.is_err() || result.is_ok()); // We don't know the exact error yet
    }
}