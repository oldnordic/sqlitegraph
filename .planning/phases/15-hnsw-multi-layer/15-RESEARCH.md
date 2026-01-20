# Phase 15: HNSW Multi-Layer - Research

**Researched:** 2026-01-20
**Domain:** HNSW Multi-Layer Vector Search Implementation
**Confidence:** HIGH

## Summary

Phase 15 upgrades the existing single-layer HNSW implementation to proper multi-layer search with O(log N) complexity. The codebase already has substantial multi-layer infrastructure in place (`multilayer.rs` with `LayerMappings`, `LevelDistributor`, `MultiLayerNodeManager`) but it's not wired into the main `HnswIndex::insert_vector()` and `HnswIndex::search()` paths. The current `determine_insertion_level()` function always returns 0 (single-layer mode).

The core implementation work is:
1. **Wire `LevelDistributor` into insertion path** - Replace stub `determine_insertion_level()` with proper exponential distribution
2. **Enable multi-layer search descent** - Implement greedy descent from top layer to base layer
3. **Integrate `LayerMappings`** - Use bidirectional ID mapping for layer-local node management
4. **Add persistence for layer assignments** - Store each vector's highest layer in database
5. **Benchmark O(log N) complexity** - Verify search performance improvement
6. **Verify >95% recall** - Compare against exact nearest neighbor

**Primary recommendation:** The multi-layer foundation already exists in `multilayer.rs`. The phase should focus on integration rather than greenfield implementation. Use existing `LevelDistributor::sample_level()` for exponential distribution and `LayerMappings` for ID translation.

## Standard Stack

The HNSW implementation uses existing Rust infrastructure with no external dependencies beyond what's already in use:

### Core (Already in Use)
| Component | Version | Purpose | Why Standard |
|-----------|---------|---------|--------------|
| `HnswIndex` | existing | Main HNSW index orchestrator | Core index API |
| `HnswLayer` | existing | Per-layer graph management | Layer isolation |
| `NeighborhoodSearch` | existing | k-NN search within layers | Greedy search algorithm |
| `LevelDistributor` | existing | Exponential level assignment | Probabilistic layer distribution |
| `LayerMappings` | existing | Global-to-local ID translation | Resolves 1-based/0-based ID conflict |
| `MultiLayerNodeManager` | existing | Multi-layer orchestration | Coordinates multi-layer operations |

### Dependencies (Already Required)
| Library | Purpose | Where Used |
|---------|---------|------------|
| `rand` | Seeded RNG for deterministic level assignment | `LevelDistributor` |
| `criterion` | Benchmarking framework | HNSW performance verification |
| `rusqlite` | SQLite persistence | Layer assignment storage |

### For Benchmarks (New for Phase 15)
| Library | Purpose | Use Case |
|---------|---------|----------|
| `criterion` | O(log N) complexity verification | Search performance scaling |
| Custom exact NN | Recall verification baseline | Compare HNSW vs brute force |

**No new dependencies required.** All multi-layer infrastructure exists.

## Architecture Patterns

### Recommended Project Structure

The HNSW module is already well-organized. Phase 15 adds integration points:

```
src/hnsw/
├── mod.rs              # Public API (already exists)
├── index.rs            # HnswIndex - ADD multi-layer search
├── multilayer.rs       # Multi-layer components (ALREADY EXISTS)
├── layer.rs            # HnswLayer (already exists)
├── neighborhood.rs     # NeighborhoodSearch (already exists)
├── config.rs           # HnswConfig (already has enable_multilayer)
├── storage.rs          # Vector persistence
└── builder.rs          # Configuration builder
```

### Pattern 1: Multi-Layer Insertion with Exponential Distribution

**What:** Each vector is assigned a level `l` using exponential distribution P(l) = m^(-l), then inserted into all layers 0..=l.

**When to use:** All insertions when `enable_multilayer = true`.

