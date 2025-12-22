//! Recovery Orchestration Coordinator for SQLiteGraph V2 WAL
//!
//! This module implements a deterministic, centralized recovery coordinator that
//! orchestrates crash recovery using existing WAL reader + replayer components.
//! No new recovery logic is duplicated - the coordinator delegates to existing
//! specialized modules while centralizing decision-making and authority resolution.
//!
//! ## Architecture
//!
//! The recovery coordinator follows this pattern:
//! 1. **Analysis Phase**: Determine recovery state using existing signals
//! 2. **Decision Phase**: Apply explicit authority resolution rules
//! 3. **Orchestration Phase**: Coordinate existing WAL reader and replayer
//! 4. **Validation Phase**: Ensure recovery completion and consistency
//!
//! ## Guarantees
//!
//! - **Deterministic**: Same inputs always produce same outputs
//! - **No Side Effects During Inspection**: Analysis phase never modifies files
//! - **Idempotent Recovery**: Replay can be safely repeated
//! - **Authority Resolution**: Clear WAL vs graph file precedence rules

use super::{
    states::{Authority, RecoveryContext, RecoveryState as ExplicitRecoveryState},
};
use crate::backend::native::v2::wal::V2WALConfig;
use crate::backend::native::{NativeBackendError, NativeResult};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// Recovery coordinator that orchestrates crash recovery
///
/// This coordinator uses ONLY existing components:
/// - RecoveryContext for state determination
/// - V2WALRecoveryEngine for existing recovery orchestration
/// - V2WALReader for WAL file access
/// - Existing replayer for transaction replay
///
/// No new recovery logic is implemented - this is pure orchestration.
pub struct RecoveryCoordinator {
    /// Recovery configuration
    config: V2WALConfig,

    /// Database file path
    database_path: PathBuf,

    /// Checkpoint file path
    checkpoint_path: PathBuf,
}

/// Recovery operation result
#[derive(Debug, Clone)]
pub struct RecoveryCoordinatorResult {
    /// Recovery decision that was made
    pub decision: RecoveryDecision,

    /// Whether recovery was performed
    pub recovery_performed: bool,

    /// Recovery duration
    pub duration: Duration,

    /// Authority used for decisions
    pub authority: Authority,

    /// Recovery state detected
    pub state: ExplicitRecoveryState,

    /// Recovery warnings (if any)
    pub warnings: Vec<String>,

    /// Recovery metrics (if recovery was performed)
    pub metrics: Option<super::core::RecoveryMetrics>,
}

/// Recovery decision made by coordinator
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryDecision {
    /// No recovery needed - system is clean
    NoRecoveryNeeded,

    /// Recovery performed using WAL authority
    RecoveryPerformed,

    /// System is unrecoverable
    Unrecoverable,
}

impl RecoveryCoordinator {
    /// Create new recovery coordinator
    pub fn new(
        config: V2WALConfig,
        database_path: PathBuf,
        checkpoint_path: PathBuf,
    ) -> Self {
        Self {
            config,
            database_path,
            checkpoint_path,
        }
    }

