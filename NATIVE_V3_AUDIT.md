# Native-v3 Audit

Date: 2026-07-03
Scope: `/home/feanor/Projects/sqlitegraph/sqlitegraph-core`

This document records what native-v3 is confirmed to do, what it explicitly does not do, and what remains unproven. Every claim below is backed by code inspection and commands run on this checkout.

## Summary

Native-v3 is substantially more complete than some older caveats suggest in the areas of:

- persistence and reopen durability
- KV durability and WAL recovery
- MVCC snapshot versioning and isolation
- transactions and savepoints
- public SQL access
- HNSW persistence and turbovec routing
- async traversal support

The main incompleteness is query parity, not storage parity. In particular:

- `pattern_search` now works on V3
- `snapshot_import` is still explicitly unsupported
- `query_nodes_by_name_pattern` does not match SQLite semantics
- Cypher tests mostly validate SQLite execution, not V3 execution
- multi-hop Cypher parity on V3 is not proven and current code strongly suggests a gap

## Confirmed Working

### Reopen Durability And Persistence

Verified by:

- `cargo test --features native-v3 --test v3_reopen_durability -- --nocapture`
- `cargo test --features native-v3 --test kv_durability_tests -- --nocapture`
- `cargo test --features native-v3 --test v3_verify_file_persistence -- --nocapture`
- `cargo test --features native-v3 --test v3_persistence_100 -- --nocapture`
- `cargo test --features native-v3 --test test_10k_bug_reproduction -- --nocapture`
- `cargo test --features native-v3 --test v3_sync_fix_validation -- --nocapture`

Confirmed by passing tests:

- file reopen preserves graph: [v3_reopen_durability.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/v3_reopen_durability.rs:25)
- KV survives flush/truncate cycle: [kv_durability_tests.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/kv_durability_tests.rs:327)
- persistence across reopen: [v3_verify_file_persistence.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/v3_verify_file_persistence.rs:10)
- 100-node persistence sweep: [v3_persistence_100.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/v3_persistence_100.rs:10)

Supporting implementation:

- `Drop` triggers `flush_to_disk()` and `sync_header()`: [backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:254)
- `flush_to_disk()` checkpoints WAL, persists indexes, writes KV checkpoint, truncates WAL: [backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:1795)
- header sync path: [backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:1848)

### MVCC Snapshots

Verified by:

- `cargo test --features native-v3 --test native_v3_comprehensive_feature_test -- --nocapture`
- targeted snapshot tests run by subagent:
  - `cargo test --features native-v3 checkpoint_assigns_monotonic_versions -- --nocapture`
  - `cargo test --features native-v3 as_of_finds_correct_version -- --nocapture`
  - `cargo test --features native-v3 snapshot_creation -- --nocapture`
  - `cargo test --features native-v3 snapshot_version_increments -- --nocapture`

Confirmed by:

- snapshot API implementation: [backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:686)
- comprehensive snapshot isolation test: [native_v3_comprehensive_feature_test.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/native_v3_comprehensive_feature_test.rs:143)

### Transactions And Savepoints

Verified by:

- `cargo test --features native-v3 --test native_v3_comprehensive_feature_test -- --nocapture`

Confirmed by:

- `begin_transaction()`: [backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:2044)
- savepoint support: [backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:2077)
- transaction management test: [native_v3_comprehensive_feature_test.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/native_v3_comprehensive_feature_test.rs:501)

### Public SQL Interface

Verified by:

- `cargo test --features native-v3 --test native_v3_comprehensive_feature_test -- --nocapture`

Confirmed by:

- `execute_sql()`: [backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:1925)
- parameterized/update variants nearby in same file
- public SQL interface test: [native_v3_comprehensive_feature_test.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/native_v3_comprehensive_feature_test.rs:425)

### HNSW And Turbovec

Verified by:

- `cargo test --features native-v3 --test hnsw_persistence_tests -- --nocapture`
- `cargo test --features native-v3 --test native_v3_comprehensive_feature_test -- --nocapture`

Confirmed by:

- HNSW vector persistence test: [hnsw_persistence_tests.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/hnsw_persistence_tests.rs:48)
- HNSW integration test: [native_v3_comprehensive_feature_test.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/native_v3_comprehensive_feature_test.rs:96)
- turbovec integration test: [native_v3_comprehensive_feature_test.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/native_v3_comprehensive_feature_test.rs:757)

### Async Traversal

Verified by:

- `cargo test --features native-v3 --test v3_async_traversal_tests -- --nocapture`

Confirmed by:

- async BFS and k-hop test: [v3_async_traversal_tests.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/v3_async_traversal_tests.rs:8)
- async backend implementation block: [backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:4696)

## Confirmed Not Working

### `pattern_search`

This now works on V3 with trait-level ordered leg expansion.

- implementation: [backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:3448)
- truth-audit test: [v3_query_truth_tests.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/v3_query_truth_tests.rs:27)

Verified by:

- `cargo test --features native-v3 --test v3_query_truth_tests -- --nocapture`

### `snapshot_import`

This is also explicitly unsupported.

- implementation: [backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:3571)
- truth-audit test: [v3_query_truth_tests.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/v3_query_truth_tests.rs:85)

Verified by:

- `cargo test --features native-v3 --test v3_query_truth_tests -- --nocapture`

### Reopen Corruption Investigation Test

The ignored corruption investigation is currently not a valid reopen signal because it fails before reopen during edge insertion.

- failing test: [reopen_corruption_investigation.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/reopen_corruption_investigation.rs:47)

