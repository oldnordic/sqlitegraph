# HNSW Multi-Layer Implementation Research

**Research Date:** 2026-01-20
**Focus:** Hierarchical Navigable Small World multi-layer algorithm implementation

## Overview

HNSW (Hierarchical Navigable Small World) is a graph-based algorithm for approximate nearest neighbor (ANN) search. Its key innovation is a **multi-layer graph structure** that provides O(log N) search complexity.

## Key Sources

- [Original HNSW Paper (arXiv:1603.09320)](https://arxiv.org/abs/1603.09320) - Malkov & Yashunin, 2016
- [Redis Blog: HNSW improves search (June 2025)](https://redis.io/blog/how-hnsw-algorithms-can-improve-search/)
- [Milvus Blog: Understanding HNSW (May 2025)](https://milvus.io/blog/understand-hierarchical-navigable-small-worlds-hnsw-for-vector-search.md)
- [Pinecone Learning: HNSW layered format](https://www.pinecone.io/learn/series/faiss/hnsw/)
- [MongoDB: HNSW Basics](https://www.mongodb.com/resources/basics/hierarchical-navigable-small-world)
- [Medium: Similarity Search Part 4]((https://medium.com/data-science/similarity-search-part-4-hierarchical-navigable-small-world-hnsw-2aad4fe87d37))
- [GitHub: brtholomy/hnsw tutorial](https://github.com/brtholomy/hnsw)

---

## Multi-Layer Structure

### Layer Concept

HNSW creates a **hierarchical skip-list-like graph**:

```
Layer 2:  o-------o-------o       (fewest nodes, longest edges)
Layer 1:  o---o---o---o---o---o   (more nodes, medium edges)
Layer 0:  o-o-o-o-o-o-o-o-o-o-o-o   (all nodes, shortest edges)
```

**Key properties:**
- Higher layers have fewer nodes with longer edges
- Lower layers have more nodes with shorter edges
- Search starts at top layer, "zooms in" down through layers
- Combines principles from **navigable small worlds** and **skip lists**

### Layer Distribution

**Exponential distribution** determines which layer each node is inserted into:

```
P(layer = L) = ml^L / (1 - mL)   for L > 0
P(layer = 0) = 1 - mL
```

Where:
- `mL` (layer multiplier, typically 0.5-0.7): Probability of having another layer
- Higher mL = more layers, faster search but more memory

**Algorithm for insertion layer:**
```
function determine_insertion_level(mL):
    level = 0
    while random() < mL and level < max_level:
        level += 1
    return level
```

### Search Complexity

| Layers Used | Search Complexity | Memory Overhead |
|-------------|-------------------|------------------|
| Only layer 0 | O(N) linear | Minimal |
| 2 layers | O(√N) | ~2x |
| Full multi-layer | O(log N) | ~1/(1-mL) |

---

## Insertion Algorithm

### Multi-Layer Insert

```
INSERT(vector):
    1. L = determine_insertion_level(mL)      # Target layer for new node
    2. For each layer ℓ from max_layer down to L+1:
           - Find ef_construction nearest neighbors at layer ℓ
           - Select candidate neighbors
    3. For layer L down to 0:
           - Find ef_construction nearest neighbors at layer ℓ
           - Select M neighbors (heuristic: closest by distance)
           - Add bidirectional edges to selected neighbors
    4. max_layer = max(max_layer, L)
```

**Key parameters:**
- `ef_construction`: Candidates considered during insert (higher = better quality, slower)
- `M`: Max neighbors per node per layer
- `mL`: Layer probability multiplier

---

## Search Algorithm

### Multi-Layer Search

```
SEARCH(query, k):
    1. Set entry_point = top-layer node
    2. For ℓ from max_layer down to 1:
           - Greedy search to nearest neighbor at layer ℓ
           - Update entry_point to found neighbor
    3. At layer 0:
           - ef-search nearest neighbors using entry_point
           - Return k closest results
```

**Key insight:** Higher layers provide "highways" for fast traversal; layer 0 provides accuracy.

---

## Implementation Gaps in SQLiteGraph

### Current State

**File:** `sqlitegraph/src/hnsw/index.rs:921-922`

```rust
// TODO: Implement multi-layer insertion
fn determine_insertion_level(&self) -> usize {
    0 // Only using layer 0
}
```

**Impact:** Search complexity is O(N) instead of O(log N) — significant performance degradation for large indexes.

### Required Implementation

**1. Layer Selection Function**
```rust
fn determine_insertion_level(&self) -> usize {
    let mut level = 0;
    let ml = self.config.ml; // Layer multiplier (e.g., 0.5)

    while self.rng.gen::<f64>() < ml && level < self.config.max_layers {
        level += 1;
    }

    level
}
```

**2. Multi-Layer Data Structure**
```rust
struct HnswIndex {
    layers: Vec<GraphLayer>,  // One graph per layer
    max_layer: usize,
    // ... other fields
}

struct GraphLayer {
    nodes: HashMap<NodeId, Vec<NodeId>>,  // Adjacency list
    entry_point: Option<NodeId>,
}
```

**3. Layer- aware Insert**
```rust
fn insert_vector(&mut self, vector: Vec<f32>) -> Result<NodeId> {
    let target_layer = self.determine_insertion_level();

    // Insert into all layers 0..=target_layer
    for layer in (0..=target_layer).rev() {
        let neighbors = self.search_layer(vector, layer, self.config.ef_construction);
        self.add_connections(node_id, neighbors, layer);
    }

    // Update max_layer if needed
    self.max_layer = self.max_layer.max(target_layer);

    Ok(node_id)
}
```

**4. Layer- aware Search**
```rust
fn search_nearest(&self, query: &[f32], k: usize) -> Vec<(NodeId, f64)> {
    let mut entry_point = self.layers[self.max_layer].entry_point;

    // Greedy search down through layers
    for layer in (1..=self.max_layer).rev() {
        entry_point = self.greedy_search_layer(query, layer, entry_point);
    }

    // Final search at layer 0 with ef-search
    self.search_layer_0(query, entry_point, k, self.config.ef_search)
}
```

---

## Configuration Parameters

| Parameter | Typical Range | Effect | SQLiteGraph Default |
|-----------|---------------|--------|---------------------|
| `M` (neighbors) | 5-64 | More = better recall, more memory | Need to add |
| `ef_construction` | 40-400 | Higher = better quality, slower insert | Need to add |
| `ef_search` | 10-100 | Higher = better recall, slower search | Need to add |
| `mL` (layer mult) | 0.3-0.7 | Higher = more layers, faster search | Need to add |
| `max_layers` | 16-64 | Absolute maximum layers | Need to add |

---

## Testing Strategy

### Unit Tests Required

**1. Layer Distribution**
```rust
#[test]
fn test_layer_distribution() {
    // Verify exponential distribution
    // Layer 0 should be most common
    // Higher layers increasingly rare
}
```

**2. Multi-Layer Insert**
```rust
#[test]
fn test_multilayer_insert() {
    // Insert vectors
    // Verify nodes appear in correct layers
    // Verify layer 0 has all nodes
}
```

**3. Multi-Layer Search**
```rust
#[test]
fn test_multilayer_search_correctness() {
    // Verify multi-layer search finds same results as layer 0
    // Verify multi-layer search is faster (benchmark)
}
```

**4. Search Performance**
```rust
#[test]
fn test_search_complexity() {
    // O(log N) vs O(N) for large indexes
    // Measure actual complexity with different sizes
}
```

### Benchmarks Required

| Benchmark | Metric | Target |
|-----------|--------|--------|
| Insert throughput | vectors/sec | Maintain within 2x of current |
| Search latency | ms/query | < 10ms for 10K vectors |
| Recall | % of exact NN | > 95% |
| Memory overhead | bytes/vector | < 1.5x layer 0 only |

---

## Anti-Patterns to Avoid

**1. Skipping Layer 0**
- All nodes MUST exist in layer 0
- Higher layers are subsets, not replacements

**2. Invalid Layer Selection**
- Must use exponential distribution
- Don't use random uniform distribution

**3. Inconsistent Entry Points**
- Each layer needs its own entry point
- Or track global top-layer entry point

**4. Ignoring `max_layers`**
- Unbounded layer growth causes memory issues
- Set reasonable maximum (16-64)

---

## Migration Path for SQLiteGraph

### Phase 1: Add Configuration
- Add `M`, `ef_construction`, `ef_search`, `mL`, `max_layers` to `HnswConfig`
- Update builder pattern
- Maintain backward compatibility

### Phase 2: Implement Layer Selection
- Implement `determine_insertion_level()`
- Add exponential distribution with `ml` parameter
- Unit test distribution

### Phase 3: Multi-Layer Data Structure
- Change from single graph to `Vec<GraphLayer>`
- Update all insert/search logic
- Maintain existing API

### Phase 4: Performance Validation
- Benchmark against single-layer
- Verify O(log N) search
- Verify recall > 95%

### Phase 5: Persistence Update
- Update schema to store layer info
- Migrate existing single-layer indexes

---

## References

- [arXiv:1603.09320 - Efficient and robust approximate nearest neighbor search](https://arxiv.org/abs/1603.09320) - Original HNSW paper
- [Zilliz: Understanding HNSW](https://zilliz.com/learn/hierarchical-navigable-small-worlds-HNSW)
- [TigerData: Vector Database Basics - HNSW](https://www.tigerdata.com/blog/vector-database-basics-hnsw)

---
*HNSW Multi-Layer Research: 2026-01-20*
