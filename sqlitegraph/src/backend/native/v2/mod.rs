//! V2 clustered edge kernel implementation.
//!
//! This module implements the Phase 14 redesign with compact edge records
//! and clustered adjacency for optimal I/O locality and storage efficiency.

pub mod edge_cluster;
pub mod export;
pub mod free_space;
pub mod import;
pub mod migration;
pub mod node_record_v2;
pub mod planner;
pub mod snapshot;
pub mod storage;
pub mod string_table;
pub mod wal;

// Re-export V2 types
pub use edge_cluster::{CompactEdgeRecord, Direction, EdgeCluster, EdgeRecordCompactExt};
pub use export::{
    ExportConsistencyReport, ExportFactory, ExportManifest, V2Exporter, V2ExportConfig, ExportResult,
};
pub use free_space::FreeSpaceManager;
pub use import::{
    ImportFactory, ImportMode, ImportValidator, PostImportValidator, V2Importer, V2ImportConfig,
    ImportResult, ImportValidationReport, SnapshotImporter, SnapshotImportConfig, SnapshotImportResult, SnapshotImportValidationReport,
};
pub use migration::{detect_format_version, migrate_file, FormatVersion, MigrationResult};
pub use node_record_v2::{NodeRecordV2, NodeRecordV2Ext};
pub use planner::{ExportPlanner, PlannerDecision, DecisionReason, WalAnalysis};
pub use storage::{JsonLimits, JsonValidationError, parse_and_validate_json, parse_and_validate_json_str};
pub use string_table::StringTable;
pub use wal::{
    GraphOperationResult, GraphWALIntegrationConfig, NodeRecordV2WALExt, OperationMetrics,
    IsolationLevel, V2GraphWALIntegrator, V2WALConfig, V2WALHeader, V2WALManager,
    V2WALRecord, V2WALRecordType, WALManagerMetrics,
};

use crate::backend::native::{NativeBackendError, NativeResult};

/// V2 magic bytes for file format identification (SAME AS V1 - magic never changes!)
pub const V2_MAGIC: [u8; 8] = [b'S', b'Q', b'L', b'T', b'G', b'F', 0, 0];
// V3 format: schema_version is u32 (4 bytes) + reserved (4 bytes)
pub const V2_FORMAT_VERSION: u32 = 3;

/// Expected performance targets for V2 format
pub mod performance_targets {
    /// Compact edge records should be < 100 bytes average
    pub const MAX_AVG_EDGE_SIZE: usize = 100;

    /// Storage improvement should be > 70%
    pub const MIN_STORAGE_IMPROVEMENT: f64 = 0.7;

    /// I/O operations should be reduced by > 10x
    pub const MIN_IO_REDUCTION_FACTOR: f64 = 10.0;

    /// Adjacency operations should be > 2x faster
    pub const MIN_ADJACENCY_SPEEDUP: f64 = 2.0;
}

/// Validation utilities for V2 format compliance
pub struct ValidationMetrics {
    pub storage_efficiency: f64,
    pub io_locality_score: f64,
    pub avg_edge_size: usize,
    pub cluster_utilization: f64,
}

impl ValidationMetrics {
    /// Validate that V2 implementation meets performance targets
    pub fn validate_targets(&self) -> NativeResult<()> {
        if self.storage_efficiency < performance_targets::MIN_STORAGE_IMPROVEMENT {
            return Err(NativeBackendError::ValidationFailed {
                metric: "storage_efficiency".to_string(),
                expected: performance_targets::MIN_STORAGE_IMPROVEMENT,
                actual: self.storage_efficiency,
            });
        }

        if self.avg_edge_size > performance_targets::MAX_AVG_EDGE_SIZE {
            return Err(NativeBackendError::ValidationFailed {
                metric: "avg_edge_size".to_string(),
                expected: performance_targets::MAX_AVG_EDGE_SIZE as f64,
                actual: self.avg_edge_size as f64,
            });
        }

        if self.io_locality_score < performance_targets::MIN_IO_REDUCTION_FACTOR {
            return Err(NativeBackendError::ValidationFailed {
                metric: "io_locality_score".to_string(),
                expected: performance_targets::MIN_IO_REDUCTION_FACTOR,
                actual: self.io_locality_score,
            });
        }

        Ok(())
    }
}

#[cfg(feature = "v2_experimental")]
pub fn rebuild_v2_index_if_enabled(
    node_store: &mut crate::backend::native::node_store::NodeStore<'_>,
) -> NativeResult<()> {
    node_store.rebuild_v2_index()
}

#[cfg(not(feature = "v2_experimental"))]
pub fn rebuild_v2_index_if_enabled(
    _node_store: &mut crate::backend::native::node_store::NodeStore<'_>,
) -> NativeResult<()> {
    Ok(())
}
