# SQLiteGraph Manual

Usage guide for SQLiteGraph in the current 3.4.6 release line, with dual backend
architecture (SQLite and Native V3).

> Looking for the Python wrapper? See
> [`sqlitegraph-py/README.md`](./sqlitegraph-py/README.md) — `pip install sqlitegraph`.
>
> Looking for the query language? See
> [`docs/QUERY_LANGUAGE.md`](./docs/QUERY_LANGUAGE.md) for the CLI and
> Python `Graph.query()` grammar.

---

## 1. Quick Start

### Installation

```toml
[dependencies]
sqlitegraph = "3.4.6"

# For Native V3 backend
sqlitegraph = { version = "3.4.6", features = ["native-v3"] }
```

**System Requirements** (for native-v3 turbovec optimization):
- **Arch Linux**: `sudo pacman -S blas-openblas`
- **Ubuntu/Debian**: `sudo apt-get install libopenblas-dev`
- **Fedora/RHEL**: `sudo dnf install openblas-devel`

*Note*: OpenBLAS is only required for large-scale embedding datasets (>1K vectors). Small datasets use HNSW-only search with no external dependencies.
```

### Basic Usage

```rust
use sqlitegraph::backend::{GraphBackend, NodeSpec};
use sqlitegraph::backend::sqlite::SqliteGraphBackend;

let graph = SqliteGraphBackend::in_memory()?;

let id = graph.insert_node(NodeSpec {
    kind: "User".to_string(),
    name: "Alice".to_string(),
    file_path: None,
    data: serde_json::json!({"age": 30}),
})?;
println!("Created node: {}", id);
```

---

## 2. Backend Selection

### SQLite Backend (Default)

**Use**: General purpose, ACID transactions, existing SQLite data

```rust
use sqlitegraph::{SqliteGraph, GraphEntity, GraphEdge};

let graph = SqliteGraph::open_in_memory()?;
// SQLite operations with full ACID compliance
```

### Native V3 Backend

**Status**: Experimental. Functional bugs should be reported.
**Use**: Large graphs, traversal-heavy workloads

```rust
use sqlitegraph::backend::native::v3::V3Backend;

let graph = V3Backend::create("graph.graph")?;
```

For weighted native-v3 traversals, warm the known source set before the first
pass and then read shared weighted neighbors without re-sorting:

```rust
use sqlitegraph::backend::{BackendDirection, NeighborQuery};
use sqlitegraph::snapshot::SnapshotId;

graph.warm_neighbors_for_sources(
    SnapshotId::current(),
    &[1, 2, 3],
    NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    },
)?;

let weighted = graph.neighbors_weighted_shared(
    SnapshotId::current(),
    1,
    NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    },
)?;
```

`neighbors_weighted_shared()` returns unfiltered neighbors in descending weight
order. `warm_neighbors_for_sources()` is useful when the caller already knows
the frontier and wants to hide the first cold edge-page pass.

### Backend Comparison

|| Characteristic | SQLite Backend | Native V3 Backend |
||----------------|----------------|-------------------|
|| **Status** | Stable | Experimental (all functional bugs should be reported) |
|| **Performance** | Standard SQLite | See [benchmarks](#benchmarks) for workload-specific behavior |
|| **Transactions** | Full ACID | Atomic commits, WAL |
|| **Memory Usage** | SQLite overhead | Configurable buffers |
|| **Use Cases** | General purpose | Traverse-heavy workloads, KV, pub/sub |

---

## 3. Core Operations

### Entity Management (SQLite Backend)

```rust
use sqlitegraph::{SqliteGraph, GraphEntity};

let graph = SqliteGraph::open_in_memory()?;

// Create entity
let entity = GraphEntity {
    id: 0,
    kind: "User".to_string(),
    name: "Alice".to_string(),
    file_path: None,
    data: serde_json::json!({"age": 30}),
};

let entity_id = graph.insert_entity(&entity)?;
let retrieved = graph.get_entity(entity_id)?;

// Update entity
let mut updated_entity = retrieved;
updated_entity.name = "Alice Smith".to_string();
graph.update_entity(&updated_entity)?;
```

### Node Management (Native V3 Backend)

```rust
use sqlitegraph::backend::{EdgeSpec, GraphBackend, NodeSpec};
use sqlitegraph::backend::native::v3::V3Backend;

let graph = V3Backend::create("graph.graph")?;

// Create node
let node_spec = NodeSpec {
    kind: "User".to_string(),
    name: "Alice".to_string(),
    file_path: None,
    data: serde_json::json!({"age": 30}),
};

let node_id = graph.insert_node(node_spec)?;

// Create edge
let edge_spec = EdgeSpec {
    from: node_id,
    to: node_id,
    edge_type: "self_ref".to_string(),
    data: serde_json::json!({"type": "demo"}),
};

let edge_id = graph.insert_edge(edge_spec)?;
```

---

## 4. Graph Algorithms

### Overview

SQLiteGraph includes a graph algorithms library with **35+ algorithms** across 13 categories:

| Category | Algorithms |
|----------|------------|
| **Core Graph Theory** | WCC, SCC, Transitive Closure, Transitive Reduction, Topological Sort |
| **Reachability** | Forward, Backward, Can-Reach, Unreachable Nodes |
| **Core CFG** | Dominators, Post-Dominators, Control Dependence |
| **Derived CFG** | Dominance Frontiers, Natural Loops |
| **Path Analysis** | Enumerate Paths, Enumerate Paths Constrained |
| **Dependency** | Critical Path, Minimal Cycle Basis |
| **Program Analysis** | Backward Slice, Forward Slice, SCC Collapse |
| **Distributed Systems** | Min Cut, Min Vertex Cut, Graph Partitioning |
| **Observability** | Happens-Before, Impact Radius |
| **ML/Inference** | Subgraph Isomorphism, Graph Rewrite, Structural Similarity |
| **Graph Diff** | Graph Diff, Validate Refactor |
| **Security** | Taint Forward, Taint Backward, Sink Analysis, Discover Sources/Sinks |

### Usage Examples

```rust
use sqlitegraph::algo;

// Core graph theory
let sccs = algo::strongly_connected_components(&graph)?;
let sorted = algo::topological_sort(&graph)?;

// Reachability
let reachable = algo::forward_reachability(&graph, start_node)?;

// CFG analysis
let dominators = algo::dominators(&graph, entry_node)?;
let loops = algo::natural_loops(&graph)?;

// Program slicing
let slice = algo::backward_slice(&graph, target_node)?;

// Security analysis
let tainted = algo::taint_forward(&graph, source_nodes)?;

