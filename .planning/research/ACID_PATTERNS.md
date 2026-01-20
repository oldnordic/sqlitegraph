# ACID Transaction Patterns Research

**Research Date:** 2026-01-20
**Focus:** Embedded database ACID implementation with Write-Ahead Logging

## Overview

ACID transactions are the foundation of database reliability. This research covers patterns for implementing Atomicity, Consistency, Isolation, and Durability in embedded databases with Write-Ahead Logging (WAL).

## Key Sources

- [How to Build an ACID Compliant Database - Deeb (Aug 2025)](https://www.deebkit.com/posts/how-to-build-acid)
- [The Write-Ahead Log: A Foundation for Reliability (Dec 2024)](https://www.architecture-weekly.com/p/the-write-ahead-log-a-foundation)
- [Implementation of ACID Transaction in Database (Nov 2024)](https://dev.to/jaiminbariya/implementation-of-acid-transaction-in-database-44nc)
- [BPF-DB: A Kernel-Embedded Transactional Database (2025)](https://www.pdl.cmu.edu/PDL-FTP/Database/butrovich-sigmod2025.pdf)
- [A Write-Ahead Log is Not Universal (July 2024)](https://notes.eatonphil.com/2024-07-01-a-write-ahead-log-is-not-a-universal-part-of-durability.html)
- [nano-wal - Rust WAL crate](https://lib.rs/database-implementations)

---

## Atomicity (A)

**Definition:** All operations in a transaction succeed or none do.

### Implementation Patterns

**1. Write-Ahead Logging (WAL) Pattern**
```
1. BEGIN_TRANSACTION
2. Write operation records to WAL (before data pages)
3. Mark transaction as COMMITTED in WAL
4. Flush WAL to disk (fsync)
5. Apply changes to actual data pages (can be deferred)
6. Write checkpoint record
```

**Key insight:** WAL must be flushed BEFORE data pages for atomicity.

**2. Undo Log Pattern (for rollback)**
```
1. Capture before-image for each modified record
2. Store undo information in WAL
3. On rollback: apply undo records in reverse order
4. Reclaim resources (slots, locks)
```

**Critical for node deletion:** Must capture enough state to reconstruct deleted nodes.

### SQLiteGraph Implementation Gap

**Current issue:** Node deletion WAL replay is stubbed with warning log.
**Impact:** Crash after node deletion cannot restore correctly.
**Fix required:** Capture rollback data during node deletion (before-image of node record).

---

## Consistency (C)

**Definition:** Database transitions from one valid state to another.

### Implementation Patterns

**1. Constraint Validation**
```
1. Validate before commit (NOT after WAL write)
2. Check referential integrity
3. Check unique constraints
4. Check data type constraints
```

**2. Validation Points**
- **Pre-commit:** Before WAL flush (fail fast)
- **Post-replay:** After crash recovery (detect corruption)
- **Checkpoint:** When creating consistent snapshots

### SQLiteGraph Implementation Gaps

**Disabled cluster overlap validation:** Code commented out due to "timing issues"
- Location: `node_record_v2/validation.rs:79-119`
- Risk: Silent data corruption
- Fix: Implement validation after allocation completion

**Checkpoint state validation mismatch:** 40 lines of commented code
- Location: `wal/checkpoint/validation/invariants.rs:236-275`
- Risk: Checkpoint corruption undetected
- Fix: Update validation to match actual `CheckpointState` enum

---

## Isolation (I)

**Definition:** Concurrent transactions don't interfere with each other.

### Isolation Levels (ANSI SQL standard)

| Level | Dirty Reads | Non-Repeatable Reads | Phantom Reads | Implementation |
|-------|-------------|---------------------|---------------|----------------|
| READ UNCOMMITTED | Yes | Yes | Yes | No locking |
| READ COMMITTED | No | Yes | Yes | Row-level locks |
| REPEATABLE READ | No | No | Yes | Snapshot isolation |
| SERIALIZABLE | No | No | No | Full locking |

### Implementation Patterns

**1. MVCC (Multi-Version Concurrency Control)**
```
1. Readers access snapshot (old version)
2. Writers create new version
3. No reader-writer blocking
4. Cleanup old versions after all readers finish
```

**SQLiteGraph has:** MVCC-lite with ArcSwap<Arc<SnapshotState>>
**Gap:** Only single-writer supported; concurrent writes not implemented

**2. Two-Phase Locking (2PL)**
```
1. Growing phase: Acquire locks, don't release
2. Shrinking phase: Release locks, don't acquire
3. Deadlock detection required
```

**SQLiteGraph gap:** Deadlock detection incomplete (placeholder parameters)

**3. Deadlock Detection**
```
1. Wait-for graph: Track which transactions wait for which
2. Cycle detection: Find cycles in wait-for graph
3. Victim selection: Abort one transaction in cycle
```

**SQLiteGraph implementation:**
- Transaction-level detection exists
- Resource-level detection stubbed
- Files: `transaction_coordinator.rs:274,367`

---

## Durability (D)

**Definition:** Committed transactions survive crashes.

### Implementation Patterns

**1. WAL + Checkpoint**
```
Normal operation:
  - Write changes to WAL
  - Flush WAL (fsync)
  - Update data pages (async)
  - Periodic checkpoint (WAL -> data pages)

Crash recovery:
  - Load last checkpoint
  - Replay WAL from checkpoint LSN
  - Apply committed transactions
  - Discard uncommitted/rolled back
```

**2. Checkpoint Strategies**

| Strategy | Trigger | Pros | Cons |
|----------|---------|------|------|
| Time-based | Interval | Simple | May miss data |
| Transaction-count | N transactions | Predictable | Variable WAL size |
| Size-based | WAL threshold | Bounded WAL | Complex to implement |

**SQLiteGraph gap:** Only time-based strategy wired; others return hardcoded `false`.

### Recovery Best Practices

**1. Recovery Sequence**
```
1. Find last valid checkpoint
2. Read checkpoint LSN
3. Scan WAL from checkpoint LSN
4. For each record:
   - If COMMITTED: apply
   - If ROLLED_BACK: skip (or apply undo)
   - If IN_PROGRESS: treat as rolled back
5. Update checkpoint
```

**2. Parallel Recovery**
```
1. Sort transactions by LSN
2. Replay in parallel (respect dependencies)
3. Aggregate results
4. Expected speedup: 2-3x for large WALs
```

**SQLiteGraph:** Has parallel recovery (rayon) from v1.0
**Remaining gap:** Node deletion rollback not implemented

---

## Anti-Patterns to Avoid

**1. Flushing WAL After Data Pages**
- Wrong order breaks atomicity
- Correct: WAL flush → data pages

**2. Skipping Validation**
- Commented out validation code
- Deferring validation "until later"
- Result: Silent corruption

**3. Incomplete Rollback**
- Stubbed rollback operations
- Missing undo information
- Result: Inconsistent state after crash

**4. Unsafe Lifetime Extension**
- `std::mem::transmute` to extend lifetimes
- Breaks Rust's safety guarantees
- Risk: Use-after-free, data races

---

## Implementation Checklist for SQLiteGraph v1.1

### Atomicity
- [ ] Capture before-image for node deletion
- [ ] Implement node deletion rollback in WAL replayer
- [ ] Add rollback tests for all operation types
- [ ] Verify rollback correctness with crash tests

### Consistency
- [ ] Re-enable cluster overlap validation
- [ ] Fix checkpoint state validation
- [ ] Add pre-commit constraint checks
- [ ] Add post-recovery validation

### Isolation
- [ ] Implement resource-level deadlock detection
- [ ] Add transaction isolation level API
- [ ] Design concurrent write coordination
- [ ] Add concurrent write tests

### Durability
- [ ] Wire transaction-count checkpoint trigger
- [ ] Wire size-based checkpoint trigger
- [ ] Test all checkpoint strategies
- [ ] Verify recovery after all crash scenarios

---

## References

- [StackOverflow: Why WAL over command log](https://stackoverflow.com/questions/14181180/why-do-sql-databases-use-a-write-ahead-log-over-a-command-log)
- [VLDB: Replicated Write-Ahead Logging](https://www.vldb.org/pvldb/vol17/p3745-xu.pdf)
- [GitHub: Write-Ahead Logging Topic](https://github.com/topics/write-ahead-logging)

---
*ACID Patterns Research: 2026-01-20*
