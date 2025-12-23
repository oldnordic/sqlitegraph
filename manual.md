# SQLiteGraph Manual

This manual describes how to use SQLiteGraph with its dual backend architecture (SQLite and Native V2) for deterministic graph database operations.

---

## 1. Quick Start

### Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
sqlitegraph = "0.2.5"

# For Native V2 high-performance backend
sqlitegraph = { version = "0.2.5", features = ["native-v2"] }
```

### Basic Usage

```bash
# Run tests
cargo test

# Run benchmarks
cargo bench

# Run working examples
cargo run --example basic_functionality_test
cargo run --example native_v2_test --features native-v2
```

---

## 2. Backend Selection

### SQLite Backend (Default)

**Use Case**: General purpose, ACID transactions, existing SQLite data

```rust
use sqlitegraph::{SqliteGraph, GraphEntity, GraphEdge};

let graph = SqliteGraph::open_in_memory()?;
// SQLite operations with full ACID compliance
```

### Native V2 Backend (High Performance)

**Use Case**: High-performance scenarios, large graphs, speed-critical applications

```rust
use sqlitegraph::{GraphConfig, open_graph, NodeSpec, EdgeSpec};

let config = GraphConfig::native();
let graph = open_graph("graph.db", &config)?;
// Optimized for performance with clustered adjacency
```

### Backend Comparison

| Characteristic | SQLite Backend | Native V2 Backend |
|----------------|----------------|-------------------|
| **Performance** | Standard SQLite performance | 10x faster (50K-100K ops/sec) |
| **Transactions** | Full ACID compliance | Atomic commits, optimized |
| **Maturity** | Battle-tested, mature | Production ready, V2 architecture |
| **Memory Usage** | SQLite overhead | Configurable buffers |
| **Use Cases** | General purpose, data integrity | High performance, large graphs |

---

## 3. Core Operations

### Entity Management (SQLite Backend)

```rust
use sqlitegraph::{SqliteGraph, GraphEntity};

let graph = SqliteGraph::open_in_memory()?;

// Create entity
let entity = GraphEntity {
    id: 0, // Auto-assigned
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
    to: node_id, // self-loop
    edge_type: "self_ref".to_string(),
    data: serde_json::json!({"type": "demo"}),
};

let edge_id = graph.insert_edge(edge_spec)?;
```

### Traversal Operations

```rust
// Get neighbors (both backends)
let neighbors = graph.neighbors(entity_id, None)?;
println!("Found {} neighbors", neighbors.len());

// Path operations
if graph.has_path(from_id, to_id)? {
    let path = graph.shortest_path(from_id, to_id)?;
    println!("Path: {:?}", path);
}
```

---

## 4. Testing Guide

### Running Tests

```bash
# All tests
cargo test

# Specific backend tests
cargo test --features native-v2

# Library tests only
cargo test --lib

# Integration tests
cargo test --test '*'

# Test with verbose output
cargo test -- --nocapture

# Test specific patterns
cargo test '*neighbors*'
cargo test '*v2*'
```

### Available Test Categories

**Core Functionality Tests:**
- `lib_api_smoke_tests` - Basic library API tests
- `entity_tests` - Entity CRUD operations
- `edge_tests` - Edge management
- `pattern_engine_tests` - Pattern matching
- `query_cache_tests` - Query caching

**V2-Specific Tests:**
- `v2_edge_insertion_corruption_regression` - V2 corruption prevention
- `phase65_cluster_size_corruption_regression` - V2 cluster size handling
- `phase73_node_count_corruption_capture` - V2 node counting
- `v2_graph_ops_smoke` - V2 basic operations

**Integration Tests:**
- `integration_tests` - End-to-end workflows
- `safety_tests` - Data integrity validation
- `performance_tests` - Performance regression checks

### Test Results Interpretation

**Expected Results:**
- Library tests: 69/69 passing ✅
- V2 tests: All passing with corruption prevention ✅
- Examples: Working with 10+ nodes, 20+ edges ✅

**Warning Signs:**
- Any test failures: Investigate immediately
- Performance regression: Check benchmark baselines
- V2 corruption test failures: Critical, investigate storage layer

---

## 5. Performance Optimization

### Native V2 Performance Tuning

```rust
use sqlitegraph::{GraphConfig, NativeConfig};

