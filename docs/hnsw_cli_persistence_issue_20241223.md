# HNSW CLI Persistence Issue

**Date**: 2025-12-23
**Status**: CRITICAL ARCHITECTURAL LIMITATION DISCOVERED
**Impact**: HNSW CLI commands cannot work across multiple invocations

## Problem Statement

HNSW indexes are stored in-memory within `SqliteGraph` and **DO NOT persist** to the database file.

## Evidence

### Test 1: Create Index
```bash
$ rm -f /tmp/test_hnsw.db && cargo run -p sqlitegraph-cli -- \
  --backend sqlite --db /tmp/test_hnsw.db \
  hnsw-create --dimension 3 --m 16 --ef-construction 200 --distance-metric cosine

{
  "command": "hnsw-create",
  "dimension": 3,
  "distance_metric": "cosine",
  "ef_construction": 200,
  "m": 16,
  "status": "created"
}
```

**Result**: ✅ Index created successfully in memory

### Test 2: Insert Vectors
```bash
$ cargo run -p sqlitegraph-cli -- \
  --backend sqlite --db /tmp/test_hnsw.db \
  hnsw-insert --input /tmp/test_vectors.json

{
  "command": "hnsw-insert",
  "errors": [
    "Vector 0: invalid input: HNSW index 'default' not found",
    "Vector 1: invalid input: HNSW index 'default' not found",
    "Vector 2: invalid input: HNSW index 'default' not found"
  ],
  "index_name": "default",
  "input": "/tmp/test_vectors.json",
  "status": "completed_with_errors",
  "vectors_inserted": 0,
  "vectors_processed": 3
}
```

**Result**: ❌ "HNSW index 'default' not found"

## Root Cause

### Architecture Implementation

**File**: `sqlitegraph/src/graph/core.rs`

**HNSW Index Storage** (lines 29-30):
```rust
pub struct SqliteGraph {
    // ... other fields ...
    /// HNSW vector indexes stored by name
    pub(crate) hnsw_indexes: RwLock<HashMap<String, HnswIndex>>,
}
```

**Constructor** (line 100):
```rust
Self {
    conn,
    // ... other fields ...
    hnsw_indexes: RwLock::new(HashMap::new()),  // ❌ Always empty on new instance
}
```

**CLI Command Lifecycle** (`sqlitegraph-cli/src/main.rs`):

```rust
fn main() {
    // ... parse args ...
    let client = open_backend(&config, auto_migrate).unwrap();  // Creates NEW SqliteGraph
    run_command(&client, &config.command, &config.command_args);  // Runs one command
    // Process exits - SqliteGraph dropped, HashMap lost
}
```

### Why HNSW Indexes Don't Persist

1. **Storage Location**: HNSW indexes are stored in `RwLock<HashMap<String, HnswIndex>>`
2. **Initialization**: Constructor creates empty HashMap: `RwLock::new(HashMap::new())`
3. **No Database Tables**: No schema tables to persist HNSW index metadata or data
4. **No Serialization**: HNSW index structures are never serialized to disk
5. **CLI Architecture**: Each command opens a fresh database connection and creates a new `SqliteGraph` instance

### Why This Matters

The `hnsw-create` command **works but is useless**:
- ✅ Creates `HnswIndex` object
- ✅ Stores in `HashMap<String, HnswIndex>` in memory
- ❌ Does NOT persist to database
- ❌ Subsequent commands create new `SqliteGraph` with empty `HashMap`
- ❌ Index is lost when CLI process exits

## Solutions

### Option 1: Persist HNSW Index to Database (Recommended)

**Required Changes**:

1. **Database Schema**:
   - Add `hnsw_indexes` table to store index metadata (name, config)
   - Add `hnsw_vectors` table to store vector data and metadata
   - Add `hnsw_layers` table to persist HNSW graph structure

2. **SqliteGraph Initialization**:
   - Load existing HNSW indexes on construction
   - Deserialize index structures from database

3. **HNSW Index API**:
   - Make `insert_vector()` persist to database
   - Make HNSW operations write to database
   - Cache frequently-accessed data in-memory

**Pros**:
- True persistence across CLI invocations
- Multi-process safety
- Works with existing database backup/restore

**Cons**:
- Complex implementation
- Performance overhead for serialization
- Schema changes required

### Option 2: In-Memory Only with Long-Lived Process (Workaround)

**Required Changes**:

