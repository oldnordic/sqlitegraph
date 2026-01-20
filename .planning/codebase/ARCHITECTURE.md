# Architecture

**Analysis Date:** 2026-01-20

## Pattern Overview

**Overall:** Layered Backend-Agnostic Graph Database with Dual Storage Engines

**Key Characteristics:**
- Unified `GraphBackend` trait abstracts storage layer from graph operations
- Dual backend support: SQLite (mature, ACID) and Native V2 (high-performance, custom binary)
- MVCC-lite snapshot system for concurrent read isolation
- LRU-K adjacency cache optimized for graph traversal workloads
- Pluggable algorithm layer (PageRank, Betweenness, Louvain, Label Propagation)
- HNSW vector search with pluggable storage backends

## Layers

**API Layer (`lib.rs` exports):**
- Purpose: Public API surface, re-exports stable types and functions
- Location: `src/lib.rs`
- Contains: Re-exports of core types, configuration, algorithms, utilities
- Depends on: All modules (re-exports their public types)
- Used by: External consumers, CLI tools

**Configuration Layer (`config/`):**
- Purpose: Backend selection and runtime configuration
- Location: `src/config/mod.rs`
- Contains: `GraphConfig`, `BackendKind`, `SqliteConfig`, `NativeConfig`, `open_graph()`
- Depends on: Backend implementations
- Used by: API layer, application code

**Graph Core Layer (`graph/`):**
- Purpose: Main graph database implementation with entity/edge storage
- Location: `src/graph/mod.rs`, `src/graph/core.rs`
- Contains: `SqliteGraph`, `GraphEntity`, `GraphEdge`, CRUD operations
- Depends on: Backend layer, cache, MVCC, HNSW, schema
- Used by: All graph operations, algorithms, queries

**Backend Abstraction Layer (`backend.rs`, `backend/`):**
- Purpose: Unified interface for different storage engines
- Location: `src/backend.rs`, `src/backend/sqlite/`, `src/backend/native/`
- Contains: `GraphBackend` trait, `SqliteGraphBackend`, `NativeGraphBackend`
- Depends on: Storage-specific implementations (rusqlite, custom file format)
- Used by: Graph core, algorithms

**Algorithm Layer (`algo.rs`):**
- Purpose: Graph analysis algorithms (centrality, community detection)
- Location: `src/algo.rs`
- Contains: `pagerank`, `betweenness_centrality`, `louvain_communities`, `label_propagation`
- Depends on: Graph core (works with any backend via `GraphBackend`)
- Used by: Application code for graph analysis

**Query Layer (`query.rs`, `pattern_engine/`):**
- Purpose: High-level query interface and pattern matching
- Location: `src/query.rs`, `src/pattern_engine/mod.rs`
- Contains: `GraphQuery`, `PatternTriple`, `match_triples`
- Depends on: Graph core, pattern engine cache
- Used by: Application code for complex queries

**HNSW Layer (`hnsw/`):**
- Purpose: Approximate nearest neighbor vector search
- Location: `src/hnsw/mod.rs`
- Contains: `HnswIndex`, `HnswConfig`, distance metrics
- Depends on: Storage backends (SQLite or in-memory)
- Used by: Vector similarity applications

**Utilities Layer:**
- `cache.rs`: LRU-K adjacency cache for traversal optimization
- `mvcc.rs`: Snapshot isolation system (`SnapshotManager`, `GraphSnapshot`)
- `progress.rs`: Progress tracking callbacks (`ProgressCallback`, `ConsoleProgress`)
- `recovery.rs`: Backup/restore utilities
- `introspection.rs`: Debugging/observability APIs

**Native V2 Backend Sub-Layers:**
- `backend/native/v2/wal/`: Write-Ahead Log with checkpoint/recovery
- `backend/native/v2/edge_cluster/`: Compact edge records with clustering
- `backend/native/v2/string_table/`: String deduplication storage
- `backend/native/v2/free_space/`: Free space management for reuse
- `backend/native/v2/snapshot/`: Atomic snapshot operations
- `backend/native/v2/export/`, `backend/native/v2/import/`: Data migration

## Data Flow

**Graph Write Operations (Insert):**

1. Application calls `graph.insert_node(spec)` or `graph.insert_edge(...)`
2. Graph core validates input and creates entity/edge records
3. Backend's `insert_node()` / `insert_edge()` is invoked
4. Backend writes to storage (SQLite tables or native file format)
5. Adjacency cache is invalidated for affected nodes
6. MVCC snapshot manager is notified for next snapshot update

**Graph Read Operations (Neighbor Query):**

1. Application calls `graph.fetch_outgoing(node_id)`
2. Graph core checks adjacency cache (LRU-K)
3. On cache hit: Return cached neighbors immediately (~100ns)
4. On cache miss:
   - Query backend via `neighbors(node_id, query)`
   - Backend retrieves from storage (SQLite or native file)
   - Results cached for future access
   - Return neighbors
5. For read-only isolation: `graph.snapshot()` creates `GraphSnapshot`

**Algorithm Execution (PageRank):**

1. Application calls `pagerank(&graph, damping, iterations)`
2. Algorithm iterates over all nodes via `graph.all_entity_ids()`
3. Each iteration:
   - Fetch outgoing neighbors for each node (cached)
   - Compute rank contributions
   - Update scores