// High-performance configuration
let config = GraphConfig::native()
    .with_buffer_size(128 * 1024 * 1024)  // 128MB buffers
    .with_capacity(1_000_000, 5_000_000); // 1M nodes, 5M edges pre-allocation

let graph = open_graph("large_graph.db", &config)?;
```

### SQLite Performance Tuning

```rust
use sqlitegraph::{GraphConfig, SqliteConfig};

// Optimized SQLite configuration
let config = GraphConfig::sqlite()
    .with_wal_mode()                    // Better concurrency
    .with_cache_size(256_000)           // 256MB cache
    .with_synchronous_mode("NORMAL");   // Balanced safety/performance

let graph = open_graph("optimized.db", &config)?;
```

### Performance Benchmarks

```bash
# Run all benchmarks
cargo bench

# Specific benchmark suites
cargo bench --bench insert
cargo bench --bench bfs
cargo bench --bench k_hop

# Performance regression check
cargo test perf_gate_tests
```

### Current Performance Characteristics

| Operation | SQLite Backend | Native V2 Backend |
|-----------|----------------|-------------------|
| **Node Insert** | ~5,000 ops/sec | ~50,000 ops/sec |
| **Edge Insert** | ~10,000 ops/sec | ~100,000 ops/sec |
| **Neighbor Query** | ~20,000 ops/sec | ~200,000 ops/sec |
| **Path Finding** | Variable | Optimized for locality |

---

## 6. Error Handling & Debugging

### Common Error Types

```rust
use sqlitegraph::SqliteGraphError;

match graph.insert_entity(&entity) {
    Ok(id) => println!("Created entity: {}", id),
    Err(SqliteGraphError::ValidationError(msg)) => {
        eprintln!("Validation failed: {}", msg);
    }
    Err(SqliteGraphError::ConnectionError(msg)) => {
        eprintln!("Database connection failed: {}", msg);
    }
    Err(err) => eprintln!("Unexpected error: {}", err),
}
```

### Debug Features

```toml
# Enable debug tracing for V2 I/O operations
sqlitegraph = { version = "0.2.5", features = ["trace_v2_io"] }
```

```bash
# Run with debug output
RUST_LOG=debug cargo run --example native_v2_test --features trace_v2_io
```

### Environment Variables

```bash
# Enable detailed logging
export RUST_LOG=debug

# Enable V2 slot debugging
export V2_SLOT_DEBUG=1

# Enable cluster debugging
export EDGE_CLUSTER_DEBUG=1

# Enable transaction debugging
export TX_BEGIN_AUDIT=1
```

---

## 7. Safety & Data Integrity

### Built-in Safety Features

**Orphan Edge Detection:**
```rust
use sqlitegraph::run_safety_checks;

