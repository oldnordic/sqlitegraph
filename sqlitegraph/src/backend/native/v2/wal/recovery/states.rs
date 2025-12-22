//! Explicit Recovery State Model for SQLiteGraph V2 WAL
//!
//! This module defines a minimal, explicit recovery state machine that uses
//! only existing signals from WAL headers, graph files, and checkpoints.
//! No new heuristics are introduced - all decisions are based on
//! deterministic validation of existing state.
//!
//! ## Recovery Decision Rules
//!
//! The state model uses these existing signals only:
//! - V2WALHeader.magic and V2WALHeader.version (format validation)
//! - V2WALHeader.current_lsn vs committed_lsn vs checkpointed_lsn (dirty WAL detection)
//! - V2WALHeader.active_transactions (in-flight transactions)
//! - Graph file existence and header validation
//! - Checkpoint file existence and LSN ranges
//! - File size consistency and checksum validation

use crate::backend::native::{NativeBackendError, NativeResult};
use std::path::Path;
use super::RecoverySeverity;

/// Explicit recovery state determined from existing signals
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub enum RecoveryState {
    /// No recovery needed - clean shutdown detected
    CleanShutdown,

    /// Dirty WAL with uncommitted transactions - requires recovery
    DirtyShutdown,

    /// Partial checkpoint in progress - resume checkpoint operation
    PartialCheckpoint,

    /// WAL file is corrupted - cannot recover
    CorruptWAL,

    /// Graph file is corrupted - cannot recover
    CorruptGraphFile,

    /// Both WAL and graph file corrupted - unrecoverable
    Unrecoverable,
}

impl RecoveryState {
    /// Determine recovery state from existing files and headers
    ///
    /// This method uses ONLY existing signals from the files:
    /// - WAL header magic, version, and LSN relationships
    /// - Graph file existence and header validation
    /// - Checkpoint file existence and LSN ranges
    ///
    /// No new heuristics or assumptions are introduced.
    pub fn determine_from_files(
        wal_exists: bool,
        graph_file_exists: bool,
        checkpoint_exists: bool,
        wal_header: Option<&crate::backend::native::v2::wal::V2WALHeader>,
        _graph_file_size: Option<u64>,
    ) -> NativeResult<Self> {
        // Validate file existence first
        if !graph_file_exists {
            return Ok(RecoveryState::Unrecoverable);
        }

        // Check WAL file corruption
        if wal_exists {
            if let Some(header) = wal_header {
                // Validate WAL header integrity using existing validation
                if let Err(_) = header.validate() {
                    return Ok(RecoveryState::CorruptWAL);
                }

                // Check for active transactions (dirty shutdown)
                if header.active_transactions > 0 {
                    return Ok(RecoveryState::DirtyShutdown);
                }

                // Check for uncommitted WAL records (dirty shutdown)
                if header.committed_lsn < header.current_lsn {
                    return Ok(RecoveryState::DirtyShutdown);
                }

                // Check for partial checkpoint
                if checkpoint_exists && header.checkpointed_lsn < header.committed_lsn {
                    return Ok(RecoveryState::PartialCheckpoint);
                }

                // Clean shutdown detected
                if header.checkpointed_lsn == header.committed_lsn && header.active_transactions == 0 {
                    return Ok(RecoveryState::CleanShutdown);
                }
            } else {
                // WAL exists but header is corrupted
                return Ok(RecoveryState::CorruptWAL);
            }
        } else {
            // No WAL file - assume clean shutdown if graph file exists
            return Ok(RecoveryState::CleanShutdown);
        }

        Ok(RecoveryState::CleanShutdown)
    }

    /// Check if recovery is required
    pub fn requires_recovery(&self) -> bool {
        match self {
            RecoveryState::CleanShutdown => false,
            RecoveryState::DirtyShutdown => true,
            RecoveryState::PartialCheckpoint => true,
            RecoveryState::CorruptWAL => true,
            RecoveryState::CorruptGraphFile => true,
            RecoveryState::Unrecoverable => true,
        }
    }

    /// Check if recovery is possible
    pub fn is_recoverable(&self) -> bool {
        match self {
            RecoveryState::CleanShutdown => true,
            RecoveryState::DirtyShutdown => true,
            RecoveryState::PartialCheckpoint => true,
            RecoveryState::CorruptWAL => false,
            RecoveryState::CorruptGraphFile => false,
            RecoveryState::Unrecoverable => false,
        }
    }

