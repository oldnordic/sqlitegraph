# Project State

## Project Reference

See: .planning/PROJECT.md (updated 2026-01-20)

**Core value:** Feature parity, performance, and reliability equally. Native V2 must match or exceed SQLite backend capabilities while maintaining rock-solid MVCC correctness and achieving best-in-class embedded graph database performance.
**Current focus:** Phase 22 - Scaling & Dependencies (v1.1 ACID & Reliability)

## Current Position

Phase: 22 (Scaling & Dependencies) - Plan 2 of 4 complete
Status: Phase 22 in progress (2/4 plans executed)
Last activity: 2026-01-20 — Completed 22-02: Dirty Block Overflow Strategy

Progress: [████████████████████████████████░] 95% (Phase 11-21 complete, 22-02 complete)

## Performance Metrics

**Velocity:**
- Total plans completed: 67 (33 for v0.2/v1.0, 34 for v1.1)
- Average duration: 8min (last 5 plans: 6min, 5min, 5min, 5min, 5min)
- Total execution time: TBD

**By Phase:**

| Phase | Plans | Total | Avg/Plan |
|-------|-------|-------|----------|
| 1-10 (completed) | 33 | TBD | TBD |
| 11-15 (v1.1) | 23/45 | TBD | TBD |

**Recent Trend:**
- Last 5 plans: 14-03 (4min), 14-04 (6min), 15-01 (8min), 15-02 (3min), 15-03 (2min), 15-04 (45min with bug fix)
- Trend: Stable
| 11-14 (v1.1) | 17/17 | 122min | 7min |
| 15 (HNSW Multi-Layer) | 4/4 | 58min | 15min |

**Recent Trend:**
- Last 5 plans: 14-02 (2min), 14-03 (4min), 14-04 (6min), 15-01 (8min), 15-02 (3min), 15-03 (2min), 15-04 (45min with bug fix)
- Trend: Stable (15-04 took longer due to bug fix and extensive debugging)

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
- Added transactions_since_checkpoint to CheckpointManagerState as pub field for strategy evaluation - 14-03
- Added checkpointed_wal_size to CheckpointManagerState for adaptive size delta calculations - 14-03
- TransactionCount strategy uses state.transactions_since_checkpoint for accurate trigger evaluation - 14-03
- Counters reset in force_checkpoint() success branch to prevent immediate re-triggering - 14-03
- on_checkpoint_completed() callback provides external notification path for counter synchronization - 14-03
- Adaptive strategy combines time interval guard with OR condition for size/transaction triggers - 14-03
- checkpoint_strategy field added to NativeConfig with Option<CheckpointStrategy> type - 14-04
- Builder methods provide convenient API: with_checkpoint_strategy, with_transaction_checkpoint, with_size_checkpoint, with_time_checkpoint - 14-04
- Tests verify all checkpoint strategies (transaction-count, size-based) and counter reset behavior - 14-04

**v1.1 Scaling (Phase 15):**
- LevelDistributor field added to HnswIndex for exponential level assignment - 15-01
- determine_insertion_level() uses P(level) = m^(-level) distribution via LevelDistributor::sample_level_internal() - 15-01
- LevelDistributor only initialized when enable_multilayer=true to avoid RNG overhead in single-layer mode - 15-01
- Deterministic seeding with default seed of 42 for reproducible behavior, configurable via multilayer_deterministic_seed - 15-01
- Base M parameter uses multilayer_level_distribution_base if set, otherwise falls back to config.m - 15-01
- determine_insertion_level signature changed from &self to &mut self for mutable RNG access - 15-01
- MultiLayerNodeManager field added to HnswIndex for tracking layer assignments and ID translation - 15-02
- insert_into_layer() uses LayerMappings.get_local_id() for ID translation in multi-layer mode, falls back to direct conversion in single-layer mode - 15-02
- insert_vector() registers with MultiLayerNodeManager before inserting into layers to ensure mappings exist - 15-02
- Multi-layer insertion flow: determine_insertion_level() -> manager.insert_vector() -> insert_into_layer() for each layer - 15-02
- Greedy descent search implemented: top layer to layer 1 uses k=1, layer 0 uses full ef_search - 15-03
- Helper methods for ID translation: get_local_id_for_layer, get_global_id_for_layer abstract single/multi-layer modes - 15-03
- load_vectors_as_array helper loads vectors once per search to avoid repeated storage access - 15-03
- Fixed critical graph connectivity bug: connections were pruned by node_id instead of distance - 15-04
- Added prune_connections_by_distance() for proper distance-based connection pruning - 15-04
- Lenient reverse connection pruning (2*M limit) maintains graph connectivity - 15-04
- Achieved 100% recall on 1000-vector test (was 10% before fix) - 15-04
- Verified O(log N) scaling: 2.90x time for 10x data (100 -> 1000 vectors) - 15-04