// With progress tracking (for supported algorithms)
use sqlitegraph::progress::ConsoleProgress;
let scores = algo::pagerank_with_progress(&graph, 0.85, 50, ConsoleProgress::new())?;
```

### Full Documentation

**User Guides:**
- **[MIGRATION.md](docs/MIGRATION.md)** - SQLite to Native V3 migration guide
- **[TROUBLESHOOTING.md](docs/TROUBLESHOOTING.md)** - Common issues and solutions
- **[PHILOSOPHY.md](docs/PHILOSOPHY.md)** - Design principles and trade-offs

**Developer Documentation:**
- **[docs/INDEX.md](docs/INDEX.md)** - Complete documentation index
- **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)** - System architecture
- **[docs/TESTING.md](docs/TESTING.md)** - Testing guide
- **[docs/DEBUGGING.md](docs/DEBUGGING.md)** - Debugging and profiling
- **[docs/GRAPH_ALGORITHMS_GUIDE.md](docs/GRAPH_ALGORITHMS_GUIDE.md)** - Algorithm reference

### Quick Reference

| Category | Module | Example Use |
|----------|--------|-------------|
| **Core** | `algo::wcc`, `algo::scc` | Graph decomposition |
| **Reachability** | `algo::forward_reachability` | "What can this node reach?" |
| **CFG** | `algo::dominators`, `algo::natural_loops` | Control flow analysis |
| **Slicing** | `algo::backward_slice` | Program debugging |
| **Security** | `algo::taint_forward` | Security analysis |
| **ML** | `algo::subgraph_isomorphism` | Pattern matching |

### TypedDiGraph (In-Memory)

`TypedDiGraph<N, E>` is a lightweight in-memory directed graph with generic
node and edge weights. It does **not** implement `GraphBackend` — no SQLite, no
disk I/O. Use it for transient analysis passes, build DAGs, and dependency
graphs where persistence is unnecessary.

```rust
use sqlitegraph::typed_digraph::{TypedDiGraph, NodeIndex, Direction};
use sqlitegraph::typed_digraph::algo::{toposort, tarjan_scc, is_cyclic_directed, Dfs};

let mut g = TypedDiGraph::<&str, ()>::new();
let a = g.add_node("compile");
let b = g.add_node("link");
let c = g.add_node("run");
g.add_edge(a, b, ());
g.add_edge(b, c, ());

assert!(!is_cyclic_directed(&g));
let order = toposort(&g).expect("acyclic");
let sccs = tarjan_scc(&g);

let mut dfs = Dfs::new(&g, a);
let visited: Vec<NodeIndex> = dfs.by_ref().collect();
```

See [API.md](API.md#typeddigraph-api) for the full method reference.

---

## 5. Temporal Topology Analysis

> Available since 3.3.0. The temporal topology module lets you watch how the
> *shape* of your graph changes over time — when clusters appear, grow, merge,
> vanish, and when circular dependencies form and dissolve.

This section assumes you can create nodes and edges as shown in
[Section 3. Core Operations](#3-core-operations). Every idea below is
introduced as we go — no prior topology background is needed.

### 5.1 What a "version chain" is, in plain terms

Think of your graph as a living document that keeps getting edited. A
**checkpoint** is like taking a photograph of the graph at one instant: it
freezes every node and every edge exactly as they are right now. SQLiteGraph
keeps these photographs in order, forming a **version chain** — a timeline of
saved graph states, from oldest to newest.

Each checkpoint gets a **version number**, which is just a counter that goes
`1, 2, 3, …` and never goes backward. Version `1` is the first photograph you
took, version `2` the next, and so on.

Two things make this useful:

- You can look at any old photograph later and see the graph exactly as it was
  at that moment.
- Because you have many photographs in order, you can watch how the graph's
  structure *evolves* — which is what "temporal" analysis is about.

By default SQLiteGraph does **not** keep old photographs — so there is zero
overhead when you don't ask for it. You opt in by calling `checkpoint()`. The
chain is also **bounded**: it keeps the most recent 64 versions by default, and
when it fills up the oldest photograph is discarded.

### 5.2 Taking a checkpoint

Creating a checkpoint is a single call on your graph:

```rust
use sqlitegraph::SqliteGraph;

let graph = SqliteGraph::open_in_memory()?;

// ... insert nodes and edges here (see Section 3) ...

// Take a photograph of the current graph state.
let v1: u64 = graph.checkpoint();
println!("Saved version {}", v1); // e.g. "Saved version 1"
```

`graph.checkpoint()` returns the version number it just assigned. That number is
the "name" of this photograph — you'll use it to look the state up later.

You can take as many as you like, interspersed with edits:

```rust
let v1 = graph.checkpoint(); // first photograph

// ... make more edits: insert nodes/edges ...

let v2 = graph.checkpoint(); // second photograph

// ... more edits ...

let v3 = graph.checkpoint(); // third photograph
```

**One requirement to keep in mind.** A checkpoint captures the in-memory
adjacency cache — the fast lookup table of "who connects to whom." If you insert
nodes and edges and then immediately checkpoint without ever reading from the
graph, the cache may not be warmed yet and the photograph could be incomplete.
Do a read first (for example a traversal or `graph.snapshot()`) to make sure the
cache reflects all your writes, *then* checkpoint.

### 5.3 Reading the graph as it was at a checkpoint

Once you have a version number, two complementary approaches let you look back
in time.

**Option A — get the whole frozen state at once.**
`graph.snapshot_as_of(version)` returns the saved photograph directly, as a
`VersionedSnapshot`:

```rust
use sqlitegraph::VersionedSnapshot;

// Look up version 1. Returns None if it was never taken or has been evicted.
if let Some(snap): Option<VersionedSnapshot> = graph.snapshot_as_of(v1) {
    println!(
        "version {} had {} nodes and {} edges",
        snap.version,
        snap.state.node_count(),
        snap.state.edge_count(),
    );

    // The adjacency ("who points to whom") lives in snap.state.
    // For a single node:
    if let Some(neighbors) = snap.state.get_outgoing(node_id) {
        println!("node {} pointed at: {:?}", node_id, neighbors);
    }
}
```

You can also fetch **all** retained photographs at once, oldest first — this is
the input the temporal analysis functions expect:

```rust
let history: Vec<VersionedSnapshot> = graph.snapshot_versions();
println!("{} versions retained", history.len());
```

**Option B — pass the version number to a traversal.**
Many read operations accept a `SnapshotId` (a thin wrapper around a version
number). `SnapshotId::from_lsn(version)` builds one from a checkpoint version,
so you can ask questions like "what could this node reach, *as of version N*?"

Traversal methods (`neighbors`, `bfs`) live on the `GraphBackend` trait. On the
SQLite backend that type is `SqliteGraphBackend`, and it exposes the inner
`SqliteGraph` (where the version chain lives) through `.graph()`:

```rust
use sqlitegraph::SnapshotId;
use sqlitegraph::backend::{
    BackendDirection, GraphBackend, NeighborQuery, SqliteGraphBackend,
};