Verified by:

- `cargo test --features native-v3 --test reopen_corruption_investigation -- --ignored --nocapture`

Observed failure:

- `InvalidInput("edge endpoints must reference existing entities")`

## Confirmed Semantics Mismatch

### `query_nodes_by_name_pattern`

V3 does not match SQLite semantics here.

- SQLite uses `GLOB`: [sqlite impl_.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/sqlite/impl_.rs:1031)
- V3 uses exact/prefix/substring logic: [backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:3589)
- audit test documenting mismatch: [v3_query_truth_tests.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/v3_query_truth_tests.rs:120)

Implications:

- `*` is treated specially by V3 substring logic rather than full SQLite `GLOB`
- `?` and character classes like `[abc]` do not have SQLite behavior

### Edge-pattern semantic field filters

V3 edge-pattern filtering now correctly honors semantic node fields like `name`,
`kind`, and `file_path`.

- matcher implementation: [backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:2591)
- parity test: [v3_cypher_parity_tests.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/v3_cypher_parity_tests.rs:1)

## Working But Misdocumented

### `query_nodes_by_kind`

The audit test comment says this is correct but slow due to O(n) scan. The current implementation uses the kind index.

- implementation: [backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:3580)
- index module: [kind_index.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/kind_index.rs:1)
- stale comment in test: [v3_query_truth_tests.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/v3_query_truth_tests.rs:171)

## Not Yet Proven

### Cypher Parity On V3

The generic Cypher suite passes:

- `cargo test --features native-v3 --test cypher_tests -- --nocapture`

But those tests are executed against `SqliteGraphBackend`, not `V3Backend`.

- SQLite-only harness: [cypher_tests.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/cypher_tests.rs:10)

So the passing suite proves parser behavior and SQLite execution semantics. It does not prove V3 execution parity.

### Multi-hop Cypher On V3

This looks incomplete from code inspection.

- `execute_multi_hop()` requires `backend.get_graph_ref()`: [cypher.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/cypher.rs:1342)
- V3 returns `None` from `get_graph_ref()`: [backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:3984)

That strongly suggests V3 multi-hop Cypher is not actually at parity, even though low-level traversal and graph algorithms are working.

### Historical Snapshot Semantics On V3 Query Helpers

V3 accepts historical snapshot IDs for some helper queries, but this audit did not prove correctness of historical-state results for those helpers.

Evidence:

- V3 accepts any snapshot in these paths: [backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:390)
- subagent verified targeted acceptance tests in `v3_algorithm_tests`
- SQLite explicitly rejects historical snapshots for comparable helper/query paths: [sqlite_snapshot_tests.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/tests/sqlite_snapshot_tests.rs:299)

### Power-loss Durability After KV Checkpoint Rename

No failure was reproduced, but the current code path does not fully prove crash durability after rename because the parent directory is not fsynced after the checkpoint file rename.

- rename/fsync area: [wal.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/wal.rs:1781)

This is a proof gap, not a reproduced bug.

## Documentation Contradictions

The repo currently contains conflicting claims.

- root status doc claims full parity: [NATIVE_V3_IMPLEMENTATION_STATUS.md](/home/feanor/Projects/sqlitegraph/NATIVE_V3_IMPLEMENTATION_STATUS.md:8)
- query docs still say edge patterns require SQLite and V3 returns an error: [QUERY_LANGUAGE.md](/home/feanor/Projects/sqlitegraph/docs/QUERY_LANGUAGE.md:217)
- core backend code still explicitly returns `Unsupported` for `pattern_search` and `snapshot_import`

The audit evidence supports the code, not the optimistic parity claim.

## Recommended Fix Order

1. Fix or narrow documentation claims first.
   - Replace “full feature parity” with a capability matrix based on this audit.
   - Distinguish low-level triple matching from trait-level `pattern_search` and from full Cypher execution.

2. Close the explicit query parity gaps.
   - implement `pattern_search`
   - implement `snapshot_import` or clearly scope it out
   - align `query_nodes_by_name_pattern` with SQLite `GLOB` semantics if parity is the goal

3. Add V3-native Cypher execution tests.
   - duplicate the current `cypher_tests` harness for `V3Backend`
   - add focused tests for edge patterns, label filters, multi-hop, variable-depth, and `WHERE`

4. Resolve the ignored corruption investigation.
   - make `reopen_corruption_investigation` fail or pass for the intended reopen reason, not at setup time

5. Tighten durability proof around checkpoint rename.
   - add parent-directory fsync after rename if the design requires power-loss robustness
   - add a dedicated crash-window test if practical

## Commands Run In Main Audit

- `cargo test --features native-v3 --test v3_query_truth_tests -- --nocapture`
- `cargo test --features native-v3 --test native_v3_comprehensive_feature_test -- --nocapture`
- `cargo test --features native-v3 --test v3_reopen_durability -- --nocapture`
- `cargo test --features native-v3 --test v3_async_traversal_tests -- --nocapture`
- `cargo test --features native-v3 --test hnsw_persistence_tests -- --nocapture`
- `cargo test --features native-v3 --test cypher_tests -- --nocapture`

## Bottom Line

Native-v3 storage, durability, KV/WAL recovery, snapshots, transactions, SQL access, HNSW, and async traversal are in much better shape than the current uncertainty suggests.

The main missing work is user-visible query parity and truthful documentation:

- storage layer: mostly working
- advanced features: mostly working
- query parity: incomplete
- docs/status claims: overstated and need correction
