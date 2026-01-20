---
phase: 13-acid-isolation
plan: 02
subsystem: Transaction Coordination
tags: [wait-for-graph, deadlock-detection, lock-manager, v2-wal]

# Dependency graph
requires:
  - phase: 13-01
    provides: Synchronous transaction coordinator with unified IsolationLevel enum
provides:
  - Wait-for graph population during lock contention
  - Cycle detection using DeadlockDetector::detect_cycle
  - Automatic deadlock detection returning DeadlockDetected error
affects: [13-03-victim-selection, 13-04-lock-ordering]

# Tech tracking
tech-stack: []
patterns: [wait-for-graph, deadlock-detection-DFS]

key-files:
  created: []
  modified:
    - sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs

key-decisions:
  - "Move deadlock detection from pre-check to post-check: detect_cycle is called after wait edges are added, not before lock acquisition"

patterns-established:
  - "Wait-for graph edges are added synchronously when lock acquisition fails"
  - "Cycle detection runs after wait-for graph population, not before"
  - "Cleanup happens on all transaction exit paths: commit, rollback, cleanup"

# Metrics
duration: 3min
completed: 2026-01-20
---

# Phase 13 Plan 02: Wait-for Graph Population Summary

**Wait-for graph population on lock contention with synchronous deadlock detection using DFS-based cycle detection.**

## Performance

- **Duration:** 3 min
- **Started:** 2026-01-20T10:59:32Z
- **Completed:** 2026-01-20T11:02:42Z
- **Tasks:** 5/5 completed
- **Files modified:** 1

## Accomplishments

- Added `DeadlockDetector::add_wait_edge` method to populate wait-for graph with directed edges (tx_waiter -> tx_holder)
- Added `DeadlockDetector::detect_cycle` method that uses existing `has_cycle_util` for synchronous cycle detection
- Updated `V2LockManager` to hold `DeadlockDetector` reference and call `add_wait_edge` when Exclusive lock acquisition fails
- Changed `V2TransactionCoordinator::acquire_lock` to check `detect_cycle` after failed lock acquisition (not before)
- Fixed `rollback_transaction` to clean up wait-for graph via `remove_transaction` call

## Task Commits

Each task was committed atomically:

1. **Task 1: Add add_wait_edge method to DeadlockDetector** - `5999c50` (feat)
2. **Task 2: Add detect_cycle method to DeadlockDetector** - `db4261e` (feat)
3. **Task 3: Update V2LockManager to populate wait-for graph** - `51a90da` (feat)
4. **Task 4: Update acquire_lock to use detect_cycle** - `e2918ab` (feat)
5. **Task 5: Add wait-for graph cleanup to rollback** - `d0a2590` (feat)

## Files Created/Modified

- `sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs`
  - Added `DeadlockDetector::add_wait_edge` method
  - Added `DeadlockDetector::detect_cycle` method
  - Updated `V2LockManager` struct with `deadlock_detector` field
  - Updated `V2LockManager::new` to accept `Arc<DeadlockDetector>`
  - Updated `V2LockManager::acquire_lock` to call `add_wait_edge` on contention
  - Updated `V2TransactionCoordinator::acquire_lock` to check `detect_cycle` after failed lock
  - Added `deadlock_detector.remove_transaction` call to `rollback_transaction`

## Decisions Made

1. **Post-check deadlock detection instead of pre-check**: The original code had `would_cause_deadlock` called before lock acquisition. This was replaced with `detect_cycle` called after lock acquisition fails because wait edges are only added during the failed lock acquisition. This means we detect actual deadlocks that exist in the wait-for graph, not potential deadlocks before edges are created.

2. **All transaction exit paths must clean up wait-for graph**: Added `remove_transaction` call to `rollback_transaction` which was missing. The commit path already had cleanup via `cleanup_transaction`, but rollback was missing it.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Added missing wait-for graph cleanup to rollback_transaction**

- **Found during:** Task 5 (Verify cleanup paths)
- **Issue:** `rollback_transaction` did not call `deadlock_detector.remove_transaction(tx_id)`, while `cleanup_transaction` (called after commit) did. This is a bug because wait-for graph entries would accumulate and never be cleaned up when transactions are rolled back.
- **Fix:** Added `self.deadlock_detector.remove_transaction(tx_id);` call at the end of `rollback_transaction`, after `unregister_transaction`.
- **Files modified:** `sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs`
- **Verification:** `grep -n "remove_transaction"` shows calls on lines 300 (definition), 667 (rollback), and 809 (cleanup)
- **Committed in:** `d0a2590` (Task 5 commit)

**2. [Rule 3 - Blocking] Fixed test to pass detector to V2LockManager::new**

- **Found during:** Task 3 (Running cargo check)
- **Issue:** Test `test_transaction_coordinator_basic` called `V2LockManager::new()` without arguments, but the signature was changed to require `Arc<DeadlockDetector>`.
- **Fix:** Updated test to create `DeadlockDetector::new()` first, wrap in `Arc::new()`, then pass to `V2LockManager::new()`.
- **Files modified:** `sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs`
- **Verification:** Test compiles and passes
- **Committed in:** `51a90da` (Task 3 commit)

---

**Total deviations:** 2 auto-fixed (1 bug, 1 blocking)
**Impact on plan:** Both fixes necessary for correctness. The missing cleanup in rollback was a real bug that would cause wait-for graph accumulation.

## Issues Encountered

None - all tasks completed as planned.

## Verification

Overall phase verification criteria:

1. [x] `DeadlockDetector::add_wait_edge` adds directed edges to wait_for_graph
2. [x] `DeadlockDetector::detect_cycle` finds cycles using has_cycle_util
3. [x] `V2LockManager` calls add_wait_edge when lock acquisition fails
4. [x] `V2TransactionCoordinator` checks detect_cycle after failed lock acquisition
5. [x] Wait-for graph cleanup happens on transaction completion (commit, rollback, cleanup)
6. [x] `cargo test -p sqlitegraph --lib transaction_coordinator` tests compile and pass (8 passed)

## Success Criteria

All success criteria met:

1. [x] Wait-for graph is populated during lock contention
2. [x] Cycle detection can find cycles in the wait-for graph
3. [x] DeadlockDetected error is returned when cycles are found
4. [x] All changes are synchronous (no async/await)

## Next Phase Readiness

Wait-for graph population is complete. Ready for 13-03 (Victim selection) which will implement logic to select which transaction to abort when a deadlock is detected.

---
*Phase: 13-acid-isolation*
*Plan: 02*
*Completed: 2026-01-20*
