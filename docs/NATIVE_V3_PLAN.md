# Native-v3 Storage Engine — Implementation Plan

## Goal
Build unified storage binary (SQL + Graph + CSR + HNSW + Pub/Sub) with 10× traversal speed and 46% RSS reduction

## Risk Assessment

**Complexity:** High (6 layers: Pub/Sub, CSR adjacency, bidirectional index, HNSW, SQL, sharding)

**Dependencies:**
- Magellan DB (existing sqlitegraph project)
- graphtransformer findings (F21 sharding, F16/F17 bidirectional, F23 chunked extraction)
- HNSW library (need to integrate or implement)

**Integration risk:**
- Must not break existing sqlitegraph API
- Graph schema changes require migration
- Performance targets depend on real data (122M-token corpus)

**Rollback plan:**
- Each phase is separate crate/module, can be disabled via feature flags
- If phase fails: revert commit, keep learnings, adjust spec
- Final integration in separate branch, merge only when all phases pass

---

## Phase Breakdown

### Phase 1: CSR Shard Reader (4-6 hours, 1-2 days)

**Scope:** Implement shard file reader + manifest parser

**Checkpoint:** User confirms shards load correctly, edge counts match manifest.json

**Success criteria:**
- [ ] `cargo test shard_reader --exact` passes
- [ ] `cargo test --lib` shows no regressions
- [ ] Shard reader returns 249 shards, 4,018,804 edges
- [ ] Load time < 2s on real data

**Verification:**
- [ ] `cargo test shard_reader --exact`
- [ ] `cargo test --lib`
- [ ] External: Verify against manifest.json (python3 check)
- [ ] External: Time `shard_reader --shard-dir data/shards/`

**Design decisions:**
1. **In-memory Vec<CsrShard>** (load all shards, not lazy)
   - Tradeoff: 225 MB RAM vs random disk access
   - Verify: Fits within 700 MB RSS target?

2. **Serde for JSON parsing**
   - Tradeoff: Add dependency vs manual parsing
   - Verify: Accept serde dependency?

**Integration:** Phase 1 output (`ShardedGraph`) → Phase 2 input

---

### Phase 2: Subgraph Builder (1-2 days)

**Scope:** Implement BFS subgraph builder from prompt tokens using sharded CSR

**Checkpoint:** User confirms subgraph returns correct node/edge counts vs full graph

**Success criteria:**
- [ ] `cargo test subgraph_builder --exact` passes
- [ ] Subgraph depth=2: 3,187 nodes, 4,329 edges (matches F21)
- [ ] Build time < 1s for depth=2
- [ ] RSS < 700 MB during build

**Verification:**
- [ ] `cargo test subgraph_builder --exact`
- [ ] External: Compare node counts vs graphtransformer Python output
- [ ] External: Benchmark `cargo bench subgraph_depth` → 41,600 tok/s

**Design decisions:**
1. **BFS depth=2 for generation** (F21 optimal)
   - Tradeoff: Depth=1 lacks context, depth=3 approaches full graph RSS
   - Verify: Sufficient for your coherence needs?

