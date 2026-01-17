//! Traversal-aware LRU-K cache for edge clusters.
//!
//! This module implements an intelligent caching strategy optimized for graph traversal workloads.
//! It uses LRU-K eviction policy (K=2) and prioritizes high-degree nodes and sequential access patterns.

use super::cluster::EdgeCluster;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

/// Cache key combining node ID and direction for precise cluster identification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub node_id: i64,
    pub direction: super::cluster_trace::Direction,
}

impl CacheKey {
    pub fn new(node_id: i64, direction: super::cluster_trace::Direction) -> Self {
        Self { node_id, direction }
    }
}

/// Access pattern detection for distinguishing traversal from lookup workloads.
#[derive(Debug, Clone)]
pub struct AccessPatternTracker {
    /// Recent node access history for pattern detection.
    access_history: Vec<i64>,
    /// Maximum history size for pattern detection.
    max_history: usize,
}

impl AccessPatternTracker {
    pub fn new(max_history: usize) -> Self {
        Self {
            access_history: Vec::with_capacity(max_history),
            max_history,
        }
    }

    /// Record an access and detect if it follows a traversal pattern.
    pub fn record_access(&mut self, node_id: i64) -> AccessType {
        self.access_history.push(node_id);
        if self.access_history.len() > self.max_history {
            self.access_history.remove(0);
        }

        // Detect sequential access (traversal) vs random access (lookup)
        if self.access_history.len() >= 2 {
            let _last = self.access_history[self.access_history.len() - 2];
            // If we're accessing nodes that were recently accessed, it's likely a traversal
            if self.is_traversal_pattern(node_id) {
                return AccessType::Traversal;
            }
        }

        AccessType::Lookup
    }

    /// Check if current access indicates a traversal pattern.
    fn is_traversal_pattern(&self, node_id: i64) -> bool {
        // Simple heuristic: if we're revisiting nodes from recent history,
        // it's likely a BFS/DFS traversal rather than random lookups
        self.access_history.iter().filter(|&&id| id == node_id).count() > 0
    }
}

/// Type of access performed on a cache entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    /// Sequential neighbor access during graph traversal.
    Traversal,
    /// Random node access during point lookups.
    Lookup,
}

/// Cache entry with LRU-K metadata.
#[derive(Debug, Clone)]
pub struct CacheEntry {
    /// The edge cluster data.
    pub data: Arc<EdgeCluster>,
    /// Number of times this entry has been accessed (for LRU-K).
    pub access_count: u32,
    /// Timestamp of last access.
    pub last_access: Instant,
    /// Traversal score increases on sequential accesses (higher = more important).
    pub traversal_score: f64,
    /// Timestamps of last K accesses (for LRU-K).
    access_history: [Option<Instant>; 2],
}

impl CacheEntry {
    pub fn new(data: Arc<EdgeCluster>) -> Self {
        let now = Instant::now();
        Self {
            data,
            access_count: 0,
            last_access: now,
            traversal_score: 0.0,
            access_history: [None, None],
        }
    }

    /// Record an access and update metadata.
    pub fn record_access(&mut self, access_type: AccessType) {
        self.access_count += 1;
        self.last_access = Instant::now();

        // Update access history for LRU-K (K=2)
        self.access_history[1] = self.access_history[0];
        self.access_history[0] = Some(Instant::now());

        // Update traversal score based on access type
        match access_type {
            AccessType::Traversal => {
                // Increase traversal score significantly for sequential accesses
                self.traversal_score += 1.0;
            }
            AccessType::Lookup => {
                // Slight increase for lookups, but prioritize traversal patterns
                self.traversal_score += 0.1;
            }
        }
    }

    /// Calculate combined score for eviction decisions (higher = should stay in cache).
    pub fn eviction_score(&self) -> f64 {
        // Combine traversal score and recency for LRU-K
        let recency_score = if let Some(most_recent) = self.access_history[0] {
            1.0 / (most_recent.elapsed().as_secs_f64() + 1.0)
        } else {
            0.0
        };

        self.traversal_score * 10.0 + recency_score
    }

    /// Check if this is a high-degree node (degree > 100).
    pub fn is_high_degree(&self) -> bool {
        self.data.edge_count() > 100
    }
}

/// Traversal-aware cache with LRU-K eviction policy.
pub struct TraversalAwareCache {
    /// Cache entries stored in a simple HashMap (we'll manually track LRU order).
    entries: HashMap<CacheKey, CacheEntry>,
    /// Access pattern tracker for detecting traversal workloads.
    access_pattern: AccessPatternTracker,
    /// Maximum number of entries to store.
    max_capacity: usize,
    /// Cache hit/miss statistics.
    stats: CacheStats,
}

/// Cache performance statistics.
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub traversals: u64,
    pub lookups: u64,
}

