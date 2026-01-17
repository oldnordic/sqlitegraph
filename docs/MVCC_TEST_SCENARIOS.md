# MVCC Test Scenarios for Concurrent Operations

**Date:** 2026-01-17
**Phase:** 04 - MVCC Completion
**Plan:** 04-01 - MVCC Gap Analysis and Baseline
**Purpose:** Specification for concurrent test implementation (Plan 04-02)

---

## Executive Summary

This document provides comprehensive test scenarios for validating MVCC-lite behavior under concurrent access. Each scenario includes objectives, setup requirements, success criteria, expected behavior, and failure detection strategies.

**Total Scenarios:** 24
**Categories:** Race Conditions (6), Stress Tests (6), Correctness (6), Performance (6)

---

## 1. Race Condition Scenarios

### Scenario 1.1: Snapshot Acquisition During State Update
**Objective:** Verify atomic snapshot acquisition during concurrent state updates

**Setup:**
- Thread A: Continuously update snapshot state (1000 iterations)
- Thread B: Continuously acquire snapshots (1000 iterations)
- Baseline graph: 100 nodes, 200 edges

**Procedure:**
```rust
// Thread A
for i in 0..1000 {
    manager.update_snapshot(&new_outgoing, &new_incoming);
}

// Thread B (concurrent)
for _ in 0..1000 {
    let snapshot = manager.acquire_snapshot();
    // Validate snapshot consistency
}
```

**Success Criteria:**
- No crashes or panics
- All snapshots are internally consistent
- No torn data (node/edge counts match)
- ArcSwap atomic operations complete without data races

**Expected Behavior:**
- Each snapshot sees a consistent state (either before or after update)
- No snapshot sees partially updated state
- ArcSwap ensures atomic pointer swaps

**Failure Detection:**
- Panic indicates data race
- Inconsistent snapshot (node_count != edges) indicates torn read
- Use `loom` for systematic concurrency testing

**Test File:** `mvcc_concurrent_tests.rs::test_snapshot_during_state_update`

---

### Scenario 1.2: Read During Write (Multiple Readers, One Writer)
**Objective:** Verify snapshot isolation under concurrent read/write workload

**Setup:**
- Thread W: Single writer performing continuous graph mutations
- Threads R1-R10: Ten readers acquiring snapshots simultaneously
- Duration: 10 seconds
- Graph: 1000 nodes, 5000 edges

**Procedure:**
```rust
// Writer thread
loop {
    insert_entity(&graph, new_entity)?;
    insert_edge(&graph, new_edge)?;
    sleep(Duration::from_millis(10));
}

// Reader threads (x10)
loop {
    let snapshot = graph.acquire_snapshot()?;
    verify_snapshot_consistency(&snapshot)?;
    sleep(Duration::from_millis(50));
}
```

**Success Criteria:**
- All readers see consistent snapshots
- No reader sees partial writes
- Snapshot isolation maintained
- No deadlocks or livelocks

**Expected Behavior:**
- Readers acquire snapshots without blocking writer
- Writer progresses without blocking readers
- Snapshots isolated from concurrent writes

**Failure Detection:**
- Deadlock: test hangs (use timeout)
- Data race: use `RUSTFLAGS="-Z sanitizer=thread"`
- Inconsistent snapshot: validation fails

**Test File:** `mvcc_concurrent_tests.rs::test_read_during_write_stress`

---

### Scenario 1.3: Snapshot Acquisition During WAL Checkpoint
**Objective:** Define and verify behavior during checkpoint operations

**Setup:**
- Thread A: Trigger WAL checkpoints every 100ms
- Thread B: Acquire snapshots continuously
- Graph: Large (10K nodes) to ensure checkpoint takes time
- Backend: SQLite with WAL mode

**Procedure:**
```rust
// Thread A
loop {
    graph.checkpoint()?;
    sleep(Duration::from_millis(100));
}

// Thread B
loop {
    let snapshot = graph.acquire_snapshot()?;
    verify_snapshot(&snapshot)?;
}
```

**Success Criteria:**
- No crashes during checkpoint
- Snapshots either see pre-checkpoint or post-checkpoint state
- No undefined behavior or corruption

**Expected Behavior:**
- **Current:** Undefined (documented gap)
- **Desired:** Block snapshot acquisition during checkpoint
- **Alternative:** Allow acquisition but document as implementation-defined

