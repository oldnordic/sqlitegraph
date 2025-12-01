# sqlitegraph

Deterministic, embedded graph backend built entirely on SQLite. The crate
targets SynCore/SPEC 16–17 requirements and is currently **alpha-quality**:
all mandated features and tests are present, but integration into larger
systems is still ongoing.

## Features

- Graph storage (entities/edges) using `graph_entities` / `graph_edges` plus
  deterministic label/property indexes.
- Deterministic graph primitives: neighbors, BFS, k-hop traversal, shortest
  path, connected components, cycle detection, degree metrics.
- Pattern queries (`PatternQuery`) and reasoning pipelines (pattern, k-hop,
  filter, score steps) for higher-level candidate expansion/ranking.
- Backend abstraction (`GraphBackend`) + sqlite adapter, ergonomic
  `BackendClient` helpers (`NodeId`, label/property lookups, pipeline
  explanations) and dual-read/write tooling.
- Benchmark regression gates (`bench_gates`) with the committed
  `sqlitegraph_bench.json` baseline plus deterministic dataset generators for
  SPEC 17 performance validation.
- Safety tooling providing subgraph dumps, pipeline execution/explain,
  DSL parsing, and `safety-check` reports.

## Status

- ✅ SPEC 16 / SPEC 17 feature set implemented inside this crate
- ✅ Deterministic multi-hop, pattern, reasoning, dual-read/write, migration,
  and benchmark gating
- ✅ Examples demonstrating practical workflows
- ⚠️ Still awaiting broader SynCore wiring and real-world performance tuning;
  expect public APIs to stabilize as integration feedback arrives.

## Quick start

```bash
cargo test
cargo bench
```

To run curated examples:

```bash
cargo run --example basic_usage
cargo run --example migration_flow
```



To run the curated examples:

```bash
cargo run --example basic_usage
cargo run --example migration_flow
```

## Test Coverage

- `tests/subgraph_tests.rs` exercises cycles, self-loops, depth limits, and signature determinism for subgraph extraction.
- `tests/pipeline_tests.rs` and `tests/dsl_tests.rs` cover every pipeline composition plus DSL ambiguity/invalid cases.
- `tests/backend_trait_tests.rs` and `tests/migration_tests.rs` run trait-level suites and MigrationManager stress scenarios (dual-write, shadow-read, high-load).
- `tests/cli_reasoning_tests.rs` / `cli_safety_tests.rs` validate subgraph/pipeline/explain/dsl-parse/safety-check functionality (including `--strict`), keeping outputs deterministic.
- `tests/perf_gate_tests.rs` enforces the regression gates recorded in `sqlitegraph_bench.json` via `bench_gates::check_thresholds`.
Performance thresholds in sqlitegraph_bench.json prevent regressions across releases.

## Safety Invariants

- Orphan edge detection ensures every edge’s `from_id`/`to_id` resolves to an existing entity before subgraphs or pipelines run.
- Duplicate edge detection (same `(from,to,type)` tuple) guards against inflated traversal counts and ensures deterministic reasoning.
- Invalid label/property references (rows pointing at missing entity IDs) are rejected so DSL filters operate only on real metadata.
- `run_safety_checks` with strict mode fails CI/automation whenever the report shows orphan/duplicate/metadata issues.
- `run_integrity_sweep` performs an automated integrity sweep (entities/edges/labels/properties) akin to `sqlite3 --check`, flagging out-of-order IDs, invalid JSON payloads, or metadata that points at missing entities.
- Migration/shadow-read tooling reuses the same validators to keep dual-write transitions safe.

## DSL Constraints

- The embedded DSL supports deterministic `pattern`, `k-hop`, `filter`, and `score` steps only; clauses must be explicitly ordered.
- Repetition syntax (`CALLS*2`) and arrow chains (`CALLS->USES`) may not mix conflicting filters, and only a single `filter type=...` clause is permitted.
- Unknown tokens or conflicting clauses trigger parser errors that bubble up through the CLI/tests, preventing ambiguous reasoning requests.

## Programmatic usage

```rust
use sqlitegraph::{BackendClient, NodeId};
use sqlitegraph::backend::{NodeSpec, SqliteGraphBackend};

let backend = SqliteGraphBackend::in_memory()?;
let client = BackendClient::new(backend);
let fn_id = client.insert_node(NodeSpec::new("Fn", "demo"))?;
let neighbors = client.neighbors_of(NodeId(fn_id))?;
let safety = sqlitegraph::run_safety_checks(client.backend().graph())?;
println!("nodes={:?} safety={:?}", neighbors, safety);
```

Higher-level exports include structural subgraph extraction
(`subgraph::extract_subgraph`), reasoning pipelines (`pipeline::run_pipeline`),
DSL parsing (`dsl::parse_dsl`), ergonomic wrappers (`NodeId`, `Label`,
`PropertyKey`/`PropertyValue`), label/property indexes, and the benchmark
gating utilities (`bench_gates`).

Full module documentation lives in `src/`—every public module is capped at
≤300 LOC for auditability.

## Schema Compatibility Matrix

| Schema version | Changes | Notes |
| -------------- | ------- | ----- |
| 1 | Base tables (`graph_entities`, `graph_edges`, `graph_labels`, `graph_properties`) plus indexes and the `graph_meta` row containing the schema version. | Created whenever an empty database file is opened. |
| 2 | Adds `graph_meta_history` and records each applied migration step. | Call `run_pending_migrations` when upgrading older files. Newer binaries refuse to open DBs whose version exceeds `SCHEMA_VERSION`. |

Use `read_schema_version` to inspect the current version. All future migrations must append a row to this table so the upgrade path remains auditable.



## Manual & License

- Operator/developer manual: [`manual.md`](manual.md) (build/test, ergonomic
  APIs, benchmark gates, reasoning/safety, migration guidance).
- License: GPL-3.0-only (see [`LICENSE`](LICENSE)).

## Development Notes: Dead Code Warnings

SQLiteGraph contains several internal modules used only by:
- CLI,
- test suite,
- benchmarks,
- migration tooling,
- dual-runtime verification.

Because these modules are exercised through tests, CLI binary, examples, or dynamic-dispatch paths, **clippy cannot detect their usage** and reports `dead_code` warnings.

A full audit confirmed:

- 149 warnings flagged by clippy  
- 149 are false positives (all are used)  
- 0 actual unused items  

No code was removed, and no `#[allow(dead_code)]` suppressions were added. These warnings are informational and expected for this architecture.
