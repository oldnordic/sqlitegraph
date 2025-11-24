# sqlitegraph Manual

This manual describes how to interact with the sqlitegraph backend from Rust
and the provided CLI tooling. It complements the examples and serves as an
operator guide for TDD, benchmarking, dual-read/write migration, and CLI
operations.

---

## 1. Building & Testing

```bash
cargo test
cargo bench        # deterministic Criterion benches
```

Benchmarks produce HTML reports under `target/criterion`. Use `cargo bench
--bench bench_insert` (etc.) to isolate suites. The `bench_driver` binary runs
all benches sequentially and surfaces pass/fail summaries.

---

## 2. Core Rust APIs

### Graph primitives

- `SqliteGraph::open(path)` / `open_in_memory()`
- CRUD: `insert_entity`, `get_entity`, `update_entity`, `insert_edge`, etc.
- `GraphQuery` – neighbors, incoming/outgoing, edges_of_type, `has_path`,
  k-hop APIs (`k_hop_outgoing`, `k_hop_filtered`, `chain`), `pattern_matches`.
- `GraphReasoner` – `analyze(start, &PatternQuery, &ReasoningConfig)` returning
  ranked `ReasoningCandidate`s.

### Backend abstraction

- `GraphBackend` trait standardizes inserts, traversal, pattern queries and
  multi-hop functions.
- `SqliteGraphBackend` is the sqlite-backed implementation (supports `in_memory`
  and `from_graph`).
- `BackendClient` wraps any `GraphBackend` for simple operations (insert,
  neighbors, BFS).

### Multi-hop & pattern querying

- `PatternQuery` + `NodeConstraint` describe chained edge traversals with node
  filters.
- Multi-hop utilities guarantee deterministic traversal order (sorted adjacency).

### Reasoning & analog layer

- `GraphReasoner` pairs pattern matches with deterministic multi-hop expansions
  and structural scores (`ReasoningCandidate`).

---

## 3. Migration & Dual-run Tooling

### Dual-runtime harness

- `DualRuntime` compares two `GraphBackend`s (primary vs mirror) across a set of
  nodes (`DualRuntimeJob`) reporting matches/diffs with logs.

### Dual-write helper

- `DualWriter` mirrors nodes/edges across two backends, tracking `MirrorStats`.

### Migration manager

- `MigrationManager` manages a base + shadow sqlite backend pair:
  - `insert_node` / `insert_edge` dual-writes.
  - `shadow_read(job)` runs the dual runtime harness.
  - `cutover()` flips the active backend; `is_cutover()` surfaces status.
  - `active_backend()` returns the current read backend.

---

## 4. CLI

Binary: `sqlitegraph` (see `src/bin/sqlitegraph.rs`).

Usage:

```
sqlitegraph [--backend sqlite] [--db memory|PATH] [--command status|list]
```

Commands:

- `status` (default) – prints backend type + entity count.
- `list` – lists entity IDs + names.
- Unrecognized commands fall back to `status`.

Environment variable `CARGO_BIN_EXE_sqlitegraph` is used in tests; the CLI
operates purely locally (in-memory or file-backed sqlite). Use `cargo run
--bin sqlitegraph -- --help` for details.

---

## 5. Examples

### `basic_usage`

Demonstrates:

- Graph construction (`Function`/`Struct` nodes, CALLS/USES edges).
- Neighbors via `GraphQuery`.
- Pattern matching and reasoning scoring (`reasoning score=...` log).

Invoke: `cargo run --example basic_usage`.

### `migration_flow`

Demonstrates:

- `MigrationManager` dual writes, shadow reads, and cutover activation.
- Output includes `shadow_read matches=…` and `cutover active=…`.

Invoke: `cargo run --example migration_flow`.

---

## 6. Benchmark Gating

- `BenchRun` stores name/mean/samples.
- `BenchGate` accepts thresholds + baseline runs + tolerance.
- `GateEnforcer::evaluate(runs)` yields `GateReport { passed, reasons }`.

Use gating in CI to enforce max latency/regression tolerance (see
`tests/bench_gate_tests.rs` and `tests/bench_report_tests.rs`).

---

## 7. Deterministic Dataset Generators

`bench_utils` includes data generators (line, star, grid, Erdos–Renyi, scale
free). All take `(node_count, seed)`; outputs are sorted deterministically.
`tests/bench_data_tests.rs` ensures invariants.

---

## 8. Dual-read/dual-write Workflows

1. Use `DualWriter` or `MigrationManager` to mirror nodes/edges into a shadow
   backend.
2. Execute `shadow_read` jobs across key nodes to compare neighbors + BFS.
3. Inspect `DualRuntimeReport`—the `log` field records match/mismatch entries.
4. Once confident, `cutover()` to shadow backend; `active_backend()` will now
   point to the mirror instance.

---

## 9. Coding Guidelines (Enforced in Repo)

- Max 300 LOC per file (per spec).
- Deterministic ordering (sorted adjacency, seeded RNG) everywhere.
- No async/runtime dependencies; pure Rust + SQLite.
- Tests first (TDD) before implementations.

---

With the manual + README in place, sqlitegraph is fully documented and ready to
serve as the embedded graph backend needed to replace Neo4j. Contributions
should follow the existing TDD workflow and keep new files under 300 LOC.