**v1.1 Memory Safety (Phase 16):**
- All 19 transmute sites identified and categorized as "API Redesign Needed" - 16-01
- NodeStore<'a> and EdgeStore<'a> have lifetime parameters tied to GraphFile requiring API redesign - 16-01
- Three replacement options identified: (A) Arc<RwLock<GraphFile>> API redesign, (B) Scoped lifetimes, (C) Keep with docs - 16-01
- Decision deferred: Performance impact analysis required before API redesign commitment - 16-01
- Consolidated transmute operations into documented store_helpers modules in checkpoint/operations.rs, checkpoint/record/integrator.rs, and recovery/validator.rs - 16-02
- Established consistent pattern for remaining replayer transmute sites (rollback.rs, edge_ops.rs, transaction_ops.rs) - 16-02
- All checkpoint and validation tests pass after transmute consolidation - 16-02
- Created centralized store_helpers.rs module with create_node_store() and create_edge_store() documented-safe functions - 16-03
- Replaced all 13 replayer transmute sites (rollback.rs: 7, edge_ops.rs: 3, transaction_ops.rs: 1, operations_with_problematic_tests.rs: 2) - 16-03
- Zero inline transmutes remain in WAL recovery replayer code - 16-03
- Miri CI integration with MIRIFLAGS: -Zmiri-disable-isolation -Zmiri-ignore-leaks -Zmiri-symbolic-alignment-check - 16-04
- JSON validation defaults: 10MB max size, 128 max depth (configurable via V2WALConfig) - 16-04
- Size check BEFORE parsing (prevents memory allocation), depth check AFTER parsing (prevents stack overflow) - 16-04
- All 5 Miri tests pass (store_helpers), all 20 JSON validation tests pass (malicious payloads) - 16-04
- Requirements UNSAFE-06, UNSAFE-07, INPUT-01, INPUT-02, INPUT-03, INPUT-04 satisfied - 16-04

**v1.1 Code Structure (Phase 18):**
- Used pub use re-exports in mod.rs to maintain public API surface during file splits - 18-01
- Categorized algorithms by function: centrality (pagerank, betweenness), community (louvain, label_prop), structure (components, cycles, degrees) - 18-01
- Module splitting pattern: mod.rs with pub use re-exports for clean API - 18-01
- Used include! macro instead of proper submodules to avoid Rust module system complexity - 18-02
- Module files use full crate paths for types since included in parent scope - 18-02
- Module header comments use // instead of //! to avoid doc comment errors with include! - 18-02
- Split index.rs from 2006 LOC into 4 focused files: index.rs (701), index_api.rs (602), index_internal.rs (300), index_persist.rs (482) - 18-02
- Used delegation pattern where RollbackSystem/TransactionValidator delegate to operation-specific functions in submodules - 18-03
- Split rollback.rs (1912 LOC) into 7 operation-specific modules totaling 1537 LOC - 18-03
- Split validator.rs (1509 LOC) into 7 validation-specific modules totaling 1408 LOC - 18-03
- Created rollback/ subdirectory with node_ops, edge_ops, cluster_ops, string_ops, header_ops, free_space_ops - 18-03
- Created validator/ subdirectory with node_validation, edge_validation, cluster_validation, string_validation, free_space_validation, cross_record - 18-03
- Simplified checkpoint/operations.rs from 1657 LOC to 27 LOC re-export module - 18-04
- Clone audit completed: 222 clone() calls documented, ~95% necessary for Rust ownership model - 18-04
- Only optimize clones if profiling shows hot paths; Arc clones, config clones, and RwLock snapshots are idiomatic - 18-04

