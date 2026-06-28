# Native-v3 Algorithm Compatibility Matrix

Documenting which graph algorithms work with native-v3 backend and which need updates.

## Backend-Agnostic Algorithms (Fully Compatible)

The following algorithms use the `GraphBackend` trait and work with **all backends** (SQLite, Native V2, Native V3):

| Algorithm | Works with V3 | Notes |
|-----------|----------------|-------|
| `pagerank` | ✅ Yes | Uses `GraphBackend::nodes()`, `GraphBackend::outgoing_edges()` |
| `betweenness_centrality` | ✅ Yes | Uses BFS traversal via `GraphBackend::outgoing_edges()` |
| `bfs` | ✅ Yes | Uses `GraphBackend::outgoing_edges()` |
| `shortest_path` | ✅ Yes | Uses `GraphBackend::outgoing_edges()` with Dijkstra |
| `strongly_connected_components` | ✅ Yes | Uses `GraphBackend::nodes()`, `GraphBackend::outgoing_edges()` |
| `topological_sort` | ✅ Yes | Uses `GraphBackend::nodes()`, `GraphBackend::outgoing_edges()` |
| `connected_components` | ✅ Yes | Uses `GraphBackend::nodes()`, `GraphBackend::outgoing_edges()` |
| `dfs_traversal` | ✅ Yes | Uses `GraphBackend::outgoing_edges()` |
| `k_hop_neighbors` | ✅ Yes | Uses `GraphBackend::outgoing_edges()` |

## High-Level Algorithms (Graph API Compatible)

These algorithms use the high-level `SqliteGraph` API and work with native-v3:

| Category | Algorithm | Works with V3 | Notes |
|----------|-----------|----------------|-------|
| **Centrality** | `pagerank` | ✅ Yes | Uses graph iterators |
| **Centrality** | `betweenness_centrality` | ✅ Yes | Uses graph traversal |
| **Community** | `label_propagation` | ✅ Yes | Uses graph structure |
| **Community** | `louvain_communities` | ✅ Yes | Uses graph structure |
| **Structure** | `connected_components` | ✅ Yes | Uses graph structure |
| **Structure** | `strongly_connected_components` | ✅ Yes | Uses Tarjan's algorithm |
| **Structure** | `weakly_connected_components` | ✅ Yes | Undirected view |
| **Structure** | `cycle_basis` | ✅ Yes | Paton's algorithm |
| **Structure** | `topological_sort` | ✅ Yes | Kahn's algorithm |
| **Reachability** | `transitive_closure` | ✅ Yes | BFS-based |
| **Reachability** | `transitive_reduction` | ✅ Yes | Graph simplification |
| **Reachability** | `reachable_from` | ✅ Yes | BFS traversal |
| **Dependency** | `critical_path` | ✅ Yes | DAG longest path |
| **Control Flow** | `dominators` | ✅ Yes | CFG analysis |
| **Control Flow** | `post_dominators` | ✅ Yes | Reverse CFG |
| **Control Flow** | `dominance_frontiers` | ✅ Yes | SSA construction |
| **Control Flow** | `natural_loops` | ✅ Yes | Loop detection |
| **Program** | `backward_slice` | ✅ Yes | Impact analysis |
| **Program** | `forward_slice` | ✅ Yes | Impact analysis |
| **Call Graph** | `collapse_sccs` | ✅ Yes | Condensation DAG |
| **Partition** | `partition_bfs_level` | ✅ Yes | Graph sharding |
| **Partition** | `partition_greedy` | ✅ Yes | Cut optimization |
| **Partition** | `partition_kway` | ✅ Yes | Multi-way partitioning |
| **Path** | `enumerate_paths` | ✅ Yes | DFS with bounds |
| **Security** | `propagate_taint_forward` | ✅ Yes | Taint analysis |
| **Security** | `sink_reachability_analysis` | ✅ Yes | Vulnerability detection |
| **ML** | `find_subgraph_patterns` | ✅ Yes | VF2 algorithm |
| **ML** | `structural_similarity` | ✅ Yes | Graph similarity |
| **ML** | `rewrite_graph_patterns` | ✅ Yes | Graph rewriting |
| **Diff** | `graph_diff` | ✅ Yes | Structural delta |
| **Observability** | `happens_before_analysis` | ✅ Yes | Vector clocks |
| **Observability** | `impact_radius` | ✅ Yes | Blast zones |

## Native-V3 Specific Features

