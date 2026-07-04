# Agile Methodology Plan: Native-v3 Parity + Performance

**Spec:** `SPEC_native_v3_parity_perf_2026-07-04.md`
**Skill chain:** grounded-coding-core → grounded-coding-planning → grounded-coding-tdd → grounded-coding-verification

## Rules (non-negotiable — quoted from user)

- small phases
- check and cross-check results at the end of each phase
- update CHANGELOG at the end of each phase
- regression tests (TDD: fail before, pass after)
- no guessing, no mocks, no stubs, no todos, no fixme, no for now, no placeholders, no shortcuts, no cheating
- goal achieved when it works end-to-end correctly
- if you find a bug in the code even if not caused by this change, fix it
- use grounded coding skill: graph query before every edit
- fork/send subagents for independent work

## Phase 1: Wire FileCoordinator into V3Backend (V3-P1, V3-P2)

**Goal:** Cold-path reads on V3 stop doing open/seek/read/close per cache miss.

**Graph queries (run before editing):**
```
magellan find --db <DB> --name "FileCoordinator"
magellan find --db <DB> --name "set_file_coordinator"
magellan refs --db <DB> --name "set_file_coordinator" --direction in
```

**Steps:**
1. Read `backend/native/v3/file_coordinator.rs` — study FileCoordinator struct and create()
2. Read `backend/native/v3/backend.rs` — find V3Backend::open() and V3Backend::create(). Identify where node_store, edge_btree, edge_store are constructed.
3. In open() and create(), after constructing node_store/edge_btree/edge_store, create one `Arc<FileCoordinator>` and call `set_file_coordinator()` on all three.
4. Change FileCoordinator inner type from Mutex to RwLock (file_coordinator.rs:14,25). read_page takes .read(), write_page takes .write().
5. Verify the wiring: add a test that opens a V3 DB, does a cold read, and asserts the FileCoordinator is being used (not opening new file handles per read).

**Regression test (TDD):**
- Write `test_v3_file_coordinator_wired` that:
  - Creates a V3 DB with 100 nodes
  - Drops the backend (clears caches)
  - Reopens, does a get_node
  - Asserts the FileCoordinator handle is present and was used
- Write `test_v3_concurrent_reads` that spawns 4 threads doing get_node simultaneously, asserts no deadlock and all succeed

**Cross-check (all must pass):**
```bash
cd /home/feanor/Projects/sqlitegraph/sqlitegraph-core
cargo check --all-targets --features native-v3
cargo test --features native-v3 --test v3_reopen_durability
cargo test --features native-v3 -- --nocapture 2>&1 | tail -3
cargo clippy --all-targets --features native-v3 -- -D warnings
cargo fmt --all -- --check
rg 'todo!|unimplemented!|FIXME|TODO|HACK|for now|placeholder|temporary' src/backend/native/v3/ --type rust | grep -v test
```

**Benchmark cross-check:** Run the readpath_probe bench. Cold get_node should drop from ~424µs toward <100µs.

**CHANGELOG:** Add entry under [Unreleased].

---

## Phase 2: Rewire multi-hop Cypher (V3-C1, V3-C2)

**Goal:** `MATCH (n)-->(m)-->(o)` returns correct results on V3.

**Graph queries (run before editing):**
```
magellan find --db <DB> --name "execute_multi_hop"
magellan refs --db <DB> --name "execute_multi_hop" --direction in
magellan context symbol --db <DB> --name "chain_query" --depth 2
magellan find --db <DB> --name "chain_query" --path src/multi_hop.rs
```

**Steps:**
1. Read `cypher.rs:1337-1405` (execute_multi_hop) — trace the call to get_graph_ref + multi_hop::chain_query
2. Read `backend.rs:3414-3448` (V3 chain_query impl) — confirm it works, note the edge_type deferred comment
3. Read `multi_hop.rs:73-102` (free function chain_query) — understand the SQLite-specific SQL path
4. Rewire execute_multi_hop: replace `multi_hop::chain_query(graph, start_id, &chain)` with `backend.chain_query(snapshot, start_id, &chain)`. Remove the `get_graph_ref()` call.
5. Fix V3 chain_query edge_type filtering: when step.edge_type is Some(t), call `edge_store.neighbors_filtered(node, dir, t)` instead of `edge_store.neighbors(node, dir)`.
6. Add sort+dedup to V3 chain_query for behavioral parity with the SQLite free function.