**Failure Detection:**
- Panic or corruption indicates gap in checkpoint coordination
- SQLite errors indicate locking issues

**Test File:** `mvcc_concurrent_tests.rs::test_snapshot_during_checkpoint`

**Gap Analysis:** See Gap 5 in MVCC_GAP_ANALYSIS.md

---

### Scenario 1.4: Multiple Simultaneous Snapshot Creations
**Objective:** Stress test snapshot creation under high concurrency

**Setup:**
- 100 threads all acquiring snapshots simultaneously
- Barrier synchronization to ensure simultaneous start
- Graph: 500 nodes, 2000 edges

**Procedure:**
```rust
let barrier = Arc::new(Barrier::new(100));

let handles: Vec<_> = (0..100).map(|_| {
    let barrier = barrier.clone();
    thread::spawn(move || {
        barrier.wait();
        let snapshot = graph.acquire_snapshot()?;
        validate_snapshot(&snapshot)
    })
}).collect();

// Wait for all threads and check results
```

**Success Criteria:**
- All 100 threads successfully acquire snapshots
- No deadlocks or excessive contention
- All snapshots valid and consistent
- Total time < 5 seconds

**Expected Behavior:**
- ArcSwap allows lock-free concurrent reads
- All threads see consistent state
- No thundering herd problem

**Failure Detection:**
- Timeout indicates deadlock
- High contention latency indicates performance issue
- Use flamegraph to identify bottlenecks

**Test File:** `mvcc_concurrent_tests.rs::test_100_simultaneous_snapshots`

---

### Scenario 1.5: Snapshot During Transaction Rollback
**Objective:** Verify snapshot behavior when transaction rolled back

**Setup:**
- Thread A: Start transaction, modify graph, rollback
- Thread B: Acquire snapshot during transaction
- Graph: 100 nodes

**Procedure:**
```rust
// Thread A
let tx = graph.begin_transaction()?;
insert_entity(&tx, entity)?;
// Thread B acquires snapshot here
tx.rollback()?;

// Thread B
let snapshot = graph.acquire_snapshot()?;
// Should NOT see uncommitted entity
```

**Success Criteria:**
- Snapshot does not see uncommitted data
- Snapshot sees pre-transaction state
- No corruption or inconsistency

**Expected Behavior:**
- **Current:** Undefined (transaction isolation not tested)
- **Desired:** Snapshot sees only committed data
- SQLite connection should provide this isolation

**Failure Detection:**
- Snapshot sees uncommitted data = isolation violation
- Use transaction-aware validation

**Test File:** `mvcc_concurrent_tests.rs::test_snapshot_during_rollback`

**Gap Analysis:** See Edge Case 1 in MVCC_GAP_ANALYSIS.md

---

### Scenario 1.6: Cache Update During Snapshot Access
**Objective:** Verify snapshot isolation when cache invalidated

**Setup:**
- Thread A: Perform writes that invalidate cache
- Thread B: Access snapshot data (cached adjacency)
- Graph: 200 nodes, 1000 edges

**Procedure:**
```rust
// Thread A
loop {
    insert_entity(&graph, new_entity)?;
    insert_edge(&graph, new_edge)?;
    // Cache invalidated here
}

// Thread B
let snapshot = graph.acquire_snapshot()?;
let neighbors = snapshot.get_outgoing(node_id)?;
// Verify neighbors unchanged even after cache invalidation
```

**Success Criteria:**
- Snapshot neighbors unchanged despite cache invalidation
- Snapshot uses cloned HashMap, not shared cache
- No cross-contamination

**Expected Behavior:**
- Snapshot has independent copy of adjacency data
- Cache invalidation does not affect snapshot
- Full isolation between snapshot and live cache

**Failure Detection:**
- Snapshot neighbors change = isolation violation
- Use sanitizers to detect data races

**Test File:** `mvcc_concurrent_tests.rs::test_cache_invalidation_during_snapshot`

---

## 2. Stress Test Scenarios

### Scenario 2.1: 100 Threads Snapshot Acquisition
**Objective:** Maximum concurrency stress test

**Setup:**
- 100 threads acquiring snapshots in tight loop
- Duration: 30 seconds
- Graph: 1000 nodes, 5000 edges
- Measure: Throughput, latency, contention

