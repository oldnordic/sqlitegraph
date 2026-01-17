# MVCC Gap Analysis

**Date:** 2026-01-17
**Phase:** 04 - MVCC Completion
**Plan:** 04-01 - MVCC Gap Analysis and Baseline
**Status:** Initial Analysis

---

## Executive Summary

The current MVCC-lite implementation provides a foundation for read-only snapshots using ArcSwap for atomic state management and cloned HashMaps for isolation. However, **critical gaps exist in concurrent access testing, integration with public APIs, and lifecycle management**. This analysis identifies 12 gaps across severity levels and provides recommendations for each.

**Key Finding:** The implementation has NO true concurrent access tests - all existing tests are single-threaded, making the concurrency safety claims unproven.

---

## 1. Current Implementation Review

### 1.1 Architecture Overview

**Location:** `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/mvcc.rs`

The MVCC-lite system uses the **S3 Hybrid Design**:

- **SnapshotState**: Immutable struct storing cloned HashMaps (outgoing/incoming adjacency)
- **ArcSwap**: Lock-free atomic updates for snapshot manager
- **GraphSnapshot**: Read-only wrapper with separate SQLite connection
- **SnapshotManager**: Coordinates state updates and snapshot acquisition

```rust
// Key components from mvcc.rs
pub struct SnapshotState {
    pub outgoing: HashMap<NodeId, Vec<NodeId>>,
    pub incoming: HashMap<NodeId, Vec<NodeId>>,
    pub created_at: std::time::SystemTime,
}

pub struct SnapshotManager {
    current: ArcSwap<SnapshotState>,
}

pub struct GraphSnapshot {
    state: Arc<SnapshotState>,
    conn: Connection,  // Read-only SQLite connection
}
```

### 1.2 Integration Points

**SqliteGraph Integration** (`sqlitegraph/src/graph/snapshot.rs`):

```rust
impl SqliteGraph {
    pub fn acquire_snapshot(&self) -> Result<GraphSnapshot, SqliteGraphError> {
        self.update_snapshot();  // Syncs cache state
        let snapshot_state = self.snapshot_manager.acquire_snapshot();
        GraphSnapshot::new(snapshot_state, ":memory:")  // Uses in-memory DB
    }
}
```

**Current State:**
- ✅ Basic snapshot acquisition implemented
- ✅ ArcSwap provides atomic state loading
- ✅ Cloned HashMaps provide isolation
- ❌ **No public API integration for `SnapshotManager`**
- ❌ **No integration with Native V2 backend**
- ❌ **No coordination with WAL operations**

### 1.3 Isolation Guarantees

**What Works:**
- SnapshotState stores cloned HashMaps (true copy, not Arc-shared)
- ArcSwap::load() provides atomic pointer acquisition
- Read-only SQLite connection prevents database mutations

**What's Missing:**
- No verification that snapshots remain unchanged under concurrent writes
- No testing of snapshot acquisition during state updates
- No lifecycle management for snapshot cleanup

---

## 2. Identified Gaps

### Gap 1: No Concurrent Access Tests
**Severity:** CRITICAL
**Impact:** Concurrency safety is entirely unproven. The system claims MVCC-lite semantics but has never been tested under concurrent load.

**Evidence:**
- All tests in `mvcc_snapshot_tests.rs` are single-threaded
- No `rayon` or `std::thread` usage in test suite
- No stress tests for simultaneous snapshot acquisition

**Recommended Fix:**
Implement multi-threaded test suite (Plan 02):
- 100+ threads acquiring snapshots simultaneously
- Concurrent reads + writes with snapshot validation
- Race condition detection using loom or similar

**Test Scenarios Needed:**
- Thread A acquires snapshot while Thread B updates state
- 10 threads reading snapshots while 1 thread writing
- Snapshot acquisition during WAL checkpoint

---

### Gap 2: SnapshotManager Not Exposed in Public API
**Severity:** HIGH
**Impact:** Users cannot create snapshots directly, reducing utility of MVCC system.

**Current State:**
```rust
// In lib.rs - only GraphSnapshot is exported
pub use mvcc::{GraphSnapshot, SnapshotState};
// SnapshotManager is NOT exported
```

**Recommended Fix:**
Export SnapshotManager and integrate with SqliteGraph public API:
```rust
// Proposed addition to lib.rs
pub use mvcc::{GraphSnapshot, SnapshotState, SnapshotManager};

// Add to SqliteGraph
impl SqliteGraph {
    pub fn snapshot_manager(&self) -> &SnapshotManager {
        &self.snapshot_manager
    }
}
```

---

### Gap 3: No Native V2 Backend Integration
**Severity:** HIGH
**Impact:** MVCC system only works with SQLite backend, not Native V2 (the high-performance backend).