**Implementation:**
```rust
// Source: Existing codebase src/hnsw/multilayer.rs:352-497
// LevelDistributor already implements exponential distribution

// In HnswIndex::insert_vector(), replace current stub:
pub fn insert_vector(&mut self, vector: &[f32], metadata: Option<Value>) -> Result<u64, HnswError> {
    let vector_id = self.storage.store_vector(vector, metadata)?;

    // NEW: Use LevelDistributor for exponential distribution
    let insertion_level = if self.config.enable_multilayer {
        self.level_distributor.sample_level_internal()
    } else {
        0  // Single-layer mode for backward compatibility
    };

    // Insert into all layers from insertion_level down to 0
    for level in (0..=insertion_level).rev() {
        self.insert_into_layer(vector_id, level)?;
    }

    // Update entry points
    if insertion_level >= self.entry_points.len() {
        self.entry_points.push(vector_id);
    }

    self.vector_count += 1;
    Ok(vector_id)
}
```

### Pattern 2: Greedy Descent Multi-Layer Search

**What:** Search starts at the top layer's entry point, performs greedy search to find nearest neighbor in that layer, then uses that as entry point for the next layer down, finally performing ef-search at layer 0.

**When to use:** All searches when multi-layer mode is enabled.

**Implementation:**
```rust
// In HnswIndex::search(), add multi-layer descent:
pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(u64, f32)>, HnswError> {
    if self.vector_count == 0 {
        return Ok(Vec::new());
    }

    // NEW: Multi-layer greedy descent
    let mut closest_entry_point = self.entry_points.last()
        .copied()
        .ok_or(HnswError::Index(HnswIndexError::IndexNotInitialized))?;

    // Descend from top layer to layer 1
    for level in (1..self.layers.len()).rev() {
        if self.layers[level].node_count() == 0 {
            continue;
        }

        // Greedy search in this layer to find better entry point
        let result = self.search_engine.search_layer(
            &self.layers[level],
            query,
            &self.vectors_array,
            &[closest_entry_point - 1],  // Convert to 0-based
            1,  // Only need closest neighbor
        )?;

        if !result.neighbors().is_empty() {
            closest_entry_point = result.neighbors()[0] + 1;  // Back to 1-based
        }
    }

    // Layer 0: Full ef-search with k results
    let search_result = self.search_engine.search_layer(
        &self.layers[0],
        query,
        &self.vectors_array,
        &[closest_entry_point - 1],
        self.config.ef_search.max(k),
    )?;

    // Convert results to 1-based vector IDs
    let results: Vec<(u64, f32)> = search_result.neighbors()
        .iter()
        .zip(search_result.distances())
        .map(|(&node_id, &dist)| (node_id + 1, dist))
        .take(k)
        .collect();

    Ok(results)
}
```

### Pattern 3: Layer Assignments Persistence

**What:** Store each vector's highest layer assignment in the database for graph reconstruction after restart.

**When to use:** During vector persistence and index loading.

**Schema addition:**
```sql
-- Add to hnsw_vectors table
ALTER TABLE hnsw_vectors ADD COLUMN highest_layer INTEGER DEFAULT 0;
```

### Anti-Patterns to Avoid

- **Mixing 1-based and 0-based IDs without translation**: Always use `LayerMappings` for ID conversion
- **Skipping layers in insertion**: Must insert into ALL layers 0..=target_level for HNSW correctness
- **Using ef_search at higher layers**: Higher layers use greedy descent (k=1), only layer 0 uses full ef-search
- **Not updating entry points**: New highest-level vectors must become entry points
- **Forgetting seed for deterministic behavior**: Use `multilayer_deterministic_seed` for reproducible level assignment

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Exponential level distribution | Custom random logic | `LevelDistributor::sample_level()` | Already implements correct P(l) = m^(-l) distribution |
| Global-to-local ID mapping | Manual HashMap lookups | `LayerMappings` | Bidirectional mapping with consistency validation |
| Layer-local node management | Custom per-layer indexing | `HnswLayer` with 0-based node IDs | Sequential node ID enforcement |
| Greedy search within layer | Custom k-NN implementation | `NeighborhoodSearch::search_layer()` | Ef-sized candidate list with visited set |
| Configuration building | Manual config construction | `HnswConfigBuilder` | Already has multi-layer fields |