let backend = SqliteGraphBackend::in_memory()?;
// ... insert nodes/edges, then warm the cache with a read ...
let v1 = backend.graph().checkpoint();

// Later: traverse the graph AS OF that checkpoint, not the current live state.
let reached = backend.neighbors(
    SnapshotId::from_lsn(v1),
    node_id,
    NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    },
)?;
```

> **Scope of historical reads.** The version chain stores adjacency (the
> "who connects to whom" map), not full typed node/edge properties. So
> traversal-style reads like `neighbors()` and `bfs()` work historically, while
> property reads (such as `get_node()`) always read the current live state.

### 5.4 Reading the whole timeline: the temporal topology module

Now the interesting part. The `temporal` module takes a sequence of versions
(what you got from `graph.snapshot_versions()`) and tells you how the graph's
*topology* — its structural shape — changed across them. First, three
plain-language concepts.

**Strongly connected component (SCC).** In a directed graph, an SCC is a group
of nodes where you can walk from *any* node in the group to *any other* node in
the group by following the arrows — and come back. Picture a roundabout where
every exit also loops back to every entrance: everyone can reach everyone. A
node that isn't part of any such mutual-reachability group is an SCC of size 1
all by itself.

**Betti numbers (counts of structural features).**
- **β₀** ("beta-zero") is simply *the number of SCCs*. If your graph splits
  into 7 separate mutually-reachable groups, β₀ = 7.
- **β₁** ("beta-one") is the **cyclomatic number**: how many independent loops
  the graph has, computed as `edges − nodes + connected_pieces`. Think of it as
  "how many extra connections beyond a simple tree." A tree has β₁ = 0; every
  extra edge that closes a loop adds 1.

**Persistent homology and barcodes.** "Persistent homology" is a formal name
for a simple, powerful idea: follow a structural feature across the whole
timeline and record **when it is born** (first appears) and **when it dies**
(last seen). The result is a **barcode** — a list of bars, each like a life
`birth → death`. A long bar means a structure was stable for a long time; a
short bar means it came and went quickly. Here the "timeline" is the sequence of
checkpoint versions (the version number acts as the time axis).

The module ships four building blocks.

#### 5.4.1 The SCC landscape at each version: `temporal_persistence_sweep`

This is the overview. For every version it computes a `TemporalPersistencePoint`
describing the graph at that moment: how many nodes, how many SCCs (β₀), the
size of the biggest SCC, the cyclomatic number (β₁), and how many non-trivial
SCCs (size ≥ 2) exist.

```rust
use sqlitegraph::{temporal_persistence_sweep, TemporalPersistencePoint};

let versions = graph.snapshot_versions();
let sweep: Vec<TemporalPersistencePoint> = temporal_persistence_sweep(&versions);

for pt in &sweep {
    println!(
        "version {}: {} nodes, {} SCCs (β₀), β₁={}, {} non-trivial SCCs",
        pt.version, pt.active, pt.n_components, pt.cycle_rank, pt.n_nontrivial_sccs,
    );
}
```

A `TemporalPersistencePoint` has these fields:

| Field | Meaning |
|-------|---------|
| `version` | The version number this point describes |
| `active` | Total nodes at this version |
| `n_components` | Number of SCCs — this is β₀ |
| `largest_size` | Size of the biggest SCC |
| `fraction_largest` | `largest_size / active` (how dominant one SCC is) |
| `cycle_rank` | β₁, the cyclomatic number (`edges − nodes + pieces`) |
| `n_nontrivial_sccs` | How many SCCs have size ≥ 2 (each implies a directed cycle) |
| `largest_nontrivial_size` | Size of the biggest size-≥-2 SCC |

Read the `n_components` column over time to see your graph *fragmenting* (count
rising — things are splitting apart) or *consolidating* (count falling — things
are merging together).

#### 5.4.2 Exact component lifetimes: `scc_lineage_barcode`

The sweep tells you *how many* SCCs exist each version, but not *which is
which*. `scc_lineage_barcode` goes further: it follows individual SCCs across
versions and gives each one an exact `(birth, death)` bar. This is the precise
0-dimensional persistent homology over your version chain.

How it tells "this SCC is the same one as before" without giving them names: it
compares the **membership** (the set of node IDs) of each SCC against the
previous version's SCCs using **Jaccard similarity** — the fraction of shared
members. If two SCCs at adjacent versions overlap a lot, they're treated as the
same lineage; if an SCC appears with no overlap to anything before, a new
lineage is *born*; if a lineage's members no longer appear, it *dies*.

```rust
use sqlitegraph::scc_lineage_barcode;

let bars = scc_lineage_barcode(&versions);

for bar in &bars {
    let death = bar
        .death_version
        .map(|d| format!("died at version {}", d))
        .unwrap_or_else(|| "still alive".to_string());
    println!(
        "born at version {} (size {}), peaked at {}, {} — seen {} versions",
        bar.birth_version, bar.birth_size, bar.peak_size, death, bar.versions_seen,
    );
}
```

Each `LineageBarcode` has: `birth_version`, `death_version` (`None` if it
survived to the latest checkpoint), `birth_size`, `peak_size`, `final_size`, and
`versions_seen`. Plot these as horizontal bars on a version axis and you get the
classic persistence barcode — long bars are stable components, short ones are
fleeting.

#### 5.4.3 Circular-dependency lifecycle: `cycle_scc_barcode`

A **non-trivial SCC** (size ≥ 2) is special: because every node in it can reach
every other, there is guaranteed to be at least one directed cycle inside it.
For a code or dependency graph, a directed cycle is a **circular dependency**
(A depends on B depends on … depends on A) — usually something you want to
catch.

`cycle_scc_barcode` is the same lineage-tracking machinery as
`scc_lineage_barcode`, but it ignores lone nodes and only tracks SCCs of size
≥ 2. Its bars are the birth and death of circular-dependency clusters over the
life of your graph: when a cycle first appears, when it grows, and when a
refactor finally breaks it.

```rust
use sqlitegraph::cycle_scc_barcode;

let cycle_bars = cycle_scc_barcode(&versions);

for bar in &cycle_bars {
    println!(
        "circular dependency: versions {}–{:?}, peak size {}",
        bar.birth_version, bar.death_version, bar.peak_size,
    );
}
```

#### 5.4.4 The cyclomatic number at one moment: `cycle_rank_snapshot`

Sometimes you don't want a whole timeline — you want β₁ for a single snapshot.
`cycle_rank_snapshot` takes one `SnapshotState` (the `.state` inside any
`VersionedSnapshot`) and returns `edges − nodes + connected_pieces`:

```rust
use sqlitegraph::cycle_rank_snapshot;

