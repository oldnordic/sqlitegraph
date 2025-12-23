# SQLiteGraph

Deterministic, embedded graph database with SQLite and high-performance Native V2 backends.

**Current Status: Active Development**

**V2 WAL Recovery System**: 100% Feature Coverage Complete (v0.2.5)
- All handle operations implemented (11/11)
- All rollback operations implemented (11/11)
- Edge cascade cleanup complete
- Full transaction integrity and graph corruption prevention

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
- **Edge Cascade Cleanup**: Automatic edge cleanup on node deletion (graph integrity)
- **JSON Data Storage**: Arbitrary JSON metadata with entities and edges
- **Deterministic Operations**: Consistent ordering and behavior

### Transaction Safety & Recovery
- **Write-Ahead Log (WAL)**: Durable transaction logging
- **Rollback Operations**: Complete rollback coverage for all operations
- **Transaction Recovery**: Crash recovery with WAL replay
- **Graph Integrity**: Cascade cleanup prevents dangling references

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
sqlitegraph = "0.2.3"
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
sqlitegraph = { version = "0.2.2", features = ["native-v2"] }
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

## Backend Selection Guide

| Use Case | Recommended Backend | Why |
|----------|-------------------|-----|
| **General purpose** | SQLite Backend | Mature, reliable, ACID transactions |
| **High performance** | Native V2 Backend | Optimized storage, clustered adjacency |
| **Existing SQLite data** | SQLite Backend | Use existing databases directly |
| **Maximum speed** | Native V2 Backend | Custom binary format, minimal overhead |
| **Development/testing** | Either Backend | Both work in-memory for fast iteration |

### Feature Flags

```toml
# Default - SQLite backend only
sqlitegraph = "0.2.3"

# Native V2 backend (high performance)
sqlitegraph = { version = "0.2.2", features = ["native-v2"] }

# Legacy compatibility (alias)
sqlitegraph = { version = "0.2.2", features = ["v2_experimental"] }

# Development features
sqlitegraph = { version = "0.2.2", features = ["trace_v2_io"] }
```

## Examples

Run the working examples:

```bash
# Basic SQLite functionality
cargo run --example basic_functionality_test

# Native V2 backend demonstration
cargo run --example native_v2_test --features native-v2

# Performance characterization (advanced)
cargo run --example phase55_v2_performance_characterization --features native-v2
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

## Testing

```bash
# Run all tests
cargo test

# Test specific backend
cargo test --features native-v2

# Run benchmarks
cargo bench
```

## Documentation

- **[Manual](manual.md)** - Detailed operator guide
- **[API Documentation](docs/sqlitegraph_api_documentation.md)** - Complete API reference
- **[Examples](examples/)** - Working code examples
- **[CHANGELOG](CHANGELOG.md)** - Version history and changes

## Development Notes

### V2 Architecture Status

**V2 Native Backend Status**
- All V1 legacy code removed
- Clustered adjacency storage implemented
- **WAL Recovery System**: 100% feature coverage (v0.2.5)
- **Transaction Rollback**: Complete rollback operations (11/11)
- **Edge Cascade Cleanup**: Automatic cleanup on node deletion
- Corruption prevention active
- Comprehensive test coverage (647/647 tests passing)
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

### V2 WAL Recovery Implementation Status

**Completed Operations (v0.2.5)**:
- ✅ All handle operations (11/11): node_insert, node_update, node_delete, string_insert, cluster_create, edge_insert, edge_update, edge_delete, free_space_allocate, free_space_deallocate, header_update
- ✅ All rollback operations (11/11): rollback_node_insert, rollback_node_update, rollback_node_delete, rollback_string_insert, rollback_header_update, rollback_edge_insert, rollback_edge_update, rollback_edge_delete, rollback_free_space_allocate, rollback_free_space_deallocate, rollback_cluster_create
- ✅ Edge cascade cleanup on node deletion
- ✅ Cluster reference cleanup on node deletion
- ✅ NodeRecordV2 cleanup on edge insert rollback

**Technical Documentation**: See [CHANGELOG v0.2.5](CHANGELOG.md) for implementation details

## License

GPL-3.0-only - see [LICENSE](LICENSE) for details.

## Contributing

This crate follows a deterministic development approach with:
- 300 LOC module limits for auditability
- Comprehensive test coverage requirements
- Performance regression prevention
- V2-only development (V1 permanently removed)