**Key insight:** The multi-layer infrastructure is complete. This phase is about integration, not new algorithms.

## Common Pitfalls

### Pitfall 1: ID System Confusion (1-based vs 0-based)

**What goes wrong:** Vector storage uses 1-based IDs (1, 2, 3...) while `HnswLayer` uses 0-based node IDs (0, 1, 2...). Direct mixing causes off-by-one errors.

**Why it happens:** Different subsystems evolved independently. `VectorStorage` assigns sequential 1-based IDs; `HnswLayer::add_node()` expects sequential 0-based IDs.

**How to avoid:** Always use `LayerMappings` for ID translation:
```rust
// CORRECT: Use mapping
let local_id = self.layer_mappings.get_local_id(vector_id, layer_id)?;
layer.add_node(local_id)?;

// WRONG: Direct conversion
layer.add_node(vector_id - 1)?;  // May conflict!
```

**Warning signs:** Panics in `add_node()`, "invalid node ID" errors, inconsistent search results.

### Pitfall 2: Forgetting Multi-Layer Insertion

**What goes wrong:** Vector inserted only into target level, not all layers 0..=target_level. Search fails because vector doesn't exist in expected layers.

**Why it happens:** Misunderstanding HNSW - each vector appears in ALL layers from 0 to its assigned level.

**How to avoid:** Always iterate from target_level down to 0:
```rust
// CORRECT: Insert into all required layers
for level in (0..=target_level).rev() {
    self.insert_into_layer(vector_id, level)?;
}

// WRONG: Only insert into target level
self.insert_into_layer(vector_id, target_level)?;
```

**Warning signs:** Search returns empty results, vectors "disappear" from higher layers.

### Pitfall 3: Missing Entry Point Updates

**What goes wrong:** New high-level vectors not added to entry points. Search has no starting point at top layers.

**Why it happens:** Entry points must be explicitly managed; they don't auto-update.

**How to avoid:** Check if new vector's level exceeds current max:
```rust
if target_level >= self.entry_points.len() {
    self.entry_points.push(vector_id);
}
```

**Warning signs:** All searches start from wrong entry point, degraded performance.

### Pitfall 4: Wrong ef Parameter at Higher Layers

**What goes wrong:** Using full ef-search at higher layers. Performance degrades to O(N) instead of O(log N).

**Why it happens:** Higher layers are for navigation, not precise search.

**How to avoid:** Higher layers use k=1 (greedy descent), only layer 0 uses ef_search:
```rust
// Higher layers: Greedy descent (k=1)
let ef = if level == 0 { self.config.ef_search } else { 1 };
```

**Warning signs:** Search time scales linearly with dataset size, not logarithmically.

### Pitfall 5: Non-Deterministic Level Assignment

**What goes wrong:** Same dataset produces different HNSW structure on each run. Tests fail intermittently.

**Why it happens:** Default `StdRng::from_entropy()` produces different sequences.

**How to avoid:** Always seed the `LevelDistributor` for reproducible behavior:
```rust
let mut distributor = LevelDistributor::new(base_m, max_layers)
    .with_seed(self.config.multilayer_deterministic_seed.unwrap_or(42));
```

**Warning signs:** Flaky tests, inconsistent benchmarks.

## Code Examples

### Multi-Layer Search Implementation

Verified pattern from HNSW literature and existing codebase structure:

