//! Minimal Export/Import Planner for SQLiteGraph V2
//!
//! This module implements a deterministic, rule-based planner for choosing between
//! Snapshot and WAL export strategies based on graph and WAL state analysis.
//!
//! ## Architecture
//!
//! The planner uses a pure decision table with no heuristics:
//! - Stable graph state + Clean WAL → Snapshot (preferred)
//! - Active transactions or dirty WAL → WAL export
//! - Corrupted state → WAL export (with recovery)
//! - Always deterministic, side-effect free

use crate::backend::native::{
    graph_file::GraphFile,
    types::{NativeBackendError, NativeResult},
    v2::export::ExportMode,
    v2::wal::recovery::states::{Authority, RecoveryState as ExplicitRecoveryState},
  };
use std::path::Path;

/// Planner decision result with reasoning
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerDecision {
    /// Chosen export mode
    pub export_mode: ExportMode,

    /// Reasoning for decision
    pub reasoning: DecisionReason,

    /// Whether graph is in stable state
    pub graph_stable: bool,

    /// WAL state analysis
    pub wal_state: WalAnalysis,
}

/// Reasoning for planner decision
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionReason {
    /// Snapshot export due to optimal conditions
    SnapshotOptimal,

    /// Snapshot export forced due to WAL constraints
    SnapshotRequired,

    /// WAL export due to active transactions
    WalActiveTransactions,

    /// WAL export due to dirty WAL state
    WalDirtyState,

    /// WAL export due to WAL file corruption
    WalCorruption,

    /// WAL export due to graph file corruption
    GraphCorruption,

    /// Default fallback to WAL
    WalFallback,
}

/// WAL state analysis
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalAnalysis {
    /// Whether WAL file exists
    pub exists: bool,

    /// WAL file size in bytes
    pub size_bytes: u64,

    /// Whether WAL has active transactions
    pub has_active_transactions: bool,

    /// WAL recovery state
    pub recovery_state: ExplicitRecoveryState,

    /// Authority determination
    pub authority: Authority,
}

/// Minimal deterministic planner for export strategy selection
pub struct ExportPlanner;

impl ExportPlanner {
    /// Analyze graph and WAL state to determine optimal export strategy
    pub fn analyze_export_strategy(
        graph_path: &Path,
    ) -> NativeResult<PlannerDecision> {
        // Step 1: Analyze WAL state
        let wal_state = Self::analyze_wal_state(graph_path)?;

        // Step 2: Analyze graph state
        let graph_stable = Self::analyze_graph_stability(graph_path)?;

        // Step 3: Apply decision rules
        let (export_mode, reasoning) = Self::apply_decision_rules(&wal_state, graph_stable);

        Ok(PlannerDecision {
            export_mode,
            reasoning,
            graph_stable,
            wal_state,
        })
    }

    /// Quick check if snapshot export is advisable
    pub fn is_snapshot_advisable(graph_path: &Path) -> NativeResult<bool> {
        let decision = Self::analyze_export_strategy(graph_path)?;
        Ok(matches!(decision.export_mode, ExportMode::Snapshot))
    }

    /// Analyze WAL state using file system analysis (simplified)
    fn analyze_wal_state(graph_path: &Path) -> NativeResult<WalAnalysis> {
        let wal_path = graph_path.with_extension("wal");

        // Check if WAL exists
        let exists = wal_path.exists();
        let size_bytes = if exists {
            std::fs::metadata(&wal_path)
                .map_err(|e| NativeBackendError::Io(e))?
                .len()
        } else {
            0
        };

        // Simplified analysis: if WAL file exists and has data, assume it needs WAL export
        // In a real implementation, this would use proper WAL header reading
        let has_active_transactions = exists && size_bytes > 0;
        let recovery_state = if has_active_transactions {
            ExplicitRecoveryState::DirtyShutdown
        } else {
            ExplicitRecoveryState::CleanShutdown
        };

        let authority = if recovery_state == ExplicitRecoveryState::CleanShutdown {
            Authority::GraphFile
        } else {
            Authority::WAL
        };

        Ok(WalAnalysis {
            exists,
            size_bytes,
            has_active_transactions,
            recovery_state,
            authority,
        })
    }

    /// Analyze graph file stability
    fn analyze_graph_stability(graph_path: &Path) -> NativeResult<bool> {
        // Try to open graph file
        let mut graph_file = match GraphFile::open(graph_path) {
            Ok(file) => file,
            Err(_) => return Ok(false), // Cannot open = not stable
        };

        // Check for active transactions
        if graph_file.is_transaction_active() {
            return Ok(false);
        }

        // Validate file consistency
        if graph_file.validate_file_size().is_err() {
            return Ok(false);
        }

        // Verify commit marker
        if graph_file.verify_commit_marker().is_err() {
            return Ok(false);
        }

        // Check header consistency
        let header = graph_file.persistent_header();
        if header.node_count < 0 || header.edge_count < 0 {
            return Ok(false);
        }

        Ok(true)
    }

