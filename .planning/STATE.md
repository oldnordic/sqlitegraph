# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-01-20)

**Core value:** Feature parity, performance, and reliability equally. Native V2 must match or exceed SQLite backend capabilities while maintaining rock-solid MVCC correctness and achieving best-in-class embedded graph database performance.
**Current focus:** Phase 14 - ACID Durability (v1.1 ACID & Reliability)

## Current Position

Phase: 14 of 22 (ACID Durability)
Plan: 2 of 4 in current phase
Status: Phase 14 plan 02 complete, plan 03 pending
Last activity: 2026-01-20 — Completed 14-02 size-based checkpoint trigger

Progress: [██████████████████████████████████ ] 80%

## Performance Metrics

**Velocity:**
- Total plans completed: 44
- Average duration: TBD
- Total execution time: TBD

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1-10 (completed) | 33 | TBD | TBD |
| 11-14 (v1.1) | 15/45 | 112min | 7min |

**Recent Trend:**
- Last 5 plans: 13-01 (19min), 13-02 (3min), 13-03 (6min), 13-04 (3min), 14-01 (2min)
- Trend: Stable

*Updated after each plan completion*

## Accumulated Context

### Decisions

Decisions are logged in PROJECT.md Key Decisions table.
Recent decisions affecting current work:

- Phase 1-10: Established production-ready foundation with Native V2 backend, HNSW persistence, graph algorithms, and developer tooling
- Phase 11-22: Focused on completing ACID guarantees, memory safety, code structure, and scaling

**v1.0 Key Decisions:**
- LRU-K traversal-aware cache for edge clusters (100% hit ratio achieved)
- Delta encoding and bit-packing for edge compression (30-50% memory reduction)
- Metadata-first HNSW persistence approach
- Parallel WAL recovery using rayon (2-3x speedup for large WALs)
- Lock-free atomic statistics (AtomicU64 counters)

**v1.1 Key Decisions:**
- Use CompactEdgeRecord binary serialization for edge data in WAL (not JSON) - 11-01
- Capture edges BEFORE cascade deletion to preserve data for rollback - 11-01
- Replace serde_json with NodeRecordV2::serialize/deserialize for consistency - 11-01
- EdgeCluster::create_from_compact_edges for cluster restoration during rollback - 11-02
- FreeSpaceManager::remove_from_free_list for slot reclamation during rollback - 11-02
- Rollback state persistence deferred to Phase 13+ (memory-only acceptable for recovery replay) - 11-03
- IN_PROGRESS transactions filtered by `committed=true && commit_lsn.is_some()` - 11-03
- Bidirectional cluster overlap check: `incoming_offset < outgoing_end && outgoing_offset < incoming_end` - 12-01
- Calculate actual overlap_size and only error if > 0 to allow adjacent clusters - 12-01
- Only validate when both cluster offsets > 0 to prevent false positives during sequential allocation - 12-01
- Made CheckpointManagerState public with pub fields to allow validation access - 12-02
- State validation checks consistency between CheckpointState enum and CheckpointManagerState metadata - 12-02
- Pre-commit validation hook validates transaction constraints before commit - 12-03
- Post-recovery validation hook uses RecoveryValidator after WAL replay completes - 12-04
- validate_post_recovery called between replay_transactions and finalize_recovery - 12-04
- Store graph_file_path in RecoveryValidator for database-level validation - 12-05
- Only run database integrity checks when perform_consistency_checks is enabled - 12-05
- Validate node_count consistency against transactions_replayed count - 12-05
- Synchronous transaction coordinator eliminates tokio runtime dependency - 13-01
- Unified IsolationLevel enum across coordinator and manager (includes Snapshot variant) - 13-01
- Wait-for graph edges added synchronously when Exclusive lock acquisition fails - 13-02
- Deadlock detection runs AFTER wait edges are added (post-check, not pre-check) - 13-02
- All transaction exit paths (commit, rollback, cleanup) remove wait-for graph entries - 13-02
- Victim selection uses max_by_key on (start_time, tx_id) to select youngest transaction - 13-03
- Non-victim transactions automatically retry lock acquisition after victim abort - 13-03
- abort_victim writes TransactionAbort WAL record with reason "deadlock_victim" - 13-03
- Added transactions_since_checkpoint field to WALManagerMetrics as resettable counter - 14-01
- Counter increments in commit_transaction after committed_transactions increment - 14-01
- Public accessor get_transactions_since_checkpoint() exposes counter to checkpoint manager - 14-01
- SizeThreshold checkpoint strategy reads actual WAL file size via std::fs::metadata().len() - 14-02
- get_wal_size() helper method exposes WAL size for external monitoring - 14-02
- estimate_wal_size() in manager.rs confirmed correct - uses std::fs::metadata with metrics fallback - 14-02

### Pending Todos

None yet.

### Blockers/Concerns

None yet.

## Session Continuity

Last session: 2026-01-20 (plan 14-02 execution)
Stopped at: Completed 14-02 - Size-based checkpoint trigger
Resume file: None

### Roadmap Evolution

- **v0.2 Foundation** (2026-01-17): Phases 1-7 complete
- **v1.0 Production** (2026-01-17): Phases 8-10 complete
- **v1.1 ACID & Reliability** (2026-01-20): IN PROGRESS
  - Goal: Complete ACID transaction correctness for Native V2 backend
  - Scope: 78 requirements across 12 phases (11-22)
  - Total: 45 plans for v1.1 milestone
  - Progress: Phase 11 complete (3/3), Phase 12 complete (5/5, verified 4/4), Phase 13 complete (4/4, verified 4/4), Phase 14 in progress (2/4)
