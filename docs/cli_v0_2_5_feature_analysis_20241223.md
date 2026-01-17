# CLI v0.2.5 Feature Analysis - What's Missing

## Current CLI Status

### ✅ Features Currently Working

1. **Graph Dump/Load** (from `sqlitegraph::recovery`):
   - `dump-graph --output <path>` - Exports graph to JSON lines format
   - `load-graph --input <path>` - Imports graph from JSON lines format
   - **What it dumps:**
     - Entities (nodes) with metadata: id, kind, name, file_path, data
     - Edges with metadata: id, from_id, to_id, edge_type, data
     - Labels (entity tags)
     - Properties (key-value pairs on entities)
   - **Format:** JSON Lines (newline-delimited JSON) with tagged records
   - **Source:** `sqlitegraph/src/recovery.rs:44-74`

2. **Basic Operations:**
   - `status` - Shows backend type, schema version, node count
   - `list` - Lists all entities
   - `migrate` - Runs database migrations
   - `dsl-parse` - Parses DSL pattern queries

### ❌ Major v0.2.5 Features NOT in CLI

#### 1. **HNSW Embedding Vector System** (CRITICAL - 148KB of code!)

**Location:** `sqlitegraph/src/hnsw/`

**Components:**
- `builder.rs` (14KB) - HNSW index construction
- `index.rs` (24KB) - Main HNSW index interface
- `multilayer.rs` (29KB) - Multi-layer graph structure
- `storage.rs` (22KB) - Vector storage layer
- `config.rs` (15KB) - Configuration options
- `distance_functions.rs` (10KB) - Distance metrics
- `errors.rs` (20KB) - Error handling
- `layer.rs` (17KB) - Layer management
- `neighborhood.rs` (22KB) - Neighborhood search

**What it does:**
- Hierarchical Navigable Small World (HNSW) algorithm for approximate nearest neighbor search
- High-dimensional vector similarity search
- Used for embedding-based semantic search
- Supports multiple distance metrics (Euclidean, Cosine, etc.)

**Why it matters:**
- This is a MAJOR feature (148KB of highly optimized Rust code)
- Critical for vector similarity search, recommendations, semantic queries
- Completely missing from CLI - no commands to build/search HNSW indexes

**What CLI should add:**
```bash
# Commands needed:
hnsw-build --entity-id <id> --dimension <dim> --metric <cosine|euclidean>
hnsw-search --entity-id <id> --k <count>
hnsw-insert --entity-id <id> --vector <values>
hnsw-delete --entity-id <id>
hnsw-stats
```

#### 2. **Native V2 Backend** (IMPORTANT - Latest storage format)

**Location:** `sqlitegraph/src/backend/native/`

**What it is:**
- Next-generation storage backend (not SQLite-based)
- Uses custom binary format for graph data
- Optimized for graph operations and large datasets
- Faster startup with large datasets

**Why it matters:**
- CLI only supports SQLite backend currently
- Missing `--backend native-v2` option
- Can't test/use the latest storage technology

**What CLI should add:**
```bash
# Update open_backend() to support:
--backend native-v2 --database <path>
```

#### 3. **WAL (Write-Ahead Log) Recovery System**

**Location:** Likely in backend implementation, not fully exposed in recovery.rs

**Current state:**
- `recovery.rs` only has dump/load JSON functions
- Missing WAL-specific commands

**Why it matters:**
- WAL mode is critical for data integrity
- Manual checkpoint control for maintenance
- Recovery operations for corrupted databases

**What CLI should add:**
```bash
# WAL commands needed:
wal-checkpoint --database <path> [--mode full|restart|truncate]
wal-recovery --database <path> [--backup]
wal-info --database <path>
wal-truncate --database <path>
```

#### 4. **MVCC Snapshot System**

**Location:** `sqlitegraph/src/mvcc.rs`

**Exported:** `pub use mvcc::{GraphSnapshot, SnapshotState};`

**What it does:**
- Multi-Version Concurrency Control
- Read isolation with snapshot consistency
- Transaction-safe point-in-time reads

**Why it matters:**
- Critical for concurrent access
- Prevents read/write conflicts
- Missing from CLI commands

**What CLI should add:**
```bash
# Snapshot commands needed:
snapshot-create --name <id>
snapshot-list
snapshot-restore --name <id>
snapshot-delete --name <id>
```

