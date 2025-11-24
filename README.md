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
- Benchmark regression gates (`bench_gates`) and deterministic dataset
  generators for SPEC 17 performance validation.
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
  deterministic nodes/edges/signature as JSON.
- `pipeline --dsl "pattern CALLS filter type=Fn"` (or `--file pipeline.json`) –
  runs a reasoning pipeline, returning JSON nodes/scores.
- `explain-pipeline --dsl ...` – emits per-step counts, filters, scoring notes.
- `dsl-parse --input "CALLS->USES"` – classifies DSL input (pattern/pipeline/
  subgraph) with metadata.
- `safety-check` – runs orphan/duplicate/label/property validators and prints a
  JSON safety report.

## Manual & License

- Operator/developer manual: [`manual.md`](manual.md) (build/test, ergonomic
  APIs, benchmark gates, CLI reasoning/safety, migration guidance).
- License: GPL-3.0-only (see [`LICENSE`](LICENSE)).
