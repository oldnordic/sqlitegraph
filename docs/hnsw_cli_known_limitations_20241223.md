# HNSW CLI - Known Limitations

**Date**: 2025-12-23
**Status**: Documented Limitations
**Affected Component**: `sqlitegraph-cli`

## Overview

The HNSW (Hierarchical Navigable Small World) vector search functionality is **fully implemented in the sqlitegraph library** but has a **known limitation in the CLI** regarding persistence across command invocations.

## What Works

### Library API (Fully Functional)

The Rust library API provides complete HNSW functionality:

```rust
use sqlitegraph::{SqliteGraph, hnsw::{HnswConfig, DistanceMetric}};

// Open database
let graph = SqliteGraph::open("mydb.db")?;

// Create HNSW index
let config = HnswConfig::builder()
    .dimension(768)
    .distance_metric(DistanceMetric::Cosine)
    .build()?;

let hnsw = graph.hnsw_index("vectors", config)?;

// Insert vectors with metadata
let vector_id = hnsw.insert_vector(&embedding, Some(metadata))?;

// Search for similar vectors
let results = hnsw.search(&query_embedding, 10)?;

// Get statistics
let stats = hnsw.statistics()?;
```

**Library Features**:
- Complete HNSW implementation
- In-memory index persistence within a single process
- All operations work correctly
- Thread-safe with RwLock protection

### CLI Commands (Implemented但有 Limitations)

All four HNSW CLI commands are **implemented and compile**:
- `hnsw-create` - Creates HNSW index with configuration
- `hnsw-insert` - Inserts vectors from JSON file
- `hnsw-search` - Performs KNN search
- `hnsw-stats` - Displays index statistics

## Known Limitation

### CLI: No Cross-Session Persistence

**Problem**: HNSW indexes are stored in-memory within `SqliteGraph` and do not persist across CLI command invocations.

**Root Cause**:
```rust
// Each CLI invocation creates a NEW SqliteGraph instance
fn main() {
    let client = open_backend(&config)?;  // Creates NEW SqliteGraph
    run_command(&client, &command, &args)?;  // Runs ONE command
    // Process exits - in-memory HNSW indexes lost
}
```

**Impact**:
```bash
# Session 1: Create index
$ sqlitegraph --backend sqlite --db test.db hnsw-create --dimension 768 ...
{"status": "created"}

# Session 2: Try to insert vectors (FAILS)
$ sqlitegraph --backend sqlite --db test.db hnsw-insert --input vectors.json
{"error": "HNSW index 'default' not found", "vectors_inserted": 0}
```

**Why This Happens**:
1. `hnsw-create` creates index in memory for process A
2. Process A exits, index is lost
3. `hnsw-insert` creates new process B with empty index storage
4. Process B cannot find index created by process A

## Workarounds

### Option 1: Use Rust API (Recommended)

For persistent vector search, use the library API directly in your application:

```rust
let graph = SqliteGraph::open("vectors.db")?;

// Create index once
let hnsw = graph.hnsw_index("embeddings", config)?;

// Index persists for lifetime of 'graph' object
// Add vectors as needed
hnsw.insert_vector(&vec1, metadata1)?;
hnsw.insert_vector(&vec2, metadata2)?;

// Search works as expected
let results = hnsw.search(&query, k)?;
```

### Option 2: Single-Session Testing

For testing HNSW within a single process:

```bash
# This works in the same process (not currently supported by CLI)
# You would need to write a small Rust program:

use sqlitegraph::{SqliteGraph, hnsw::*};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let graph = SqliteGraph::open("test.db")?;
    let hnsw = graph.hnsw_index("vectors", config)?;

    // All operations work in same process
    hnsw.insert_vector(&vec1, None)?;
    let results = hnsw.search(&query, 10)?;

    Ok(())
}
```

### Option 3: Future CLI Enhancement

Planned implementation (not yet started):
- Add HNSW persistence tables to database schema
- Auto-load indexes on CLI startup
- Auto-save indexes on modifications
- Estimated effort: 4-6 hours

See `docs/hnsw_persistence_implementation_status_20241223.md` for details.

## Technical Details

### Architecture

**File**: `sqlitegraph/src/graph/core.rs`

```rust
pub struct SqliteGraph {
    pub(crate) hnsw_indexes: RwLock<HashMap<String, HnswIndex>>,
    // HNSW indexes stored in-memory, keyed by name
}
```

**Constructor** (line 100):
```rust
Self {
    // ... other fields ...
    hnsw_indexes: RwLock::new(HashMap::new()),  // Always empty on new instance
}
```

**Database Schema**:
- No HNSW tables in current schema (v3 adds HNSW tables but not yet utilized)
- Vectors are stored in in-memory `VectorStorage` backends
- No persistence layer exists yet

### Current Implementation Status

**Complete**:
- HNSW library (100%)
- CLI command implementations (100%)
- In-memory storage and management (100%)

**Incomplete**:
- Database persistence for HNSW indexes (0%)
- Auto-load on SqliteGraph construction (0%)
- Auto-save on modifications (0%)

## Recommendations

### For Users

**If you need persistent vector search**:
- Use the Rust library API in your application
- The library is production-ready and fully functional
- CLI is useful for testing and development only

**If you only need to test HNSW**:
- Use in-memory database (`:memory:`)
- Complete all operations in one session (requires custom script)
- Or use the Rust API for testing

### For Developers

**Priority**: Low
- Library API is the primary interface
- CLI is convenience tool for development
- Persistence can be added when needed

**Implementation Path**:
1. Add database tables for HNSW metadata
2. Implement vector persistence layer
3. Auto-load indexes on database open
4. Auto-save on modifications
5. Test cross-session functionality

**Estimated Effort**: 4-6 hours for full implementation

## Documentation

- **Implementation Status**: `docs/hnsw_cli_persistence_issue_20241223.md`
- **Persistence Design**: `docs/hnsw_persistence_implementation_status_20241223.md`
- **CLI Status**: `docs/hnsw_cli_status_20241223.md`

## Conclusion

The HNSW vector search feature is **fully functional via the Rust library API**. The CLI provides command-line access for testing but lacks cross-session persistence due to architectural limitations. This is a known limitation that can be addressed in future development if persistent CLI-based HNSW operations become a requirement.

For most use cases, the library API provides the needed functionality without requiring CLI persistence.
