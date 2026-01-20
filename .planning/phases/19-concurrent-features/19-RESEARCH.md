# Phase 19: Concurrent Features - Research

**Researched:** 2026-01-20
**Domain:** SQLite connection pooling for Rust graph database
**Confidence:** HIGH

## Summary

SQLiteGraph currently opens direct connections (`Connection::open()`) without pooling. The project uses `rusqlite 0.31` and needs connection pooling to support concurrent access patterns. After researching available options, **r2d2 with r2d2_sqlite** is the recommended standard approach, though version compatibility must be verified.

**Primary recommendation:** Use `r2d2` (generic connection pool) with `r2d2_sqlite` adapter for SQLite-specific connection management. The alternative `rusqlite-pool` crate is simpler but requires compatibility verification with rusqlite 0.31.

## Current State: Connection Management

### Direct Connection Opens

**File: `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/graph/core.rs`**

```rust
// Lines 51-56: Direct connection opening in SqliteGraph
pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, SqliteGraphError> {
    let conn =
        Connection::open(path).map_err(|e| SqliteGraphError::connection(e.to_string()))?;
    ensure_schema(&conn)?;
    Ok(Self::from_connection(conn))
}
```

**Current architecture:**
- `SqliteGraph` owns a single `pub conn: Connection` (line 27)
- `connection()` method returns `InstrumentedConnection<'_>` wrapping `&self.conn`
- Graph uses interior mutability (`RefCell` patterns in caches)
- **NOT thread-safe for concurrent writes** (documented in `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/graph/mod.rs`)

### Connection Access Pattern

**File: `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/graph/adjacency.rs`**

```rust
// Lines 10-12: Connection wrapper
pub(crate) fn connection(&self) -> InstrumentedConnection<'_> {
    InstrumentedConnection::new(&self.conn, &self.metrics, &self.statement_tracker)
}
```

Every query operation calls `self.connection()` to borrow the underlying connection. With a pool, this would need to change to a checked-out connection handle.

### Configuration Pattern

**File: `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/config/sqlite.rs`**

```rust
/// Configuration for SQLite backend operations.
#[derive(Clone, Debug, Default)]
pub struct SqliteConfig {
    /// Skip schema initialization during opening
    pub without_migrations: bool,
    /// Optional cache size for prepared statements
    pub cache_size: Option<usize>,
    /// Additional SQLite PRAGMA settings
    pub pragma_settings: HashMap<String, String>,
}
```

The pattern for configurable options uses `Option<usize>` for numeric settings and builder pattern methods like `with_cache_size()`.

## Standard Stack

### Core Libraries

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `r2d2` | 0.8 | Generic connection pool | Industry standard, mature, well-maintained |
| `r2d2_sqlite` | *verify latest* | SQLite adapter for r2d2 | Provides ManageConnection trait for rusqlite |
| `rusqlite` | 0.31 | SQLite bindings (existing) | Already in project |

**Installation:**
```toml
[dependencies]
r2d2 = "0.8"
r2d2_sqlite = "*"  # Check latest compatible version
```

### Supporting Options

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `deadpool` | 0.9+ | Async-first connection pool | If migrating to async runtime |
| `rusqlite-pool` | 0.2 | Minimal sync pool | If r2d2 compatibility issues |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| r2d2 + r2d2_sqlite | rusqlite-pool | Simpler API but newer, less ecosystem adoption |
| r2d2 | deadpool | Deadpool is async-focused; r2d2 better for sync code |

## Architecture Patterns

### Recommended Project Structure

```
src/
├── graph/
│   ├── core.rs           # Modify to add pool support
│   ├── adjacency.rs      # Update connection() method
│   └── pool.rs           # NEW: Connection pool wrapper
├── config/
│   └── sqlite.rs         # Add pool_size field
└── backend/
    └── sqlite/
        └── pool_manager.rs  # NEW: Pool management
```

### Pattern 1: r2d2 Pool Wrapper

**What:** Wrap `r2d2::Pool<r2d2_sqlite::SqliteConnectionManager>` for connection management

**When to use:** Standard synchronous operations with concurrent access

**Example:**
```rust
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use rusqlite::Connection;

// In SqliteConfig
pub pool_size: Option<usize>,  // NEW field

// In SqliteGraph
pub struct SqliteGraph {
    pool: Pool<SqliteConnectionManager>,  // Instead of single Connection
    // ... other fields
}

impl SqliteGraph {
    pub fn open<P: AsRef<Path>>(path: P, cfg: &SqliteConfig) -> Result<Self, SqliteGraphError> {
        let manager = SqliteConnectionManager::file(path);
        let pool_size = cfg.pool_size.unwrap_or(5);  // Default 5 connections
        let pool = Pool::builder()
            .max_size(pool_size)
            .build(manager)
            .map_err(|e| SqliteGraphError::connection(e.to_string()))?;

        // Initialize schema using first connection
        let conn = pool.get()
            .map_err(|e| SqliteGraphError::connection(e.to_string()))?;
        ensure_schema(&conn)?;

        Ok(Self { pool, /* ... */ })
    }

    // Operations now checkout connections:
    pub(crate) fn connection(&self) -> Result<PooledConnection<SqliteConnectionManager>, SqliteGraphError> {
        self.pool.get()
            .map_err(|e| SqliteGraphError::connection(e.to_string()))
    }
}
```