if let Some(snap) = graph.snapshot_as_of(v2) {
    let beta1: usize = cycle_rank_snapshot(&snap.state);
    println!("β₁ (independent cycles) at version {} = {}", v2, beta1);
}
```

**A useful distinction.** β₁ counts *undirected* cycles — any loop at all,
including a two-way street (A→B and B→A). A *directed* cycle (a true circular
dependency) is a stricter condition, and is exactly what `cycle_scc_barcode`
reports via non-trivial SCCs. Pair the two: rising β₁ with few non-trivial SCCs
means growing *structural* redundancy (lots of back-edges) but no real circular
deps; both rising means real dependency cycles are forming.

### 5.5 Putting it together: a full pass

A complete temporal analysis is just "checkpoint as you go, then run the
analysis on the history":

```rust
use sqlitegraph::{
    SqliteGraph, temporal_persistence_sweep, scc_lineage_barcode, cycle_scc_barcode,
};

let graph = SqliteGraph::open_in_memory()?;

// Build the graph over several stages, checkpointing after each meaningful
// change. (Warm the cache with a read before each checkpoint — see 5.2.)
let mut versions = Vec::new();
versions.push(graph.checkpoint()); // stage 1
// ... edits, then a read ...
versions.push(graph.checkpoint()); // stage 2
// ... edits, then a read ...
versions.push(graph.checkpoint()); // stage 3

// Load the full retained history (oldest first).
let history = graph.snapshot_versions();

// 1. Per-version landscape.
let sweep = temporal_persistence_sweep(&history);
// 2. Exact component lifetimes.
let component_bars = scc_lineage_barcode(&history);
// 3. Circular-dependency lifetimes.
let cycle_bars = cycle_scc_barcode(&history);

println!(
    "{} sweep points, {} component bars, {} cycle bars",
    sweep.len(),
    component_bars.len(),
    cycle_bars.len(),
);
```

**Practical tips:**

- Checkpoint at *meaningful* moments (a commit, a refactor, a release) rather
  than after every tiny edit — the analysis is most readable when each version
  represents a real state you care about.
- Because the chain is bounded (default 64), checkpoint strategically if you
  need to look far back. Query `graph.snapshot_version_count()` and
  `graph.snapshot_oldest_version()` to see what's still retained.
- For a quick "is the graph getting more or less tangled over time?" answer, the
  `cycle_rank` (β₁) and `n_nontrivial_sccs` columns of the sweep are the two
  numbers to watch.

---

## 6. Testing

### Running Tests

```bash
# All tests
cargo test --workspace

# With Native V3 backend
cargo test --workspace --features native-v3

# Specific test patterns
cargo test '*pagerank*'
cargo test '*wal*'
```

### Test Coverage

**Known test coverage:**
- 180+ graph algorithm tests passing (35 algorithms across 13 categories)
- 59 pubsub tests passing (event emission, filtering, multiple subscribers)
- 42 WAL tests passing (recovery, corruption, checkpoints)
- 53 concurrent MVCC tests passing (snapshots, stress testing)
- 134 HNSW tests passing
- 65 MVCC lifecycle tests passing

**Total**: 530+ tests passing

---

## 7. Performance

### Native V3 Performance

Performance depends heavily on graph shape, cache state, storage medium, and
which benchmark harness you use. The project now distinguishes:

- release-mode microbenchmarks for quick sanity checks
- curated Criterion comparisons for fast backend checks
- Criterion suites for workload comparisons

Use the commands and workflow in [docs/BENCHMARKING.md](docs/BENCHMARKING.md)
instead of treating any single table in this manual as universal.

### Parallel WAL Recovery

```rust
use sqlitegraph::{GraphConfig, open_graph};

// Default: 4 threads
let config = GraphConfig::native();
let graph = open_graph("large.db", &config)?;

// Custom: 8 threads
let config = GraphConfig::native().with_parallel_recovery(8);
let graph = open_graph("large.db", &config)?;
```

**Performance**:
- Measure on your real workload using the benchmarking workflow in
  [docs/BENCHMARKING.md](docs/BENCHMARKING.md)
- Recovery speedup depends on WAL size, storage medium, and contention

### V3 Performance Improvements

#### LRU Node Caching

Automatic LRU caching for node lookups:

```rust
use sqlitegraph::backend::native::v3::NodeCache;

// Cache is automatic - no configuration needed
// Default: 1000 nodes, 85-95% hit rate for traversals

// Warm-cache behavior is workload-dependent; benchmark on your hardware
let node = backend.get_node_by_id(node_id)?;
```

**Performance Impact:**
- Warm-cache lookups can improve substantially, but the exact gain is
  workload-dependent
- Validate hit rate and memory tradeoffs with the benchmark workflow in
  [docs/BENCHMARKING.md](docs/BENCHMARKING.md)

#### Parallel BFS

Level-wise parallel BFS for large graphs:

```rust
use sqlitegraph::backend::native::v3::algorithm::parallel_bfs;

// Automatic parallelization for large graphs
let result = parallel_bfs(&backend, start_node, None)?;

// Sequential fallback for small graphs (< 1000 nodes)
// No manual configuration needed
```

**Performance Impact:**
- Feature implemented
- Small graphs (<1K nodes): sequential fallback to avoid overhead
- Measure on your workload before claiming a speedup

#### Adaptive Page Sizing

Storage media detection for optimal page size:

```rust
// Automatic based on storage type
// SSD: 4KB pages (feature implemented, not verified)
// HDD: 16KB pages (feature implemented, not verified)
// Unknown: 8KB fallback

// No configuration needed
```

**Performance Impact:**
- Designed to reduce storage-mismatch overhead
- Verify on the target disk using the benchmark workflow before quoting gains

#### Delta Encoding

Edge ID compression for storage efficiency:

```rust
// Automatic compression - no API needed
// 87.5% space savings for sequential edge IDs
// 8:1 compression ratio typical
```

---

## 8. Error Handling

### Common Error Types

```rust
use sqlitegraph::SqliteGraphError;

match graph.insert_entity(&entity) {
    Ok(id) => println!("Created: {}", id),
    Err(SqliteGraphError::ValidationError(msg)) => {
        eprintln!("Validation failed: {}", msg);
    }
    Err(SqliteGraphError::ConnectionError(msg)) => {
        eprintln!("Connection failed: {}", msg);
    }
    Err(err) => eprintln!("Error: {}", err),
}
```

### Debug Features

```toml
# Enable debug logging / native backend diagnostics
sqlitegraph = { version = "3.2", features = ["debug", "v3-forensics"] }
```

```bash
# Run with debug output
RUST_LOG=debug cargo run --features debug,v3-forensics
```

---

## 9. Vector Search (HNSW)

### Basic HNSW Usage

```rust
use sqlitegraph::hnsw::{HnswConfig, DistanceMetric, HnswIndex};

let config = HnswConfig::builder()
    .dimension(1536)
    .distance_metric(DistanceMetric::Cosine)
    .build()?;

