# Roadmap: SQLiteGraph

## Overview

Complete and improve SQLiteGraph, an embedded graph database in Rust with dual backend support. The journey from current state to a production-ready, high-performance graph database with complete Native V2 backend, persistent HNSW vector search, advanced algorithms, and comprehensive developer tooling.

## Domain Expertise

None — No specialized domain expertise directories available. Relying on codebase documentation and Rust embedded systems patterns.

## Milestones

- **v0.2 Foundation** — Phases 1-7 (shipped 2026-01-17)
- **v1.0 Production** — Phases 8-10 (shipped 2026-01-17)
- **v1.1 ACID & Reliability** — Phases 11-21 (in progress)

---

## Phases

<details>
<summary>✅ v0.2 Foundation (Phases 1-7) - SHIPPED 2026-01-17</summary>

**Milestone Goal:** Establish production-ready foundation with Native V2 backend, HNSW vector search, MVCC, and performance optimizations.

### Phase 1: Foundation Cleanup
**Goal**: Address technical debt to improve maintainability
**Depends on**: Nothing (first phase)
**Plans**: 3 plans

Plans:
- [x] 01-01: Break down large WAL files (4,113 line operations.rs, 1,657 line rollback.rs) ✅
- [x] 01-02: Remove unused imports and dead code ✅
- [x] 01-03: Gate debug prints behind single feature flag ✅

### Phase 2: WAL Integration
**Goal**: Complete WAL recovery and checkpoint functionality
**Depends on**: Phase 1
**Plans**: 3 plans

Plans:
- [x] 02-01: Wire automatic checkpointing into commit path ✅
- [x] 02-02: Fix checkpoint V2 integration TODOs ✅
- [x] 02-03: Add WAL recovery edge case tests ✅

### Phase 3: Native V2 Reads
**Goal**: Implement read path optimizations for Native V2
**Depends on**: Phase 2
**Plans**: 3 plans

Plans:
- [x] 03-01: Implement traversal-aware cache policy (LRU-K eviction) ✅
- [x] 03-02: Compressed edge representation (delta encoding, bit-packing) ✅
- [x] 03-03: Read path performance benchmarks and validation ✅

### Phase 4: MVCC Completion
**Goal**: Fix identified MVCC gaps and edge cases
**Depends on**: Phase 3
**Plans**: 3 plans

Plans:
- [x] 04-01: Identify and document MVCC gaps ✅
- [x] 04-02: Improve snapshot isolation correctness ✅
- [x] 04-03: Add concurrent operation tests ✅

### Phase 5: HNSW Persistence
**Goal**: Enable HNSW index save/restore to disk
**Depends on**: Phase 4
**Plans**: 3 plans

Plans:
- [x] 05-01: Implement HNSW index metadata persistence ✅
- [x] 05-02: Implement vector persistence and index restore ✅
- [x] 05-03: Add comprehensive persistence tests and benchmarks ✅

### Phase 6: HNSW CLI
**Goal**: Fix HNSW indexes lost across CLI invocations
**Depends on**: Phase 5
**Plans**: 2 plans

Plans:
- [x] 06-01: Integrate persistent HNSW with CLI ✅
- [x] 06-02: Add CLI commands for index management ✅

### Phase 7: Performance
**Goal**: Optimize WAL recovery, reduce lock contention, improve benchmarks
**Depends on**: Phase 6
**Plans**: 3 plans

Plans:
- [x] 07-01: Implement parallel WAL recovery ✅
- [x] 07-02: Reduce lock contention with lock-free structures ✅
- [x] 07-03: Add comprehensive performance benchmarks ✅

</details>

<details>
<summary>✅ v1.0 Production (Phases 8-10) - SHIPPED 2026-01-17</summary>

**Milestone Goal:** Complete production-ready graph database with advanced algorithms, introspection APIs for LLM tooling, and comprehensive documentation.

### Phase 8: Graph Algorithms
**Goal**: Add centrality measures and community detection
**Depends on**: Phase 7
**Plans**: 3

Plans:
- [x] 08-01: Implement centrality algorithms (PageRank, betweenness) ✅
- [x] 08-02: Implement community detection (Louvain, label propagation) ✅
- [x] 08-03: Add algorithm benchmarks and tests ✅

### Phase 9: Developer Tooling
**Goal**: Add debugging, profiling, and introspection utilities
**Depends on**: Phase 8
**Plans**: 3

Plans:
- [x] 09-01: Add profiling/introspection APIs ✅
- [x] 09-02: Create debugging utilities ✅
- [x] 09-03: Add developer CLI commands ✅