### Pattern 2: Configuration Builder Extension

```rust
// In SqliteConfig
impl SqliteConfig {
    /// Set connection pool size (builder pattern)
    pub fn with_pool_size(mut self, size: usize) -> Self {
        self.pool_size = Some(size);
        self
    }

    /// Set maximum pool size (alias)
    pub fn with_max_connections(mut self, max: usize) -> Self {
        self.pool_size = Some(max);
        self
    }
}

// Usage
let cfg = SqliteConfig::new()
    .with_pool_size(10)
    .with_wal_mode();
```

### Anti-Patterns to Avoid

- **Single connection shared across threads:** rusqlite `Connection` is not `Sync`; sharing causes data races
- **Creating new connections per query:** Defeats the purpose of pooling; high overhead
- **Unbounded pool size:** Can exhaust file descriptors; use sensible limits
- **Holding connections indefinitely:** Return to pool promptly after use

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Connection queue | `Arc<Mutex<Vec<Connection>>>` | `r2d2::Pool` | Handles timeouts, reconnection, health checks |
| Semaphore limiting | `Semaphore::new(5)` | `r2d2` built-in | Includes connection recycling logic |
| Manual connection lifecycle | Drop guards | `PooledConnection` | Auto-return on drop, proper cleanup |

**Key insight:** Connection pools need connection validation, timeout handling, and proper recycling. Custom implementations miss edge cases like database restarts, network issues, and connection state corruption.

## Common Pitfalls

### Pitfall 1: Version Incompatibility

**What goes wrong:** `r2d2_sqlite 0.24` depends on older `rusqlite 0.24.x`, incompatible with project's `rusqlite 0.31`

**Why it happens:** rusqlite had breaking API changes between versions

**How to avoid:**
1. Check `r2d2_sqlite` latest version supports `rusqlite 0.31`
2. If not available, consider `rusqlite-pool` or implement custom `ManageConnection` for r2d2
3. Verify with `cargo tree` before committing

**Warning signs:** Compilation errors about missing `rusqlite` types or trait implementations

### Pitfall 2: Blocking the Pool

**What goes wrong:** Long-running queries hold connections, starving other operations

**Why it happens:** Connection held for entire operation duration

**How to avoid:**
- Use transactions for bulk operations
- Return connections to pool promptly
- Set appropriate timeouts via pool builder

**Warning signs:** Operations timeout waiting for connections from pool

### Pitfall 3: In-Memory Databases

**What goes wrong:** Connection pooling with `:memory:` databases creates separate databases per connection

**Why it happens:** Each in-memory connection has isolated data

**How to avoid:**
- Detect in-memory connections (`is_in_memory_connection()`)
- Skip pooling for in-memory databases
- Document this limitation clearly

**Warning signs:** Data not visible across different connection handles

### Pitfall 4: WAL Mode Concurrency

**What goes wrong:** WAL mode not enabled limits concurrent read/write effectiveness

**Why it happens:** Default DELETE mode blocks readers during writes

**How to avoid:**
- WAL mode is already enabled in `from_connection()` (line 85-88 of core.rs)
- Ensure pool initialization preserves WAL configuration
- Consider `busy_timeout` configuration for pool

## Code Examples

### Pool Initialization with Configuration

```rust
use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;

pub fn open_with_pool<P: AsRef<Path>>(
    path: P,
    cfg: &SqliteConfig
) -> Result<SqliteGraph, SqliteGraphError> {
    let manager = SqliteConnectionManager::file(&path);

    // Configure pool builder
    let mut builder = Pool::builder();
    if let Some(size) = cfg.pool_size {
        builder = builder.max_size(size);
    }

    // Build pool
    let pool = builder
        .build(manager)
        .map_err(|e| SqliteGraphError::connection(e.to_string()))?;

    // Initialize schema with first connection
    let conn = pool.get()
        .map_err(|e| SqliteGraphError::connection(e.to_string()))?;
    ensure_schema(&conn)?;

    SqliteGraph::from_pool(pool, cfg)
}
```

### Connection Checkout Pattern

