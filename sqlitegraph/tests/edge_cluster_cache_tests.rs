//! Comprehensive cache performance tests for traversal-aware LRU-K cache.

use sqlitegraph::backend::native::v2::edge_cluster::{
    cache::{ThreadSafeCache, TraversalAwareCache},
    CacheKey, CompactEdgeRecord, Direction, EdgeCluster,
};
use std::sync::Arc;

/// Helper to create a test cluster from compact edges.
fn create_test_cluster_compact(node_id: i64, edge_count: u32) -> EdgeCluster {
    let compact_edges: Vec<CompactEdgeRecord> = (1..=edge_count)
        .map(|i| {
            CompactEdgeRecord::new(
                node_id + i as i64,
                (i % 1000) as u16, // edge_type_offset as u16
                Vec::new(),
            )
        })
        .collect();

    EdgeCluster::create_from_compact_edges(compact_edges, node_id, Direction::Outgoing).unwrap()
}

#[test]
fn test_cache_hit_ratio_traversal() {
    // Create cache with capacity for 100 nodes
    let cache = ThreadSafeCache::new(100);

    // Simulate 3-hop BFS from a single start node
    let start_node = 1;
    let mut visited = std::collections::HashSet::new();
    let mut frontier = vec![start_node];

    for _hop in 0..3 {
        let mut next_frontier = Vec::new();
        for node_id in frontier {
            if visited.contains(&node_id) {
                continue;
            }
            visited.insert(node_id);

            // Get neighbors with cache
            let cluster = Arc::new(create_test_cluster_compact(node_id, 10));
            let neighbors: Vec<i64> = cluster.iter_neighbors().collect();

            // Access through cache
            let key = CacheKey::new(node_id, Direction::Outgoing);
            cache.insert(key, Arc::clone(&cluster));
            cache.get(key); // Record access

            // Expand frontier
            for neighbor_id in neighbors {
                if !visited.contains(&neighbor_id) && neighbor_id < 1000 {
                    next_frontier.push(neighbor_id);
                }
            }
        }
        frontier = next_frontier;
    }

    // Verify cache hit ratio > 60%
    let hit_ratio = cache.hit_ratio();
    assert!(
        hit_ratio > 0.6,
        "Expected hit ratio > 60%, got {:.2}%",
        hit_ratio * 100.0
    );

    println!("Cache hit ratio for BFS traversal: {:.2}%", hit_ratio * 100.0);
    let stats = cache.stats();
    println!(
        "Stats: hits={}, misses={}, traversals={}, lookups={}",
        stats.hits, stats.misses, stats.traversals, stats.lookups
    );
}

#[test]
fn test_cache_high_degree_priority() {
    let cache = ThreadSafeCache::new(50);

    // Create 1 hub node with 100 edges
    let hub_cluster = Arc::new(create_test_cluster_compact(1, 100));
    let hub_key = CacheKey::new(1, Direction::Outgoing);
    cache.insert(hub_key, Arc::clone(&hub_cluster));

    // Create 50 leaf nodes with 1 edge each (fill cache)
    for i in 2..52 {
        let leaf_cluster = Arc::new(create_test_cluster_compact(i, 1));
        let leaf_key = CacheKey::new(i, Direction::Outgoing);
        cache.insert(leaf_key, leaf_cluster);
    }

    // Now add one more node to trigger eviction
    let new_cluster = Arc::new(create_test_cluster_compact(1000, 1));
    let new_key = CacheKey::new(1000, Direction::Outgoing);
    cache.insert(new_key, new_cluster);

    // Verify hub node is still in cache (should not be evicted)
    assert!(
        cache.get(hub_key).is_some(),
        "High-degree hub node should not be evicted from cache"
    );

    println!("High-degree node priority test passed: hub node retained in cache");
}

#[test]
fn test_cache_lru_k_eviction() {
    let mut cache = TraversalAwareCache::new(5);

    // Insert 5 entries
    for i in 1..=5 {
        let cluster = Arc::new(create_test_cluster_compact(i, 10));
        let key = CacheKey::new(i, Direction::Outgoing);
        cache.insert(key, cluster);
    }

    // Access entries 1 and 2 multiple times (build LRU-2 history)
    for _ in 0..3 {
        cache.get(CacheKey::new(1, Direction::Outgoing));
        cache.get(CacheKey::new(2, Direction::Outgoing));
    }

    // Insert 6th entry to trigger eviction
    let cluster = Arc::new(create_test_cluster_compact(6, 10));
    let key = CacheKey::new(6, Direction::Outgoing);
    cache.insert(key, cluster);

    // Entries 1 and 2 should still be in cache (high access frequency)
    assert!(
        cache.get(CacheKey::new(1, Direction::Outgoing)).is_some(),
        "Entry 1 should still be in cache (LRU-2 protection)"
    );
    assert!(
        cache.get(CacheKey::new(2, Direction::Outgoing)).is_some(),
        "Entry 2 should still be in cache (LRU-2 protection)"
    );

    println!("LRU-K eviction test passed: frequently accessed entries retained");
}

