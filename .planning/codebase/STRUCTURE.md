# Codebase Structure

**Analysis Date:** 2026-01-20

## Directory Layout

```
sqlitegraph/                # Main project root
├── sqlitegraph/            # Library crate source
│   ├── src/               # All source files (239 .rs files)
│   │   ├── graph/         # Core graph implementation
│   │   ├── backend/       # Storage backend implementations
│   │   ├── hnsw/          # HNSW vector search
│   │   ├── pattern_engine/       # Pattern matching
│   │   ├── pattern_engine_cache/ # Pattern matching cache
│   │   └── backend/
│   │       ├── sqlite/    # SQLite backend
│   │       ├── native/    # Native storage backend
│   │       │   └── v2/    # Native V2 clustered edge format
│   │       │       ├── edge_cluster/    # Compact edge records
│   │       │       ├── free_space/      # Free space management
│   │       │       ├── string_table/    # String deduplication
│   │       │       ├── wal/             # Write-Ahead Log
│   │       │       │   ├── checkpoint/  # WAL checkpointing
│   │       │       │   └── recovery/    # WAL recovery
│   │       │       ├── export/          # Snapshot export
│   │       │       ├── import/          # Snapshot import
│   │       │       └── snapshot/        # Atomic snapshots
│   │       ├── graph_file/             # Native file operations
│   │       ├── adjacency/              # Native adjacency iterators
│   │       └── edge_store/             # Edge storage management
├── benches/               # Criterion benchmarks (16 files)
├── tests/                 # Integration tests (50+ files)
├── docs/                  # Project documentation
├── scripts/               # Utility scripts
├── Cargo.toml             # Library manifest
├── CHANGELOG.md           # Version history
├── CLAUDE.md              # Development rules
└── README.md              # Project overview
```

## Directory Purposes

**sqlitegraph/src/:**
- Purpose: All library source code (239 Rust files, 44 directories)
- Contains: Core implementation, algorithms, backends, utilities
- Key files: `lib.rs` (entry point), `graph/core.rs` (main struct), `backend.rs` (trait)

**sqlitegraph/src/graph/:**
- Purpose: Core graph database implementation
- Contains: `SqliteGraph`, `GraphEntity`, `GraphEdge`, CRUD operations
- Key files:
  - `mod.rs` - Module documentation and re-exports
  - `core.rs` - `SqliteGraph` struct and construction
  - `types.rs` - `GraphEntity`, `GraphEdge` data structures
  - `entity_ops.rs` - Node/entity CRUD operations
  - `edge_ops.rs` - Edge/relationship CRUD operations
  - `adjacency.rs` - Adjacency list management
  - `snapshot.rs` - Snapshot creation and management
  - `pattern_matching.rs` - Pattern matching integration

**sqlitegraph/src/backend/:**
- Purpose: Storage backend abstraction and implementations
- Contains: `GraphBackend` trait, backend implementations
- Key files:
  - `sqlite/mod.rs` - SQLite backend (`SqliteGraphBackend`)
  - `sqlite/helpers.rs` - SQLite query helpers
  - `native/mod.rs` - Native backend organization
  - `native/graph_backend.rs` - `NativeGraphBackend` implementation
  - `native/v2/mod.rs` - V2 clustered edge kernel

**sqlitegraph/src/backend/native/v2/:**
- Purpose: High-performance native storage with WAL and clustering
- Contains: WAL system, edge clustering, free space management
- Key files:
  - `mod.rs` - V2 module organization and exports
  - `edge_cluster/` - Compact edge record format
  - `wal/mod.rs` - WAL manager and integration
  - `wal/checkpoint/` - Checkpoint coordinator and strategies
  - `wal/recovery/` - Recovery replayer and validator
  - `string_table/mod.rs` - String deduplication
  - `free_space/` - Block-level free space management
  - `export/`, `import/` - Snapshot migration

