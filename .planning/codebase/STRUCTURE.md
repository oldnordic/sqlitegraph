# Codebase Structure

**Analysis Date:** 2026-01-20

## Directory Layout

```
[project-root]/
├── sqlitegraph/          # Main library crate (core implementation)
│   ├── src/             # Library source code
│   │   ├── algo.rs      # Graph algorithms (PageRank, Louvain, etc.)
│   │   ├── api_ergonomics.rs  # Public API type aliases
│   │   ├── backend.rs   # Backend trait and redirection
│   │   ├── backend/     # Backend implementations
│   │   │   ├── sqlite/  # SQLite backend (mod.rs, types.rs, helpers.rs)
│   │   │   └── native/  # Native backend (file-based storage)
│   │   │       ├── adjacency/      # Adjacency iteration
│   │   │       ├── edge_store/     # Edge storage management
│   │   │       ├── graph_file/     # File I/O and memory mapping
│   │   │       ├── v2/             # V2 clustered edge kernel
│   │   │       │   ├── edge_cluster/    # Compact edge records
│   │   │       │   ├── string_table/    # String deduplication
│   │   │       │   ├── wal/            # Write-ahead log
│   │   │       │   │   ├── checkpoint/  # Checkpoint operations
│   │   │       │   │   ├── recovery/    # WAL recovery
│   │   │       │   │   └── metrics/     # WAL metrics
│   │   │       │   ├── export/         # Snapshot export
│   │   │       │   ├── import/         # Snapshot import
│   │   │       │   ├── free_space/     # Free space management
│   │   │       │   └── node_record_v2/ # V2 node records
│   │   │       ├── graph_backend.rs     # Native GraphBackend impl
│   │   │       ├── node_store.rs        # Node storage
│   │   │       ├── constants.rs         # File format constants
│   │   │       ├── cpu_tuning.rs        # CPU feature detection
│   │   │       └── mod.rs               # Native module exports
│   │   ├── bfs.rs       # Breadth-first search traversal
│   │   ├── cache.rs     # LRU-K adjacency cache
│   │   ├── config/      # Configuration and factory
│   │   │   ├── mod.rs          # Config module exports
│   │   │   ├── config.rs       # GraphConfig struct
│   │   │   ├── factory.rs      # open_graph() factory
│   │   │   ├── kinds.rs        # BackendKind enum
│   │   │   ├── native.rs       # NativeConfig options
│   │   │   └── sqlite.rs       # SqliteConfig options
│   │   ├── debug.rs     # Debugging utilities
│   │   ├── dsl.rs       # Domain-specific language helpers
│   │   ├── errors.rs    # Error type definitions
│   │   ├── graph/       # Core graph database implementation
│   │   │   ├── mod.rs            # Graph module exports
│   │   │   ├── core.rs           # SqliteGraph struct
│   │   │   ├── types.rs          # GraphEntity, GraphEdge
│   │   │   ├── adjacency.rs      # Adjacency list management
│   │   │   ├── entity_ops.rs     # Node CRUD operations
│   │   │   ├── edge_ops.rs       # Edge CRUD operations
│   │   │   ├── pattern_matching.rs  # Triple pattern matching
│   │   │   ├── snapshot.rs       # Internal snapshot logic
│   │   │   └── metrics/          # Performance instrumentation
│   │   ├── hnsw/       # HNSW vector search index
│   │   │   ├── mod.rs                  # HNSW module exports
│   │   │   ├── index.rs                # HnswIndex struct
│   │   │   ├── config.rs               # HnswConfig struct
│   │   │   ├── builder.rs              # Config builder
│   │   │   ├── distance_metric.rs      # Distance functions
│   │   │   ├── multilayer.rs           # Multi-layer graph
│   │   │   ├── neighborhood.rs         # Neighbor selection
│   │   │   ├── storage.rs              # Vector storage trait
│   │   │   └── errors.rs               # HNSW error types
│   │   ├── index.rs    # Index utilities (add_label, add_property)
│   │   ├── introspection.rs  # Graph introspection API
│   │   ├── lib.rs      # Library root with public API re-exports
│   │   ├── multi_hop.rs # Multi-hop traversal (k-hop, chain queries)
│   │   ├── mvcc.rs     # MVCC-lite snapshot system
│   │   ├── pattern.rs  # Pattern matching types
│   │   ├── pattern_engine/     # Triple pattern engine
│   │   │   ├── mod.rs               # Pattern engine exports
│   │   │   ├── matcher.rs           # Pattern matcher
│   │   │   ├── pattern.rs           # PatternTriple type
│   │   │   ├── query.rs             # PatternQuery type
│   │   │   └── property.rs          # Property filtering
│   │   ├── pattern_engine_cache/  # Pattern engine caching
│   │   │   ├── mod.rs                  # Cache exports
│   │   │   ├── fast_path_detection.rs  # Fast path detection
│   │   │   ├── fast_path_execution.rs  # Fast path execution
│   │   │   └── edge_validation.rs      # Edge validation helpers
│   │   ├── progress.rs  # Progress tracking for algorithms
│   │   ├── query.rs    # GraphQuery fluent API
│   │   ├── query_cache.rs  # Query result caching
│   │   ├── recovery.rs  # Backup and restore utilities
│   │   ├── schema.rs   # Database schema management
│   │   ├── graph_opt.rs  # Graph operations (bulk insert, cache stats)
│   │   └── tests/      # Integration tests
│   ├── benches/    # Criterion benchmarks
│   │   ├── bfs.rs
│   │   ├── k_hop.rs
│   │   ├── insert.rs
│   │   ├── hnsw.rs
│   │   ├── comprehensive_performance.rs
│   │   └── wal_recovery_benchmarks.rs
│   └── Cargo.toml   # Library crate manifest
├── sqlitegraph-cli/    # CLI binary crate
│   ├── src/
│   │   └── main.rs    # CLI entry point
│   └── Cargo.toml
├── docs/           # Documentation
├── scripts/        # Utility scripts
├── tests/          # Integration tests
├── Cargo.toml      # Workspace configuration
├── Cargo.lock      # Dependency lock file
├── README.md       # Project overview
├── CHANGELOG.md    # Version history
└── CLAUDE.md       # Development rules

```

