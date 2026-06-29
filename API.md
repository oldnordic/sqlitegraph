# SQLiteGraph API Reference

**Version:** 3.4.6
**Last Updated:** 2026-06-29

The complete API documentation is available at **[docs.rs/sqlitegraph](https://docs.rs/sqlitegraph)**.

This document provides a quick overview of the current public API surface.

---

## Backend Quick Reference

| Backend | Status | Module | Best For |
|---------|--------|--------|----------|
| **SQLite** | ✅ Stable | `backend::sqlite` | Debuggable, SQL ecosystem |
| **Native V3** | ⚠️ Experimental | `backend::native::v3` | Graph-oriented storage, packed edge store, KV, pub/sub (Bugs should be reported) |

---

## Table of Contents

- [GraphBackend Trait (Unified API)](#graphbackend-trait-unified-api)
- [SQLite Backend API](#sqlite-backend-api)
- [Native V3 Backend API](#native-v3-backend-api)
- [Graph Algorithms API](#graph-algorithms-api)
- [Cypher-Inspired Query API](#cypher-inspired-query-api)
- [HNSW Vector Search API](#hnsw-vector-search-api)
- [KV Store API](#kv-store-api)
- [Pub/Sub API](#pubsub-api)
- [Temporal Topology API](#temporal-topology-api)
- [MVCC Snapshot Tracking API](#mvcc-snapshot-tracking-api)

---

## GraphBackend Trait (Unified API)

All backends implement `GraphBackend` - use this trait for backend-agnostic code:

```rust
use sqlitegraph::backend::{GraphBackend, NodeSpec, EdgeSpec};

fn create_user(backend: &dyn GraphBackend, name: &str) -> Result<i64, SqliteGraphError> {
    backend.insert_node(NodeSpec {
        kind: "User".to_string(),
        name: name.to_string(),
        file_path: None,
        data: serde_json::json!({"created": "now"}),
    })
}

// Works with any backend:
let sqlite = SqliteGraphBackend::in_memory()?;
let v3 = V3Backend::create("data.graph")?;

create_user(&sqlite, "Alice")?;
create_user(&v3, "Bob")?;
```

### Core Trait Methods

| Method | Description |
|--------|-------------|
| `insert_node(spec)` | Insert node, returns ID |
| `insert_nodes_bulk(items)` | Insert many nodes atomically |
| `insert_edge(spec)` | Insert edge, returns ID |
| `insert_edges_bulk(items)` | Insert many edges atomically |
| `neighbors(snapshot, node, query)` | Get neighbors with direction filter |
| `bfs_filtered(snapshot, start, depth, edge_types, direction)` | BFS restricted to edge types |
| `shortest_path_filtered(snapshot, start, end, edge_types)` | Shortest path restricted to edge types |
| `entity_ids()` | Get all node IDs |
| `subscribe(filter)` | Subscribe to events (Pub/Sub) |
| `kv_get(snapshot, key)` | Get KV value |
| `kv_set(key, value, ttl)` | Set KV value |

---

## SQLite Backend API

**Status:** Stable, mature, debuggable

```rust
use sqlitegraph::backend::sqlite::SqliteGraphBackend;

// In-memory (testing)
let backend = SqliteGraphBackend::in_memory()?;

// From existing SqliteGraph
let backend = SqliteGraphBackend::from_graph(graph);

// Access underlying graph for SQL queries
let graph = backend.graph();
```

### Pub/Sub

```rust
use sqlitegraph::backend::{SubscriptionFilter, PubSubEvent};

let (sub_id, rx) = backend.subscribe(SubscriptionFilter::all())?;

// Events emitted on insert_node/insert_edge
backend.insert_node(NodeSpec { ... })?; // Emits NodeChanged
```

### HNSW Vector Storage

```rust
use sqlitegraph::hnsw::storage::SQLiteVectorStorage;

let storage = SQLiteVectorStorage::new(index_id, conn);
```

---

## Native V3 Backend API

**Status:** Experimental. Functional bugs should be reported.

```rust
use sqlitegraph::backend::native::v3::V3Backend;

// Create new database
let backend = V3Backend::create("data.graph")?;

// Open existing database
let backend = V3Backend::open("data.graph")?;

// Create with WAL enabled
let backend = V3Backend::create_with_wal("data.graph", true)?;
```

### Weighted Edge Retrieval And Bulk Ingestion

Native V3 exposes weighted adjacency helpers used by graph-walk workloads:

```rust
use sqlitegraph::backend::{BackendDirection, NeighborQuery};
use sqlitegraph::snapshot::SnapshotId;

backend.batch_insert_edges_with_weights(vec![
    (1, 2, 0.75, None),
    (1, 3, 0.50, None),
])?;

let neighbors = backend.neighbors_weighted_shared(
    SnapshotId::current(),
    1,
    NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    },
)?;

for &(dst, weight) in neighbors.iter() {
    println!("{dst}: {weight}");
}

backend.warm_neighbors_for_sources(
    SnapshotId::current(),
    &[1, 2, 3],
    NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    },
)?;
```

Notes:
- `batch_insert_edges_with_weights(...)` inserts weighted edges through the native V3 edge store in one batch.
- `neighbors_weighted_shared(...)` returns `Arc<[(i64, f32)]>`, avoids per-call allocation on hot read paths, and returns unfiltered neighbors in descending weight order.
- `warm_neighbors_for_sources(...)` preloads the weighted neighbor cache for known source sets so the first query pass can avoid cold edge-page reads.
- The native V3 edge store now packs multiple small `(src, dir)` clusters into shared edge pages while keeping the public API unchanged.

### Lazy Initialization Inspection

```rust
// Check if features have been initialized
assert!(!backend.is_kv_initialized());      // false until first kv_get/set
assert!(!backend.is_pubsub_initialized());  // false until first subscribe

backend.kv_set_v3(b"key".to_vec(), KvValue::Integer(42), None);
assert!(backend.is_kv_initialized());       // true now
```

### V3-Native KV API

V3 provides methods that work directly with V3 `KvValue` (no feature gates needed):

```rust
use sqlitegraph::backend::native::v3::KvValue;
use sqlitegraph::snapshot::SnapshotId;

// Get (returns Option<KvValue>)
// SnapshotId::current() returns SnapshotId(0) - works for both SQLite and V3
let value = backend.kv_get_v3(SnapshotId::current(), b"my_key");

// For native-v3 specific use cases needing unique snapshot IDs:
let unique_snapshot = SnapshotId::new_incrementing();
let value = backend.kv_get_v3(unique_snapshot, b"my_key");

// Set
backend.kv_set_v3(b"my_key".to_vec(), KvValue::String("value".into()), None);

// Delete
backend.kv_delete_v3(b"my_key");
```

**Snapshot Behavior:**
- `SnapshotId::current()` returns `SnapshotId(0)` - works with all backends
- `SnapshotId::new_incrementing()` returns unique incrementing IDs (native-v3 only)
- SQLite backend only supports `SnapshotId(0)` (no historical snapshots)
- Native-v3 backend supports both snapshot types

### Node Caching

V3Backend includes an LRU cache for node record lookups:

```rust
use sqlitegraph::backend::native::v3::NodeCache;

// The cache is automatically created with the backend
// Default capacity: 1000 nodes

// Manual cache control (advanced usage)
let cache = NodeCache::new(1000);
cache.insert(node_id, node_record);
if let Some(record) = cache.get(node_id) {
    // Cache hit - use record
}

// Invalidate entries on mutations
cache.invalidate(node_id);

// Clear entire cache
cache.clear();

// Check cache statistics
let cached_count = cache.len();
let is_empty = cache.is_empty();
```

**Performance Impact:**
- Warm-cache lookups can improve significantly
- Hit rate depends on graph shape and workload locality
- Thread-safe: Mutex-protected for concurrent access
- Use [docs/BENCHMARKING.md](docs/BENCHMARKING.md) to measure on your hardware

### Parallel BFS

V3Backend supports parallel breadth-first search using Rayon:

```rust
use sqlitegraph::backend::native::v3::algorithm::parallel_bfs;
use sqlitegraph::backend::native::v3::algorithm::BfsConfig;

// Standard parallel BFS
let result = parallel_bfs(&backend, start_node, None)?;

// With custom configuration
let config = BfsConfig {
    max_depth: Some(100),
    sequential_threshold: Some(1000), // Use sequential BFS for < 1000 nodes
};
let result = parallel_bfs(&backend, start_node, Some(config))?;

// Result contains visited nodes and levels
println!("Visited {} nodes", result.visited_count);
println!("Max depth: {}", result.max_depth);
```

**Performance Impact:**
- **Thread-safe:** Chunked processing with no shared state during the parallel phase
- **Sequential fallback:** Automatically uses sequential BFS for graphs <1K nodes
- **Status:** Stable for small graphs, experimental for larger graphs
- Measure on your workload before claiming a speedup; benchmark methodology is in [docs/BENCHMARKING.md](docs/BENCHMARKING.md)

### Adaptive Page Sizing

V3Backend automatically adapts page size based on storage media:

```rust
// Automatic - no API needed
// SSD detection → 4KB pages (better random read performance)
// HDD detection → 16KB pages (reduce seek overhead)
// Fallback → 8KB pages if detection fails

// Manual override (advanced usage)
use sqlitegraph::backend::native::v3::storage::adaptive_page;

let media_type = adaptive_page::detect_media_type(db_path)?;
match media_type {
    adaptive_page::MediaDetectorResult::SSD => println!("Using 4KB pages"),
    adaptive_page::MediaDetectorResult::HDD => println!("Using 16KB pages"),
    adaptive_page::MediaDetectorResult::Unknown => println!("Using 8KB pages"),
}
```

**Performance Impact:**
- SSD detection → 4KB pages (matches SSD block size)
- HDD detection → 16KB pages (reduces seek overhead)
- Fallback → 8KB pages if detection fails
- Measure on the target medium with the workflow in [docs/BENCHMARKING.md](docs/BENCHMARKING.md)

### HNSW Vector Storage

```rust
// Create storage backed by V3 KV
let storage = backend.create_hnsw_storage("embeddings").unwrap();
```

### Pub/Sub

```rust
let (sub_id, rx) = backend.subscribe(SubscriptionFilter::all())?;
```

---

## Graph Algorithms API

All algorithms work with any backend via `&dyn GraphBackend`:

```rust
use sqlitegraph::algo;

// With V3 backend
let v3 = V3Backend::create("data.graph")?;
let scores = algo::pagerank(&v3, 0.85, 50)?;

// With SQLite backend
let sqlite = SqliteGraphBackend::in_memory()?;
let scores = algo::pagerank(&sqlite, 0.85, 50)?;
```

### Algorithm Categories

| Category | Count | Examples |
|----------|-------|----------|
| Core Graph Theory | 5 | SCC, WCC, Topological Sort |
| CFG Analysis | 5 | Dominators, Control Dependence |
| Path Analysis | 4 | Shortest Path, Cycle Basis |
| Security | 4 | Taint Analysis, Sink Discovery |
| Program Analysis | 3 | Slicing, SCC Collapse |
| ... | ... | ... |

**Total: 35+ algorithms**

See [GRAPH_ALGORITHMS_GUIDE.md](docs/GRAPH_ALGORITHMS_GUIDE.md) for complete list.

---

## TypedDiGraph API

A lightweight in-memory directed graph with generic node (`N`) and edge (`E`)
weights. Independent of `GraphBackend` — no SQLite, no disk I/O. Designed for
build DAGs, dependency graphs, and transient analysis passes.

```rust
use sqlitegraph::typed_digraph::{TypedDiGraph, NodeIndex, EdgeIndex, Direction};
use sqlitegraph::typed_digraph::algo::{is_cyclic_directed, tarjan_scc, toposort, Dfs};
```

### Construction & Mutation

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `TypedDiGraph<N, E>::new()` | Empty graph |
| `add_node` | `(&mut self, N) -> NodeIndex` | Insert node, return index |
| `add_edge` | `(&mut self, NodeIndex, NodeIndex, E) -> EdgeIndex` | Insert directed edge |
| `remove_node` | `(&mut self, NodeIndex) -> Option<N>` | Remove node and its edges |
| `remove_edge` | `(&mut self, EdgeIndex) -> Option<E>` | Remove single edge |
| `clear` | `(&mut self)` | Remove all nodes and edges |

### Queries

| Method | Signature | Description |
|--------|-----------|-------------|
| `node_count` | `(&self) -> usize` | Number of valid nodes |
| `edge_count` | `(&self) -> usize` | Number of valid edges |
| `raw_node_count` | `(&self) -> usize` | Slots including removed |
| `contains_node` | `(&self, NodeIndex) -> bool` | Node validity check |
| `node_weight` | `(&self, NodeIndex) -> Option<&N>` | Borrow node weight |
| `node_weight_mut` | `(&mut self, NodeIndex) -> Option<&mut N>` | Mutably borrow |
| `edge_weight` | `(&self, EdgeIndex) -> Option<&E>` | Borrow edge weight |
| `edge_endpoints` | `(&self, EdgeIndex) -> Option<(NodeIndex, NodeIndex)>` | Source and target |
| `neighbors_directed` | `(&self, NodeIndex, Direction) -> impl Iterator` | Adjacent nodes |
| `node_indices` | `(&self) -> impl Iterator<Item = NodeIndex>` | All valid node IDs |
| `edge_indices` | `(&self) -> impl Iterator<Item = EdgeIndex>` | All valid edge IDs |
| `degree` | `(&self, NodeIndex) -> usize` | Undirected degree |
| `degrees` | `(&self, NodeIndex) -> (usize, usize)` | `(in_degree, out_degree)` |

### Algorithms

| Function | Signature | Description |
|----------|-----------|-------------|
| `is_cyclic_directed` | `(&TypedDiGraph<N,E>) -> bool` | Cycle detection |
| `tarjan_scc` | `(&TypedDiGraph<N,E>) -> Vec<Vec<NodeIndex>>` | Strongly connected components |
| `toposort` | `(&TypedDiGraph<N,E>) -> Result<Vec<NodeIndex>, CycleError>` | Topological sort |
| `Dfs::new` | `(&TypedDiGraph<N,E>, NodeIndex) -> Dfs` | Depth-first visitor (implements `Iterator`) |

---

## Cypher-Inspired Query API

The query parser and executor live in `sqlitegraph::cypher` and currently
require the SQLite backend for execution:

```rust
use sqlitegraph::backend::sqlite::SqliteGraphBackend;

let backend = SqliteGraphBackend::in_memory()?;
let query = sqlitegraph::cypher::parse(
    "MATCH (a:User)-[:KNOWS]->(b:User) RETURN a.name, b.name",
)?;
let result = sqlitegraph::cypher::execute(&backend, &query)?;
println!("{}", result);
```

Supported statements include `MATCH`, `CREATE`, `SET`, `DELETE`, and
`CALL db.index.vector.queryNodes(...)`. See
[docs/QUERY_LANGUAGE.md](docs/QUERY_LANGUAGE.md) for the full grammar and the
CLI/Python examples.

Python exposes the same executor through `Graph.query(query_str)` and returns a
dict with `results` and `count`.

---

## HNSW Vector Search API

### Creating an Index

**SQLite Backend:**
```rust
use sqlitegraph::SqliteGraph;
use sqlitegraph::hnsw::{DistanceMetric, HnswConfigBuilder};

let graph = SqliteGraph::open("vectors.db")?;
let config = HnswConfigBuilder::new()
    .dimension(768)
    .distance_metric(DistanceMetric::Cosine)
    .build()?;

let _guard = graph.hnsw_index_persistent("my_index", config)?;
```

**Direct construction with an explicit storage backend:**
```rust
use rusqlite::Connection;
use sqlitegraph::hnsw::{DistanceMetric, HnswConfigBuilder, HnswIndex};

let config = HnswConfigBuilder::new()
    .dimension(768)
    .distance_metric(DistanceMetric::Cosine)
    .build()?;

let conn = Connection::open("vectors.db")?;
let mut index = HnswIndex::with_persistent_storage("my_index", config, conn)?;
```

### Common Operations

```rust
use serde_json::json;

let vector = vec![0.1, 0.2, 0.3 /* ... 768 dims */];
let id = index.insert_vector(&vector, Some(json!({"doc_id": "123"})))?;

let batch = vec![
    (vec![0.1, 0.2, 0.3], Some(json!({"doc_id": "123"}))),
    (vec![0.3, 0.2, 0.1], Some(json!({"doc_id": "124"}))),
];
let ids = index.batch_insert_vectors(&batch)?;

let results = index.search(&query_vector, 10)?; // top 10
for (id, distance) in results {
    println!("ID: {}, Distance: {}", id, distance);
}

let stats = index.statistics()?;
println!(
    "vectors={} inserts={} searches={} cache_hits={} cache_misses={}",
    stats.vector_count,
    stats.insert_count,
    stats.search_count,
    stats.vector_cache_hits,
    stats.vector_cache_misses,
);
```

### Accessing a Persistent Index Through `SqliteGraph`

```rust
let stats = graph.get_hnsw_index_ref("my_index", |idx| idx.statistics())??;
println!("indexed vectors: {}", stats.vector_count);

let index_names = graph.list_hnsw_indexes()?;
println!("available indexes: {:?}", index_names);
```

---

## KV Store API

### Availability by Backend

| Backend | Status | Notes |
|---------|--------|-------|
| **V3** | ✅ Full | Lazy initialization |
| **SQLite** | ✅ Full | SQL table |

### V3 Native Methods (Recommended)

```rust
use sqlitegraph::snapshot::SnapshotId;

// Get (SnapshotId::current() returns 0 - works with all backends)
match backend.kv_get_v3(SnapshotId::current(), b"counter") {
    Some(KvValue::Integer(n)) => println!("Count: {}", n),
    _ => println!("Not found"),
}

// Set with TTL (60 seconds)
backend.kv_set_v3(
    b"session".to_vec(),
    KvValue::Json(json!({"user": "alice"})),
    Some(60),
);

// Delete
backend.kv_delete_v3(b"session");
```

### Generic Trait Methods

```rust
use sqlitegraph::backend::{GraphBackend, KvValue};

fn set_config(backend: &dyn GraphBackend) -> Result<(), SqliteGraphError> {
    backend.kv_set(
        b"config".to_vec(),
        KvValue::Json(json!({"version": "1.0"})),
        None,
    )
}
```

---

## Pub/Sub API

### Availability by Backend

| Backend | Status | Notes |
|---------|--------|-------|
| **V3** | ✅ Full | Lazy initialization |
| **SQLite** | ✅ Full | In-memory publisher |

### Basic Usage

```rust
use sqlitegraph::backend::{SubscriptionFilter, PubSubEvent};

// Subscribe
let filter = SubscriptionFilter {
    node_changes: true,
    edge_changes: false,
    kv_changes: false,
    snapshot_commits: false,
};
let (sub_id, rx) = backend.subscribe(filter)?;

// Receive events
std::thread::spawn(move || {
    while let Ok(event) = rx.recv() {
        match event {
            PubSubEvent::NodeChanged { node_id, snapshot_id } => {
                println!("Node {} changed at snapshot {}", node_id, snapshot_id);
            }
            PubSubEvent::EdgeChanged { edge_id, snapshot_id } => {
                println!("Edge {} changed", edge_id);
            }
            _ => {}
        }
    }
});

// Operations emit events
backend.insert_node(NodeSpec { ... })?; // Emits NodeChanged
backend.insert_edge(EdgeSpec { ... })?;  // Emits EdgeChanged

// Cleanup
backend.unsubscribe(sub_id)?;
```

### Event Types

```rust
pub enum PubSubEvent {
    NodeChanged { node_id: i64, snapshot_id: u64 },
    EdgeChanged { edge_id: i64, snapshot_id: u64 },
    KVChanged { key_hash: u64, snapshot_id: u64 },
    SnapshotCommitted { snapshot_id: u64 },
}
```

---

## Temporal Topology API

The `sqlitegraph::temporal` module analyses how graph topology evolves across the
MVCC version chain. It sweeps over retained [`VersionedSnapshot`]s and produces
three complementary topological views: an exact H₀ connected-component barcode,
a β₁ (cycle-rank) trajectory, and a circular-dependency lifecycle barcode.

> **Availability:** These functions operate purely on in-memory `SnapshotState`
> adjacency — there is no SQLite I/O during analysis. They require that versions
> have been retained via [`SqliteGraph::checkpoint`]; the chain is empty by
> default, so with no checkpoints the analysis is a no-op.

### Version Chain (`SqliteGraph`)

Temporal tracking is opt-in. By default nothing is retained; call `checkpoint()`
to capture numbered versions into the bounded history chain (default capacity
64, configurable via `SnapshotManager::with_max_history`). When the chain is
full, the oldest version is evicted (FIFO).

```rust
use sqlitegraph::SqliteGraph;

let graph = SqliteGraph::open("data.db")?;

// Capture the current live state as a numbered version.
let v1 = graph.checkpoint(); // warm the adjacency cache first for accuracy
// ... mutate the graph ...
let v2 = graph.checkpoint();

// Retrieve a historical snapshot by version number.
let snapshot = graph.snapshot_as_of(v1)?;

// All retained versions (oldest first) — feed this slice to the temporal
// analysis functions.
let versions = graph.snapshot_versions();

// Chain metadata
println!(
    "retained: {} (oldest {:?}, newest {:?})",
    graph.snapshot_version_count(),
    graph.snapshot_oldest_version(),
    graph.snapshot_newest_version(),
);
```

### Free Functions

| Function | Signature | Description |
|----------|-----------|-------------|
| `temporal_persistence_sweep` | `(versions: &[VersionedSnapshot]) -> Vec<TemporalPersistencePoint>` | One measurement point per version: β₀ (component count), largest SCC size, β₁ cycle-rank, and non-trivial SCC counts |
| `scc_lineage_barcode` | `(versions: &[VersionedSnapshot]) -> Vec<LineageBarcode>` | Exact H₀ component barcode via stable membership-identity (Jaccard) matching. Replaces the deprecated `compute_temporal_barcode` |
| `cycle_scc_barcode` | `(versions: &[VersionedSnapshot]) -> Vec<LineageBarcode>` | Circular-dependency lifecycle: tracks non-trivial SCCs (size ≥ 2) that each contain at least one directed cycle |
| `cycle_rank_snapshot` | `(state: &SnapshotState) -> usize` | Cyclomatic number β₁ = E − V + W (independent undirected cycles) for a single snapshot |
| `compute_temporal_barcode` *(deprecated)* | `(points: &[TemporalPersistencePoint]) -> Vec<TemporalBarcode>` | LIFO count-delta approximation; unreliable when components merge/split simultaneously. Use `scc_lineage_barcode` instead |

```rust
use sqlitegraph::temporal;

let versions = graph.snapshot_versions();

// 1. Scalar trajectory: β₀, β₁, and SCC sizes at each version.
let sweep = temporal::temporal_persistence_sweep(&versions);

// 2. Exact H₀ barcode — when connected components are born and die.
let h0 = temporal::scc_lineage_barcode(&versions);

// 3. Circular-dependency barcode — when cycles form and dissolve.
let cycles = temporal::cycle_scc_barcode(&versions);

// β₁ for the most recent retained snapshot alone.
let latest = graph.snapshot_as_of(graph.snapshot_newest_version().unwrap()).unwrap();
let beta1 = temporal::cycle_rank_snapshot(&latest.state);
```

### Structs

#### `TemporalPersistencePoint`

One measurement point in the temporal persistence sweep.

| Field | Type | Description |
|-------|------|-------------|
| `version` | `u64` | Version number at this measurement |
| `active` | `usize` | Total nodes in this snapshot (V) |
| `n_components` | `usize` | Number of strongly connected components (β₀) |
| `largest_size` | `usize` | Size of the largest SCC |
| `fraction_largest` | `f32` | `largest_size / active` (0.0 when `active == 0`) |
| `cycle_rank` | `usize` | Cyclomatic number β₁ = E − V + W |
| `n_nontrivial_sccs` | `usize` | Number of non-trivial SCCs (size ≥ 2) — cycle-containing |
| `largest_nontrivial_size` | `usize` | Size of the largest non-trivial SCC, or 0 if none |

#### `LineageBarcode`

One bar in the exact lineage-tracked barcode (produced by `scc_lineage_barcode`
and `cycle_scc_barcode`). Birth/death are determined by real membership lineage
(Jaccard overlap), not by component-count deltas.

| Field | Type | Description |
|-------|------|-------------|
| `birth_version` | `u64` | Version where this lineage first appeared |
| `death_version` | `Option<u64>` | Last version seen, or `None` if it survived to the end of the sweep |
| `birth_size` | `usize` | Member-set size when this lineage was born |
| `peak_size` | `usize` | Peak member-set size across the lineage's life |
| `final_size` | `usize` | Member-set size at the last observed version |
| `versions_seen` | `usize` | Number of versions this lineage was observed (including birth) |

#### `VersionedSnapshot`

A versioned point-in-time snapshot (defined in `sqlitegraph::mvcc`, re-exported
at the crate root).

| Field | Type | Description |
|-------|------|-------------|
| `version` | `u64` | Monotonic version number assigned at checkpoint time (starts at 1) |
| `created_at` | `SystemTime` | Wall-clock timestamp when this version was captured |
| `state` | `Arc<SnapshotState>` | The immutable adjacency data for this version |

#### `TemporalBarcode` *(deprecated)*

One bar in the approximate LIFO barcode produced by the deprecated
`compute_temporal_barcode`. Prefer `LineageBarcode` / `scc_lineage_barcode`.

| Field | Type | Description |
|-------|------|-------------|
| `birth_version` | `u64` | Version where this component first appeared |
| `death_version` | `Option<u64>` | Last version seen, or `None` if it survived |
| `peak_size` | `usize` | Largest size this component reached across its lifetime |

---

## MVCC Snapshot Tracking API

**Status:** Stable (v3.4), available in both SQLite and Native V3 backends

Named snapshots with metadata tracking and optimized time-travel queries using pre-aggregated statistics.

### Core Types

```rust
use sqlitegraph::graph::SnapshotMetadata;

pub struct SnapshotMetadata {
    pub snapshot_id: String,
    pub timestamp: i64,
    pub description: Option<String>,
}
```

### Snapshot Management

```rust
use sqlitegraph::SqliteGraph;

let graph = SqliteGraph::open_in_memory()?;

// Create named snapshot with optional description
let timestamp = graph.create_snapshot("snapshot_001")?;
let timestamp = graph.create_snapshot_with_description("snapshot_002", Some("Weekly backup"))?;

// List all snapshots (sorted by created_at DESC)
let snapshots = graph.list_snapshots()?;
for snapshot in snapshots {
    println!("Snapshot {}: {} ({:?})",
        snapshot.snapshot_id,
        snapshot.timestamp,
        snapshot.description
    );
}

// Delete snapshot (cascades to snapshot_stats)
graph.delete_snapshot("snapshot_001")?;
```

### Batch Insert with Snapshot Tagging

```rust
use sqlitegraph::graph::{GraphEntity, GraphEdge};

let snapshot_id = "batch_001";
graph.create_snapshot(snapshot_id)?;

// Insert entities with snapshot tagging
let entities = vec![
    GraphEntity {
        id: 0,
        kind: "User".to_string(),
        name: "Alice".to_string(),
        file_path: None,
        data: serde_json::json!({"age": 30}),
    },
    GraphEntity {
        id: 0,
        kind: "User".to_string(),
        name: "Bob".to_string(),
        file_path: None,
        data: serde_json::json!({"age": 31}),
    },
];
graph.batch_insert_entities_with_snapshot(&entities, snapshot_id)?;

// Insert edges with snapshot tagging
let edges = vec![
    GraphEdge {
        id: 0,
        from_id: alice_id,
        to_id: bob_id,
        edge_type: "KNOWS".to_string(),
        data: serde_json::json!({"since": 2020}),
    },
];
graph.batch_insert_edges_with_snapshot(&edges, snapshot_id)?;
```

### Time-Travel Queries

```rust
use sqlitegraph::graph::GraphStats;

// Query graph state as of timestamp (uses pre-aggregated stats - O(1) not O(N))
let stats: GraphStats = graph.query_as_of(timestamp)?;

println!("As of {}: {} entities, {} edges",
    timestamp,
    stats.total_entities,
    stats.total_edges
);

// Pre-aggregated stats table (snapshot_stats) enables:
// - O(1) time-travel queries (no COUNT(*) scans)
// - Multi-dimensional queries (snapshot_id, created_at)
// - Partition-ready schema for horizontal scaling
```

### Performance Characteristics

**Small datasets (<10K entities):** Negligible difference (SQL optimizer handles both)

**Medium datasets (10K-1M entities):**
- Pre-aggregated stats: ~10-100x faster for time-travel
- `query_as_of()` uses single indexed lookup vs full table scan

**Large datasets (>1M entities):**
- Eliminates COUNT(*) table scans entirely
- Enables horizontal partitioning by snapshot ranges
- Composite `(snapshot_id, created_at)` indexes support multi-dimensional queries

---

## Error Types

```rust
pub enum SqliteGraphError {
    ConnectionError(String),
    SchemaError(String),
    QueryError(String),
    NotFound(String),
    InvalidInput(String),
    TransactionError(String),
    ValidationError(String),
    Unsupported(String),  // Feature not supported by backend
    NativeError(NativeBackendError),
}
```

**Common Error Cases:**
- `Unsupported` - Backend doesn't support feature (e.g., KV on older SQLite)
- `NotFound` - Entity/edge doesn't exist
- `InvalidInput` - Invalid parameters (e.g., wrong vector dimension)

---

## Full Documentation

- **API docs:** https://docs.rs/sqlitegraph
- **Architecture:** [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)
- **Algorithms:** [docs/GRAPH_ALGORITHMS_GUIDE.md](docs/GRAPH_ALGORITHMS_GUIDE.md)
- **Changelog:** [CHANGELOG.md](CHANGELOG.md)

---

**Note:** This API reference is maintained for the current 3.4.6 line. We document deprecations and limitations honestly; check backend-specific sections for feature availability.
