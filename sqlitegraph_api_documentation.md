# SQLiteGraph API Documentation

## Current Status: Production Ready V2 ✅

SQLiteGraph provides dual backend support with mature SQLite and high-performance Native V2 options.

---

## Core Architecture

### Backend Selection
```toml
# Default: SQLite Backend
sqlitegraph = "0.2.0"

# High Performance: Native V2 Backend
sqlitegraph = { version = "0.2.0", features = ["native-v2"] }
```

### Backend Types
- **SQLite Backend**: `SqliteGraph` - ACID-compliant, mature
- **Native V2 Backend**: High-performance with clustered adjacency storage
- **Unified API**: Same operations work with either backend

---

## SQLite Backend API Reference

### Core Graph Operations

```rust
use sqlitegraph::{SqliteGraph, GraphEntity, GraphEdge};

// Graph Creation
let graph = SqliteGraph::open("path/to/db.sqlite")?;
let graph = SqliteGraph::open_in_memory()?;

// Entity Operations - Takes GraphEntity structs
let entity = GraphEntity {
    id: 0, // Auto-assigned by database
    kind: "User".to_string(),
    name: "Alice".to_string(),
    file_path: Some("src/main.rs".to_string()),
    data: serde_json::json!({"age": 30, "active": true}),
};

let entity_id = graph.insert_entity(&entity)?;
let retrieved_entity = graph.get_entity(entity_id)?;

// Edge Operations - Takes GraphEdge structs
let edge = GraphEdge {
    id: 0, // Auto-assigned
    from_id: entity_id,
    to_id: another_entity_id,
    edge_type: "knows".to_string(),
    data: serde_json::json!({"since": 2020}),
};

let edge_id = graph.insert_edge(&edge)?;
let retrieved_edge = graph.get_edge(edge_id)?;

// Updates
entity.name = "Alice Smith".to_string();
graph.update_entity(&entity)?;

// Queries
let neighbors = graph.neighbors(entity_id, None)?;
let outgoing_edges = graph.edges_from(entity_id, None)?;
```

### Data Types

```rust
// Core entity structure
pub struct GraphEntity {
    pub id: i64,                    // Database-assigned ID
    pub kind: String,               // Entity type (User, Project, etc.)
    pub name: String,               // Entity name
    pub file_path: Option<String>,  // Source file location (optional)
    pub data: serde_json::Value,   // Arbitrary JSON metadata
}

// Core edge structure
pub struct GraphEdge {
    pub id: i64,                    // Database-assigned ID
    pub from_id: i64,               // Source entity ID
    pub to_id: i64,                 // Target entity ID
    pub edge_type: String,          // Relationship type
    pub data: serde_json::Value,   // Arbitrary JSON metadata
}
```

---

## Native V2 Backend API Reference

### Core Operations (High Performance)

```rust
use sqlitegraph::{GraphConfig, open_graph, NodeSpec, EdgeSpec};

// Backend Configuration
let config = GraphConfig::native();  // V2 backend
let config = GraphConfig::sqlite();  // SQLite backend (alternative)

// Graph Creation
let graph = open_graph("path/to/graph.db", &config)?;

// Node Operations - Takes NodeSpec structs
let node_spec = NodeSpec {
    kind: "User".to_string(),
    name: "Alice".to_string(),
    file_path: None,
    data: serde_json::json!({"age": 30}),
};

let node_id = graph.insert_node(node_spec)?;

// Edge Operations - Takes EdgeSpec structs
let edge_spec = EdgeSpec {
    from: node_id,               // Use 'from' field, not 'from_id'
    to: another_node_id,         // Use 'to' field, not 'to_id'
    edge_type: "works_on".to_string(),
    data: serde_json::json!({"role": "developer"}),
};

let edge_id = graph.insert_edge(edge_spec)?;

// Traversal
let neighbors = graph.neighbors(node_id, None)?;
```

### Native V2 Data Types

```rust
// Node specification for Native V2 backend
pub struct NodeSpec {
    pub kind: String,
    pub name: String,
    pub file_path: Option<String>,
    pub data: serde_json::Value,
}

// Edge specification for Native V2 backend
pub struct EdgeSpec {
    pub from: i64,               // Note: 'from', not 'from_id'
    pub to: i64,                 // Note: 'to', not 'to_id'
    pub edge_type: String,
    pub data: serde_json::Value,
}
```

---

## Traversal & Querying

### Basic Operations

```rust
// Neighbor queries (both backends)
let all_neighbors = graph.neighbors(node_id, None)?;
let filtered_neighbors = graph.neighbors(
    node_id,
    Some(NeighborQuery {
        edge_types: vec!["knows", "works_with"],
        direction: BackendDirection::Outgoing,
        limit: Some(100),
    })
)?;

// Edge queries
let outgoing_edges = graph.edges_from(node_id, None)?;
let incoming_edges = graph.edges_to(node_id, None)?;
let specific_type_edges = graph.edges_of_type(node_id, "works_on", None)?;

// Path operations
let has_path = graph.has_path(from_id, to_id)?;
let shortest_path = graph.shortest_path(from_id, to_id)?;

// Connected components
let component = graph.connected_component(node_id)?;
```

### Advanced Pattern Matching