### Phase 10: Testing & Docs
**Goal**: Comprehensive test coverage and module documentation
**Depends on**: Phase 9
**Plans**: 3

Plans:
- [x] 10-01: Fix broken WAL tests and add edge case tests ✅
- [x] 10-02: Add concurrent operation tests ✅
- [x] 10-03: Add module documentation ✅

</details>

---

### 🚧 v1.1 ACID & Reliability (In Progress)

**Milestone Goal:** Complete ACID transaction correctness for Native V2 backend and resolve all identified technical debt, security issues, and reliability concerns.

### Phase 11: ACID Atomicity
**Goal**: Complete rollback implementation for all operations, especially node deletion
**Depends on**: Phase 10
**Requirements**: ACID-01, ACID-02, ACID-03, ACID-04, ACID-05, ACID-06
**Success Criteria** (what must be TRUE):
  1. Deleting a node captures complete before-image (node record + all edges) in WAL
  2. Rollback restores deleted node to its exact previous state with all edges
  3. Crash recovery treats IN_PROGRESS transactions as ABORTED and rolls them back
  4. All rollback operations persist their state to WAL before executing
**Plans**: 3 plans

Plans:
- [x] 11-01: Implement node deletion before-image capture in WAL ✅
- [x] 11-02: Implement node deletion rollback with slot reclamation ✅
- [x] 11-03: Add WAL recovery tests for IN_PROGRESS transactions ✅

### Phase 12: ACID Consistency
**Goal**: Enable all runtime validation for data integrity
**Depends on**: Phase 11
**Requirements**: ACID-07, ACID-08, ACID-09, ACID-10, ACID-11, ACID-12
**Success Criteria** (what must be TRUE):
  1. Cluster overlap validation detects allocation corruption at runtime
  2. Checkpoint state validation detects corrupted checkpoints
  3. Pre-commit validation checks database constraints before persisting
  4. Post-recovery validation verifies database integrity after WAL replay
**Plans**: 5 plans

Plans:
- [x] 12-01: Re-enable cluster overlap validation with sequencing support ✅
- [x] 12-02: Fix checkpoint state validation to match CheckpointState enum ✅
- [x] 12-03: Add pre-commit constraint validation ✅
- [x] 12-04: Add post-recovery validation hook ✅
- [x] 12-05: Add comprehensive integrity checks ✅

### Phase 13: ACID Isolation
**Goal**: Implement transaction coordinator with deadlock detection
**Depends on**: Phase 12
**Requirements**: ACID-13, ACID-14, ACID-15, ACID-16, ACID-17, ACID-18, CW-01, CW-02, CW-03
**Success Criteria** (what must be TRUE):
  1. Transaction coordinator tracks resource-level locks for all active transactions
  2. Deadlock detection identifies cycles in wait-for graph
  3. Deadlock victim selection aborts the youngest transaction in the cycle
  4. Multiple writers can commit transactions concurrently without deadlocks
**Plans**: 4 plans

Plans:
- [x] 13-01: Implement transaction coordinator with resource-level lock tracking ✅
- [x] 13-02: Build wait-for graph and cycle detection ✅
- [x] 13-03: Add victim selection and transaction abort ✅
- [x] 13-04: Design and document lock acquisition ordering ✅

### Phase 14: ACID Durability
**Goal**: Complete all checkpoint trigger strategies
**Depends on**: Phase 13
**Requirements**: ACID-19, ACID-20, ACID-21, ACID-22, ACID-23, CP-01, CP-02, CP-03, CP-04
**Success Criteria** (what must be TRUE):
  1. Transaction-count checkpoint triggers after N transactions
  2. Size-based checkpoint triggers when WAL exceeds threshold
  3. WAL manager tracks transaction count and file size accurately
  4. All three checkpoint strategies reset counters after completion
**Plans**: 4 plans

Plans:
- [x] 14-01: Add transaction counter to WALManagerMetrics with increment in commit_transaction ✅
- [x] 14-02: Wire size-based checkpoint trigger using std::fs::metadata ✅
- [x] 14-03: Integrate counter tracking between checkpoint manager and WAL manager with reset logic ✅
- [x] 14-04: Add checkpoint configuration to NativeConfig and comprehensive tests ✅

### Phase 15: HNSW Multi-Layer
**Goal**: Implement O(log N) HNSW search with multi-layer graph
**Depends on**: Phase 14
**Requirements**: HNSW-01, HNSW-02, HNSW-03, HNSW-04, HNSW-05, HNSW-06, HNSW-07, HNSW-08, HNSW-09, HNSW-10
**Success Criteria** (what must be TRUE):
  1. HNSW insertion distributes nodes across multiple layers using exponential distribution
  2. HNSW search performs greedy descent through higher layers
  3. Multi-layer HNSW achieves O(log N) search complexity (verified by benchmarks)
  4. Multi-layer HNSW maintains >95% recall vs exact nearest neighbor
