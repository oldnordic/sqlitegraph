# sqlitegraph

Deterministic, embedded graph backend built entirely on SQLite. The crate
targets SynCore/SPEC 16–17 requirements and is currently **alpha-quality**:
all mandated features and tests are present, but integration into larger
systems is still ongoing.

## Features

- Graph storage (entities/edges) using the existing `graph_entities` /
  `graph_edges` schema.
- Deterministic graph primitives: neighbors, BFS, k-hop traversal, shortest
  path, connected components, cycle detection, degree metrics.
- Pattern queries (`PatternQuery`) and reasoning layer (`GraphReasoner`) for
  higher-level candidate expansion/ranking.
- Backend abstraction (`GraphBackend`) + in-memory sqlite adapter
  (`SqliteGraphBackend`).
- Dual-read/dual-write tooling, migration manager, benchmark regression gates,
  and deterministic dataset generators.
- CLI + examples demonstrating end-to-end usage.

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
cargo run --bin sqlitegraph             # default `status` command (memory backend)
cargo run --bin sqlitegraph -- --db ./graph.db --command list
```

To run the curated examples:

```bash
cargo run --example basic_usage
cargo run --example migration_flow
```

## Programmatic usage

```rust
use sqlitegraph::{SqliteGraph, GraphQuery};

let graph = SqliteGraph::open_in_memory()?;
let node = graph.insert_entity(&my_entity)?;

let query = graph.query();
let neighbors = query.neighbors(node)?;
```

Higher-level APIs (exports from `lib.rs`) include:

- `SqliteGraph`, `GraphEntity`, `GraphEdge`
- `GraphQuery`, `GraphReasoner`, `ReasoningConfig`
- `PatternQuery`, `NodeConstraint`
- `GraphBackend`, `SqliteGraphBackend`
- `BackendClient` (programmatic wrapper)
- `MigrationManager`
- Dual-read/write harnesses

Full module documentation lives in `src/`—every public module is capped at
≤300 LOC for auditability.

## Manual & License

- Operator/developer manual: [`manual.md`](manual.md).
- License: GPL-3.0-only (see [`LICENSE`](LICENSE)).