**Procedure:**
```rust
let handles: Vec<_> = (0..100).map(|i| {
    thread::spawn(move || {
        let start = Instant::now();
        let mut count = 0;
        while start.elapsed() < Duration::from_secs(30) {
            let _snapshot = graph.acquire_snapshot()?;
            count += 1;
        }
        (i, count)
    })
}).collect();
```

**Success Criteria:**
- Total snapshots > 10,000
- Average latency < 10ms per snapshot
- P99 latency < 100ms
- No crashes or hangs

**Expected Behavior:**
- ArcSwap scales linearly with threads
- Lock-free reads minimize contention
- Throughput increases with thread count (to a point)

**Failure Detection:**
- Throughput plateaus or decreases = contention
- High P99 latency = bottleneck
- Use perf/flamegraph for analysis

**Test File:** `mvcc_stress_tests.rs::test_100_threads_30_seconds`

---

### Scenario 2.2: Rapid Snapshot Creation/Destruction Cycles
**Objective:** Stress test lifecycle management

**Setup:**
- Single thread creating/dropping snapshots rapidly
- 10,000 iterations
- Graph: 100 nodes
- Measure: Memory usage, GC pressure

**Procedure:**
```rust
for i in 0..10_000 {
    let snapshot = graph.acquire_snapshot()?;
    assert!(snapshot.node_count() > 0);
    // Snapshot dropped here
}
// Verify no memory leaks
```

**Success Criteria:**
- All 10,000 iterations complete
- No memory leaks (verify with valgrind)
- No file descriptor leaks
- Stable memory usage

**Expected Behavior:**
- Arc reference counting properly cleans up
- SQLite connections closed on drop
- No resource leaks

**Failure Detection:**
- Memory usage increases monotonically = leak
- Use `valgrind --leak-check=full`
- Monitor `/proc/self/fd` for file descriptor leaks

**Test File:** `mvcc_stress_tests.rs::test_rapid_snapshot_lifecycle`

---

### Scenario 2.3: Large Graph Under Memory Pressure
**Objective:** Verify behavior with limited memory

**Setup:**
- Graph: 100K nodes, 500K edges
- Snapshots: 100 concurrent
- Memory limit: 2GB (via cgroup or ulimit)
- Monitor: OOM, swap usage

**Procedure:**
```rust
// Set memory limit
set_memory_limit(2_000_000_000)?;

let mut snapshots = Vec::new();
for _ in 0..100 {
    snapshots.push(graph.acquire_snapshot()?);
    // Monitor memory usage
}

// Verify all snapshots valid
for snapshot in snapshots {
    assert!(snapshot.node_count() > 0);
}
```

**Success Criteria:**
- System does not OOM
- All 100 snapshots created successfully
- Graceful degradation if memory exhausted
- Clear error message if cannot allocate

**Expected Behavior:**
- **Current:** May OOM (documented gap)
- **Desired:** LRU eviction or memory limit enforcement
- **Alternative:** Return error if memory insufficient

**Failure Detection:**
- OOM killer terminates process
- Use `dmesg` to check for OOM events
- Monitor `/proc/meminfo`

**Test File:** `mvcc_stress_tests.rs::test_large_graph_memory_pressure`

**Gap Analysis:** See Gap 6 in MVCC_GAP_ANALYSIS.md

---

### Scenario 2.4: Sustained Concurrent Reads + Writes
**Objective:** Long-running mixed workload

**Setup:**
- 5 writer threads (continuous writes)
- 20 reader threads (continuous snapshots)
- Duration: 5 minutes
- Graph: Starts at 1000 nodes, grows to 10K nodes

**Procedure:**
```rust
// Writers
for i in 0..5 {
    thread::spawn(move || {
        loop {
            insert_entity(&graph, random_entity())?;
            insert_edge(&graph, random_edge())?;
        }
    });
}

// Readers
for i in 0..20 {
    thread::spawn(move || {
        loop {
            let snapshot = graph.acquire_snapshot()?;
            validate_snapshot(&snapshot)?;
        }
    });
}

sleep(Duration::from_secs(300));
```

**Success Criteria:**
- System remains stable for 5 minutes
- No deadlocks or data corruption
- Writer throughput > 100 ops/sec
- Reader throughput > 1000 ops/sec

