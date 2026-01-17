# Architecture

**Analysis Date:** 2026-01-17

## Pattern Overview

**Overall:** Multi-Workspace Rust Library with Dual Backend Support

**Key Characteristics:**
- Trait-based backend abstraction
- Feature-gated native backend for performance
- In-process embedded database (no server)
- MVCC snapshot system for concurrency
- CLI application separate from core library

## Layers

**Public API Layer:**
- Purpose: Clean, stable interface for consumers
- Contains: Re-exports of core types and functions
- Location: `sqlitegraph/src/lib.rs:80-157`
- Depends on: Backend abstraction layer
- Used by: CLI and external applications

**Backend Abstraction Layer:**
- Purpose: Unified interface for multiple storage backends
- Contains: `GraphBackend` trait with 11 core operations
- Location: `sqlitegraph/src/backend.rs:29-97`
- Depends on: Storage layer implementations
- Used by: Public API and graph operations

**Storage Layer:**
- Purpose: Data persistence and transaction management
- Contains: SQLite backend and Native V2 backend
- Locations:
  - SQLite: `sqlitegraph/src/backend/sqlite/`
  - Native: `sqlitegraph/src/backend/native/`
- Depends on: Filesystem and memory management
- Used by: Backend abstraction layer

**Algorithm Layer:**
- Purpose: Graph traversals and pattern matching
- Contains: BFS, k-hop, shortest path, pattern engine
- Locations:
  - `sqlitegraph/src/algo.rs`
  - `sqlitegraph/src/bfs.rs`
  - `sqlitegraph/src/multi_hop.rs`
  - `sqlitegraph/src/pattern_engine/`
- Depends on: Backend abstraction
- Used by: Public API

**Infrastructure Layer:**
- Purpose: Cross-cutting system services
- Contains: MVCC, WAL, configuration, error handling
- Locations:
  - MVCC: `sqlitegraph/src/backend/native/v2/snapshot.rs`
  - WAL: `sqlitegraph/src/backend/native/v2/wal/`
  - Config: `sqlitegraph/src/config/`
- Depends on: System primitives
- Used by: All layers

## Data Flow

**Graph Query Execution:**

1. User invokes operation (CLI or library call)
2. `GraphBackend` trait method called
3. Backend-specific implementation processes:
   - SQLite: SQL query execution
   - Native: Direct memory/file operations
4. Result returned as `Result<T, SqliteGraphError>`

**Native V2 Write Flow:**

1. Write operation requested
2. Operation logged to WAL (Write-Ahead Log)
3. Modification applied to in-memory structure
4. Optional checkpoint flushes to main file
5. MVCC snapshot updated for readers

**State Management:**
- SQLite: ACID transactions via database
- Native V2: MVCC with `ArcSwap<Arc<SnapshotState>>`
- Each operation is independent (no persistent in-memory state)

## Key Abstractions

**GraphBackend Trait:**
- Purpose: Define storage-agnostic graph operations
- Examples: `insert_node`, `get_node`, `neighbors`, `bfs`
- Pattern: Trait-based with factory function `open_graph()`
- Location: `sqlitegraph/src/backend.rs:29-97`

**Backend Implementations:**
- Purpose: Concrete storage implementations
- Examples: `SqliteGraphBackend`, `NativeGraphBackend`
- Pattern: Interior mutability with `RwLock<GraphFile>`
- Location: `sqlitegraph/src/backend/`

**Configuration Builders:**
- Purpose: Type-safe configuration
- Examples: `GraphConfig`, `SqliteConfig`, `NativeConfig`, `HnswConfigBuilder`
- Pattern: Builder pattern with sensible defaults
- Location: `sqlitegraph/src/config/config.rs`

**HNSW Index:**
- Purpose: Hierarchical Navigable Small World vector search
- Examples: `HnswIndex`, distance metrics (Cosine, Euclidean, etc.)
- Pattern: In-memory index with optional persistence
- Location: `sqlitegraph/src/hnsw/`

## Entry Points

**Library Entry:**
- Location: `sqlitegraph/src/lib.rs`
- Triggers: External library usage
- Responsibilities: Re-exports, feature gate management, public API surface

**CLI Entry:**
- Location: `sqlitegraph-cli/src/main.rs:88-169`
- Triggers: Command-line invocation
- Responsibilities: Argument parsing, backend selection, command dispatch

**Backend Factory:**
- Location: `sqlitegraph/src/config/factory.rs`
- Triggers: Library initialization
- Responsibilities: Backend instantiation based on `GraphConfig`

## Error Handling

**Strategy:** Result-based error propagation with thiserror

**Patterns:**
- Custom `SqliteGraphError` enum with variants
- `?` operator for propagation
- Context-preserving error chains
- Location: `sqlitegraph/src/errors.rs`

## Cross-Cutting Concerns

**Logging:**
- Basic log support (log 0.4)
- Debug prints gated behind `trace_v2_io` feature
- Environment variables: `V2_SLOT_DEBUG`, `CLUSTER_DEBUG`, etc.

**Validation:**
- Type-safe API via Rust's type system
- No separate validation layer
- Compile-time guarantees via trait bounds

**Concurrency:**
- MVCC snapshot isolation for Native V2
- `RwLock` for interior mutability
- `ArcSwap` for lock-free snapshot updates

**Feature Flags:**
- `sqlite-backend` (default) - Enable SQLite backend
- `native-v2` - Enable high-performance native backend
- `bench-ci` - CI benchmarking optimizations
- `trace_v2_io` - Debug I/O tracing

---

*Architecture analysis: 2026-01-17*
*Update when major patterns change*