| Feature | Status | Integration |
|---------|--------|------------|
| **CSR Sharding** | ✅ Complete | `sharding::CsrShard`, `sharding::ShardReader` |
| **Subgraph Builder** | ✅ Complete | `sharding::SubgraphBuilder` |
| **Bidirectional Index** | ✅ Complete | `sharding::BidirectionalIndex` |
| **HNSW Semantic Layer** | ✅ Complete | `sharding::SemanticLayer` (uses HNSW backend) |
| **Property Store** | ✅ Complete | `sharding::PropertyStore` (SQLite-backed) |
| **Pub/Sub Layer** | ✅ Complete | `sharding::PubSub` (WAL-backed) |

## Algorithm + Native-V3 Integration Patterns

### Pattern 1: CSR-Aware Traversal

For algorithms that benefit from locality (BFS, partitioning):

```rust
use sqlitegraph::sharding::{SubgraphBuilder, CsrShard};

// Load only relevant shards for prompt
let shard = shard_reader.get_shard(token_id / 1000)?;
let mut builder = SubgraphBuilder::new_from_shard(&shard);

// Build prompt-local subgraph
let tokens = vec![token_id];
let subgraph = builder.build_from_tokens(&tokens, depth=2);
```

### Pattern 2: Semantic-Aware Search

For algorithms needing semantic similarity:

```rust
use sqlitegraph::sharding::SemanticLayer;

// Fall back to HNSW when CSR lacks coverage
let csr_results = csr_query(token_id);
if csr_results.is_empty() {
    let layer = get_semantic_layer();
    let semantic_results = layer.knn_search(&embedding, k=10);
}
```

### Pattern 3: Property-Enhanced Analysis

For algorithms needing metadata (security, observability):

```rust
use sqlitegraph::sharding::PropertyStore;

// Query with metadata enrichment
let store = PropertyStore::new(&db_path)?;
let text = store.get_token_text(token_id)?;
let metadata = store.get_metadata(token_id)?;
```

### Pattern 4: Change-Stream Monitoring

For reactive algorithms (incremental updates, pub/sub):

```rust
use sqlitegraph::sharding::PubSub;

// Subscribe to graph changes
pubsub.subscribe(
    vec!["graph.edge".to_string()],
    Box::new(|change| {
        // React to graph mutation
        match change.change_type {
            ChangeType::EdgeInserted => handle_new_edge(),
            ChangeType::EdgeDeleted => handle_edge_removal(),
            _ => {}
        }
    }),
);
```

## Performance Expectations

### Native-V3 vs SQLite for Algorithms

| Operation | SQLite | Native-V3 | Notes |
|-----------|---------|-----------|-------|
| Node iteration | Index scan | CSR shard read | V3 faster for prompt-local queries |
| Edge traversal | JOIN + scan | CSR forward edge | V3 avoids table scan |
| BFS depth-2 | 2× query | O(shard_size) | V3 limits traversal to loaded shards |
| PageRank | Table scan | CSR iteration | Similar, V3 better for large graphs |
| SCC detection | Graph traversal | Graph traversal | Same complexity, V3 has locality |

### CSR + HNSW Hybrid Performance

- **CSR topology queries**: O(log N) via shard indexing
- **HNSW semantic queries**: O(log N) via HNSW graph
- **Hybrid approach**: CSR for exact matches, HNSW for semantic fallback

## Missing Implementations

The following native-v3 features are **not yet implemented**:

| Feature | Priority | Use Case |
|---------|----------|---------|
| `ShardReader` file I/O | High | Load CSR shards from disk |
| Multi-shard queries | High | Cross-shard subgraph construction |
| BidirectionalIndex from ShardReader | Medium | Load-time reverse edge building |
| Incremental HNSW updates | Medium | Fine-tune workflows |
| CSR shard compression | Low | Reduce storage for large graphs |

## Migration Guide

For existing code using graph algorithms:

1. **No changes needed** for backend-agnostic algorithms (`algo::backend::*`)
2. **Use CSR layer** for prompt-local queries: `sharding::SubgraphBuilder`
3. **Use semantic layer** for semantic search: `sharding::SemanticLayer`
4. **Use property store** for metadata: `sharding::PropertyStore`
5. **Use pub/sub** for reactive updates: `sharding::PubSub`

## Testing Strategy

- **Unit tests**: Each native-v3 component tested in isolation (67 sharding tests)
- **Integration tests**: Cross-component tests with real data (data_tests.rs)
- **Algorithm tests**: Verify algorithms work with native-v3 backend
- **Performance tests**: Benchmark V3 vs SQLite for each algorithm

## Future Work

1. **Algorithm-native-v3 integration**: Optimize algorithms to use CSR sharding directly
2. **Parallel algorithms**: Leverage native-v3 parallel structure for multi-threaded algorithms
3. **Incremental updates**: Pub/sub + HNSW for real-time graph updates
4. **Compression**: CSR shard compression for storage efficiency
5. **Caching**: Multi-level caching for hot shards and embeddings
