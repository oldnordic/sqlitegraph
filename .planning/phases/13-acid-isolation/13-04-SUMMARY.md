---
phase: 13-acid-isolation
plan: 04
subsystem: concurrency
tags: [lock-management, deadlock-prevention, isolation-levels, transaction-coordinator]

# Dependency graph
requires:
  - phase: 13-acid-isolation
    provides: ResourceId enum, LockType enum, V2LockManager, DeadlockDetector skeleton
provides:
  - Global lock ordering key function for deadlock prevention
  - Operation-specific lock acquisition patterns (Node, Edge, Batch)
  - Isolation level semantics (ReadCommitted, RepeatableRead, Serializable)
  - Deadlock prevention vs detection strategy documentation
affects: [13-05, 13-06, transaction_coordinator implementation]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Global lock ordering for deadlock prevention
    - Multi-resource lock acquisition with sorted keys
    - Intention locks (IS/IX) for hierarchical locking

key-files:
  created:
    - docs/concurrent-write-design.md
  modified: []

key-decisions:
  - "Non-overlapping key ranges for each resource type (Node: 0-4B, Edge: 4B-8B, Cluster: 8B-12B, StringTable: 12B-16B)"
  - "Min-first node locking for edge operations prevents A->B / B->A deadlocks"
  - "Pessimistic concurrency with explicit locks rather than optimistic approach"

patterns-established:
  - "Pattern 1: Always acquire locks in ascending order of lock_order_key()"
  - "Pattern 2: For edge operations, lock min(node_a, node_b) before max(node_a, node_b)"
  - "Pattern 3: Lock releases in any order (only acquisition order matters for deadlock)"

# Metrics
duration: 3min
completed: 2026-01-20
---

# Phase 13: Plan 04 - Concurrent Write Design Summary

**Global lock ordering strategy with non-overlapping key ranges (Node: 0-4B, Edge: 4B-8B, Cluster: 8B-12B, StringTable: 12B-16B) and operation-specific acquisition patterns for deadlock prevention**

## Performance

- **Duration:** 3 min
- **Started:** 2026-01-20T10:38:37Z
- **Completed:** 2026-01-20T10:41:00Z
- **Tasks:** 1
- **Files created:** 1

## Accomplishments

- Created comprehensive concurrent write design document at `docs/concurrent-write-design.md`
- Defined global lock ordering function `lock_order_key()` for deadlock prevention
- Documented operation-specific lock patterns for Node, Edge, and Batch operations
- Explained deadlock prevention vs detection trade-offs
- Specified isolation level semantics (ReadCommitted, RepeatableRead, Serializable)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create concurrent write design document** - `bdff08f` (docs)

**Plan metadata:** (to be added after this commit)

## Files Created/Modified

- `docs/concurrent-write-design.md` - Lock acquisition ordering strategy and operation patterns
  - Global ordering key function with non-overlapping ranges
  - Node operations (Insert, Delete, Get) with lock specifications
  - Edge operations (Insert, Delete, Update) with min-first ordering
  - Multi-edge batch operations with sorted lock acquisition
  - Isolation levels (ReadCommitted, RepeatableRead, Serializable)
  - Deadlock prevention via ordering vs detection via wait-for graph

## Decisions Made

- **Non-overlapping key ranges**: Each resource type has its own 4B range (Node: 0-4B, Edge: 4B-8B, etc.) to prevent any possibility of key collisions between types
- **FreeSpace locks at u64::MAX**: FreeSpace locks are always acquired last since they affect global state and should not block individual resource operations
- **Min-first node locking for edges**: When inserting edges between nodes A and B, always lock min(A,B) first - this prevents the classic A->B / B->A deadlock pattern
- **Pessimistic concurrency**: Uses explicit locks rather than optimistic concurrency control - provides stronger guarantees at the cost of potential lock contention

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- Design document complete, ready for implementation in transaction_coordinator.rs
- Lock ordering function can be implemented directly from the specification
- Operation-specific patterns guide lock acquisition in NativeGraphBackend
- Wait-for graph integration can reference deadlock detection section

**Requirements satisfied:**
- ACID-18: Concurrent write design document defines lock acquisition ordering
- CW-02: Lock acquisition ordering prevents deadlocks

---
*Phase: 13-acid-isolation, Plan: 04*
*Completed: 2026-01-20*
