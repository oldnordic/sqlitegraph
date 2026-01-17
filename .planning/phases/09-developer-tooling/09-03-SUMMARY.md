# Plan 09-03 Summary: CLI Debug Commands

## Accomplishments

### Task 1: debug-stats Command
Added `debug-stats` CLI command that outputs structured JSON introspection data:
- Backend type (sqlite/native)
- Node count
- Edge count (exact for <10K, estimated for larger graphs)
- Cache statistics (hits, misses, hit ratio)
- File sizes (database and WAL)
- In-memory detection

**Verification:**
```bash
$ sqlitegraph debug-stats
{
  "backend_type": "sqlite",
  "node_count": 0,
  "edge_count": { "Exact": 0 },
  "cache_stats": { "hits": 0, "misses": 0, "entries": 0 },
  "file_size": 114688,
  "wal_size": null,
  "is_in_memory": false
}
```

### Task 2: debug-dump Command
Added `debug-dump` CLI command for exporting graph structure:
- Supports JSONL format (streaming, memory efficient for large graphs)
- Supports JSON array format (for small graphs <1000 nodes)
- Exports all nodes with properties
- Exports all edges with metadata
- Validates output path is writable

**Usage:**
```bash
sqlitegraph debug-dump --output /tmp/graph.jsonl
sqlitegraph debug-dump --output /tmp/graph.json --format json
```

**Output Format:**
```json
{"type": "node", "id": 123, "kind": "Person", "name": "Alice", "file_path": null, "data": {}}
{"type": "edge", "id": 456, "from": 123, "to": 789, "edge_type": "KNOWS", "data": {}}
```

### Task 3: Progress Bars and Algorithm Commands

**Progress for Existing Commands:**
- `bfs`: Shows "BFS: starting from node X" and completion message
- `k-hop`: Shows "K-hop: processing depth X" and completion message
- `shortest-path`: Shows "Shortest path: searching from X to Y" and completion message

**New Algorithm Commands with Progress:**
1. `pagerank --iterations N [--damping-factor F]`
   - PageRank centrality algorithm
   - Shows iteration progress via ConsoleProgress
   - Default damping factor: 0.85

2. `betweenness`
   - Betweenness centrality algorithm (computes for all nodes)
   - Shows per-source progress via ConsoleProgress
   - Returns top 10 nodes by centrality

3. `louvain [--max-iterations N]`
   - Louvain community detection algorithm
   - Shows iteration progress via ConsoleProgress
   - Default max iterations: 100
   - Returns communities (up to 10 shown in output)

**Progress Output Example:**
```
BFS: starting from node 1
BFS: visited nodes [5/5]
Complete
```

All progress output goes to stderr, keeping stdout free for JSON data output.

### Task 4: debug-trace Command
Added `debug-trace` command that:
- Enables RUST_LOG=debug for command duration
- Re-runs the specified command with trace logging
- Useful for debugging algorithm behavior

**Usage:**
```bash
sqlitegraph debug-trace bfs --start 1 --max-depth 2
```

## Issues Encountered

1. **GraphEdge struct field naming**: Initial implementation used `edge.kind` but the actual field is `edge.edge_type`. Fixed by checking the struct definition.

2. **Algorithm function signatures**: Initial implementation had incorrect parameters:
   - `betweenness_centrality_with_progress` doesn't take a sample_size parameter
   - `louvain_communities_with_progress` takes `max_iterations: usize` not `resolution: f64`
   Fixed by reading the actual function signatures in algo.rs.

3. **ProgressCallback trait not in scope**: ConsoleProgress methods weren't accessible without importing ProgressCallback trait. Fixed by updating imports to include both `ConsoleProgress` and `ProgressCallback`.

## Deviations from Plan

1. **Simplified progress reporting for BFS/k-hop/shortest-path**: Instead of creating instrumented variants, added simple progress messages before/after algorithm execution. The algorithms complete quickly enough that detailed progress isn't necessary.

2. **No --format flag for debug-stats**: Plan mentioned optional --format (json|table) flag, but kept it simple with JSON only since that's the primary use case for LLM consumption.

3. **betweenness command parameters**: Plan mentioned `--sample-size` parameter, but the actual algorithm doesn't support sampling - it computes for all nodes. Removed the parameter.

## Next Phase Readiness

Phase 9 (Developer Tooling) is now **COMPLETE** with all 3 plans finished:
- ✅ Plan 09-01: Introspection APIs
- ✅ Plan 09-02: Algorithm Progress Tracking
- ✅ Plan 09-03: CLI Debug Commands

**Ready for Phase 10: Testing & Documentation**
- Invariants and guarantees documentation
- API documentation
- Testing coverage
- Performance validation

## Commits

1. `16cf5bc`: feat(cli): add debug-stats, debug-dump, and debug-trace commands
2. `3aab659`: feat(cli): add progress bars to algorithm commands and new algorithm CLI commands