**v1.1 Concurrent Features (Phase 19):**
- Use r2d2_sqlite 0.24 for compatibility with rusqlite 0.31 (0.32+ requires rusqlite 0.38+) - 19-01
- Created PoolManager wrapper instead of directly exposing r2d2::Pool for future flexibility - 19-01
- Use Arc<GraphMetrics> and Arc<StatementTracker> for shared ownership in pooled connections - 19-01
- Create ConnectionWrapper enum to unify borrowed (in-memory) and pooled (file-based) access patterns - 19-01
- Keep in-memory databases without pooling since each connection would have isolated data - 19-01
- Default pool size of 5 connections (configurable via with_max_size()) - 19-01
- Pool size configurable via SqliteConfig::with_pool_size() or with_max_connections() - 19-02
- open_with_config() reads cfg.pool_size.unwrap_or(5) and passes to PoolManager - 19-02
- All open methods delegate to config-aware versions to avoid code duplication - 19-02
- Connection pool benchmarks demonstrate 4-5x throughput improvement vs direct connections - 19-03
- Warm checkout latency: 1.7 µs (12x faster than direct Connection::open at ~20 µs) - 19-03
- Optimal pool size validated: 5-10 connections for 8 concurrent threads - 19-03
- Benchmarks measure: checkout latency, concurrent access, query throughput, pool size impact - 19-03

**v1.1 Data Management (Phase 20):**
- Schema version changed from u64 (8 bytes) to u32 (4 bytes) with 4-byte reserved field - 20-01
- File format version bumped from 2 to 3 to reflect schema_version layout change - 20-01
- Backward-compatible decode handles v2 files (8-byte schema_version) by reading lower 32 bits as schema_version, upper 32 bits as reserved - 20-01
- Migration module with FormatVersion enum (V1/V2/V3/Unknown) for format detection - 20-02
- detect_format_version reads file header and extracts version field at offset 8-11 - 20-02
- migrate_file performs atomic V2-to-V3 conversion with backup creation and rollback on failure - 20-02
- Atomic migration uses temp file + fsync + atomic rename pattern for crash safety - 20-02
- GraphFile::open triggers auto-migration for V2 files, transparent to caller - 20-02
- Backup file (.bak) retained after migration for safety, caller can delete - 20-02
- Backup API with BackupConfig (builder pattern) and BackupResult metadata - 20-03
- create_backup() wraps SnapshotExporter with optional checkpoint-before-backup via V2WALManager::force_checkpoint - 20-03
- GraphBackend::backup() method provides unified interface for both Native and SQLite backends - 20-03
- SQLite backend uses VACUUM INTO for clean backups, Native uses SnapshotExporter - 20-03
- Multi-level API: crate root create_backup(), database_backup(), backend::native::v2::backup::create_backup() - 20-03
- RestoreConfig with builder pattern (with_overwrite, with_validation, with_checksum_verification) - 20-04
- restore_backup() wraps SnapshotImporter with manifest validation and checksum verification - 20-04
- Crate-level restore_from_backup() provides convenient restore API matching create_backup() pattern - 20-04
- Multi-level restore API: crate root restore_from_backup(), database_restore(), backend::native::v2::restore_backup() - 20-04