**Regression test (TDD):**
- Write `test_v3_multi_hop_two_step` that:
  - Creates V3 DB: A→B→C (edges with types)
  - Runs multi-hop: start=A, chain=[step1: any type outgoing, step2: any type outgoing]
  - Asserts result = {C}
- Write `test_v3_multi_hop_edge_type_filter` that:
  - Creates V3 DB: A -[KNOWS]-> B -[LIKES]-> C, A -[KNOWS]-> D
  - Runs multi-hop: start=A, chain=[step1: KNOWS, step2: LIKES]
  - Asserts result = {C} (not D)

**Cross-check:**
```bash
cargo test --features native-v3 --test v3_cypher_parity_tests
cargo test --features native-v3 --test v3_query_truth_tests
cargo check --all-targets --features native-v3
cargo clippy --all-targets --features native-v3 -- -D warnings
cargo fmt --all -- --check
```

The existing test `test_v3_execute_multi_hop_reports_current_gap` currently asserts multi-hop FAILS. After the fix, it must be updated to assert SUCCESS — OR removed and replaced with the new positive tests above. (The test documenting the gap becomes the regression test proving the gap is closed.)

**CHANGELOG:** Add entry.

---

## Phase 3: GLOB semantics for query_nodes_by_name_pattern (V3-C4)

**Goal:** `query_nodes_by_name_pattern("User")` matches exactly "User", not "SuperUser".

**Graph queries:**
```
magellan find --db <DB> --name "query_nodes_by_name_pattern"
magellan context symbol --db <DB> --name "get_substring" --depth 1
magellan find --db <DB> --name "NameIndex"
```

**Steps:**
1. Read `backend.rs:3651-3680` — the V3 dispatch logic for name patterns
2. Read `name_index.rs:8-12, 74-83` — NameIndex capabilities (exact, prefix, substring)
3. Fix: in the `else` branch (no wildcards), call `name_index.get_exact(pattern)` instead of `get_substring(pattern)`.
4. For patterns with `*` in the middle (`*suffix`, `pre*mid*`): implement a `glob_match()` helper that compiles the GLOB pattern to a simple matcher. Scan all names in the index. ~15 LOC.

**Regression test (TDD):**
- Write `test_v3_name_pattern_exact_match` that:
  - Creates nodes "User", "SuperUser", "UserAdmin"
  - Queries pattern "User"
  - Asserts result = {User} (not SuperUser, not UserAdmin)
- Write `test_v3_name_pattern_glob_prefix` that:
  - Queries "User*"
  - Asserts result = {User, UserAdmin}
- Write `test_v3_name_pattern_glob_suffix` that:
  - Queries "*User"
  - Asserts result = {User, SuperUser}

**Cross-check:** Same gates as Phase 1+2.

**CHANGELOG:** Add entry.

---

## Phase 4: Right-size caches (V3-P3, V3-P4, V3-P5)

**Goal:** Caches don't thrash on working sets > 256KB. Edge cache is bounded.

**Graph queries:**
```
magellan find --db <DB> --name "PAGE_CACHE_SIZE"
magellan find --db <DB> --name "NodeStore"
magellan find --db <DB> --name "V3EdgeStore"
```