let hnsw = HnswIndex::new(config)?;

// Insert vector
let vector_id = hnsw.insert_vector(&embedding, Some(metadata))?;

// Search
let results = hnsw.search(&query, k)?;
```

### Distance Metrics

| Metric | Best For | Speed |
|--------|----------|-------|
| **Cosine** | Text embeddings | Fast |
| **Euclidean** | General similarity | Medium |
| **Dot Product** | Normalized vectors | Very fast |
| **Manhattan** | Sparse vectors | Slow |

### CLI Commands

```bash
# Create index
sqlitegraph --backend sqlite --db mygraph.db --write hnsw create \
    --name vectors --dim 3 --metric cosine

# Insert vectors
sqlitegraph --backend sqlite --db mygraph.db --write hnsw insert \
    --name vectors --vector "0.1,0.2,0.3"

# Search
sqlitegraph --backend sqlite --db mygraph.db hnsw search \
    --name vectors --vector "0.1,0.2,0.3" --k 10

# List indexes
sqlitegraph --backend sqlite --db mygraph.db hnsw list
```

---

## 10. KV Store (Key-Value Storage)

### Overview

The Native V3 backend includes a transactional key-value store for storing arbitrary data alongside your graph. The KV store participates in transactions and emits events through the pub/sub system.

### Availability

| Backend | KV Store Support |
|---------|------------------|
| **Native V3** | Full support |
| **SQLite** | Full support |

### Basic Usage

```rust
use sqlitegraph::{GraphConfig, open_graph};
use sqlitegraph::backend::KvValue;

let config = GraphConfig::native();
let graph = open_graph("graph.db", &config)?;

// Set a value
graph.kv_set(
    b"user:123:name".to_vec(),
    KvValue::String("Alice".to_string()),
    None,  // No TTL
)?;

// Get a value (requires snapshot_id)
let snapshot = graph.snapshot()?;
if let Some(KvValue::String(name)) = graph.kv_get(snapshot.id, b"user:123:name")? {
    println!("User name: {}", name);
}

// Delete a value
graph.kv_delete(b"user:123:name")?;
```

### Value Types

The `KvValue` enum supports multiple data types:

| Type | Rust Value | Example |
|------|------------|---------|
| `Bytes` | `Vec<u8>` | Raw binary data |
| `String` | `String` | Text values |
| `Integer` | `i64` | 64-bit integers |
| `Float` | `Float` | Floating point numbers |
| `Boolean` | `bool` | True/false |
| `Json` | `serde_json::Value` | JSON objects, arrays |

```rust
use sqlitegraph::backend::KvValue;
use serde_json::json;

// Different value types
graph.kv_set(b"counter".to_vec(), KvValue::Integer(42), None)?;
graph.kv_set(b"price".to_vec(), KvValue::Float(19.99), None)?;
graph.kv_set(b"active".to_vec(), KvValue::Boolean(true), None)?;
graph.kv_set(b"config".to_vec(), KvValue::Json(json!({
    "theme": "dark",
    "notifications": true
})), None)?;
graph.kv_set(b"binary".to_vec(), KvValue::Bytes(vec![0x00, 0xFF, 0xAA]), None)?;
```

### TTL (Time-To-Live)

Keys can be set with an optional TTL in seconds. After expiration, the key returns `None`.

```rust
// Set a key that expires in 60 seconds
graph.kv_set(
    b"temp_session".to_vec(),
    KvValue::String("active".to_string()),
    Some(60),  // Expires in 60 seconds
)?;

// Set a key that expires in 1 hour
graph.kv_set(
    b"cache:data".to_vec(),
    KvValue::Bytes(cached_data),
    Some(3600),  // 1 hour
)?;
```

### Transactional Behavior

KV operations are **atomic** with graph operations within the same transaction:

```rust
// Create a node and store metadata atomically
let node_id = graph.insert_node(NodeSpec {
    kind: "User".to_string(),
    name: "alice".to_string(),
    file_path: None,
    data: json!({"age": 30}),
})?;

// Store metadata in KV - commits with the transaction
graph.kv_set(
    format!("user_metadata:{}", node_id).into_bytes(),
    KvValue::Json(json!({
        "created_at": "2026-02-03",
        "verified": true
    })),
    None,
)?;

// If commit fails, both node and KV data are rolled back
```

### Use Cases

#### 1. Secondary Indexes

```rust
// Index users by email for fast lookup
let node_id = graph.insert_node(user_spec)?;
graph.kv_set(
    format!("index:email:{}", user_email).into_bytes(),
    KvValue::Integer(node_id),
    None,
)?;

// Later, find user by email
if let Some(KvValue::Integer(node_id)) = graph.kv_get(snapshot.id, b"index:email:alice@example.com")? {
    let user = graph.get_node(snapshot.id, node_id)?;
}
```

#### 2. Caching

```rust
// Cache expensive computation results
graph.kv_set(
    b"cache:expensive_result".to_vec(),
    KvValue::Json(json!(result)),
    Some(300),  // Cache for 5 minutes
)?;
```

#### 3. Counters and Aggregates

```rust
// Track counts (read-modify-write pattern)
let count = match graph.kv_get(snapshot.id, b"counter:requests")? {
    Some(KvValue::Integer(n)) => n + 1,
    _ => 1,
};
graph.kv_set(b"counter:requests".to_vec(), KvValue::Integer(count), None)?;
```

#### 4. Configuration

```rust
// Store application configuration
graph.kv_set(
    b"config:max_connections".to_vec(),
    KvValue::Integer(100),
    None,
);
graph.kv_set(
    b"config:debug_mode".to_vec(),
    KvValue::Boolean(false),
    None,
);
```

### Pub/Sub Integration

KV changes emit `KVChanged` events for subscribers:

```rust
use sqlitegraph::backend::{SubscriptionFilter, PubSubEvent};

let filter = SubscriptionFilter::all();
let (sub_id, rx) = graph.subscribe(filter)?;

// In a separate thread
while let Ok(event) = rx.recv() {
    if let PubSubEvent::KVChanged { key_hash, snapshot_id } = event {
        println!("KV changed: hash={}, snapshot={}", key_hash, snapshot_id);
    }
}
```

### Metadata

Each KV entry has internal metadata (not directly exposed):
- `created_at` - Unix timestamp when key was created
- `updated_at` - Unix timestamp of last update
- `ttl_seconds` - TTL if set
- `version` - Monotonically increasing version number

### Limitations

- **Current snapshot only**: Historical KV reads are not exposed through the public API.
- **Byte keys**: Keys are `Vec<u8>` - use string encoding for text keys
- **No snapshots**: Can't query historical KV values, only current snapshot
- **Full enumeration**: No API to enumerate all keys without prefix (use `kv_prefix_scan(b"")` for all keys)

### KV Prefix Scanning

The `kv_prefix_scan()` method enables efficient retrieval of all KV entries with a given prefix:

```rust
use sqlitegraph::{GraphConfig, open_graph};