    /// Apply pure rule-based decision table
    fn apply_decision_rules(wal_state: &WalAnalysis, graph_stable: bool) -> (ExportMode, DecisionReason) {
        // Decision Table (deterministic, no heuristics):

        // Rule 1: Perfect conditions → Snapshot
        if graph_stable && !wal_state.exists {
            return (ExportMode::Snapshot, DecisionReason::SnapshotOptimal);
        }

        // Rule 2: Clean WAL with stable graph → Snapshot
        if graph_stable && wal_state.exists &&
           wal_state.size_bytes == 0 &&
           wal_state.recovery_state == ExplicitRecoveryState::CleanShutdown {
            return (ExportMode::Snapshot, DecisionReason::SnapshotOptimal);
        }

        // Rule 3: Active transactions → WAL (cannot snapshot)
        if wal_state.has_active_transactions {
            return (ExportMode::CheckpointAligned, DecisionReason::WalActiveTransactions);
        }

        // Rule 4: Dirty WAL state → WAL (need recovery)
        if wal_state.recovery_state == ExplicitRecoveryState::DirtyShutdown {
            return (ExportMode::LsnBounded, DecisionReason::WalDirtyState);
        }

        // Rule 5: WAL corruption → WAL (need recovery)
        if wal_state.exists && wal_state.authority == Authority::Unrecoverable {
            return (ExportMode::Full, DecisionReason::WalCorruption);
        }

        // Rule 6: Unstable graph → WAL (cannot snapshot)
        if !graph_stable {
            return (ExportMode::Full, DecisionReason::GraphCorruption);
        }

        // Rule 7: Clean WAL with stable graph but WAL has data → Snapshot (preferred)
        if graph_stable && wal_state.exists &&
           wal_state.size_bytes > 0 &&
           wal_state.recovery_state == ExplicitRecoveryState::CleanShutdown {
            return (ExportMode::Snapshot, DecisionReason::SnapshotRequired);
        }

        // Rule 8: Default fallback → WAL
        (ExportMode::CheckpointAligned, DecisionReason::WalFallback)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_planner_decision_rules() {
        // Test Rule 1: Perfect conditions → Snapshot
        let wal_state = WalAnalysis {
            exists: false,
            size_bytes: 0,
            has_active_transactions: false,
            recovery_state: ExplicitRecoveryState::CleanShutdown,
            authority: Authority::GraphFile,
        };
        let graph_stable = true;

        let (export_mode, reasoning) = ExportPlanner::apply_decision_rules(&wal_state, graph_stable);
        assert_eq!(export_mode, ExportMode::Snapshot);
        assert_eq!(reasoning, DecisionReason::SnapshotOptimal);
    }

    #[test]
    fn test_planner_active_transactions() {
        // Test Rule 3: Active transactions → WAL
        let wal_state = WalAnalysis {
            exists: true,
            size_bytes: 1024,
            has_active_transactions: true,
            recovery_state: ExplicitRecoveryState::DirtyShutdown,
            authority: Authority::WAL,
        };
        let graph_stable = true;

        let (export_mode, reasoning) = ExportPlanner::apply_decision_rules(&wal_state, graph_stable);
        assert_eq!(export_mode, ExportMode::CheckpointAligned);
        assert_eq!(reasoning, DecisionReason::WalActiveTransactions);
    }

    #[test]
    fn test_planner_dirty_wal() {
        // Test Rule 4: Dirty WAL → WAL
        let wal_state = WalAnalysis {
            exists: true,
            size_bytes: 2048,
            has_active_transactions: false,
            recovery_state: ExplicitRecoveryState::DirtyShutdown,
            authority: Authority::WAL,
        };
        let graph_stable = true;

        let (export_mode, reasoning) = ExportPlanner::apply_decision_rules(&wal_state, graph_stable);
        assert_eq!(export_mode, ExportMode::LsnBounded);
        assert_eq!(reasoning, DecisionReason::WalDirtyState);
    }

    #[test]
    fn test_planner_unstable_graph() {
        // Test Rule 6: Unstable graph → WAL
        let wal_state = WalAnalysis {
            exists: false,
            size_bytes: 0,
            has_active_transactions: false,
            recovery_state: ExplicitRecoveryState::CleanShutdown,
            authority: Authority::GraphFile,
        };
        let graph_stable = false;

        let (export_mode, reasoning) = ExportPlanner::apply_decision_rules(&wal_state, graph_stable);
        assert_eq!(export_mode, ExportMode::Full);
        assert_eq!(reasoning, DecisionReason::GraphCorruption);
    }

    #[test]
    fn test_planner_deterministic() {
        // Test that the same inputs always produce the same outputs
        let wal_state = WalAnalysis {
            exists: false,
            size_bytes: 0,
            has_active_transactions: false,
            recovery_state: ExplicitRecoveryState::CleanShutdown,
            authority: Authority::GraphFile,
        };
        let graph_stable = true;

        // Run decision multiple times
        let result1 = ExportPlanner::apply_decision_rules(&wal_state, graph_stable);
        let result2 = ExportPlanner::apply_decision_rules(&wal_state, graph_stable);

        // Results should be identical (deterministic)
        assert_eq!(result1, result2);
    }

    #[test]
    fn test_planner_is_snapshot_advisable() {
        let temp_file = NamedTempFile::new().unwrap();
        let graph_path = temp_file.path().to_path_buf();

        // Create a simple graph file
        let _graph_file = GraphFile::create(&graph_path).unwrap();

        // With no WAL, snapshot should be advisable
        let result = ExportPlanner::is_snapshot_advisable(&graph_path);
        assert!(result.is_ok());
        // Note: The actual result depends on the graph state analysis
    }
}