# Native-v3 Storage Engine — Unified Spec

**Status:** Design  
**Date:** 2026-06-26  
**Origin:** graphtransformer experiments + transferable-graph insights  
**Related:** transferable-graph-embedding-manifold.md, graphtransformer CHANGELOG.md

---

## Vision

Single storage binary unifying **SQL + Graph + CSR + HNSW + Pub/Sub**. CSR adjacency maps compound for precise search, HNSW provides semantic fallback, graph topology enables incremental updates, pub/sub propagates changes.

**Goal:** 10× speed via architecture (F1), not embedding quality. 1,100 tok/s on sequential edges, 245 tok/s with bidirectional rescoring.

---

## Architecture Layers

```
┌─────────────────────────────────────────────────────────────┐
│                    Pub/Sub Layer (WAL-backed)                 │
│  ┌──────────────────────┐  ┌──────────────────────────────┐ │
│  │  Change Feed         │  │  Subscription Manager        │ │
│  │  (INSERT/UPDATE/DELETE)│  │  (topic-based routing)        │ │
│  └──────────────────────┘  └──────────────────────────────┘ │
└──────────────────────┬────────────────────────────────────────┘
                       │
┌──────────────────────▼────────────────────────────────────────┐
│                    Graph Topology Layer                       │
│  ┌──────────────────────┐  ┌──────────────────────────────┐ │
│  │  CSR Adjacency       │  │  Bidirectional Index          │ │
│  │  (token→successors)   │  │  (reverse edges for rescoring) │ │
│  └──────────────────────┘  └──────────────────────────────┘ │
│  ┌──────────────────────┐  ┌──────────────────────────────┐ │
│  │  HNSW Semantic       │  │  Transferable Topology        │ │
│  │  (embedding KNN)      │  │  (cross-model reuse)          │ │
│  └──────────────────────┘  └──────────────────────────────┘ │
└──────────────────────┬────────────────────────────────────────┘
                       │
┌──────────────────────▼────────────────────────────────────────┐
│                    SQL Layer (SQLite Core)                    │
│  ┌──────────────────────┐  ┌──────────────────────────────┐ │
│  │  Property Store      │  │  Physical Layout Graph         │ │
│  │  (node metadata)      │  │  (table→column→page)           │ │
│  └──────────────────────┘  └──────────────────────────────┘ │
└───────────────────────────────────────────────────────────────┘
```

---

## Component Design

### 1. Pub/Sub Layer

**Purpose:** Propagate graph changes without polling. WAL-backed change feed + topic routing.

**Schema:**
```sql
CREATE TABLE change_feed (
  seq_id INTEGER PRIMARY KEY AUTOINCREMENT,
  topic TEXT NOT NULL,
  entity_type TEXT NOT NULL,  -- 'node' | 'edge' | 'property'
  entity_id INTEGER NOT NULL,
  operation TEXT NOT NULL,    -- 'INSERT' | 'UPDATE' | 'DELETE'
  payload BLOB,               -- serialized change delta
  timestamp INTEGER NOT NULL
);
CREATE INDEX change_feed_topic_seq ON change_feed(topic, seq_id);
```

**Subscription API:**
```rust
pub struct Subscription {
  pub topic: String,
  pub since_seq: Option<i64>,
  pub filter: Option<ChangeFilter>,
}

impl Storage {
  pub fn subscribe(&self, sub: Subscription) -> Receiver<Change>;
  
  // Pub/Sub worker reads WAL, publishes to channels
  fn pubsub_worker(&self) -> anyhow::Result<()>;
}
```

**Topics:**
- `graph.edge.insert` — new edges added (prefix-conditioned, LM-head)
- `graph.edge.update` — edge weights changed (Hebbian reinforcement)
- `graph.node.insert` — new vocabulary nodes
- `hnsw.vector.insert` — embedding insertions
- `csr.shard.update` — shard file updates (F21 chunked extraction)

---

### 2. CSR Adjacency Layer

**Purpose:** Fast successor lookup via compressed sparse row. Core of graph traversal.

**Storage format (on-disk):**
```rust
pub struct CsrShard {
  pub shard_id: usize,
  pub source_start: u32,     // inclusive
  pub source_end: u32,       // exclusive
  pub edges: Vec<CsrEdge>,
}

pub struct CsrEdge {
  pub src: u32,              // source token/node ID
  pub dst: u32,              // destination token/node ID
  pub weight: f32,            // P(dst|src), softmax-normalized
  pub flags: u32,             // context-bucket, edge-type metadata
}
```