let config = GraphConfig::native();
let graph = open_graph("graph.db", &config)?;

// Get all keys with prefix "user:"
let snapshot = graph.snapshot()?;
let results = graph.kv_prefix_scan(snapshot.id, b"user:")?;

for (key, value) in results {
    println!("{:?} = {:?}", String::from_utf8_lossy(&key), value);
}

// Get all KV entries (empty prefix)
let all_entries = graph.kv_prefix_scan(snapshot.id, b"")?;
```

**Features:**
- Prefix matching for hierarchical key organization
- Results returned in lexicographic order
- MVCC snapshot isolation respected
- TTL filtering for expired entries

**Use Cases:**
- Secondary index enumeration (e.g., `index:user:*` → all user IDs)
- Hierarchical data retrieval (e.g., `cache:region:*` → all regional caches)
- Namespace-based key management

`kv_prefix_scan()` is currently documented as a library API. The public CLI does
not expose KV scan commands.

---

## 11. Query API Enhancements

### Overview

The query helper APIs support working with graph data without maintaining
external ID tracking for every lookup. These features are useful for pub/sub
use cases like agent messaging and topic-based subscriptions.

### Query Nodes by Kind

Find all nodes with a specific kind:

```rust
use sqlitegraph::{GraphConfig, open_graph};

let config = GraphConfig::native();
let graph = open_graph("graph.db", &config)?;

// Find all agent nodes
let snapshot = graph.snapshot()?;
let agent_ids = graph.query_nodes_by_kind(snapshot.id, "agent")?;

println!("Found {} agents", agent_ids.len());
for node_id in agent_ids {
    let node = graph.get_node(snapshot.id, node_id)?;
    println!("  - {}: {:?}", node.name, node.data);
}
```

**Features:**
- Direct kind filtering without full graph scan
- Works on both SQLite and Native V3 backends
- Returns sorted node IDs for consistent output
- MVCC snapshot isolation respected

**CLI:**
```bash
# Find all nodes with kind "agent"
sqlitegraph --db mygraph.db list --kind agent

# Find all message nodes
sqlitegraph --db mygraph.db list --kind message
```

### Query Nodes by Name Pattern

Find nodes using glob patterns (`*` matches any sequence, `?` matches single character):

```rust
// Find all nodes with name matching "msg_index:*"
let msg_ids = graph.query_nodes_by_name_pattern(snapshot.id, "msg_index:*")?;

// Find nodes with pattern "agent-?" (single digit)
let agent_ids = graph.query_nodes_by_name_pattern(snapshot.id, "agent-?")?;

// Escape wildcards for literal matching
let literal = graph.query_nodes_by_name_pattern(snapshot.id, "file\\*.txt")?;
```

**Pattern Syntax:**
| Pattern | Matches | Does Not Match |
|---------|---------|----------------|
| `msg_index:*` | `msg_index:agent-1`, `msg_index:agent-2` | `Message_Index:agent-1` |
| `agent-?` | `agent-1`, `agent-A` | `agent-12`, `agent-` |
| `\*test\?` | `*test?` | `test`, `123testX` |

**CLI:**
```bash
# Find nodes matching "msg_index:*"
sqlitegraph mygraph.db nodes-by-name --pattern "msg_index:*"

# Find nodes with single-character suffix
sqlitegraph mygraph.db nodes-by-name --pattern "agent-?"
```

### Pub/Sub Pattern Filters

Subscribe to events matching glob patterns on node kind or name:

```rust
use sqlitegraph::backend::SubscriptionFilter;

// Subscribe to all agent events (kind pattern)
let filter = SubscriptionFilter::kind_patterns(vec!["agent:*".to_string()]);
let (sub_id, rx) = graph.subscribe(filter)?;

// Subscribe to message index events (name pattern)
let filter = SubscriptionFilter::name_patterns(vec!["msg_index:*".to_string()]);
let (sub_id, rx) = graph.subscribe(filter)?;

// Multiple patterns
let filter = SubscriptionFilter::kind_patterns(vec![
    "agent:*".to_string(),
    "message:*".to_string(),
    "system:*".to_string(),
]);
let (sub_id, rx) = graph.subscribe(filter)?;
```

**How Pattern Matching Works:**
1. When a node event occurs (creation/modification), the node's kind/name is checked
2. Patterns are evaluated in order; if any pattern matches, the event is delivered
3. Matching is case-sensitive
4. Supports `*` (any sequence, including empty) and `?` (exactly one character)
5. Escape with `\*` and `\?` for literal asterisk/question mark

**Use Cases:**
- **Agent Messaging**: Subscribe to `agent:*` to receive all agent events
- **Topic-Based Pub/Sub**: Subscribe to `msg_index:agent-*` for specific agent messages
- **Hierarchical Organization**: Subscribe to `cache:region-*` for regional cache events
- **Dynamic Discovery**: Find nodes by pattern without maintaining ID registries

### Query API Use Cases

#### 1. Agent Messaging System

```rust
// Create message queue node
let msg_node_id = graph.insert_node(NodeSpec {
    kind: "message_queue".to_string(),
    name: "msg_index:agent-123".to_string(),
    file_path: None,
    data: json!({"owner": "agent-123"}),
})?;

// Subscribe to this agent's messages
let filter = SubscriptionFilter::name_patterns(vec!["msg_index:agent-123".to_string()]);
let (sub_id, rx) = graph.subscribe(filter)?;

// Or subscribe to all agents' messages
let filter = SubscriptionFilter::name_patterns(vec!["msg_index:*".to_string()]);
let (sub_id, rx) = graph.subscribe(filter)?;
```

#### 2. Dynamic Entity Discovery

```rust
// Find all agents without maintaining ID registry
let agent_ids = graph.query_nodes_by_kind(snapshot.id, "agent")?;

for agent_id in agent_ids {
    let agent = graph.get_node(snapshot.id, agent_id)?;
    println!("Active agent: {}", agent.name);
}
```

#### 3. Hierarchical KV Indexing

```rust
// Index users by email
graph.kv_set(
    format!("index:email:{}", email).into_bytes(),
    KvValue::Integer(user_id),
    None,
)?;

// Later, enumerate all users in the index
let all_users = graph.kv_prefix_scan(snapshot.id, b"index:email:")?;
for (key, value) in all_users {
    let email = String::from_utf8_lossy(&key).replace("index:email:", "");
    println!("User: {} -> ID: {:?}", email, value);
}
```

#### 4. Topic-Based Subscriptions

```rust
// Subscribe to all cache events
let cache_filter = SubscriptionFilter::kind_patterns(vec!["cache:*".to_string()]);
let (sub_id, rx) = graph.subscribe(cache_filter)?;