**Current State:**
- Native V2 has separate snapshot system (`backend/native/v2/snapshot/`)
- No coordination between MVCC snapshots and V2 snapshots
- V2 snapshots bypass MVCC entirely

**Recommended Fix:**
Integrate SnapshotManager with Native V2 backend:
- Cache adjacency maps in SnapshotState for V2
- Use V2's read-only graph file access
- Coordinate snapshot acquisition with V2 WAL operations

---

### Gap 4: Undefined Behavior During WAL Recovery
**Severity:** HIGH
**Impact:** Snapshot acquired during WAL recovery may see inconsistent state.

**Current Behavior:**
```rust
pub fn acquire_snapshot(&self) -> Result<GraphSnapshot, SqliteGraphError> {
    self.update_snapshot();  // What if WAL is being recovered?
    let snapshot_state = self.snapshot_manager.acquire_snapshot();
    // ...
}
```

**Recommended Fix:**
Add WAL recovery coordination:
- Block snapshot acquisition during WAL recovery
- Or: Document that snapshots during recovery are undefined
- Add `is_recovering()` flag to block acquisitions

---

### Gap 5: Undefined Behavior During Checkpoint
**Severity:** MEDIUM
**Impact:** Snapshot during checkpoint may see partial state.

**Current Behavior:**
```rust
// Checkpoint flushes WAL to main DB
fn checkpoint(&self) -> Result<(), SqliteGraphError> {
    // No coordination with snapshot acquisition
}
```

**Recommended Fix:**
Coordinate checkpoint with snapshots:
- Option 1: Block new snapshots during checkpoint
- Option 2: Acquire snapshot before checkpoint, use consistent view
- Document checkpoint/snapshot interaction

---

### Gap 6: No Memory Pressure Handling
**Severity:** MEDIUM
**Impact:** Many large snapshots can cause OOM under memory pressure.

**Current Behavior:**
```rust
// Each snapshot clones entire HashMap
outgoing: outgoing.clone(),  // Full copy, no limits
incoming: incoming.clone(),
```

**Recommended Fix:**
Add memory management:
- Track total snapshot memory usage
- Limit max snapshots per manager
- Implement LRU eviction for old snapshots
- Add configuration for memory limits

---

### Gap 7: No Snapshot Lifecycle Management
**Severity:** MEDIUM
**Impact:** Snapshots are never explicitly cleaned up, relying on Rust's RAII only.

**Current Behavior:**
```rust
// No explicit cleanup, just Drop impl
impl Drop for GraphSnapshot {
    // Implicit: closes SQLite connection
}
```

**Missing Features:**
- No explicit snapshot release API
- No tracking of active snapshots
- No metrics on snapshot lifetime
- No way to force cleanup

**Recommended Fix:**
Add lifecycle management:
- Track active snapshot count
- Add `release_snapshot()` method
- Provide metrics (creation time, lifetime)
- Consider weak reference tracking

---

### Gap 8: RwLock Contention in HNSW Indexes
**Severity:** MEDIUM
**Impact:** Snapshot reads may contend with HNSW index writes.

**Current State:**
- HNSW indexes use RwLock for thread safety
- Snapshot reads through HNSW may acquire read locks
- Write operations blocked by active snapshot reads

**Recommended Fix:**
Analyze lock contention patterns:
- Profile HNSW lock acquisition during snapshot operations
- Consider lock-free read structures for HNSW
- Document HNSW interaction with snapshots

---

### Gap 9: No Cache Coherency Testing
**Severity:** LOW
**Impact:** Unknown if snapshot caches remain coherent under concurrent modifications.

**Current State:**
- Tests verify snapshot isolation from writes
- No tests for cache behavior under concurrent access
- Cache invalidation not tested with multiple snapshots

**Recommended Fix:**
Add cache coherency tests:
- Verify snapshot caches independent from main graph
- Test cache invalidation doesn't affect snapshots
- Measure cache hit rates under concurrent access

---

### Gap 10: Empty Graph Snapshot Edge Cases
**Severity:** LOW
**Impact:** Minor - empty graph snapshots work but not thoroughly tested.

**Current State:**
- `test_empty_graph_snapshot()` exists but commented out
- No stress tests for empty snapshots

**Recommended Fix:**
Complete empty graph testing:
- Snapshot of completely empty graph
- Snapshot after all entities deleted
- Rapid snapshot creation on empty graph

---

### Gap 11: Large Graph Snapshot Performance
**Severity:** LOW
**Impact:** Large graphs (10K+ nodes) may have slow snapshot creation.

