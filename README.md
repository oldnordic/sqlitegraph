# SQLiteGraph

[![crates.io](https://img.shields.io/crates/v/sqlitegraph.svg)](https://crates.io/crates/sqlitegraph)
[![Documentation](https://docs.rs/sqlitegraph/badge.svg)](https://docs.rs/sqlitegraph)

Embedded graph database with dual backend architecture, graph algorithms,
Cypher-inspired queries, and HNSW vector search.

**Positioning:** Single-binary embedded database (no server). Persistent storage with
atomic batch commits. Graph algorithms + HNSW vector search in one engine.
SQLite: stable, mature, and easy to inspect with standard tooling. Native V3:
graph-oriented storage with cache, KV, pub/sub, and traversal features. See the
benchmarks below for workload-specific behavior.

## Technical Architecture

**Dual backend**
- SQLite backend — stable, mature, inspectable with standard tooling
- Native V3 backend — graph-oriented storage with bounded adjacency cache, KV, pub/sub, and traversal primitives

**Vector + graph queries**
- HNSW vector similarity search (`HnswIndex`, batch insert, cosine guard)
- Cypher-inspired query layer — `pattern_match`, structural graph queries
- Graph algorithms — BFS, k-hop, shortest path, louvain community detection, SCC, A*

**Concurrency & durability**
- MVCC snapshot reads during traversal (`src/mvcc.rs`)
- Write-ahead log, atomic batch commits
- Bounded LRU adjacency cache (degree-1000 hit: ~131 ns → ~13 ns)

**Temporal topology**
- Version chains via `checkpoint()` / `snapshot_as_of(n)`
- Persistent-homology analysis: H₀ component lifetimes (`scc_lineage_barcode`), β₁ cyclomatic trajectory (`cycle_rank_snapshot`), circular-dependency lifecycle (`cycle_scc_barcode`)

**Bindings**
- Python bindings via PyO3 / maturin (`sqlitegraph-py`)

**Keyword index:** embedded graph database · HNSW vector search · Cypher · MVCC · write-ahead log · persistent homology · temporal versioning · louvain · SQLite · Native V3 · Python bindings · Rust

## What's New in 3.4

The 3.4 release adds **MVCC snapshot tracking** and **scale optimizations** for
time-travel queries, eliminating expensive COUNT(*) table scans on large
datasets.

- **MVCC snapshot tracking (v8)** — named snapshots (`create_snapshot()`,
  `list_snapshots()`, `delete_snapshot()`), batch insert with snapshot tagging
  (`batch_insert_entities_with_snapshot()`, `batch_insert_edges_with_snapshot()`),
  and time-travel queries (`query_as_of(timestamp)`) filter by `created_at` for
  historical graph state.
- **Scale optimization (v9)** — pre-aggregated `snapshot_stats` table for O(1)
  time-travel vs O(N) COUNT(*) scans, composite indexes on `(snapshot_id,
  created_at)` for multi-dimensional queries, partition-ready schema design.

See [CHANGELOG](https://github.com/oldnordic/sqlitegraph/blob/main/CHANGELOG.md) for full history.

## System Requirements

**Rust**: 1.89 or later (edition 2024)

**System Libraries (for native-v3 turbovec optimization)**:
- **Arch Linux**: `sudo pacman -S blas-openblas`
- **Ubuntu/Debian**: `sudo apt-get install libopenblas-dev`
- **Fedora/RHEL**: `sudo dnf install openblas-devel`

*Optional*: OpenBLAS is only required for large-scale embedding datasets (>1K vectors) where turbovec optimization activates. Small datasets use HNSW-only search with no external dependencies.

## What's New in 3.3

The 3.3 release adds a **temporal version chain** with persistent-homology
topology analysis, a **bounded adjacency cache** (was unbounded — a memory
leak under large traversals), and fixes **13 silently-broken criterion
benchmarks**.

- **Temporal topology (`sqlitegraph::temporal`)** — capture graph versions with
  `checkpoint()`, read historical adjacency with `snapshot_as_of(n)`, and run
  persistent-homology analysis over the version chain: exact H₀ component
  lifetimes (`scc_lineage_barcode`), circular-dependency lifecycle
  (`cycle_scc_barcode`), and β₁ cyclomatic-number trajectory
  (`cycle_rank_snapshot`). `as_of` lookup: 24.7 ns at 1000 versions.
- **Bounded LRU cache** — adjacency lists stored as `Arc<Vec<i64>>`; cache hit
  at degree 1000 drops from ~131 ns to ~13 ns (–90 %).
- **HNSW cosine guard** — zero-magnitude / non-finite vectors return a typed
  error instead of panicking.

See [CHANGELOG](https://github.com/oldnordic/sqlitegraph/blob/main/CHANGELOG.md) for full history.

## What's New in the 3.2 Line

The 3.2 release line built on the 3.0 backend/model transition with concrete
HNSW and traversal improvements:

- `HnswIndex::batch_insert_vectors()` for lower-overhead bulk vector ingestion.
- Transactional topology persistence for HNSW metadata and layers.
- `HnswIndexStats` runtime counters for inserts, searches, vector-cache hits,
  and vector-cache misses.
- Streaming traversal iterators for BFS, DFS, topological sort, and connected
  components, so callers can avoid materializing full `Vec`s when they only
  need incremental results.
- `parking_lot`-based lock cleanup across the HNSW path and related hot locks.

## Backends

| Feature | SQLite | Native V3 |
|---------|--------|-----------|
| Status | Stable | Stable |
| Storage | `.db` file | `.graph` file |
| Capacity model | Storage-limited | Storage-limited |
| Graph algorithms | 35+ | 35+ |
| HNSW vectors | Yes | Yes |
| Pub/Sub | Yes | Yes |
| LRU Cache | No | Yes |
| Parallel BFS | No | Yes |
| MVCC snapshots | Yes (v8) | Yes (v8) |
| Time-travel queries | Yes (optimized v9) | Yes (optimized v9) |

## Benchmarks

See [Architecture](https://github.com/oldnordic/sqlitegraph/blob/main/docs/ARCHITECTURE.md) for system design details and [Benchmarking](https://github.com/oldnordic/sqlitegraph/blob/main/docs/BENCHMARKING.md) for methodology.

**Representative clean samples from 2026-06-07 (AMD Ryzen 7 7800X3D, tmpfs, Rust 1.95.0):**

| Benchmark | SQLite | V3 |
|-----------|--------|----|
| Criterion `bfs_traversal/small_random_1k_5k` | `2.3680 ms` | `3.3191 ms` |
| Criterion `bfs_traversal/medium_random_10k_50k` | `26.510 ms` | `56.240 ms` |
| Release microbenchmark point lookup | `3965 ns` | `146 ns` |

These numbers are workload-specific. The release microbenchmark is warm-cache
and intentionally narrow; the Criterion suites are better for backend
comparisons under realistic workloads.

**Run benchmarks yourself:**

```bash
git clone https://github.com/oldnordic/sqlitegraph.git
cd sqlitegraph
./scripts/run-curated-benchmarks.sh

cd sqlitegraph-core
cargo run --release --example test_performance_comparison --features native-v3
cargo bench --features native-v3 --bench backend_comparison
cargo bench --features native-v3 --bench sqlite_v3_curated
```

## Quick Start

```toml
[dependencies]
# SQLite backend (default)
sqlitegraph = "3.4"

# OR Native V3 backend (graph-oriented storage)
sqlitegraph = { version = "3.4", features = ["native-v3"] }
```

```rust
use sqlitegraph::backend::{GraphBackend, NodeSpec};
use sqlitegraph::backend::sqlite::SqliteGraphBackend;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = SqliteGraphBackend::in_memory()?;

    let node_id = backend.insert_node(NodeSpec {
        kind: "User".to_string(),
        name: "Alice".to_string(),
        file_path: None,
        data: serde_json::json!({"age": 30}),
    })?;

    println!("Created node: {}", node_id);
    Ok(())
}
```

## TypedDiGraph (In-Memory)

A lightweight in-memory directed graph with typed node and edge weights,
independent of the `GraphBackend` persistence layer. Useful for build-system
DAGs, dependency graphs, and analysis passes that don't need disk storage.

```rust
use sqlitegraph::typed_digraph::{TypedDiGraph, NodeIndex, Direction};
use sqlitegraph::typed_digraph::algo::{toposort, tarjan_scc, Dfs};

let mut g = TypedDiGraph::<&str, i32>::new();
let a = g.add_node("compile");
let b = g.add_node("link");
let c = g.add_node("run");
g.add_edge(a, b, 1);
g.add_edge(b, c, 2);

// Topological order
let order = toposort(&g).expect("acyclic");
assert_eq!(order, vec![a, b, c]);

// DFS traversal
let mut dfs = Dfs::new(&g, a);
assert_eq!(dfs.by_ref().collect::<Vec<_>>(), vec![a, b, c]);
```

Available in the current 3.x line. See [API.md](API.md#typeddigraph-api) for the full method list.

## CLI

```bash
cargo install sqlitegraph-cli

# Query (works with both SQLite and native-v3 backends)
sqlitegraph --backend sqlite --db graph.db query "MATCH (n:User) RETURN n.name"
sqlitegraph --backend v3 --db graph.db query "MATCH (n:User) RETURN n.name"

# Algorithms
sqlitegraph --db graph.db bfs --start 1 --depth 3
sqlitegraph --db graph.db algo pagerank --iterations 100
```

## Copy-Paste Demos

### Python: CRUD, Query, Algorithms, HNSW

```python
from sqlitegraph import Graph

g = Graph.open_in_memory()

alice = g.add_node(kind="User", name="Alice", data={"age": 30})
bob = g.add_node(kind="User", name="Bob", data={"age": 31})
g.add_edge(alice, bob, "KNOWS")

print(g.query("MATCH (a:User)-[:KNOWS]->(b:User) RETURN a.name, b.name"))
print(g.strongly_connected_components())

idx = g.create_hnsw_index("embeddings", dimension=3, metric="cosine")
idx.insert_vector([1.0, 0.8, 0.1], {"label": "graph databases"})
idx.insert_vector([0.1, 0.2, 1.0], {"label": "baking"})
print(idx.search([1.0, 0.9, 0.0], 1))
g.delete_hnsw_index("embeddings")
```

### CLI: From Empty Database to Query Result

```bash
rm -f /tmp/sqlitegraph-demo.db

sqlitegraph --db /tmp/sqlitegraph-demo.db --write insert --kind User --name Alice --data '{"age":30}'
sqlitegraph --db /tmp/sqlitegraph-demo.db --write insert --kind User --name Bob --data '{"age":31}'
sqlitegraph --backend sqlite --db /tmp/sqlitegraph-demo.db --write query 'CREATE (1)-[:KNOWS]->(2)'

sqlitegraph --backend sqlite --db /tmp/sqlitegraph-demo.db query 'MATCH (a:User)-[:KNOWS]->(b:User) RETURN a.name, b.name'
sqlitegraph --db /tmp/sqlitegraph-demo.db algo scc
```

### Rust: Hybrid Runtime

The Rust example below shows the full mixed-runtime story: ordinary SQLite rows,
Native V3 graph metadata, SQLite-backed HNSW vectors, and V3 pub/sub:

```bash
cargo run -p sqlitegraph --example hybrid_sqlite_v3_hnsw_pubsub --features native-v3
```

## Python

A Python wrapper around the same Rust engine is published to PyPI as
[`sqlitegraph`](https://pypi.org/project/sqlitegraph/). The source lives in
[`sqlitegraph-py/`](./sqlitegraph-py/) and ships as a single `abi3` wheel per
platform (Python 3.10+).

```bash
pip install sqlitegraph
```

```python
from sqlitegraph import Graph

g = Graph.open_in_memory()
alice = g.add_node(kind="User", name="Alice", data={"age": 30})
order = g.add_node(kind="Order", name="Order-123")
g.add_edge(alice, order, "placed")

print(g.neighbors(alice))
```

The Python surface covers node/edge CRUD, BFS/k-hop/shortest path, PageRank,
Louvain, connected components, SCC, label propagation, cycle search,
dominators, critical path, `Graph.query()`, HNSW vector indexes, typed
exceptions (`GraphError`, `NotFoundError`, `InvalidArgumentError`,
`BackendError`), and type stubs for editors. See
[`sqlitegraph-py/README.md`](./sqlitegraph-py/README.md) for the full Python API
and examples.

## Ecosystem

Tools built on SQLiteGraph:

| Tool | Purpose | Repository | crates.io |
|------|---------|------------|-----------|
| **Magellan** | Code graph indexing, symbol navigation | [github.com/oldnordic/magellan](https://github.com/oldnordic/magellan) | [crates.io/crates/magellan](https://crates.io/crates/magellan) |
| **llmgrep** | Semantic code search | [github.com/oldnordic/llmgrep](https://github.com/oldnordic/llmgrep) | [crates.io/crates/llmgrep](https://crates.io/crates/llmgrep) |
| **Mirage** | CFG analysis, path enumeration | [github.com/oldnordic/mirage](https://github.com/oldnordic/mirage) | [crates.io/crates/mirage-analyzer](https://crates.io/crates/mirage-analyzer) |
| **splice** | Precision code editing | [github.com/oldnordic/splice](https://github.com/oldnordic/splice) | [crates.io/crates/splice](https://crates.io/crates/splice) |

## Documentation

- [Architecture](https://github.com/oldnordic/sqlitegraph/blob/main/docs/ARCHITECTURE.md) - System design
- [Manual](https://github.com/oldnordic/sqlitegraph/blob/main/MANUAL.md) - API guide
- [Query Language](https://github.com/oldnordic/sqlitegraph/blob/main/docs/QUERY_LANGUAGE.md) - Cypher-inspired query reference
- [Changelog](https://github.com/oldnordic/sqlitegraph/blob/main/CHANGELOG.md) - Version history
- [SnapshotId Migration Guide](https://github.com/oldnordic/sqlitegraph/blob/main/docs/SNAPSHOTID_MIGRATION.md) - v2.1.2 API changes

## License

GPL-3.0-only