// Subscribe to specific cache region
let region_filter = SubscriptionFilter::name_patterns(vec!["cache:region-us-west".to_string()]);
let (sub_id, rx) = graph.subscribe(region_filter)?;
```

### Backend Support

| Feature | Native V3 | SQLite |
|---------|-----------|--------|
| `kv_prefix_scan()` | Full support | Full support |
| `query_nodes_by_kind()` | Full support | Full support |
| `query_nodes_by_name_pattern()` | Full support | Full support |
| Pattern subscription filters | Full support | Full support |

### Performance Characteristics

| Operation | Complexity | Notes |
|-----------|------------|-------|
| `kv_prefix_scan()` | O(K) where K = keys scanned | Faster with specific prefixes |
| `query_nodes_by_kind()` | O(N) where N = total nodes | Full scan required |
| `query_nodes_by_name_pattern()` | O(N) where N = total nodes | Full scan + pattern match |
| Pattern matching | O(P) where P = patterns | Per-event, cheap |

**Optimization Tips:**
- Use specific prefixes for `kv_prefix_scan()` to avoid scanning all keys
- Consider maintaining a separate index if querying by kind/name frequently
- Use kind-based filtering before name patterns for hierarchical filtering

---

## 12. Developer Tools

### Introspection API

```rust
use sqlitegraph::introspection::GraphIntrospection;

let intro = GraphIntrospection::new(&graph)?;
println!("Nodes: {}", intro.node_count()?);
println!("Edges (estimated): {}", intro.edge_count_estimate()?);
println!("JSON: {}", intro.to_json()?);
```

### Progress Tracking

```rust
use sqlitegraph::progress::{ProgressCallback, ConsoleProgress};

// No-op (default)
let scores = algo::pagerank_with_progress(&graph, 0.85, 50, NoProgress)?;

// Console progress bars
let scores = algo::pagerank_with_progress(&graph, 0.85, 50, ConsoleProgress::new())?;
```

### CLI Inspection Commands

```bash
sqlitegraph --backend sqlite --db mygraph.db status
sqlitegraph --backend sqlite --db mygraph.db list
sqlitegraph --backend sqlite --db mygraph.db list --kind User
```

---

## 13. Safety & Integrity

### Safety Checks

```rust
use sqlitegraph::run_safety_checks;

let report = run_safety_checks(&graph)?;
if report.has_orphans() {
    eprintln!("Warning: {} orphan edges", report.orphan_count());
}
```

### V3 WAL Recovery

The Native V3 backend includes WAL recovery with:
- Transaction rollback for all operations
- Edge cascade cleanup on node deletion
- Cluster reference cleanup

**Tested**: 42 WAL tests passing (recovery, corruption, checkpoints)

---

## 14. CLI Usage

### Available Commands

```bash
# Status
sqlitegraph --db mygraph.db status

# List entities
sqlitegraph --db mygraph.db list

# Export/Import
sqlitegraph --db mygraph.db --write export --output backup.json
sqlitegraph --db mygraph.db --write import --input backup.json

# Query language
sqlitegraph --db mygraph.db query 'MATCH (n:User) RETURN n.name'
sqlitegraph --db mygraph.db --write query 'CREATE (1)-[:KNOWS]->(2)'

# Graph algorithms (35+ algorithms available)
sqlitegraph --backend sqlite --db mygraph.db algo pagerank --iterations 100
sqlitegraph --backend sqlite --db mygraph.db algo betweenness
sqlitegraph --backend sqlite --db mygraph.db algo components
sqlitegraph --backend sqlite --db mygraph.db algo scc
sqlitegraph --backend sqlite --db mygraph.db algo louvain --max-iterations 100
sqlitegraph --backend sqlite --db mygraph.db algo label-prop --max-iterations 50
sqlitegraph --backend sqlite --db mygraph.db algo cycles --limit 100

# CFG analysis
sqlitegraph --backend sqlite --db mygraph.db algo dominators --entry 1
sqlitegraph --backend sqlite --db mygraph.db algo critical-path

# HNSW vector indexes
sqlitegraph --db mygraph.db --write hnsw create --name embeddings --dim 3 --metric cosine
sqlitegraph --db mygraph.db --write hnsw insert --name embeddings --vector "1.0,0.8,0.1"
sqlitegraph --db mygraph.db hnsw search --name embeddings --k 5 --vector "1.0,0.9,0.0"
sqlitegraph --db mygraph.db hnsw list
sqlitegraph --db mygraph.db --write hnsw delete --name embeddings

# For complete query details, see docs/QUERY_LANGUAGE.md
```

---

## 15. Migration

### SQLite to Native V3

```rust
// Before (SQLite)
let graph = SqliteGraph::open("data.db")?;
let entity = GraphEntity { /* fields */ };
let id = graph.insert_entity(&entity)?;

// After (Native V3)
let config = GraphConfig::native();
let graph = open_graph("data.db", &config)?;
let node_spec = NodeSpec { /* similar fields */ };
let id = graph.insert_node(node_spec)?;
```

### Key Differences

| Aspect | SQLite Backend | Native V3 Backend |
|--------|----------------|-------------------|
| **Data Types** | `GraphEntity`/`GraphEdge` | `NodeSpec`/`EdgeSpec` |
| **Edge Fields** | `from_id`/`to_id` | `from`/`to` |
| **Construction** | `SqliteGraph::open()` | `open_graph(&config)` |

---

## 16. Troubleshooting

### Common Issues

**Compilation Errors:**
- Add feature flags: `--features native-v3`
- Check backend-specific data types

**Runtime Issues:**
- Run integrity checks through the library safety-check helpers.
- Check buffer configuration for large graphs

**Performance:**
- Use Native V3 for traversals
- Enable parallel WAL recovery for large databases

### Getting Help

```bash
# Check test status
cargo test --lib 2>&1 | tail -5

# Run specific test with output
cargo test test_name -- --nocapture

# Check compilation
cargo check --features native-v3
```

---

## 17. Pub/Sub Events

### Overview

The Native V3 backend includes an in-process publish/subscribe system for receiving notifications when graph data changes. Events are emitted when transactions commit and carry only identifiers (not full data payloads).

### Availability

| Backend | Pub/Sub Support |
|---------|-----------------|
| **Native V3** | Full support |
| **SQLite** | Full support |

### Event Types

Four event types are emitted on transaction commit:

| Event Type | Fields | Description |
|------------|--------|-------------|
| `NodeChanged` | `node_id`, `snapshot_id` | A node was created or modified |
| `EdgeChanged` | `edge_id`, `snapshot_id` | An edge was created or modified |
| `KVChanged` | `key_hash`, `snapshot_id` | A KV entry was created, modified, or deleted |
| `SnapshotCommitted` | `snapshot_id` | A transaction was committed |

**Important**: Events are emitted on **commit only**, not on rollback.

### Basic Usage

```rust
use sqlitegraph::{GraphConfig, open_graph};
use sqlitegraph::backend::{SubscriptionFilter, PubSubEvent};