**Expected Behavior:**
- Writers and readers make progress
- No starvation of readers or writers
- System scales with workload

**Failure Detection:**
- Hang = deadlock (use watchdog)
- Corruption detected = data race
- Throughput drops to zero = starvation

**Test File:** `mvcc_stress_tests.rs::test_sustained_mixed_workload`

---

### Scenario 2.5: Snapshot Acquisition Spikes
**Objective:** Test burst behavior

**Setup:**
- Baseline: 1 snapshot/sec
- Spike: 1000 snapshots in 1 second
- Pattern: Baseline 10s, Spike 1s, repeat 5 times
- Graph: 500 nodes

**Procedure:**
```rust
for cycle in 0..5 {
    // Baseline
    for _ in 0..10 {
        thread::sleep(Duration::from_secs(1));
        graph.acquire_snapshot()?;
    }

    // Spike
    let start = Instant::now();
    for _ in 0..1000 {
        graph.acquire_snapshot()?;
    }
    let spike_duration = start.elapsed();

    assert!(spike_duration < Duration::from_secs(2));
}
```

**Success Criteria:**
- System handles spikes without crashing
- Spike latency < 2 seconds
- Returns to baseline after spike
- No resource exhaustion

**Expected Behavior:**
- ArcSwap handles bursts well
- No thundering herd
- Graceful degradation under load

**Failure Detection:**
- Latency spike > 2s = bottleneck
- Crash = resource exhaustion
- Monitor latency percentiles

**Test File:** `mvcc_stress_tests.rs::test_snapshot_acquisition_spikes`

---

### Scenario 2.6: Concurrent Snapshot + Checkpoint
**Objective:** Stress test WAL/snapshot coordination

**Setup:**
- Thread A: Checkpoint every 500ms
- Threads B-J: 9 threads acquiring snapshots continuously
- Duration: 60 seconds
- Graph: 5000 nodes, 20K edges
- Backend: SQLite with WAL

**Procedure:**
```rust
// Checkpoint thread
thread::spawn(|| {
    loop {
        graph.checkpoint()?;
        sleep(Duration::from_millis(500));
    }
});

// Snapshot threads (x9)
for _ in 0..9 {
    thread::spawn(|| {
        loop {
            let snapshot = graph.acquire_snapshot()?;
            validate_snapshot(&snapshot)?;
        }
    });
}

sleep(Duration::from_secs(60));
```

**Success Criteria:**
- No SQLite locking errors
- All checkpoints succeed
- All snapshots valid
- No deadlocks

**Expected Behavior:**
- **Current:** Undefined (Gap 5)
- **Desired:** Checkpoint blocks new snapshots
- **Alternative:** Snapshots bypass checkpoint locks

**Failure Detection:**
- `SQLITE_BUSY` errors = lock contention
- Deadlock = coordination gap
- Monitor checkpoint duration

**Test File:** `mvcc_stress_tests.rs::test_concurrent_snapshot_checkpoint`

**Gap Analysis:** See Gap 5 in MVCC_GAP_ANALYSIS.md

---

## 3. Correctness Scenarios

### Scenario 3.1: Snapshot Isolation Guarantees
**Objective:** Verify ACID-like isolation properties

**Setup:**
- Create baseline graph (100 nodes, 500 edges)
- Acquire snapshot S1
- Perform 100 write operations
- Acquire snapshot S2
- Verify S1 and S2 properties

**Procedure:**
```rust
let snapshot1 = graph.acquire_snapshot()?;
let state1 = (snapshot1.node_count(), snapshot1.edge_count());

// Perform writes
for _ in 0..100 {
    insert_entity(&graph, new_entity)?;
    insert_edge(&graph, new_edge)?;
}

let snapshot2 = graph.acquire_snapshot()?;
let state2 = (snapshot2.node_count(), snapshot2.edge_count());

// Verify isolation
assert_eq!(snapshot1.node_count(), state1.0);  // Unchanged
assert!(snapshot2.node_count() > snapshot1.node_count());  // Grew
```

**Success Criteria:**
- Snapshot1 unchanged after writes
- Snapshot2 reflects all writes
- No cross-contamination
- Repeatable reads within each snapshot

**Expected Behavior:**
- Snapshots provide point-in-time consistency
- Cloned HashMaps ensure full isolation
- No phantom reads

