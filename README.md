# SQLiteGraph

**Embedded Graph Database with Native V2 Backend**

SQLiteGraph is an embedded graph database in Rust featuring a dual backend architecture. It provides SQLite and Native V2 storage options with graph algorithms, HNSW vector search, and MVCC snapshots.

**Current Status: v1.0.0 - Tested with Phase 1-10 completion**

SQLiteGraph provides two backend options:
- **SQLite Backend**: SQLite storage with ACID transactions
- **Native V2 Backend**: Clustered adjacency storage with WAL

## Features

### Native V2 Architecture
- **Clustered Adjacency Storage**: Stores edges in clusters for locality
- **Write-Ahead Logging (WAL)**: Transaction logging with crash recovery
- **Snapshot System**: Export/import with lifecycle management
- **Cross-Platform Atomic Operations**: Concurrent access across platforms
- **Storage Format**: Binary format with 70%+ size reduction vs legacy V1

### Dual Backend Architecture
- **SQLite Backend**: Traditional SQLite with full ACID transactions
- **Native V2 Backend**: Clustered adjacency for traversal-heavy workloads
- **Unified API**: Single API works with both backends
- **Runtime Selection**: Switch backends via configuration

### Core Graph Operations
- **Entity/Node Management**: Insert, update, retrieve, delete
- **Edge Management**: Create and manage typed relationships
- **JSON Data Storage**: Arbitrary JSON metadata on entities and edges
- **Bulk Operations**: Batch insert for higher throughput

### Traversal & Querying
- **Neighbor Queries**: Get incoming/outgoing connections
- **Pattern Matching**: Graph pattern queries
- **Traversal Algorithms**: BFS, shortest path, connected components

### Graph Algorithms (Phase 8)
- **PageRank**: Importance ranking (O(|E|) iterations)
- **Betweenness Centrality**: Node importance via shortest paths (O(|V||E|))
- **Label Propagation**: Fast community detection (O(|E|))
- **Louvain Method**: Modularity-based clustering (O(|E| log |V|))

### Performance & Reliability
- **MVCC Snapshots**: Read isolation with snapshot views
- **Parallel WAL Recovery**: 2-3x speedup for large WAL files (500+ transactions)
- **Automated Benchmarks**: Criterion-based regression detection
- **Safety Tools**: Orphan edge detection and integrity checks

### Vector Search (HNSW)
- **HNSW Algorithm**: Hierarchical Navigable Small World for ANN search
- **Supported Metrics**: Cosine, Euclidean, Dot Product, Manhattan
- **OpenAI Compatible**: Support for 1536-dimensional embeddings
- **Flexible Dimensions**: Any size from 1-4096

### Developer Tools (Phase 9)
- **Introspection API**: `GraphIntrospection` for statistics and debugging
- **Progress Tracking**: `ProgressCallback` with `ConsoleProgress`
- **CLI Debug Commands**: `debug-stats`, `debug-dump`, `debug-trace`
- **Algorithm CLI Commands**: `pagerank`, `betweenness`, `louvain` with progress bars

## Performance Benchmarks

**Based on actual benchmark runs (Phase 3, 7):**

Native V2 Backend Performance (tested):
- **Node Insert**: ~50K ops/sec
- **Edge Insert**: ~100K ops/sec for bulk inserts
- **Neighbor Query**: Sub-millisecond for clustered nodes
- **Vector Search**: Sub-millisecond with 95%+ accuracy
- **Parallel WAL Recovery**: 2-3x speedup for 500+ transactions
- **Storage Efficiency**: 70%+ reduction vs V1 format