**Current Behavior:**
```rust
outgoing: outgoing.clone(),  // O(N) clone time
incoming: incoming.clone(),
```

**Recommended Fix:**
Add performance optimization:
- Measure baseline for 10K, 100K node graphs
- Consider copy-on-write structures for large maps
- Add lazy cloning for snapshot data
- Document performance characteristics

---

### Gap 12: No Snapshot Ordering Guarantees
**Severity:** LOW
**Impact:** No way to compare snapshots or determine temporal ordering.

**Current State:**
```rust
pub struct SnapshotState {
    pub created_at: std::time::SystemTime,  // Only timestamp
}
```

**Missing Features:**
- No snapshot sequence number
- No way to determine if snapshot A is newer than B
- No monotonic ordering guarantees

**Recommended Fix:**
Add ordering support:
- Add sequence counter to SnapshotManager
- Implement `PartialOrd` for snapshots
- Document ordering guarantees

---

## 3. Concurrency Issues

### Issue 1: Unproven Atomic Updates
**Severity:** CRITICAL

ArcSwap provides atomic pointer swaps, but we haven't verified:
- What happens if snapshot acquisition races with state update?
- Are there any ABA problems?
- Does the cloned HashMap provide sufficient isolation?

**Test Needed:**
```rust
#[test]
fn test_snapshot_during_state_update() {
    // Thread A: Continuously update snapshot state
    // Thread B: Continuously acquire snapshots
    // Verify: No crashes, no data races, snapshots are consistent
}
```

### Issue 2: RwLock Contention in Backend
**Severity:** MEDIUM

When snapshots access the graph through backend operations, they may acquire RwLocks:
- HNSW index lookups acquire read locks
- Cache operations may have internal locks
- Backend writes blocked by snapshot reads

**Test Needed:**
```rust
#[test]
fn test_snapshot_read_write_contention() {
    // 1 thread: Continuous writes
    // 10 threads: Continuous snapshot reads
    // Measure: Lock contention, throughput
}
```

### Issue 3: SQLite Connection Pool Exhaustion
**Severity:** LOW

Each snapshot creates a new read-only SQLite connection:
```rust
let conn = Connection::open_with_flags(
    db_path,
    OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
)?;
```

**Risk:** 1000 snapshots = 1000 connections

**Fix Needed:**
- Use connection pooling
- Or: Share one connection across snapshots (if thread-safe)
- Document connection limits

---

## 4. Edge Cases

### Edge Case 1: Snapshot During Transaction Rollback
**Scenario:** Snapshot acquired during uncommitted transaction

**Current Behavior:** Undefined
**Expected:** Snapshot should not see uncommitted data
**Test:** `test_snapshot_during_rollback()`

### Edge Case 2: Rapid Snapshot Creation/Expiration
**Scenario:** Create and drop 10K snapshots rapidly

**Current Behavior:** May cause memory pressure
**Expected:** System should handle gracefully with GC
**Test:** `test_rapid_snapshot_lifecycle()`

### Edge Case 3: Snapshot with Deleted Entities
**Scenario:** Snapshot contains entity, then entity deleted

**Current Behavior:** Undefined
**Expected:** Snapshot should retain entity
**Test:** `test_snapshot_with_deleted_entities()` (exists but commented)

### Edge Case 4: Large Graph Under Memory Pressure
**Scenario:** 100K node graph, 100 snapshots, memory constrained

**Current Behavior:** May OOM
**Expected:** Graceful degradation or error
**Test:** `test_large_graph_memory_pressure()`

### Edge Case 5: Snapshot Acquisition During Checkpoint
**Scenario:** WAL checkpoint in progress, snapshot acquired

**Current Behavior:** Undefined
**Expected:** Consistent snapshot or blocked acquisition
**Test:** `test_snapshot_during_checkpoint()`

---

## 5. Missing Integration

### Integration 1: SqliteGraph Public API
**Status:** PARTIAL

**What's Exposed:**
- `GraphSnapshot` type
- `SqliteGraph::acquire_snapshot()`

**What's Missing:**
- `SnapshotManager` type
- Direct snapshot state access
- Snapshot lifecycle callbacks

**Recommendation:**
Full public API for snapshot management:
```rust
impl SqliteGraph {
    pub fn snapshot_manager(&self) -> &SnapshotManager;
    pub fn active_snapshot_count(&self) -> usize;
    pub fn set_snapshot_callback(&self, cb: SnapshotCallback);
}
```

### Integration 2: Native V2 Backend
**Status:** NONE

**Current Situation:**
- Native V2 has its own snapshot format (file-based)
- No coordination between V2 snapshots and MVCC snapshots
- V2 operations bypass SnapshotManager entirely

