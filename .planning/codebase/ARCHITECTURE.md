# Architecture

**Analysis Date:** 2026-01-20

## Pattern Overview

**Overall:** Layered architecture with pluggable backends

**Key Characteristics:**
- Unified `GraphBackend` trait abstracts storage implementation
- Two independent backend implementations (SQLite, Native V2)
- MVCC-lite snapshot isolation for concurrent reads
- LRU-K caching layer for traversal optimization
- Feature-based compilation for backend selection

## Layers

**Public API Layer:**
- Purpose: User-facing graph operations and configuration
- Location: `sqlitegraph/src/lib.rs`, `sqlitegraph/src/graph/`
- Contains: `SqliteGraph`, `GraphEntity`, `GraphEdge`, `open_graph()`, `GraphQuery`
- Depends on: Backend layer, cache layer, storage layer
- Used by: Application code, CLI

**Backend Abstraction Layer:**
- Purpose: Unified interface for storage implementations
- Location: `sqlitegraph/src/backend.rs`
- Contains: `GraphBackend` trait, `BackendKind` enum, `NodeSpec`, `EdgeSpec`
- Depends on: Storage implementations
- Used by: Public API layer, algorithms

**SQLite Backend Layer:**
- Purpose: ACID-compliant graph storage using SQLite
- Location: `sqlitegraph/src/backend/sqlite/`
- Contains: `SqliteGraphBackend`, SQLite schema, prepared statements
- Depends on: rusqlite library
- Used by: Applications needing SQL compatibility, ACID guarantees

**Native Backend Layer:**
- Purpose: High-performance binary file format storage
- Location: `sqlitegraph/src/backend/native/`
- Contains: `NativeGraphBackend`, graph file I/O, edge clustering, WAL
- Depends on: memmap2, binrw, bytemuck
- Used by: Applications needing performance, no external dependencies

**Native V2 Layer (Clustered Edge Kernel):**
- Purpose: Optimized edge storage with spatial locality
- Location: `sqlitegraph/src/backend/native/v2/`
- Contains: `EdgeCluster`, `NodeRecordV2`, `StringTable`, `V2WALManager`
- Depends on: Native backend primitives
- Used by: Native backend when V2 format is selected

**Algorithm Layer:**
- Purpose: Graph analysis algorithms
- Location: `sqlitegraph/src/algo.rs`
- Contains: PageRank, Betweenness, Louvain, Label Propagation
- Depends on: Backend abstraction layer
- Used by: Applications for graph analytics

**Vector Search Layer:**
- Purpose: Approximate nearest neighbor search
- Location: `sqlitegraph/src/hnsw/`
- Contains: `HnswIndex`, `HnswConfig`, distance metrics, multilayer graph
- Depends on: Backend for vector persistence
- Used by: Applications needing similarity search

**Cache Layer:**
- Purpose: LRU-K adjacency caching for traversal optimization
- Location: `sqlitegraph/src/cache.rs`
- Contains: `AdjacencyCache`, `CacheStats`
- Depends on: ahash, parking_lot
- Used by: Graph core, traversal operations

**MVCC Layer:**
- Purpose: Snapshot isolation for concurrent reads
- Location: `sqlitegraph/src/mvcc.rs`
- Contains: `SnapshotManager`, `GraphSnapshot`, `SnapshotState`
- Depends on: arc-swap, rusqlite
- Used by: Graph core for thread-safe snapshots

**Pattern Engine Layer:**
- Purpose: Triple pattern matching
- Location: `sqlitegraph/src/pattern_engine/`, `sqlitegraph/src/pattern_engine_cache/`
- Contains: `PatternTriple`, `match_triples()`, fast path detection
- Depends on: Backend abstraction layer
- Used by: Query layer, pattern-based searches

## Data Flow

**Graph Creation Flow:**

1. User calls `open_graph(path, &config)` with `GraphConfig`
2. Factory function checks `config.backend` (SQLite or Native)
3. Backend-specific initialization:
   - SQLite: Opens SQLite connection, runs migrations
   - Native: Opens/mmap graph file, validates header
4. `SqliteGraph` wraps backend with cache, metrics, snapshots
5. HNSW indexes loaded from storage if present

**Insert Operation Flow:**

1. User calls `graph.insert_node(spec)` or `graph.insert_edge(...)`
2. Operation validates input (labels, properties)
3. Cache invalidates affected adjacency lists
4. Backend executes storage operation:
   - SQLite: INSERT with transaction
   - Native: Append to WAL, update in-memory structures
5. Metrics updated (statement tracker, counters)

**Query Operation Flow:**