**sqlitegraph/src/hnsw/:**
- Purpose: Hierarchical Navigable Small World vector search
- Contains: HNSW index, configuration, distance metrics
- Key files:
  - `mod.rs` - Module documentation
  - `index.rs` - `HnswIndex` main implementation
  - `config.rs` - `HnswConfig`, `HnswConfigBuilder`
  - `distance_metric.rs` - Distance metric enum
  - `distance_functions.rs` - SIMD-ready calculations
  - `storage.rs` - `VectorStorage` trait for pluggable backends

**sqlitegraph/src/pattern_engine/:**
- Purpose: Triple pattern matching
- Contains: `PatternTriple`, `match_triples`, query execution
- Key files:
  - `mod.rs` - Module exports
  - `pattern.rs` - `PatternTriple` definition
  - `matcher.rs` - Pattern matching logic
  - `query.rs` - Pattern query execution

**sqlitegraph/src/pattern_engine_cache/:**
- Purpose: Fast-path pattern matching with caching
- Contains: Edge validation, fast path detection/execution
- Key files:
  - `mod.rs` - Cache module exports
  - `fast_path_detection.rs` - Detects cacheable patterns
  - `fast_path_execution.rs` - Executes cached patterns
  - `edge_validation.rs` - Edge existence validation

**sqlitegraph/benches/:**
- Purpose: Criterion benchmarks for performance testing
- Contains: Algorithm benchmarks, backend comparisons, WAL recovery
- Key files:
  - `algo_benchmarks.rs` - PageRank, Betweenness, Louvain
  - `bfs.rs`, `k_hop.rs` - Traversal benchmarks
  - `hnsw.rs` - Vector search benchmarks
  - `comprehensive_performance.rs` - Full system comparison
  - `wal_recovery_benchmarks.rs` - WAL recovery performance

**sqlitegraph/tests/:**
- Purpose: Integration tests (not unit tests in src/)
- Contains: Algorithm validation, regression tests, stress tests
- Key files:
  - `algo_tests.rs` - Algorithm correctness validation
  - `adjacency_iterator_infinite_loop_test.rs` - Regression tests
  - `cluster_offset_corruption_regression.rs` - Data integrity tests

**sqlitegraph/docs/:**
- Purpose: Project documentation (mdBook format)
- Contains: Architecture guides, API documentation, tutorials

## Key File Locations

**Entry Points:**
- `sqlitegraph/src/lib.rs`: Library entry point, public API re-exports
- `sqlitegraph/src/config/factory.rs`: `open_graph()` factory function
- `sqlitegraph/src/client.rs`: CLI entry point (binary crate)

**Configuration:**
- `sqlitegraph/Cargo.toml`: Library manifest, dependencies, features
- `sqlitegraph/src/config/mod.rs`: Configuration module re-exports
- `sqlitegraph/src/config/kinds.rs`: `BackendKind` enum
- `sqlitegraph/src/config/config.rs`: `GraphConfig` struct
- `sqlitegraph/src/config/native.rs`: `NativeConfig` (CPU profiles, capacity)
- `sqlitegraph/src/config/sqlite.rs`: `SqliteConfig` (WAL mode, cache size)

**Core Logic:**
- `sqlitegraph/src/graph/core.rs`: `SqliteGraph` main struct
- `sqlitegraph/src/graph/types.rs`: `GraphEntity`, `GraphEdge`
- `sqlitegraph/src/graph/entity_ops.rs`: Node/Entity CRUD
- `sqlitegraph/src/graph/edge_ops.rs`: Edge/Relationship CRUD
- `sqlitegraph/src/backend.rs`: `GraphBackend` trait definition
- `sqlitegraph/src/backend/native/graph_backend.rs`: Native backend implementation

**Algorithms:**
- `sqlitegraph/src/algo.rs`: All graph algorithms (PageRank, Betweenness, etc.)
- `sqlitegraph/src/bfs.rs`: Breadth-first search traversal
- `sqlitegraph/src/multi_hop.rs`: k-hop queries and chain queries

**Testing:**
- `sqlitegraph/tests/`: Integration tests (50+ test files)
- `sqlitegraph/benches/`: Criterion benchmarks (16 bench files)
- Unit tests embedded in `src/**/*.rs` files in `#[cfg(test)]` modules