**Recommendation:**
Create unified snapshot layer:
- Abstract `Snapshot` trait
- V2 and SQLite both implement trait
- Common lifecycle management

### Integration 3: WAL Operations
**Status:** NONE

**Current Situation:**
- WAL checkpoint doesn't coordinate with snapshots
- WAL recovery doesn't snapshot state
- No WAL-based snapshot isolation

**Recommendation:**
Add WAL-MVCC coordination:
- Snapshot acquisition queries WAL position
- Checkpoint blocks new snapshots
- Recovery snapshot captures pre-recovery state

---

## 6. Test Scenarios Needed

### Scenario 1: Concurrent Snapshot Acquisition
**Thread Count:** 100
**Duration:** 10 seconds
**Validation:** No crashes, all snapshots consistent

### Scenario 2: Read-Write Stress Test
**Threads:** 1 writer, 10 readers (snapshots)
**Duration:** 30 seconds
**Validation:** No data races, snapshots isolated

### Scenario 3: Snapshot During State Update
**Threads:** 1 updater, 1 snapshotter
**Iterations:** 10,000
**Validation:** Atomic update, no torn snapshots

### Scenario 4: Memory Pressure Test
**Setup:** Large graph (10K nodes), 100 snapshots
**Validation:** Memory usage monitored, no OOM

### Scenario 5: Cache Coherency
**Setup:** 10 snapshots, concurrent writes
**Validation:** Snapshot caches independent, no cross-contamination

---

## 7. Recommendations by Priority

### Immediate (Critical/High Severity)
1. **Implement concurrent access tests** - Verify MVCC-lite claims
2. **Expose SnapshotManager in public API** - Enable user snapshot control
3. **Define WAL/snapshot interaction** - Prevent undefined behavior

### Short-term (Medium Severity)
4. **Add Native V2 integration** - Unify snapshot systems
5. **Implement memory pressure handling** - Prevent OOM
6. **Add snapshot lifecycle management** - Explicit cleanup, metrics

### Long-term (Low Severity)
7. **Optimize large graph snapshots** - Copy-on-write, lazy cloning
8. **Add snapshot ordering** - Sequence numbers, comparison
9. **Improve empty graph edge cases** - Better testing
10. **Document cache coherency** - Clear guarantees

---

## 8. Implementation Roadmap

### Plan 04-02: Concurrent Tests
- Implement multi-threaded test suite
- Add race condition detection
- Stress test with 100+ threads
- Verify atomic guarantees

### Plan 04-03: Integration & Lifecycle
- Expose SnapshotManager in public API
- Add Native V2 integration
- Implement lifecycle management
- Add memory pressure handling

### Plan 04-04: Performance & Optimization
- Benchmark large graph snapshots
- Implement copy-on-write optimizations
- Add connection pooling
- Document performance characteristics

---

## 9. Risk Assessment

### High Risk Areas
1. **Concurrent access unproven** - Could have data races
2. **WAL interaction undefined** - Could cause corruption
3. **Memory pressure unhandled** - Could cause OOM

### Medium Risk Areas
4. **Cache coherency untested** - Could have subtle bugs
5. **Lifecycle management missing** - Could leak resources
6. **Native V2 not integrated** - Fragmented user experience

### Low Risk Areas
7. **Performance not optimized** - Works but slow
8. **Edge cases incomplete** - Minor bugs possible
9. **Public API limited** - Reduced utility

---

## 10. Success Criteria

Phase 04 complete when:
- [ ] All 12 gaps addressed or documented
- [ ] Concurrent access test suite passing
- [ ] SnapshotManager exposed in public API
- [ ] Native V2 integration designed
- [ ] Memory pressure handling implemented
- [ ] WAL/snapshot interaction documented
- [ ] Performance baselines established
- [ ] All edge cases tested

---

## Appendix A: Files Analyzed

1. `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/mvcc.rs` - Core MVCC implementation
2. `/home/feanor/Projects/sqlitegraph/sqlitegraph/tests/mvcc_snapshot_tests.rs` - Single-threaded tests
3. `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/graph/snapshot.rs` - SqliteGraph integration
4. `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/lib.rs` - Public API exports
5. `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend.rs` - Backend trait definitions
6. `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/snapshot/mod.rs` - V2 snapshot system

---

## Appendix B: Severity Definitions

- **CRITICAL:** System may be unsafe or produce incorrect results under concurrent access
- **HIGH:** Major feature missing, significantly reduces utility
- **MEDIUM:** Important feature missing, may cause issues in production
- **LOW:** Minor issue or optimization opportunity

---

**End of MVCC Gap Analysis**
**Next Step:** Execute Plan 04-02 (Concurrent Test Implementation)