    /// Perform crash recovery orchestration
    ///
    /// This method implements the complete recovery workflow:
    /// 1. Analyze files to determine recovery state
    /// 2. Apply explicit authority resolution rules
    /// 3. Orchestrate recovery using existing components
    /// 4. Return structured result
    ///
    /// All logic is deterministic and uses only existing signals.
    pub fn orchestrate_recovery(&self) -> NativeResult<RecoveryCoordinatorResult> {
        let start_time = Instant::now();

        // Phase 1: Analysis - Determine recovery state (no side effects)
        let context = RecoveryContext::analyze_files(
            &self.config.wal_path,
            &self.database_path,
            &self.checkpoint_path,
        )?;

        let decision = self.make_recovery_decision(context.state, context.authority);

        match decision {
            RecoveryDecision::NoRecoveryNeeded => {
                // No recovery needed - return clean result
                Ok(RecoveryCoordinatorResult {
                    decision,
                    recovery_performed: false,
                    duration: start_time.elapsed(),
                    authority: context.authority,
                    state: context.state,
                    warnings: context.diagnostics,
                    metrics: None,
                })
            }

            RecoveryDecision::RecoveryPerformed => {
                // Phase 2: Orchestration - Use existing recovery engine
                let recovery_result = self.perform_existing_recovery(&context)?;

                Ok(RecoveryCoordinatorResult {
                    decision,
                    recovery_performed: true,
                    duration: start_time.elapsed(),
                    authority: context.authority,
                    state: context.state,
                    warnings: recovery_result.warnings,
                    metrics: Some(recovery_result.metrics),
                })
            }

            RecoveryDecision::Unrecoverable => {
                // System is unrecoverable
                Ok(RecoveryCoordinatorResult {
                    decision,
                    recovery_performed: false,
                    duration: start_time.elapsed(),
                    authority: context.authority,
                    state: context.state,
                    warnings: context.diagnostics,
                    metrics: None,
                })
            }
        }
    }

    /// Make recovery decision based on state and authority
    ///
    /// This implements the explicit decision rules:
    /// - CleanShutdown + GraphFile authority -> No recovery needed
    /// - DirtyShutdown + WAL authority -> Perform recovery
    /// - CorruptWAL/Unrecoverable -> System unrecoverable
    fn make_recovery_decision(&self, state: ExplicitRecoveryState, authority: Authority) -> RecoveryDecision {
        match (state, authority) {
            (ExplicitRecoveryState::CleanShutdown, Authority::GraphFile) => {
                RecoveryDecision::NoRecoveryNeeded
            }
            (ExplicitRecoveryState::DirtyShutdown, Authority::WAL) => {
                RecoveryDecision::RecoveryPerformed
            }
            (ExplicitRecoveryState::PartialCheckpoint, Authority::WAL) => {
                RecoveryDecision::RecoveryPerformed
            }
            (ExplicitRecoveryState::CorruptWAL, Authority::Unrecoverable) => {
                RecoveryDecision::Unrecoverable
            }
            (ExplicitRecoveryState::CorruptGraphFile, Authority::Unrecoverable) => {
                RecoveryDecision::Unrecoverable
            }
            (ExplicitRecoveryState::Unrecoverable, Authority::Unrecoverable) => {
                RecoveryDecision::Unrecoverable
            }
            // Any other combination is considered unrecoverable for safety
            _ => RecoveryDecision::Unrecoverable,
        }
    }

    /// Perform recovery using existing V2WALRecoveryEngine
    ///
    /// This method delegates entirely to existing recovery infrastructure.
    /// No new recovery logic is implemented - this is pure orchestration.
    fn perform_existing_recovery(&self, _context: &RecoveryContext) -> NativeResult<super::core::RecoverySuccess> {
        // Create recovery engine using existing factory
        let engine = super::RecoveryFactory::create_v2_optimized_engine(
            self.config.clone(),
            self.database_path.clone(),
        ).map_err(|e| NativeBackendError::from(e))?;

        // Perform recovery using existing engine
        let recovery_result = engine.recover()
            .map_err(|e| NativeBackendError::from(e))?;

        Ok(recovery_result)
    }

    /// Validate recovery was successful
    ///
    /// This method validates that recovery completed successfully
    /// by checking file consistency and state alignment.
    pub fn validate_recovery(&self, result: &RecoveryCoordinatorResult) -> NativeResult<bool> {
        match result.decision {
            RecoveryDecision::NoRecoveryNeeded => {
                // Validate clean state
                self.validate_clean_state()
            }
            RecoveryDecision::RecoveryPerformed => {
                // Validate recovered state
                self.validate_recovered_state()
            }
            RecoveryDecision::Unrecoverable => {
                // No validation needed for unrecoverable state
                Ok(false)
            }
        }
    }

