# Phase 13: ACID Isolation - Research

**Researched:** 2026-01-20
**Domain:** Transaction coordination, deadlock detection, isolation levels
**Confidence:** HIGH

## Summary

Phase 13 requires implementing transaction coordination with deadlock detection and isolation level management for the V2 native backend. **Significant foundational work already exists** in `/sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs`. The main implementation task is to:

1. **Activate and complete the existing skeleton code** - The `V2TransactionCoordinator`, `V2LockManager`, `DeadlockDetector`, and `IsolationManager` are already defined but contain placeholder implementations (noted by `TODO` comments)
2. **Implement cycle detection and victim selection** - The wait-for graph DFS traversal exists but needs integration with actual lock wait tracking
3. **Define lock acquisition ordering** - No ordering strategy is documented yet (critical for CW-02)
4. **Remove async/await requirements** - Current code uses `async fn` but `tokio` is not available (tests are `#[ignore]` with note about "requires tokio runtime")
5. **Integrate with NativeGraphBackend** - The coordinator exists but is not used by the main backend

**Primary recommendation:** Complete the existing skeleton implementation, convert from async to synchronous code, integrate with `NativeGraphBackend`, and define a lock acquisition ordering strategy.

## Existing Implementation Summary

### Files Already Present

| File | Status | Purpose |
|------|--------|---------|
| `transaction_coordinator.rs` | Skeleton implementation | Main coordinator with lock manager, deadlock detector, isolation manager |
| `transaction_state.rs` | Complete | Runtime transaction state (begin, commit, rollback) |
| `manager.rs` | Complete | WAL manager with active transaction tracking |
| `graph_integration.rs` | Complete | Graph-integrated transaction operations |
| `graph_backend.rs` | Partial | Backend with WAL integrator support |

### Key Existing Types

**From `transaction_coordinator.rs`:**

```rust
// Already defined - just needs implementation completion
pub enum IsolationLevel {
    ReadCommitted = 1,
    RepeatableRead = 2,
    Serializable = 3,
}

pub enum ResourceId {
    Node(NativeNodeId),
    Edge(NativeEdgeId),
    Cluster(i64),
    StringTable(u32),
    FreeSpace,
}

pub enum LockType {
    Shared,
    Exclusive,
    IntentionShared,
    IntentionExclusive,
}

pub struct V2LockManager {
    lock_table: Arc<RwLock<HashMap<ResourceId, (LockType, HashSet<TransactionId>)>>>,
    wait_queue: Arc<Mutex<VecDeque<LockRequest>>>,
    lock_timeout: Duration,
}

pub struct DeadlockDetector {
    wait_for_graph: Arc<RwLock<HashMap<TransactionId, HashSet<TransactionId>>>>,
    last_detection: Arc<Mutex<Instant>>,
    detection_interval: Duration,
}

pub struct IsolationManager {
    transaction_isolation: Arc<RwLock<HashMap<TransactionId, IsolationLevel>>>,
    read_timestamps: Arc<RwLock<HashMap<ResourceId, SystemTime>>>,
}
```

### What's Already Working

1. **WAL transaction lifecycle** - `V2WALManager` has complete `begin_transaction`, `commit_transaction`, `rollback_transaction`
2. **Lock table structure** - `V2LockManager` has basic lock acquire/release with Shared/Exclusive semantics
3. **DFS-based cycle detection** - `DeadlockDetector::has_cycle_util` implements standard DFS cycle detection
4. **Two-phase commit structure** - `TwoPhaseCommitCoordinator` has prepare/finalize/abort phases
5. **Isolation level enum** - Three levels defined (ReadCommitted, RepeatableRead, Serializable)

### What's Missing (TODOs noted in code)

1. **Resource-specific deadlock detection** (line 274): `// TODO: Implement resource-specific deadlock detection`
2. **Lock type validation for isolation** (line 367): `// TODO: Implement lock type validation`
3. **Wait-for graph updates on lock conflicts** - Lock manager doesn't update the wait-for graph when transactions must wait
4. **Victim selection** - No `select_victim()` method exists to find youngest transaction in cycle
5. **Lock acquisition ordering** - No global ordering defined to prevent deadlocks (CW-02)
6. **Integration with actual graph operations** - Coordinator exists but isn't called by backend
7. **Async to sync conversion** - All methods are `async fn` but tests disabled due to no tokio runtime

