# Concurrent Write Design for SQLiteGraph Native V2

**Document Version:** 1.0
**Last Updated:** 2026-01-20
**Status:** Design

## Overview

This document describes the concurrency control strategy for the Native V2 backend,
focusing on lock management, deadlock detection, and lock acquisition ordering.

The design implements a pessimistic concurrency control approach using explicit
resource-level locking. This ensures strong consistency guarantees while preventing
deadlocks through a global lock acquisition ordering strategy.

## Architecture

### Components

1. **V2TransactionCoordinator** - Main coordinator for transaction lifecycle
2. **V2LockManager** - Resource-level lock management
3. **DeadlockDetector** - Wait-for graph based deadlock detection
4. **IsolationManager** - Isolation level enforcement

### Resource Types

The following resource types can be locked:

| ResourceId | Description | Granularity |
|------------|-------------|-------------|
| Node(id) | Individual node lock | Fine-grained |
| Edge(id) | Individual edge lock | Fine-grained |
| Cluster(node_id) | All edges for a node | Medium-grained |
| StringTable(id) | String table slot | Fine-grained |
| FreeSpace | Free space blocks | Coarse-grained |

## Lock Acquisition Ordering

### Global Ordering Key

To prevent deadlocks, all multi-resource lock acquisitions follow a global ordering:

```rust
fn lock_order_key(resource: &ResourceId) -> u64 {
    match resource {
        ResourceId::Node(id) => *id as u64,                          // 0..=4,294,967,295
        ResourceId::Edge(id) => (*id as u64) + 4_000_000_000,         // 4B..=8B
        ResourceId::Cluster(id) => (*id as u64) + 8_000_000_000,      // 8B..=12B
        ResourceId::StringTable(id) => (*id as u64) + 12_000_000_000, // 12B..=16B
        ResourceId::FreeSpace => u64::MAX,                            // 18,446,744,073,709,551,615
    }
}
```

This ordering creates non-overlapping ranges for each resource type:

- **Node locks**: 0 - 4,294,967,295 (entire u32 space)
- **Edge locks**: 4,000,000,000 - 8,294,967,295 (offset by 4B)
- **Cluster locks**: 8,000,000,000 - 12,294,967,295 (offset by 8B)
- **StringTable locks**: 12,000,000,000 - 16,294,967,295 (offset by 12B)
- **FreeSpace**: Always last (u64::MAX)

### Ordering Rules

