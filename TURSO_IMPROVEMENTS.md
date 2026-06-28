# Turso → SQLitegraph Improvements

**Analysis:** turso database engine vs sqlitegraph (graph layer on SQLite)  
**Goal:** Extract turso improvements for sqlitegraph

---

## Turso Strengths (Adopt for sqlitegraph)

### 1. **Async I/O Model** ⭐ HIGH PRIORITY

**Turso:** `bindings/rust/src/connection.rs:82`
```rust
pub async fn execute(&mut self, sql: &str, params: impl IntoParams) -> Result<usize> {
    let (sender, receiver) = oneshot::channel();
    self.send_request(Request::Execute {
        sql: sql.to_string(),
        params: params.into_params(),
        sender,
    }).await?;
    
    let result = receiver.await?;
    match result {
        Response::Execute(count) => Ok(count),
        Response::Error(err) => Err(err.into()),
    }
}
```

**Pattern:** Command channel → worker thread → response channel.

**SQLitegraph gap:** Currently sync-only (`rusqlite` blocking calls).

**Implement for sqlitegraph:**
```rust
// In sqlitegraph-core/src/backend/sqlite.rs
pub struct AsyncSqliteBackend {
    command_tx: mpsc::UnboundedSender<SqlCommand>,
    worker_thread: JoinHandle<()>,
}

impl AsyncSqliteBackend {
    pub async fn new(path: &str) -> Result<Self> {
        let (command_tx, command_rx) = mpsc::unbounded_channel();
        
        let worker_thread = tokio::task::spawn_blocking(move || {
            let conn = rusqlite::Connection::connect(path)?;
            
            for cmd in command_rx {
                match cmd {
                    SqlCommand::Execute { sql, params, responder } => {
                        let result = conn.execute(&sql, params);
                        responder.send(result);
                    }
                    SqlCommand::Query { sql, params, responder } => {
                        let result = conn.query(&sql, params);
                        responder.send(result);
                    }
                }
            }
            
            Ok(())
        });
        
        Ok(Self { command_tx, worker_thread })
    }
    
    pub async fn insert_node(&mut self, node: &GraphEntity) -> Result<NodeId> {
        let (responder, receiver) = oneshot::channel();
        
        self.command_tx.send(SqlCommand::Execute {
            sql: "INSERT INTO nodes (id, data) VALUES (?, ?)",
            params: vec![node.id.clone(), node.data.clone()],
            responder,
        }).await?;
        
        let result = receiver.await??;
        Ok(node.id)
    }
}
```

---

### 2. **Extension System** ⭐⭐ HIGH PRIORITY

**Turso:** `extensions/core/src/lib.rs:45`
```rust
pub trait ExtensionApi {
    // Scalar functions
    fn scalar(&mut self, ctx: &Context, argc: i32, argv: &mut[*const u8]) -> Result<Value>;
    
    // Aggregate functions
    fn aggregate(&mut self, ctx: &mut AggContext, argc: i32, argv: &mut[*const u8]) -> Result<Value>;
    
    // Virtual tables
    fn vtab(&mut self, ctx: &Context, argc: i32, argv: &mut[*const u8]) -> Result<VTab>;
}
```

**Features:** Crypto, regexp, csv, fuzzy, ipaddr, percentile extensions.

**SQLitegraph gap:** No extensibility. Hardcoded algorithms only.

**Implement for sqlitegraph:**
```rust
// In sqlitegraph-core/src/extension.rs
pub trait GraphExtension {
    fn name(&self) -> &str;
    
    // Custom scalar functions for queries
    fn scalar(&self, input: &ExtensionInput) -> Result<ExtensionOutput>;
    
    // Custom aggregate functions (e.g., community detection)
    fn aggregate(&self, nodes: &[GraphEntity]) -> Result<f64>;
}

// Register extension
pub trait GraphBackend {
    fn register_extension(&mut self, extension: Box<dyn GraphExtension>) -> Result<()>;
    
    fn call_extension(&self, name: &str, input: &ExtensionInput) -> Result<ExtensionOutput>;
}

// Example: Pagerank extension
struct PagerankExtension;
impl GraphExtension for PagerankExtension {
    fn name(&self) -> &str { "pagerank" }
    
    fn scalar(&self, input: &ExtensionInput) -> Result<ExtensionOutput> {
        match input {
            ExtensionInput::Compute(nodes) => {
                let scores = pagerank(&self.graph, 0.85, 100)?;
                Ok(ExtensionOutput::Float(scores))
            }
            _ => Err(anyhow!("Invalid input")),
        }
    }
}
```

