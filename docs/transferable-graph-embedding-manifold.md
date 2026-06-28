# Transferable Graph — Embedding Manifold as Portable Topology

**Date:** 2026-06-26  
**Origin:** graphtransformer F21/F22 + native-v3 design session  
**Status:** Design insight — not yet implemented  
**Related:** native-v3-storage-engine.md, HNSW/TurboQuant layer

---

## Core Claim

A CSR graph built over an embedding space encodes the **manifold topology**, not just
the coordinates. Topology is more stable than coordinates. This makes the graph
transferable across model variants, domains, and fine-tuning steps.

---

## What "Transferable" Means

The graph encodes: which nodes are neighbors, which transitions are probable,
which paths are reachable. These structural properties survive moderate shifts
in the embedding space — the skeleton of the manifold is more stable than its
specific coordinates in R^n.

Three transfer directions:

### 1. Cross-Model (Same Family)

```
Model A (e.g. Qwen3.5-4B) → extract CSR graph
Model B (e.g. Qwen3.6-36B, same tokenizer) → map B's embeddings into A's graph
```

Same tokenizer = same node IDs. Embedding coordinates shift but connectivity
structure mostly survives. Find linear (or nearest-neighbor) map between
embedding spaces, remap node positions, keep edges.

Cost: one mapping pass over vocabulary. Not a full re-extraction.

### 2. Cross-Domain (Same Model)

Graph learned on prose corpus partially transfers to code corpus on the same model.
Structural tokens (`(`, `)`, `;`, `{`, `return`, `fn`) maintain their topology
because their co-occurrence patterns are domain-invariant.
Content tokens shift more. Partial transfer — reuse structure tokens, re-extract
content tokens only.

### 3. Incremental Fine-Tune

Model gets SFT'd or LoRA-adapted. Embedding space shifts locally.

```
1. Compute embedding delta: ||new_embed[i] - old_embed[i]|| for all i
2. Flag tokens above displacement threshold (e.g. top 5% movers)
3. Re-extract edges only for displaced tokens
4. Update those nodes in CSR, keep rest unchanged
```

Graph update, not full rebuild. Fine-tune cost: O(displaced_tokens × k)
not O(full_vocab × k).

---

## Why This Works — HNSW Already Does It

HNSW (Hierarchical Navigable Small World) builds a graph over vector space.
New vectors are inserted by finding neighbors in the existing graph structure.
The graph IS the manifold index.

This is not a new concept — it's the foundation of approximate nearest-neighbor search.
The insight here: **the same transferability property applies to CSR token-transition graphs**,
not just HNSW neighbor graphs.

A token-transition CSR encodes a directed manifold: `P(next | current)` defines
a flow field on the embedding space. The flow field topology (attractors, repellers,
transition paths) is more stable than the point coordinates.

---

## native-v3 / sqlitegraph Implications

### HNSW Index (TurboQuant layer)

TurboQuant HNSW index built on Model A's embeddings is partially reusable
for a fine-tuned variant:

```
Model A → build TurboQuant HNSW index (expensive, one-time)
Model A + SFT → compute displaced embeddings → insert into existing graph
                 update only displaced neighborhoods
```

Don't rebuild from scratch. Incremental insert into existing graph structure.
Cost drops from O(N log N) full build to O(displaced × log N) update.

### CSR Adjacency (Graph layer)

Cross-model reuse for graph traversal queries:
- Build symbol/entity graph on one codebase
- New version of codebase (refactored but same domain) → map new embeddings
  into existing graph, inherit edge structure for unchanged symbols
- Only re-extract edges for symbols that moved significantly in embedding space

### SQL Layer (Physical Layout Graph)

Schema topology (table→column→rowgroup→page) is stable across data updates.
Schema changes (ALTER TABLE, new index) = local graph update, not full rebuild.
The physical layout graph inherits its topology between schema versions.

---

## The Universal Statistics Claim

Token transition probabilities for common sequences are partially model-agnostic:
- `the → ` (article completion)
- `( → ` (argument patterns)
- `if → ` (conditional patterns)

These hold across model sizes and families because they reflect statistical
regularities in training corpora, not model-specific representations.

**Implication:** A graph built from a small model (fast extraction) partially transfers
to a large model (expensive extraction). Extract cheaply on the small model,
fine-tune the edge weights on the large model, save the full extraction cost.

---

## Experiment Design

To validate transferability:

1. Extract CSR graph from Qwen3.5-4B (done — existing edge files)
2. Extract CSR graph from Qwen3.6-36B (target)
3. Measure edge overlap: what fraction of top-k successors match between the two graphs?
4. If overlap > 50%: topology is transferable, only weight rescaling needed
5. If overlap < 20%: models differ too much, full re-extraction required

Intermediate result (40-70% overlap) = partial transfer, re-extract only
low-overlap nodes.

---

## Summary

| Transfer Type | Cost | When to Use |
|--------------|------|-------------|
| Cross-model same family | One mapping pass | Model upgrade, same tokenizer |
| Cross-domain same model | Re-extract content tokens only | Domain shift (prose→code) |
| Incremental fine-tune | Re-extract displaced tokens only | SFT, LoRA, quantization |
| Full rebuild | O(vocab × k) extraction | New tokenizer, architecture change |

Transferable graphs turn expensive extraction into a one-time cost
amortized across model versions, domains, and fine-tuning cycles.