## Lock Management

### Current Locking Patterns

**Existing uses of `parking_lot`:**

| File | Usage | Granularity |
|------|-------|-------------|
| `graph_backend.rs` | `RwLock<GraphFile>` | Entire graph file |
| `manager.rs` | `RwLock<HashMap<u64, ActiveTransaction>>` | Transaction registry |
| `transaction_coordinator.rs` | `RwLock<HashMap<ResourceId, ...>>` | Lock table per resource |
| `cache.rs` | `RwLock<TraversalAwareCache>` | Edge cluster cache |

### Lock Granularity

The existing `ResourceId` enum defines five lock granularities:

1. **Node-level** - Individual node locks
2. **Edge-level** - Individual edge locks
3. **Cluster-level** - Edge cluster locks (multiple edges per node)
4. **StringTable** - String table slot locks
5. **FreeSpace** - Free space block locks

**Gap:** No guidance on when to use which granularity. For Phase 13, we need to define:

- Node operations: Acquire `Node(node_id)` locks
- Edge operations: Acquire both `Node(source_id)` and `Node(target_id)` locks (or cluster locks)
- Cluster operations: Acquire `Cluster(node_id, direction)` locks

### Deadlock Detection Gaps

The `DeadlockDetector` has cycle detection but **doesn't get updated when locks are contested**:

**Current flow (incomplete):**
1. `V2LockManager::acquire_lock()` returns `false` if lock unavailable
2. `V2TransactionCoordinator::acquire_lock()` adds request to wait_queue
3. **Missing:** Update `wait_for_graph` to record that tx is waiting for lock holder

**What needs to be added:**
```rust
// In V2LockManager::acquire_lock(), when lock can't be acquired:
if !acquired {
    // Get current lock holders
    let holders = entry.1.clone();
    // Add to wait-for graph
    for holder in holders {
        deadlock_detector.add_wait_edge(tx_id, holder);
    }
}
```

## Isolation Levels

### Current API Status

**Already defined** in `transaction_coordinator.rs`:

```rust
pub enum IsolationLevel {
    ReadCommitted = 1,
    RepeatableRead = 2,
    Serializable = 3,
}
```

**Also defined** in `manager.rs` (different enum, needs consolidation):

```rust
pub enum TransactionIsolation {
    ReadCommitted,
    Serializable,
    Snapshot,
}
```

**Gap:** Two different enums for isolation levels. The coordinator's version includes `RepeatableRead` while manager's includes `Snapshot`. These need to be unified.

### Isolation Level Implementation Gaps

**ReadCommitted (ACID-17):**
- Already implemented in `IsolationManager::validate_access()`
- Always allows reads (current behavior is correct)

**RepeatableRead (ACID-17):**
- Skeleton exists: records first read timestamp per resource
- **Missing:** Validation that resource hasn't been modified since first read
- **Gap:** No version tracking for resources to detect modifications

**Serializable (ACID-17):**
- Skeleton exists: `// Serializable would require more complex validation`
- **Missing:** Full serializable validation (typically requires conflict detection)
- **Gap:** No implementation of serializability checking

### Isolation Level API

The API already exists but needs to be exposed:

```rust
// Already defined, not exposed to users
pub async fn begin_transaction(&self, isolation_level: IsolationLevel) -> NativeResult<TransactionId>
```

**What's needed:** Add user-facing API methods to `NativeGraphBackend`:

```rust
// Not yet present - needs to be added
impl NativeGraphBackend {
    pub fn begin_transaction(&self, isolation: IsolationLevel) -> NativeResult<TransactionHandle> {
        // Use wal_integrator to begin transaction with isolation level
    }

    pub fn commit_transaction(&self, tx: TransactionHandle) -> NativeResult<()> {
        // Commit through coordinator
    }

    pub fn rollback_transaction(&self, tx: TransactionHandle) -> NativeResult<()> {
        // Rollback through coordinator
    }
}
```

## Deadlock Detection Gaps