**Failure Detection:**
- Snapshot1 changes = isolation violation
- Snapshot2 missing writes = consistency violation
- Use property-based testing (quickcheck)

**Test File:** `mvcc_correctness_tests.rs::test_snapshot_isolation_guarantees`

---

### Scenario 3.2: Data Race Detection with Sanitizers
**Objective:** Systematic data race detection

**Setup:**
- All concurrent scenarios compiled with thread sanitizer
- 24-hour stress test
- All scenarios from Section 1 and 2

**Procedure:**
```bash
# Run with thread sanitizer
RUSTFLAGS="-Z sanitizer=thread" \
cargo test --package sqlitegraph mvcc_concurrent_tests --release
```

**Success Criteria:**
- Zero data race reports
- All tests pass
- No sanitizer warnings

**Expected Behavior:**
- ArcSwap provides lock-free synchronization
- Cloned HashMaps prevent data races
- RwLock usage correct

**Failure Detection:**
- Thread sanitizer detects data race
- Fix: Add proper synchronization or Arc wrapping

**Test File:** `mvcc_correctness_tests.rs::test_with_thread_sanitizer`

---

### Scenario 3.3: No Data Races in Concurrent Access
**Objective:** Verify lock-free claims

**Setup:**
- Use Loom for systematic concurrency testing
- Test all ArcSwap operations
- Enumerate all possible interleavings

**Procedure:**
```rust
#[test]
fn test_arc_swap_with_loom() {
    loom::model(|| {
        let manager = SnapshotManager::new();
        let handle1 = thread::spawn(|| {
            manager.acquire_snapshot();
        });
        let handle2 = thread::spawn(|| {
            manager.update_snapshot(&outgoing, &incoming);
        });
        handle1.join().unwrap();
        handle2.join().unwrap();
    });
}
```

**Success Criteria:**
- All Loom tests pass
- All interleavings explored
- No memory corruption

**Expected Behavior:**
- ArcSwap atomic operations prevent races
- Proper memory ordering
- No use-after-free

**Failure Detection:**
- Loom detects race or memory corruption
- Fix: Adjust atomic ordering or synchronization

**Test File:** `mvcc_correctness_tests.rs::test_arc_swap_with_loom`

---

### Scenario 3.4: Snapshot Consistency Under Concurrent Writes
**Objective:** Verify snapshot internal consistency

**Setup:**
- 10 writer threads
- 5 snapshot reader threads
- Each snapshot validates internal consistency
- Duration: 60 seconds

**Procedure:**
```rust
// Readers
for _ in 0..5 {
    thread::spawn(|| {
        loop {
            let snapshot = graph.acquire_snapshot()?;

            // Validate internal consistency
            let node_count = snapshot.node_count();
            let edge_count = snapshot.edge_count();

            // Verify adjacency data consistent
            for node in snapshot.nodes() {
                let outgoing = snapshot.get_outgoing(node);
                let incoming = snapshot.get_incoming(node);
                // Validate edge counts match
            }
        }
    });
}
```

**Success Criteria:**
- All snapshots internally consistent
- No orphan edges (edge referencing non-existent node)
- No duplicate edges
- Adjacency symmetry (outgoing/incoming match)

**Expected Behavior:**
- Cloned HashMaps provide consistent snapshot
- No partial updates visible
- Graph invariants preserved

**Failure Detection:**
- Inconsistent snapshot = bug in cloning or coordination
- Use invariant checking

**Test File:** `mvcc_correctness_tests.rs::test_snapshot_consistency_under_writes`

---

### Scenario 3.5: ArcSwap Atomic Guarantees Verification
**Objective:** Verify ArcSwap provides atomic swaps

**Setup:**
- Thread A: Update state 1000 times
- Thread B: Acquire snapshots 1000 times
- Verify monotonic state progression

**Procedure:**
```rust
let state = Arc::new(AtomicUsize::new(0));

// Thread A
thread::spawn(|| {
    for i in 1..=1000 {
        manager.update_snapshot_with_state(i);
        state.store(i, Ordering::Release);
    }
});

// Thread B
thread::spawn(|| {
    loop {
        let snapshot = manager.acquire_snapshot();
        let snapshot_state = snapshot.get_state();
        let current_state = state.load(Ordering::Acquire);

        // Verify monotonic: snapshot_state <= current_state
        assert!(snapshot_state <= current_state);
    }
});
```

