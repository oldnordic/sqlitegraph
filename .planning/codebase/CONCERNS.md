# Codebase Concerns

**Analysis Date:** 2026-01-20

## Tech Debt

**Incomplete HNSW Multi-Layer Implementation:**
- Issue: HNSW index only uses base layer (layer 0), multi-layer graph functionality is stubbed
- Files: `sqlitegraph/src/hnsw/index.rs:921-922`
- Impact: HNSW performance is degraded - missing O(log N) search complexity benefit from hierarchical layers
- Fix approach: Implement `determine_insertion_level()` with proper exponential distribution based on `ml` parameter and layer count

**Unimplemented Checkpoint Strategies:**
- Issue: Three checkpoint strategies return hardcoded `false` instead of actual condition evaluation
- Files: `sqlitegraph/src/backend/native/v2/wal/checkpoint/core.rs:676,679,682`
- Impact: Checkpointing only works with time-based strategy; transaction-count and size-based triggers are non-functional
- Fix approach: Implement transaction counter in WAL manager, track WAL file size, wire up to `should_checkpoint()`

**Placeholder Node Deletion in WAL Recovery:**
- Issue: Node deletion replay operation is stubbed with warning log
- Files: `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations_with_problematic_tests.rs:455-457`
- Impact: WAL recovery cannot restore node deletions, potentially causing database inconsistency after crash
- Fix approach: Implement rollback data capture during node deletion, proper slot reclamation

**Disabled Cluster Overlap Validation:**
- Issue: Cluster overlap validation is commented out due to "timing issues"
- Files: `sqlitegraph/src/backend/native/v2/node_record_v2/validation.rs:79-119`
- Impact: No runtime detection of cluster allocation corruption in node records
- Fix approach: Implement validation that accounts for allocation sequencing or add validation at allocation completion

**Checkpoint State Invariant Validation Mismatch:**
- Issue: Validation code written for different struct than actual `CheckpointState` enum
- Files: `sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/invariants.rs:236-275`
- Impact: 40 lines of commented validation code provide no invariant checking
- Fix approach: Update validation to match `CheckpointState` enum or add proper fields to enum

**Schema Version Field Size Inconsistency:**
- Issue: Schema version uses 8 bytes but comment suggests it should be 4 bytes
- Files: `sqlitegraph/src/backend/native/graph_file/encoding.rs:134`
- Impact: Wasted disk space (4 bytes per file), potential compatibility issues
- Fix approach: Migrate to 4-byte schema version field with format version bump

## Known Bugs

**Tests Written for Unimplemented Features (TDD Debt):**
- Symptoms: Multiple tests marked with "TODO: This test will fail until real implementation is complete"
- Files: `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations_with_problematic_tests.rs:838,876,904,942,976,998,1022,1064`
- Trigger: Running WAL recovery tests
- Workaround: None - tests will fail
- Note: These represent intentional TDD "failing tests" debt

## Security Considerations

**Unsafe Lifetime Transmutation:**
- Risk: `std::mem::transmute` used to extend `GraphFile` lifetime to `'static` in multiple locations
- Files:
  - `sqlitegraph/src/backend/native/v2/wal/checkpoint/operations.rs:449-450`
  - `sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:40`
  - `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/rollback.rs:142,179,224,524,629,716,890`
- Current mitigation: Comments claim this is "production pattern" when GraphFile is owned by integrator
- Recommendations:
  - Audit all transmute sites for actual lifetime guarantees
  - Consider replacing with `Arc<RwLock<GraphFile>>` pattern without lifetime transmutation
  - Add miri tests to validate safety invariants

**Deadlock Detection Not Fully Implemented:**
- Risk: Deadlock detector has placeholder parameters
- Files: `sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs:274,367`
- Current mitigation: Only transaction-level deadlock detection; resource-specific detection stubbed
- Recommendations: Implement resource-level deadlock detection or document why it's not needed

**No Input Sanitization on External Data:**
- Risk: User-provided JSON data stored without validation
- Files: Throughout graph entity/edge operations
- Current mitigation: Relies on serde_json for deserialization safety
- Recommendations: Add size limits, depth limits for JSON payloads

## Performance Bottlenecks

**Large File Exceeding Module Complexity Guidelines:**
- Problem: Multiple files exceed 600 LOC guideline, some approaching 1600 LOC
- Files:
  - `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/rollback.rs` (1654 LOC)
  - `sqlitegraph/src/hnsw/index.rs` (1605 LOC)
  - `sqlitegraph/src/backend/native/v2/wal/checkpoint/operations.rs` (1594 LOC)
  - `sqlitegraph/src/algo.rs` (1398 LOC)
  - `sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs` (1300 LOC)
- Cause: WAL recovery and HNSW implementation complexity
- Improvement path: Split into smaller submodules by responsibility

**Clone Operations:**
- Problem: 263 clone() calls detected in codebase
- Files: Throughout `sqlitegraph/src/`
- Cause: Arc/RwLock patterns requiring clone for shared access
- Improvement path: Audit clones for necessity; use references where possible

**No Connection Pooling:**
- Problem: Each graph operation may open/close connections
- Files: `sqlitegraph/src/backend/sqlite/`
- Cause: SQLite backend uses rusqlite connection per operation
- Improvement path: Implement connection pooling for concurrent operations