4. Progress callbacks report iteration status
5. Return sorted `(node_id, score)` pairs

**Pattern Matching:**

1. Application creates `PatternTriple { subject, predicate, object }`
2. `match_triples(&graph, pattern)` is invoked
3. Pattern engine cache checks for fast-path execution
4. On fast-path miss: Full pattern matching via backend
5. Results filtered by property constraints
6. Return matching triples

**Native V2 WAL Flow:**

1. Write operation begins
2. `V2WALManager` writes record to WAL file
3. Operation applied to in-memory structures
4. On checkpoint: `V2GraphWALIntegrator` flushes to graph file
5. Recovery: `V2WALRecovery` replays uncommitted records

**State Management:**
- Graph state: SQLite database or native binary file
- Cache state: In-memory `AdjacencyCache` with LRU-K eviction
- Snapshot state: `Arc<SnapshotState>` with cloned adjacency HashMaps
- HNSW state: In-memory index + optional SQLite persistence

## Key Abstractions

**GraphBackend Trait:**
- Purpose: Unified interface for storage backends
- Examples: `SqliteGraphBackend`, `NativeGraphBackend`
- Pattern: Strategy pattern - different implementations for same operations
- Methods: `insert_node`, `get_node`, `insert_edge`, `neighbors`, `bfs`, `shortest_path`, `checkpoint`, `snapshot_export`, `snapshot_import`

**NodeId / Entity Abstraction:**
- Purpose: Unique identifier for graph nodes (64-bit signed integer)
- Type: `i64` (NodeId)
- Pattern: Value object - used throughout for node references

**GraphEntity / GraphEdge:**
- Purpose: Rich representation of nodes and relationships
- Examples: `GraphEntity { id, kind, name, file_path, data }`
- Pattern: Active Record pattern - entities with JSON-serialized properties

**AdjacencyCache:**
- Purpose: LRU-K caching for traversal optimization
- Pattern: Cache-Aside pattern with automatic invalidation
- Key insight: K=2 distinguishes traversal patterns from random access

**SnapshotManager:**
- Purpose: MVCC-lite read isolation using ArcSwap for atomic updates
- Pattern: Copy-on-write via immutable `SnapshotState`
- Memory ordering: Acquire/Release for happens-before guarantees

**HnswIndex:**
- Purpose: Hierarchical Navigable Small World for ANN search
- Pattern: Pluggable storage via `VectorStorage` trait
- Layer structure: Log scale layering with dense bottom layer

## Entry Points

**Library Entry Point (`lib.rs`):**
- Location: `src/lib.rs`
- Triggers: External crate dependencies
- Responsibilities:
  - Re-exports public API types
  - Configures feature flags (sqlite-backend, native-v2)
  - Documents module organization
  - Provides quick start examples

**Factory Function (`open_graph`):**
- Location: `src/config/factory.rs`
- Triggers: Application code opening a database
- Responsibilities:
  - Inspects `GraphConfig` to determine backend
  - Creates `SqliteGraph` with appropriate backend
  - Initializes schema, cache, WAL, HNSW indexes

**CLI Entry Point (`client.rs` / `main`):**
- Location: Project root binary (not in library crate)
- Triggers: Command-line execution
- Responsibilities:
  - Parse command-line arguments
  - Open graph database
  - Execute commands (query, import, export, introspect)

**Schema Initialization (`schema.rs`):**
- Location: `src/schema.rs`
- Triggers: Graph creation or first open
- Responsibilities:
  - Create SQLite tables if missing
  - Run migrations
  - Set up indexes and pragmas

## Error Handling

**Strategy:** Result-based error propagation with thiserror

**Patterns:**
- `Result<T, SqliteGraphError>` - All public operations return Results
- `SqliteGraphError` enum with variants for different error types
- Backend-specific errors (`NativeBackendError`) convert to `SqliteGraphError`
- Errors are propagated via `?` operator without context loss

**Error Categories:**
- `EntityNotFound`: Requested node/edge doesn't exist
- `DatabaseError`: SQLite or native I/O errors
- `QueryError`: Invalid query syntax or parameters
- `ValidationError`: Input validation failures
- `RecoveryError`: Backup/restore operation failures

## Cross-Cutting Concerns

**Logging:** `log` crate with feature-gated debug level
- Disabled in release builds for zero overhead
- Enabled via `debug` feature for development

**Validation:** Input validation at API boundaries
- Node/edge specs validated before backend operations
- Property values checked for JSON serializability
- Edge types validated for empty strings

**Authentication:** Not applicable (embedded database)

**Concurrency:**
- NOT thread-safe for writes (interior mutability via RefCell)
- Thread-safe for concurrent reads via `GraphSnapshot`
- ArcSwap provides lock-free snapshot updates
- Native backend uses RwLock for cache and HNSW indexes

**Determinism:**
- All algorithms use deterministic tiebreaking
- Random seed controls HNSW layer assignment
- Sorted iteration for consistent output

**Performance Optimization:**
- LRU-K caching reduces database round-trips
- Prepared statement caching (128 capacity)
- WAL mode for SQLite (better concurrency)
- SIMD-ready distance calculations for HNSW

---

*Architecture analysis: 2026-01-20*