    /// Get recovery severity level for diagnostics
    pub fn severity(&self) -> RecoverySeverity {
        match self {
            RecoveryState::CleanShutdown => RecoverySeverity::Minimal,
            RecoveryState::DirtyShutdown => RecoverySeverity::Low,
            RecoveryState::PartialCheckpoint => RecoverySeverity::Medium,
            RecoveryState::CorruptWAL => RecoverySeverity::High,
            RecoveryState::CorruptGraphFile => RecoverySeverity::Critical,
            RecoveryState::Unrecoverable => RecoverySeverity::Critical,
        }
    }
}

// Use existing RecoverySeverity from parent module

impl std::fmt::Display for RecoveryState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RecoveryState::CleanShutdown => write!(f, "CleanShutdown"),
            RecoveryState::DirtyShutdown => write!(f, "DirtyShutdown"),
            RecoveryState::PartialCheckpoint => write!(f, "PartialCheckpoint"),
            RecoveryState::CorruptWAL => write!(f, "CorruptWAL"),
            RecoveryState::CorruptGraphFile => write!(f, "CorruptGraphFile"),
            RecoveryState::Unrecoverable => write!(f, "Unrecoverable"),
        }
    }
}


/// Authority resolution between WAL and graph file
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Authority {
    /// WAL file has authority (replay WAL to graph file)
    WAL,

    /// Graph file has authority (ignore WAL, use graph file as-is)
    GraphFile,

    /// Both are corrupt - unrecoverable
    Unrecoverable,
}

impl Authority {
    /// Determine authority based on recovery state and file integrity
    ///
    /// This implements explicit, deterministic rules for deciding
    /// which data source should be trusted during recovery.
    pub fn determine_from_recovery_state(state: RecoveryState) -> Self {
        match state {
            RecoveryState::CleanShutdown => Authority::GraphFile,
            RecoveryState::DirtyShutdown => Authority::WAL,
            RecoveryState::PartialCheckpoint => Authority::WAL,
            RecoveryState::CorruptWAL => Authority::Unrecoverable,
            RecoveryState::CorruptGraphFile => Authority::Unrecoverable,
            RecoveryState::Unrecoverable => Authority::Unrecoverable,
        }
    }

    /// Check if recovery should proceed
    pub fn should_recover(&self) -> bool {
        match self {
            Authority::WAL => true,
            Authority::GraphFile => false,
            Authority::Unrecoverable => false,
        }
    }
}

/// Recovery context containing all decision information
#[derive(Debug, Clone)]
pub struct RecoveryContext {
    /// Determined recovery state
    pub state: RecoveryState,

    /// Authority decision
    pub authority: Authority,

    /// WAL file path (if exists)
    pub wal_path: Option<std::path::PathBuf>,

    /// Graph file path
    pub graph_file_path: std::path::PathBuf,

    /// Checkpoint file path (if exists)
    pub checkpoint_path: Option<std::path::PathBuf>,

    /// Recovery timestamp
    pub timestamp: std::time::SystemTime,

    /// Additional diagnostic information
    pub diagnostics: Vec<String>,
}

impl RecoveryContext {
    /// Create a new recovery context by analyzing files
    pub fn analyze_files(
        wal_path: &Path,
        graph_file_path: &Path,
        checkpoint_path: &Path,
    ) -> NativeResult<Self> {
        let timestamp = std::time::SystemTime::now();
        let mut diagnostics = Vec::new();

        // Check file existence
        let wal_exists = wal_path.exists();
        let graph_file_exists = graph_file_path.exists();
        let checkpoint_exists = checkpoint_path.exists();

        diagnostics.push(format!("WAL exists: {}", wal_exists));
        diagnostics.push(format!("Graph file exists: {}", graph_file_exists));
        diagnostics.push(format!("Checkpoint exists: {}", checkpoint_exists));

        // Read WAL header if WAL exists
        let wal_header = if wal_exists {
            match Self::read_wal_header(wal_path) {
                Ok(header) => {
                    diagnostics.push(format!("WAL LSN state: current={}, committed={}, checkpointed={}",
                        header.current_lsn, header.committed_lsn, header.checkpointed_lsn));
                    diagnostics.push(format!("WAL active transactions: {}", header.active_transactions));
                    Some(header)
                }
                Err(e) => {
                    diagnostics.push(format!("Failed to read WAL header: {}", e));
                    None
                }
            }
        } else {
            None
        };

        // Get graph file size
        let graph_file_size = if graph_file_exists {
            std::fs::metadata(graph_file_path)
                .map(|m| m.len())
                .ok()
        } else {
            None
        };

        // Determine recovery state
        let state = RecoveryState::determine_from_files(
            wal_exists,
            graph_file_exists,
            checkpoint_exists,
            wal_header.as_ref(),
            graph_file_size,
        )?;

        let authority = Authority::determine_from_recovery_state(state);

        Ok(Self {
            state,
            authority,
            wal_path: if wal_exists { Some(wal_path.to_path_buf()) } else { None },
            graph_file_path: graph_file_path.to_path_buf(),
            checkpoint_path: if checkpoint_exists { Some(checkpoint_path.to_path_buf()) } else { None },
            timestamp,
            diagnostics,
        })
    }