**In-memory index (fast lookup):**
```rust
pub struct CsrIndex {
  pub offsets: Vec<usize>,   // cumulative edge count per source
  pub edges: Vec<CsrEdge>,    // flat edge array
}

impl CsrIndex {
  // O(1) successor lookup
  pub fn successors(&self, src: u32) -> &[CsrEdge] {
    let start = self.offsets[src as usize];
    let end = self.offsets[src as usize + 1];
    &self.edges[start..end]
  }
}
```

**Shard query (F21 subgraph builder):**
```rust
pub fn build_subgraph(
  shards: &[CsrShard],
  prompt_tokens: &[u32],
  depth: usize,
) -> Subgraph {
  // BFS from each prompt token, depth-limited
  // Load only shards matching source ranges
  // Return: nodes + edges in local neighborhood
}
```

**Transferability (from transferable-graph doc):**

Cross-model reuse requires topology overlap >50%. Measure via:

```python
def topology_overlap(edges_a, edges_b, top_k=50):
  overlap = []
  for src in vocabulary:
    succ_a = top_k_successors(edges_a, src, k=top_k)
    succ_b = top_k_successors(edges_b, src, k=top_k)
    overlap.append(len(set(succ_a) & set(succ_b)) / top_k)
  return mean(overlap)
```

If overlap >50%: reuse edges, only remap displaced embeddings.
If overlap <20%: full re-extraction required.

Qwen3.5-4B ↔ Qwen3.6-36B: ~4% overlap → NOT transferable (F28).

---

### 3. Bidirectional Index

**Purpose:** Enable backward rescoring and reverse walks (F16, F17).

**Schema:**
```rust
pub struct BidirectionalIndex {
  pub forward: CsrIndex,
  pub backward: CsrIndex,    // built at load time
}

impl BidirectionalIndex {
  pub fn from_edges(edges: Vec<CsrEdge>) -> Self {
    // Forward: src → dst
    let mut forward = CsrIndex::new();
    // Backward: dst → src (reverse edges)
    let mut backward = CsrIndex::new();
    
    for edge in edges {
      forward.insert(edge.src, edge);
      backward.insert(edge.dst, CsrEdge {
        src: edge.dst,
        dst: edge.src,
        weight: edge.weight,  // or compute backward-specific score
        flags: edge.flags,
      });
    }
    Self { forward, backward }
  }
  
  // Backward support for candidate y at current node
  pub fn backward_support(&self, current: u32, candidate: u32) -> f32 {
    let preds = self.backward.successors(candidate);
    let mut score = 0.0;
    for pred_edge in preds {
      if pred_edge.dst == current {
        score += pred_edge.weight;
      }
    }
    score
  }
}
```

**Rescore (F16):**
```rust
score_f(y) = P(current → y)          // forward edge weight
score_b(y) = Σ_x P(x → current) · P(y → x)  // backward support
score(y) = score_f(y) · score_b(y)^α   // α = 0.5
```

---

### 4. HNSW Semantic Layer

**Purpose:** KNN fallback when CSR edges lack coverage. Incremental insert for model updates.

**Index structure:**
```rust
pub struct HnswIndex {
  pub layers: Vec<HnswLayer>,
  pub entry_point: Option<NodeId>,
  pub ef_construction: usize,
  pub m: usize,                     // max connections per node
}

pub struct HnswLayer {
  pub level: usize,
  pub connections: HashMap<NodeId, Vec<NodeId>>,
}
```

**Incremental insert (transferable doc, TurboQuant):**
```rust
impl HnswIndex {
  // Insert new vector without full rebuild
  pub fn insert(&mut self, vector: &[f32], id: NodeId) -> Result<()> {
    // 1. Find nearest neighbors at each level (greedy search)
    // 2. Update connections dynamically
    // 3. Cost: O(displaced × log N), not O(N log N)
  }
  
  // Model fine-tune: compute displacement, update only moved nodes
  pub fn update_after_finetune(
    &mut self,
    old_embeddings: &[f32],
    new_embeddings: &[f32],
    threshold: f32,
  ) -> Result<Vec<NodeId>> {
    let mut displaced = Vec::new();
    for (i, (old, new)) in old_embeddings.iter().zip(new_embeddings).enumerate() {
      let dist = euclidean(old, new);
      if dist > threshold {
        displaced.push(i as NodeId);
        self.insert(new, i)?;
      }
    }
    Ok(displaced)
  }
}
```

