# SPEC: Native-v3 Parity + Performance Fix (sqlitegraph 3.7.0)

**Date:** 2026-07-04
**Status:** Planning
**Skill chain:** grounded-coding-core → grounded-coding-planning → grounded-coding-tdd

## Scope Basis

Two subagent investigations (2026-07-04) confirmed three parity gaps and one
performance bottleneck on the native-v3 backend. All root causes are verified
with file:line evidence from source code and graph queries against
`~/.magellan/sqlitegraph/sqlitegraph-core.db`.

**Evidence sources:**
- Multi-hop Cypher report (deleg_44a31bb7) — 180 lines, 18 cited locations
- Performance report (deleg_11bed2ae) — 97 lines, 5 cited locations, 4 hard benchmark numbers
- Graph queries: `magellan find` for execute_multi_hop, chain_query, get_graph_ref, FileCoordinator, set_file_coordinator, V3EdgeStore, NameIndex, snapshot_import, query_nodes_by_name_pattern

## Goal

Multi-hop Cypher works on V3 end-to-end (query → traversal → results),
snapshot_import works on V3, query_nodes_by_name_pattern has GLOB semantics,
and V3 cold-path reads are competitive with sqlite-backend (not 10× slower).
All verified by real test execution and benchmarks.

## Defect Table

| ID | Severity | File:Line | Description |
|----|----------|-----------|-------------|
| V3-P1 | Critical | `backend/native/v3/file_coordinator.rs` (never instantiated) | FileCoordinator exists but is never wired in V3Backend::open()/create(). Every cache miss = open/seek/read/close per subsystem. Cold path 10× slower than sqlite. |
| V3-P2 | Critical | `file_coordinator.rs:14,25` | FileCoordinator uses Mutex, not RwLock — serializes all reads even after P1 fix |
| V3-C1 | High | `cypher.rs:1342-1344` + `backend.rs:4046-4048` | execute_multi_hop calls get_graph_ref() → returns None on V3. V3's chain_query exists but isn't called. |
| V3-C2 | High | `backend.rs:3414-3448` | V3 chain_query ignores step.edge_type (kind filtering deferred) |
| V3-C3 | Medium | `backend.rs:3636-3640` | snapshot_import returns Unsupported on V3 |
| V3-C4 | Medium | `backend.rs:3678-3680` | query_nodes_by_name_pattern uses substring instead of GLOB for non-wildcard patterns |
| V3-P3 | Medium | `store.rs:51, btree.rs:70` | PAGE_CACHE_SIZE=64 (256KB) — too small, FIFO eviction (not LRU) |
| V3-P4 | Medium | `edge_compat.rs:712` | Edge cache unbounded HashMap (memory leak, no eviction) |
| V3-P5 | Low | `store.rs:893-919` | load_node_page skips unpacked_page_cache on mutable path — double-decode |

## Out of Scope

- **mmap page layer** — significant new infrastructure, deferred to 3.8.0
- **CSR adjacency structure** — V3EdgeStore is sufficient for parity
- **freeze() fast path** — valuable but separate workstream
- **sqlitegraph-py publish** — separate release (pyo3 advisory)
- **The existing dirty worktree files** (`Cargo.toml`, `benches/readpath_probe.rs`) — these are the subagent's benchmark artifacts. The bench file is valuable — we will adopt and register it properly. The Cargo.toml change is the bench registration — we keep it.

## Agile Methodology Rules (binding)

1. Small phases, each independently shippable
2. Check and cross-check results at the end of each phase
3. Update CHANGELOG at the end of each phase
4. Regression tests (TDD: test fails before fix, passes after)
5. No guessing, no mocks, no stubs, no todos, no fixme, no for now, no placeholders, no shortcuts
6. No cheating to get positive results
7. Goal achieved when it works end-to-end correctly
8. Fix bugs found in the code even if not caused by this change
9. Use grounded coding skill: graph query before every edit
10. Fork/send subagents for independent work

## Phases

### Phase 1: Wire FileCoordinator into V3Backend (V3-P1, V3-P2)
**Why first:** Biggest perf win. Fixes the 10× cold-path regression. Unblocks meaningful benchmarking of later phases. The plumbing exists — it's purely a wiring fix.

### Phase 2: Rewire multi-hop Cypher to trait chain_query (V3-C1, V3-C2)
**Why second:** Biggest feature win. Depends on V3 having working reads (Phase 1 makes cold reads competitive so tests are meaningful). ~15-25 LOC in cypher.rs + edge_type filter in V3 chain_query.

### Phase 3: GLOB semantics for query_nodes_by_name_pattern (V3-C4)
**Why third:** Trivial fix (~1-5 LOC), correctness issue. Quick win after the larger phases.

### Phase 4: Right-size caches + bounded edge cache (V3-P3, V3-P4, V3-P5)
**Why fourth:** Performance tuning after the correctness fixes. Cache sizing needs the FileCoordinator fix (Phase 1) to measure true cache behavior.

### Phase 5: snapshot_import on V3 (V3-C3)
**Why fifth:** Medium complexity (~60-100 LOC). Mechanical: reuse DumpRecord parsing, loop over insert_node/insert_edge. Last because it's additive feature work, not on the critical path.

### Phase 6: Adopt readpath_probe benchmark + end-to-end verification
**Why last:** The benchmark file from the subagent needs proper registration. Then full E2E: run the multi-hop Cypher parity tests, run benchmarks showing cold-path improvement, verify all consumers still work.

## Definition of Done

1. V3 multi-hop Cypher returns correct results (parity test passes on V3)
2. V3 cold-path get_node is within 2× of sqlite (was 10× slower)
3. V3 warm-path retains 20-34× advantage
4. snapshot_import works on V3
5. query_nodes_by_name_pattern matches GLOB semantics
6. All existing tests pass (no regressions)
7. New regression tests added for each fix
8. Pre-claim scan clean (no banned patterns)
9. CHANGELOG has one entry per phase
10. Consumers (magellan, llmgrep, mirage, splice) verified compatible