**Success Criteria:**
- No snapshot sees state > current state
- No torn state (partial update visible)
- Atomic swaps verified

**Expected Behavior:**
- ArcSwap guarantees atomic pointer swap
- No intermediate states visible
- Linearizable updates

**Failure Detection:**
- Snapshot sees future state = race condition
- Non-monotonic state = atomic violation

**Test File:** `mvcc_correctness_tests.rs::test_arc_swap_atomicity`

---

### Scenario 3.6: Concurrent Snapshot Ordering
**Objective:** Verify snapshot temporal ordering

**Setup:**
- Acquire 10 snapshots concurrently
- Verify creation timestamps
- Verify state monotonicity

**Procedure:**
```rust
let barrier = Arc::new(Barrier::new(10));

let handles: Vec<_> = (0..10).map(|i| {
    let barrier = barrier.clone();
    thread::spawn(move || {
        barrier.wait();
        let snapshot = graph.acquire_snapshot()?;
        Ok::<_, Error>((i, snapshot.created_at()))
    })
}).collect();

// Verify ordering
let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
let timestamps: Vec<_> = results.iter().map(|(_, t)| *t).collect();
assert!(timestamps.is_sorted());
```

**Success Criteria:**
- Timestamps monotonically increasing
- No duplicate timestamps (within resolution)
- State progression matches timestamps

**Expected Behavior:**
- SystemTime provides monotonic timestamps
- Snapshots ordered by creation time
- No timestamp wraparound

**Failure Detection:**
- Non-monotonic timestamps = clock issue
- Duplicate timestamps = resolution too coarse

**Test File:** `mvcc_correctness_tests.rs::test_snapshot_ordering`

---

## 4. Performance Scenarios

### Scenario 4.1: Snapshot Contention Under High Load
**Objective:** Measure scalability and contention

**Setup:**
- Vary thread count: 1, 2, 4, 8, 16, 32, 64, 128
- Fixed workload: 10,000 snapshot acquisitions per thread
- Measure: Throughput, latency, CPU usage

**Procedure:**
```rust
for thread_count in [1, 2, 4, 8, 16, 32, 64, 128] {
    let start = Instant::now();

    let handles: Vec<_> = (0..thread_count).map(|_| {
        thread::spawn(|| {
            for _ in 0..10_000 {
                let _snapshot = graph.acquire_snapshot()?;
            }
        })
    }).collect();

    for h in handles { h.join().unwrap(); }

    let duration = start.elapsed();
    let throughput = (thread_count * 10_000) / duration.as_secs();
    println!("Threads: {}, Throughput: {}/s", thread_count, throughput);
}
```

**Success Criteria:**
- Throughput increases with thread count (to ~16 threads)
- Latency remains < 10ms P99
- CPU usage scales linearly
- No throughput collapse at high thread counts

**Expected Behavior:**
- ArcSwap scales well (lock-free)
- Throughput plateaus at core count
- Minimal contention overhead

**Failure Detection:**
- Throughput decreases with more threads = contention
- High P99 latency = bottleneck
- Use flamegraph to identify hotspots

**Test File:** `mvcc_performance_tests.rs::test_snapshot_contention_scaling`

---

### Scenario 4.2: Reader/Writer Priority Handling
**Objective:** Verify fair access under contention

**Setup:**
- 1 writer thread (continuous writes)
- 10 reader threads (continuous snapshots)
- Measure: Reader latency, writer throughput
- Duration: 60 seconds

**Procedure:**
```rust
// Writer
thread::spawn(|| {
    loop {
        insert_entity(&graph, new_entity)?;
        insert_edge(&graph, new_edge)?;
    }
});

// Readers
for i in 0..10 {
    thread::spawn(move || {
        loop {
            let start = Instant::now();
            let snapshot = graph.acquire_snapshot()?;
            let latency = start.elapsed();
            reader_latencies[i].push(latency);
        }
    });
}

sleep(Duration::from_secs(60));
```

**Success Criteria:**
- Reader P50 latency < 5ms
- Reader P99 latency < 50ms
- Writer throughput > 100 ops/sec
- No starvation (both readers and writer progress)

**Expected Behavior:**
- Readers not blocked by writer
- Writer not starved by readers
- Fair scheduling

