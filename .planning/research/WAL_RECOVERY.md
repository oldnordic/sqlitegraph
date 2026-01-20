# WAL Recovery and Checkpoint Best Practices

**Research Date:** 2026-01-20
**Focus:** Write-Ahead Log recovery patterns and checkpoint strategies for embedded databases

## Overview

Write-Ahead Logging (WAL) is the foundation of database durability and crash recovery. This research covers recovery patterns, checkpoint strategies, and best practices for embedded databases.

## Key Sources

- [Understanding Crash Recovery in Databases](https://adamdjellouli.com/articles/databases_notes/11_security_best_practices/07_crash_recovery_in_databases)
- [Fast Checkpoint Recovery for Frequently Updated Databases - CMU](https://15721.courses.cs.cmu.edu/spring2016/papers/p265-cao.pdf)
- [Main Memory Database Recovery Strategies - SIGMOD 2023](https://2023.sigmod.org/tutorials/tutorial1.pdf)
- [8 Steps for PostgreSQL Disaster Recovery - PGEdge (Apr 2025)](https://www.pgedge.com/blog/8-steps-to-proactively-handle-postgresql-database-disaster)
- [Fast Checkpoint and Recovery - MIT](https://dspace.mit.edu/bitstream/handle/1721.1/91701/894502502-MIT.pdf?sequence=2)
- [Evaluation of Checkpoint Recovery - VLDB 2009](https://www.cs.cornell.edu/~tuancao/2009-VLDB-Checkpoint.pdf)
- [Best Practices for SQL Backup - Oreateai (Dec 2025)](https://www.oreateai.com/blog/best-practices-for-sql-database-backup-and-recovery-strategies/5a9b9e23f4c65d88a7e26dbe8f45c8e9)

---

## WAL Architecture

### WAL Purpose

1. **Durability:** Committed changes survive crashes
2. **Atomicity:** All-or-nothing transaction semantics
3. **Recovery:** Restore consistent state after crash
4. **Performance:** Async flush of data pages

### WAL Record Types

| Record Type | Purpose | Rollback Action |
|-------------|---------|------------------|
| BEGIN | Mark transaction start | Discard |
| INSERT | Data insertion | DELETE (with undo info) |
| UPDATE | Data modification | UPDATE (with before-image) |
| DELETE | Data deletion | INSERT (with before-image) |
| COMMIT | Mark transaction complete | Apply |
| ABORT | Mark transaction rollback | Discard |
| CHECKPOINT | Snapshot marker | N/A |

### WAL Format

```
[WAL Header]
  - Magic number
  - Format version
  - Page size

[Records]
  - LSN (Log Sequence Number) - 8 bytes
  - Transaction ID - 8 bytes
  - Record type - 1 byte
  - Payload length - 4 bytes
  - Payload / Undo information
  - CRC checksum

[Checkpoint Record]
  - Checkpoint LSN
  - Timestamp
  - Transaction count
  - Data page positions
```

---

## Recovery Algorithm

### Standard Recovery Sequence

```
RECOVERY():
    1. Find last valid checkpoint in WAL
    2. Load checkpoint data pages into memory
    3. Get checkpoint LSN
    4. Scan WAL from checkpoint LSN to end
    5. For each transaction:
        a. If COMMITTED: apply all operations
        b. If ABORTED: skip all operations
        c. If IN_PROGRESS: treat as ABORTED (crash during commit)
    6. Write new checkpoint
    7. Truncate WAL (optional)
```

### Parallel Recovery (from v1.0)

SQLiteGraph has rayon-based parallel recovery:

```
PARALLEL_RECOVERY():
    1. Read all transactions from WAL
    2. Sort by LSN (establish ordering)
    3. Group independent transactions
    4. Replay groups in parallel (rayon par_iter)
    5. Sequential aggregation of results
    6. AtomicU64 counters for statistics

Speedup: 2-3x for large WAL files (500+ transactions)
```

### Recovery Completeness

**Critical gap:** Node deletion rollback is stubbed

**Current state:**
```rust
// operations_with_problematic_tests.rs:455-457
fn replay_node_deletion(...) {
    warn!("Node deletion replay not implemented");
    // TODO: Implement rollback
}
```

**Required for complete rollback:**
1. Capture before-image of deleted node
2. Store deleted node's slot position
3. Capture before-images of all edges (incoming/outgoing)
4. On rollback: restore node, restore edges, reclaim slot

---

## Checkpoint Strategies

### Strategy Comparison

| Strategy | Trigger | Pros | Cons | Use Case |
|----------|---------|------|------|----------|
| **Time-based** | Interval | Simple, predictable | May miss data, variable WAL size | General purpose |
| **Transaction-count** | N txns | Bounded work | Variable time | High-throughput |
| **Size-based** | WAL threshold | Bounded disk | Complex | Disk-constrained |
| **Hybrid** | Any trigger | Flexible | Most complex | Production |

### SQLiteGraph Current State

**Implemented:**
- ✅ Time-based checkpoint trigger
- ✅ Background checkpoint thread (configurable)

**Stubbed (return hardcoded `false`):**
- ❌ Transaction-count trigger (`checkpoint/core.rs:676`)
- ❌ Size-based trigger (`checkpoint/core.rs:679`)
- ❌ WAL-full trigger (`checkpoint/core.rs:682`)

**Implementation required:**

```rust
// Add to WAL manager
struct WalMetrics {
    transactions_since_checkpoint: AtomicU64,
    current_wal_size: AtomicU64,
    last_checkpoint_time: Instant,
}

// Transaction-count trigger
fn should_checkpoint_transaction_count(&self) -> bool {
    self.metrics.transactions_since_checkpoint.load(Ordering::Relaxed)
        >= self.config.max_transactions_before_checkpoint
}

// Size-based trigger
fn should_checkpoint_size(&self) -> bool {
    self.metrics.current_wal_size.load(Ordering::Relaxed)
        >= self.config.max_wal_size_before_checkpoint
}
```

### Checkpoint Process

```
CHECKPOINT():
    1. Stop accepting new transactions (or queue them)
    2. Flush all pending transactions to WAL
    3. Write CHECKPOINT record with current LSN
    4. Flush WAL (fsync)
    5. Copy dirty data pages to main storage
    6. Flush main storage (fsync)
    7. Update checkpoint metadata
    8. Resume accepting transactions
```

**Fuzzy checkpointing (alternative):**
- Don't stop accepting transactions
- Copy pages while allowing concurrent writes
- More complex but better availability

---

## Checkpoint Validation

### Invariant Checking

**Current gap:** 40 lines of commented validation code

**Location:** `wal/checkpoint/validation/invariants.rs:236-275`

**Required invariants:**
```rust
// Checkpoint state should be valid
fn validate_checkpoint_state(state: &CheckpointState) -> Result<()> {
    match state {
        CheckpointState::Idle => Ok(()),  // Valid
        CheckpointState::InProgress { .. } => {
            // Verify LSN is valid
            // Verify metadata file exists
            // Verify no corruption
        }
        CheckpointState::Complete { .. } => {
            // Verify checkpoint file exists
            // Verify LSN is monotonically increasing
        }
        _ => Err(CheckpointError::InvalidState),
    }
}
```

### Recovery Validation

**Post-recovery checks:**
```rust
fn validate_post_recovery(graph: &GraphFile) -> Result<()> {
    // 1. Check header magic
    // 2. Verify LSN consistency
    // 3. Validate cluster allocations (no overlap)
    // 4. Verify node/edge counts
    // 5. Check string table integrity
}
```

---

## Crash Scenarios

### Scenario Analysis

| Crash Point | WAL State | Recovery Action | Data Loss Risk |
|-------------|-----------|-----------------|----------------|
| Before WAL write | N/A | No recovery needed | None |
| During WAL write | Partial record | Skip to next checkpoint | Current txn only |
| After WAL flush, before data page | WAL intact | Replay WAL | None |
| During checkpoint | Partial checkpoint | Use previous checkpoint + replay WAL | None |
| After checkpoint, before WAL truncate | Both intact | Truncate WAL | None |

### Node Deletion Crash

**Worst case:** Crash after node deletion written to WAL but before rollback data captured

**Recovery:**
1. Detect incomplete transaction (no COMMIT record)
2. Check if rollback data exists
3. If yes: restore node, restore edges
4. If no: mark as corruption, log error

**This is why rollback data capture is critical.**

---

## Best Practices

### DO ✅

1. **Flush WAL before data pages**
   - Ensures atomicity
   - Enables recovery

2. **Use LSN (Log Sequence Number)**
   - Monotonically increasing
   - Enables ordering and replay

3. **Capture before-images for all mutations**
   - Enables rollback
   - Required for node deletion

4. **Validate after recovery**
   - Detect corruption early
   - Don't assume recovery succeeded

5. **Test crash scenarios**
   - Kill process during writes
   - Verify recovery
   - Automate with tests

### DON'T ❌

1. **Skip rollback implementation**
   - Current node deletion stub
   - Data integrity risk

2. **Flush data pages before WAL**
   - Breaks atomicity
   - Wrong order

3. **Ignore validation**
   - Commented out code
   - Silent corruption

4. **Assume success**
   - Always verify fsync return
   - Always verify checksums

5. **Hardcode return values**
   - Current checkpoint triggers
   - Non-functional code

---

## Implementation Checklist for SQLiteGraph v1.1

### WAL Recovery
- [ ] Implement node deletion rollback
- [ ] Capture before-images for all mutations
- [ ] Add crash simulation tests
- [ ] Verify all rollback operations

### Checkpoint Triggers
- [ ] Wire transaction-count trigger
- [ ] Wire size-based trigger
- [ ] Add transaction counter to WAL metrics
- [ ] Track WAL file size
- [ ] Test all checkpoint strategies

### Validation
- [ ] Fix checkpoint state validation
- [ ] Add post-recovery validation
- [ ] Enable cluster overlap validation
- [ ] Add checksum verification

### Testing
- [ ] Crash tests at each WAL operation
- [ ] Recovery tests for all crash points
- [ ] Checkpoint trigger tests
- [ ] Corruption detection tests

---

## References

- [Quora: How checkpoints are used in database recovery](https://www.quora.com/How-checkpoints-are-used-in-database-recovery)
- [SIGMOD 2023: Main Memory Database Recovery](https://2023.sigmod.org/tutorials/tutorial1.pdf)
- [MIT: Fast Checkpoint for In-Memory DBs](https://dspace.mit.edu/bitstream/handle/1721.1/91701/894502502-MIT.pdf?sequence=2)

---
*WAL Recovery and Checkpoint Research: 2026-01-20*
