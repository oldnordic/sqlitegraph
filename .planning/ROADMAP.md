# Roadmap: SQLiteGraph

## Overview

Complete and improve SQLiteGraph, an embedded graph database in Rust with dual backend support. The journey from current state to a production-ready, high-performance graph database with complete Native V2 backend, persistent HNSW vector search, advanced algorithms, and comprehensive developer tooling.

## Domain Expertise

None — No specialized domain expertise directories available. Relying on codebase documentation and Rust embedded systems patterns.

## Milestones

- ✅ **v0.2 Foundation** — Phases 1-7 (shipped 2026-01-17)
- 🚧 **v1.0 Production** — Phases 8-10 (in progress)

---

## Phases

<details>
<summary>✅ v0.2 Foundation (Phases 1-7) - SHIPPED 2026-01-17</summary>

**Milestone Goal:** Establish production-ready foundation with Native V2 backend, HNSW vector search, MVCC, and performance optimizations.

### Phase 1: Foundation Cleanup
**Goal**: Address technical debt to improve maintainability
**Depends on**: Nothing (first phase)
**Research**: Unlikely (internal code cleanup)
**Plans**: 3 plans

Plans:
- [x] 01-01: Break down large WAL files (4,113 line operations.rs, 1,657 line rollback.rs) ✅
- [x] 01-02: Remove unused imports and dead code ✅
- [x] 01-03: Gate debug prints behind single feature flag ✅

### Phase 2: WAL Integration
**Goal**: Complete WAL recovery and checkpoint functionality
**Depends on**: Phase 1
**Research**: Unlikely (internal code integration, architecture understood)
**Plans**: 3 plans

Plans:
- [x] 02-01: Wire automatic checkpointing into commit path ✅
- [x] 02-02: Fix checkpoint V2 integration TODOs ✅
- [x] 02-03: Add WAL recovery edge case tests ✅

### Phase 3: Native V2 Reads
**Goal**: Implement read path optimizations for Native V2 (NOT BETrees - see docs/BETREE_RESEARCH.md)
**Depends on**: Phase 2
**Research**: Complete (betree research concluded: inappropriate for graph DB workloads)
**Research topics**: Read path caching, compression for cache efficiency, traversal-aware optimizations
**Plans**: 3 plans

Plans:
- [x] 03-01: Implement traversal-aware cache policy (LRU-K eviction) ✅
- [x] 03-02: Compressed edge representation (delta encoding, bit-packing) ✅
- [x] 03-03: Read path performance benchmarks and validation ✅

**Status**: Complete ✅ (2026-01-17)
**Performance**: See docs/PHASE3_PERFORMANCE_REPORT.md
**Key Results**:
- Cache hit ratio: 100% for BFS (exceeds 60% target by 67%)
- Compression ratio: 30-50% memory reduction (exceeds 1.5x target)
- Benchmark suite: 22 benchmarks with regression detection

**IMPORTANT**: BETrees were evaluated and REJECTED for primary storage due to 20-50% read performance degradation. Current clustered adjacency is already optimal for graph workloads. See docs/BETREE_RESEARCH.md for full analysis.

### Phase 4: MVCC Completion
**Goal**: Fix identified MVCC gaps and edge cases
**Depends on**: Phase 3
**Research**: Likely (MVCC patterns, snapshot isolation edge cases)
**Research topics**: Concurrent read/write patterns, snapshot lifecycle management
**Plans**: 3 plans

Plans:
- [x] 04-01: Identify and document MVCC gaps ✅
- [x] 04-02: Improve snapshot isolation correctness ✅
- [x] 04-03: Add concurrent operation tests ✅

### Phase 5: HNSW Persistence
**Goal**: Enable HNSW index save/restore to disk
**Depends on**: Phase 4
**Research**: Complete (schema exists, pragmatic approach documented)
**Research topics**: HNSW serialization format, incremental index updates, recovery from corruption
**Plans**: 3 plans

**Approach**: Pragmatic vector storage + index rebuild
- Store vectors as BLOB in hnsw_vectors table
- On load: read vectors, rebuild HNSW graph structure (O(N log N) rebuild cost)
- Simpler than full graph serialization to hnsw_layers table

Plans:
- [x] 05-01: Implement HNSW index metadata persistence ✅
- [x] 05-02: Implement vector persistence and index restore ✅
- [x] 05-03: Add comprehensive persistence tests and benchmarks ✅

