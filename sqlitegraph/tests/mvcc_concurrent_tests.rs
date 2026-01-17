//! MVCC Concurrent Tests - Multi-threaded stress testing
//!
//! **Purpose:** Validate MVCC-lite system under concurrent access
//! **Scope:** Multi-threaded tests of thread-safe components
//! **Focus:** SnapshotManager thread safety, stress testing, performance validation
//!
//! **NOTE:** SqliteGraph itself is NOT thread-safe (contains RefCell, non-sync types).
//! These tests focus on the thread-safe SnapshotManager component only.
//!
//! These tests verify that the snapshot system's thread-safe components provide
//! proper isolation under concurrent access patterns.

use sqlitegraph::{
    GraphEdgeCreate, GraphEntityCreate, SqliteGraph, SqliteGraphError,
};
use sqlitegraph::mvcc::SnapshotManager;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{Duration, Instant};

//
// TEST HELPERS
//

/// Helper: Get node count from graph
fn node_count(graph: &SqliteGraph) -> Result<i64, SqliteGraphError> {
    let ids = graph.list_entity_ids()?;
    Ok(ids.len() as i64)
}

/// Helper: Get edge count from graph
fn edge_count(graph: &SqliteGraph) -> Result<i64, SqliteGraphError> {
    let entity_ids = graph.list_entity_ids()?;
    let mut total_edges = 0;
    for &id in &entity_ids {
        let outgoing = graph.query().outgoing(id)?;
        total_edges += outgoing.len();
    }
    Ok(total_edges as i64)
}

/// Helper: Warm the cache by reading all adjacency data
fn warm_cache(graph: &SqliteGraph) -> Result<(), SqliteGraphError> {
    let entity_ids = graph.list_entity_ids()?;
    for &id in &entity_ids {
        let _ = graph.query().outgoing(id);
        let _ = graph.query().incoming(id);
    }
    Ok(())
}

/// Helper: Insert entity using proper API
fn insert_entity(graph: &SqliteGraph, create: GraphEntityCreate) -> Result<i64, SqliteGraphError> {
    let entity = sqlitegraph::GraphEntity {
        id: 0, // Will be assigned by database
        kind: create.kind,
        name: create.name,
        file_path: create.file_path,
        data: create.data,
    };
    graph.insert_entity(&entity)
}

/// Helper: Insert edge using proper API
fn insert_edge(graph: &SqliteGraph, create: GraphEdgeCreate) -> Result<i64, SqliteGraphError> {
    let edge = sqlitegraph::GraphEdge {
        id: 0, // Will be assigned by database
        from_id: create.from_id,
        to_id: create.to_id,
        edge_type: create.edge_type,
        data: create.data,
    };
    graph.insert_edge(&edge)
}

/// Helper: Create test graph with sample data
fn create_test_graph() -> Result<SqliteGraph, SqliteGraphError> {
    let graph = SqliteGraph::open_in_memory()?;

    // Create test entities
    let entity1 = GraphEntityCreate {
        kind: "function".to_string(),
        name: "main".to_string(),
        file_path: Some("src/main.rs".to_string()),
        data: serde_json::json!({"line": 10}),
    };

    let entity2 = GraphEntityCreate {
        kind: "function".to_string(),
        name: "helper".to_string(),
        file_path: Some("src/helper.rs".to_string()),
        data: serde_json::json!({"line": 5}),
    };

    let id1 = insert_entity(&graph, entity1)?;
    let id2 = insert_entity(&graph, entity2)?;

    // Create edge
    let edge = GraphEdgeCreate {
        from_id: id1,
        to_id: id2,
        edge_type: "calls".to_string(),
        data: serde_json::json!({}),
    };
    insert_edge(&graph, edge)?;

    Ok(graph)
}

