# SQLiteGraph

Deterministic, embedded graph database with SQLite and high-performance Native V2 backends.

**Current Status: Active Development**

SQLiteGraph provides two backend options for different use cases:
- **SQLite Backend**: Mature, ACID-compliant, battle-tested
- **Native V2 Backend**: High-performance, clustered adjacency, experimental

## Features

### Dual Backend Architecture
- **SQLite Backend**: Traditional SQLite storage with full ACID transactions
- **Native V2 Backend**: Custom binary format with clustered adjacency for performance
- **Backend Abstraction**: Unified API works with either backend
- **Easy Migration**: Switch backends with configuration changes

### Core Graph Operations
- **Entity Management**: Insert, update, retrieve, delete graph entities
- **Edge Management**: Create and manage relationships between entities
- **JSON Data Storage**: Arbitrary JSON metadata with entities and edges
- **Deterministic Operations**: Consistent ordering and behavior

### Traversal & Querying
- **Neighbor Queries**: Get incoming/outgoing connections
- **Pattern Matching**: Advanced graph pattern queries
- **Traversal Algorithms**: BFS, shortest path, connected components
- **Reasoning Pipelines**: Multi-step analysis with filtering and scoring

### Performance & Safety
- **Benchmark Gates**: Automated performance regression prevention
- **Safety Tools**: Orphan edge detection, integrity validation
- **Memory Management**: Configurable caching and buffer management
- **Error Handling**: Comprehensive error reporting and recovery

## Quick Start

Add to your `Cargo.toml`:

```toml
[dependencies]
sqlitegraph = "0.2.1"
```

### SQLite Backend (Default)

```rust
use sqlitegraph::{SqliteGraph, GraphEntity, GraphEdge};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create in-memory SQLite graph
    let graph = SqliteGraph::open_in_memory()?;

    // Create entities with proper structure
    let user_entity = GraphEntity {
        id: 0, // Assigned by database
        kind: "User".to_string(),
        name: "Alice".to_string(),
        file_path: None,
        data: serde_json::json!({"age": 30}),
    };

    let project_entity = GraphEntity {
        id: 0,
        kind: "Project".to_string(),
        name: "SQLiteGraph".to_string(),
        file_path: None,
        data: serde_json::json!({"status": "active"}),
    };

    // Insert entities
    let user_id = graph.insert_entity(&user_entity)?;
    let project_id = graph.insert_entity(&project_entity)?;

    // Create relationship
    let works_on_edge = GraphEdge {
        id: 0,
        from_id: user_id,
        to_id: project_id,
        edge_type: "works_on".to_string(),
        data: serde_json::json!({"role": "developer"}),
    };

    let edge_id = graph.insert_edge(&works_on_edge)?;

    println!("Created graph: {} entities, {} edges", 2, 1);
    println!("Edge ID: {}", edge_id);

    Ok(())
}
```

### Native V2 Backend (High Performance)

Enable the Native V2 backend in your `Cargo.toml`:

```toml
[dependencies]
sqlitegraph = { version = "0.2.1", features = ["native-v2"] }
```

```rust
use sqlitegraph::{GraphConfig, open_graph, NodeSpec, EdgeSpec};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Use Native V2 backend
    let cfg = GraphConfig::native();
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("graph.db");

    let graph = open_graph(&db_path, &cfg)?;

    // Insert nodes
    let node_spec = NodeSpec {
        kind: "User".to_string(),
        name: "Alice".to_string(),
        file_path: None,
        data: serde_json::json!({"age": 30}),
    };
    let user_id = graph.insert_node(node_spec)?;

    // Insert edges
    let edge_spec = EdgeSpec {
        from: user_id,
        to: user_id, // self-loop for demo
        edge_type: "self_ref".to_string(),
        data: serde_json::json!({"type": "demo"}),
    };
    let edge_id = graph.insert_edge(edge_spec)?;

    println!("Native V2: Node {}, Edge {}", user_id, edge_id);
    Ok(())
}
```

## Testing

```bash
# Run all tests
cargo test

# Test specific backend
cargo test --features native-v2

# Run benchmarks
cargo bench

# Run working examples
cargo run --example basic_functionality_test
cargo run --example native_v2_test --features native-v2
```

## Current Capabilities

### ✅ **What Works Today**

**Core Operations:**
- Entity CRUD operations with JSON metadata
- Edge creation and management
- In-memory and persistent storage
- Both backends fully functional

**Performance:**
- Native V2: 50K-100K operations/second (benchmarked)
- SQLite: Standard SQLite performance with optimizations
- Deterministic behavior across platforms

**Data Integrity:**
- ACID transactions (SQLite backend)
- Corruption prevention in V2 backend
- Comprehensive safety checks
- Benchmark regression gates

### ⚠️ **Current Limitations**

**Scope:**
- Focused on embedded use cases (not distributed)
- Single-machine graph processing
- No built-in clustering or replication

**API Surface:**
- Concentrated on graph operations, limited advanced analytics
- No built-in machine learning or advanced analytics
- Limited visualization capabilities

**Performance Characteristics:**
- Native V2 optimized for read-heavy workloads
- Write performance varies by workload pattern
- Large graphs (>1M edges) may need tuning

## Documentation

- **[Manual](manual.md)** - Detailed operator guide
- **[API Documentation](../docs/sqlitegraph_api_documentation.md)** - Complete API reference
- **[Examples](examples/)** - Working code examples
- **[CHANGELOG](CHANGELOG.md)** - Version history and changes

## License

GPL-3.0-only - see [LICENSE](LICENSE) for details.

## Development Notes

### V2 Architecture Status

**V2 Native Backend Status**
- All V1 legacy code removed
- Clustered adjacency storage implemented
- Corruption prevention active
- Comprehensive test coverage
- Experimental high-performance features

### Performance Benchmarks

Current performance characteristics (Native V2):
- **Node insertion**: ~50K ops/sec
- **Edge insertion**: ~100K ops/sec
- **Traversal**: Varies by graph structure
- **Memory usage**: Optimized with configurable buffers

### Known Limitations

1. **Compilation Warnings**: ~50 warnings (non-critical, mostly unused code paths)
2. **Single Machine**: No built-in distributed capabilities
3. **Memory Usage**: Large graphs may require buffer tuning
4. **Documentation**: API evolving as new features added