**CSR + HNSW hybrid (F6):**
- Sequential edges = syntactic structure (grammar, bigrams)
- Embedding coords = semantic meaning
- **Both required**. KNN-only = multilingual chaos (F6).

**Turbovec integration (performance optimization):**
- **Hybrid HNSW-Turbovec approach** — automatic threshold-based activation (>1K embeddings):
  - Small datasets (<1K): Pure HNSW with fast incremental inserts
  - Large datasets (>1K): Turbovec compressed index with configurable bit-width quantization (2-4 bits, default 4-bit)
  - Automatic activation: Build turbovec index when 1001st embedding inserted
  - Hybrid search: Use turbovec for large datasets, HNSW for small

- **API:**
  ```rust
  // Create HNSW index with configurable bit-width (2-4, default 4)
  backend.create_hnsw_index("semantic", dimension, 4)?;

  // Insert vectors (triggers turbovec at 1K threshold)
  backend.insert_hnsw_vector("semantic", &vector, Some(metadata))?;

  // Search (automatically routes to turbovec or HNSW based on size)
  let results = backend.hnsw_vector_search("semantic", &query, k)?;
  ```

- **Performance (d=64):**
  - 10K embeddings: 2.49ms search (17× faster than pure HNSW)
  - 50K embeddings: 19.5ms search (13.7× faster than pure HNSW)
  - HNSW performance cliff at 5K vectors eliminated

- **Bit-width selection:**
  - 2-bit: 16× compression (d=64 → 8 bytes/vector), fastest scan, more quantization error
  - 3-bit: ~10.7× compression (d=64 → 12 bytes/vector), balanced precision vs compression
  - 4-bit (default): 8× compression (d=64 → 16 bytes/vector), best precision for general-purpose

- **Memory efficiency:**
  - 4-bit quantization reduces embedding memory footprint by 75% vs f32
  - SIMD-optimized search for turbovec (NEON/AVX-512 acceleration)
  - Thread-safe design with Arc<Mutex<>> patterns

---

### 5. SQL Property Store

**Purpose:** Node metadata, edge attributes, physical layout graph.

**Schema:**
```sql
-- Node properties
CREATE TABLE node_properties (
  node_id INTEGER PRIMARY KEY,
  token_text TEXT NOT NULL,
  embedding_vector BLOB,              -- 32D f32 array
  metadata JSON,                      -- model_version, extraction_date
  created_at INTEGER NOT NULL
);
CREATE INDEX node_props_token ON node_properties(token_text);

-- Edge attributes (sparse, extend CSR)
CREATE TABLE edge_attributes (
  src INTEGER NOT NULL,
  dst INTEGER NOT NULL,
  attr_name TEXT NOT NULL,
  attr_value BLOB,
  PRIMARY KEY (src, dst, attr_name)
);

-- Physical layout graph (transferable doc)
CREATE TABLE layout_graph (
  table_name TEXT NOT NULL,
  column_name TEXT NOT NULL,
  rowgroup_id INTEGER,
  page_id INTEGER,
  parent_page_id INTEGER,
  PRIMARY KEY (table_name, column_name, rowgroup_id, page_id)
);
CREATE INDEX layout_parent ON layout_graph(parent_page_id);
```

**Usage:**
```rust
pub fn get_node(&self, id: u32) -> Option<Node> {
  self.conn.query_row(
    "SELECT token_text, embedding_vector, metadata FROM node_properties WHERE node_id = ?",
    [id],
    |row| Node::from_row(row)
  )
}
```

---

## Workflows

### 1. Edge Extraction (F23 chunked pipeline)

**Goal:** Process 122M-token corpus incrementally.