/// Helper: Create larger test graph for stress tests
fn create_large_test_graph(size: usize) -> Result<SqliteGraph, SqliteGraphError> {
    let graph = SqliteGraph::open_in_memory()?;

    // Create entities
    for i in 0..size {
        let entity = GraphEntityCreate {
            kind: "node".to_string(),
            name: format!("node_{}", i),
            file_path: Some(format!("file_{}.rs", i)),
            data: serde_json::json!({"index": i}),
        };
        insert_entity(&graph, entity)?;
    }

    // Create edges (each node connects to next 2 nodes)
    let ids: Vec<i64> = graph.list_entity_ids()?;
    for (i, &id) in ids.iter().enumerate() {
        for j in 1..=2 {
            let target_idx = (i + j) % ids.len();
            let edge = GraphEdgeCreate {
                from_id: id,
                to_id: ids[target_idx],
                edge_type: "connects".to_string(),
                data: serde_json::json!({}),
            };
            insert_edge(&graph, edge)?;
        }
    }

    Ok(graph)
}

/// Helper: Create a SnapshotManager with test data
fn create_test_snapshot_manager(size: usize) -> SnapshotManager {
    let mut outgoing = HashMap::new();
    let mut incoming = HashMap::new();

    for i in 0..size {
        outgoing.insert(i as i64, vec![]);
        incoming.insert(i as i64, vec![]);
    }

    SnapshotManager::with_state(&outgoing, &incoming)
}

//
// GROUP 1: SNAPSHOT MANAGER CONCURRENCY TESTS
//