---

### 3. **WAL + Checkpointing** ⭐ MEDIUM PRIORITY

**Turso:** `core/storage/wal.rs` (Write-ahead log with checkpointing)

**SQLitegraph gap:** Uses rusqlite's built-in WAL, no custom checkpointing control.

**Improve:**
```rust
// In sqlitegraph-core/src/backend/sqlite.rs
pub struct SqliteBackendConfig {
    pub wal_mode: bool,
    pub checkpoint_threshold: u64,  // Checkpoint every N bytes
    pub checkpoint_timeout_secs: u64,
}

impl SqliteBackend {
    pub fn with_config(path: &str, config: &SqliteBackendConfig) -> Result<Self> {
        let mut conn = rusqlite::Connection::connect(path)?;
        
        if config.wal_mode {
            conn.pragma_update(None, "journal_mode", &"WAL")?;
        }
        
        // Background checkpoint task
        if config.checkpoint_threshold > 0 {
            tokio::spawn(async move {
                let mut interval = tokio::time::interval(Duration::from_secs(config.checkpoint_timeout_secs));
                loop {
                    interval.tick().await;
                    if let Ok(wal_size) = get_wal_size(path) {
                        if wal_size > config.checkpoint_threshold {
                            checkpoint(path);
                        }
                    }
                }
            });
        }
        
        Ok(Self { conn, config: config.clone() })
    }
}
```

---

### 4. **MVCC Snapshots** ⭐⭐ HIGH PRIORITY

**Turso:** `core/mvcc/` — Multi-Version Concurrency Control

**SQLitegraph:** Has MVCC but simpler implementation (`sqlitegraph-core/src/mvcc/`).

**Turso improvements:**
- Snapshot reuse across operations
- Configurable isolation levels
- Snapshot version tracking

**Port to sqlitegraph:**
```rust
// In sqlitegraph-core/src/mvcc/snapshot.rs
pub struct Snapshot {
    version: u64,
    reader_count: AtomicU64,
    db_copy: Option<Arc<DbCopy>>,
}

impl GraphBackend for SqliteGraph {
    fn snapshot(&mut self) -> Result<Snapshot> {
        let version = self.current_version.fetch_add(1, Ordering::SeqCst);
        
        // Snapshot index and node cache
        let index_copy = self.index.clone();
        let cache_copy = self.node_cache.frozen_clone();
        
        Ok(Snapshot {
            version,
            reader_count: AtomicU64::new(1),
            db_copy: None,  // Lazy load on first read
        })
    }
}

impl Snapshot {
    pub fn release(&self) {
        self.reader_count.fetch_sub(1, Ordering::SeqCst);
    }
}
```

---

### 5. **Connection Pooling** ⭐ MEDIUM PRIORITY

**Turso:** `core/connection.rs` — Pool management for concurrent access

**SQLitegraph gap:** Single connection, no pooling.

**Implement:**
```rust
// In sqlitegraph-core/src/pool.rs
use deadpool::managed::Pool;
use deadpool::managed::reusability::Reusability;

pub struct SqlConnectionManager {
    path: String,
}

impl Manager for SqlConnectionManager {
    type Type = rusqlite::Connection;
    type Error = rusqlite::Error;
    
    fn create(&self) -> Result<Self::Type, Self::Error> {
        rusqlite::Connection::connect(&self.path)
    }
    
    fn recycle(&self, conn: &mut Self::Type) -> Reusability {
        // Check if connection is still valid
        match conn.execute("SELECT 1", []) {
            Ok(_) => Reusability::Yes,
            Err(_) => Reusability::No,
        }
    }
}

pub type SqlitePool = Pool<SqlConnectionManager>;

impl GraphBackend for SqliteGraph {
    async fn insert_node_pooled(&self, node: &GraphEntity) -> Result<NodeId> {
        let mut conn = self.pool.get().await?;
        
        conn.execute(
            "INSERT INTO nodes (id, data) VALUES (?, ?)",
            [node.id(), serde_json::to_string(node)?()]
        )?;
        
        Ok(node.id())
    }
}
```

---

### 6. **Progress Tracking** ⭐ LOW PRIORITY

