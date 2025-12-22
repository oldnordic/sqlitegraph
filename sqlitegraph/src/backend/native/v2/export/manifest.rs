//! V2 Export Manifest Generation and Validation
//!
//! This module implements manifest generation and validation for V2 exports,
//! providing metadata about export consistency, boundaries, and integrity.

use crate::backend::native::types::{NativeBackendError, NativeResult};
use crate::backend::native::v2::wal::recovery::states::{Authority, RecoveryState as ExplicitRecoveryState};
use super::ExportMode;
use std::path::Path;

/// Export manifest containing metadata about exported database state
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ExportManifest {
    // Format identification
    pub magic: [u8; 8],                    // "V2EXPMF"
    pub version: u32,                      // Manifest format version

    // Consistency information
    pub recovery_state: ExplicitRecoveryState,
    pub authority: Authority,
    pub export_mode: ExportMode,

    // LSN boundaries
    pub graph_checkpoint_lsn: u64,
    pub wal_start_lsn: Option<u64>,
    pub wal_end_lsn: Option<u64>,

    // Format compatibility
    pub graph_format_version: u32,
    pub wal_format_version: u32,
    pub v2_clustered_edges: bool,

    // Integrity
    pub export_timestamp: u64,
    pub export_duration_ms: u64,
    pub graph_checksum: u64,
    pub wal_checksum: Option<u64>,
    pub total_records: u64,
    pub total_bytes: u64,

    // Reserved for future
    pub reserved: [u64; 8],
}

impl ExportManifest {
    /// Magic bytes for V2 export manifest
    pub const MAGIC: [u8; 8] = [b'V', b'2', b'X', b'P', b'M', b'F', 0, 0];

    /// Current manifest format version
    pub const VERSION: u32 = 1;

    /// Create a new export manifest
    pub fn new() -> Self {
        Self {
            magic: Self::MAGIC,
            version: Self::VERSION,
            recovery_state: ExplicitRecoveryState::CleanShutdown,
            authority: Authority::GraphFile,
            export_mode: ExportMode::CheckpointAligned,
            graph_checkpoint_lsn: 0,
            wal_start_lsn: None,
            wal_end_lsn: None,
            graph_format_version: 2,
            wal_format_version: 1,
            v2_clustered_edges: true,
            export_timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            export_duration_ms: 0,
            graph_checksum: 0,
            wal_checksum: None,
            total_records: 0,
            total_bytes: 0,
            reserved: [0; 8],
        }
    }

    /// Validate manifest integrity
    pub fn validate(&self) -> NativeResult<()> {
        if self.magic != Self::MAGIC {
            return Err(NativeBackendError::CorruptStringTable {
                reason: "Invalid manifest magic bytes".to_string(),
            });
        }

        if self.version != Self::VERSION {
            return Err(NativeBackendError::CorruptStringTable {
                reason: "Unsupported manifest version".to_string(),
            });
        }

        // Validate LSN consistency
        if let (Some(start), Some(end)) = (self.wal_start_lsn, self.wal_end_lsn) {
            if start > end {
                return Err(NativeBackendError::CorruptStringTable {
                    reason: "WAL start LSN cannot be greater than end LSN".to_string(),
                });
            }
        }

        // Validate checkpoint LSN consistency
        if let Some(wal_start) = self.wal_start_lsn {
            if self.graph_checkpoint_lsn > wal_start {
                return Err(NativeBackendError::CorruptStringTable {
                    reason: "Checkpoint LSN cannot be greater than WAL start LSN".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Check if export includes WAL records
    pub fn includes_wal(&self) -> bool {
        self.wal_start_lsn.is_some() && self.wal_end_lsn.is_some()
    }

    /// Get LSN range covered by export
    pub fn lsn_range(&self) -> Option<(u64, u64)> {
        match (self.wal_start_lsn, self.wal_end_lsn) {
            (Some(start), Some(end)) => Some((start, end)),
            (None, None) => Some((self.graph_checkpoint_lsn, self.graph_checkpoint_lsn)),
            _ => None,
        }
    }
}

/// Manifest serializer for binary format
pub struct ManifestSerializer;

impl ManifestSerializer {
    /// Serialize manifest to bytes using JSON format
    pub fn serialize(manifest: &ExportManifest) -> NativeResult<Vec<u8>> {
        serde_json::to_vec(manifest)
            .map_err(|e| NativeBackendError::JsonError(e))
    }

    /// Deserialize manifest from bytes using JSON format
    pub fn deserialize(data: &[u8]) -> NativeResult<ExportManifest> {
        serde_json::from_slice(data)
            .map_err(|e| NativeBackendError::JsonError(e))
    }

    /// Write manifest to file using JSON format
    pub fn write_to_file<P: AsRef<Path>>(
        manifest: &ExportManifest,
        path: P,
    ) -> NativeResult<()> {
        let json_data = Self::serialize(manifest)?;
        std::fs::write(path, json_data)
            .map_err(|e| NativeBackendError::Io(e))?;
        Ok(())
    }

    /// Read manifest from file using JSON format
    pub fn read_from_file<P: AsRef<Path>>(path: P) -> NativeResult<ExportManifest> {
        let json_data = std::fs::read(path)
            .map_err(|e| NativeBackendError::Io(e))?;
        Self::deserialize(&json_data)
    }
}

/// Manifest validator for export integrity
pub struct ManifestValidator {
    manifest: ExportManifest,
}

impl ManifestValidator {
    /// Create validator for manifest
    pub fn new(manifest: ExportManifest) -> Self {
        Self { manifest }
    }

    /// Validate manifest format and consistency
    pub fn validate(&self) -> NativeResult<()> {
        self.manifest.validate()
    }

    /// Validate manifest matches expected export state
    pub fn validate_export_state(&self) -> NativeResult<()> {
        // This will fail initially until we implement state validation
        Err(NativeBackendError::CorruptStringTable {
            reason: "ManifestValidator::validate_export_state not yet implemented".to_string(),
        })
    }

    /// Check if manifest is compatible with target system
    pub fn check_compatibility(&self) -> NativeResult<()> {
        // This will fail initially until we implement compatibility checking
        Err(NativeBackendError::CorruptStringTable {
            reason: "ManifestValidator::check_compatibility not yet implemented".to_string(),
        })
    }
}

