# CSR Sharded Subgraph Builder — Phase 1 Spec

## Goal
Enable 10× faster graph traversal via sharded CSR subgraphs with 46% RSS reduction

## Phase 1: CSR Shard Reader

### Scope
Implement CSR shard file reader + manifest parser for loading 249 shards into ShardedGraph struct

### Success Criteria
- [ ] `cargo test shard_reader --exact` passes all unit tests
- [ ] `cargo test --lib` shows no regressions in existing graph code
- [ ] Shard reader returns 249 shards with total edge count 4,018,804
- [ ] `shard_reader --shard-dir data/qwen35_4b_prefix_dense_hybrid_train_shards/` completes in <2s

### Checkpoint
User runs shard reader on real data, confirms:
- All 249 shards load successfully
- Total edges match manifest.json
- Load time < 2s before proceeding to Phase 2 (subgraph builder)

### Context
**Prior work:** 
- graphtransformer F21: Sharded subgraph prototype (docs/NATIVE_V3_SPEC.md lines 849-926)
- Python script `scripts/shard_csr_edges.py` creates 1000-source shards
- Format: Each shard = CSR edges with source range [source_start, source_end)

**Constraints:**
- Must read existing CSR format (no format changes)
- Compatible with bidirectional index (Phase 3)
- No breaking changes to existing graph queries

**Dependencies:**
- Shard files exist in `data/qwen35_4b_prefix_dense_hybrid_train_shards/`
- `manifest.json` contains shard metadata (shard_id, source_start, source_end, edge_count)
- Serde for JSON parsing

### Design Decisions

1. **Shard storage: In-memory Vec<CsrShard>**
   - Tradeoff: Loads all 249 shards into RAM (~225 MB) vs lazy loading
   - Why: Subgraph builder needs random access across shards; lazy loading would add disk seeks on every BFS hop
   - Verify: Does ~225 MB RSS fit within your <700 MB target for depth=2? (Should: 225 MB shard data + 80 MB subgraph buffer = 305 MB total)

2. **Use serde for CSR deserialization**
   - Tradeoff: Adds serde dependency vs manual binary parsing faster
   - Why: Correctness over speed; CSR format simple, serde overhead negligible
   - Verify: Accept serde dependency? (serde already in Cargo.toml for other types)

3. **Error handling: Fail fast if shard missing**
   - Tradeoff: Panic vs Result return for missing shards
   - Why: Missing shard = corrupt data; better to fail immediately than silently return wrong graph
   - Verify: Should shard loader be graceful (skip missing) or strict (fail all)? → **Strict: fail all if any shard missing**

### Verification Plan

**Unit tests:**
```bash
# Test shard parsing
cargo test shard_reader --exact

# Test no regressions
cargo test --lib
```

**External signals:**
```bash
# Verify against real data
shard_reader --shard-dir data/qwen35_4b_prefix_dense_hybrid_train_shards/
echo "Shards loaded: $(shard_reader --count)"
echo "Total edges: $(shard_reader --total-edges)"
# Expected: Shards=249, Edges=4,018,804

# Verify load time
time shard_reader --shard-dir data/qwen35_4b_prefix_dense_hybrid_train_shards/
# Expected: <2s real time
```

**Correctness check:**
```bash
# Verify shard counts match manifest
python3 -c "
import json
with open('data/qwen35_4b_prefix_dense_hybrid_train_shards/manifest.json') as f:
    manifest = json.load(f)
    print(f'Manifest shards: {len(manifest["shards"])}')
    print(f'Manifest edges: {sum(s["edge_count"] for s in manifest["shards"])}')
"
# Expected: 249 shards, 4,018,804 edges

# Compare with Rust output
# Should match exactly
```

---

## Phase 2 Preview (Not Yet Spec'd)

**Next phase:** Subgraph builder using shards loaded in Phase 1

**Phase 1 output → Phase 2 input:**
- `ShardedGraph { shards: Vec<CsrShard>, manifest: Manifest }`
- Phase 2 BFS builder uses `shards` for prompt-local subgraph construction

**Pre-requisite for Phase 2:** Phase 1 checkpoint verified by user

---

## Risk Assessment

**Complexity:** Low-Medium (file I/O, deserialization, no graph algorithms)

**Dependencies:** Shard files exist (from graphtransformer F23 chunked extraction)

**Integration risk:** None (new isolated module, no existing code touched)

**Rollback plan:** If Phase 1 fails or spec wrong:
1. Revert commit: `git reset --hard HEAD~1`
2. Adjust spec and retry
3. Shard files remain untouched (safe to retry)

---

## Definition of Done

Phase 1 complete when:
- [ ] All success criteria met (tests pass, shard counts correct)
- [ ] Checkpoint verified by user (load time < 2s, edge counts match)
- [ ] No regressions (`cargo test --lib` clean)
- [ ] External signal verified (real data loads correctly)
- [ ] Code committed with message following convention
- [ ] Ready for Phase 2 (subgraph builder spec)

---

## Implementation Notes

**File structure:**
```
src/sharding/
  mod.rs          // Module exports
  shard.rs       // CsrShard, ShardReader types
  manifest.rs    // Manifest type, parsing
  reader.rs      // ShardReader impl
tests/
  sharding/
    shard_reader_test.rs  // Unit tests
```

**Key types (sketch):**
```rust
pub struct CsrShard {
    pub shard_id: usize,
    pub source_start: u32,
    pub source_end: u32,
    pub edges: Vec<CsrEdge>,
}

pub struct ShardReader {
    shards: Vec<CsrShard>,
    manifest: Manifest,
}

impl ShardReader {
    pub fn from_dir(shard_dir: &Path) -> Result<Self>;
    pub fn total_edges(&self) -> usize;
    pub fn get_shard(&self, shard_id: usize) -> Option<&CsrShard>;
}
```

**Estimated effort:** 4-6 hours
- Type definitions: 1h
- Manifest parsing: 1h
- Shard loading logic: 2h
- Unit tests: 1h
- Integration verification: 1h