### Current Cycle Detection

**Working:** `DeadlockDetector::has_cycle_util()` performs standard DFS cycle detection.

**Not Working:** The wait-for graph is never populated with wait relationships.

**What needs implementation:**

1. **Add wait edges when locks are contested:**
```rust
// When tx A requests lock held by tx B
deadlock_detector.add_wait_edge(tx_a, tx_b);
```

2. **Remove edges when locks are released:**
```rust
// When tx B releases a lock
deadlock_detector.remove_transaction(tx_b); // Already exists
```

3. **Detect and resolve deadlocks:**
```rust
// After adding wait edge, check for cycle
if deadlock_detector.detect_cycle(vec![tx_a]) {
    let victim = deadlock_detector.select_victim(cycle)?;
    coordinator.abort_transaction(victim)?;
}
```

### Victim Selection (ACID-16)

**Missing entirely:** No method to select victim from detected cycle.

**Required behavior:** Select the **youngest** transaction (lowest `start_time`).

**Implementation needed:**
```rust
impl DeadlockDetector {
    pub fn select_victim(&self, cycle: &[TransactionId], contexts: &HashMap<TransactionId, TransactionContext>) -> TransactionId {
        cycle.iter()
            .min_by_key(|&&tx_id| contexts.get(&tx_id).map(|c| c.start_time).unwrap_or(Instant::now()))
            .copied()
            .unwrap()
    }
}
```

## Key Files to Modify

| File | Changes Needed |
|------|----------------|
| `transaction_coordinator.rs` | Complete TODO items, remove async/await, add victim selection |
| `manager.rs` | Unify `TransactionIsolation` with `IsolationLevel` |
| `graph_backend.rs` | Add transaction API methods using coordinator |
| `mod.rs` | Export `IsolationLevel`, `TransactionContext` for public API |
| `graph_integration.rs` | Update to use unified isolation level enum |

## Dependencies

### Within This Phase

```
13-01 (Resource-level lock tracking)
    -> 13-02 (Wait-for graph + cycle detection)
        -> 13-03 (Victim selection + abort)
    -> 13-04 (Lock ordering design document)
```

