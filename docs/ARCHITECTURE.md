# SQLiteGraph Architecture

**Last Updated:** 2026-06-29
**Version:** v3.4.6

This document describes the architecture of SQLiteGraph from a developer's perspective. For user-facing documentation, see [README.md](../README.md) and [MANUAL.md](../MANUAL.md).

---

## Table of Contents

1. [High-Level Overview](#high-level-overview)
2. [Directory Structure](#directory-structure)
3. [Core Components](#core-components)
4. [Backend Architecture](#backend-architecture)
5. [Data Flow](#data-flow)
6. [Key Design Decisions](#key-design-decisions)
7. [Module Reference](#module-reference)

---

## High-Level Overview

SQLiteGraph is an embedded graph database with **two** storage backends:

```
┌─────────────────────────────────────────────────────────────────────┐
│                        User Application                             │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│                    GraphBackend Trait (Unified API)                 │
│     - insert_node, insert_edge, neighbors, subscribe, etc.          │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
           ┌────────────────────┴────────────────────┐
           │                                         │
           ▼                                         ▼
┌──────────────────┐                    ┌──────────────────┐
│  SQLite Backend  │                    │   Native V3      │
│  (Stable)        │                    │ (Experimental)   │
├──────────────────┤                    ├──────────────────┤
│ - SQL storage    │                    │ - Binary format  │
│ - ACID via SQL   │                    │ - B+Tree index   │
│ - Debuggable     │                    │ - Packed edge    │
│                  │                    │   pages + warm   │
│                  │                    │   cache path     │
└──────────────────┘                    └──────────────────┘
```

### Backend Status

| Backend | Status | Use Case |
|---------|--------|----------|
| **SQLite** | ✅ Stable | Debuggable, familiar SQL ecosystem |
| **Native V3** | Experimental | Graph-oriented storage, packed edge pages, KV, pub/sub |

### Key Architectural Principles

1. **Unified Backend Trait**: Same API works with all backends
2. **Lazy Initialization**: V3 backend doesn't allocate until features are used
3. **Honest Engineering**: Document limitations, deprecate when better alternatives exist
4. **MVCC Isolation**: Snapshot-based reads without blocking writers
5. **Pluggable Components**: Algorithms, storage, and indexing are decoupled
6. **Packed Adjacency Path**: Native V3 stores many small `(src, dir)` clusters in shared edge pages and bulk-warms them through the edge-store cache

---

## Directory Structure

```
sqlitegraph/
├── Cargo.toml                 # Main library manifest
├── src/
│   ├── lib.rs                 # Public API exports
│   ├── error.rs               # Error types
│   ├── backend/               # Storage abstraction
│   │   ├── mod.rs             # GraphBackend trait + generic types
│   │   ├── sqlite/            # SQLite backend (stable)
│   │   │   ├── impl_.rs       # SqliteGraphBackend
│   │   │   └── pubsub_tests.rs
│   │   └── native/            # Native backends
│   │       ├── v3/            # V3 backend (experimental)
│   │       │   ├── backend.rs # V3Backend implementation
│   │       │   ├── btree/     # B+Tree index
│   │       │   ├── kv_store/  # Lazy KV storage
│   │       │   ├── pubsub/    # Publisher implementation
│   │       │   └── wal/       # Write-ahead logging
│   ├── algo/                  # Graph algorithms (backend-agnostic)
│   │   ├── backend/           # Generic algorithms for &dyn GraphBackend
│   │   └── ...                # 35+ algorithms
│   ├── hnsw/                  # Vector similarity search
│   │   ├── storage.rs         # VectorStorage trait + SQLite implementation
│   │   ├── v3_storage.rs      # V3 vector storage
│   │   └── ...
│   └── ...
└── docs/
```

---

## Core Components

### 1. GraphBackend Trait

The `GraphBackend` trait is the core abstraction. All backends implement this trait.

**Location:** `src/backend/mod.rs`

```rust
pub trait GraphBackend: Send + Sync {
    // Core graph operations
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError>;
    fn insert_edge(&self, edge: EdgeSpec) -> Result<i64, SqliteGraphError>;
    fn neighbors(&self, snapshot_id: SnapshotId, node: i64, query: NeighborQuery) 
        -> Result<Vec<i64>, SqliteGraphError>;
    
    // Key-Value operations (optional - backends return error if unsupported)
    fn kv_get(&self, snapshot_id: SnapshotId, key: &[u8]) -> Result<Option<KvValue>, SqliteGraphError>;
    fn kv_set(&self, key: Vec<u8>, value: KvValue, ttl_seconds: Option<u64>) -> Result<(), SqliteGraphError>;
    
    // Pub/Sub (works on all backends now)
    fn subscribe(&self, filter: SubscriptionFilter) 
        -> Result<(u64, Receiver<PubSubEvent>), SqliteGraphError>;
    fn unsubscribe(&self, subscriber_id: u64) -> Result<bool, SqliteGraphError>;
}
```

**Generic Pub/Sub Types:** The trait includes generic `PubSubEvent` and `SubscriptionFilter` types that work across all backends.

### 2. Backend Algorithms

**Location:** `src/algo/backend/`

Generic algorithm implementations that work with any `GraphBackend`:

```rust
// centrality.rs
pub fn pagerank(graph: &dyn GraphBackend, damping: f64, iterations: usize) 
    -> Result<Vec<(i64, f64)>, SqliteGraphError>;

pub fn betweenness_centrality(graph: &dyn GraphBackend) 
    -> Result<Vec<(i64, f64)>, SqliteGraphError>;

// graph_ops.rs  
pub fn strongly_connected_components(graph: &dyn GraphBackend) 
    -> Result<SccResult, SqliteGraphError>;

pub fn shortest_path(graph: &dyn GraphBackend, start: i64, end: i64) 
    -> Result<Option<Vec<i64>>, SqliteGraphError>;
```

All 35+ algorithms are backend-agnostic.

#### Parallel Algorithms (v2.1.0+)

**Location:** `src/backend/native/v3/algorithm/parallel_bfs.rs`

Breadth-first search with level-wise parallelism using Rayon:

```rust
pub fn parallel_bfs_traversal(
    graph: &SqliteGraph,
    start_node: i64,
) -> Result<BFSResult, SqliteGraphError> {
    // Sequential fallback for small graphs
    if graph.header().node_count < 1000 {
        return bfs_traversal(graph, start_node);
    }

    // Parallel BFS
    let mut visited = Arc::new(Mutex::new(HashSet::new()));
    let mut current_level = vec![start_node];

    while !current_level.is_empty() {
        // Mark current level as visited
        visited.lock().extend(current_level.iter().cloned());

        // Fetch neighbors in parallel
        let next_level: Vec<i64> = current_level
            .par_iter()  // Rayon parallel iterator
            .flat_map(|&node| {
                graph.neighbors(SnapshotId::current(), node, NeighborQuery::Outgoing)
                    .unwrap_or_default()
            })
            .filter(|&neighbor| {
                !visited.lock().contains(&neighbor)
            })
            .collect();

        current_level = next_level;
    }

    Ok(BFSResult { ... })
}
```

**Performance Impact:**
- **⚠️ WARNING (v2.1.0 and earlier):** Had thread-safety bugs and was slower than sequential
- **Fixed in v2.1.1:** Chunked processing with zero shared state
- **Current Performance:** 1.0-1.17× speedup on small graphs (100-500 nodes)
- **Thread safety:** Thread-local collections, no locks during parallel phase
- **Best for:** Graphs with wide levels (high branching factor)

**Use Cases:**
- Social network analysis (friend discovery)
- Recommendation systems (collaborative filtering)
- Graph traversal (reachability queries)

### 3. Vector Storage Abstraction

**Location:** `src/hnsw/storage.rs`

```rust
pub trait VectorStorage {
    fn store_vector(&mut self, vector: &[f32], metadata: Option<Value>) -> Result<u64, HnswError>;
    fn get_vector(&self, id: u64) -> Result<Option<Vec<f32>>, HnswError>;
    fn delete_vector(&mut self, id: u64) -> Result<(), HnswError>;
    fn vector_count(&self) -> Result<usize, HnswError>;
}
```

**Implementations:**
- `SQLiteVectorStorage` - SQL table storage
- `V3VectorStorage` - KV store storage (new)
- `InMemoryVectorStorage` - RAM only

### 4. Temporal Topology (MVCC Version Chain)

**Location:** `src/temporal.rs` (+ version-chain storage in `src/mvcc.rs`)

The temporal module analyses how graph topology evolves across a sequence of
checkpointed versions, producing a research-grade picture of topological
evolution for code graphs (call-graph circular-dependency lifecycle, component
merges/splits, structural-redundancy growth). It is the discrete analogue of
persistent homology, with **version number as the filtration parameter**.

#### Version Chain Storage

Retained versions live in `SnapshotManager` as a **bounded `VecDeque`**
(`history`), capped at `max_history` (default 64). The chain is empty by default
— zero overhead until `checkpoint()` is called. When full, the oldest version is
evicted FIFO. Each entry is a `VersionedSnapshot { version: u64, created_at:
SystemTime, state: Arc<SnapshotState> }`. The live state is held separately in an
`ArcSwap` for lock-free current reads; the history deque is guarded by a
short-lived `Mutex` (writes are rare relative to `as_of` reads, which do a
read-lock + binary search). `SqliteGraph` exposes this as `checkpoint()` /
`snapshot_as_of(v)` / `snapshot_versions()`.

#### Lineage Matching (Jaccard Overlap)

Across adjacent versions, components are linked by **greedy Jaccard-overlap
matching**. For every pair of SCCs at version *i* and *i*+1 the similarity
`|intersection| / |union|` is computed; pairs are then assigned
highest-overlap-first, with each SCC matched to at most one predecessor or
successor. A component is *born* when it shares no members with any predecessor,
and *dies* when no successor shares its members. This yields exact
`(birth_version, death_version)` bars — there is no component-count-delta
approximation (the old `compute_temporal_barcode` used a LIFO count-delta policy
and is deprecated in favour of this identity-based matching).

#### The Three Topology Layers

| Layer | Function | Invariant |
|-------|----------|-----------|
| **Exact H₀** — connected-component lifecycle | `scc_lineage_barcode` | Tracks *all* SCCs by membership identity; the discrete analogue of 0-dimensional persistent homology |
| **β₁ trajectory** — cycle-rank | `cycle_rank_snapshot` | Cyclomatic number β₁ = E − V + W (independent undirected cycles) — a scalar per version, not a barcode |
| **Circular-dependency barcode** | `cycle_scc_barcode` | Tracks *non-trivial* SCCs (size ≥ 2); each such SCC contains ≥ 1 directed cycle, so for a call graph this is the circular-dependency lifecycle |

`temporal_persistence_sweep` is the scalar companion: one
`TemporalPersistencePoint` per version carrying β₀, β₁, largest-SCC size, and
non-trivial SCC counts.

**What it is NOT:** none of these is classical multi-dimensional H₁ persistent
homology with per-cycle-generator tracking. β₁ counts *undirected* cycles;
`cycle_scc_barcode` detects cycle *presence* via SCCs and tracks
cycle-containing components across versions — the practically useful signal for
code-graph evolution, but a different invariant than true H₁ (which would
require a per-version cycle basis plus generator matching across versions).

---

## Backend Architecture

### SQLite Backend

**Location:** `src/backend/sqlite/`

**Status:** Stable, mature, debuggable

| Aspect | Implementation |
|--------|----------------|
| **Storage** | SQLite database file |
| **Schema** | `entities`, `edges`, `kv_store`, `hnsw_vectors` tables |
| **Transactions** | Full ACID via SQLite |
| **Concurrency** | Database-level locks |
| **Pub/Sub** | In-memory publisher with event emission |

**Key Features:**
- SQL-accessible for debugging
- Mature, well-tested
- Creates tables on-demand
- No node limits

### Native V3 Backend (Experimental)

**Location:** `src/backend/native/v3/`

**Status:** Experimental graph-oriented backend with packed edge pages, warmable adjacency cache, KV, and pub/sub

| Aspect | Implementation |
|--------|----------------|
| **Storage** | Binary file (.graph) |
| **Index** | B+Tree for O(log n) node lookups |
| **Capacity** | Storage-limited |
| **KV Store** | Lazy-initialized in-memory HashMap |
| **Pub/Sub** | Lazy-initialized Publisher |
| **WAL** | Optional for durability |
| **Weighted adjacency** | Descending-weight reads + bulk warm API |

#### File Format

```
db.graph
├── Header (1KB)
│   ├── Magic: "SQLGV3"
│   ├── Version: 3
│   ├── Root index page
│   └── Node count
├── B+Tree Index
│   ├── Internal nodes (keys → page_ids)
│   └── Leaf nodes (node_id → page_id)
├── Node Pages
│   └── Variable-size node records
├── Edge Clusters
│   └── Edge adjacency data
└── Free space bitmap
```

#### Lazy Initialization

V3 uses `Option<T>` for optional features:

```rust
pub struct V3Backend {
    // Core (always present)
    btree: RwLock<BTreeManager>,
    node_store: RwLock<NodeStore>,
    edge_store: RwLock<V3EdgeStore>,
    
    // Optional (lazy initialized)
    kv_store: RwLock<Option<KvStore>>,      // Created on first kv_get/set
    publisher: RwLock<Option<Publisher>>,   // Created on first subscribe
}
```

**Benefits:**
- Zero overhead if KV/PubSub not used
- Memory efficient for simple graph workloads
- Full features available when needed

#### LRU Caching (v2.1.0+)

Node record lookups are cached using an LRU (Least Recently Used) cache:

**Location:** `src/backend/native/v3/node/cache.rs`

```rust
pub struct NodeCache {
    cache: Mutex<lru::LruCache<i64, Arc<NodePage>>>,
    capacity: usize,
}

impl NodeCache {
    pub fn new(capacity: usize) -> Self {
        NodeCache {
            cache: Mutex::new(LruCache::new(capacity)),
            capacity,
        }
    }

    pub fn get(&self, node_id: i64) -> Option<Arc<NodePage>> {
        self.cache.lock().get(&node_id).cloned()
    }

    pub fn insert(&self, node_id: i64, page: Arc<NodePage>) {
        self.cache.lock().put(node_id, page);
    }

    pub fn invalidate(&self, node_id: i64) {
        self.cache.lock().pop(&node_id);
    }

    pub fn clear(&self) {
        self.cache.lock().clear();
    }
}
```

**Integration with V3Backend:**

```rust
impl V3Backend {
    fn get_node_internal(&self, node_id: i64) -> Result<NodeRecord, SqliteGraphError> {
        // Try cache first
        if let Some(cached) = self.node_cache.get(node_id) {
            return Ok(cached.records[...].clone());
        }

        // Cache miss - load from storage
        let page = self.node_store.read().load_page(node_id)?;
        let record = page.records[...].clone();

        // Insert into cache
        self.node_cache.insert(node_id, page);

        Ok(record)
    }
}
```

**Performance Impact:**
- **Point lookups:** 114× faster (cached access vs disk I/O)
- **Cache hit rate:** ~85-95% for traversal-heavy workloads
- **Memory overhead:** ~100KB per 1000 cached nodes (configurable)
- **Thread safety:** Mutex-protected for concurrent access

**Cache Invalidation:**
- Node mutations (insert/update) invalidate affected entries
- Manual cache clearing via `clear()`
- Automatic eviction when capacity exceeded

#### Adaptive Page Sizing (v2.1.0+)

Storage page size adapts based on media type (SSD vs HDD):

**Location:** `src/backend/native/v3/storage/adaptive_page.rs`

**Media Detection:**
```rust
pub fn detect_media_type(db_path: &Path) -> MediaDetectorResult {
    // Linux: Check /sys/block for rotational flag
    // SSDs: rotational=0 → 4KB pages (smaller random reads)
    // HDDs: rotational=1 → 16KB pages (reduce seek overhead)
}
```

**Performance Impact (VERIFIED 2026-04-23):**
- **SSD workloads:** 15-25% improvement (4KB pages)
- **HDD workloads:** 15-25% improvement (16KB pages)
- **Read operations:** Up to 58% faster with larger pages
- **Write operations:** 4KB pages 25% better on SSD
- **Detection overhead:** <0.001ms (negligible)
- **Status:** ✅ Validated with benchmarks

#### Delta Encoding (v2.1.0+)

Edge IDs are compressed using delta encoding:

**Location:** `src/backend/native/v3/compression/edge_delta.rs`

**Algorithm:**
1. Store differences between sequential edge IDs
2. Use zigzag encoding for signed integers
3. Varint encoding for compact representation

**Compression Ratio:** ~87.5% space savings (8:1) for sequential edge IDs

**Example:**
```
Original: [100, 101, 102, 500, 501]
Deltas:  [100, 1, 1, 398, 1]
Encoded: 0x64 0x02 0x02 0x9D 0x05 0x02
```

#### Pub/Sub Implementation

```rust
impl GraphBackend for V3Backend {
    fn subscribe(&self, filter: SubscriptionFilter) -> Result<...> {
        // Lazy initialize publisher
        if self.publisher.read().is_none() {
            *self.publisher.write() = Some(Publisher::new());
        }
        // ... subscribe logic
    }
}
```

## Key Design Decisions

### 1. Why Lazy Initialization in V3?

**Problem:** Native backends always allocated KV store and Publisher even when unused.

**Solution:** V3 uses `Option<T>` with lazy initialization.

**Trade-offs:**
- ✅ Zero memory overhead for unused features
- ✅ Slightly more complex code (Option handling)
- ✅ First access has initialization cost

### 2. Why Generic Pub/Sub Types?

**Problem:** Pub/Sub types were tied to native feature, breaking compilation without it.

**Solution:** Moved generic `PubSubEvent` and `SubscriptionFilter` to `backend/mod.rs`.

**Result:** All backends can implement Pub/Sub with same types.

### 3. Why Backend-Agnostic Algorithms?

**Problem:** Old algorithms used `&SqliteGraph`, requiring SQLite backend.

**Solution:** New algorithms use `&dyn GraphBackend`.

**Result:** Same algorithms work with SQLite and V3 backends.

---

## Module Reference

### Backend Modules

| Module | Path | Purpose |
|--------|------|---------|
| `GraphBackend` | `src/backend/mod.rs` | Core trait definition |
| `SqliteGraphBackend` | `src/backend/sqlite/impl_.rs` | SQLite implementation |
| `V3Backend` | `src/backend/native/v3/backend.rs` | V3 implementation |
| `PubSubEvent` | `src/backend/mod.rs` | Generic event types |

### Algorithm Modules

| Module | Path | Algorithms |
|--------|------|------------|
| `centrality` | `src/algo/backend/centrality.rs` | PageRank, Betweenness |
| `graph_ops` | `src/algo/backend/graph_ops.rs` | SCC, Shortest Path, Topological Sort |
| `traversal` | `src/algo/backend/traversal.rs` | BFS, DFS, k-hop |

### HNSW Modules

| Module | Path | Purpose |
|--------|------|---------|
| `SQLiteVectorStorage` | `src/hnsw/storage.rs` | SQL-backed vector storage |
| `V3VectorStorage` | `src/hnsw/v3_storage.rs` | KV-backed vector storage |

---

## Testing Strategy

Each backend has comprehensive tests:

```bash
# SQLite backend
cargo test --lib backend::sqlite

# V3 backend  
cargo test --features native-v3 --lib backend::native::v3

# Backend-agnostic algorithms
cargo test --features native-v3 --lib algo::backend

# HNSW storage
cargo test --features native-v3 --lib hnsw::v3_storage
```

**Test Philosophy:**
- TDD for all new features
- No stubs or mocks - test real implementations
- Integration tests for backend switching

---

## Future Directions

1. **Performance benchmarks** - Comprehensive V3 vs SQLite comparison
2. **Distributed features** - Optional clustering layer
3. **Additional algorithms** - More graph algorithms from research literature

---

**Note:** This architecture document is honest about limitations and deprecation. We don't hide technical debt or pretend all backends are equal. Choose based on your needs.
