# Cypher Backend-Agnostic Implementation Plan

## Phase 1: Trait Extensions (Foundation)

### Step 1.1: Add methods to GraphBackend trait

**File:** `sqlitegraph-core/src/backend.rs`

**Add to trait definition (after line 115):**
```rust
// NEW: Pattern matching support
fn match_triples(&self, pattern: &PatternTriple) -> Result<Vec<TripleMatch>, SqliteGraphError> {
    // Default implementation for backends without pattern matching
    Err(SqliteGraphError::UnsupportedOperation("match_triples not implemented"))
}

// NEW: HNSW vector search
fn vector_search(&self, snapshot: SnapshotId, index_name: &str, vector: &[f32], k: usize) -> Result<Vec<(i64, f32)>, SqliteGraphError> {
    // Default implementation for backends without HNSW
    Err(SqliteGraphError::UnsupportedOperation("vector_search not implemented"))
}

// NEW: Graph introspection
fn get_graph_ref(&self) -> Option<&SqliteGraph> {
    None  // Default for non-SQLite backends
}
```

**Verification:** `cargo check --all-targets`

### Step 1.2: Implement for SqliteGraphBackend

**File:** `sqlitegraph-core/src/backend/sqlite/impl_.rs`

**Add implementation:**
```rust
impl GraphBackend for SqliteGraphBackend {
    // ... existing implementations ...
    
    fn match_triples(&self, pattern: &PatternTriple) -> Result<Vec<TripleMatch>, SqliteGraphError> {
        crate::match_triples(&self.graph, pattern)
    }
    
    fn vector_search(&self, snapshot: SnapshotId, index_name: &str, vector: &[f32], k: usize) -> Result<Vec<(i64, f32)>, SqliteGraphError> {
        let graph = self.graph();
        let hnsw = graph.get_hnsw_index_ref(snapshot, index_name)
            .map_err(|e| SqliteGraphError::Other(e.to_string()))?;
        hnsw.search(vector, k)
            .map(|results| results.into_iter().map(|(id, dist)| (id as i64, dist)).collect())
            .map_err(|e| SqliteGraphError::Other(e.to_string()))
    }
    
    fn get_graph_ref(&self) -> Option<&SqliteGraph> {
        Some(&self.graph)
    }
}
```

**Verification:** `cargo check --all-targets`, `cargo test --lib`

### Step 1.3: Stub native-v3 implementations

**File:** `sqlitegraph-core/src/backend/native/impl_.rs`

**Add stub implementations:**
```rust
fn match_triples(&self, pattern: &PatternTriple) -> Result<Vec<TripleMatch>, SqliteGraphError> {
    Err(SqliteGraphError::UnsupportedOperation("match_triples not supported on native-v3"))
}

fn vector_search(&self, _snapshot: SnapshotId, _index_name: &str, _vector: &[f32], _k: usize) -> Result<Vec<(i64, f32)>, SqliteGraphError> {
    Err(SqliteGraphError::UnsupportedOperation("vector_search not supported on native-v3"))
}

fn get_graph_ref(&self) -> Option<&SqliteGraph> {
    None
}
```

**Verification:** `cargo check --all-targets --features native-v3`

## Phase 2: Cypher Signature Migration

### Step 2.1: Change execute function signature

**File:** `sqlitegraph-core/src/cypher.rs`

**Line 1188:**
```rust
// BEFORE
pub fn execute(backend: &SqliteGraphBackend, query: &CypherQuery) -> Result<Value, String>

// AFTER
pub fn execute<B: GraphBackend>(backend: &B, query: &CypherQuery) -> Result<Value, String>
```

### Step 2.2: Change all executor signatures

**Lines to update:** 1209, 1226, 1273, 1339, 1407, 1586, 1668, 1699, 1716, 1770, 1784, 1808

**Pattern:**
```rust
// BEFORE
fn execute_match(backend: &SqliteGraphBackend, query: &CypherQuery) -> Result<Value, String>

// AFTER
fn execute_match<B: GraphBackend>(backend: &B, query: &CypherQuery) -> Result<Value, String>
```

### Step 2.3: Fix SQLite-leak sites

**Line 1280, 1344, 1417, 1790:** Replace `backend.graph()` calls
```rust
// BEFORE
let graph = backend.graph();

// AFTER
let graph = backend.get_graph_ref().ok_or_else(|| "Graph introspection failed".to_string())?;
```

### Step 2.4: Fix pattern_engine call

**Line 1302:** Replace match_triples call
```rust
// BEFORE
let matches = crate::match_triples(&backend.graph(), &pattern)?;

// AFTER
let matches = backend.match_triples(&pattern)?;
```

### Step 2.5: Fix HNSW call

**Line 1792:** Replace HNSW call
```rust
// BEFORE
let hnsw = backend.get_hnsw_index_ref(snapshot, index_name)?;

// AFTER
let results = backend.vector_search(snapshot, index_name, vector, k)?;
```

**Verification:** `cargo test --lib -- --test-threads=1` (cypher tests only)

## Phase 3: CLI Backend Dispatch

### Step 3.1: Add backend detection to CliClient

**File:** `sqlitegraph-cli/src/client.rs`

