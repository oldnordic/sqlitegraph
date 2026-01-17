# Roadmap: SQLiteGraph

## Overview

Complete and improve SQLiteGraph, an embedded graph database in Rust with dual backend support. The journey from current state to a production-ready, high-performance graph database with complete Native V2 backend, persistent HNSW vector search, advanced algorithms, and comprehensive developer tooling.

## Domain Expertise

None — No specialized domain expertise directories available. Relying on codebase documentation and Rust embedded systems patterns.

## Phases

- [x] **Phase 1: Foundation Cleanup** — Address tech debt, large files, unused imports, debug scaffolding ✅
- [ ] **Phase 2: WAL Integration** — Complete WAL validator/replayer wiring, enable automatic checkpointing
- [ ] **Phase 3: Native V2 Reads** — Implement betree and read path optimizations
- [ ] **Phase 4: MVCC Completion** — Fix identified MVCC gaps and edge cases
- [ ] **Phase 5: HNSW Persistence** — Enable index save/restore to disk
- [ ] **Phase 6: HNSW CLI** — Fix indexes lost across CLI invocations
- [ ] **Phase 7: Performance** — WAL recovery parallelization, lock contention reduction, benchmarking
- [ ] **Phase 8: Graph Algorithms** — Centrality measures, community detection
- [ ] **Phase 9: Developer Tooling** — Debugging, profiling, introspection utilities
- [ ] **Phase 10: Testing & Docs** — Comprehensive test coverage, module documentation

## Phase Details

### Phase 1: Foundation Cleanup
**Goal**: Address technical debt to improve maintainability
**Depends on**: Nothing (first phase)
**Research**: Unlikely (internal code cleanup)
**Plans**: TBD

Plans:
- [x] 01-01: Break down large WAL files (4,113 line operations.rs, 1,657 line rollback.rs) ✅
- [x] 01-02: Remove unused imports and dead code ✅
- [x] 01-03: Gate debug prints behind single feature flag ✅

### Phase 2: WAL Integration
**Goal**: Complete WAL recovery and checkpoint functionality
**Depends on**: Phase 1
**Research**: Unlikely (internal code integration, architecture understood)
**Plans**: 3 plans created

Plans:
- [x] 02-01: Wire automatic checkpointing into commit path ✅
- [x] 02-02: Fix checkpoint V2 integration TODOs ✅
- [x] 02-03: Add WAL recovery edge case tests ✅

### Phase 3: Native V2 Reads
**Goal**: Implement read path optimizations for Native V2 (NOT BETrees - see docs/BETREE_RESEARCH.md)
**Depends on**: Phase 2
**Research**: Complete (betree research concluded: inappropriate for graph DB workloads)
**Research topics**: Read path caching, compression for cache efficiency, traversal-aware optimizations
**Plans**: 3 plans created

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
**Plans**: 3 plans created

**Plan Details**:
- **04-01**: Identify and document MVCC gaps (gap analysis, baseline tests, test scenarios)
- **04-02**: Improve snapshot isolation correctness (race condition fixes, concurrent read/write tests, public API integration)
- **04-03**: Add concurrent operation tests (WAL coordination, lifecycle edge cases, performance benchmarks)

Plans:
- [ ] 04-01: Identify and document MVCC gaps
- [ ] 04-02: Improve snapshot isolation correctness
- [ ] 04-03: Add concurrent operation tests

### Phase 5: HNSW Persistence
**Goal**: Enable HNSW index save/restore to disk
**Depends on**: Phase 4
**Research**: Complete (schema exists, pragmatic approach documented)
**Research topics**: HNSW serialization format, incremental index updates, recovery from corruption
**Plans**: 3 plans created

**Plan Details**:
- **05-01**: Metadata persistence (save/load index config to hnsw_indexes table)
- **05-02**: Vector persistence and index rebuild (vectors to hnsw_vectors, rebuild graph on load)
- **05-03**: Tests, error handling, and benchmarks (comprehensive validation, corruption recovery)

**Approach**: Pragmatic vector storage + index rebuild (from docs/hnsw_persistence_implementation_status_20241223.md)
- Store vectors as BLOB in hnsw_vectors table
- On load: read vectors, rebuild HNSW graph structure (O(N log N) rebuild cost)
- Simpler than full graph serialization to hnsw_layers table

Plans:
- [ ] 05-01: Implement HNSW index metadata persistence
- [ ] 05-02: Implement vector persistence and index restore
- [ ] 05-03: Add comprehensive persistence tests and benchmarks

### Phase 6: HNSW CLI
**Goal**: Fix HNSW indexes lost across CLI invocations
**Depends on**: Phase 5
**Research**: Unlikely (builds on Phase 5 persistence)
**Plans**: TBD

Plans:
- [ ] 06-01: Integrate persistent HNSW with CLI
- [ ] 06-02: Add CLI commands for index management

### Phase 7: Performance
**Goal**: Optimize WAL recovery, reduce lock contention, improve benchmarks
**Depends on**: Phase 6
**Research**: Likely (parallel recovery patterns, lock-free data structures)
**Research topics**: Parallel WAL replay strategies, lock-free snapshot updates, profiling tools
**Plans**: TBD

Plans:
- [ ] 07-01: Implement parallel WAL recovery
- [ ] 07-02: Reduce lock contention with lock-free structures
- [ ] 07-03: Add comprehensive performance benchmarks

### Phase 8: Graph Algorithms
**Goal**: Add centrality measures and community detection
**Depends on**: Phase 7
**Research**: Likely (algorithm selection, implementation patterns)
**Research topics**: PageRank, betweenness centrality, Louvain method, label propagation
**Plans**: TBD

Plans:
- [ ] 08-01: Implement centrality algorithms (PageRank, betweenness)
- [ ] 08-02: Implement community detection (Louvain, label propagation)
- [ ] 08-03: Add algorithm benchmarks and tests

### Phase 9: Developer Tooling
**Goal**: Add debugging, profiling, and introspection utilities
**Depends on**: Phase 8
**Research**: Likely (Rust profiling tools, debugging patterns)
**Research topics: flamegraph integration, introspection APIs, debugging hooks
**Plans**: TBD

Plans:
- [ ] 09-01: Add profiling/introspection APIs
- [ ] 09-02: Create debugging utilities
- [ ] 09-03: Add developer CLI commands

### Phase 10: Testing & Docs
**Goal**: Comprehensive test coverage and module documentation
**Depends on**: Phase 9
**Research**: Unlikely (testing patterns and documentation)
**Plans**: TBD

Plans:
- [ ] 10-01: Add WAL recovery edge case tests
- [ ] 10-02: Add concurrent operation tests
- [ ] 10-03: Add module documentation (~1,093 files missing docs)

## Progress

**Execution Order:**
Phases execute in numeric order: 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → 9 → 10

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation Cleanup | 3/3 | Complete | 2026-01-17 |
| 2. WAL Integration | 3/3 | Complete | 2026-01-17 |
| 3. Native V2 Reads | 3/3 | Complete | 2026-01-17 |
| 4. MVCC Completion | 3/3 | Complete | 2026-01-17 |
| 5. HNSW Persistence | 0/3 | Not started | - |
| 6. HNSW CLI | 0/2 | Not started | - |
| 7. Performance | 0/3 | Not started | - |
| 8. Graph Algorithms | 0/3 | Not started | - |
| 9. Developer Tooling | 0/3 | Not started | - |
| 10. Testing & Docs | 0/3 | Not started | - |