    /// Validate clean system state
    fn validate_clean_state(&self) -> NativeResult<bool> {
        // Check that all files exist and are consistent
        if !self.database_path.exists() {
            return Ok(false);
        }

        if self.config.wal_path.exists() {
            // If WAL exists, it should be in clean state
            let context = RecoveryContext::analyze_files(
                &self.config.wal_path,
                &self.database_path,
                &self.checkpoint_path,
            )?;

            Ok(context.state == ExplicitRecoveryState::CleanShutdown)
        } else {
            // No WAL file is acceptable for clean state
            Ok(true)
        }
    }

    /// Validate recovered system state
    fn validate_recovered_state(&self) -> NativeResult<bool> {
        // Check that recovery resulted in consistent state
        if !self.database_path.exists() {
            return Ok(false);
        }

        // Verify that no active transactions remain
        if self.config.wal_path.exists() {
            let context = RecoveryContext::analyze_files(
                &self.config.wal_path,
                &self.database_path,
                &self.checkpoint_path,
            )?;

            // After successful recovery, state should be clean or have no active transactions
            match context.state {
                ExplicitRecoveryState::CleanShutdown => Ok(true),
                ExplicitRecoveryState::DirtyShutdown => {
                    // Dirty shutdown is acceptable if no active transactions
                    let header = Self::read_wal_header(&self.config.wal_path)?;
                    Ok(header.active_transactions == 0)
                }
                _ => Ok(false),
            }
        } else {
            // WAL was cleaned up during recovery
            Ok(true)
        }
    }

    /// Read WAL header for validation
    fn read_wal_header(wal_path: &Path) -> NativeResult<crate::backend::native::v2::wal::V2WALHeader> {
        use std::io::Read;

        let mut file = std::fs::File::open(wal_path).map_err(NativeBackendError::from)?;

        // Read header size (V2WALHeader is #[repr(C)])
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

    /// Get recovery coordinator statistics
    pub fn get_statistics(&self) -> RecoveryCoordinatorStats {
        RecoveryCoordinatorStats {
            config: self.config.clone(),
            database_path: self.database_path.clone(),
            checkpoint_path: self.checkpoint_path.clone(),
        }
    }
}

/// Recovery coordinator statistics
#[derive(Debug, Clone)]
pub struct RecoveryCoordinatorStats {
    /// WAL configuration
    pub config: V2WALConfig,

    /// Database file path
    pub database_path: PathBuf,

    /// Checkpoint file path
    pub checkpoint_path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_recovery_coordinator_creation() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };
        let database_path = temp_dir.path().join("test.v2");
        let checkpoint_path = temp_dir.path().join("test.checkpoint");

        let coordinator = RecoveryCoordinator::new(config, database_path.clone(), checkpoint_path);

        assert_eq!(coordinator.database_path, database_path);
    }

    #[test]
    fn test_recovery_decision_clean_shutdown() {
        let coordinator = RecoveryCoordinator::new(
            V2WALConfig::default(),
            PathBuf::from("test.v2"),
            PathBuf::from("test.checkpoint"),
        );

        let decision = coordinator.make_recovery_decision(
            ExplicitRecoveryState::CleanShutdown,
            Authority::GraphFile,
        );

        assert_eq!(decision, RecoveryDecision::NoRecoveryNeeded);
    }

    #[test]
    fn test_recovery_decision_dirty_shutdown() {
        let coordinator = RecoveryCoordinator::new(
            V2WALConfig::default(),
            PathBuf::from("test.v2"),
            PathBuf::from("test.checkpoint"),
        );

        let decision = coordinator.make_recovery_decision(
            ExplicitRecoveryState::DirtyShutdown,
            Authority::WAL,
        );

        assert_eq!(decision, RecoveryDecision::RecoveryPerformed);
    }

    #[test]
    fn test_recovery_decision_corrupt() {
        let coordinator = RecoveryCoordinator::new(
            V2WALConfig::default(),
            PathBuf::from("test.v2"),
            PathBuf::from("test.checkpoint"),
        );

        let decision = coordinator.make_recovery_decision(
            ExplicitRecoveryState::CorruptWAL,
            Authority::Unrecoverable,
        );

        assert_eq!(decision, RecoveryDecision::Unrecoverable);
    }
}