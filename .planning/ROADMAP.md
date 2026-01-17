# Roadmap: SQLiteGraph

## Overview

Complete and improve SQLiteGraph, an embedded graph database in Rust with dual backend support. The journey from current state to a production-ready, high-performance graph database with complete Native V2 backend, persistent HNSW vector search, advanced algorithms, and comprehensive developer tooling.

## Domain Expertise

None — No specialized domain expertise directories available. Relying on codebase documentation and Rust embedded systems patterns.

## Phases

- [ ] **Phase 1: Foundation Cleanup** — Address tech debt, large files, unused imports, debug scaffolding
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
- [ ] 01-03: Gate debug prints behind single feature flag

### Phase 2: WAL Integration
**Goal**: Complete WAL recovery and checkpoint functionality
**Depends on**: Phase 1
**Research**: Likely (WAL architecture patterns, checkpoint strategies)
**Research topics**: WAL checkpoint triggers, recovery edge cases, validation/replayer wiring
**Plans**: TBD

Plans:
- [ ] 02-01: Wire WAL/Checkpoint placeholder functions
- [ ] 02-02: Implement automatic checkpointing
- [ ] 02-03: Add WAL recovery edge case tests

### Phase 3: Native V2 Reads
**Goal**: Implement betree and read path optimizations for Native V2
**Depends on**: Phase 2
**Research**: Likely (betree data structures, read optimization patterns)
**Research topics**: B-tree vs B-epsilon-tree for graph data, read path caching strategies
**Plans**: TBD

Plans:
- [ ] 03-01: Design and implement betree for Native V2 reads
- [ ] 03-02: Optimize read path performance
- [ ] 03-03: Add read performance benchmarks

### Phase 4: MVCC Completion
**Goal**: Fix identified MVCC gaps and edge cases
**Depends on**: Phase 3
**Research**: Likely (MVCC patterns, snapshot isolation edge cases)
**Research topics**: Concurrent read/write patterns, snapshot lifecycle management
**Plans**: TBD

Plans:
- [ ] 04-01: Identify and fix MVCC gaps
- [ ] 04-02: Improve snapshot isolation correctness
- [ ] 04-03: Add concurrent operation tests

### Phase 5: HNSW Persistence
**Goal**: Enable HNSW index save/restore to disk
**Depends on**: Phase 4
**Research**: Likely (index serialization, incremental updates)
**Research topics**: HNSW serialization format, incremental index updates, recovery from corruption
**Plans**: TBD

Plans:
- [ ] 05-01: Design HNSW persistence format
- [ ] 05-02: Implement index save to disk
- [ ] 05-03: Implement index restore from disk

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
| 1. Foundation Cleanup | 2/3 | In progress | 2026-01-17 |
| 2. WAL Integration | 0/3 | Not started | - |
| 3. Native V2 Reads | 0/3 | Not started | - |
| 4. MVCC Completion | 0/3 | Not started | - |
| 5. HNSW Persistence | 0/3 | Not started | - |
| 6. HNSW CLI | 0/2 | Not started | - |
| 7. Performance | 0/3 | Not started | - |
| 8. Graph Algorithms | 0/3 | Not started | - |
| 9. Developer Tooling | 0/3 | Not started | - |
| 10. Testing & Docs | 0/3 | Not started | - |