1. **CLI Architecture**:
   - Add REPL mode where one process handles multiple commands
   - Keep `SqliteGraph` instance alive between commands

2. **Usage**:
   ```bash
   $ sqlitegraph --backend sqlite --db /tmp/test_hnsw.db repl
   > hnsw-create --dimension 3 --m 16 --ef-construction 200 --distance-metric cosine
   > hnsw-insert --input /tmp/test_vectors.json
   > hnsw-stats
   > hnsw-search --input /tmp/test_query.json --k 2
   > exit
   ```

**Pros**:
- Simple implementation
- No database schema changes
- Fast (no serialization overhead)

**Cons**:
- Indexes lost when REPL exits
- Doesn't solve persistence problem
- Different UX from current CLI

### Option 3: Serialize to File (Alternative)

**Required Changes**:

1. **File Format**:
   - Define binary format for HNSW index serialization
   - Store indexes in separate `.hnsw` files alongside `.db` file

2. **API Changes**:
   - `hnsw-create()` writes to `.hnsw` file
   - `SqliteGraph` constructor loads `.hnsw` files
   - Track index metadata in database

**Pros**:
- Clean separation from SQLite database
- Efficient binary format

**Cons**:
- Complex file management
- Need to handle sync between `.db` and `.hnsw` files
- Multiple files to manage

## Current Status

### Working
- ✅ HNSW library implementation is complete
- ✅ `HnswIndex` API works in-memory
- ✅ CLI commands compile and run

### Not Working
- ❌ HNSW indexes do NOT persist between CLI invocations
- ❌ `hnsw-create` creates index but it's lost immediately
- ❌ `hnsw-insert`, `hnsw-search`, `hnsw-stats` fail with "index not found"

### Files Modified

**sqlitegraph/src/graph/core.rs**:
- Added `hnsw_indexes: RwLock<HashMap<String, HnswIndex>>` field
- Updated constructor to initialize empty HashMap

**sqlitegraph/src/hnsw/index.rs**:
- Modified `hnsw_index()` to store indexes in HashMap
- Added `get_hnsw_index_ref()` for read-only access
- Added `get_hnsw_index_mut()` for modifications
- Added `list_hnsw_indexes()` to enumerate indexes

**sqlitegraph-cli/src/main.rs**:
- Implemented `run_hnsw_insert()` - parses JSON, inserts vectors
- Implemented `run_hnsw_search()` - performs KNN search
- Implemented `run_hnsw_stats()` - displays index statistics

## Recommendation

**Implement Option 1 (Database Persistence)**:

This is the only solution that:
1. Works with existing CLI architecture
2. Provides true persistence
3. Integrates cleanly with SQLite database
4. Scales to production use cases

## Testing Commands

These commands demonstrate the issue:

```bash
# Setup
rm -f /tmp/test_hnsw.db
cat > /tmp/vectors.json << 'EOF'
[
  {"vector": [1.0, 0.0, 0.0], "metadata": {"label": "a"}},
  {"vector": [0.0, 1.0, 0.0], "metadata": {"label": "b"}},
  {"vector": [0.0, 0.0, 1.0], "metadata": {"label": "c"}}
]
EOF

# Test 1: Create index - SUCCEEDS but index is lost
cargo run -p sqlitegraph-cli -- \
  --backend sqlite --db /tmp/test_hnsw.db \
  hnsw-create --dimension 3 --m 16 --ef-construction 200 --distance-metric cosine

# Test 2: Insert vectors - FAILS because index doesn't persist
cargo run -p sqlitegraph-cli -- \
  --backend sqlite --db /tmp/test_hnsw.db \
  hnsw-insert --input /tmp/vectors.json

# Test 3: Stats - FAILS because index doesn't exist
cargo run -p sqlitegraph-cli -- \
  --backend sqlite --db /tmp/test_hnsw.db \
  hnsw-stats

# Test 4: Search - FAILS because index doesn't exist
cargo run -p sqlitegraph-cli -- \
  --backend sqlite --db /tmp/test_hnsw.db \
  hnsw-search --input /tmp/query.json --k 2
```

## Conclusion

The HNSW CLI implementation is **architecturally incomplete**. While the code compiles and runs correctly, the HNSW indexes only exist in-memory and are lost between CLI command invocations.

**Current State**: The HNSW CLI commands cannot be used in production without persistence.

**Path Forward**: Implement database persistence for HNSW indexes (Option 1) or document that HNSW is only available for programmatic use via the Rust API, not the CLI.