```rust
// In each operation that needs database access
pub fn fetch_outgoing(&self, id: i64) -> Result<Vec<i64>, SqliteGraphError> {
    if let Some(cached) = self.outgoing_cache.get(id) {
        return Ok(cached);
    }

    // Checkout connection from pool
    let conn = self.connection()?;  // Returns PooledConnection
    let mut stmt = conn.prepare_cached(
        "SELECT to_id FROM graph_edges WHERE from_id=?1 ORDER BY to_id"
    )?;

    // ... query logic
    // Connection automatically returned to pool when `conn` drops
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Direct `Connection::open()` | `r2d2::Pool` wrapper | Phase 19 | Enables concurrent access, reduces overhead |
| Single connection per graph | Configurable pool size | Phase 19 | Better resource utilization |
| Manual connection lifecycle | Managed pool with health checks | Phase 19 | More robust, handles reconnection |

**Deprecated/outdated:**
- Opening new connection per query: High overhead, not production-ready
- Sharing single `Connection` via `Arc<Mutex<>>`: Defeats SQLite's WAL concurrency benefits
- Custom `Vec<Connection>` implementations: Missing health checks, timeout handling

## Open Questions

1. **r2d2_sqlite compatibility with rusqlite 0.31**
   - What we know: Project uses rusqlite 0.31; r2d2_sqlite 0.24 uses older rusqlite
   - What's unclear: Does r2d2_sqlite 0.25+ support rusqlite 0.31?
   - Recommendation: Verify with `cargo check` after adding dependency; if incompatible, use `rusqlite-pool` or implement custom `ManageConnection`

2. **Default pool size**
   - What we know: Typical defaults are 5-10 connections
   - What's unclear: What's optimal for graph database workloads?
   - Recommendation: Start with 5, make configurable, benchmark in Phase 20

3. **Snapshot behavior with pool**
   - What we know: `GraphSnapshot` opens separate read-only connections
   - What's unclear: Should snapshots also use pool or separate connections?
   - Recommendation: Snapshots should get dedicated connections from pool with read-only configuration

## Files Requiring Modification

### Core Changes (Required)

| File | Changes | Impact |
|------|---------|--------|
| `src/graph/core.rs` | Replace `Connection` with `Pool`, add pool initialization | High |
| `src/graph/adjacency.rs` | Update `connection()` to return pooled connection | High |
| `src/config/sqlite.rs` | Add `pool_size` field and builder methods | Medium |

### Secondary Changes (Likely)

| File | Changes | Impact |
|------|---------|--------|
| `src/mvcc.rs` | Update snapshot connection handling for pool | Medium |
| `src/config/factory.rs` | Pass pool configuration through factory | Medium |
| `src/hnsw/index_api.rs` | Update connection access patterns | Low |

### New Files (Recommended)

| File | Purpose |
|------|---------|
| `src/graph/pool.rs` | Pool wrapper and management utilities |
| `tests/concurrent_tests.rs` | Concurrent access tests |

## Sources

### Primary (HIGH confidence)

- [rusqlite 0.31.0 on crates.io](https://crates.io/crates/rusqlite/0.31.0) - Current project version
- [r2d2 documentation on docs.rs](https://docs.rs/r2d2/latest/r2d2/) - Generic connection pool API
- [r2d2_sqlite on lib.rs](https://lib.rs/crates/r2d2_sqlite) - SQLite adapter for r2d2
- [rusqlite-pool on docs.rs](https://docs.rs/rusqlite-pool/latest/rusqlite_pool/) - Alternative minimal pool
- [rusqlite issue #188](https://github.com/rusqlite/rusqlite/issues/188) - Thread safety discussion

### Secondary (MEDIUM confidence)

- [StackOverflow: Multi-threaded SQLite with rusqlite](https://stackoverflow.com/questions/62560396/how-to-use-sqlite-via-rusqlite-from-multiple-threads) - r2d2 usage pattern
- [Reddit: Sharing connection pool object](https://users.rust-lang.org/t/sharing-a-connection-pool-object/54501) - Pool sharing patterns
- [r2d2 GitHub repository](https://github.com/sfackler/r2d2) - Official source and examples

### Tertiary (LOW confidence)

- [OneUptime: Rust Database Connection Pooling (2026-01-07)](https://oneuptime.com/blog/post/2026-01-07-rust-database-connection-pooling/view) - General pooling best practices
- [deadpool-sqlite on docs.rs](https://docs.rs/deadpool-sqlite) - Alternative async-focused pool

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - r2d2 is industry standard; r2d2_sqlite is well-established
- Architecture: HIGH - Pool wrapper pattern is standard; configuration pattern matches existing codebase
- Pitfalls: MEDIUM - Version compatibility needs runtime verification; in-memory DB edge case confirmed
- Implementation details: MEDIUM - Specific API calls verified but compatibility testing needed

**Research date:** 2026-01-20
**Valid until:** 2026-02-20 (30 days - library ecosystem evolves)