**Failure Detection:**
- High reader latency = writer blocks readers
- Low writer throughput = readers starve writer
- Monitor latency distribution

**Test File:** `mvcc_performance_tests.rs::test_reader_writer_priority`

---

### Scenario 4.3: Cache Coherency Under Concurrent Access
**Objective:** Verify cache behavior under concurrency

**Setup:**
- 10 threads reading snapshots
- 5 threads writing
- Measure: Cache hit rate, invalidation frequency
- Duration: 60 seconds

**Procedure:**
```rust
// Monitor cache stats
let initial_stats = cache_stats(&graph);

// Run workload (readers + writers)
// ...

let final_stats = cache_stats(&graph);

println!("Cache hits: {}", final_stats.hits - initial_stats.hits);
println!("Cache misses: {}", final_stats.misses - initial_stats.misses);
```

**Success Criteria:**
- Cache hit rate > 90%
- Cache invalidations don't affect snapshots
- No cache coherency issues

**Expected Behavior:**
- Snapshots use cloned data, not cache
- Cache invalidation independent of snapshots
- High cache hit rate for main graph

**Failure Detection:**
- Low cache hit rate = thrashing
- Snapshot changes with cache invalidation = bug

**Test File:** `mvcc_performance_tests.rs::test_cache_coherency_concurrent`

---

### Scenario 4.4: Memory Allocation Pattern Analysis
**Objective:** Measure allocation overhead

**Setup:**
- Use allocator profiling (e.g., jemalloc stats)
- 1000 snapshot acquisitions
- Measure: Allocation count, total bytes, peak usage

**Procedure:**
```rust
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

let snapshot = graph.acquire_snapshot()?;
// Get allocation stats
let stats = get_allocation_stats();
println!("Allocations: {}", stats.allocations());
println!("Total bytes: {}", stats.total_bytes());
```

**Success Criteria:**
- Per-snapshot allocation < 1MB (for 1000-node graph)
- No memory leaks (deallocations match allocations)
- Allocation count scales with graph size

**Expected Behavior:**
- HashMap clone allocates new memory
- Arc::clone is cheap (just pointer increment)
- Predictable allocation pattern

**Failure Detection:**
- High allocation count = inefficiency
- Memory leak = deallocations < allocations
- Use heap profiling tools

**Test File:** `mvcc_performance_tests.rs::test_memory_allocation_patterns`

---

### Scenario 4.5: Snapshot Clone Performance
**Objective:** Benchmark Arc::clone performance

**Setup:**
- Create snapshot
- Clone 1,000,000 times
- Measure: Total time, per-clone time

**Procedure:**
```rust
let snapshot = graph.acquire_snapshot()?;

let start = Instant::now();
for _ in 0..1_000_000 {
    let _clone = snapshot.state().clone();
}
let duration = start.elapsed();

let per_clone = duration / 1_000_000;
println!("Per-clone time: {:?}", per_clone);
```

**Success Criteria:**
- Per-clone time < 100ns
- Total time < 100ms
- No allocations during clone (Arc::clone is just atomic increment)

**Expected Behavior:**
- Arc::clone is very fast (atomic fetch_add)
- No memory allocation
- Constant time regardless of graph size

**Failure Detection:**
- Slow clone = synchronization overhead
- Allocations during clone = bug

**Test File:** `mvcc_performance_tests.rs::test_snapshot_clone_performance`

---

### Scenario 4.6: Concurrent Snapshot Throughput Benchmark
**Objective:** Establish performance baseline

**Setup:**
- Vary thread count: 1, 2, 4, 8, 16, 32
- Fixed graph: 1000 nodes, 5000 edges
- Measure: Snapshots/second

**Procedure:**
```rust
for thread_count in [1, 2, 4, 8, 16, 32] {
    let start = Instant::now();

    let handles: Vec<_> = (0..thread_count).map(|_| {
        thread::spawn(|| {
            let mut count = 0;
            for _ in 0..10_000 {
                let _ = graph.acquire_snapshot();
                count += 1;
            }
            count
        })
    }).collect();

    let total: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
    let duration = start.elapsed();
    let throughput = total / duration.as_secs_f64();

    println!("Threads: {}, Throughput: {}/s", thread_count, throughput);
}
```