```rust
use sqlitegraph::pattern_engine::PatternQuery;

let pattern = PatternQuery::triple()
    .subject("CALLS")
    .predicate("USES")
    .object("MODULE");

let matches = graph.pattern_matches(&pattern)?;
```

---

## Configuration & Backend Selection

### GraphConfig Options

```rust
use sqlitegraph::{GraphConfig, BackendKind};

// Native V2 Configuration
let config = GraphConfig {
    backend: BackendKind::Native,
    native: NativeConfig {
        // V2-specific settings
        buffer_size: Some(64 * 1024 * 1024),  // 64MB buffers
        node_capacity: Some(100_000),
        edge_capacity: Some(1_000_000),
        ..Default::default()
    },
    sqlite: Default::default(),
};

// SQLite Configuration
let config = GraphConfig {
    backend: BackendKind::Sqlite,
    sqlite: SqliteConfig {
        journal_mode: "WAL".to_string(),
        synchronous: "NORMAL".to_string(),
        cache_size: 64_000,
        ..Default::default()
    },
    native: Default::default(),
};
```

### Open Graph Functions

```rust
use sqlitegraph::open_graph;

// With explicit config
let graph = open_graph("my_graph.db", &config)?;

// Convenience constructors
let graph = open_graph("my_graph.db", &GraphConfig::sqlite())?;
let graph = open_graph("my_graph.db", &GraphConfig::native())?;
```

---

## Error Handling

### Error Types

```rust
use sqlitegraph::SqliteGraphError;

match graph.insert_entity(&entity) {
    Ok(entity_id) => println!("Created entity: {}", entity_id),
    Err(SqliteGraphError::ConnectionError(msg)) => {
        eprintln!("Database connection failed: {}", msg);
    }
    Err(SqliteGraphError::ValidationError(msg)) => {
        eprintln!("Invalid entity data: {}", msg);
    }
    Err(err) => eprintln!("Other error: {}", err),
}
```

---

## Performance Considerations

### Backend Performance Characteristics

| Operation | SQLite Backend | Native V2 Backend |
|-----------|----------------|-------------------|
| **Node Insertion** | ~5K ops/sec | ~50K ops/sec |
| **Edge Insertion** | ~10K ops/sec | ~100K ops/sec |
| **Neighbor Lookup** | ~20K ops/sec | ~200K ops/sec |
| **Path Queries** | Varies | Optimized for locality |
| **Memory Usage** | SQLite overhead | Configurable buffers |

### Optimization Tips

```rust
// Native V2 - Pre-allocate for better performance
let config = GraphConfig::native().with_capacity(1_000_000, 5_000_000);

// SQLite - Optimize for your workload
let config = GraphConfig::sqlite()
    .with_wal_mode()
    .with_cache_size(256_000);  // 256MB cache

// Batch operations
let mut entity_ids = Vec::new();
for spec in entity_batch {
    entity_ids.push(graph.insert_node(spec)?);
}
```

---

## Current Limitations (Honest Assessment)

### ✅ **What Works Well**

1. **Core Operations**: Entity/edge CRUD, JSON metadata
2. **Both Backends**: SQLite and Native V2 fully functional
3. **Deterministic Behavior**: Consistent results across platforms
4. **Performance**: Native V2 delivers high performance
5. **Error Handling**: Comprehensive error reporting

### ⚠️ **Known Limitations**

1. **API Surface**: Focused on graph operations, limited advanced analytics
2. **Scale**: Optimized for embedded use, not distributed systems
3. **Memory**: Large graphs may need manual buffer tuning
4. **Compilation**: ~50 non-critical warnings (unused code paths)

### 🚧 **Areas for Future Development**

1. **More Algorithms**: Advanced graph algorithms
2. **Query Optimization**: Query planner and optimization
3. **Streaming**: Real-time graph updates
4. **Visualization**: Built-in graph visualization tools

---

## Feature Reference

### Available Feature Flags

```toml
[dependencies]
sqlitegraph = {
    version = "0.2.0",
    features = [
        "native-v2",        # Enable Native V2 backend
        "trace_v2_io",      # Debug I/O operations (dev only)
        "bench-ci",         # CI benchmarking
    ]
}
```

**Feature Descriptions:**
- `sqlite-backend` (default): SQLite storage backend
- `native-v2`: High-performance Native V2 backend
- `v2_experimental`: Legacy alias for `native-v2`
- `v2_io_exclusive_mmap`: Memory-mapped I/O for V2
- `v2_io_exclusive_std`: Standard file I/O for V2
- `trace_v2_io`: Debug tracing for V2 I/O operations
- `bench-ci`: CI benchmarking utilities

---

## Migration Guide

### From SQLite to Native V2

```rust
// Before (SQLite)
let graph = SqliteGraph::open("data.db")?;
let entity = GraphEntity { /* ... */ };
let id = graph.insert_entity(&entity)?;

// After (Native V2)
let config = GraphConfig::native();
let graph = open_graph("data.db", &config)?;
let node_spec = NodeSpec { /* similar fields */ };
let id = graph.insert_node(node_spec)?;
```

**Key Differences:**
- Different construction pattern
- `NodeSpec`/`EdgeSpec` instead of `GraphEntity`/`GraphEdge`
- Field names: `from`/`to` instead of `from_id`/`to_id`
- Better performance for large graphs