impl TraversalAwareCache {
    /// Create a new traversal-aware cache with specified capacity.
    pub fn new(max_capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(max_capacity),
            access_pattern: AccessPatternTracker::new(100),
            max_capacity,
            stats: CacheStats::default(),
        }
    }

    /// Get a cluster from the cache, recording the access pattern.
    pub fn get(&mut self, key: CacheKey) -> Option<Arc<EdgeCluster>> {
        // Detect access pattern
        let access_type = self.access_pattern.record_access(key.node_id);

        // Update statistics
        match access_type {
            AccessType::Traversal => self.stats.traversals += 1,
            AccessType::Lookup => self.stats.lookups += 1,
        }

        // Try to get from cache
        if let Some(entry) = self.entries.get_mut(&key) {
            self.stats.hits += 1;
            entry.record_access(access_type);
            return Some(Arc::clone(&entry.data));
        }

        self.stats.misses += 1;
        None
    }

    /// Insert a cluster into the cache, evicting if necessary.
    pub fn insert(&mut self, key: CacheKey, cluster: Arc<EdgeCluster>) {
        // If already present, update the entry
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.data = cluster;
            entry.record_access(AccessType::Lookup);
            return;
        }

        // Evict if at capacity
        if self.entries.len() >= self.max_capacity {
            self.evict_one();
        }

        // Insert new entry
        let entry = CacheEntry::new(cluster);
        self.entries.insert(key, entry);
    }

    /// Remove a specific entry from the cache.
    pub fn remove(&mut self, key: &CacheKey) -> Option<Arc<EdgeCluster>> {
        self.entries.remove(key).map(|entry| entry.data)
    }

    /// Evict one entry using LRU-K policy with traversal score consideration.
    fn evict_one(&mut self) {
        if self.entries.is_empty() {
            return;
        }

        // Find entry with lowest eviction score
        let mut worst_key = None;
        let mut worst_score = f64::MAX;

        for (key, entry) in &self.entries {
            let score = entry.eviction_score();

            // Prefer to evict non-high-degree nodes
            let adjusted_score = if entry.is_high_degree() {
                score * 2.0 // Protect high-degree nodes by doubling their score
            } else {
                score
            };

            if adjusted_score < worst_score {
                worst_score = adjusted_score;
                worst_key = Some(*key);
            }
        }

        // Evict the worst entry
        if let Some(key) = worst_key {
            self.entries.remove(&key);
        }
    }

    /// Clear all entries from the cache.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Get current number of entries in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Check if the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get cache statistics.
    pub fn stats(&self) -> &CacheStats {
        &self.stats
    }

    /// Calculate cache hit ratio.
    pub fn hit_ratio(&self) -> f64 {
        let total = self.stats.hits + self.stats.misses;
        if total == 0 {
            0.0
        } else {
            self.stats.hits as f64 / total as f64
        }
    }
}

/// Thread-safe wrapper for TraversalAwareCache.
pub struct ThreadSafeCache {
    inner: Arc<RwLock<TraversalAwareCache>>,
}

impl ThreadSafeCache {
    /// Create a new thread-safe cache.
    pub fn new(max_capacity: usize) -> Self {
        Self {
            inner: Arc::new(RwLock::new(TraversalAwareCache::new(max_capacity))),
        }
    }

    /// Get a cluster from the cache.
    pub fn get(&self, key: CacheKey) -> Option<Arc<EdgeCluster>> {
        self.inner.write().get(key)
    }

    /// Insert a cluster into the cache.
    pub fn insert(&self, key: CacheKey, cluster: Arc<EdgeCluster>) {
        self.inner.write().insert(key, cluster);
    }

    /// Remove a cluster from the cache.
    pub fn remove(&self, key: &CacheKey) -> Option<Arc<EdgeCluster>> {
        self.inner.write().remove(key)
    }

    /// Get cache statistics.
    pub fn stats(&self) -> CacheStats {
        self.inner.read().stats().clone()
    }

    /// Calculate cache hit ratio.
    pub fn hit_ratio(&self) -> f64 {
        self.inner.read().hit_ratio()
    }

    /// Get the inner Arc for cloning.
    pub fn inner(&self) -> &Arc<RwLock<TraversalAwareCache>> {
        &self.inner
    }
}

impl Clone for ThreadSafeCache {
    fn clone(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::v2::edge_cluster::cluster_trace::Direction;

    #[test]
    fn test_cache_basics() {
        let mut cache = TraversalAwareCache::new(3);

        let key1 = CacheKey::new(1, Direction::Outgoing);
        let key2 = CacheKey::new(2, Direction::Outgoing);

        // Insert and retrieve
        let cluster = Arc::new(EdgeCluster::create_from_compact_edges(
            vec![],
            1,
            Direction::Outgoing,
        ).unwrap());

        cache.insert(key1, Arc::clone(&cluster));
        assert!(cache.get(key1).is_some());
        assert!(cache.get(key2).is_none());

        // Test stats
        assert_eq!(cache.stats().hits, 1);
        assert_eq!(cache.stats().misses, 1);
    }

    #[test]
    fn test_cache_eviction() {
        let mut cache = TraversalAwareCache::new(2);

        let key1 = CacheKey::new(1, Direction::Outgoing);
        let key2 = CacheKey::new(2, Direction::Outgoing);
        let key3 = CacheKey::new(3, Direction::Outgoing);

        let cluster = Arc::new(EdgeCluster::create_from_compact_edges(
            vec![],
            1,
            Direction::Outgoing,
        ).unwrap());

        // Fill cache
        cache.insert(key1, Arc::clone(&cluster));
        cache.insert(key2, Arc::clone(&cluster));

        // Insert third entry should trigger eviction
        cache.insert(key3, Arc::clone(&cluster));

        // Cache should still have 2 entries
        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_hit_ratio() {
        let mut cache = TraversalAwareCache::new(10);

        let key1 = CacheKey::new(1, Direction::Outgoing);
        let cluster = Arc::new(EdgeCluster::create_from_compact_edges(
            vec![],
            1,
            Direction::Outgoing,
        ).unwrap());

        cache.insert(key1, Arc::clone(&cluster));

        // 5 hits, 5 misses = 50%
        for _ in 0..5 {
            cache.get(key1);
        }
        for i in 2..7 {
            cache.get(CacheKey::new(i, Direction::Outgoing));
        }

        assert!((cache.hit_ratio() - 0.5).abs() < 0.01);
    }
}