**Success Criteria:**
- 1 thread: > 10,000 snapshots/sec
- 8 threads: > 50,000 snapshots/sec
- 32 threads: > 100,000 snapshots/sec
- Linear scaling to ~8 threads

**Expected Behavior:**
- High throughput due to lock-free design
- Scales to number of cores
- Plateaus at core count

**Failure Detection:**
- Low throughput = performance regression
- Poor scaling = contention

**Test File:** `mvcc_benchmarks.rs::snapshot_throughput`

---

## 5. Implementation Priority

### Phase 1: Critical Correctness (Plans 02-01 through 02-03)
1. Scenario 1.1: Snapshot During State Update
2. Scenario 1.2: Read During Write
3. Scenario 3.2: Data Race Detection with Sanitizers
4. Scenario 3.3: No Data Races (Loom)

### Phase 2: Stress Testing (Plans 02-04 through 02-06)
5. Scenario 2.1: 100 Threads Stress Test
6. Scenario 2.2: Rapid Lifecycle
7. Scenario 2.4: Sustained Workload
8. Scenario 2.5: Acquisition Spikes

### Phase 3: Performance (Plans 02-07 through 02-09)
9. Scenario 4.1: Contention Under Load
10. Scenario 4.2: Reader/Writer Priority
11. Scenario 4.6: Throughput Benchmark

### Phase 4: Edge Cases (Plans 02-10 through 02-12)
12. Scenario 1.3: During Checkpoint
13. Scenario 1.5: During Rollback
14. Scenario 2.3: Memory Pressure

---

## 6. Test Infrastructure

### Required Tools
- **loom**: Systematic concurrency testing
- **thread sanitizer**: Data race detection
- **criterion**: Performance benchmarking
- **flamegraph**: Performance profiling
- **valgrind**: Memory leak detection

### Test Organization
```
sqlitegraph/tests/
├── mvcc_baseline_tests.rs      (existing - Plan 01)
├── mvcc_concurrent_tests.rs    (new - Plan 02)
├── mvcc_stress_tests.rs        (new - Plan 02)
├── mvcc_correctness_tests.rs   (new - Plan 02)
└── mvcc_performance_tests.rs   (new - Plan 02)

sqlitegraph/benches/
└── mvcc_benchmarks.rs          (new - Plan 02)
```

### CI Integration
```yaml
# .github/workflows/mvcc-tests.yml
name: MVCC Concurrent Tests

on: [push, pull_request]

jobs:
  concurrent:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Run thread sanitizer
        run: |
          RUSTFLAGS="-Z sanitizer=thread" \
          cargo test --package sqlitegraph mvcc_concurrent_tests
      - name: Run stress tests
        run: cargo test --package sqlitegraph mvcc_stress_tests --release
```

---

## 7. Success Metrics

### Correctness
- [ ] All scenarios pass without errors
- [ ] Zero data race reports (thread sanitizer)
- [ ] All Loom tests pass (100% interleaving coverage)
- [ ] No deadlocks or livelocks

### Performance
- [ ] Throughput > 10,000 snapshots/sec (single thread)
- [ ] P99 latency < 100ms (concurrent)
- [ ] Linear scaling to 8 threads
- [ ] Memory usage < 10MB per snapshot (1K nodes)

### Stress
- [ ] 100 threads, 30 seconds: no crashes
- [ ] 10K rapid lifecycle ops: no leaks
- [ ] 5-minute mixed workload: stable

### Edge Cases
- [ ] Checkpoint coordination: defined behavior
- [ ] Rollback isolation: verified
- [ ] Memory pressure: graceful degradation

---

## 8. Gap Analysis Cross-Reference

This test suite directly addresses gaps from MVCC_GAP_ANALYSIS.md:

| Gap | Scenarios |
|-----|-----------|
| Gap 1: No concurrent tests | All 24 scenarios |
| Gap 4: Undefined during WAL recovery | 1.3, 2.6 |
| Gap 5: Undefined during checkpoint | 1.3, 2.6 |
| Gap 6: No memory pressure handling | 2.3 |
| Gap 8: RwLock contention | 4.1, 4.2 |
| Gap 9: No cache coherency tests | 1.6, 4.3 |

---

**End of MVCC Test Scenarios Document**
**Next Step:** Implement scenarios in Plan 04-02 (Concurrent Test Implementation)