```rust
// Source: Synthesized from existing NeighborhoodSearch::search_layer()
// and HNSW algorithm specifications

/// Multi-layer search with greedy descent
fn search_multilayer(&self, query: &[f32], k: usize) -> Result<Vec<(u64, f32)>, HnswError> {
    let vectors_array = self.load_vectors_as_array()?;

    // Start from top layer entry point
    let mut entry_point = *self.entry_points.last()
        .ok_or(HnswError::Index(HnswIndexError::IndexNotInitialized))?;

    // Greedy descent through higher layers
    for level in (1..self.layers.len()).rev() {
        if self.layers[level].node_count() == 0 {
            continue;
        }

        let local_id = self.layer_mappings.get_local_id(entry_point, level)
            .ok_or(HnswError::Index(HnswIndexError::NodeNotFound(entry_point)))?;

        // Find closest neighbor in this layer (k=1 for greedy)
        let result = self.search_engine.search_layer(
            &self.layers[level],
            query,
            &vectors_array,
            &[local_id],
            1,  // Only need nearest for descent
        )?;

        if !result.neighbors().is_empty() {
            let closest_local = result.neighbors()[0];
            entry_point = self.layer_mappings.get_global_id(level, closest_local)
                .ok_or(HnswError::Index(HnswIndexError::InvalidNodeId(closest_local)))?;
        }
    }

    // Layer 0: Full ef-search
    let local_entry = self.layer_mappings.get_local_id(entry_point, 0)
        .ok_or(HnswError::Index(HnswIndexError::NodeNotFound(entry_point)))?;

    let result = self.search_engine.search_layer(
        &self.layers[0],
        query,
        &vectors_array,
        &[local_entry],
        self.config.ef_search.max(k),
    )?;

    // Convert results back to global IDs
    let mut results = Vec::with_capacity(k.min(result.len()));
    for (i, &local_id) in result.neighbors().iter().enumerate() {
        if i >= k { break; }
        if let Some(global_id) = self.layer_mappings.get_global_id(0, local_id) {
            results.push((global_id, result.distances()[i]));
        }
    }

    Ok(results)
}
```

### Benchmark Pattern for O(log N) Verification

```rust
// Source: Criterion benchmarking patterns
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn bench_hnsw_search_scaling(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("hnsw_scaling");
    group.sample_size(100);  // More samples for stable timing

    let dataset_sizes = vec![1_000, 10_000, 100_000, 1_000_000];
    let dimension = 768;  // OpenAI embedding size

    for &size in &dataset_sizes {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |bencher, &size| {
                // Setup: Create multi-layer HNSW index
                let config = hnsw_config()
                    .dimension(dimension)
                    .m_connections(16)
                    .ef_construction(200)
                    .ef_search(50)
                    .enable_multilayer(true)
                    .multilayer_deterministic_seed(42)
                    .build()
                    .unwrap();

                let mut hnsw = HnswIndex::new("bench", config).unwrap();
                let vectors = generate_vectors(size, dimension);

                for vector in &vectors {
                    hnsw.insert_vector(vector, None).unwrap();
                }

                let query = &vectors[0];

                // Benchmark search
                bencher.iter(|| {
                    hnsw.search(query, 10).unwrap()
                });
            }
        );
    }

    group.finish();
}

criterion_group!(benches, bench_hnsw_search_scaling);
criterion_main!(benches);
```

### Recall Verification Pattern

