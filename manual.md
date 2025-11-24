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

### Backend abstraction & ergonomic client

- `GraphBackend` trait standardizes inserts, traversal, pattern queries and
  multi-hop functions.
- `SqliteGraphBackend` is the sqlite-backed implementation (supports `in_memory`
  and `from_graph`).
- `BackendClient` wraps any `GraphBackend` and provides ergonomic helpers using
  wrapper types:
  - `NodeId` / `EdgeId` newtypes (deterministic ordering guarantees).
  - `Label`, `PropertyKey`, `PropertyValue` helpers for the label/property
    indexes (`labeled`, `with_property`).
  - `neighbors_of`, `get_node`, `subgraph`, `run_pattern`, `run_pipeline`,
    `explain_pipeline`.
  - `explain_pipeline` returns `PipelineExplanation` with step summaries, per
    step counts, filters and scoring notes.

### Multi-hop & pattern querying

- `PatternQuery` + `NodeConstraint` describe chained edge traversals with node
  filters.
- Multi-hop utilities guarantee deterministic traversal order (sorted adjacency).

### Reasoning & analog layer

- `GraphReasoner` pairs pattern matches with deterministic multi-hop expansions
  and structural scores (`ReasoningCandidate`).
- `ReasoningPipeline` (pattern / KHops / Filter / Score steps) powers the CLI
  reasoning commands and programmatic inference.

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
sqlitegraph [--backend sqlite] [--db memory|PATH] --command <subcommand> [args]
```

Deterministic subcommands:

- `status` (default) – backend + entity count.
- `list` – entity IDs + names (ascending id).
- `subgraph --root N --depth D [--types edge=CALLS --types node=Fn]` – emits a
  JSON neighborhood (`nodes`, `edges`, `signature`) using the same deterministic
  traversal as `subgraph::extract_subgraph`.
- `pipeline --dsl "<dsl>"` or `--file pipeline.json` – runs a reasoning pipeline
  and returns `nodes` plus detailed `scores` (node/score pairs).
- `explain-pipeline --dsl "<dsl>"` – returns `steps_summary`, `node_counts`,
  `filters`, `scoring` arrays describing the executed pipeline.
- `dsl-parse --input "<expr>"` – parses DSL input and classifies it as
  pattern/pipeline/subgraph with metadata (legs, depth, filter counts).
- `safety-check` – runs all validators (orphans, duplicates, invalid
  labels/properties) and emits a JSON report (used for CI/operations).
- Legacy `dsl:<expr>` form redirects to `dsl-parse` but the structured command
  is preferred.

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

## 6. Benchmark Gating & Performance Logging

- `bench_gates::record_bench_run(name, BenchMetric)` records deterministic
  metrics (ops/sec, bytes/sec, free-form notes) into a JSON file
  (`sqlitegraph_bench.json` by default, path configurable via
  `set_bench_file_path`).
- `bench_gates::check_thresholds(name, BenchThreshold)` enforces minimum throughput
  / maximum latency before CI passes.
- `bench_gates::compare_to_baseline` compares against stored metrics to detect
  regressions or improvements.
- Criterion benches call `record_bench_run` exactly once per benchmark; the
  values are deterministic mock metrics so tests remain repeatable while the
  real benches still write actual measurements when run manually.
- Use gating in CI to enforce max latency/regression tolerance (see
  `tests/bench_gates_tests.rs` for API coverage).

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

---

## 10. DSL parser

- `parse_dsl("CALLS->USES")` → `PatternQuery` with appropriate legs.
- `parse_dsl("3-hop type=Fn")` → `SubgraphRequest { depth: 3, allowed_node_types: ["Fn"] }`.
- `parse_dsl("pattern CALLS*3 filter type=Module")` → `ReasoningPipeline` with a
  repeated CALL chain and a node filter.
- Invalid inputs return `DslResult::Error(message)`.

Use the DSL for quick CLI interactions (`--command dsl:CALLS->USES`) or to
bootstrap pipelines/subgraph requests from config files.

---

## 11. CLI reasoning & DSL workflows

- Build DSL expressions (`CALLS->USES`, `pattern CALLS*3 filter type=Module`,
  `3-hop type=Fn`) and feed them to:
  - `sqlitegraph --command pipeline --dsl "<expr>"` to execute reasoning.
  - `sqlitegraph --command explain-pipeline --dsl "<expr>"` to inspect per-step
    counts (useful for tuning TDD tests).
  - `sqlitegraph --command dsl-parse --input "<expr>"` for dry-run validation.
- Pipelines can also be provided via files (JSON containing `{ "dsl": "..." }`).
- CLI commands mirror library functions, so you can reproduce failing CI runs
  locally by copying the DSL snippet.

## 12. Safety checks

- Library: `run_safety_checks(&SqliteGraph)` aggregates:
  - `validate_referential_integrity` (edges referencing missing nodes).
  - `validate_no_duplicate_edges` (duplicate rows per from/to/type triple).
  - `validate_labels_properties` (label/property rows referencing missing nodes).
- Strict mode: `run_strict_safety_checks` returns `SafetyError` if any counters
  are non-zero.
- CLI: `sqlitegraph --command safety-check [--db path]` emits the structured
  JSON report, enabling automation/CI.
- Tests: `tests/safety_tests.rs` cover each validator, `tests/cli_safety_tests.rs`
  ensures CLI parity.

---

## 12. Label & property indexes

- `index::add_label`, `index::get_entities_by_label`
- `index::add_property`, `index::get_entities_by_property`

All functions are deterministic and sort by `entity_id`. These APIs are used by
the extended `BackendClient` helpers and CLI tooling.