let cfg = GraphConfig::native();
let graph = open_graph("graph.db", &cfg)?;

// Subscribe to all events
let filter = SubscriptionFilter::all();
let (subscriber_id, rx) = graph.subscribe(filter)?;

// In a separate task/thread, receive events
std::thread::spawn(move || {
    while let Ok(event) = rx.recv() {
        match event {
            PubSubEvent::NodeChanged { node_id, snapshot_id } => {
                println!("Node {} changed in snapshot {}", node_id, snapshot_id);
            }
            PubSubEvent::EdgeChanged { edge_id, snapshot_id } => {
                println!("Edge {} changed in snapshot {}", edge_id, snapshot_id);
            }
            PubSubEvent::KVChanged { key_hash, snapshot_id } => {
                println!("KV hash {} changed in snapshot {}", key_hash, snapshot_id);
            }
            PubSubEvent::SnapshotCommitted { snapshot_id } => {
                println!("Transaction committed: snapshot {}", snapshot_id);
            }
        }
    }
});

// Unsubscribe when done
graph.unsubscribe(subscriber_id)?;
```

### Filtering Events

You can filter events by type and/or specific entity IDs:

```rust
use sqlitegraph::backend::{SubscriptionFilter, PubSubEventType};

// Subscribe only to node events
let node_filter = SubscriptionFilter::event_types(vec![PubSubEventType::Node]);
let (id, rx) = graph.subscribe(node_filter)?;

// Subscribe only to specific node IDs
let specific_nodes = SubscriptionFilter::nodes(vec![1, 2, 3]);
let (id, rx) = graph.subscribe(specific_nodes)?;

// Subscribe to node AND edge events
let multi_filter = SubscriptionFilter::event_types(vec![
    PubSubEventType::Node,
    PubSubEventType::Edge,
]);
let (id, rx) = graph.subscribe(multi_filter)?;
```

### SubscriptionFilter API

| Method | Description |
|--------|-------------|
| `all()` | Match all events |
| `nodes(ids)` | Match only specific node IDs |
| `edges(ids)` | Match only specific edge IDs |
| `keys(hashes)` | Match only specific key hashes |
| `event_types(types)` | Match specific event types |

### ID-Only Design

Events carry only identifiers, not full entity data. This design:

- **Reduces overhead**: Events are lightweight (just IDs)
- **Ensures consistency**: Consumers read from a specific snapshot
- **Decouples schema**: Event structure doesn't change when entities change

To read actual data, use the provided `snapshot_id`:

```rust
// Event gives you the ID
if let PubSubEvent::NodeChanged { node_id, snapshot_id } = event {
    // Query the graph for actual data at that snapshot
    let node = graph.get_node_at_snapshot(node_id, snapshot_id)?;
}
```

### Limitations

The pub/sub system is **minimal and best-effort**:

- **In-process only**: No networking or IPC support
- **No Persistence**: Events are lost if the process crashes
- **No Delivery Guarantees**: Events dropped if channel is full or receiver is gone
- **No Ordering**: Subscribers may receive events in different orders
- **Backend support**: SQLite and Native V3 publish events inside the current process.

### Thread Safety

- Multiple threads can safely subscribe/unsubscribe concurrently
- Each subscriber gets their own channel
- `Publisher` uses `Arc<Mutex<>>` for internal synchronization

### Query API Notes

The current `GraphBackend` API includes direct ID lookups plus helper methods
such as `query_nodes_by_kind()` and `query_nodes_by_name_pattern()`. For dynamic
application entities, it can still be useful to keep explicit secondary indexes
in KV:

```rust
// Create node and track the ID
let node_id = graph.insert_node(NodeSpec {
    kind: "message_index".to_string(),
    name: "msg_index:agent-123".to_string(),
    file_path: None,
    data: serde_json::json!({}),
})?;

// Store ID in KV for later lookup
graph.kv_set(
    b"msg_index_id:agent-123",
    KvValue::Integer(node_id),
    None,
)?;
```

For message queues, user sessions, or other dynamic entities, maintain an index
in KV:

```rust
// Index pattern: store entity IDs in KV
// Key: "index:{entity_type}:{entity_name}" -> node_id
// Data: "index_data:{entity_type}:{entity_name}" -> JSON with actual state

graph.kv_set(
    b"index:messages:agent-123",
    KvValue::Integer(node_id),
    None,
)?;
```

When subscribing to events for specific nodes, use `SubscriptionFilter.nodes()`
with the IDs returned during node creation.

**Why this design?**

- **Simplicity**: Keeps direct ID lookups explicit
- **Performance**: Direct ID lookup is O(1)
- **Explicit tracking**: Users control their own index strategies
- **Extensibility**: KV store provides indexing options

### Test Coverage

**Known test coverage:**
- 59 pubsub tests passing (integration + module tests)
- Tests cover: event emission, filtering, multiple subscribers, unsubscribe

---

## Architecture Status

**Current feature areas:**
- Graph Algorithms Library: 35 graph algorithms across 13 categories
  - Core Graph Theory: WCC, SCC, Transitive Closure, Transitive Reduction, Topological Sort
  - Reachability: Forward, Backward, Can-Reach, Unreachable Nodes
  - CFG Analysis: Dominators, Post-Dominators, Control Dependence, Dominance Frontiers, Natural Loops
  - Path Analysis: Enumerate Paths, Dominance-Constrained Path Enumeration
  - Dependency Systems: Critical Path, Minimal Cycle Basis
  - Program Analysis: Backward/Forward Slicing, SCC Collapse for Call Graphs
  - Distributed Systems: Min Cut, Min Vertex Cut, Graph Partitioning
  - Observability: Happens-Before Analysis, Impact Radius
  - ML/Inference: Subgraph Isomorphism, Graph Rewriting, Structural Similarity
  - Graph Diff: Structural Delta, Refactor Validation
  - Security: Taint Propagation (Forward/Backward), Sink Analysis, Source/Sink Discovery
  - CLI commands for the most commonly used algorithms, with additional algorithms available through the Rust API

**Storage and runtime:**
- SQLite backend: ACID storage, SQL tooling compatibility, query-language execution.
- Native V3 backend: graph-oriented file format, WAL/checkpoint durability, KV support.
- Pub/Sub events: in-process notifications for node, edge, KV, and snapshot changes.
- HNSW vector search: persistent vector indexes with create/insert/search/list/delete surfaces.
- CLI and Python bindings: query-language access, algorithms, and vector-index operations.

**Test Coverage:** 530+ tests passing in the documented baseline, including 180+ algorithm tests and 59 pubsub tests.
