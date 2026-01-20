---
phase: 19-concurrent-features
plan: 01
subsystem: [database, pooling, sqlite]
tags: [r2d2, connection-pool, rusqlite, sqlite, concurrent-access, wal-mode]

# Dependency graph
requires:
  - phase: [11-18]
    provides: [SQLite graph backend with HNSW indexes, metrics tracking, schema management]
provides:
  - Connection pooling for SQLite backend using r2d2
  - PoolManager wrapper for connection lifecycle management
  - ConnectionWrapper enum for unified borrowed/pooled access
  - Support for concurrent reads from multiple threads
affects: [concurrent-writes, multi-threading, performance]

# Tech tracking
tech-stack:
  added: [r2d2 = "0.8", r2d2_sqlite = "0.24"]
  patterns: [Connection pooling via r2d2, enum wrapper for borrowed/owned connections, Arc for shared metrics]

key-files:
  created:
    - sqlitegraph/src/graph/pool.rs - PoolManager wrapper with 187 lines
  modified:
    - sqlitegraph/Cargo.toml - Added r2d2 dependencies
    - sqlitegraph/src/graph/core.rs - Replaced Connection with PoolManager
    - sqlitegraph/src/graph/adjacency.rs - Updated connection() accessor
    - sqlitegraph/src/graph/metrics/instrumented.rs - Added PooledInstrumentedConnection
    - sqlitegraph/src/graph/metrics/mod.rs - Exported new types
    - sqlitegraph/src/graph/mod.rs - Exported ConnectionWrapper
    - sqlitegraph/src/config/factory.rs - Updated to use connection()
    - sqlitegraph/src/graph_opt.rs - Updated TransactionGuard
    - sqlitegraph/src/hnsw/index_api.rs - Updated to use connection()
    - sqlitegraph/src/hnsw/index.rs - Updated to use connection()
    - sqlitegraph/src/graph/edge_ops.rs - Updated to use connection()
    - sqlitegraph/src/graph/entity_ops.rs - Updated to use connection()
    - sqlitegraph/src/graph/metrics_schema.rs - Updated to use connection()

key-decisions:
  - "Use r2d2_sqlite 0.24 for compatibility with rusqlite 0.31 (0.32 requires rusqlite 0.38+)"
  - "Create PoolManager wrapper instead of directly exposing r2d2::Pool for future flexibility"
  - "Use Arc<GraphMetrics> and Arc<StatementTracker> for shared ownership in pooled connections"
  - "Create ConnectionWrapper enum to unify borrowed (in-memory) and pooled (file-based) access patterns"
  - "Keep in-memory databases without pooling since each connection would have isolated data"
  - "Default pool size of 5 connections (configurable via with_max_size())"

patterns-established:
  - "Pattern 1: Connection pooling via r2d2 for SQLite concurrent access"
  - "Pattern 2: Enum wrapper pattern for unified borrowed/owned resource access"
  - "Pattern 3: Arc-wrapped shared metrics for pooled connections"

# Metrics
duration: 19min
completed: 2026-01-20
---

# Phase 19: Plan 01 Summary

**r2d2-based connection pooling for SQLite backend with concurrent access support and automatic connection return-on-drop**

## Performance

- **Duration:** 19 min
- **Started:** 2026-01-20T17:30:39Z
- **Completed:** 2026-01-20T17:49:48Z
- **Tasks:** 3 (merged into 2 commits)
- **Files modified:** 14 files created/modified

## Accomplishments

- Added r2d2 and r2d2_sqlite dependencies (0.8 and 0.24 respectively for rusqlite 0.31 compatibility)
- Created PoolManager wrapper with support for file-based (pooled) and in-memory (direct) databases
- Replaced direct Connection with PoolManager in SqliteGraph struct
- Created ConnectionWrapper enum for unified borrowed/pooled connection access
- Created PooledInstrumentedConnection for owned pooled connections
- Updated all connection access sites to use connection() method
- Maintained backward compatibility with existing test suite (717 tests passed)