## Directory Purposes

**sqlitegraph/src/:**
- Purpose: Core library implementation
- Contains: All graph database logic, backend implementations, algorithms
- Key files: `lib.rs` (public API), `graph/core.rs` (SqliteGraph), `backend.rs` (trait)

**sqlitegraph/src/backend/:**
- Purpose: Storage backend abstraction and implementations
- Contains: `GraphBackend` trait, SQLite and Native implementations
- Key files: `backend.rs` (trait definition), `sqlite/mod.rs`, `native/mod.rs`

**sqlitegraph/src/backend/native/v2/:**
- Purpose: Next-generation native storage with clustered edges
- Contains: Edge clustering, WAL system, snapshot support
- Key files: `edge_cluster/`, `wal/`, `string_table/`

**sqlitegraph/src/graph/:**
- Purpose: Core graph database operations
- Contains: SqliteGraph implementation, entity/edge operations
- Key files: `core.rs`, `types.rs`, `adjacency.rs`

**sqlitegraph/src/hnsw/:**
- Purpose: Vector similarity search using HNSW algorithm
- Contains: Index implementation, distance metrics, multi-layer graph
- Key files: `index.rs`, `config.rs`, `distance_metric.rs`

**sqlitegraph/src/algo.rs:**
- Purpose: Graph analysis algorithms
- Contains: PageRank, Betweenness, Louvain, Label Propagation
- Single large file with algorithm implementations

**sqlitegraph/src/mvcc.rs:**
- Purpose: Snapshot isolation for concurrent reads
- Contains: SnapshotManager, GraphSnapshot, SnapshotState

**sqlitegraph/src/cache.rs:**
- Purpose: LRU-K adjacency caching
- Contains: AdjacencyCache, CacheStats

**sqlitegraph/src/pattern_engine/:**
- Purpose: Triple pattern matching
- Contains: PatternTriple, matcher, query types

**sqlitegraph/src/config/:**
- Purpose: Backend selection and configuration
- Contains: GraphConfig, BackendKind, factory functions

**sqlitegraph/benches/:**
- Purpose: Performance benchmarks using Criterion
- Contains: BFS, k-hop, insert, HNSW benchmarks

**sqlitegraph-cli/src/:**
- Purpose: Command-line interface
- Contains: CLI argument parsing, command dispatch

**docs/:**
- Purpose: Project documentation
- Contains: API docs, architecture guides

## Key File Locations

**Entry Points:**
- `sqlitegraph/src/lib.rs`: Library root, public API re-exports
- `sqlitegraph/src/config/factory.rs`: `open_graph()` factory function
- `sqlitegraph/src/graph/core.rs`: `SqliteGraph` main struct
- `sqlitegraph-cli/src/main.rs`: CLI binary entry point

**Configuration:**
- `sqlitegraph/src/config/mod.rs`: Config module exports
- `sqlitegraph/src/config/kinds.rs`: BackendKind enum
- `sqlitegraph/Cargo.toml`: Library features and dependencies
- `Cargo.toml`: Workspace configuration