**Plans**: 4 plans

Plans:
- [x] 15-01: Wire exponential level distribution into insertion path with LevelDistributor ✅
- [x] 15-02: Add multi-layer graph structure with LayerMappings integration ✅
- [x] 15-03: Update search for greedy descent through higher layers ✅
- [x] 15-04: Add O(log N) benchmarks and layer assignment persistence ✅

### Phase 16: Memory Safety
**Goal**: Eliminate unsafe transmute and add input validation
**Depends on**: Phase 15
**Requirements**: UNSAFE-01, UNSAFE-02, UNSAFE-03, UNSAFE-04, UNSAFE-05, UNSAFE-06, UNSAFE-07, INPUT-01, INPUT-02, INPUT-03, INPUT-04
**Success Criteria** (what must be TRUE):
  1. All unsafe transmute sites replaced with Arc<RwLock<GraphFile>>
  2. Miri tests validate safety of all former transmute sites
  3. JSON payloads are limited to configurable size and depth
  4. CI runs Miri tests on every commit
**Plans**: 4 plans

Plans:
- [x] 16-01: Audit and document all 19 transmute sites ✅
- [x] 16-02: Replace checkpoint and validator transmutes with Arc<RwLock<GraphFile>> (6 sites) ✅
- [x] 16-03: Replace replayer transmutes with Arc<RwLock<GraphFile>> (13 sites) ✅
- [x] 16-04: Add Miri tests, CI integration, and JSON input validation ✅

### Phase 17: Input Validation
**Goal**: Add safe handling of external data (COMPLETED IN PHASE 16)
**Depends on**: Phase 16
**Requirements**: INPUT-01 through INPUT-04 completed in Phase 16
**Status**: COMPLETED ✅
**Note**: All INPUT requirements were satisfied in Phase 16. This phase is now redundant.

### Phase 18: Code Structure
**Goal**: Split large files for maintainability
**Depends on**: Phase 17
**Requirements**: REFAC-01, REFAC-02, REFAC-03, REFAC-04, REFAC-05, REFAC-06, REFAC-07, CLONE-01, CLONE-02, CLONE-03
**Success Criteria** (what must be TRUE):
  1. All files over 600 LOC are split into focused submodules
  2. All split modules maintain test coverage
  3. Unnecessary clone() calls are documented with findings
**Plans**: 4 plans

Plans:
- [x] 18-01: Split algo.rs (1398 LOC) into centrality, community, structure modules ✅
- [x] 18-02: Split hnsw/index.rs (2006 LOC) into API, persistence, internal modules ✅
- [x] 18-03: Split rollback.rs (1912 LOC) and validator.rs (1509 LOC) into operation-specific modules ✅
- [x] 18-04: Split checkpoint/operations.rs (1657 LOC) and complete clone audit (231 total) ✅

### Phase 19: Concurrent Features
**Goal**: Add connection pooling and concurrent write support
**Depends on**: Phase 18
**Requirements**: POOL-01, POOL-02, POOL-03
**Success Criteria** (what must be TRUE):
  1. SQLite backend uses connection pool for concurrent access
  2. Pool size is configurable via configuration
  3. Connection reuse reduces open/close overhead
**Plans**: 3 plans

Plans:
- [x] 19-01: Implement connection pool for SQLite backend ✅
- [x] 19-02: Add configurable pool size ✅
- [x] 19-03: Add benchmarks for connection reuse ✅

### Phase 20: Data Management
**Goal**: Add migration and backup/restore APIs
**Depends on**: Phase 19
**Requirements**: MIGRATE-01, MIGRATE-02, MIGRATE-03, MIGRATE-04, BACKUP-01, BACKUP-02, BACKUP-03, SCHEMA-01, SCHEMA-02, SCHEMA-03
**Success Criteria** (what must be TRUE):
  1. File migration API detects old format versions automatically
  2. Migration converts to current format atomically
  3. Backup API creates consistent snapshots of database
  4. Restore API loads snapshots and verifies integrity
**Plans**: 4 plans

Plans:
- [x] 20-01: Change schema version from u64 to u32 and bump file format to v3 ✅
- [x] 20-02: Implement file format migration API ✅
- [x] 20-03: Implement backup API ✅
- [x] 20-04: Implement restore API ✅