## Naming Conventions

**Files:**
- Core modules: `mod_name.rs` (e.g., `algo.rs`, `cache.rs`)
- Submodules: `mod_name/mod.rs` with child files (e.g., `graph/mod.rs`, `graph/core.rs`)
- Test files: `*_tests.rs` in `tests/` directory (e.g., `algo_tests.rs`)
- Benchmark files: `*.rs` in `benches/` directory (e.g., `bfs.rs`)

**Directories:**
- Plural names for collections: `metrics/`, `benchmarks/`, `tests/`
- Snake_case for multi-word directories: `graph_file/`, `edge_store/`, `string_table/`
- Feature-specific directories: `pattern_engine/`, `pattern_engine_cache/`
- Version-specific directories: `v2/` (native V2 format)

**Types:**
- Structs: `PascalCase` (e.g., `SqliteGraph`, `GraphEntity`, `HnswIndex`)
- Enums: `PascalCase` (e.g., `BackendKind`, `DistanceMetric`)
- Traits: `PascalCase` (e.g., `GraphBackend`, `ProgressCallback`)
- Type aliases: `PascalCase` (e.g., `NodeId`, `Label`)

**Functions:**
- Public API: `snake_case` (e.g., `insert_node`, `fetch_outgoing`, `pagerank`)
- Private helpers: `snake_case` with leading underscore if unused
- Methods: `snake_case` (e.g., `graph.neighbors()`, `cache.get()`)

**Constants:**
- SCREAMING_SNAKE_CASE for constants (e.g., `MAX_AVG_EDGE_SIZE`, `V2_MAGIC`)

## Where to Add New Code

**New Graph Algorithm:**
- Primary code: `src/algo.rs` (add public function at module level)
- Tests: `tests/algo_tests.rs` (add test case)
- Benchmarks: `benches/algo_benchmarks.rs` (add benchmark)

**New Backend Feature:**
- SQLite: `src/backend/sqlite/helpers.rs` or new file in `src/backend/sqlite/`
- Native V2: `src/backend/native/v2/` (create new submodule if needed)
- Tests: `src/backend/native/v2/wal/tests.rs` or separate test file

**New Pattern Matching Feature:**
- Implementation: `src/pattern_engine/` (matcher, pattern, or query)
- Cache optimization: `src/pattern_engine_cache/`
- Tests: `src/pattern_engine/tests.rs`

**New HNSW Distance Metric:**
- Implementation: `src/hnsw/distance_functions.rs`
- Enum variant: `src/hnsw/distance_metric.rs`
- Tests: Unit tests in `src/hnsw/distance_functions.rs`

**New Storage Format Version:**
- Create: `src/backend/native/v3/` (new directory)
- Modules: Follow v2 structure (edge_cluster, wal, string_table, etc.)
- Update: `src/backend/native/mod.rs` to include v3

**Utilities:**
- Shared helpers: `src/bench_utils.rs` (for benchmarks)
- Common types: `src/api_ergonomics.rs` (public type aliases)
- Error types: `src/errors.rs`

**Configuration Options:**
- Backend selection: `src/config/kinds.rs`
- Native config: `src/config/native.rs`
- SQLite config: `src/config/sqlite.rs`
- Factory logic: `src/config/factory.rs`

## Special Directories

**sqlitegraph/sqlitegraph/.cargo/:**
- Purpose: Vendored cargo registry (for vendored dependencies)
- Generated: Yes (by cargo vendor)
- Committed: Yes

**sqlitegraph/sqlitegraph/target/:**
- Purpose: Cargo build output (not in project root, but in subdirectory)
- Generated: Yes (by cargo build)
- Committed: No (in .gitignore)

**sqlitegraph/docs/:**
- Purpose: mdBook documentation source
- Generated: Partially (some auto-generated from docs)
- Committed: Yes

**sqlitegraph/scripts/:**
- Purpose: Utility scripts for development, testing, deployment
- Generated: No
- Committed: Yes

---

*Structure analysis: 2026-01-20*