## Task Commits

Each task was committed atomically:

1. **Task 1: Add r2d2 dependencies and create PoolManager** - `2b714bf` (feat)
   - Added r2d2 = "0.8" and r2d2_sqlite = "0.24" to Cargo.toml
   - Created sqlitegraph/src/graph/pool.rs with PoolManager wrapper (187 lines)

2. **Tasks 2-3: Replace direct Connection with PoolManager and update connection() accessor** - `ab220b3` (feat)
   - Replaced `pub conn: Connection` with `pool: PoolManager` in SqliteGraph
   - Updated open() methods to use pool
   - Created ConnectionWrapper and StatementWrapper enums
   - Updated all call sites (edge_ops, entity_ops, metrics_schema, factory, HNSW)

## Files Created/Modified

- `sqlitegraph/Cargo.toml` - Added r2d2 and r2d2_sqlite dependencies
- `sqlitegraph/src/graph/pool.rs` - NEW: PoolManager wrapper with new(), with_max_size(), in_memory(), from_connection(), configure_pool(), configure_direct()
- `sqlitegraph/src/graph/core.rs` - Replaced Connection with PoolManager, updated metrics/tracker to Arc<>
- `sqlitegraph/src/graph/adjacency.rs` - Created ConnectionWrapper and StatementWrapper enums, updated connection() method
- `sqlitegraph/src/graph/metrics/instrumented.rs` - Added PooledInstrumentedConnection, PooledInstrumentedCachedStatement
- `sqlitegraph/src/graph/metrics/mod.rs` - Exported new pooled types
- `sqlitegraph/src/graph/mod.rs` - Exported ConnectionWrapper and StatementWrapper
- `sqlitegraph/src/config/factory.rs` - Updated to use connection() instead of conn
- `sqlitegraph/src/graph_opt.rs` - Updated TransactionGuard to use ConnectionWrapper
- `sqlitegraph/src/hnsw/index_api.rs` - Updated to use connection()
- `sqlitegraph/src/hnsw/index.rs` - Updated to use connection()
- `sqlitegraph/src/graph/edge_ops.rs` - Updated to use connection()
- `sqlitegraph/src/graph/entity_ops.rs` - Updated to use connection()
- `sqlitegraph/src/graph/metrics_schema.rs` - Updated to use connection()

## Decisions Made

- **r2d2_sqlite version:** Used 0.24 (not latest 0.32) because 0.32 requires rusqlite 0.38+, but project uses 0.31
- **Arc for metrics/tracker:** Changed GraphMetrics and StatementTracker from owned to Arc-wrapped in SqliteGraph for shared ownership in pooled connections
- **Enum wrapper pattern:** Created ConnectionWrapper enum instead of a trait to avoid complex lifetime issues with trait objects
- **In-memory exemption:** Kept direct connection for in-memory databases since each connection would have isolated data
- **Pool size default:** Set to 5 connections (balance between resource usage and concurrency)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

- **Lifetime complexity with trait objects:** Initial trait-based approach had complex lifetime bounds. Solution: Switched to enum wrapper pattern (ConnectionWrapper) that directly forwards to the appropriate implementation.
- **Mutable pool for configure_direct:** configure_direct() requires &mut self but pool was not declared mut. Solution: Added `mut` to pool variables in open_in_memory() methods.
- **Metrics/tracker ownership:** Pooled connections need owned Arc for metrics/tracker, but borrowed connections used references. Solution: Changed SqliteGraph to store Arc<> for both, which works for both cases.

## Next Phase Readiness

- Connection pooling foundation in place for concurrent reads
- WAL mode already enabled on pool initialization for better concurrency
- Next phase can build on this for concurrent write support or thread-safe operations

---
*Phase: 19-concurrent-features*
*Completed: 2026-01-20*
