# SQLiteGraph Manual

Comprehensive usage guide for SQLiteGraph with dual backend architecture (SQLite and Native V2).

---

## 1. Quick Start

### Installation

```toml
[dependencies]
sqlitegraph = "1.0"

# For Native V2 backend
sqlitegraph = { version = "1.0", features = ["native-v2"] }
```

### Basic Usage

```rust
use sqlitegraph::{SqliteGraph, GraphEntity};

let graph = SqliteGraph::open_in_memory()?;

let entity = GraphEntity {
    id: 0,
    kind: "User".to_string(),
    name: "Alice".to_string(),
    file_path: None,
    data: serde_json::json!({"age": 30}),
};

let id = graph.insert_entity(&entity)?;
println!("Created entity: {}", id);
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

### Native V2 Backend

**Use**: High-performance scenarios, large graphs, traversal-heavy workloads

```rust
use sqlitegraph::{GraphConfig, open_graph, NodeSpec, EdgeSpec};

let config = GraphConfig::native();
let graph = open_graph("graph.db", &config)?;
// Clustered adjacency for locality
```

### Backend Comparison

| Characteristic | SQLite Backend | Native V2 Backend |
|----------------|----------------|-------------------|
| **Performance** | Standard SQLite | 10x faster for traversals |
| **Transactions** | Full ACID | Atomic commits, WAL |
| **Memory Usage** | SQLite overhead | Configurable buffers |
| **Use Cases** | General purpose | High performance, large graphs |

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

### Node Management (Native V2 Backend)

```rust
use sqlitegraph::{GraphConfig, open_graph, NodeSpec, EdgeSpec};

let config = GraphConfig::native();
let graph = open_graph("graph.db", &config)?;

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

### PageRank

```rust
use sqlitegraph::algo;

// Basic PageRank
let scores = algo::pagerank(&graph, 0.85, 50)?;

// With progress tracking
use sqlitegraph::progress::ConsoleProgress;
let scores = algo::pagerank_with_progress(&graph, 0.85, 50, ConsoleProgress::new())?;
```

### Betweenness Centrality

```rust
// Node importance via shortest paths
let centrality = algo::betweenness_centrality(&graph)?;
```

### Community Detection

```rust
// Label Propagation (fast)
let communities = algo::label_propagation(&graph)?;

// Louvain (higher quality)
let partition = algo::louvain_communities(&graph, 0.01)?;
```

### Algorithm Characteristics

| Algorithm | Complexity | Best For |
|-----------|------------|----------|
| **PageRank** | O(|E| × iterations) | Importance ranking |
| **Betweenness** | O(|V||E|) | Critical nodes |
| **Label Propagation** | O(|E|) | Fast communities |
| **Louvain** | O(|E| log |V|) | Quality clustering |

---

## 5. Testing

### Running Tests

```bash
# All tests
cargo test --workspace

# With Native V2 backend
cargo test --workspace --features native-v2

# Specific test patterns
cargo test '*pagerank*'
cargo test '*wal*'
```

### Test Coverage

**v1.0 Test Results:**
- 42 WAL tests passing (recovery, corruption, checkpoints)
- 53 concurrent MVCC tests passing (snapshots, stress testing)
- 27 algorithm tests passing (PageRank, Betweenness, Louvain, Label Propagation)
- 134 HNSW tests passing
- 65 MVCC lifecycle tests passing

**Total**: 300+ tests passing

---

## 6. Performance

### Native V2 Performance

Based on actual benchmarks (Phase 3, 7):

| Operation | Performance |
|-----------|-------------|
| **Node Insert** | ~50K ops/sec |
| **Edge Insert** | ~100K ops/sec (bulk) |
| **Neighbor Query** | <1ms (clustered) |
| **Vector Search** | <1ms with 95%+ accuracy |

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
- 2-3x speedup for 500+ transactions
- 1.5-2x speedup for 50-100 transactions

---