**Note**: Performance varies based on workload, hardware, and configuration.

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
sqlitegraph = "1.0"
```

### SQLite Backend (Default)

```rust
use sqlitegraph::{SqliteGraph, GraphEntity, GraphEdge};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let graph = SqliteGraph::open_in_memory()?;

    let user_entity = GraphEntity {
        id: 0,
        kind: "User".to_string(),
        name: "Alice".to_string(),
        file_path: None,
        data: serde_json::json!({"age": 30}),
    };

    let user_id = graph.insert_entity(&user_entity)?;
    println!("Created entity: {}", user_id);

    Ok(())
}
```

### Native V2 Backend

```toml
[dependencies]
sqlitegraph = { version = "1.0", features = ["native-v2"] }
```

```rust
use sqlitegraph::{GraphConfig, open_graph, NodeSpec, EdgeSpec};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cfg = GraphConfig::native();
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("graph.db");

    let graph = open_graph(&db_path, &cfg)?;

    let node_spec = NodeSpec {
        kind: "User".to_string(),
        name: "Alice".to_string(),
        file_path: None,
        data: serde_json::json!({"age": 30}),
    };
    let user_id = graph.insert_node(node_spec)?;

    println!("Created node: {}", user_id);
    Ok(())
}
```

## Backend Selection Guide

| Use Case | Recommended Backend | Why |
|----------|-------------------|-----|
| **High-Performance Systems** | Native V2 Backend | Clustered adjacency for traversals |
| **Enterprise Applications** | SQLite Backend | ACID transactions, tooling ecosystem |
| **Existing SQLite Integration** | SQLite Backend | Direct compatibility |
| **Vector Search Workloads** | Native V2 Backend | HNSW integration |
| **Development/Testing** | Either Backend | Unified API, both support in-memory |

### Feature Flags

```toml
# Default - SQLite backend only
sqlitegraph = "1.0"

# Native V2 backend
sqlitegraph = { version = "1.0", features = ["native-v2"] }

# Development features - I/O tracing
sqlitegraph = { version = "1.0", features = ["trace_v2_io"] }
```

## CLI Tool

```bash
# Basic status
sqlitegraph --command status --database memory

# List entities
sqlitegraph --command list --database mygraph.db

# Export/import
sqlitegraph --command dump-graph --output backup.json --database mygraph.db
sqlitegraph --command load-graph --input backup.json --database mygraph.db

# HNSW vector search
sqlitegraph --backend sqlite --db mygraph.db hnsw-create --dimension 768 --distance-metric cosine
sqlitegraph --backend sqlite --db mygraph.db hnsw-insert --index-name vectors --input vectors.json
sqlitegraph --backend sqlite --db mygraph.db hnsw-search --index-name vectors --input query.json --k 10

# Algorithm commands (with progress bars)
sqlitegraph --backend sqlite --db mygraph.db pagerank --progress
sqlitegraph --backend sqlite --db mygraph.db betweenness --progress
sqlitegraph --backend sqlite --db mygraph.db louvain --progress
```

## Graph Algorithms

```rust
use sqlitegraph::algo;

// PageRank - importance ranking
let scores = algo::pagerank(&graph, 0.85, 50)?;

// Betweenness Centrality - node importance via shortest paths
let centrality = algo::betweenness_centrality(&graph)?;

// Label Propagation - fast community detection
let communities = algo::label_propagation(&graph)?;

// Louvain - modularity-based clustering
let partition = algo::louvain_communities(&graph, 0.01)?;

// With progress tracking
use sqlitegraph::progress::ConsoleProgress;
let scores = algo::pagerank_with_progress(&graph, 0.85, 50, ConsoleProgress::new())?;
```

## Testing

**Test Coverage (Phase 10):**
- 42 WAL tests passing (recovery, corruption, checkpoints)
- 53 concurrent MVCC tests passing (snapshots, stress testing)
- 27 algorithm tests passing (PageRank, Betweenness, Louvain, Label Propagation)
- 134 HNSW tests passing
- 65 MVCC lifecycle tests passing

```bash
# Run all tests
cargo test --workspace

# With Native V2 backend
cargo test --workspace --features native-v2

# Run benchmarks
cargo bench

# Documentation tests
cargo test --doc
```

## Documentation

- **[Operator Manual](manual.md)** - Comprehensive usage guide
- **[CHANGELOG](CHANGELOG.md)** - Version history
- **[API Docs](https://docs.rs/sqlitegraph)** - rustdoc API reference

## Architecture

### Design Principles
- **300 LOC Module Limit**: Maintainable boundaries
- **TDD Methodology**: Test-driven development
- **Performance Benchmarks**: Criterion-based regression gates

### Module Organization
- Core graph operations with dual backend support
- Graph algorithms (centrality, community detection)
- HNSW vector search with persistence
- MVCC snapshots for read isolation
- Introspection and debugging tools

## License

GPL-3.0-or-later - see [LICENSE](LICENSE) for details.

## Contributing

Contributions welcome. Please:
1. Read the [Operator Manual](manual.md)
2. Run tests to verify setup
3. Follow TDD methodology
4. Keep modules under 300 LOC
5. Add tests for new features