#### 5. **Pattern Matching Engine**

**Location:** `sqlitegraph/src/pattern_engine.rs`

**Exported:** `pub use pattern_engine::{PatternTriple, TripleMatch, match_triples};`

**What it does:**
- Triple pattern matching
- Fast path with cache (`match_triples_fast`)
- Graph pattern queries

**Current state:**
- CLI has `dsl-parse` which uses PatternQuery
- But doesn't expose full pattern matching capabilities

**What CLI should add:**
```bash
# Pattern commands needed:
pattern-match --subject <id> --predicate <type> --object <id>
pattern-triples --entity <id> [--depth <n>]
```

#### 6. **Bulk Operations**

**Location:** `sqlitegraph/src/graph_opt.rs`

**Exported:** `pub use graph_opt::{bulk_insert_entities, bulk_insert_edges};`

**What it does:**
- High-performance batch insertions
- Essential for large datasets
- Significantly faster than individual inserts

**Why it matters:**
- CLI doesn't expose bulk operations
- Loading large graphs is slow without batch operations
- Critical for data migration/loading

**What CLI should add:**
```bash
# Bulk commands needed:
bulk-insert --entities <file> [--batch-size <n>]
bulk-insert --edges <file> [--batch-size <n>]
```

#### 7. **Cache Statistics**

**Location:** `sqlitegraph/src/cache.rs`

**Exported:** `pub use cache::CacheStats;`

**What it does:**
- Pattern matching cache
- Performance monitoring
- Cache hit/miss statistics

**What CLI should add:**
```bash
# Cache commands needed:
cache-stats
cache-clear
cache-size
```

#### 8. **Graph Query System**

**Location:** `sqlitegraph/src/query.rs`

**Exported:** `pub use query::GraphQuery;`

**What it does:**
- High-level query interface
- Complex query composition
- Fluent query builder API

**What CLI should add:**
```bash
# Query commands needed:
query --nodes <filter> --edges <filter>
query-explain "<query string>"
```

## Summary: What CLI is Missing

### Critical Missing Features (Priority 1)
1. **HNSW Embedding System** - 148KB of vector search code (MAJOR!)
2. **Native V2 Backend Support** - Latest storage format
3. **WAL Recovery Commands** - Data integrity & recovery
4. **Bulk Operations** - Performance for large datasets

### Important Missing Features (Priority 2)
5. **MVCC Snapshots** - Concurrent access & consistency
6. **Enhanced Pattern Matching** - Beyond basic dsl-parse
7. **Cache Management** - Performance optimization
8. **Query System** - High-level query interface

### Current CLI Limitations

**What dump-graph DOES read:**
- ✅ Entities (nodes) with all metadata
- ✅ Edges with all metadata
- ✅ Labels (tags)
- ✅ Properties (key-value pairs)

**What dump-graph does NOT read:**
- ❌ HNSW vector indexes (embeddings)
- ❌ HNSW neighborhoods & layers
- ❌ WAL log files
- ❌ MVCC snapshots
- ❌ Pattern cache
- ❌ Query statistics

## Binary Format Support

**Question:** "Does the CLI read the graph binary?"

**Answer:** NO - Current CLI uses SQLite backend which stores data in SQLite database format (.db files). The recovery dump/load commands use JSON text format, NOT the native binary format.

**To support native binary format:**
```rust
// Need to add:
use sqlitegraph::NativeGraphBackend;

pub fn new_native(backend: NativeGraphBackend) -> Self {
    Self::Native(backend)
}
```

## Recommendation

The CLI is currently **severely limited** compared to v0.2.5 capabilities:

1. **Missing 148KB of HNSW embedding search** - This is huge!
2. **No Native V2 backend support** - Can't use latest storage
3. **No WAL operations** - Limited recovery capabilities
4. **No bulk operations** - Slow for large datasets
5. **No snapshot management** - Limited concurrency support

The CLI needs **significant expansion** to expose v0.2.5's full capabilities.

## Next Steps

Should I implement any of these missing features? Priority recommendations:

1. **HNSW Commands** (highest value - major feature)
2. **Native V2 Backend** (important for latest storage)
3. **WAL Commands** (important for recovery)
4. **Bulk Operations** (important for performance)
5. **MVCC Snapshots** (important for concurrency)

Let me know which features you want added to the CLI first.