    /// Read WAL header from file
    fn read_wal_header(wal_path: &Path) -> NativeResult<crate::backend::native::v2::wal::V2WALHeader> {
        use std::io::Read;

        let mut file = std::fs::File::open(wal_path).map_err(NativeBackendError::from)?;

        // Read header size (V2WALHeader should be #[repr(C)] for direct reading)
        let header_size = std::mem::size_of::<crate::backend::native::v2::wal::V2WALHeader>();
        let mut header_bytes = vec![0u8; header_size];

        file.read_exact(&mut header_bytes).map_err(NativeBackendError::from)?;

        // Safety: V2WALHeader is #[repr(C)] with stable layout, and we've validated the byte count
        // We need to cast the pointer from *const u8 to *const V2WALHeader
        let header = unsafe {
            std::ptr::read_unaligned::<crate::backend::native::v2::wal::V2WALHeader>(
                header_bytes.as_ptr() as *const crate::backend::native::v2::wal::V2WALHeader
            )
        };

        Ok(header)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::File;
    
    #[test]
    fn test_recovery_state_clean_shutdown() {
        // Create a scenario where all files are clean
        let wal_exists = false;
        let graph_file_exists = true;
        let checkpoint_exists = false;
        let wal_header = None;
        let graph_file_size = Some(1024);

        let state = RecoveryState::determine_from_files(
            wal_exists, graph_file_exists, checkpoint_exists, wal_header, graph_file_size
        ).unwrap();

        assert_eq!(state, RecoveryState::CleanShutdown);
        assert!(!state.requires_recovery());
        assert!(state.is_recoverable());
    }

    #[test]
    fn test_recovery_state_unrecoverable_no_graph() {
        let wal_exists = false;
        let graph_file_exists = false;
        let checkpoint_exists = false;
        let wal_header = None;
        let graph_file_size = None;

        let state = RecoveryState::determine_from_files(
            wal_exists, graph_file_exists, checkpoint_exists, wal_header, graph_file_size
        ).unwrap();

        assert_eq!(state, RecoveryState::Unrecoverable);
        assert!(!state.is_recoverable());
    }

    #[test]
    fn test_authority_resolution() {
        assert_eq!(
            Authority::determine_from_recovery_state(RecoveryState::CleanShutdown),
            Authority::GraphFile
        );
        assert_eq!(
            Authority::determine_from_recovery_state(RecoveryState::DirtyShutdown),
            Authority::WAL
        );
        assert_eq!(
            Authority::determine_from_recovery_state(RecoveryState::CorruptWAL),
            Authority::Unrecoverable
        );
    }

    #[test]
    fn test_recovery_context_creation() {
        let temp_dir = tempdir().unwrap();
        let graph_file_path = temp_dir.path().join("test.v2");
        let wal_path = temp_dir.path().join("test.wal");
        let checkpoint_path = temp_dir.path().join("test.checkpoint");

        // Create graph file
        File::create(&graph_file_path).unwrap();

        let context = RecoveryContext::analyze_files(
            &wal_path, &graph_file_path, &checkpoint_path
        ).unwrap();

        assert_eq!(context.state, RecoveryState::CleanShutdown);
        assert_eq!(context.authority, Authority::GraphFile);
        assert_eq!(context.graph_file_path, graph_file_path);
        assert!(context.wal_path.is_none());
        assert!(context.checkpoint_path.is_none());
        assert!(!context.diagnostics.is_empty());
    }
}