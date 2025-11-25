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
- CLI + safety tooling providing subgraph dumps, pipeline execution/explain,
  DSL parsing, and repository-wide `safety-check` reports.

## Status

- ✅ SPEC 16 / SPEC 17 feature set implemented inside this crate
- ✅ Deterministic multi-hop, pattern, reasoning, dual-read/write, migration,
  and benchmark gating
- ✅ CLI + examples demonstrating practical workflows
- ⚠️ Still awaiting broader SynCore wiring and real-world performance tuning;
  expect public APIs to stabilize as integration feedback arrives.

## Quick start

```bash
cargo test
cargo bench
```

To inspect CLI usage:

```bash
cargo run --bin sqlitegraph -- --help
cargo run --bin sqlitegraph                     # default `status` (memory backend)
cargo run --bin sqlitegraph -- --command list
cargo run --bin sqlitegraph -- --db ./graph.db --command subgraph --root 1 --depth 2
cargo run --bin sqlitegraph -- --db ./graph.db --command pipeline --dsl "pattern CALLS filter type=Fn"
cargo run --bin sqlitegraph -- --db ./graph.db --command safety-check
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
- `tests/cli_reasoning_tests.rs` / `cli_safety_tests.rs` validate the CLI subgraph/pipeline/explain/dsl-parse/safety-check commands (including `--strict`), keeping outputs deterministic.
- `tests/perf_gate_tests.rs` enforces the regression gates recorded in `sqlitegraph_bench.json` via `bench_gates::check_thresholds`.
Performance thresholds in sqlitegraph_bench.json prevent regressions across releases.

## Safety Invariants

- Orphan edge detection ensures every edge’s `from_id`/`to_id` resolves to an existing entity before subgraphs or pipelines run.
- Duplicate edge detection (same `(from,to,type)` tuple) guards against inflated traversal counts and ensures deterministic reasoning.
- Invalid label/property references (rows pointing at missing entity IDs) are rejected so DSL filters operate only on real metadata.
- `safety-check --strict` fails CI/automation whenever the report shows orphan/duplicate/metadata issues.
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

## CLI

`sqlitegraph --help` lists all commands. Highlights:

- `status`, `list` – legacy info commands (file or memory DB).
- `subgraph --root N --depth D [--types edge=CALLS --types node=Fn]` – dumps
  deterministic nodes/edges/signature plus the applied node/edge filters.
- `pipeline --dsl "pattern CALLS filter type=Fn"` (or `--file pipeline.json`) –
  runs a reasoning pipeline, returning JSON nodes/scores and echoing the DSL.
- `explain-pipeline --dsl ...` – emits per-step counts, filters, scoring notes,
  and the parsed DSL for deterministic auditing.
- `dsl-parse --input "CALLS->USES"` – classifies DSL input (pattern/pipeline/
  subgraph) with metadata.
- `safety-check [--strict]` – runs orphan/duplicate/label/property validators,
  printing a JSON safety report; with `--strict`, the command fails when issues
  are detected so CI can enforce invariants.
- `sqlitegraph --command metrics [--reset-metrics]` – dumps the instrumentation
  snapshot (prepare/execute counts, transaction totals, cache hits/misses) and
  optionally resets the counters for interactive investigations.

## Manual & License

- Operator/developer manual: [`manual.md`](manual.md) (build/test, ergonomic
  APIs, benchmark gates, CLI reasoning/safety, migration guidance).
- License: GPL-3.0-only (see [`LICENSE`](LICENSE)).