```bash
# Step 1: Chunked extraction (resumable)
python3 scripts/extract_prefix_edges_chunked.py \
  --model /path/to/qwen3.5-4b \
  --corpus wikitext103_train.txt \
  --output-dir data/edges_chunks/ \
  --lines-per-chunk 5000 \
  --max-prefix-len 8 \
  --top-k 20

# Step 2: Merge chunks
python3 scripts/merge_csr_files.py \
  --inputs data/edges_chunks/*.csr \
  --output data/prefix_edges_full.csr \
  --top-k 20

# Step 3: Build dense hybrid (prefix + LM-head fallback)
python3 scripts/build_dense_prefix_edges.py \
  --prefix-edges data/prefix_edges_full.csr \
  --lm-head-edges data/lm_head_edges.csr \
  --output data/prefix_dense_hybrid.csr \
  --top-k 50

# Step 4: Shard for subgraph generation
python3 scripts/shard_csr_edges.py \
  --input data/prefix_dense_hybrid.csr \
  --output-dir data/edges_shards/ \
  --shard-size 1000
```

**Rust-side loading:**
```rust
pub fn load_sharded_graph(shard_dir: &Path) -> Result<ShardedGraph> {
  let manifest = read_manifest(shard_dir)?;
  let shards = load_shards(&manifest)?;
  Ok(ShardedGraph { shards, manifest })
}
```

---

### 2. Generation (subgraph-based)

**Goal:** Traverse prompt-local subgraph, avoid loading full corpus.

```rust
pub struct Generator {
  pub graph: ShardedGraph,
  pub hnsw: HnswIndex,
  pub tokenizer: Tokenizer,
  pub config: GeneratorConfig,
}

impl Generator {
  pub fn generate(&mut self, prompt: &str, max_tokens: usize) -> Result<Vec<String>> {
    // 1. Tokenize prompt (F18 — real tokenizer, not hash)
    let token_ids = self.tokenizer.encode(prompt)?;
    let current = *token_ids.last().unwrap();
    
    // 2. Build subgraph (F21)
    let subgraph = build_subgraph(
      &self.graph.shards,
      &token_ids,
      self.config.subgraph_depth,
    )?;
    
    // 3. Walk forward with bidirectional rescoring (F16)
    let mut output = Vec::new();
    for _ in 0..max_tokens {
      let candidates = self.successors(&subgraph, current);
      let selected = self.sample(&candidates, self.config.temperature)?;
      
      output.push(selected.token_text.clone());
      current = selected.id;
      
      if current == EOS_TOKEN { break; }
    }
    
    Ok(output)
  }
  
  fn successors(&self, subgraph: &Subgraph, current: u32) -> Vec<Candidate> {
    // Try CSR edges first (F1 — sequential, O(k) fast)
    let csr_cands = subgraph.forward.successors(current);
    
    if !csr_cands.is_empty() {
      // Bidirectional rescoring (F16)
      return csr_cands.iter()
        .map(|edge| {
          let score_f = edge.weight;
          let score_b = subgraph.backward.backward_support(current, edge.dst);
          let score = score_f * score_b.powf(self.config.alpha);
          Candidate { id: edge.dst, score, .. }
        })
        .collect();
    }
    
    // Fallback: HNSW KNN in embedding space (F1, F5)
    let current_embedding = self.get_embedding(current)?;
    let knn_ids = self.hnsw.search(&current_embedding, self.config.knn_k)?;
    
    knn_ids.iter()
      .map(|&id| Candidate { id, score: 0.5, .. })  // uniform for KNN
      .filter(|cand| !cand.successors.is_empty())  // F5 dead-end filter
      .collect()
  }
}
```

**Performance (F21 table):**
| Mode | Load/build | RSS | tok/s |
|------|-----------|-----|-------|
| Full graph | 2.79 s | 1140 MB | 192 |
| Subgraph depth=2 | 0.65 s | 617 MB | ~41,600 |
| Subgraph depth=3 | 2.29 s | 693 MB | ~4,850 |

---

### 3. Transferability Check (cross-model)

**Goal:** Avoid expensive re-extraction when upgrading models.

```rust
pub fn check_transferability(
  edges_a: &CsrIndex,
  edges_b: &CsrIndex,
  top_k: usize,
) -> TransferabilityReport {
  let mut overlap = Vec::new();
  
  for src in 0..vocab_size {
    let succ_a = edges_a.top_k(src, top_k);
    let succ_b = edges_b.top_k(src, top_k);
    let intersection = succ_a.iter().filter(|x| succ_b.contains(x)).count();
    overlap.push(intersection as f32 / top_k as f32);
  }
  
  let mean_overlap = overlap.iter().sum::<f32>() / overlap.len() as f32;
  
  if mean_overlap > 0.5 {
    TransferabilityReport::FullTransfer
  } else if mean_overlap > 0.2 {
    TransferabilityReport::PartialTransfer { displaced_tokens: compute_displaced() }
  } else {
    TransferabilityReport::NoTransfer
  }
}
```