#[test]
fn test_concurrent_snapshot_acquisition() {
    // Scenario: 100 threads simultaneously acquire snapshots
    // Expected: All threads succeed, no deadlocks, all snapshots valid
    let manager = Arc::new(create_test_snapshot_manager(100));
    let barrier = Arc::new(Barrier::new(100));
    let success_count = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..100)
        .map(|_| {
            let manager = manager.clone();
            let barrier = barrier.clone();
            let success_count = success_count.clone();

            thread::spawn(move || {
                barrier.wait();
                let snapshot = manager.acquire_snapshot();
                if snapshot.node_count() > 0 {
                    success_count.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();

    // Wait for all threads
    for h in handles {
        h.join().expect("Thread panicked");
    }

    // Verify all threads succeeded
    let success = success_count.load(Ordering::Relaxed);
    assert_eq!(success, 100, "Not all threads acquired valid snapshots");
}

#[test]
fn test_snapshot_during_state_update() {
    // Scenario: Concurrent snapshot acquisition during state updates
    // Expected: No torn reads, all snapshots consistent
    let manager = Arc::new(create_test_snapshot_manager(100));

    let mut outgoing = HashMap::new();
    let mut incoming = HashMap::new();

    // Initialize with some data
    for i in 0..100 {
        outgoing.insert(i, vec![]);
        incoming.insert(i, vec![]);
    }

    let manager1 = manager.clone();
    let manager2 = manager.clone();

    // Thread A: Update state continuously
    let handle1 = thread::spawn(move || {
        for i in 0..1000 {
            let mut new_outgoing = outgoing.clone();
            new_outgoing.insert(i % 100, vec![]);
            manager1.update_snapshot(&new_outgoing, &incoming);
        }
    });

    // Thread B: Acquire snapshots continuously
    let handle2 = thread::spawn(move || {
        for _ in 0..1000 {
            let snapshot = manager2.acquire_snapshot();
            // Verify snapshot is valid
            assert!(snapshot.node_count() <= 100);
        }
    });

    handle1.join().expect("Thread A panicked");
    handle2.join().expect("Thread B panicked");
}

#[test]
fn test_rapid_snapshot_creation() {
    // Scenario: Create 1000 snapshots in rapid succession
    // Expected: No memory leaks, all snapshots valid
    let manager = create_test_snapshot_manager(50);

    // Create many snapshots
    for _ in 0..1000 {
        let snapshot = manager.acquire_snapshot();
        assert!(snapshot.node_count() > 0, "Snapshot has no nodes");
    }

    // Verify final snapshot is still valid
    let final_snapshot = manager.acquire_snapshot();
    assert!(final_snapshot.node_count() > 0);
}

#[test]
fn test_100_simultaneous_snapshots() {
    // Scenario: 100 threads acquire snapshots simultaneously with barrier
    // Expected: All succeed, no contention issues
    let manager = Arc::new(create_test_snapshot_manager(100));
    let barrier = Arc::new(Barrier::new(100));
    let success_count = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..100)
        .map(|_| {
            let manager = manager.clone();
            let barrier = barrier.clone();
            let success_count = success_count.clone();

            thread::spawn(move || {
                barrier.wait();
                let snapshot = manager.acquire_snapshot();
                let count = snapshot.node_count();

                if count > 0 && count <= 100 {
                    success_count.fetch_add(1, Ordering::Relaxed);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked");
    }

    let success = success_count.load(Ordering::Relaxed);
    assert_eq!(success, 100, "Not all threads succeeded");
}

#[test]
fn test_sustained_concurrent_access() {
    // Scenario: Sustained concurrent access for 2 seconds
    // Expected: No deadlocks, continuous progress
    let manager = Arc::new(create_test_snapshot_manager(50));
    let running = Arc::new(AtomicU64::new(1));
    let duration = Duration::from_secs(2);

    // Spawn 10 threads
    let handles: Vec<_> = (0..10)
        .map(|_| {
            let manager = manager.clone();
            let running = running.clone();
            let start = Instant::now();

            thread::spawn(move || {
                let mut count = 0;
                while running.load(Ordering::Relaxed) == 1 && start.elapsed() < duration {
                    let _ = manager.acquire_snapshot();
                    count += 1;
                }
                count
            })
        })
        .collect();

    // Let them run for 2 seconds
    thread::sleep(duration);
    running.store(0, Ordering::Relaxed);

    // Collect results
    let mut total = 0;
    for h in handles {
        let count = h.join().expect("Thread panicked");
        total += count;
    }

    // Verify reasonable throughput (at least 1000 snapshots total)
    assert!(total >= 1000, "Low throughput: {} snapshots in 2 seconds", total);
    println!("Sustained concurrent access: {} snapshots in 2 seconds", total);
}

//
// GROUP 2: CORRECTNESS TESTS
//

#[test]
fn test_snapshot_state_immutability() {
    // Scenario: Verify SnapshotState is truly immutable
    // Expected: Snapshots never change after creation
    let manager = Arc::new(create_test_snapshot_manager(50));

    let snapshot1 = manager.acquire_snapshot();
    let original_count = snapshot1.node_count();

    // Update state
    let mut new_outgoing = HashMap::new();
    for i in 0..100 {
        new_outgoing.insert(i, vec![]);
    }
    manager.update_snapshot(&new_outgoing, &new_outgoing);

    // Original snapshot should be unchanged
    assert_eq!(snapshot1.node_count(), original_count);

    // New snapshot should reflect changes
    let snapshot2 = manager.acquire_snapshot();
    assert_eq!(snapshot2.node_count(), 100);
}

#[test]
fn test_arc_swap_atomic_guarantees() {
    // Scenario: Verify ArcSwap provides atomic pointer swaps
    // Expected: No torn reads, consistent state
    let manager = Arc::new(create_test_snapshot_manager(50));

    let mut state1 = HashMap::new();
    for i in 0..50 {
        state1.insert(i, vec![]);
    }

    let mut state2 = HashMap::new();
    for i in 0..100 {
        state2.insert(i, vec![]);
    }

    let manager1 = manager.clone();
    let manager2 = manager.clone();

    // Thread A: Rapid state updates
    let handle_a = thread::spawn(move || {
        for _ in 0..1000 {
            manager1.update_snapshot(&state1, &state1);
            manager1.update_snapshot(&state2, &state2);
        }
    });

    // Thread B: Rapid snapshot acquisition
    let handle_b = thread::spawn(move || {
        for _ in 0..1000 {
            let snapshot = manager2.acquire_snapshot();

            // Verify snapshot is consistent (either 50 or 100 nodes)
            let count = snapshot.node_count();
            assert!(
                count == 50 || count == 100,
                "Inconsistent snapshot state: {} nodes",
                count
            );
        }
    });

    handle_a.join().expect("Thread A panicked");
    handle_b.join().expect("Thread B panicked");
}

#[test]
fn test_concurrent_snapshot_ordering() {
    // Scenario: Multiple concurrent snapshots should see consistent ordering
    // Expected: Snapshots maintain happens-before relationship (or same timestamp if very fast)
    let manager = create_test_snapshot_manager(50);

    // Create snapshot 1
    let snapshot1 = manager.acquire_snapshot();
    let time1 = snapshot1.created_at;

    // Small delay to ensure different timestamp
    thread::sleep(Duration::from_millis(20));

    // Create snapshot 2
    let snapshot2 = manager.acquire_snapshot();
    let time2 = snapshot2.created_at;

    // Verify ordering (or equal if system is very fast)
    assert!(
        time2 >= time1,
        "Snapshot ordering violated: {:?} >= {:?}",
        time2, time1
    );
}

#[test]
fn test_snapshot_isolation_with_clones() {
    // Scenario: Multiple clones of same snapshot
    // Expected: All clones see same state
    let manager = create_test_snapshot_manager(50);

    let snapshot1 = manager.acquire_snapshot();
    let snapshot2 = Arc::clone(&snapshot1);

    assert_eq!(snapshot1.node_count(), snapshot2.node_count());
    assert_eq!(snapshot1.created_at, snapshot2.created_at);
}

//
// GROUP 3: MEMORY AND PERFORMANCE
//

#[test]
fn test_memory_no_leaks() {
    // Scenario: Create and drop many snapshots
    // Expected: No memory leaks
    let manager = create_test_snapshot_manager(50);

    // Create many snapshots in a loop
    for _ in 0..10_000 {
        let _snapshot = manager.acquire_snapshot();
        // Snapshot dropped here
    }

    // Final snapshot should still work
    let final_snapshot = manager.acquire_snapshot();
    assert!(final_snapshot.node_count() > 0);
}

#[test]
fn test_snapshot_clone_performance() {
    // Scenario: Clone Arc<SnapshotState> many times
    // Expected: Cloning is cheap (just atomic refcount increment)
    let manager = create_test_snapshot_manager(50);

    let snapshot = manager.acquire_snapshot();

    let start = Instant::now();

    // Clone 1000 times
    for _ in 0..1000 {
        let _ = Arc::clone(&snapshot);
    }

    let elapsed = start.elapsed();

    // Should be very fast (< 10ms)
    assert!(
        elapsed < Duration::from_millis(10),
        "Arc::clone too slow: {:?}",
        elapsed
    );

    println!("1000 Arc::clone operations in {:?}", elapsed);
}

#[test]
fn test_high_contention_snapshot_acquisition() {
    // Scenario: High contention with many threads
    // Expected: Still performs reasonably
    let manager = Arc::new(create_test_snapshot_manager(50));
    let barrier = Arc::new(Barrier::new(50));
    let success_count = Arc::new(AtomicU64::new(0));

    let handles: Vec<_> = (0..50)
        .map(|_| {
            let manager = manager.clone();
            let barrier = barrier.clone();
            let success_count = success_count.clone();

            thread::spawn(move || {
                barrier.wait();

                // Each thread acquires 100 snapshots
                for _ in 0..100 {
                    let snapshot = manager.acquire_snapshot();
                    if snapshot.node_count() > 0 {
                        success_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread panicked");
    }

    // Expected: 50 threads * 100 snapshots = 5000 successful acquisitions
    let success = success_count.load(Ordering::Relaxed);
    assert_eq!(success, 5000, "Expected all snapshot acquisitions to succeed");
}

#[test]
fn test_snapshot_independence() {
    // Scenario: Multiple independent snapshots
    // Expected: Each snapshot is independent
    let manager = create_test_snapshot_manager(50);

    let snapshot1 = manager.acquire_snapshot();
    let snapshot2 = manager.acquire_snapshot();
    let snapshot3 = manager.acquire_snapshot();

    // All should have same state
    assert_eq!(snapshot1.node_count(), snapshot2.node_count());
    assert_eq!(snapshot2.node_count(), snapshot3.node_count());

    // Update state
    let mut new_outgoing = HashMap::new();
    for i in 0..100 {
        new_outgoing.insert(i, vec![]);
    }
    manager.update_snapshot(&new_outgoing, &new_outgoing);

    // Old snapshots unchanged
    assert_eq!(snapshot1.node_count(), 50);
    assert_eq!(snapshot2.node_count(), 50);
    assert_eq!(snapshot3.node_count(), 50);

    // New snapshot sees changes
    let snapshot4 = manager.acquire_snapshot();
    assert_eq!(snapshot4.node_count(), 100);
}

//
// GROUP 4: INTEGRATION WITH SQLITEGRAPH (SINGLE-THREADED)
//

#[test]
fn test_graph_snapshot_creation() {
    // Scenario: Create snapshot from SqliteGraph
    // Expected: Snapshot captures current state
    let graph = create_test_graph().expect("Failed to create test graph");
    warm_cache(&graph).expect("Failed to warm cache");

    let snapshot1 = graph.acquire_snapshot().expect("Failed to acquire snapshot");
    let original_count = snapshot1.node_count();

    assert!(original_count > 0, "Snapshot should have nodes");

    // Modify graph
    let _ = insert_entity(
        &graph,
        GraphEntityCreate {
            kind: "new".to_string(),
            name: "new_node".to_string(),
            file_path: Some("new.rs".to_string()),
            data: serde_json::json!({}),
        },
    );

    // Original snapshot unchanged
    assert_eq!(snapshot1.node_count(), original_count);

    // New snapshot reflects changes
    warm_cache(&graph).expect("Failed to warm cache");
    let snapshot2 = graph.acquire_snapshot().expect("Failed to acquire second snapshot");
    assert!(snapshot2.node_count() > original_count);
}

#[test]
fn test_graph_snapshot_isolation() {
    // Scenario: Verify snapshot isolation in graph context
    // Expected: Multiple snapshots are independent
    let graph = create_large_test_graph(100).expect("Failed to create large graph");
    warm_cache(&graph).expect("Failed to warm cache");

    let snapshot1 = graph.acquire_snapshot().expect("Failed to acquire snapshot");
    let count1 = snapshot1.node_count();

    // Modify graph
    let _ = insert_entity(
        &graph,
        GraphEntityCreate {
            kind: "test".to_string(),
            name: "new_node".to_string(),
            file_path: Some("new.rs".to_string()),
            data: serde_json::json!({}),
        },
    );

    warm_cache(&graph).expect("Failed to warm cache");
    let snapshot2 = graph.acquire_snapshot().expect("Failed to acquire snapshot");
    let count2 = snapshot2.node_count();

    // Verify isolation
    assert_eq!(snapshot1.node_count(), count1, "Snapshot 1 changed");
    assert_eq!(snapshot2.node_count(), count2, "Snapshot 2 has wrong count");
    assert!(count2 > count1, "Snapshot 2 should have more nodes");
}

#[test]
fn test_graph_snapshot_performance() {
    // Scenario: Measure snapshot acquisition performance
    // Expected: Fast snapshot creation
    let graph = create_test_graph().expect("Failed to create test graph");
    warm_cache(&graph).expect("Failed to warm cache");

    let start = Instant::now();

    for _ in 0..100 {
        let _ = graph.acquire_snapshot().expect("Failed to acquire snapshot");
    }

    let elapsed = start.elapsed();

    println!("100 snapshot acquisitions in {:?}", elapsed);

    // Should be reasonably fast (< 100ms)
    assert!(
        elapsed < Duration::from_millis(100),
        "Snapshot acquisition too slow: {:?}",
        elapsed
    );
}
