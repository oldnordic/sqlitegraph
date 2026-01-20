# Research Summary: v1.1 ACID & Reliability

**Research Date:** 2026-01-20
**Focus:** ACID transactions, HNSW multi-layer, WAL recovery, and Rust unsafe code patterns

---

## Key Findings

### ACID Transaction Patterns

**Critical insight:** SQLiteGraph has WAL but ACID is incomplete

| Property | Status | Gap |
|----------|--------|-----|
| **Atomicity** | Partial | Node deletion rollback stubbed |
| **Consistency** | Partial | Cluster validation disabled, checkpoint validation commented out |
| **Isolation** | Partial | Single-writer only, deadlock detection incomplete |
| **Durability** | Partial | Only time-based checkpoint works |

**Most critical:** Node deletion WAL replay must capture before-image data for complete rollback.

### HNSW Multi-Layer

**Performance impact:** Using only layer 0 means O(N) search instead of O(log N)

**Implementation:**
```rust
fn determine_insertion_level(&self) -> usize {
    let mut level = 0;
    let ml = self.config.ml; // Layer multiplier (0.5-0.7)
    while self.rng.gen::<f64>() < ml && level < self.config.max_layers {
        level += 1;
    }
    level
}
```

**Key parameters to add:**
- `M`: Max neighbors per node per layer (5-64)
- `ef_construction`: Candidates during insert (40-400)
- `ef_search`: Candidates during search (10-100)
- `ml`: Layer probability (0.3-0.7)

### WAL Recovery and Checkpoints

**3 checkpoint strategies return hardcoded `false`:**
- Transaction-count trigger
- Size-based trigger
- WAL-full trigger

**Recovery sequence:**
1. Find last valid checkpoint
2. Load checkpoint data pages
3. Replay WAL from checkpoint LSN
4. Apply COMMITTED, skip ABORTED, treat IN_PROGRESS as ABORTED
5. **Critical:** Node deletion must restore node + edges

### Rust Unsafe Code

**10+ locations use `std::mem::transmute` to extend `GraphFile` lifetime:**
- Risk: Use-after-free if owner dropped
- Safe alternative: `Arc<RwLock<GraphFile>>`
- Must add Miri testing to validate safety

---

## Implementation Priority

### Phase 1: Data Integrity (Highest)

1. **Node deletion WAL replay** — CONCERNS.md item
   - Capture before-image of deleted node
   - Capture all edges (incoming/outgoing)
   - Implement rollback in replayer
   - Tests: Crash scenarios, rollback correctness

2. **Cluster overlap validation** — CONCERNS.md item
   - Re-enable commented validation
   - Account for allocation sequencing
   - Tests: Corruption detection

3. **Checkpoint state validation** — CONCERNS.md item
   - Fix validation to match CheckpointState enum
   - Enable commented invariants

### Phase 2: Memory Safety

1. **Unsafe transmute audit** — CONCERNS.md item
   - Document all 10+ sites
   - Replace with `Arc<RwLock<GraphFile>>`
   - Add Miri testing

2. **Input sanitization** — CONCERNS.md item
   - JSON size limit
   - JSON depth limit
   - Tests: Malicious input

### Phase 3: ACID Completion

1. **Checkpoint strategies** — CONCERNS.md item
   - Wire transaction-count trigger
   - Wire size-based trigger
   - Track WAL metrics

2. **HNSW multi-layer** — CONCERNS.md item
   - Implement `determine_insertion_level()`
   - Add multi-layer data structure
   - Update insert/search for layers

3. **Deadlock detection** — CONCERNS.md item
   - Implement resource-level tracking
   - Build wait-for graph
   - Add cycle detection

### Phase 4: Structure & Performance

1. **Large file refactoring** — CONCERNS.md item
   - Split rollback.rs (1654 LOC)
   - Split hnsw/index.rs (1605 LOC)
   - Split checkpoint/operations.rs (1594 LOC)
   - Split algo.rs (1398 LOC)
   - Split validator.rs (1300 LOC)

2. **Clone operations audit** — CONCERNS.md item
   - Audit 263 clone() calls
   - Reduce unnecessary clones

3. **Connection pooling** — CONCERNS.md item
   - Implement for SQLite backend

