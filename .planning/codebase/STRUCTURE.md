# Codebase Structure

**Analysis Date:** 2026-01-17

## Directory Layout

```
sqlitegraph/
├── Cargo.toml              # Workspace configuration
├── CLAUDE.md               # Development rules and guidelines
├── CONTRIBUTING.md         # Contribution workflow
├── README.md               # Project documentation
├── docs/                   # Project documentation
│   ├── DEVELOPMENT_WORKFLOW.md
│   ├── DATABASE_SCHEMA.md
│   └── *.md                # Various analysis docs
├── sqlitegraph/            # Core library crate
│   ├── Cargo.toml          # Library dependencies
│   ├── clippy.toml         # Linting configuration
│   ├── benches/            # Performance benchmarks
│   │   ├── bfs.rs
│   │   ├── insert.rs
│   │   ├── k_hop.rs
│   │   ├── hnsw.rs
│   │   └── v2_performance.rs
│   ├── examples/           # Usage examples
│   ├── src/                # Library source code
│   └── tests/              # Integration tests
└── sqlitegraph-cli/        # CLI application crate
    ├── Cargo.toml          # CLI dependencies
    └── src/                # CLI source code
```

## Directory Purposes

**sqlitegraph/src/backend/:**
- Purpose: Storage backend implementations
- Contains: SQLite and Native V2 backend code
- Key files: `mod.rs`, `sqlite/mod.rs`, `native/graph_backend.rs`
- Subdirectories:
  - `sqlite/` - SQLite-based backend
  - `native/v2/` - High-performance native backend
    - `wal/` - Write-Ahead Logging
    - `node_record_v2/` - Node storage format
    - `edge_cluster/` - Edge clustering
    - `free_space/` - Space management

**sqlitegraph/src/graph/:**
- Purpose: Core graph data structures
- Contains: Graph entity types, core operations
- Key files: `core.rs`, `types.rs`, `metrics/`
- Subdirectories: `metrics/` for performance tracking

**sqlitegraph/src/pattern_engine/:**
- Purpose: Graph pattern matching
- Contains: Pattern parsing and execution
- Key files: Pattern matching logic

**sqlitegraph/src/hnsw/:**
- Purpose: Vector similarity search (HNSW algorithm)
- Contains: HNSW index implementation
- Key files: `config.rs`, index implementation

**sqlitegraph/src/config/:**
- Purpose: Configuration management
- Contains: Config types, backend factory
- Key files: `mod.rs`, `config.rs`, `factory.rs`

**sqlitegraph/benches/:**
- Purpose: Performance benchmarks
- Contains: Criterion benchmark harnesses
- Key files: `bfs.rs`, `insert.rs`, `k_hop.rs`, `hnsw.rs`

**sqlitegraph/tests/:**
- Purpose: Integration tests
- Contains: Full-system tests
- Key files: `algo_tests.rs`, `bfs_tests.rs`, `backend_selector_tests.rs`

**sqlitegraph-cli/src/:**
- Purpose: CLI application
- Contains: Command-line interface
- Key files: `main.rs` (command dispatcher), `client.rs`

## Key File Locations

**Entry Points:**
- `sqlitegraph/src/lib.rs` - Public API gateway, re-exports
- `sqlitegraph-cli/src/main.rs:88-169` - CLI entry point and command handler

**Configuration:**
- `Cargo.toml` - Workspace configuration
- `sqlitegraph/Cargo.toml` - Library dependencies
- `sqlitegraph-cli/Cargo.toml` - CLI dependencies
- `sqlitegraph/clippy.toml` - Linting rules
- `sqlitegraph/src/config/config.rs` - Runtime configuration

**Core Logic:**
- `sqlitegraph/src/backend.rs` - Backend trait definition
- `sqlitegraph/src/algo.rs` - Graph algorithms
- `sqlitegraph/src/bfs.rs` - BFS implementation
- `sqlitegraph/src/multi_hop.rs` - Multi-hop queries
- `sqlitegraph/src/query.rs` - Query interface

**Testing:**
- `sqlitegraph/tests/` - Integration tests
- `sqlitegraph/benches/` - Performance benchmarks
- `sqlitegraph/clippy.toml` - Test-specific linting configuration

**Documentation:**
- `README.md` - User-facing documentation
- `CLAUDE.md` - Development rules (MANDATORY)
- `CONTRIBUTING.md` - Development workflow
- `docs/` - Detailed documentation

## Naming Conventions

**Files:**
- `snake_case.rs` for all Rust source files
- Descriptive names: `graph_file_core.rs`, `node_record_v2.rs`
- Test suffixes: `_tests.rs` for test files

**Directories:**
- `snake_case` for most directories
- Exception: `v2/` for version 2 components
- Plural for collections: `benches/`, `tests/`, `metrics/`

**Types:**
- `PascalCase` for structs and enums
- Backend-prefixed: `NativeNodeId`, `SlotId`
- Error suffixed: `SqliteGraphError`

**Functions:**
- `snake_case` for all functions
- Verb-noun pattern: `insert_node`, `get_neighbors`
- Constructor: `new()`, `open_in_memory()`

## Where to Add New Code

**New Graph Algorithm:**
- Primary code: `sqlitegraph/src/algo.rs` or new `sqlitegraph/src/{algorithm}.rs`
- Tests: `sqlitegraph/tests/{algorithm}_tests.rs`
- Benchmarks: `sqlitegraph/benches/{algorithm}.rs`

**New Backend Operation:**
- Trait definition: `sqlitegraph/src/backend.rs`
- SQLite impl: `sqlitegraph/src/backend/sqlite/`
- Native impl: `sqlitegraph/src/backend/native/`

**New CLI Command:**
- Definition: `sqlitegraph-cli/src/main.rs`
- Tests: `sqlitegraph/tests/` or CLI-specific tests

**New Configuration Option:**
- Type definition: `sqlitegraph/src/config/config.rs`
- Factory handling: `sqlitegraph/src/config/factory.rs`

**Utilities:**
- Shared helpers: `sqlitegraph/src/{utility_name}.rs`
- Private helpers: Inside relevant module, not exported

## Special Directories

**docs/:**
- Purpose: Project documentation and analysis
- Source: Manually written
- Committed: Yes

**sqlitegraph/src/backend/native/v2/wal/:**
- Purpose: Write-Ahead Logging for Native V2
- Source: Custom implementation
- Committed: Yes

**benches/:**
- Purpose: Performance regression tracking
- Source: Hand-written benchmarks
- Committed: Yes

---

*Structure analysis: 2026-01-17*
*Update when directory structure changes*