**Core Logic:**
- `sqlitegraph/src/graph/core.rs`: SqliteGraph implementation
- `sqlitegraph/src/graph/entity_ops.rs`: Node operations
- `sqlitegraph/src/graph/edge_ops.rs`: Edge operations
- `sqlitegraph/src/graph/adjacency.rs`: Adjacency list management
- `sqlitegraph/src/algo.rs`: Graph algorithms

**Backend Implementations:**
- `sqlitegraph/src/backend/sqlite/mod.rs`: SQLite backend
- `sqlitegraph/src/backend/native/graph_backend.rs`: Native backend
- `sqlitegraph/src/backend/native/v2/`: Native V2 clustered edge kernel

**Testing:**
- `sqlitegraph/tests/`: Integration tests
- `sqlitegraph/src/*/tests.rs`: Module unit tests
- `sqlitegraph/benches/`: Performance benchmarks

## Naming Conventions

**Files:**
- Module implementation: `mod_name.rs` (e.g., `cache.rs`, `mvcc.rs`)
- Module with submodules: `mod_name/` directory with `mod.rs`
- Tests: `tests.rs` within module directory or `tests/` at crate root
- Benchmarks: `bench_name.rs` in `benches/` directory

**Directories:**
- Backend modules: `backend/` with `sqlite/` and `native/` subdirectories
- Feature modules: Direct directory under `src/` (e.g., `hnsw/`, `pattern_engine/`)
- V2 components: `backend/native/v2/` with feature-specific subdirectories

**Types:**
- Public structs: PascalCase (e.g., `SqliteGraph`, `GraphEntity`, `HnswIndex`)
- Public functions: snake_case (e.g., `open_graph`, `fetch_outgoing`)
- Private fields: snake_case
- Type aliases: PascalCase (e.g., `NodeId`, `Label`)

**Traits:**
- Trait names: PascalCase (e.g., `GraphBackend`, `ProgressCallback`)
- Trait methods: snake_case

**Constants:**
- Module constants: SCREAMING_SNAKE_CASE (e.g., `V2_MAGIC`, `MAX_AVG_EDGE_SIZE`)
- Const generics: SCREAMING_SNAKE_CASE

## Where to Add New Code

**New Feature:**
- Primary code: `sqlitegraph/src/feature_name.rs` (single file) or `sqlitegraph/src/feature_name/` (multi-file module)
- Tests: `sqlitegraph/src/feature_name/tests.rs` or inline `#[cfg(test)]` mod
- Public API: Add `pub mod` declaration and re-export in `sqlitegraph/src/lib.rs`

**New Graph Algorithm:**
- Implementation: `sqlitegraph/src/algo.rs` (add function to existing file)
- Tests: `sqlitegraph/src/algo.rs` inline test module
- Benchmarks: `sqlitegraph/benches/algorithm_name.rs`

**New Backend:**
- Implementation: `sqlitegraph/src/backend/backend_name/` directory
- GraphBackend impl: `sqlitegraph/src/backend/backend_name/graph_backend.rs`
- Types: `sqlitegraph/src/backend/backend_name/types.rs`
- Registration: Add variant to `BackendKind` in `config/kinds.rs`

**New HNSW Distance Metric:**
- Implementation: `sqlitegraph/src/hnsw/distance_metric.rs` (add to enum)
- Tests: Inline test module in `hnsw/distance_metric.rs`

**New Pattern Operation:**
- Implementation: `sqlitegraph/src/pattern_engine/matcher.rs` or new file
- Cache optimization: `sqlitegraph/src/pattern_engine_cache/`

**Utilities:**
- Shared helpers: `sqlitegraph/src/utils.rs` (create if needed) or appropriate module
- CLI utilities: `sqlitegraph-cli/src/`

## Special Directories

**sqlitegraph/.cargo/:**
- Purpose: Vendored cargo registry for offline builds
- Generated: Yes (by cargo)
- Committed: Yes (for reproducible builds)

**sqlitegraph/target/:**
- Purpose: Build artifacts (compiler output)
- Generated: Yes
- Committed: No

**docs/:**
- Purpose: Generated and hand-written documentation
- Generated: Partially (cargo doc generates API docs)
- Committed: Yes (for hand-written docs)

**.planning/:**
- Purpose: Development planning and codebase analysis
- Generated: No
- Committed: Yes

**scripts/:**
- Purpose: Development and maintenance utilities
- Generated: No
- Committed: Yes

---

*Structure analysis: 2026-01-20*