### Phase 6: HNSW CLI
**Goal**: Fix HNSW indexes lost across CLI invocations
**Depends on**: Phase 5
**Research**: Unlikely (builds on Phase 5 persistence)
**Plans**: 2 plans

Plans:
- [x] 06-01: Integrate persistent HNSW with CLI ✅
- [x] 06-02: Add CLI commands for index management ✅

**Key Results**:
- Index metadata persists across CLI invocations
- New commands: `hnsw-list`, `hnsw-delete`, `hnsw-info`
- Added `--index-name` parameter for custom index names

**Known Limitation**: Vector persistence requires Connection sharing architecture (documented in 06-01-SUMMARY.md)

### Phase 7: Performance
**Goal**: Optimize WAL recovery, reduce lock contention, improve benchmarks
**Depends on**: Phase 6
**Research**: Likely (parallel recovery patterns, lock-free data structures)
**Research topics**: Parallel WAL replay strategies, lock-free snapshot updates, profiling tools
**Plans**: 3 plans

Plans:
- [x] 07-01: Implement parallel WAL recovery ✅
- [x] 07-02: Reduce lock contention with lock-free structures ✅
- [x] 07-03: Add comprehensive performance benchmarks ✅

**Key Results**:
- Parallel WAL recovery: 2-3x speedup for large WAL files
- Lock-free atomic statistics: AtomicU64 counters
- Comprehensive benchmark suite with CI integration

</details>

---

### 🚧 v1.0 Production (In Progress)

**Milestone Goal:** Complete production-ready graph database with advanced algorithms, introspection APIs for LLM tooling, and comprehensive documentation.

### Phase 8: Graph Algorithms
**Goal**: Add centrality measures and community detection
**Depends on**: Phase 7
**Research**: Likely (algorithm selection, implementation patterns)
**Research topics**: PageRank, betweenness centrality, Louvain method, label propagation
**Plans**: 3

Plans:
- [x] 08-01: Implement centrality algorithms (PageRank, betweenness) ✅
- [x] 08-02: Implement community detection (Louvain, label propagation) ✅
- [x] 08-03: Add algorithm benchmarks and tests ✅

**Status**: Complete ✅ (2026-01-17)
**Key Results**:
- 4 production algorithms implemented: PageRank, Betweenness Centrality, Label Propagation, Louvain
- 27 tests passing (100% pass rate)
- 4 benchmark groups with multiple topologies (random, cycle, star, barbell)
- Comprehensive rustdoc with complexity analysis
- 10 commits total

### Phase 9: Developer Tooling
**Goal**: Add debugging, profiling, and introspection utilities
**Depends on**: Phase 8
**Research**: Likely (Rust profiling tools, debugging patterns)
**Research topics: flamegraph integration, introspection APIs, debugging hooks
**Plans**: 3

Plans:
- [x] 09-01: Add profiling/introspection APIs ✅
- [x] 09-02: Create debugging utilities ✅
- [x] 09-03: Add developer CLI commands ✅

**Status**: Complete ✅ (2026-01-17)
**Key Results**:
- GraphIntrospection API with JSON serialization (LLM consumable)
- ProgressCallback trait with ConsoleProgress for long-running operations
- CLI debug commands: debug-stats, debug-dump, debug-trace
- New algorithm commands: pagerank, betweenness, louvain with progress bars
- 13 commits total across 3 plans

### Phase 10: Testing & Docs
**Goal**: Comprehensive test coverage and module documentation
**Depends on**: Phase 9
**Research**: Unlikely (testing patterns and documentation)
**Plans**: 3

Plans:
- [ ] 10-01: Fix broken WAL tests and add edge case tests
- [ ] 10-02: Add concurrent operation tests
- [ ] 10-03: Add module documentation

**Scope:**
- Fix V2WALConfig compilation errors in wal_core_tests.rs
- 16 WAL recovery edge case tests (corruption, transactions, checkpoints, recovery)
- 15 concurrent operation tests (algorithms, snapshots, lifecycle)
- Complete rustdoc for: graph, hnsw, algo, cache, introspection, progress modules
- Invariants and guarantees documentation (not marketing copy)

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10

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
| 10. Testing & Docs | v1.0 | 0/3 | Not started | - |
