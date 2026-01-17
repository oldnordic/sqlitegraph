# Bug Report: Native V2 Backend Edge Operations Fail

**Reported**: 2025-01-01
**Component**: Native V2 Backend
**Severity**: High - Blocker for production use
**Version**: 0.2.10
**Feature**: `native-v2`

---

## Summary

The Native V2 backend successfully inserts entity nodes, but **edge operations fail** with a corruption error when reading node data during edge insertion or queries.

---

## Reproduction Steps

```rust
use sqlitegraph::{open_graph, GraphConfig, NodeSpec, EdgeSpec};

fn main() -> anyhow::Result<()> {
    let config = GraphConfig::native();
    let graph = open_graph("/tmp/test_native.db", &config)?;

    // This works - node insertion succeeds
    let node1 = graph.insert_node(NodeSpec {
        kind: "Function".to_string(),
        name: "main".to_string(),
        file_path: Some("src/main.rs".to_string()),
        data: serde_json::json!({}),
    })?;

    let node2 = graph.insert_node(NodeSpec {
        kind: "Function".to_string(),
        name: "helper".to_string(),
        file_path: Some("src/helper.rs".to_string()),
        data: serde_json::json!({}),
    })?;

    // This fails - edge insertion throws corruption error
    graph.insert_edge(EdgeSpec {
        from: node1 as i64,
        to: node2 as i64,
        edge_type: "CALLS".to_string(),
        data: serde_json::json!({}),
    })?;

    Ok(())
}
```

**Test file**: See `/home/feanor/Projects/magellan/tests/native_v2_backend.rs`

---

## Error Output

```
DEBUG: Before writing edge 1 - header.edge_count = 1
DEBUG: After writing edge 1 - header.edge_count = 1
[V2_SLOT_DEBUG] READ_PRE_PARSE: node_id=1, slot_offset=0x200, version=2, io_path=FILE_READ_BYTES
Error: connection error: Corrupt node record 0: Invalid V2 node record version 0
```

The error occurs in `node_store.rs` during a read operation after the edge is written to the header.

---

## Root Cause Analysis

Hypothesis: There may be an inconsistency between:
1. How nodes are written (version 2 format)
2. How nodes are read during edge operations (expecting different version/format)

The error "Invalid V2 node record version 0" suggests:
- The node record at the target location has version 0
- But the reader expects version 2 (V2 format)
- This could be an uninitialized or misaligned read

---

## Workaround

Use the SQLite backend instead:

```rust
let config = GraphConfig::sqlite();
let graph = open_graph("data.db", &config)?;
```

The SQLite backend works correctly for all operations.

---

## Additional Issues

### 1. Excessive Debug Output

The Native V2 backend produces thousands of lines of debug output during normal operation:

```
[CLUSTER_DEBUG] initialize_v2_header() called - fixing cluster offsets to prevent node slot corruption
[V2_SLOT_DEBUG] WRITE: node_id=1, slot_offset=0x200, version=2, io_path=FILE_WRITE_BYTES
[V2_SLOT_DEBUG] WRITE: node_id=2, slot_offset=0x1200, version=2, io_path=FILE_WRITE_BYTES
...
```

This output should be behind a `debug` feature flag or controlled by an environment variable.

**Suggested fix**:
```rust
#[cfg(feature = "trace-v2-io")]
// ... debug output ...
```

---

## Expected Behavior

1. Edge insertion should succeed without corruption errors
2. Node data should be readable in the same format it was written
3. Debug output should be minimal unless explicitly enabled

---

## Environment

- OS: Linux (kernel 6.12+)
- Rust: Latest stable
- sqlitegraph: 0.2.10
- Feature: `native-v2`

---

## Files to Investigate

1. `/sqlitegraph/src/backend/native/node_store.rs` - Where the error occurs
2. `/sqlitegraph/src/backend/native/edge_store/` - Edge insertion logic
3. `/sqlitegraph/src/backend/native/graph_file/` - Graph file format handling
4. `/sqlitegraph/src/backend/native/graph_backend.rs` - Backend implementation

---

## Test Case

```bash
cd /home/feanor/Projects/magellan
cargo test --test native_v2_backend --features native-v2
```

This test will:
1. Create 1000 nodes (succeeds)
2. Insert 1000 edges (fails on first edge)
3. Run 1000 neighbor queries (skipped due to edge failure)