1. User calls `graph.fetch_outgoing(node_id)` or `graph.query().neighbors(...)`
2. Cache checked first (`AdjacencyCache::get()`)
3. On cache hit: Return cached neighbors instantly
4. On cache miss:
   - Backend query executes (SQLite SELECT or Native file read)
   - Results stored in cache
   - Results returned to user

**Algorithm Execution Flow:**

1. User calls `pagerank(&graph)` or similar algorithm
2. Algorithm uses `GraphBackend` trait for operations
3. Repeated neighbor queries benefit from cache warming
4. Results computed and returned as `Vec<(NodeId, Score)>`

**Snapshot Creation Flow:**

1. User calls `graph.snapshot()?`
2. `SnapshotManager::acquire_snapshot()` atomically loads current state
3. New read-only SQLite connection opened
4. Immutable `SnapshotState` with cloned adjacency maps returned
5. Snapshot can be safely sent to other threads

**State Management:**
- SQLite backend: Single `Connection` with `RefCell` for interior mutability
- Native backend: Memory-mapped file with `Arc` shared references
- Cache: `RwLock<AHashMap>` protected by parking_lot
- Snapshots: `Arc<SnapshotState>` for immutable sharing

## Key Abstractions

**GraphBackend Trait:**
- Purpose: Unified interface for storage backends
- Examples: `sqlitegraph/src/backend.rs:29-97`
- Pattern: Trait-based polymorphism with reference implementation

**BackendKind Enum:**
- Purpose: Runtime backend selection
- Examples: `SQLite`, `Native`
- Pattern: Feature-gated enum with compile-time validation

**NodeId / NativeNodeId:**
- Purpose: Type-safe node identifiers
- Examples: `type NodeId = i64` in backend, `pub struct NativeNodeId(pub i64)`
- Pattern: Newtype wrappers for type safety

**GraphSnapshot:**
- Purpose: Immutable point-in-time graph view
- Examples: `mvcc.rs:257-322`
- Pattern: Arc-wrapped immutable state with separate read-only connection

**AdjacencyCache:**
- Purpose: LRU-K caching for neighbor lists
- Examples: `cache.rs:148-202`
- Pattern: Cache-aside with atomic hit/miss counters

**HnswIndex:**
- Purpose: Hierarchical navigable small world vector search
- Examples: `hnsw/mod.rs`
- Pattern: Pluggable storage backend (in-memory vs SQLite)

## Entry Points

**open_graph():**
- Location: `sqlitegraph/src/config/factory.rs`
- Triggers: Database file opening, backend selection
- Responsibilities: Path validation, backend factory dispatch, schema initialization

**SqliteGraph::open():**
- Location: `sqlitegraph/src/graph/core.rs:51-56`
- Triggers: SQLite connection creation
- Responsibilities: Connection setup, WAL mode, cache initialization

**SqliteGraph::open_in_memory():**
- Location: `sqlitegraph/src/graph/core.rs:65-70`
- Triggers: In-memory database creation
- Responsibilities: Fast in-memory graph setup, no file I/O

**NativeGraphBackend::new():**
- Location: `sqlitegraph/src/backend/native/graph_backend.rs`
- Triggers: Native file opening/mmap
- Responsibilities: File validation, header parsing, memory mapping

**CLI main():**
- Location: `sqlitegraph-cli/src/main.rs`
- Triggers: Command-line argument parsing
- Responsibilities: Command dispatch, backend selection, error reporting

## Error Handling

**Strategy:** Result-based with thiserror

**Patterns:**
- `SqliteGraphError` enum with variant-specific context
- `NativeBackendError` for native-specific failures
- `HnswError` for vector search failures
- Conversion between error types via `From` trait
- All public API methods return `Result<T, Error>`

**Error Categories:**
- Connection errors: Database/file open failures
- Query errors: Invalid SQL, I/O errors
- Validation errors: Invalid input (empty labels, negative IDs)
- Entity not found: Missing node/edge lookups
- Backend-specific: WAL corruption, header validation

## Cross-Cutting Concerns

**Logging:** log crate with feature-gated debug output

**Validation:**
- Input validation at API boundaries (labels non-empty, IDs positive)
- Schema validation on database open
- Header validation for native files

**Authentication:** Not applicable (embedded database)

**Metrics:**
- `GraphMetrics` for operation counting
- `StatementTracker` for SQL query profiling
- `CacheStats` for cache hit/miss ratios

**Progress Tracking:**
- `ProgressCallback` trait for algorithm progress
- `ConsoleProgress` and `NoProgress` implementations
- Callback invoked during long-running algorithms

**Thread Safety:**
- `SqliteGraph` NOT Sync (uses RefCell)
- `GraphSnapshot` IS Sync (immutable + Arc)
- `AdjacencyCache` uses RwLock for concurrent access
- Snapshots enable concurrent reads from single writer

---

*Architecture analysis: 2026-01-20*