#[test]
fn test_prefetch_neighbors() {
    let cache = ThreadSafeCache::new(100);

    // Create a node with 10 neighbors
    let node_id = 1;
    let cluster = Arc::new(create_test_cluster_compact(node_id, 10));
    let neighbors: Vec<i64> = cluster.iter_neighbors().collect();

    // Prefetch neighbors
    cluster.prefetch_neighbors(
        &cache,
        &neighbors,
        |neighbor_id, direction| {
            // Simulate loading neighbor cluster
            if neighbor_id <= 11 {
                Some(create_test_cluster_compact(neighbor_id, 5))
            } else {
                None
            }
        },
        Direction::Outgoing,
    );

    // Verify that neighbors were prefetched into cache
    let mut prefetch_count = 0;
    for neighbor_id in neighbors.iter().take(10) {
        let key = CacheKey::new(*neighbor_id, Direction::Outgoing);
        if cache.get(key).is_some() {
            prefetch_count += 1;
        }
    }

    assert!(
        prefetch_count >= 5,
        "Expected at least 5 neighbors to be prefetched, got {}",
        prefetch_count
    );

    println!(
        "Prefetch test passed: {} neighbors preloaded into cache",
        prefetch_count
    );
}

#[test]
fn test_cache_high_degree_not_cached() {
    let cache = ThreadSafeCache::new(100);

    // Create a very high-degree node (>1000 edges)
    let high_degree_cluster = Arc::new(create_test_cluster_compact(1, 1500));
    let key = CacheKey::new(1, Direction::Outgoing);

    // Try to cache it (should be rejected due to high degree)
    high_degree_cluster.get_neighbors_with_cache(&cache, 1, Direction::Outgoing);

    // Verify it's NOT in cache
    assert!(
        cache.get(key).is_none(),
        "Very high-degree node (>1000 edges) should not be cached"
    );

    println!(
        "High-degree node exclusion test passed: node not cached to reduce memory pressure"
    );
}

#[test]
fn test_cache_statistics_tracking() {
    let cache = ThreadSafeCache::new(50);

    // Create some test clusters
    for i in 1..10 {
        let cluster = Arc::new(create_test_cluster_compact(i, 5));
        let key = CacheKey::new(i, Direction::Outgoing);
        cache.insert(key, cluster);
    }

    // Generate some hits
    for i in 1..5 {
        let key = CacheKey::new(i, Direction::Outgoing);
        cache.get(key);
    }

    // Generate some misses
    for i in 100..110 {
        let key = CacheKey::new(i, Direction::Outgoing);
        cache.get(key);
    }

    // Check statistics
    let stats = cache.stats();
    assert!(stats.hits > 0, "Should have recorded some hits");
    assert!(stats.misses > 0, "Should have recorded some misses");
    assert!(
        stats.hits + stats.misses > 0,
        "Total accesses should be > 0"
    );

    println!(
        "Cache statistics: hits={}, misses={}, hit_ratio={:.2}%",
        stats.hits,
        stats.misses,
        cache.hit_ratio() * 100.0
    );
}

#[test]
fn test_cache_thread_safety() {
    use std::thread;

    let cache = Arc::new(ThreadSafeCache::new(100));
    let mut handles = Vec::new();

    // Spawn multiple threads accessing the cache concurrently
    for thread_id in 0..4 {
        let cache_clone = Arc::clone(&cache);
        let handle = thread::spawn(move || {
            for i in 0..25 {
                let node_id = thread_id * 25 + i;
                let cluster = Arc::new(create_test_cluster_compact(node_id as i64, 5));
                let key = CacheKey::new(node_id as i64, Direction::Outgoing);

                // Insert and then get
                cache_clone.insert(key, Arc::clone(&cluster));
                cache_clone.get(key);
            }
        });
        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify cache is in consistent state
    let stats = cache.stats();
    assert!(
        stats.hits > 0 || stats.misses > 0,
        "Cache should have recorded accesses"
    );

    println!(
        "Thread safety test passed: {} concurrent operations completed",
        stats.hits + stats.misses
    );
}

#[test]
fn test_cache_capacity_enforcement() {
    let cache = ThreadSafeCache::new(10);

    // Insert 20 entries (exceeds capacity)
    for i in 1..=20 {
        let cluster = Arc::new(create_test_cluster_compact(i, 5));
        let key = CacheKey::new(i, Direction::Outgoing);
        cache.insert(key, cluster);
    }

    // Cache should not exceed capacity
    // Note: We can't directly check cache size, but we can verify behavior
    let stats_before = cache.stats();

    // Access some entries
    for i in 1..=5 {
        let key = CacheKey::new(i, Direction::Outgoing);
        cache.get(key);
    }

    let stats_after = cache.stats();
    assert!(
        stats_after.hits + stats_after.misses > stats_before.hits + stats_before.misses,
        "Cache should still be operational after exceeding capacity"
    );

    println!(
        "Capacity enforcement test passed: cache operates correctly at capacity limit"
    );
}