2. **Load only relevant shards** (don't load all 249)
   - Tradeoff: Compute shard ranges vs load everything
   - Verify: Shard range computation fast enough?

**Integration:** Phase 2 output (`Subgraph`) → Phase 3 input

---

### Phase 3: Bidirectional Index (1 day)

**Scope:** Build reverse edges from CSR for backward rescoring

**Checkpoint:** User confirms backward support computes correctly on test edges

**Success criteria:**
- [ ] `cargo test bidirectional --exact` passes
- [ ] `backward_support()` returns correct scores for test cases
- [ ] Bidirectional generation matches F16 results (245 tok/s vs baseline 192 tok/s)

**Verification:**
- [ ] `cargo test bidirectional --exact`
- [ ] External: Compare rescoring scores with graphtransformer output
- [ ] External: Run generation with `--bidirectional`, verify token counts

**Design decisions:**
1. **Build reverse at load time** (not on-demand)
   - Tradeoff: O(N) memory vs O(query) compute
   - Verify: Accept 2× edge memory for faster rescoring?

**Integration:** Phase 3 output (`BidirectionalIndex`) → Phase 4 input

---

### Phase 4: HNSW Semantic Layer (2-3 days)

**Scope:** Integrate HNSW for KNN fallback when CSR edges lack coverage

**Checkpoint:** User confirms HNSW search returns correct nearest neighbors

**Success criteria:**
- [ ] `cargo test hnsw_layer --exact` passes
- [ ] KNN search on qwen3.6 embeddings returns top-20 neighbors
- [ ] Incremental insert works (update after fine-tune)
- [ ] No multilingual chaos (F6: pure KNN = mixed languages)

**Verification:**
- [ ] `cargo test hnsw_layer --exact`
- [ ] External: Compare KNN results vs Python FAISS reference
- [ ] External: Generate with pure KNN mode, verify not multilingual chaos

**Design decisions:**
1. **Use existing HNSW crate** (don't implement from scratch)
   - Tradeoff: Add dependency vs implement ourselves
   - Verify: Which HNSW crate? (hnsw-rs, distwHNSW?)

2. **Incremental insert for fine-tune updates** (transferable doc)
   - Tradeoff: Complexity vs rebuild cost
   - Verify: Need fine-tune workflow now or later?

**Integration:** Phase 4 output (`HnswIndex`) → Phase 5 input

---

### Phase 5: SQL Property Store (1 day)

**Scope:** Implement node metadata storage in SQLite

**Checkpoint:** User confirms property queries return correct metadata

**Success criteria:**
- [ ] `cargo test property_store --exact` passes
- [ ] Node properties (token_text, embedding_vector, metadata) queryable
- [ ] Property updates persist across sessions

**Verification:**
- [ ] `cargo test property_store --exact`
- [ ] External: Query real node properties, verify token_text matches

**Design decisions:**
1. **Use rusqlite** (existing dependency)
   - Tradeoff: Synchronous API vs async
   - Verify: Synchronous OK for this use case?

**Integration:** Phase 5 output (`PropertyStore`) → Phase 6 input

---

### Phase 6: Pub/Sub Layer (2 days)

**Scope:** Implement WAL-backed change feed + topic routing

**Checkpoint:** User confirms pub/sub propagates changes correctly

**Success criteria:**
- [ ] `cargo test pubsub --exact` passes
- [ ] Subscriptions receive changes within 10ms
- [ ] Change feed survives restart (WAL replay)

**Verification:**
- [ ] `cargo test pubsub --exact`
- [ ] External: Insert edge, verify subscription receives change
- [ ] External: Kill process, restart, verify WAL replays correctly

**Design decisions:**
1. **WAL-based change feed** (append-only log)
   - Tradeoff: Write-ahead logging vs direct writes
   - Verify: WAL replay needed for crash recovery?

2. **Topic-based routing** (graph.edge.insert, hnsw.vector.insert, etc.)
   - Tradeoff: Granular topics vs catch-all
   - Verify: Which topics needed? (See KARPATHY_INTEGRATION.md)

**Integration:** Phase 6 completes all layers

---

### Phase 7: Public SQL Query Interface (2 days)

**Scope:** Implement public SQL query API for V3Backend (execute_sql, prepared statements)

**Checkpoint:** User confirms SQL queries execute against V3Backend correctly

**Success criteria:**
- [ ] `cargo test sql_query_interface --exact` passes
- [ ] `V3Backend::execute_sql(query: &str)` returns rowsets
- [ ] Prepared statements cache correctly
- [ ] Parameter binding works safely (no SQL injection)
- [ ] Query results serialize to JSON/CSV

**Verification:**
- [ ] `cargo test sql_query_interface --exact`
- [ ] External: Run `SELECT * FROM node_properties LIMIT 10` via CLI
- [ ] External: Verify parameterized queries prevent injection

**Design decisions:**
1. **rusqlite Statement cache** (prepare once, execute many)
   - Tradeoff: Memory vs parse overhead
   - Verify: Cache size limit (100 statements?)

2. **Result serialization** (Row → JSON/CSV)
   - Tradeoff: Flexibility vs performance
   - Verify: Which formats needed? (JSON, CSV, TSV?)

**Integration:** Phase 7 output (`SqlQueryInterface`) → Phase 8 input

---

### Phase 8: Transaction Management (1 day)

**Scope:** Implement BEGIN/COMMIT/ROLLBACK with MVCC integration

**Checkpoint:** User confirms transactions rollback correctly on errors

**Success criteria:**
- [ ] `cargo test transaction_management --exact` passes
- [ ] `V3Backend::begin_transaction()`, `commit()`, `rollback()` work
- [ ] Nested transactions with savepoints
- [ ] MVCC snapshots see only committed state
- [ ] Concurrent transactions don't deadlock (reasonable timeout)

**Verification:**
- [ ] `cargo test transaction_management --exact`
- [ ] External: Begin transaction, insert 100 nodes, rollback → verify count = 0
- [ ] External: Concurrent transaction test (2 threads, verify no deadlock)

**Design decisions:**
1. **SQLite savepoints** (for nested transactions)
   - Tradeoff: Complexity vs ergonomics
   - Verify: Need nested transactions or just flat?

2. **MVCC snapshot isolation** (reads see only committed state)
   - Tradeoff: Strict serializability vs performance
   - Verify: Use MVCC LSN for transaction boundaries?

**Integration:** Phase 8 output (`TransactionManager`) → Phase 9 input

---

### Phase 9: Schema Operations (2 days)

**Scope:** Implement CREATE/DROP/ALTER TABLE for user-defined schemas

**Checkpoint:** User confirms DDL operations persist correctly

**Success criteria:**
- [ ] `cargo test schema_operations --exact` passes
- [ ] `CREATE TABLE`, `DROP TABLE` work
- [ ] `ALTER TABLE ADD COLUMN` works
- [ ] Schema changes integrate with GraphBackend (node_properties, edge_attributes)
- [ ] Schema migrations work (version tracking)

**Verification:**
- [ ] `cargo test schema_operations --exact`
- [ ] External: `CREATE TABLE user_data (id INTEGER, name TEXT)` → verify via `.tables`
- [ ] External: `ALTER TABLE node_properties ADD COLUMN created_at TIMESTAMP`

**Design decisions:**
1. **Schema versioning** (track schema changes in system table)
   - Tradeoff: Complexity vs migration safety
   - Verify: Need automatic migrations or manual?

2. **Integration with existing tables** (node_properties, edge_attributes)
   - Tradeoff: Allow ALTER on system tables vs protect them
   - Verify: Which system tables are immutable?

**Integration:** Phase 9 completes public SQL API

---

## Integration Strategy

**How phases connect:**
```
Phase 1 (shards) → Phase 2 (subgraph) → Phase 3 (bidirectional) → Phase 4 (HNSW) → Phase 5 (SQL) → Phase 6 (pub/sub) → Phase 7 (SQL query API) → Phase 8 (transactions) → Phase 9 (schema DDL)
```

**Cross-phase invariants:**
- Node IDs stable (no remapping between phases)
- CSR format unchanged (all phases use same shard format)
- Graph schema compatible (no migration mid-project)

**Testing strategy:**
- Unit tests: Each phase
- Integration tests: After 2-3 phases (shard + subgraph, then add HNSW, then add pub/sub, then add SQL API, then add transactions)
- End-to-end tests: Final phase (generate with all layers active: SQL queries + graph ops + HNSW + MVCC + transactions)

---

## Verification Gates

**Pre-commit (local, fast):**
```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --lib
```

**Pre-push (CI, comprehensive):**
```bash
cargo test --workspace
cargo audit
cargo deny check
semgrep ci
```

**Pre-merge (external signal):**
```bash
# Deployment verification (simulated for native-v3)
cargo run --bin native_v3 --verify-deployment

# Coverage verification
cargo llvm-cov --html --output-dir coverage

# Graph state verification
magellan status --db ~/.magellan/sqlitegraph/sqlitegraph.db
```

---

## Rollback Strategy

**If Phase N fails:**

1. **Revert to previous working state:**
   ```bash
   git reset --hard HEAD~1  # Remove failed phase
   git checkout -b phase-N-retry  # Branch for retry
   ```

2. **Save learnings:**
   - What worked: [save]
   - What failed: [save]
   - Adjusted spec: [save]

3. **Retry with adjusted spec:**
   - Read `spec.md` again
   - Adjust design decisions
   - Re-implement Phase N

**If integration fails:**
- Bisect which phase causes issue
- Revert to last known good phase
- Keep earlier phases (they're verified)
- Adjust integration strategy

---

## Success Metrics

**Performance targets (from F21):**
- Subgraph depth=2: 41,600 tok/s, RSS < 700 MB
- Full graph (baseline): 192 tok/s
- Bidirectional: 245 tok/s (minimal overhead)

**Quality targets:**
- Zero `todo!()`, `unimplemented!()` in production code
- Zero `#[allow(dead_code)]`
- All tests passing (100% code coverage not required, but >80% ideal)

**Verification targets:**
- All external signals pass (deployment, coverage, graph sync)
- CI passes all gates (fmt, clippy, audit, deny, semgrep)
- Multi-agent verification: 2/3 critics approve for risky changes

---

## Definition of Done

**Project complete when:**
- [ ] All 9 phases implemented and verified
- [ ] Integration tests pass (all layers active: SQL + Graph + HNSW + MVCC + PubSub)
- [ ] Performance targets met (benchmarks show expected speed/RSS)
- [ ] External signals verified (real data loads correctly)
- [ ] CI passes all gates
- [ ] Documentation updated (ARCHITECTURE.md, API.md, CHANGELOG.md)
- [ ] Ready for deployment (binary built, tested in sqlitegraph-core)

---

## Estimated Timeline

**Phase 1:** 1-2 days (shard reader)
**Phase 2:** 1-2 days (subgraph builder)
**Phase 3:** 1 day (bidirectional index)
**Phase 4:** 2-3 days (HNSW layer)
**Phase 5:** 1 day (SQL property store)
**Phase 6:** 2 days (pub/sub layer)
**Phase 7:** 2 days (public SQL query API)
**Phase 8:** 1 day (transaction management)
**Phase 9:** 2 days (schema DDL operations)
**Integration + testing:** 3-4 days

**Total:** 17-21 days

**Critical path:** Phase 1 → 2 → 3 (core graph traversal)
**Can parallelize:** Phase 4 (HNSW) can start after Phase 2, Phase 5 (SQL) can start anytime, Phase 7-9 (SQL API) can start after Phase 5

---

## Next Steps After Plan Approval

1. **Review plan with user:**
   - Are phases too ambitious?
   - Are checkpoints clear?
   - Is rollback strategy sound?

2. **Begin Phase 1 implementation:**
   - Read `NATIVE_V3_PHASE1_SPEC.md`
   - Create types: `CsrShard`, `ShardReader`, `Manifest`
   - Implement unit tests
   - Verify with real data
   - Get user checkpoint approval

3. **Store discoveries to Atheneum:**
   - After each phase: store what worked, what failed, what changed
   - Build knowledge base for future sessions
