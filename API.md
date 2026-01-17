# SQLiteGraph API Reference

## Official Documentation

The complete API documentation is available at **[docs.rs/sqlitegraph](https://docs.rs/sqlitegraph)**.

This document provides a quick overview of the main API surface.

## Table of Contents

- [SQLite Backend API](#sqlite-backend-api)
- [Native V2 Backend API](#native-v2-backend-api)
- [Graph Algorithms API](#graph-algorithms-api)
- [HNSW Vector Search API](#hnsw-vector-search-api)
- [Introspection API](#introspection-api)
- [Progress Tracking API](#progress-tracking-api)
- [Error Types](#error-types)

---

## SQLite Backend API

### Core Types

```rust
use sqlitegraph::{SqliteGraph, GraphEntity, GraphEdge};

// Open database
let graph = SqliteGraph::open_in_memory()?;
let graph = SqliteGraph::open("path/to/db")?;

// Entity
pub struct GraphEntity {
    pub id: u64,
    pub kind: String,
    pub name: String,
    pub file_path: Option<String>,
    pub data: serde_json::Value,
}

// Edge
pub struct GraphEdge {
    pub id: u64,
    pub from_id: u64,
    pub to_id: u64,
    pub edge_type: String,
    pub data: serde_json::Value,
}
```

### Main Methods

| Method | Description |
|--------|-------------|
| `open_in_memory()` | Create in-memory database |
| `open(path: &str)` | Open file-based database |
| `insert_entity(&entity)` | Insert new entity, returns ID |
| `get_entity(id)` | Retrieve entity by ID |
| `update_entity(&entity)` | Update existing entity |
| `delete_entity(id)` | Delete entity |
| `insert_edge(&edge)` | Insert new edge |
| `get_edge(id)` | Retrieve edge by ID |
| `neighbors(id, direction)` | Get neighbor entities |
| `has_path(from, to)` | Check if path exists |
| `snapshot()` | Create MVCC snapshot |

---

## Native V2 Backend API

### Core Types

```rust
use sqlitegraph::{GraphConfig, open_graph, NodeSpec, EdgeSpec};

// Configuration
let config = GraphConfig::native();
let config = GraphConfig::native()
    .with_buffer_size(128 * 1024 * 1024)
    .with_parallel_recovery(8);

// Open graph
let graph = open_graph("path/to/graph.db", &config)?;

// Node
pub struct NodeSpec {
    pub kind: String,
    pub name: String,
    pub file_path: Option<String>,
    pub data: serde_json::Value,
}

// Edge
pub struct EdgeSpec {
    pub from: u64,
    pub to: u64,
    pub edge_type: String,
    pub data: serde_json::Value,
}
```

### Main Methods

| Method | Description |
|--------|-------------|
| `open_graph(path, config)` | Open Native V2 graph |
| `insert_node(spec)` | Insert new node |
| `get_node(id)` | Retrieve node by ID |
| `update_node(&spec)` | Update existing node |
| `delete_node(id)` | Delete node |
| `insert_edge(spec)` | Insert new edge |
| `neighbors(query)` | Get neighbors with query options |
| `snapshot()` | Create MVCC snapshot |

---

## Graph Algorithms API

### Available Algorithms

```rust
use sqlitegraph::algo;

// PageRank - importance ranking
let scores: HashMap<u64, f64> = algo::pagerank(&graph, 0.85, 50)?;
let scores = algo::pagerank_with_progress(&graph, 0.85, 50, progress)?;

// Betweenness Centrality - node importance
let centrality: HashMap<u64, f64> = algo::betweenness_centrality(&graph)?;

// Label Propagation - fast community detection
let communities: HashMap<u64, u64> = algo::label_propagation(&graph)?;

// Louvain - modularity-based clustering
let partition: HashMap<u64, u64> = algo::louvain_communities(&graph, 0.01)?;
```

### Algorithm Characteristics

| Algorithm | Function | Complexity | Returns |
|-----------|----------|------------|---------|
| **PageRank** | `pagerank(graph, damping, iterations)` | O(|E| × iter) | `HashMap<u64, f64>` |
| **Betweenness** | `betweenness_centrality(graph)` | O(|V||E|) | `HashMap<u64, f64>` |
| **Label Propagation** | `label_propagation(graph)` | O(|E|) | `HashMap<u64, u64>` |
| **Louvain** | `louvain_communities(graph, tolerance)` | O(|E| log |V|) | `HashMap<u64, u64>` |

---

## HNSW Vector Search API

### Core Types

```rust
use sqlitegraph::hnsw::{HnswConfig, HnswIndex, DistanceMetric};

// Configuration
let config = HnswConfig::builder()
    .dimension(1536)
    .m_connections(16)
    .ef_construction(200)
    .ef_search(50)
    .distance_metric(DistanceMetric::Cosine)
    .build()?;

// Create index
let hnsw = HnswIndex::new(config)?;
```

### Main Methods

| Method | Description |
|--------|-------------|
| `new(config)` | Create new HNSW index |
| `insert_vector(&vec, metadata)` | Insert vector with optional metadata |
| `search(&query, k)` | Search k nearest neighbors |
| `get_vector(id)` | Retrieve vector by ID |
| `len()` | Get number of vectors |
| `is_empty()` | Check if index is empty |

### Distance Metrics

| Metric | Use Case |
|--------|----------|
| `Cosine` | Text embeddings |
| `Euclidean` | General similarity |
| `DotProduct` | Normalized vectors |
| `Manhattan` | Sparse vectors |

---

## Introspection API

### GraphIntrospection

```rust
use sqlitegraph::introspection::GraphIntrospection;

let intro = GraphIntrospection::new(&graph)?;

// Get statistics
let nodes: usize = intro.node_count()?;
let edges: (usize, usize) = intro.edge_count_estimate()?;
let info: serde_json::Value = intro.backend_info()?;
let json: String = intro.to_json()?;
```

### Methods

| Method | Returns | Description |
|--------|---------|-------------|
| `new(graph)` | `GraphIntrospection` | Create introspection instance |
| `node_count()` | `usize` | Exact node count |
| `edge_count_estimate()` | `(usize, usize)` | (min, max) edge estimate |
| `backend_info()` | `serde_json::Value` | Backend-specific info |
| `to_json()` | `String` | JSON serialization |

---

## Progress Tracking API

### ProgressCallback Trait

```rust
use sqlitegraph::progress::{ProgressCallback, ProgressState, ConsoleProgress, NoProgress};

// Use with algorithms
let scores = algo::pagerank_with_progress(&graph, 0.85, 50, ConsoleProgress::new())?;

// Custom implementation
struct MyProgress;
impl ProgressCallback for MyProgress {
    fn on_progress(&self, state: &ProgressState) {
        println!("{}: {}%", state.message, state.percent);
    }
}
```

### Implementations

| Implementation | Behavior |
|----------------|----------|
| `NoProgress` | No-op, zero overhead |
| `ConsoleProgress` | Progress bars to terminal |

---

## Error Types

### SqliteGraphError

```rust
use sqlitegraph::SqliteGraphError;

match result {
    Ok(value) => /* ... */,
    Err(SqliteGraphError::ValidationError(msg)) => { /* ... */ }
    Err(SqliteGraphError::ConnectionError(msg)) => { /* ... */ }
    Err(SqliteGraphError::TransactionError(msg)) => { /* ... */ }
    Err(SqliteGraphError::NotFoundError(msg)) => { /* ... */ }
    Err(e) => { /* ... */ }
}
```

### HnswConfigError

```rust
use sqlitegraph::hnsw::HnswConfigError;

match HnswConfig::builder().build() {
    Ok(config) => /* ... */,
    Err(HnswConfigError::InvalidDimension) => { /* ... */ }
    Err(HnswConfigError::InvalidMParameter) => { /* ... */ }
    Err(e) => { /* ... */ }
}
```

---

## Feature Flags

```toml
[dependencies]
# SQLite backend only
sqlitegraph = "1.0"

# Native V2 backend
sqlitegraph = { version = "1.0", features = ["native-v2"] }

# V2 I/O tracing (development)
sqlitegraph = { version = "1.0", features = ["trace_v2_io"] }
```

---

## Thread Safety

**Important**: `SqliteGraph` is **NOT thread-safe** for concurrent writes.

For concurrent access:
- Use MVCC snapshots for concurrent reads
- Each thread should have its own graph connection
- Use external synchronization for writes

```rust
// Correct: Snapshot for concurrent reads
let snapshot = graph.snapshot()?;
// snapshot can be cloned and sent to threads
```

---

## Full Documentation

For complete API documentation with examples, see:
- **[docs.rs/sqlitegraph](https://docs.rs/sqlitegraph)** - Official rustdoc
- **[manual.md](manual.md)** - Operator manual
- **[CHANGELOG.md](CHANGELOG.md)** - Version history