## 7. Error Handling

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
# Enable V2 I/O tracing
sqlitegraph = { version = "1.0", features = ["trace_v2_io"] }
```

```bash
# Run with debug output
RUST_LOG=debug cargo run --features trace_v2_io
```

---

## 8. Vector Search (HNSW)

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
| **Dot Product** | Normalized vectors | Fastest |
| **Manhattan** | Sparse vectors | Slow |

### CLI Commands

```bash
# Create index
sqlitegraph --backend sqlite --db mygraph.db hnsw-create \
    --dimension 768 --distance-metric cosine

# Insert vectors
sqlitegraph --backend sqlite --db mygraph.db hnsw-insert \
    --index-name vectors --input vectors.json

# Search
sqlitegraph --backend sqlite --db mygraph.db hnsw-search \
    --index-name vectors --input query.json --k 10

# List indexes
sqlitegraph --backend sqlite --db mygraph.db hnsw-list
```

---

## 9. Developer Tools (Phase 9)

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

### CLI Debug Commands

```bash
# Statistics
sqlitegraph --backend sqlite --db mygraph.db debug-stats

# Dump graph
sqlitegraph --backend sqlite --db mygraph.db debug-dump --output graph.json

# Trace operations
sqlitegraph --backend sqlite --db mygraph.db debug-trace
```

---

## 10. Safety & Integrity

### Safety Checks

```rust
use sqlitegraph::run_safety_checks;

let report = run_safety_checks(&graph)?;
if report.has_orphans() {
    eprintln!("Warning: {} orphan edges", report.orphan_count());
}
```

### V2 WAL Recovery

The Native V2 backend includes WAL recovery with:
- Transaction rollback for all operations
- Edge cascade cleanup on node deletion
- Cluster reference cleanup

**Tested**: 42 WAL tests passing (recovery, corruption, checkpoints)

---

## 11. CLI Usage

### Available Commands

```bash
# Status
sqlitegraph --command status --database mygraph.db

# List entities
sqlitegraph --command list --database mygraph.db

# Export/Import
sqlitegraph --command dump-graph --output backup.json --database mygraph.db
sqlitegraph --command load-graph --input backup.json --database mygraph.db

# Safety check
sqlitegraph --command safety-check --database mygraph.db

# Graph algorithms
sqlitegraph --backend sqlite --db mygraph.db pagerank --progress
sqlitegraph --backend sqlite --db mygraph.db betweenness --progress
sqlitegraph --backend sqlite --db mygraph.db louvain --progress
```

---

## 12. Migration

### SQLite to Native V2

```rust
// Before (SQLite)
let graph = SqliteGraph::open("data.db")?;
let entity = GraphEntity { /* fields */ };
let id = graph.insert_entity(&entity)?;

// After (Native V2)
let config = GraphConfig::native();
let graph = open_graph("data.db", &config)?;
let node_spec = NodeSpec { /* similar fields */ };
let id = graph.insert_node(node_spec)?;
```

### Key Differences

| Aspect | SQLite Backend | Native V2 Backend |
|--------|----------------|-------------------|
| **Data Types** | `GraphEntity`/`GraphEdge` | `NodeSpec`/`EdgeSpec` |
| **Edge Fields** | `from_id`/`to_id` | `from`/`to` |
| **Construction** | `SqliteGraph::open()` | `open_graph(&config)` |

---

## 13. Troubleshooting

### Common Issues

**Compilation Errors:**
- Add feature flags: `--features native-v2`
- Check backend-specific data types

**Runtime Issues:**
- Run integrity checks: `safety-check` command
- Check buffer configuration for large graphs

**Performance:**
- Use Native V2 for traversals
- Enable parallel WAL recovery for large databases

### Getting Help

```bash
# Check test status
cargo test --lib 2>&1 | tail -5

# Run specific test with output
cargo test test_name -- --nocapture

# Check compilation
cargo check --features native-v2
```

---

## Architecture Status

**v1.0 Features:**
- Native V2 Backend: Clustered adjacency with WAL
- Dual Backend Support: Unified API
- Graph Algorithms: 4 production algorithms
- HNSW Vector Search: Full persistence support
- MVCC Snapshots: Read isolation
- Developer Tools: Introspection, progress tracking, CLI

**Test Coverage:** 300+ tests passing