**Turso:** `core/progress.rs` — Callbacks for long-running ops

**SQLitegraph:** Has basic progress but no structured callbacks.

**Port:**
```rust
// In sqlitegraph-core/src/progress.rs
pub trait ProgressHandler {
    fn on_progress(&self, progress: &Progress);
    fn on_complete(&self, result: &Result);
}

pub struct Progress {
    pub current: usize,
    pub total: usize,
    pub message: String,
}

// In algorithms
impl GraphBackend for SqliteGraph {
    fn louvain_with_progress(&mut self, handler: Box<dyn ProgressHandler>) -> Result<Vec<Set<NodeId>>> {
        for iteration in 0..max_iterations {
            handler.on_progress(&Progress {
                current: iteration,
                total: max_iterations,
                message: format!("Louvain iteration {}", iteration),
            });
            
            let communities = self.louvain_step()?;
        }
        
        handler.on_complete(&Ok(communities));
        Ok(communities)
    }
}
```

---

### 7. **Type-Safe Parameter Binding** ⭐ MEDIUM PRIORITY

**Turso:** `bindings/rust/src/params.rs:42`
```rust
pub trait IntoParams {
    fn into_params(self) -> Vec<ParameterValue>;
}

impl IntoParams for (&str, i32) {
    fn into_params(self) -> Vec<ParameterValue> {
        vec![
            ParameterValue::Text(self.0.to_string()),
            ParameterValue::Integer(self.1),
        ]
    }
}
```

**SQLitegraph:** Manual `rusqlite` params, no type safety.

**Implement:**
```rust
// In sqlitegraph-core/src/params.rs
pub trait GraphParams {
    fn into_params(&self) -> Vec<rusqlite::types::Value>;
}

impl GraphParams for (&str, f64) {
    fn into_params(&self) -> Vec<rusqlite::types::Value> {
        vec![
            rusqlite::types::Value::Text(self.0.to_string()),
            rusqlite::types::Value::Real(self.1 as f64),
        ]
    }
}

// Usage
impl GraphBackend for SqliteGraph {
    fn query_neighbors(&self, id: NodeId, depth: usize) -> Result<Vec<NodeId>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT neighbor FROM edges WHERE source = ?1 AND depth <= ?2"
        )?;
        
        let mut rows = stmt.query((&id.to_string(), depth as i32))?;
        // ...
    }
}
```

---

## DON'T Adopt from Turso

### 1. **SQLite Fork Dependency**
**Turso:** Forks SQLite, maintains own `sqlite3/` dir.

**SQLitegraph:** Uses system `libsqlite3` via rusqlite.

**Reason:** Sticking with rusqlite =:
- Less maintenance burden
- Automatic SQLite updates
- Smaller dependency tree

### 2. **Limbo SQL Dialect**
**Turso:** Custom SQL dialect in `limbo/`.

**SQLitegraph:** Standard SQL only.

**Reason:** Better compatibility with tools, migration paths.

---

## Implementation Priority

### Phase 1: Async Backend (Week 1-2)
1. `AsyncSqliteBackend` with command channel
2. Worker thread management
3. Response channel integration
4. Tests for concurrency correctness

### Phase 2: Extension System (Week 3-4)
1. `GraphExtension` trait definition
2. Extension registry in `SqliteGraph`
3. Example extensions (Pagerank, Louvain)
4. Documentation for custom extensions

### Phase 3: MVCC Improvements (Week 5)
1. Snapshot version tracking
2. Lazy loading for large graphs
3. Reader count optimization
4. Stress tests for concurrent reads

### Phase 4: Connection Pooling (Week 6)
1. `deadpool` integration
2. Pool configuration
3. Reusability checks
4. Benchmarks vs single connection

---

## Success Criteria

- ✅ Async backend passes all existing sync tests
- ✅ Extension system allows custom algorithms
- ✅ MVCC snapshots are reusable across operations
- ✅ Connection pool improves concurrent performance
- ✅ All changes maintain `Result<T>` error handling (no unwrap())
- ✅ No new unsafe blocks
- ✅ cargo fmt + cargo clippy pass

---

## Sources

- [Turso crate on crates.io](https://crates.io/crates/turso)
- [Turso GitHub repository](https://github.com/tursodatabase/turso)
- [sqlitegraph repository](https://github.com/oldnordic/sqlitegraph)