**Steps:**
1. Raise PAGE_CACHE_SIZE from 64 → 1024 (4MB at 4KB/page) in `store.rs:51` and `btree.rs:70`
2. Replace FIFO eviction (`cache.keys().next()`, `store.rs:1385`) with LRU. Use the `lru` crate (already a dep via sqlitegraph-cli or add it) or implement a simple LRU (HashMap + VecDeque).
3. Bound the edge cache (`edge_compat.rs:712`): replace unbounded `HashMap` with a bounded `LruCache` (8192 entries, matching sqlite-backend's AdjacencyCache).
4. Fix `load_node_page` (`store.rs:893`) to consult `unpacked_page_cache` on the mutable path, matching `lookup_node_ro`.

**Regression test (TDD):**
- Write `test_v3_cache_lru_eviction` that:
  - Inserts 2000 nodes (exceeds 1024-page cache)
  - Reads nodes in a pattern that would thrash FIFO but not LRU
  - Asserts cache hit rate is reasonable (not 0%)

**Cross-check:** Same gates + benchmark showing warm-path still fast, cold-path improved further.

**CHANGELOG:** Add entry.

---

## Phase 5: snapshot_import on V3 (V3-C3)

**Goal:** `snapshot_import(jsonl_path)` works on V3.

**Graph queries:**
```
magellan find --db <DB> --name "snapshot_import"
magellan find --db <DB> --name "load_graph_from_path"
magellan find --db <DB> --name "DumpRecord"
```

**Steps:**
1. Read `impl_.rs:770-805` — the sqlite snapshot_import that calls load_graph_from_path
2. Read `recovery.rs:76-185` — the JSONL DumpRecord parser (backend-agnostic)
3. Read `backend.rs:3636-3640` — the V3 Unsupported stub
4. Implement V3 snapshot_import: read JSONL, for each DumpRecord::Entity call insert_node, for each DumpRecord::Edge call insert_edge, for Label/Property map to V3 inline data JSON.
5. Return ImportMetadata with real counts.

**Regression test (TDD):**
- Write `test_v3_snapshot_import` that:
  - Creates a sqlite-backend DB with 50 nodes + 100 edges
  - Exports to JSONL (dump_graph_to_path)
  - Opens a V3 DB, calls snapshot_import on the JSONL
  - Asserts node count = 50, edge count = 100
  - Asserts a sample node's properties match

**Cross-check:** Same gates.

**CHANGELOG:** Add entry.

---

## Phase 6: Benchmark adoption + E2E verification

**Goal:** readpath_probe benchmark is properly registered, full E2E passes.

**Steps:**
1. Review `benches/readpath_probe.rs` (subagent artifact) — verify it compiles, runs, and produces meaningful numbers
2. Register in Cargo.toml `[[bench]]` if not already (the subagent modified Cargo.toml — verify the entry is correct)
3. Run full benchmark suite: `cargo bench --features native-v3`
4. E2E verification:
   - Run ALL V3 tests: `cargo test --features native-v3`
   - Run sqlite-backend tests: `cargo test` (default features)
   - Verify no regressions in either backend
   - Run the readpath_probe bench, compare cold-path numbers to the baseline (424µs → target <100µs)
5. Pre-claim banned pattern scan across the whole crate

**Cross-check:**
```bash
cargo test --features native-v3
cargo test  # default (sqlite-backend)
cargo bench --features native-v3 --bench readpath_probe
cargo fmt --all -- --check
cargo clippy --all-targets --features native-v3 -- -D warnings
rg 'todo!|unimplemented!|FIXME|TODO|HACK|for now|placeholder|temporary' src/ --type rust | grep -v test
```

**CHANGELOG:** Consolidate, bump to 3.7.0, commit, push, CI, publish.

---

## Subagent Strategy

**Phase 1 (FileCoordinator wiring):** Main session — needs careful surgical edit in backend.rs open()/create().

**Phase 2 (multi-hop rewire):** Main session — cypher.rs edit is small but needs understanding of the query layer.

**Phase 3 (GLOB fix):** Fork to subagent — self-contained, ~5-40 LOC, clear TDD test.

**Phase 4 (cache sizing):** Main session — touches multiple files, needs benchmarking.

**Phase 5 (snapshot_import):** Fork to subagent — self-contained feature, ~60-100 LOC, mechanical.

**Phase 6 (E2E):** Main session — verification only.

## Quality Gates (per phase)

```bash
cargo check --all-targets --features native-v3
cargo test --features native-v3
cargo clippy --all-targets --features native-v3 -- -D warnings
cargo fmt --all -- --check
rg 'todo!|unimplemented!|FIXME|TODO|HACK|for now|placeholder|temporary' src/backend/native/v3/ --type rust | grep -v test
```

## Version Bump

3.6.0 → 3.7.0 (MINOR — new features: multi-hop, snapshot_import, GLOB; performance: FileCoordinator, cache sizing)