1. **Always acquire locks in ascending order of `lock_order_key()`**
2. **When acquiring multiple locks, sort by key first**
3. **Release locks in any order** (release order doesn't cause deadlock)

### Rationale

The global ordering prevents circular wait conditions, which are a necessary
condition for deadlock. By ensuring all transactions acquire locks in the same
order, cycles in the wait-for graph cannot form naturally.

### Example: Edge Insertion

When inserting an edge from node A to node B:
- Lock order: min(A, B) first, then max(A, B)
- This prevents T1(A->B) and T2(B->A) from deadlocking

```
Transaction 1: Insert edge 5 -> 10
- Acquire Node(5) [key: 5]
- Acquire Node(10) [key: 10]
- Acquire Cluster(5) [key: 8000000005]

Transaction 2: Insert edge 10 -> 5
- Acquire Node(5) [key: 5] - blocked by T1
- Wait for T1 to release Node(5)
```

Without ordering, these transactions could deadlock:
- T1: Node(5), then Node(10)
- T2: Node(10), then Node(5)
- Result: Deadlock!

With ordering, no deadlock occurs:
- T1: Node(5), then Node(10)
- T2: Node(5), then Node(10) - blocked on Node(5)
- T2 waits for T1, T1 completes, T2 proceeds

## Operation Lock Patterns

### Node Operations

#### Insert Node

- **Locks Required:** `Node(node_id)` - Exclusive
- **Ordering:** Single lock, no ordering issues
- **Duration:** Held until commit
- **Example:**
  ```rust
  coordinator.acquire_lock(tx_id, ResourceId::Node(new_id), LockType::Exclusive)?;
  // ... perform node insertion
  // Lock released on commit/rollback
  ```

#### Delete Node

- **Locks Required:**
  1. `Node(node_id)` - Exclusive
  2. `Cluster(node_id)` - Exclusive (for edge cleanup)
- **Ordering:** Node first (lower key), then Cluster
- **Duration:** Both held until commit
- **Example:**
  ```rust
  // Acquire in order: Node before Cluster (4B < 8B+offset)
  coordinator.acquire_lock(tx_id, ResourceId::Node(node_id), LockType::Exclusive)?;
  coordinator.acquire_lock(tx_id, ResourceId::Cluster(node_id), LockType::Exclusive)?;
  // ... perform node deletion and edge cleanup
  // Locks released on commit/rollback
  ```

#### Get Node

- **Locks Required:** `Node(node_id)` - Shared
- **Ordering:** Single lock, no ordering issues
- **Duration:** Held until commit (for RepeatableRead), released immediately (for ReadCommitted)
- **Example:**
  ```rust
  coordinator.acquire_lock(tx_id, ResourceId::Node(node_id), LockType::Shared)?;
  // ... read node data
  ```

### Edge Operations

#### Insert Edge

- **Locks Required:**
  1. `Node(source_id)` - IntentionExclusive (signals intent to modify)
  2. `Node(target_id)` - IntentionExclusive
  3. `Cluster(source_id)` - Exclusive (for outgoing edges)

- **Ordering:**
  1. Lock min(source_id, target_id) first
  2. Lock max(source_id, target_id) second
  3. Lock Cluster(source_id) third

- **Duration:** All held until commit

- **Example:**
  ```rust
  let (first, second) = if source_id < target_id {
      (source_id, target_id)
  } else {
      (target_id, source_id)
  };

  // Acquire node locks in order
  coordinator.acquire_lock(tx_id, ResourceId::Node(first), LockType::IntentionExclusive)?;
  coordinator.acquire_lock(tx_id, ResourceId::Node(second), LockType::IntentionExclusive)?;
  coordinator.acquire_lock(tx_id, ResourceId::Cluster(source_id), LockType::Exclusive)?;

  // ... perform edge insertion
  ```

#### Delete Edge

- **Locks Required:** Same as Insert Edge
- **Ordering:** Same as Insert Edge
- **Duration:** All held until commit

#### Update Edge

- **Locks Required:** Same as Insert Edge
- **Ordering:** Same as Insert Edge
- **Duration:** All held until commit

### Multi-Edge Operations

#### Batch Edge Insert

- **Locks Required:** All affected nodes and clusters
- **Ordering:** Sort all ResourceIds by lock_order_key(), acquire in order
- **Duration:** All held until commit
- **Example:**
  ```rust
  let mut resources: Vec<ResourceId> = vec![
      ResourceId::Node(source1),
      ResourceId::Node(target1),
      ResourceId::Node(source2),
      ResourceId::Node(target2),
      ResourceId::Cluster(source1),
      ResourceId::Cluster(source2),
  ];

  // Sort by lock order key
  resources.sort_by_key(|r| lock_order_key(r));

  // Acquire all locks
  for resource in resources {
      coordinator.acquire_lock(tx_id, resource, LockType::IntentionExclusive)?;
  }
  ```

## Deadlock Prevention vs Detection

### Prevention (via Ordering)

- **Global lock ordering** prevents deadlocks for most operations
- **Edge operations** lock nodes in a consistent order (min first)
- **Batch operations** sort resources before acquiring locks

**Limitations:**
- Cannot prevent deadlocks from external factors (e.g., network timeouts)
- Cannot prevent deadlocks from operations with non-deterministic resource sets
- Detection still required as a safety net

### Detection (via Wait-for Graph)

- **Still required** for complex operations with non-deterministic ordering
- **Victim selection** chooses youngest transaction
- **Abort and retry** allows recovery from unavoidable deadlocks

**Wait-for Graph Update:**
- When a lock cannot be acquired, add edge: requester -> holder
- When locks are released, remove transaction from wait-for graph
- Detection runs after each failed lock acquisition

**Victim Selection:**
```rust
fn select_victim(cycle: &[TransactionId], contexts: &HashMap<TransactionId, TransactionContext>) -> TransactionId {
    // Select youngest transaction (most recent start_time)
    cycle.iter()
        .min_by_key(|&&tx_id| contexts.get(&tx_id).map(|c| c.start_time))
        .copied()
        .unwrap()
}
```

## Isolation Levels

### ReadCommitted

- **Read locks:** Shared, released immediately after read
- **Write locks:** Exclusive, held until commit
- **Phantoms:** Possible (new rows may appear in subsequent reads)

**Use case:** High throughput, acceptable phantom reads

### RepeatableRead

- **Read locks:** Shared, held until commit
- **Write locks:** Exclusive, held until commit
- **Phantoms:** Possible (not fully prevented without additional mechanisms)

**Use case:** Consistent snapshot within transaction, higher lock contention

### Serializable

- **Read locks:** Shared, held until commit
- **Write locks:** Exclusive, held until commit
- **Phantoms:** Prevented (requires additional conflict checking)

**Use case:** Strongest consistency, highest contention, lowest throughput

**Implementation Note:** Full serializable isolation requires predicate locking
or conflict serialization beyond standard lock management. The current design
provides the foundation; full serializable validation may be added in a future phase.

## Implementation Notes

### Lock Type Compatibility

| Request \ Held | Shared | Exclusive | IS | IX |
|----------------|--------|-----------|-----|-----|
| Shared | ✓ | ✗ | ✓ | ✗ |
| Exclusive | ✗ | ✗ | ✗ | ✗ |
| IS (Intention Shared) | ✓ | ✗ | ✓ | ✓ |
| IX (Intention Exclusive) | ✗ | ✗ | ✓ | ✓ |

**Explanation:**
- **Shared** locks can coexist with other Shared and IS locks
- **Exclusive** locks require exclusive access
- **IS** indicates intent to read at finer granularity
- **IX** indicates intent to write at finer granularity

### Wait-for Graph Update

- When a lock cannot be acquired, add edge: requester -> holder
- When locks are released, remove transaction from wait-for graph
- Detection runs after each failed lock acquisition

### Lock Timeout

- Default timeout: 30 seconds
- Configurable per transaction coordinator instance
- Abort transaction after timeout expires

## Future Enhancements

1. **Lock Escalation** - Upgrade multiple fine-grained locks to coarse-grained
   - Reduces lock table overhead for large batch operations
   - Trade-off: Increased contention

2. **Lock Timeout** - Abort transactions that wait too long
   - Prevents indefinite blocking
   - Configurable per transaction or globally

3. **Priority Transactions** - Prefer high-priority transactions in victim selection
   - Business-critical transactions avoid being chosen as victims
   - Age-based or explicit priority levels

4. **Optimistic Concurrency** - For read-heavy workloads
   - No locks for reads
   - Validate writes at commit time
   - Retry on conflict

## References

- **Research:** `.planning/phases/13-acid-isolation/13-RESEARCH.md`
- **Implementation:** `src/backend/native/v2/wal/transaction_coordinator.rs`
- **Requirements:** ACID-13, ACID-14, ACID-15, ACID-16, ACID-17, ACID-18, CW-01, CW-02, CW-03

## Requirements Satisfied

This design document addresses the following requirements:

- **ACID-13**: Transaction coordinator implements resource-level lock tracking
- **ACID-14**: Transaction coordinator builds wait-for graph for deadlock detection
- **ACID-15**: Transaction coordinator detects cycles in wait-for graph
- **ACID-16**: Transaction coordinator selects victim for abort (youngest transaction)
- **ACID-17**: Transaction isolation level API exists (ReadCommitted, RepeatableRead, Serializable)
- **ACID-18**: Concurrent write design document defines lock acquisition ordering
- **CW-01**: Lock manager tracks resource ownership per transaction
- **CW-02**: Lock acquisition ordering prevents deadlocks
- **CW-03**: Deadlock detection and resolution strategy defined