**Add method:**
```rust
impl CliClient {
    pub fn backend_type(&self) -> BackendType {
        match self {
            CliClient::Sqlite(_) => BackendType::Sqlite,
            #[cfg(feature = "native-v3")]
            CliClient::V3(_) => BackendType::V3,
        }
    }
}
```

### Step 3.2: Create split query runners

**File:** `sqlitegraph-cli/src/main.rs`

**Split run_query:**
```rust
fn run_query(client: &CliClient, query_str: &str) -> Result<()> {
    match client.backend_type() {
        BackendType::Sqlite => run_query_sqlite(client, query_str),
        #[cfg(feature = "native-v3")]
        BackendType::V3 => run_query_v3(client, query_str),
    }
}

fn run_query_sqlite(client: &CliClient, query_str: &str) -> Result<()> {
    let sqlite_backend = client.sqlite_backend()
        .context("Cypher queries require SQLite backend")?;
    let result = sqlitegraph_cli::query::run(sqlite_backend, query_str)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

#[cfg(feature = "native-v3")]
fn run_query_v3(client: &CliClient, query_str: &str) -> Result<()> {
    let v3_backend = client.v3_backend()
        .context("Cypher queries require native-v3 backend")?;
    let result = sqlitegraph_cli::query::run(v3_backend, query_str)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}
```

### Step 3.3: Update CLI query module

**File:** `sqlitegraph-cli/src/query.rs`

**Make run function generic:**
```rust
use sqlitegraph::backend::GraphBackend;

pub fn run<B: GraphBackend>(backend: &B, query_str: &str) -> anyhow::Result<Value> {
    let query = sqlitegraph::cypher::parse(query_str)
        .map_err(|e| anyhow::anyhow!("parse error: {e}"))?;
    let result = sqlitegraph::cypher::execute(backend, &query)
        .map_err(|e| anyhow::anyhow!("execution error: {e}"))?;
    Ok(result)
}
```

**Verification:** 
- `cargo build --release -p sqlitegraph-cli`
- Test with `sqlitegraph --backend sqlite query "..."`
- Test with `sqlitegraph --backend v3 query "..."`

## Phase 4: Testing & Verification

### Step 4.1: Update cypher tests

**File:** `sqlitegraph-core/tests/cypher_tests.rs`

**Ensure tests work with both backends:**
```rust
#[test]
fn test_cypher_sqlite_backend() {
    let backend = SqliteGraphBackend::in_memory().unwrap();
    test_cypher_queries(&backend);
}

#[cfg(feature = "native-v3")]
#[test]  
fn test_cypher_native_v3_backend() {
    let backend = NativeBackend::in_memory().unwrap();
    // Test basic queries (complex patterns may not work yet)
    test_basic_cypher_queries(&backend);
}
```

### Step 4.2: Manual CLI testing

**SQLite test:**
```bash
echo "CREATE (n:User {name: 'Alice'})" | sqlitegraph --db /tmp/test.db --backend sqlite query -
echo "MATCH (n:User) RETURN n.name" | sqlitegraph --db /tmp/test.db --backend sqlite query -
```

**Native-v3 test:**
```bash
echo "MATCH (n) RETURN n LIMIT 5" | sqlitegraph --db /tmp/test.graph --backend v3 query -
```

## Phase 5: Documentation Updates

### Step 5.1: Update CLI documentation

**Files:** `README.md`, `sqlitegraph-cli/README.md`

**Add backend flag documentation:**
```bash
sqlitegraph --db graph.db --backend sqlite query "MATCH (n) RETURN n"
sqlitegraph --db graph.db --backend v3 query "MATCH (n) RETURN n"
```

### Step 5.2: Update changelog

**File:** `CHANGELOG.md`

**Add entry:**
```markdown
## [X.Y.Z] - YYYY-MM-DD

### Added
- Cypher queries now support both SQLite and native-v3 backends via --backend flag
- GraphBackend trait extended with match_triples, vector_search, and get_graph_ref methods

### Changed  
- Cypher module signatures now accept generic GraphBackend trait instead of SqliteGraphBackend
- CLI query runner dispatches to appropriate backend based on --backend flag
```

## Rollback Plan

If Step 2 breaks existing functionality:

```bash
git checkout HEAD~1 sqlitegraph-core/src/cypher.rs
git checkout HEAD~1 sqlitegraph-core/src/backend.rs
```

If Step 3 breaks CLI:

```bash
git checkout HEAD~1 sqlitegraph-cli/src/main.rs
git checkout HEAD~1 sqlitegraph-cli/src/query.rs
```

## Success Metrics

1. ✅ All existing SQLite cypher tests pass
2. ✅ CLI queries work with `--backend sqlite` (no regression)
3. ✅ CLI queries work with `--backend v3` (new feature)
4. ✅ `cargo test --lib` passes for both `--no-default-features` and `--features native-v3`
5. ✅ No breaking changes to external API (only internal signature changes)

## Timeline Estimate

- Phase 1 (Trait): 30 min
- Phase 2 (Cypher): 1 hour
- Phase 3 (CLI): 30 min
- Phase 4 (Testing): 30 min
- Phase 5 (Docs): 15 min
**Total: ~3 hours**