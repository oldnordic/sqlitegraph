# Phase 08: Graph Algorithms - Discovery

## Goal
Add centrality measures and community detection algorithms to SQLiteGraph.

## Research: Algorithm Selection and Implementation Approaches

### Centrality Algorithms (Plan 08-01)

**1. PageRank**
- **Purpose**: Measure node importance based on link structure
- **Standard Algorithm**: Iterative computation with damping factor
- **Implementation Approach**:
  - Power iteration method (standard approach)
  - Damping factor: 0.85 (typical default)
  - Convergence threshold: 1e-6 or fixed iterations (20-50)
  - Teleportation handling for dangling nodes
- **Complexity**: O(k * |E|) where k = iterations
- **Memory**: Need to store scores for all nodes (Vec<f64>)
- **Tradeoff**: Fixed iterations vs convergence check (simpler vs more accurate)

**2. Betweenness Centrality**
- **Purpose**: Find nodes that act as bridges between graph regions
- **Standard Algorithm**: Brandes' algorithm (O(|V| * |E|))
- **Implementation Approach**:
  - Brandes' algorithm for unweighted graphs
  - For large graphs: approximate via sampled shortest paths
  - BFS-based accumulation from each node
- **Complexity**: O(|V| * |E|) - expensive for large graphs
- **Memory**: Need temporary storage for BFS traversal and accumulation
- **Tradeoff**: Exact vs approximate (required for >1K nodes)

**Decision**: Use exact Brandes for <1K nodes, sampling approximation for larger

### Community Detection (Plan 08-02)

**3. Louvain Method**
- **Purpose**: Detect communities by optimizing modularity
- **Standard Algorithm**: Iterative modularity optimization
- **Implementation Approach**:
  - Phase 1: Move nodes to maximize local modularity
  - Phase 2: Aggregate communities into super-nodes
  - Repeat until modularity stops improving
  - Simplified version (no multi-level aggregation for v1)
- **Complexity**: O(|V| log |V|) per pass
- **Memory**: Need community assignment and delta modularity tracking
- **Tradeoff**: Full multi-level vs single-pass (simpler for initial implementation)

**Decision**: Single-pass modularity optimization for MVP

**4. Label Propagation**
- **Purpose**: Fast community detection using neighbor labels
- **Standard Algorithm**: Iterate label updates until convergence
- **Implementation Approach**:
  - Initialize each node with unique label
  - Update label to most frequent neighbor label
  - Random iteration order to avoid bias
  - Convergence: no labels change or max iterations
- **Complexity**: Near-linear O(|E|) per iteration
- **Memory**: Just label storage per node
- **Tradeoff**: Deterministic vs random tiebreaking

**Decision**: Use deterministic tiebreaking (lowest ID) for reproducibility

### Implementation Constraints

**From Codebase Analysis**:
- Existing `algo.rs` module with 106 lines
- Uses `ahash::AHashSet` and `std::collections::VecDeque`
- Graph access via `fetch_outgoing(id)` and `fetch_incoming(id)`
- Returns `Result<T, SqliteGraphError>`
- Public API: functions take `&SqliteGraph` reference

**Data Structure Requirements**:
- PageRank: `HashMap<i64, f64>` or `Vec<(i64, f64)>` for scores
- Betweenness: `HashMap<i64, f64>` for centrality values
- Louvain: `HashMap<i64, i64>` for community assignments
- Label Prop: `HashMap<i64, i64>` for labels

**Integration Points**:
- Add to `src/algo.rs` (extend existing module)
- Export public functions in `src/lib.rs`
- Tests in `tests/algo_tests.rs` (extend existing file)
- Benchmarks in `benches/algo_benchmarks.rs` (new file)

## API Design

### PageRank
```rust
pub fn pagerank(
    graph: &SqliteGraph,
    damping: f64,      // 0.85 default
    iterations: usize, // 20-50 typical
) -> Result<Vec<(i64, f64)>, SqliteGraphError>
```

### Betweenness Centrality
```rust
pub fn betweenness_centrality(
    graph: &SqliteGraph,
    sample_size: Option<usize>, // None = exact, Some(n) = approximate
) -> Result<Vec<(i64, f64)>, SqliteGraphError>
```

### Louvain Communities
```rust
pub fn louvain_communities(
    graph: &SqliteGraph,
    max_iterations: usize, // convergence guard
) -> Result<Vec<Vec<i64>>, SqliteGraphError>
```

### Label Propagation
```rust
pub fn label_propagation(
    graph: &SqliteGraph,
    max_iterations: usize, // convergence guard
) -> Result<Vec<Vec<i64>>, SqliteGraphError>
```

## Performance Considerations

**Existing Patterns from Codebase**:
- Uses `ahash` for hash maps (faster than std)
- BFS uses `VecDeque` for queue
- Cache-aware adjacency access via `fetch_outgoing/fetch_incoming`
- In-memory graphs: no I/O cost
- Native V2: direct memory access for adjacency

**Algorithm-Specific Optimizations**:
- PageRank: Can use sparse representation (most scores = 0 initially)
- Betweenness: Parallelizable (each BFS is independent)
- Louvain: Early termination on small delta changes
- Label Prop: Asynchronous updates (like gossip protocol)

## Testing Strategy

**Unit Tests** (extend `algo_tests.rs`):
- Small synthetic graphs with known answers
- Cycle, line, star, complete graph patterns
- Verify deterministic output

**Integration Tests**:
- Larger graphs (100-1000 nodes)
- Performance regression detection
- Edge cases: empty graph, single node, disconnected components

**Benchmarks** (new `algo_benchmarks.rs`):
- Vary graph sizes: 10, 100, 1K, 10K nodes
- Different topologies: random, scale-free, small-world
- Criterion framework for regression detection

## Dependencies

**External**: None (pure Rust implementation)
**Internal**:
- `crate::graph::SqliteGraph` for graph access
- `crate::errors::SqliteGraphError` for error handling
- `ahash` for hash maps (already in dependency tree)

## Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Betweenness O(VE) too slow | Offer sampling approximation |
| Louvain multi-level complexity | Start with single-pass |
| PageRank convergence issues | Use fixed iterations with damping |
| Float precision issues | Use f64, document epsilon comparisons |

## Next Steps

1. **Plan 08-01**: Centrality (PageRank, Betweenness) - Parallel candidates
2. **Plan 08-02**: Community Detection (Louvain, Label Prop) - Parallel candidates
3. **Plan 08-03**: Benchmarks and comprehensive tests - Depends on 01, 02

**Wave Structure**:
- Wave 1 (parallel): 08-01 (Centrality), 08-02 (Community Detection)
- Wave 2: 08-03 (Benchmarks & Tests)