```rust
// Compare HNSW results against exact nearest neighbor
fn verify_recall(hnsw: &HnswIndex, vectors: &[Vec<f32>], queries: &[Vec<f32>]) -> f64 {
    let mut total_correct = 0;
    let mut total_possible = 0;
    let k = 10;

    for query in queries {
        // HNSW approximate results
        let hnsw_results = hnsw.search(query, k).unwrap();

        // Exact nearest neighbors (brute force)
        let mut exact_results: Vec<_> = vectors.iter()
            .enumerate()
            .map(|(i, v)| (i as u64 + 1, cosine_distance(query, v)))
            .collect();
        exact_results.sort_by_key(|(_, dist)| *dist);

        // Count overlap in top-k
        let hnsw_ids: HashSet<_> = hnsw_results.iter()
            .take(k)
            .map(|(id, _)| *id)
            .collect();

        let exact_ids: HashSet<_> = exact_results.iter()
            .take(k)
            .map(|(id, _)| *id)
            .collect();

        total_correct += hnsw_ids.intersection(&exact_ids).count();
        total_possible += k;
    }

    (total_correct as f64 / total_possible as f64) * 100.0
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Single-layer HNSW (NSW) | Multi-layer HNSW with exponential distribution | Original Malkov paper (2016) | O(N) -> O(log N) search |
| Static level assignment | Probabilistic exponential distribution | 2016 | Natural hierarchy, better performance |
| Fixed ef parameter | Separate ef_construction and ef_search | 2016 | Tunable build-time vs query-time tradeoffs |
| Random level assignment | Seeded deterministic assignment | 2020s | Reproducible benchmarks |

**Key Papers:**
- [Malkov & Yashunin (2016)](https://arxiv.org/abs/1603.09320) - Original HNSW paper
- [The Impacts of Data, Ordering, and Intrinsic Dimensionality (2024)](https://arxiv.org/html/2405.17813v1) - Recall analysis

**Deprecated/outdated:**
- NSW (non-hierarchical): Superseded by HNSW, use only for comparison
- Fixed layer counts: Exponential distribution is standard
- Underspecified RNG: Always use seeded RNG for reproducibility

## Open Questions

1. **Layer assignment persistence format**
   - **What we know:** Need to store `highest_layer` per vector in database
   - **What's unclear:** Exact schema migration strategy for existing indexes
   - **Recommendation:** Add `highest_layer INTEGER DEFAULT 0` column to `hnsw_vectors`, run ALTER TABLE migration

2. **Backward compatibility for single-layer indexes**
   - **What we know:** `enable_multilayer` config flag exists and defaults to `false`
   - **What's unclear:** Migration path for existing single-layer production indexes
   - **Recommendation:** Keep `enable_multilayer = false` as default, require explicit opt-in

3. **Benchmark dataset sizes for O(log N) verification**
   - **What we know:** Need multiple orders of magnitude (1K, 10K, 100K, 1M vectors)
   - **What's unclear:** Practical upper limit given test infrastructure
   - **Recommendation:** Start with 1K-100K, extend to 1M if feasible

## Sources

### Primary (HIGH confidence)
- **Original codebase analysis** - Read all HNSW source files in `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/hnsw/`
  - `multilayer.rs` (lines 1-890) - Complete multi-layer infrastructure
  - `index.rs` (lines 1-1606) - Current single-layer implementation
  - `config.rs` (lines 1-470) - Multi-layer configuration options
  - `layer.rs` (lines 1-593) - Per-layer graph management
  - `neighborhood.rs` (lines 1-664) - Greedy search implementation
- **Project STATE.md** - Current position and accumulated decisions

### Secondary (MEDIUM confidence)
- [Efficient and robust approximate nearest neighbor search (Malkov & Yashunin, 2016)](https://arxiv.org/abs/1603.09320) - Original HNSW paper with algorithm specification
- [Redis Blog - How HNSW algorithms can improve search (June 2025)](https://redis.io/blog/how-hnsw-algorithms-can-improve-search/) - Confirms greedy descent and ef parameter usage
- [Understanding HNSW: A Practical Guide (Sept 2025)](https://www.ashutosh.dev/understanding-hnsw-a-practical-guide/) - Tuning parameters explanation
- [HNSW: Efficient Graph-Based ANN Search (Nov 2025)](https://www.emergentmind.com/topics/hnsw-algorithm) - Near-logarithmic search performance
- [The Impacts of Data, Ordering, and Intrinsic Dimensionality (2024)](https://arxiv.org/html/2405.17813v1) - Recall vs exact KNN analysis
- [Understanding Hierarchical Navigable Small Worlds - Zilliz (July 2024)](https://zilliz.com/learn/hierarchical-navigable-small-worlds-HNSW) - Comprehensive algorithm overview
- [Hierarchical Navigable Small Worlds in Vector Search - Medium (Part 2)](https://medium.com/@adnanmasood/the-shortcut-through-space-hierarchical-navigable-small-worlds-hnsw-in-vector-search-part-2-ba2e8a64134e) - Recall@k benchmark metrics

### Tertiary (LOW confidence)
- None used for critical implementation decisions

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All components already exist in codebase
- Architecture: HIGH - Multi-layer infrastructure already implemented
- Pitfalls: HIGH - ID system confusion already observed in existing code comments
- Integration requirements: HIGH - Clear path from existing single-layer to multi-layer

**Research date:** 2026-01-20
**Valid until:** 60 days (HNSW algorithm is stable, codebase-specific integration patterns are current)
