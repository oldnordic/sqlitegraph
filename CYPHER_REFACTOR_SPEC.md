# Cypher Backend-Agnostic Refactor - Specification

## Problem Statement

Cypher query module (`src/cypher.rs`) is hardcoded to `SqliteGraphBackend`, preventing CLI cypher queries from working with native-v3 backend.

## Current Architecture

**Cypher entry point:**
```rust
pub fn execute(backend: &SqliteGraphBackend, query: &CypherQuery) -> Result<Value, String>
```

**Dependencies identified:**
1. **SQLite leaks**: 4 sites calling `backend.graph()` returning concrete `&SqliteGraph`
2. **Pattern coupling**: `match_triples(&SqliteGraph)` not generic over GraphBackend
3. **HNSW access**: `graph.get_hnsw_index_ref()` not on GraphBackend trait

**Scope:** 13 functions need signature changes, 3 missing trait methods

## Solution Architecture

### Phase 1: GraphBackend Trait Extensions

Add missing methods to `GraphBackend` trait:

```rust
trait GraphBackend {
    // Existing methods (unchanged)
    fn fetch_outgoing(&self, node: i64) -> Result<Vec<i64>, SqliteGraphError>;
    // ... 30+ existing methods
    
    // NEW: Pattern matching support
    fn match_triples(&self, pattern: &PatternTriple) -> Result<Vec<TripleMatch>, SqliteGraphError>;
    
    // NEW: HNSW vector search
    fn vector_search(&self, snapshot: SnapshotId, index_name: &str, vector: &[f32], k: usize) -> Result<Vec<(i64, f32)>, SqliteGraphError>;
    
    // NEW: Graph introspection (replaces backend.graph())
    fn get_graph_ref(&self) -> Option<&SqliteGraph>;
}
```

**Rationale:**
- `match_triples`: Moves pattern matching capability to trait level
- `vector_search`: Exposes HNSW search capability uniformly
- `get_graph_ref`: Returns `Option<&SqliteGraph>` for SQLite, `None` for native-v3

### Phase 2: Pattern Engine Refactor

**Current pattern_engine signature:**
```rust
pub fn match_triples(graph: &SqliteGraph, pattern: &PatternTriple) -> Result<Vec<TripleMatch>>
```

**Options:**
1. **Wrapper approach**: Create `match_triples_generic(&impl GraphBackend)` wrapper
2. **Refactor approach**: Make pattern_engine consume `&impl GraphBackend`

**Decision:** Wrapper approach (lower risk)
- pattern_engine remains unchanged
- cypher module calls wrapper
- native-v3 gets its own match_triples implementation

### Phase 3: Cypher Signature Migration

**Change all function signatures:**
```rust
// BEFORE
fn execute_match(backend: &SqliteGraphBackend, query: &CypherQuery) -> Result<Value, String>

// AFTER  
fn execute_match(backend: &impl GraphBackend, query: &CypherQuery) -> Result<Value, String>
```

**Affected functions (13):**
- execute (main dispatcher)
- execute_match, execute_node_match, execute_edge_match
- execute_multi_hop, execute_star, execute_variable_depth
- execute_create_node, execute_create_edge
- execute_set, execute_delete
- execute_call_vector_query
- collect_match_targets

### Phase 4: CLI Backend Dispatch

**Current CLI query runner:**
```rust
fn run_query(client: &CliClient, query_str: &str) -> Result<()> {
    let sqlite_backend = client.sqlite_backend()?;  // SQLite only
    let result = sqlitegraph_cli::query::run(sqlite_backend, query_str)?;
}
```

**New dispatch logic:**
```rust
fn run_query(client: &CliClient, query_str: &str) -> Result<()> {
    match client.backend_type() {
        BackendType::Sqlite => {
            let backend = client.sqlite_backend()?;
            run_query_sqlite(backend, query_str)
        }
        BackendType::V3 => {
            let backend = client.v3_backend()?;
            run_query_v3(backend, query_str)
        }
    }
}
```

## Implementation Strategy (Karpathy Incremental)

### Step 1: Trait Extensions (No Breaking Changes)
- Add 3 new methods to GraphBackend trait
- Implement for SqliteGraphBackend (delegates to existing code)
- Stub for native-v3 (returns "not supported" errors)
- **Verify:** `cargo check --all-targets` passes

### Step 2: Cypher Signature Migration (Breaking Change)
- Change 13 function signatures to `&impl GraphBackend`
- Fix 4 SQLite-leak sites to use `get_graph_ref()`
- Replace direct HNSW calls with trait method
- **Verify:** `cargo test --lib` passes

### Step 3: CLI Dispatch Layer
- Add backend detection to CliClient
- Implement run_query_sqlite and run_query_v3
- **Verify:** Manual CLI test with `--backend sqlite` and `--backend v3`

### Step 4: Native-v3 Implementation
- Implement `match_triples` for native-v3
- Implement `vector_search` for native-v3  
- **Verify:** Integration tests pass

## Risk Assessment

**High-risk changes:**
- Trait method additions: breaking change for all backend implementers
- Cypher signature changes: affects all callers

**Mitigation:**
- Default trait implementations where possible
- Incremental verification at each step
- Keep SQLite backend fully functional throughout

## Success Criteria

1. ✅ `cargo check --all-targets` compiles both backends
2. ✅ `cargo test --lib` passes all tests
3. ✅ CLI `--backend sqlite` query works (existing functionality preserved)
4. ✅ CLI `--backend v3` query works (new functionality)
5. ✅ No breaking changes to existing SQLite cypher usage

## Open Questions

1. Should native-v3 implement full pattern matching or return "not supported"?
2. Do we need version bump for trait changes?
3. What's native-v3 HNSW integration status?