**Rationale:**
- Lock tracking (13-01) must exist before wait-for graph can be built (13-02)
- Cycle detection (13-02) must exist before victim selection (13-03)
- Lock ordering (13-04) can be done in parallel with implementation (it's design documentation)

### External Dependencies

- **Phase 12 (ACID Consistency)** - Complete per STATE.md
- **parking_lot 0.12** - Already in Cargo.toml, provides `RwLock`, `Mutex`
- **No tokio** - Current async code cannot run; must convert to sync

## Risks and Concerns

### HIGH Confidence Risks

1. **Async/sync mismatch** - Current code is `async fn` but no async runtime exists
   - **Impact:** All tests are `#[ignore]`, code cannot run
   - **Mitigation:** Convert all `async fn` to sync `fn`, remove `.await` calls

2. **Wait-for graph never updated** - Deadlock detection cannot work
   - **Impact:** Deadlocks will not be detected (system will hang)
   - **Mitigation:** Implement `add_wait_edge()` calls in lock acquire path

3. **Two isolation level enums** - Will cause confusion and bugs
   - **Impact:** API inconsistency, potential enum conversion errors
   - **Mitigation:** Consolidate to single `IsolationLevel` enum

### MEDIUM Confidence Risks

1. **No lock acquisition ordering** (CW-02)
   - **Impact:** Deadlocks will be common even with detection
   - **Mitigation:** Define global ordering (e.g., always lock lower node_id first)

2. **RepeatableRead not validated** (ACID-17)
   - **Impact:** RepeatableRead won't prevent phantom reads
   - **Mitigation:** Implement version tracking or snapshot isolation

3. **Coordinator not integrated** with backend
   - **Impact:** All coordination code exists but isn't used
   - **Mitigation:** Wire coordinator into `NativeGraphBackend`

### LOW Confidence Risks

1. **Performance impact** of lock tracking
   - **Impact:** Unknown until measured
   - **Mitigation:** Benchmark before/after, consider lock striping

2. **Serializable isolation complexity**
   - **Impact:** Full serializable may be too complex for this phase
   - **Mitigation:** May defer full serializable to future phase

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cycle detection in directed graph | Custom DFS algorithm | Existing `has_cycle_util()` | Already implemented correctly |
| Lock table with readers/writers | Custom RwLock wrapper | `parking_lot::RwLock` | Already used throughout codebase |
| Transaction ID generation | Atomic counter | `AtomicU64::fetch_add` | Already in `manager.rs` |
| Two-phase commit protocol | Custom state machine | Existing `TwoPhaseCommitCoordinator` | Skeleton already exists |

## Standard Stack

The following is already in use and should be continued:

| Component | Version | Purpose |
|-----------|---------|---------|
| `parking_lot` | 0.12 | Fast mutex/RwLock implementation |
| `std::sync::Arc` | std | Shared ownership for coordinator |
| `std::collections::HashMap` | std | Lock table, transaction registry |
| `std::collections::HashSet` | std | Lock owners, dependencies |
| `std::time::Instant` | std | Transaction timing for victim selection |

**No new dependencies required.**

## Architecture Patterns

### Recommended Project Structure

```
src/backend/native/v2/wal/
├── transaction_coordinator.rs    # Main coordinator (modify)
├── manager.rs                     # WAL manager (minor update)
├── graph_integration.rs           # Graph-integrated ops (modify)
└── isolation.rs                   # NEW: Isolation level implementations
    ├── read_committed.rs
    ├── repeatable_read.rs
    └── serializable.rs
```

### Pattern 1: Lock Acquisition Ordering (CW-02)

**What:** Define a global order for acquiring locks to prevent deadlocks

**When to use:** All multi-resource lock acquisitions

**Example (to be implemented):**
```rust
// Global ordering: always lock resources in ascending order
fn lock_order_key(resource: &ResourceId) -> u64 {
    match resource {
        ResourceId::Node(id) => *id as u64,
        ResourceId::Edge(id) => (*id as u64) + 1_000_000,
        ResourceId::Cluster(id) => (*id as u64) + 2_000_000,
        ResourceId::StringTable(id) => (*id as u64) + 3_000_000,
        ResourceId::FreeSpace => u64::MAX,
    }
}

// When acquiring multiple locks, sort by key first
let mut resources = vec![resource1, resource2];
resources.sort_by_key(|r| lock_order_key(r));
for resource in resources {
    coordinator.acquire_lock(tx_id, resource, LockType::Exclusive)?;
}
```

### Pattern 2: Wait-for Graph Update on Lock Contention

**What:** Update wait-for graph whenever a transaction must wait for a lock

**When to use:** In lock acquisition path when lock is unavailable

**Example (to be implemented):**
```rust
pub fn acquire_lock_sync(&self, tx_id: TransactionId, resource_id: ResourceId, lock_type: LockType) -> NativeResult<bool> {
    let mut lock_table = self.lock_table.write();
    let entry = lock_table.entry(resource_id).or_insert((LockType::Shared, HashSet::new()));

    match lock_type {
        LockType::Exclusive => {
            if !entry.1.is_empty() && !entry.1.contains(&tx_id) {
                // Lock held by others - add to wait-for graph
                for &holder in &entry.1 {
                    self.add_wait_edge(tx_id, holder);
                }

                // Check for deadlock
                if self.detect_deadlock(tx_id)? {
                    return Err(NativeBackendError::DeadlockDetected {
                        tx_id,
                        conflicting_resources: vec![],
                    });
                }

                return Ok(false); // Caller must wait
            }
        }
        // ... other lock types
    }
    // ... acquire lock
}
```

### Anti-Patterns to Avoid

- **Async without runtime:** Don't use `async fn` without tokio runtime
- **Two isolation enums:** Don't define `IsolationLevel` and `TransactionIsolation` separately
- **Updating wait-for graph after detection:** Update before attempting lock
- **Global lock on lock table:** Don't use single mutex for all locks (use per-resource locking)

## Code Examples

### Converting Async to Sync (Required)

**Before (current):**
```rust
pub async fn acquire_lock(&self, tx_id: TransactionId, resource_id: ResourceId, lock_type: LockType) -> NativeResult<bool> {
    let mut lock_table = self.lock_table.write();
    // ...
}
```

**After (required):**
```rust
pub fn acquire_lock(&self, tx_id: TransactionId, resource_id: ResourceId, lock_type: LockType) -> NativeResult<bool> {
    let mut lock_table = self.lock_table.write();
    // ... (same implementation, no .await)
}
```

### Victim Selection (To Be Implemented)

```rust
impl DeadlockDetector {
    /// Select the youngest transaction from a cycle as the victim
    pub fn select_victim(
        &self,
        cycle: &[TransactionId],
        contexts: &HashMap<TransactionId, TransactionContext>,
    ) -> Option<TransactionId> {
        cycle.iter()
            .min_by_key(|&&tx_id| contexts.get(&tx_id).map(|c| c.start_time))
            .copied()
    }
}
```

### Wait-for Edge Addition (To Be Implemented)

```rust
impl DeadlockDetector {
    /// Add a wait edge: tx_waiter is waiting for tx_holder
    pub fn add_wait_edge(&self, tx_waiter: TransactionId, tx_holder: TransactionId) {
        let mut wait_for_graph = self.wait_for_graph.write();
        wait_for_graph
            .entry(tx_waiter)
            .or_insert_with(HashSet::new)
            .insert(tx_holder);
    }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No transaction coordination | `V2TransactionCoordinator` skeleton | Phase 13 (in progress) | Foundation exists, needs completion |
| No deadlock detection | DFS-based cycle detection exists | Phase 13 (in progress) | Detection algorithm ready, not wired up |
| Single isolation level | Three isolation levels defined | Phase 13 (in progress) | API exists, validation incomplete |
| Async methods | Must convert to sync | Phase 13 (required) | Tests currently blocked |

## Open Questions

1. **Lock acquisition ordering:** What is the precise ordering for multi-resource locks?
   - **What we know:** Global ordering prevents deadlocks
   - **What's unclear:** Specific order for (Node, Edge, Cluster, FreeSpace) combinations
   - **Recommendation:** Define in CW-02 design document

2. **RepeatableRead validation:** How to detect if resource was modified?
   - **What we know:** Need to track version numbers or timestamps
   - **What's unclear:** Whether to add version field to records or use WAL LSNs
   - **Recommendation:** Use WAL LSNs as version numbers (already tracked)

3. **Serializable isolation:** Full serializable or simplified variant?
   - **What we know:** True serializable requires conflict serialization
   - **What's unclear:** Whether to implement full protocol or use snapshot isolation
   - **Recommendation:** Start with snapshot isolation (already in `manager.rs`)

## Sources

### Primary (HIGH confidence)
- `/sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs` - Full file read, lines 1-1273
- `/sqlitegraph/src/backend/native/v2/wal/manager.rs` - Full file read, lines 1-1106
- `/sqlitegraph/src/backend/native/transaction_state.rs` - Full file read, lines 1-90
- `/sqlitegraph/src/backend/native/graph_backend.rs` - Full file read, lines 1-408
- `/sqlitegraph/src/backend/native/types/errors.rs` - Full file read, lines 1-207
- `/sqlitegraph/src/backend/native/v2/wal/record.rs` - Full file read, lines 1-892
- `/sqlitegraph/Cargo.toml` - Dependencies verified

### Secondary (MEDIUM confidence)
- `.planning/REQUIREMENTS.md` - Requirements ACID-13 through ACID-18, CW-01 through CW-03
- `.planning/ROADMAP.md` - Phase 13 scope and success criteria

### Tertiary (LOW confidence)
- Existing test files showing isolation and concurrent access patterns (mvcc_concurrent_tests.rs, etc.)

## Metadata

**Confidence breakdown:**
- Existing implementation: HIGH - Full source code read
- Lock management gaps: HIGH - TODOs explicitly noted in code
- Isolation level requirements: MEDIUM - Requirements documented, implementation partial
- Deadlock detection needs: HIGH - Missing components clearly identified
- Async/sync mismatch: HIGH - Tests explicitly disabled due to no tokio

**Research date:** 2026-01-20
**Valid until:** 30 days (stable codebase, implementation in progress)