### Phase 21: Test Coverage
**Goal**: Comprehensive test coverage for all critical paths
**Depends on**: Phase 20
**Requirements**: TEST-WAL-01, TEST-WAL-02, TEST-WAL-03, TEST-WAL-04, TEST-CLUS-01, TEST-CLUS-02, TEST-CLUS-03, TEST-CP-01, TEST-CP-02, TEST-CP-03, TEST-HNSW-01, TEST-HNSW-02, TEST-HNSW-03, TEST-HNSW-04, TEST-MIRI-01, TEST-MIRI-02, TEST-MIRI-03, TEST-MIRI-04
**Success Criteria** (what must be TRUE):
  1. All WAL recovery tests pass including node deletion rollback
  2. All cluster validation tests are enabled and pass
  3. All checkpoint validation tests are enabled and pass
  4. All HNSW multi-layer tests pass with O(log N) verification
**Plans**: 4 plans

Plans:
- [ ] 21-01: Enable and fix WAL recovery tests
- [ ] 21-02: Enable and fix cluster validation tests
- [ ] 21-03: Enable and fix checkpoint validation tests
- [ ] 21-04: Add HNSW multi-layer and Miri tests

### Phase 22: Scaling & Dependencies
**Goal**: Address scaling limits and dependency updates
**Depends on**: Phase 21
**Requirements**: SCALE-CP-01, SCALE-CP-02, SCALE-CP-03, SCALE-DB-01, SCALE-DB-02, SCALE-DB-03, SCALE-TX-01, SCALE-TX-02, SCALE-TX-03, SCALE-HNSW-01, SCALE-HNSW-02, DEP-RUST-01, DEP-RUST-02, DEP-BIN-01, DEP-BIN-02
**Success Criteria** (what must be TRUE):
  1. Checkpoint supports files larger than 1GB
  2. Dirty block tracking handles overflow for >50,000 blocks
  3. Transaction ID bounds are enforced with cleanup
  4. Disk-based HNSW option exists for indexes larger than RAM
**Plans**: 4 plans

Plans:
- [ ] 22-01: Implement multi-file checkpointing
- [ ] 22-02: Implement dirty block overflow strategy
- [ ] 22-03: Add transaction ID bounds and cleanup
- [ ] 22-04: Plan bincode 2.0 migration and monitor rusqlite updates

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → ... → 10 → 11 → 12 → ... → 22

| Phase | Milestone | Plans | Status | Completed |
|-------|-----------|-------|--------|-----------|
| 1. Foundation Cleanup | v0.2 | 3/3 | Complete | 2026-01-17 |
| 2. WAL Integration | v0.2 | 3/3 | Complete | 2026-01-17 |
| 3. Native V2 Reads | v0.2 | 3/3 | Complete | 2026-01-17 |
| 4. MVCC Completion | v0.2 | 3/3 | Complete | 2026-01-17 |
| 5. HNSW Persistence | v0.2 | 3/3 | Complete | 2026-01-17 |
| 6. HNSW CLI | v0.2 | 2/2 | Complete | 2026-01-17 |
| 7. Performance | v0.2 | 3/3 | Complete | 2026-01-17 |
| 8. Graph Algorithms | v1.0 | 3/3 | Complete | 2026-01-17 |
| 9. Developer Tooling | v1.0 | 3/3 | Complete | 2026-01-17 |
| 10. Testing & Docs | v1.0 | 3/3 | Complete | 2026-01-17 |
| 11. ACID Atomicity | v1.1 | 3/3 | Complete | 2026-01-20 |
| 12. ACID Consistency | v1.1 | 5/5 | Complete | 2026-01-20 |
| 13. ACID Isolation | v1.1 | 4/4 | Complete | 2026-01-20 |
| 14. ACID Durability | v1.1 | 4/4 | Complete | 2026-01-20 |
| 15. HNSW Multi-Layer | v1.1 | 4/4 | Complete | 2026-01-20 |
| 16. Memory Safety | v1.1 | 4/4 | Complete | 2026-01-20 |
| 17. Input Validation | v1.1 | 0/3 | Complete* | 2026-01-20 |
| 18. Code Structure | v1.1 | 4/4 | Complete | 2026-01-20 |
| 19. Concurrent Features | v1.1 | 3/3 | Complete | 2026-01-20 |
| 20. Data Management | v1.1 | 4/4 | Complete | 2026-01-20 |
| 21. Test Coverage | v1.1 | 0/4 | Not started | - |
| 22. Scaling & Dependencies | v1.1 | 0/4 | Not started | - |