### Phase 5: Scaling & Dependencies

1. **Scaling limits** — CONCERNS.md items
   - Multi-file checkpointing
   - Dirty block overflow strategy
   - Transaction ID bounds
   - Disk-based HNSW

2. **Dependencies** — CONCERNS.md items
   - Monitor rusqlite updates
   - Plan bincode 2.0 migration

---

## Stack Additions

**None required** — All work is internal refactoring and completion of existing systems.

**Dependencies to monitor:**
- rusqlite 0.31 — Security updates
- bincode 1.3 → 2.0 migration planning

---

## Architecture Integration

**No new components** — Work focuses on completing existing systems:

| Existing Component | Work Required |
|-------------------|---------------|
| WAL Recovery | Complete rollback, add validation |
| Checkpoint | Wire triggers, fix validation |
| HNSW | Add multi-layer |
| Transaction Coordinator | Complete deadlock detection |
| GraphFile | Replace with Arc<RwLock<GraphFile>> |

---

## Watch Out For

### High Risk 🔴

1. **Node deletion rollback not implemented** — Data corruption risk after crash
2. **Cluster validation disabled** — Silent corruption possible
3. **Unsafe transmute** — Potential use-after-free

### Medium Risk 🟡

1. **Checkpoint validation commented out** — Checkpoint corruption undetected
2. **Deadlock detection incomplete** — Potential deadlocks with concurrent writes
3. **Large files** — High complexity, fragile areas

### Low Risk 🟢

1. **HNSW single-layer** — Functional but suboptimal performance
2. **Clone operations** — Performance issue, not correctness
3. **Dependencies** — Monitor and update as needed

---

## Research Documents

- [ACID_PATTERNS.md](.planning/research/ACID_PATTERNS.md) — ACID transaction implementation patterns
- [HNSW_MULTILAYER.md](.planning/research/HNSW_MULTILAYER.md) — Multi-layer HNSW algorithm
- [WAL_RECOVERY.md](.planning/research/WAL_RECOVERY.md) — WAL recovery and checkpoint strategies
- [RUST_UNSAFE.md](.planning/research/RUST_UNSAFE.md) — Safe alternatives to unsafe transmute

---

## Sources

### ACID & WAL
- [How to Build an ACID Compliant Database](https://www.deebkit.com/posts/how-to-build-acid)
- [The Write-Ahead Log Foundation](https://www.architecture-weekly.com/p/the-write-ahead-log-a-foundation)
- [BPF-DB Kernel-Embedded Database](https://www.pdl.cmu.edu/PDL-FTP/Database/butrovich-sigmod2025.pdf)
- [Understanding Crash Recovery](https://adamdjellouli.com/articles/databases_notes/11_security_best_practices/07_crash_recovery_in_databases)

### HNSW
- [Original HNSW Paper](https://arxiv.org/abs/1603.09320)
- [Redis: HNSW Improves Search](https://redis.io/blog/how-hnsw-algorithms-can-improve-search/)
- [Milvus: Understanding HNSW](https://milvus.io/blog/understanding-hierarchical-navigable-small-worlds-hnsw-for-vector-search.md)
- [Pinecone: HNSW Layered Format](https://www.pinecone.io/learn/series/faiss/hnsw/)

### Rust & Memory Safety
- [Toward Safe Transmutation (LWN)](https://lwn.net/Articles/994334/)
- [Rust Release Notes 2025](https://doc.rust-lang.org/beta/releases.html)
- [Rust Advanced Concurrency](https://www.ruststepbystep.com/rust-advanced-concurrency-using-arc-mutex-and-rwlock-safely/)
- [RustSec Advisory Database](https://rustsec.org/advisories/)

### Checkpoint & Recovery
- [Fast Checkpoint Recovery - CMU](https://15721.courses.cs.cmu.edu/spring2016/papers/p265-cao.pdf)
- [Main Memory Recovery - SIGMOD 2023](https://2023.sigmod.org/tutorials/tutorial1.pdf)
- [Fast Checkpoint - MIT](https://dspace.mit.edu/bitstream/handle/1721.1/91701/894502502-MIT.pdf?sequence=2)

---
*Research Summary: 2026-01-20*
