| 12. ACID Consistency | v1.1 | 5 planned# Roadmap: SQLiteGraph
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned## Overview
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedComplete and improve SQLiteGraph, an embedded graph database in Rust with dual backend support. The journey from current state to a production-ready, high-performance graph database with complete Native V2 backend, persistent HNSW vector search, advanced algorithms, and comprehensive developer tooling.
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned## Domain Expertise
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedNone — No specialized domain expertise directories available. Relying on codebase documentation and Rust embedded systems patterns.
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned## Milestones
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned- ✅ **v0.2 Foundation** — Phases 1-7 (shipped 2026-01-17)
| 12. ACID Consistency | v1.1 | 5 planned- ✅ **v1.0 Production** — Phases 8-10 (shipped 2026-01-17)
| 12. ACID Consistency | v1.1 | 5 planned- 🚧 **v1.1 ACID & Reliability** — Phases 11-21 (in progress)
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned---
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned## Phases
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned<details>
| 12. ACID Consistency | v1.1 | 5 planned<summary>✅ v0.2 Foundation (Phases 1-7) - SHIPPED 2026-01-17</summary>
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned**Milestone Goal:** Establish production-ready foundation with Native V2 backend, HNSW vector search, MVCC, and performance optimizations.
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 1: Foundation Cleanup
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Address technical debt to improve maintainability
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Nothing (first phase)
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 3 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [x] 01-01: Break down large WAL files (4,113 line operations.rs, 1,657 line rollback.rs) ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 01-02: Remove unused imports and dead code ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 01-03: Gate debug prints behind single feature flag ✅
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 2: WAL Integration
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Complete WAL recovery and checkpoint functionality
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 1
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 3 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [x] 02-01: Wire automatic checkpointing into commit path ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 02-02: Fix checkpoint V2 integration TODOs ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 02-03: Add WAL recovery edge case tests ✅
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 3: Native V2 Reads
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Implement read path optimizations for Native V2
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 2
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 3 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [x] 03-01: Implement traversal-aware cache policy (LRU-K eviction) ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 03-02: Compressed edge representation (delta encoding, bit-packing) ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 03-03: Read path performance benchmarks and validation ✅
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 4: MVCC Completion
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Fix identified MVCC gaps and edge cases
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 3
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 3 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [x] 04-01: Identify and document MVCC gaps ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 04-02: Improve snapshot isolation correctness ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 04-03: Add concurrent operation tests ✅
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 5: HNSW Persistence
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Enable HNSW index save/restore to disk
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 4
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 3 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [x] 05-01: Implement HNSW index metadata persistence ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 05-02: Implement vector persistence and index restore ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 05-03: Add comprehensive persistence tests and benchmarks ✅
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 6: HNSW CLI
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Fix HNSW indexes lost across CLI invocations
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 5
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 2 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [x] 06-01: Integrate persistent HNSW with CLI ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 06-02: Add CLI commands for index management ✅
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 7: Performance
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Optimize WAL recovery, reduce lock contention, improve benchmarks
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 6
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 3 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [x] 07-01: Implement parallel WAL recovery ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 07-02: Reduce lock contention with lock-free structures ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 07-03: Add comprehensive performance benchmarks ✅
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned</details>
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned<details>
| 12. ACID Consistency | v1.1 | 5 planned<summary>✅ v1.0 Production (Phases 8-10) - SHIPPED 2026-01-17</summary>
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned**Milestone Goal:** Complete production-ready graph database with advanced algorithms, introspection APIs for LLM tooling, and comprehensive documentation.
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 8: Graph Algorithms
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Add centrality measures and community detection
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 7
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 3
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [x] 08-01: Implement centrality algorithms (PageRank, betweenness) ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 08-02: Implement community detection (Louvain, label propagation) ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 08-03: Add algorithm benchmarks and tests ✅
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 9: Developer Tooling
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Add debugging, profiling, and introspection utilities
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 8
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 3
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [x] 09-01: Add profiling/introspection APIs ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 09-02: Create debugging utilities ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 09-03: Add developer CLI commands ✅
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 10: Testing & Docs
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Comprehensive test coverage and module documentation
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 9
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 3
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [x] 10-01: Fix broken WAL tests and add edge case tests ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 10-02: Add concurrent operation tests ✅
| 12. ACID Consistency | v1.1 | 5 planned- [x] 10-03: Add module documentation ✅
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned</details>
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned---
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### 🚧 v1.1 ACID & Reliability (In Progress)
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned**Milestone Goal:** Complete ACID transaction correctness for Native V2 backend and resolve all identified technical debt, security issues, and reliability concerns.
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 11: ACID Atomicity
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Complete rollback implementation for all operations, especially node deletion
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 10
| 12. ACID Consistency | v1.1 | 5 planned**Requirements**: ACID-01, ACID-02, ACID-03, ACID-04, ACID-05, ACID-06
| 12. ACID Consistency | v1.1 | 5 planned**Success Criteria** (what must be TRUE):
| 12. ACID Consistency | v1.1 | 5 planned  1. Deleting a node captures complete before-image (node record + all edges) in WAL
| 12. ACID Consistency | v1.1 | 5 planned  2. Rollback restores deleted node to its exact previous state with all edges
| 12. ACID Consistency | v1.1 | 5 planned  3. Crash recovery treats IN_PROGRESS transactions as ABORTED and rolls them back
| 12. ACID Consistency | v1.1 | 5 planned  4. All rollback operations persist their state to WAL before executing
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 3 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 11-01: Implement node deletion before-image capture in WAL
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 11-02: Implement node deletion rollback with slot reclamation
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 11-03: Add WAL recovery tests for IN_PROGRESS transactions
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 12: ACID Consistency
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Enable all runtime validation for data integrity
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 11
| 12. ACID Consistency | v1.1 | 5 planned**Requirements**: ACID-07, ACID-08, ACID-09, ACID-10, ACID-11, ACID-12
| 12. ACID Consistency | v1.1 | 5 planned**Success Criteria** (what must be TRUE):
| 12. ACID Consistency | v1.1 | 5 planned  1. Cluster overlap validation detects allocation corruption at runtime
| 12. ACID Consistency | v1.1 | 5 planned  2. Checkpoint state validation detects corrupted checkpoints
| 12. ACID Consistency | v1.1 | 5 planned  3. Pre-commit validation checks database constraints before persisting
| 12. ACID Consistency | v1.1 | 5 planned  4. Post-recovery validation verifies database integrity after WAL replay
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 5 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 12-01: Re-enable cluster overlap validation with sequencing support
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 12-02: Fix checkpoint state validation to match CheckpointState enum
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 12-03: Add pre-commit constraint validation
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 12-04: Add post-recovery validation hook
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 12-05: Add comprehensive integrity checks
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 13: ACID Isolation
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Implement transaction coordinator with deadlock detection
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 12
| 12. ACID Consistency | v1.1 | 5 planned**Requirements**: ACID-13, ACID-14, ACID-15, ACID-16, ACID-17, ACID-18, CW-01, CW-02, CW-03
| 12. ACID Consistency | v1.1 | 5 planned**Success Criteria** (what must be TRUE):
| 12. ACID Consistency | v1.1 | 5 planned  1. Transaction coordinator tracks resource-level locks for all active transactions
| 12. ACID Consistency | v1.1 | 5 planned  2. Deadlock detection identifies cycles in wait-for graph
| 12. ACID Consistency | v1.1 | 5 planned  3. Deadlock victim selection aborts the youngest transaction in the cycle
| 12. ACID Consistency | v1.1 | 5 planned  4. Multiple writers can commit transactions concurrently without deadlocks
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 5 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 13-01: Implement transaction coordinator with resource-level lock tracking
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 13-02: Build wait-for graph and cycle detection
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 13-03: Add victim selection and transaction abort
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 13-04: Design and document lock acquisition ordering
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 14: ACID Durability
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Complete all checkpoint trigger strategies
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 13
| 12. ACID Consistency | v1.1 | 5 planned**Requirements**: ACID-19, ACID-20, ACID-21, ACID-22, ACID-23, CP-01, CP-02, CP-03, CP-04
| 12. ACID Consistency | v1.1 | 5 planned**Success Criteria** (what must be TRUE):
| 12. ACID Consistency | v1.1 | 5 planned  1. Transaction-count checkpoint triggers after N transactions
| 12. ACID Consistency | v1.1 | 5 planned  2. Size-based checkpoint triggers when WAL exceeds threshold
| 12. ACID Consistency | v1.1 | 5 planned  3. WAL manager tracks transaction count and file size accurately
| 12. ACID Consistency | v1.1 | 5 planned  4. All three checkpoint strategies reset counters after completion
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 5 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 14-01: Wire transaction-count checkpoint trigger
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 14-02: Wire size-based checkpoint trigger
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 14-03: Add WAL metrics tracking (count and size)
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 14-04: Add tests for all checkpoint strategies
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 15: HNSW Multi-Layer
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Implement O(log N) HNSW search with multi-layer graph
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 14
| 12. ACID Consistency | v1.1 | 5 planned**Requirements**: HNSW-01, HNSW-02, HNSW-03, HNSW-04, HNSW-05, HNSW-06, HNSW-07, HNSW-08, HNSW-09, HNSW-10
| 12. ACID Consistency | v1.1 | 5 planned**Success Criteria** (what must be TRUE):
| 12. ACID Consistency | v1.1 | 5 planned  1. HNSW insertion distributes nodes across multiple layers using exponential distribution
| 12. ACID Consistency | v1.1 | 5 planned  2. HNSW search performs greedy descent through higher layers
| 12. ACID Consistency | v1.1 | 5 planned  3. Multi-layer HNSW achieves O(log N) search complexity (verified by benchmarks)
| 12. ACID Consistency | v1.1 | 5 planned  4. Multi-layer HNSW maintains >95% recall vs exact nearest neighbor
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 5 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 15-01: Implement determine_insertion_level with exponential distribution
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 15-02: Add multi-layer graph structure
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 15-03: Update insert to add nodes to all layers 0..=target_layer
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 15-04: Update search for greedy descent and O(log N) benchmarks
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 16: Memory Safety
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Eliminate unsafe transmute and add input validation
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 15
| 12. ACID Consistency | v1.1 | 5 planned**Requirements**: UNSAFE-01, UNSAFE-02, UNSAFE-03, UNSAFE-04, UNSAFE-05, UNSAFE-06, UNSAFE-07, INPUT-01, INPUT-02, INPUT-03, INPUT-04
| 12. ACID Consistency | v1.1 | 5 planned**Success Criteria** (what must be TRUE):
| 12. ACID Consistency | v1.1 | 5 planned  1. All unsafe transmute sites replaced with Arc<RwLock<GraphFile>>
| 12. ACID Consistency | v1.1 | 5 planned  2. Miri tests validate safety of all former transmute sites
| 12. ACID Consistency | v1.1 | 5 planned  3. JSON payloads are limited to configurable size and depth
| 12. ACID Consistency | v1.1 | 5 planned  4. CI runs Miri tests on every commit
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 5 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 16-01: Audit and document all 10+ transmute sites
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 16-02: Replace checkpoint/operations.rs transmute with Arc<RwLock<GraphFile>>
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 16-03: Replace replayer/rollback.rs transmute sites (6)
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 16-04: Add Miri tests and CI integration
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 17: Input Validation
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Add safe handling of external data
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 16
| 12. ACID Consistency | v1.1 | 5 planned**Requirements**: INPUT-01, INPUT-02, INPUT-03, INPUT-04
| 12. ACID Consistency | v1.1 | 5 planned**Success Criteria** (what must be TRUE):
| 12. ACID Consistency | v1.1 | 5 planned  1. JSON payloads larger than configured limit are rejected
| 12. ACID Consistency | v1.1 | 5 planned  2. JSON payloads deeper than configured limit are rejected
| 12. ACID Consistency | v1.1 | 5 planned  3. Malicious payload tests cover edge cases
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 3 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 17-01: Add JSON size limit validation
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 17-02: Add JSON depth limit validation
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 17-03: Add malicious payload tests
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 18: Code Structure
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Split large files for maintainability
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 17
| 12. ACID Consistency | v1.1 | 5 planned**Requirements**: REFAC-01, REFAC-02, REFAC-03, REFAC-04, REFAC-05, REFAC-06, REFAC-07, CLONE-01, CLONE-02, CLONE-03
| 12. ACID Consistency | v1.1 | 5 planned**Success Criteria** (what must be TRUE):
| 12. ACID Consistency | v1.1 | 5 planned  1. All files over 600 LOC are split into focused submodules
| 12. ACID Consistency | v1.1 | 5 planned  2. All split modules maintain test coverage
| 12. ACID Consistency | v1.1 | 5 planned  3. Unnecessary clone() calls are replaced with references
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 5 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 18-01: Split rollback.rs (1654 LOC) into submodules
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 18-02: Split hnsw/index.rs (1605 LOC) into modules
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 18-03: Split checkpoint/operations.rs (1594 LOC) and algo.rs (1398 LOC)
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 18-04: Audit and reduce clone() calls (263 total)
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 19: Concurrent Features
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Add connection pooling and concurrent write support
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 18
| 12. ACID Consistency | v1.1 | 5 planned**Requirements**: POOL-01, POOL-02, POOL-03
| 12. ACID Consistency | v1.1 | 5 planned**Success Criteria** (what must be TRUE):
| 12. ACID Consistency | v1.1 | 5 planned  1. SQLite backend uses connection pool for concurrent access
| 12. ACID Consistency | v1.1 | 5 planned  2. Pool size is configurable via configuration
| 12. ACID Consistency | v1.1 | 5 planned  3. Connection reuse reduces open/close overhead
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 3 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 19-01: Implement connection pool for SQLite backend
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 19-02: Add configurable pool size
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 19-03: Add benchmarks for connection reuse
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 20: Data Management
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Add migration and backup/restore APIs
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 19
| 12. ACID Consistency | v1.1 | 5 planned**Requirements**: MIGRATE-01, MIGRATE-02, MIGRATE-03, MIGRATE-04, BACKUP-01, BACKUP-02, BACKUP-03, SCHEMA-01, SCHEMA-02, SCHEMA-03
| 12. ACID Consistency | v1.1 | 5 planned**Success Criteria** (what must be TRUE):
| 12. ACID Consistency | v1.1 | 5 planned  1. File migration API detects old format versions automatically
| 12. ACID Consistency | v1.1 | 5 planned  2. Migration converts to current format atomically
| 12. ACID Consistency | v1.1 | 5 planned  3. Backup API creates consistent snapshots of database
| 12. ACID Consistency | v1.1 | 5 planned  4. Restore API loads snapshots and verifies integrity
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 5 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 20-01: Implement file format migration API
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 20-02: Update schema version to 4-byte field
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 20-03: Implement backup/restore API
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 20-04: Add migration and backup tests
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 21: Test Coverage
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Comprehensive test coverage for all critical paths
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 20
| 12. ACID Consistency | v1.1 | 5 planned**Requirements**: TEST-WAL-01, TEST-WAL-02, TEST-WAL-03, TEST-WAL-04, TEST-CLUS-01, TEST-CLUS-02, TEST-CLUS-03, TEST-CP-01, TEST-CP-02, TEST-CP-03, TEST-HNSW-01, TEST-HNSW-02, TEST-HNSW-03, TEST-HNSW-04, TEST-MIRI-01, TEST-MIRI-02, TEST-MIRI-03, TEST-MIRI-04
| 12. ACID Consistency | v1.1 | 5 planned**Success Criteria** (what must be TRUE):
| 12. ACID Consistency | v1.1 | 5 planned  1. All WAL recovery tests pass including node deletion rollback
| 12. ACID Consistency | v1.1 | 5 planned  2. All cluster validation tests are enabled and pass
| 12. ACID Consistency | v1.1 | 5 planned  3. All checkpoint validation tests are enabled and pass
| 12. ACID Consistency | v1.1 | 5 planned  4. All HNSW multi-layer tests pass with O(log N) verification
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 5 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 21-01: Enable and fix WAL recovery tests
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 21-02: Enable and fix cluster validation tests
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 21-03: Enable and fix checkpoint validation tests
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 21-04: Add HNSW multi-layer and Miri tests
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned### Phase 22: Scaling & Dependencies
| 12. ACID Consistency | v1.1 | 5 planned**Goal**: Address scaling limits and dependency updates
| 12. ACID Consistency | v1.1 | 5 planned**Depends on**: Phase 21
| 12. ACID Consistency | v1.1 | 5 planned**Requirements**: SCALE-CP-01, SCALE-CP-02, SCALE-CP-03, SCALE-DB-01, SCALE-DB-02, SCALE-DB-03, SCALE-TX-01, SCALE-TX-02, SCALE-TX-03, SCALE-HNSW-01, SCALE-HNSW-02, DEP-RUST-01, DEP-RUST-02, DEP-BIN-01, DEP-BIN-02
| 12. ACID Consistency | v1.1 | 5 planned**Success Criteria** (what must be TRUE):
| 12. ACID Consistency | v1.1 | 5 planned  1. Checkpoint supports files larger than 1GB
| 12. ACID Consistency | v1.1 | 5 planned  2. Dirty block tracking handles overflow for >50,000 blocks
| 12. ACID Consistency | v1.1 | 5 planned  3. Transaction ID bounds are enforced with cleanup
| 12. ACID Consistency | v1.1 | 5 planned  4. Disk-based HNSW option exists for indexes larger than RAM
| 12. ACID Consistency | v1.1 | 5 planned**Plans**: 5 plans
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 plannedPlans:
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 22-01: Implement multi-file checkpointing
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 22-02: Implement dirty block overflow strategy
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 22-03: Add transaction ID bounds and cleanup
| 12. ACID Consistency | v1.1 | 5 planned- [ ] 22-04: Plan bincode 2.0 migration and monitor rusqlite updates
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned## Progress
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned**Execution Order:**
| 12. ACID Consistency | v1.1 | 5 plannedPhases execute in numeric order: 1 → 2 → 3 → ... → 10 → 11 → 12 → ... → 22
| 12. ACID Consistency | v1.1 | 5 planned
| 12. ACID Consistency | v1.1 | 5 planned| Phase | Milestone | Plans | Status | Completed |
| 12. ACID Consistency | v1.1 | 5 planned|-------|-----------|-------|--------|-----------|
| 12. ACID Consistency | v1.1 | 5 planned| 1. Foundation Cleanup | v0.2 | 3/3 | Complete | 2026-01-17 |
| 12. ACID Consistency | v1.1 | 5 planned| 2. WAL Integration | v0.2 | 3/3 | Complete | 2026-01-17 |
| 12. ACID Consistency | v1.1 | 5 planned| 3. Native V2 Reads | v0.2 | 3/3 | Complete | 2026-01-17 |
| 12. ACID Consistency | v1.1 | 5 planned| 4. MVCC Completion | v0.2 | 3/3 | Complete | 2026-01-17 |
| 12. ACID Consistency | v1.1 | 5 planned| 5. HNSW Persistence | v0.2 | 3/3 | Complete | 2026-01-17 |
| 12. ACID Consistency | v1.1 | 5 planned| 6. HNSW CLI | v0.2 | 2/2 | Complete | 2026-01-17 |
| 12. ACID Consistency | v1.1 | 5 planned| 7. Performance | v0.2 | 3/3 | Complete | 2026-01-17 |
| 12. ACID Consistency | v1.1 | 5 planned| 8. Graph Algorithms | v1.0 | 3/3 | Complete | 2026-01-17 |
| 12. ACID Consistency | v1.1 | 5 planned| 9. Developer Tooling | v1.0 | 3/3 | Complete | 2026-01-17 |
| 12. ACID Consistency | v1.1 | 5 planned| 10. Testing & Docs | v1.0 | 3/3 | Complete | 2026-01-17 |
| 12. ACID Consistency | v1.1 | 5 planned| 11. ACID Atomicity | v1.1 | 3/3 | Complete | 2026-01-20 |
| 12. ACID Consistency | v1.1 | 5 planned| 12. ACID Consistency | v1.1 | 4 planned | Planned | 2026-01-20 |
| 12. ACID Consistency | v1.1 | 5 planned| 13. ACID Isolation | v1.1 | 0/4 | Not started | - |
| 12. ACID Consistency | v1.1 | 5 planned| 14. ACID Durability | v1.1 | 0/4 | Not started | - |
| 12. ACID Consistency | v1.1 | 5 planned| 15. HNSW Multi-Layer | v1.1 | 0/4 | Not started | - |
| 12. ACID Consistency | v1.1 | 5 planned| 16. Memory Safety | v1.1 | 0/4 | Not started | - |
| 12. ACID Consistency | v1.1 | 5 planned| 17. Input Validation | v1.1 | 0/3 | Not started | - |
| 12. ACID Consistency | v1.1 | 5 planned| 18. Code Structure | v1.1 | 0/4 | Not started | - |
| 12. ACID Consistency | v1.1 | 5 planned| 19. Concurrent Features | v1.1 | 0/3 | Not started | - |
| 12. ACID Consistency | v1.1 | 5 planned| 20. Data Management | v1.1 | 0/4 | Not started | - |
| 12. ACID Consistency | v1.1 | 5 planned| 21. Test Coverage | v1.1 | 0/4 | Not started | - |
| 12. ACID Consistency | v1.1 | 5 planned| 22. Scaling & Dependencies | v1.1 | 0/4 | Not started | - |