let safety_report = run_safety_checks(&graph)?;
if safety_report.has_orphans() {
    eprintln!("Warning: {} orphan edges found", safety_report.orphan_count());
}
```

**Integrity Validation:**
```rust
// Comprehensive integrity sweep
let issues = graph.run_integrity_sweep()?;
for issue in issues {
    println!("Issue: {:?}", issue);
}
```

**Corruption Prevention (V2):**
- Automatic cluster offset validation
- Node slot corruption prevention
- Atomic commit system
- Comprehensive V2 regression tests
- **WAL Transaction Recovery**: Full rollback coverage for all operations (v0.2.5)
- **Edge Cascade Cleanup**: Automatic edge cleanup on node deletion (v0.2.5)
- **Cluster Reference Cleanup**: Proper memory management on node deletion (v0.2.5)

### V2 WAL Recovery System (v0.2.5)

The Native V2 backend includes a complete Write-Ahead Log (WAL) recovery system with 100% transaction rollback coverage:

**Handle Operations (11/11 complete)**:
- Node operations: insert, update, delete
- String storage: insert and manage
- Cluster management: create with proper allocation
- Edge operations: insert, update, delete
- Free space management: allocate and deallocate
- Header management: update metadata

**Rollback Operations (11/11 complete)**:
- All node operations can be rolled back
- All edge operations can be rolled back
- Cluster allocation can be rolled back
- Free space operations use conservative rollback
- Transaction integrity guaranteed

**Graph Integrity Features**:
- Edge cascade cleanup: When deleting a node, all referencing edges are automatically cleaned up
- Cluster reference cleanup: When deleting a node, cluster storage is properly deallocated
- NodeRecordV2 cleanup: Edge operations maintain consistent node metadata

**Testing**: 647/647 tests passing (100% coverage)

### Recommended Safety Practices

1. **Regular Safety Checks**: Run `run_safety_checks()` before important operations
2. **Backup Strategy**: Regular backups for production data
3. **Transaction Usage**: Use transactions for multi-step operations
4. **Performance Monitoring**: Monitor benchmark gates for regressions

---

## 8. Migration Guide

### From SQLite to Native V2

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

### Key Migration Differences

| Aspect | SQLite Backend | Native V2 Backend |
|--------|----------------|-------------------|
| **Data Types** | `GraphEntity`/`GraphEdge` | `NodeSpec`/`EdgeSpec` |
| **Edge Fields** | `from_id`/`to_id` | `from`/`to` |
| **Construction** | `SqliteGraph::open()` | `open_graph(&config)` |
| **Performance** | Standard | High performance |

### Data Migration Strategy

```rust
// 1. Export from SQLite
let sqlite_graph = SqliteGraph::open("old.db")?;
let entities = sqlite_graph.all_entities()?;
let edges = sqlite_graph.all_edges()?;

// 2. Import to Native V2
let config = GraphConfig::native();
let v2_graph = open_graph("new.db", &config)?;

for entity in entities {
    let node_spec = NodeSpec {
        kind: entity.kind,
        name: entity.name,
        file_path: entity.file_path,
        data: entity.data,
    };
    v2_graph.insert_node(node_spec)?;
}

// 3. Verify migration
let safety_check = run_safety_checks(&v2_graph)?;
assert!(!safety_check.has_orphans());
```

---

## 9. CLI Usage

### Available Commands

```bash
# Status check
cargo run --bin sqlitegraph -- --command status

# List entities
cargo run --bin sqlitegraph -- --command list

# Safety checks
cargo run --bin sqlitegraph -- --command safety-check --strict
```

### CLI Backend Selection

```bash
# SQLite backend (default)
cargo run --bin sqlitegraph -- --command status --db mydb.sqlite

# Native V2 backend
cargo run --bin sqlitegraph --features native-v2 -- --command status --db mydb.native
```

---

## 10. Troubleshooting

### Common Issues

**Compilation Errors:**
- Missing features: Add appropriate feature flags
- API mismatches: Check backend-specific data types
- Rust version: Ensure compatible Rust version

**Runtime Issues:**
- Database corruption: Run integrity checks
- Performance: Check buffer configuration
- Memory usage: Monitor graph size vs buffer allocation

**Performance Issues:**
- Slow queries: Consider Native V2 backend
- Memory pressure: Tune buffer sizes
- Large graphs: Use batch operations

### Getting Help

```bash
# Check test status
cargo test --lib 2>&1 | tail -5

# Run specific failing test
cargo test test_name -- --nocapture

# Check compilation
cargo check --features native-v2
```

### Debug Checklist

1. ✅ **Backend Selection**: Correct backend for use case
2. ✅ **Feature Flags**: Required features enabled
3. ✅ **API Usage**: Correct data types for backend
4. ✅ **Error Handling**: Proper error handling in place
5. ✅ **Performance**: Appropriate configuration
6. ✅ **Safety**: Regular integrity checks

---

## 11. Development Notes

### Architecture Status

- **V2 Native Backend**: Production ready ✅
- **V1 Legacy Code**: Permanently removed ✅
- **Dual Backend Support**: Fully functional ✅
- **Corruption Prevention**: Active ✅

### Module Organization

All modules follow the 300 LOC limit for auditability:
- Core graph operations
- Backend abstractions
- Pattern matching engine
- Safety and validation tools
- Performance benchmarking

### Future Development

Focus areas for future development:
- More advanced graph algorithms
- Query optimization
- Real-time streaming updates
- Enhanced visualization tools