**v1.1 Test Coverage (Phase 21):**
- Node deletion rollback tests use real handle_node_delete implementation from operations/node_ops.rs - 21-01
- Full delete/rollback cycle test validates complete rollback flow (create, delete, rollback, verify) - 21-01
- Tests added to operations/node_ops.rs instead of operations_with_problematic_tests.rs (file not in module tree) - 21-01
- All 9 node deletion tests pass (8 node delete tests + 1 full cycle test) - 21-01
- Cluster overlap validation bug fixed: Use distinct offsets for outgoing (1536) and incoming (5632) clusters - 21-02
- 2/3 cluster validation tests pass (cluster_headers_survive_reopen, file_length_consistency) - 21-02
- Fixed compilation errors: Added NativeBackendError, NativeResult, std IO imports - 21-02
- Test 1 (multi_cluster_offsets) reveals data persistence issue between API layers (deferred for architectural fix) - 21-02
- Comprehensive WAL checkpoint and recovery unit tests (6 tests) verify checkpoint manager API - 21-03
- Checkpoint strategy tests verify all 4 strategies (SizeThreshold, TransactionCount, TimeInterval, Adaptive) - 21-03
- Crash recovery tests verify recovery engine creation and state management - 21-03
- Checkpoint executor bug fix: LSN must be >= 1 validation handled - 21-03

**v1.1 Scaling & Dependencies (Phase 22):**
- Multi-file checkpoint module with segment writer/reader and parallel I/O support - 22-01
- SegmentWriter writes sequential checkpoint data to segment files with configurable size limits - 22-01
- SegmentReader reads and validates segment files with CRC checksums and index-based access - 22-01
- CheckpointSegments manages multiple segments with parallel write/read capabilities - 22-01
- DirtyBlockOverflowStrategy enum with 4 variants: Reject, ForceCheckpoint, SpillToDisk, HierarchicalPromotion - 22-02
- Default overflow strategy is Reject to maintain backward compatibility - 22-02
- DiskOverflowStore tracks spilled blocks with timestamp metadata for later recovery - 22-02
- mark_global_block_dirty handles overflow based on configured strategy - 22-02
- ForceCheckpoint returns checkpoint_required error for auto-triggering - 22-02
- SpillToDisk spills oldest 1000 blocks to overflow store when capacity exceeded - 22-02
- HierarchicalPromotion moves cluster-affinity blocks to cluster-specific tracking - 22-02
- Checkpoint manager auto-triggers checkpoint on ForceCheckpoint overflow - 22-02
- 11 overflow handling tests verify all strategies and edge cases - 22-02

### Pending Todos

- Layer persistence (Tasks 3-4 from 15-04) requires separate planning for database schema changes

### Blockers/Concerns

- Multi-layer mode has stability issues in release builds (fails at vector 378) - using single-layer mode for benchmarks
- Layer persistence (Tasks 3-4) deferred due to architectural complexity (ALTER TABLE, load/rebuild refactoring)

## Session Continuity

Last session: 2026-01-20 (plan 22-02 execution)
Stopped at: Completed 22-02 - Dirty Block Overflow Strategy
Resume file: None

### Roadmap Evolution

- **v0.2 Foundation** (2026-01-17): Phases 1-7 complete
- **v1.0 Production** (2026-01-17): Phases 8-10 complete
- **v1.1 ACID & Reliability** (2026-01-20): In Progress
  - Goal: Complete ACID transaction correctness for Native V2 backend
  - Scope: 78 requirements across 12 phases (11-22)
  - Total: 67 plans completed
  - Progress: Phase 11 complete (3/3), Phase 12 complete (5/5, verified 4/4), Phase 13 complete (4/4, verified 4/4), Phase 14 complete (4/4), Phase 15 complete (4/4, with Tasks 3-4 deferred), Phase 16 complete (4/4), Phase 18 complete (4/4), Phase 19 complete (3/3), Phase 20 complete (4/4, v3 file format with migration API, backup API, and restore API), Phase 21 complete (4/4: test coverage for WAL recovery, node deletion rollback, HNSW multi-layer, Miri integration)
  - Phase 22 in progress: 2/4 (22-01 multi-file checkpoint, 22-02 dirty block overflow)