## Fragile Areas

**WAL Recovery Rollback System:**
- Files: `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/rollback.rs` (1654 LOC)
- Why fragile: Complex rollback operation handling with many edge cases; uses unsafe lifetime extension
- Safe modification: Add comprehensive unit tests for each rollback operation type before changes
- Test coverage: Has tests but file size suggests high complexity

**HNSW Index Core:**
- Files: `sqlitegraph/src/hnsw/index.rs` (1605 LOC)
- Why fragile: Multi-layer management partially implemented, critical vector search functionality
- Safe modification: All changes must preserve existing search contract; add regression tests
- Test coverage: Good unit test coverage in same file

**Checkpoint Integration:**
- Files: `sqlitegraph/src/backend/native/v2/wal/checkpoint/operations.rs`, `checkpoint/record/integrator.rs`
- Why fragile: Multiple unsafe transmutes, complex state management, validation commented out
- Safe modification: Review all lifetime assumptions before any changes
- Test coverage: Integration tests exist but commented validation reduces confidence

**Edge Cluster Allocation:**
- Files: `sqlitegraph/src/backend/native/v2/edge_cluster/cluster.rs`, `node_record_v2/validation.rs`
- Why fragile: Cluster overlap validation disabled, multiple DEBUG env var gates
- Safe modification: Re-enable validation before allocation logic changes
- Test coverage: Has debug flags but automated validation is disabled

## Scaling Limits

**Checkpoint File Size:**
- Current capacity: MAX_CHECKPOINT_SIZE = 1GB
- Limit: Single checkpoint file capped at 1GB
- Scaling path: Implement multi-file checkpointing or streaming checkpoint

**Dirty Block Tracking:**
- Current capacity: MAX_DIRTY_BLOCKS_PER_CLUSTER = 10,000, MAX_GLOBAL_DIRTY_BLOCKS = 50,000
- Limit: Tracking tables capped; beyond this, blocks may not be tracked
- Scaling path: Implement overflow strategy or hierarchical tracking

**WAL Transaction Coordinator:**
- Current capacity: No explicit transaction limit documented
- Limit: Deadlock detection uses in-memory HashMap; unbounded growth possible
- Scaling path: Add transaction ID bounds and cleanup verification

**HNSW Index Size:**
- Current capacity: Limited by memory only
- Limit: No disk-based HNSW storage; all vectors in memory
- Scaling path: Implement disk-based HNSW or external vector database

## Dependencies at Risk

**rusqlite 0.31:**
- Risk: Uses bundled SQLite; may have security vulnerabilities from bundled C code
- Impact: SQLite backend core dependency
- Migration plan: Monitor rusqlite updates; consider using system SQLite for security patches

**bincode 1.3:**
- Risk: Older version; bincode 2.0 has breaking changes
- Impact: Used for serialization in multiple places
- Migration plan: Plan migration to bincode 2.0 with format version bump

## Missing Critical Features

**Concurrent Write Support:**
- Problem: Native V2 backend has WAL but concurrent writes may conflict
- Blocks: Multi-writer scenarios, high throughput writes
- Status: Transaction coordinator exists but deadlock detection incomplete

**Graph File Migration:**
- Problem: No automated migration path between storage format versions
- Blocks: Seamless upgrades between versions
- Status: Manual export/import required

**Backup/Restore API:**
- Problem: No native backup API for V2 backend (snapshot system exists but high-level API missing)
- Blocks: Production deployment without external backup tools
- Status: Low-level snapshot functions available

**Node Deletion WAL Replay:**
- Problem: Node deletion replay not implemented (see Tech Debt)
- Blocks: Full crash recovery consistency
- Status: Stub implementation

## Test Coverage Gaps

**WAL Recovery Edge Cases:**
- What's not tested: Multiple test cases marked as "will fail until implementation complete" - node deletion rollback scenarios
- Files: `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations_with_problematic_tests.rs`
- Risk: Crash recovery may not properly restore state after node deletions
- Priority: High - data consistency risk

**Cluster Overlap Validation:**
- What's not tested: Validation code is commented out entirely
- Files: `sqlitegraph/src/backend/native/v2/node_record_v2/validation.rs:79-119`
- Risk: Silent data corruption if cluster allocation bugs exist
- Priority: High - corruption detection disabled

**Checkpoint State Transitions:**
- What's not tested: Checkpoint state invariants validation is commented out
- Files: `sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/invariants.rs:236-275`
- Risk: Checkpoint corruption may go undetected
- Priority: Medium - has other validation layers

**HNSW Multi-Layer:**
- What's not tested: Multi-layer insertion and search (only layer 0 used)
- Files: `sqlitegraph/src/hnsw/index.rs`
- Risk: HNSW performance not optimal; algorithm not fully implemented
- Priority: Medium - functional but suboptimal

**Unsafe Block Testing:**
- What's not tested: Unsafe lifetime transmute not validated with miri
- Files: Multiple files using `std::mem::transmute`
- Risk: Undefined behavior in edge cases
- Priority: High - memory safety

---

*Concerns audit: 2026-01-20*
