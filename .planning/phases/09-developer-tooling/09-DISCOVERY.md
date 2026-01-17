# Phase 09: Developer Tooling - Discovery

## Goal
Add debugging, profiling, and introspection utilities for SQLiteGraph.

## Research: Introspection and Debugging APIs

### Current State Analysis

**Existing Debug Infrastructure:**
- `debug.rs`: Centralized debug logging with feature flag control (`debug` feature)
- `graph/metrics/`: SQLite operation metrics (prepares, executes, transactions, cache hits/misses)
- `cache.rs`: AdjacencyCache with stats() method (hits, misses, entries)
- `cache_stats()`: Public API for cache statistics

**Existing CLI Commands:**
- status, list, migrate, dump-graph, load-graph
- Traversal commands: bfs, k-hop, shortest-path, neighbors
- Pattern matching: pattern-match, pattern-match-fast
- HNSW commands: hnsw-create, hnsw-insert, hnsw-search, hnsw-stats, hnsw-list, hnsw-delete, hnsw-info
- WAL commands: wal-checkpoint, wal-metrics, wal-config, wal-stats
- Snapshot commands: snapshot-create, snapshot-load

### Developer Tooling Gaps

**1. Algorithm Progress Tracking (LLM Feedback)**
- No visibility into long-running algorithm execution
- PageRank, Betweenness Centrality can take minutes on large graphs
- Louvain method iterations have no progress reporting
- Need: Progress callbacks, iteration counts, estimated completion

**2. Introspection APIs for Internal State**
- No access to internal graph statistics
- Cannot inspect cache contents/behavior programmatically
- No visibility into Native V2 backend internals
- Need: Structured introspection API

**3. Performance Profiling Hooks**
- No built-in flamegraph/profiling integration
- Criterion benchmarks exist but require external tools
- No runtime performance monitoring
- Need: Profiling integration, performance snapshots

**4. Debugging Utilities**
- No graph structure inspection tools
- No entity/edge relationship visualization helpers
- No memory usage introspection
- Need: Debug queries, structure inspection

### Implementation Approach

**Plan 09-01: Introspection APIs**
- Create `introspection` module with structured introspection
- Add `GraphIntrospection` trait with backend-specific implementations
- Expose: node count, edge count, cache stats, backend type, file size
- JSON-serializable for LLM consumption

**Plan 09-02: Algorithm Progress Tracking**
- Add progress callback trait for long-running algorithms
- Implement iteration reporting for iterative algorithms
- Add cancellation tokens for expensive operations
- CLI integration: show progress bars for operations

**Plan 09-03: Debug CLI Commands**
- `debug-stats`: Show comprehensive introspection data
- `debug-dump`: Export graph structure for debugging
- `debug-trace`: Enable trace logging for operations
- `debug-profile`: Run operation with profiling

### Technical Decisions

**1. Introspection API Design**
```rust
pub struct GraphIntrospection {
    pub backend_type: String,
    pub node_count: usize,
    pub edge_count: usize,
    pub cache_stats: CacheStats,
    pub memory_usage: Option<usize>,  // If available
    pub file_size: Option<u64>,       // For file-based backends
}
```

**2. Progress Callback Trait**
```rust
pub trait ProgressCallback: Send + Sync {
    fn on_progress(&self, current: usize, total: Option<usize>, message: &str);
    fn on_complete(&self);
    fn on_error(&self, error: &SqliteGraphError);
}
```

**3. Profiling Integration**
- Use `pprof` crate for flamegraph generation (Rust standard)
- Criterion benchmarks already exist - add developer-friendly wrapper
- Feature-gated profiling (`profiling` feature)

**4. LLM Feedback Considerations**
- Progress updates should be structured (JSON for LLM parsing)
- Introspection data should be self-documenting
- Include estimated completion times when possible

### Dependencies

**External:**
- `pprof` or `flamegraph` crate for profiling (dev-dependency)
- `indicatif` for progress bars in CLI

**Internal:**
- `crate::graph::SqliteGraph` for graph access
- `crate::cache::CacheStats` for cache introspection
- `crate::errors::SqliteGraphError` for error handling
- Existing metrics infrastructure

### API Design

#### Introspection API
```rust
impl SqliteGraph {
    pub fn introspect(&self) -> Result<GraphIntrospection, SqliteGraphError> {
        // Return comprehensive introspection data
    }

    pub fn cache_stats(&self) -> CacheStats {
        // Return cache statistics
    }
}
```

#### Algorithm Progress API
```rust
pub struct NoProgress;  // Default no-op callback

impl ProgressCallback for NoProgress {
    fn on_progress(&self, _current: usize, _total: Option<usize>, _message: &str) {}
    fn on_complete(&self) {}
    fn on_error(&self, _error: &SqliteGraphError) {}
}

pub fn pagerank_with_progress<F>(
    graph: &SqliteGraph,
    damping: f64,
    iterations: usize,
    progress: &F
) -> Result<Vec<(i64, f64)>, SqliteGraphError>
where
    F: ProgressCallback,
```

#### CLI Debug Commands
```bash
sqlitegraph debug-stats           # Show introspection data (JSON)
sqlitegraph debug-dump --output   # Export graph structure
sqlitegraph debug-trace bfs ...   # Run with trace logging
sqlitegraph debug-profile pagerank --iterations 100
```

### Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Performance overhead of introspection | Feature-gated, lazy evaluation |
| Progress callback complexity | Simple trait, default no-op impl |
| Profiling tools compatibility | Use standard Rust ecosystem tools |
| LLM parsing complexity | Structured JSON output |

## Next Steps

1. **Plan 09-01**: Introspection APIs (no dependencies, Wave 1 candidate)
2. **Plan 09-02**: Algorithm Progress Tracking (independent, Wave 1 candidate)
3. **Plan 09-03**: Debug CLI Commands (depends on 01, 02, Wave 2)

**Wave Structure:**
- Wave 1 (parallel): 09-01 (Introspection), 09-02 (Progress)
- Wave 2: 09-03 (CLI commands using 01, 02)