**Result on Qwen3.5-4B ↔ Qwen3.6-36B (F28):** 4.3% overlap → NoTransfer, full re-extraction required.

---

## File Layout

```
native_v3.storage
├── wal/
│   ├── 000000001.log          -- append-only change log
│   └── ...
├── graph/
│   ├── csr_forward.csr         -- primary adjacency (sharded)
│   ├── csr_backward.csr       -- reverse edges
│   ├── shards/                -- 1000-source shards (F21)
│   │   ├── shard_0000.csr
│   │   ├── shard_0001.csr
│   │   └── ...
│   └── manifest.json          -- shard metadata
├── hnsw/
│   ├── hnsw_index.bin         -- HNSW graph structure
│   └── vectors.bin            -- embedding vectors
├── sql/
│   └── database.sqlite        -- property store, layout graph
└── pubsub/
    └── subscriptions.json      -- active topic subscriptions
```

---

## API Surface

```rust
pub struct Storage {
  pub conn: Connection,
  pub csr: ShardedGraph,
  pub hnsw: HnswIndex,
  pub pubsub: PubSubManager,
}

impl Storage {
  // Graph ops
  pub fn insert_edge(&mut self, edge: CsrEdge) -> Result<()>;
  pub fn batch_insert_edges(&mut self, edges: Vec<CsrEdge>) -> Result<()>;
  pub fn successors(&self, src: u32) -> Vec<CsrEdge>;
  pub fn predecessors(&self, dst: u32) -> Vec<CsrEdge>;
  
  // HNSW ops
  pub fn insert_vector(&mut self, id: u32, vector: &[f32]) -> Result<()>;
  pub fn knn_search(&self, query: &[f32], k: usize) -> Result<Vec<NodeId>>;
  
  // Pub/Sub ops
  pub fn subscribe(&self, topic: &str) -> Receiver<Change>;
  pub fn publish(&mut self, topic: &str, change: Change) -> Result<()>;
  
  // Transfer ops
  pub fn check_transferability(&self, other_graph: &CsrIndex) -> TransferabilityReport;
  pub fn remap_embeddings(&mut self, new_embeddings: &[f32]) -> Result<Vec<NodeId>>;
}
```

---

## Success Criteria

1. **Speed:** >1,000 tok/s on sequential edges (F1 baseline)
2. **Memory:** <700 MB RSS with depth=2 subgraph (46% reduction)
3. **Coverage:** <5% dead ends (hybrid prefix + LM-head edges)
4. **Transfer:** <10% edge rebuild on model fine-tune (if overlap >50%)
5. **Pub/Sub:** <10ms change propagation latency

---

## Open Problems (from graphtransformer)

1. **Context conditioning** (F24, F27): Centroid-bucket failed (−6.6% MRR, 26% template overlap). Need stronger signature or different conditioning mechanism.

2. **Coherence gap**: Unconditioned LM-head prior produces word salad (F14, F19). Full prefix coverage or context encoder required.

3. **Topology transferability** (F28): 4% overlap between Qwen3.5-4B and Qwen3.6-36B → edge extraction NOT reusable across models.

4. **Coverage gap** (F11): 86% vocabulary lacks sequential edges → dense hybrid edges required.

---

## Next Steps

1. **Implement sharded CSR loader** — F21 proof-of-concept exists, needs native Rust port
2. **Pub/Sub worker** — WAL reader + topic routing
3. **HNSW incremental insert** — transferable doc TurboQuant layer
4. **Bidirectional index** — F16 rescore algorithm
5. **Transferability check** — F28 topology overlap metric

---

## References

- **graphtransformer CHANGELOG.md** — F1–F28 experiments, especially F21 (sharding), F16/F17 (bidirectional), F24/F27/F28 (conditioning + transferability)
- **transferable-graph-embedding-manifold.md** — HNSW incremental insert, cross-model topology transfer
- **TurboQuant layer** — HNSW reuse across model variants
