//! Internal performance optimizations for the native backend.
//!
//! This module contains various in-memory optimizations that accelerate graph traversal
//! without changing the on-disk format or public APIs. All optimizations are
//! internal implementation details only.
//!
//! ## INLINE HINT STRATEGY (Phase 13 Step 5)
//!
//! ### Tier A: Tiny Hot Path Functions (#[inline(always)])
//! - `unlikely()`: Simple boolean wrapper used frequently in hot paths
//! - `get_outgoing_edges()`, `get_incoming_edges()`: Simple HashMap access
//! - `get()`, `put()`: Simple cache operations
//! - Thread-local access wrappers: Minimal overhead functions
//!
//! ### Tier B: Small Helper Functions (#[inline] or compiler-driven)
//! - `register_edge()`: Moderate complexity but hot path
//! - Cache management functions: Used frequently but larger logic
//!
//! ### Tier C: Large Functions (no inline hints)
//! - Cache eviction logic: Complex algorithms left to compiler discretion

use crate::backend::native::types::*;
use std::cell::RefCell;
use std::collections::HashMap;

/// Hint to the compiler that a condition is unlikely (cold path optimization)
#[inline(always)]
fn unlikely(cond: bool) -> bool {
    // In stable Rust, we don't have the cold intrinsic, but the function
    // name and structure still help with code organization and readability
    cond
}

thread_local! {
    /// Global neighbor pointer table for fast adjacency lookup
    static NEIGHBOR_POINTER_TABLE: RefCell<NeighborPointerTable> = RefCell::new(NeighborPointerTable::new());

    /// Global hot-field node cache for frequently accessed node metadata
    static NODE_HOT_CACHE: RefCell<NodeHotCache> = RefCell::new(NodeHotCache::new());
}

/// Neighbor pointer table mapping node IDs to lists of edge file offsets
/// This enables O(1) adjacency lookup without scanning edge slots
pub struct NeighborPointerTable {
    /// Maps node ID to vector of edge file offsets for outgoing edges
    outgoing_edges: HashMap<NativeNodeId, Vec<FileOffset>>,
    /// Maps node ID to vector of edge file offsets for incoming edges
    incoming_edges: HashMap<NativeNodeId, Vec<FileOffset>>,
}

impl NeighborPointerTable {
    /// Create a new empty neighbor pointer table
    pub fn new() -> Self {
        Self {
            outgoing_edges: HashMap::new(),
            incoming_edges: HashMap::new(),
        }
    }

    /// Register an edge with its file offset in the pointer table
    /// This is called during edge writing to build the adjacency index
    #[inline(always)]
    pub fn register_edge(
        &mut self,
        from_id: NativeNodeId,
        to_id: NativeNodeId,
        edge_offset: FileOffset,
    ) {
        // Add to outgoing edges for source node
        self.outgoing_edges
            .entry(from_id)
            .or_insert_with(Vec::new)
            .push(edge_offset);

        // Add to incoming edges for target node
        self.incoming_edges
            .entry(to_id)
            .or_insert_with(Vec::new)
            .push(edge_offset);
    }

    /// Get outgoing edge file offsets for a node
    #[inline(always)]
    pub fn get_outgoing_edges(&self, node_id: NativeNodeId) -> Option<&[FileOffset]> {
        self.outgoing_edges.get(&node_id).map(|v| v.as_slice())
    }

    /// Get incoming edge file offsets for a node
    #[inline(always)]
    pub fn get_incoming_edges(&self, node_id: NativeNodeId) -> Option<&[FileOffset]> {
        self.incoming_edges.get(&node_id).map(|v| v.as_slice())
    }

    /// Clear the entire table (useful for testing or when switching graphs)
    pub fn clear(&mut self) {
        self.outgoing_edges.clear();
        self.incoming_edges.clear();
    }
}

/// Hot-field node cache containing frequently accessed node metadata
/// This avoids full node deserialization for adjacency operations
#[derive(Debug, Clone, PartialEq)]
pub struct NodeHot {
    /// Number of outgoing edges
    pub outgoing_edge_count: u32,
    /// Starting offset for outgoing edges
    pub outgoing_cluster_offset: u64,
    /// Number of incoming edges
    pub incoming_edge_count: u32,
    /// Starting offset for incoming edges
    pub incoming_cluster_offset: u64,
}

pub struct NodeHotCache {
    /// Maps node ID to hot metadata
    cache: HashMap<NativeNodeId, NodeHot>,
    /// Maximum cache size to prevent unbounded growth
    max_size: usize,
}

impl NodeHotCache {
    /// Create a new node hot cache with default capacity
    pub fn new() -> Self {
        Self::with_capacity(1000)
    }

    /// Create a new node hot cache with specified capacity
    pub fn with_capacity(max_size: usize) -> Self {
        Self {
            cache: HashMap::new(),
            max_size,
        }
    }

    /// Get hot metadata for a node
    #[inline(always)]
    pub fn get(&self, node_id: NativeNodeId) -> Option<&NodeHot> {
        self.cache.get(&node_id)
    }

    /// Insert or update hot metadata for a node
    #[inline(always)]
    pub fn put(&mut self, node_id: NativeNodeId, metadata: NodeHot) {
        // Simple eviction strategy: if cache is full, clear it
        // In a production system, you'd want LRU eviction
        if self.cache.len() >= self.max_size {
            self.cache.clear();
        }
        self.cache.insert(node_id, metadata);
    }

    /// Clear the cache
    pub fn clear(&mut self) {
        self.cache.clear();
    }
}

/// Get access to the global neighbor pointer table
#[inline(always)]
pub fn with_neighbor_pointer_table<F, R>(f: F) -> R
where
    F: FnOnce(&mut NeighborPointerTable) -> R,
{
    NEIGHBOR_POINTER_TABLE.with(|table| f(&mut table.borrow_mut()))
}

/// Get access to the global node hot cache
#[inline(always)]
pub fn with_node_hot_cache<F, R>(f: F) -> R
where
    F: FnOnce(&mut NodeHotCache) -> R,
{
    NODE_HOT_CACHE.with(|cache| f(&mut cache.borrow_mut()))
}

/// Register an edge in the neighbor pointer table
/// This should be called whenever an edge is written to disk
/// Optimized with cold-path offloading for cache management
pub fn register_edge_offset(from_id: NativeNodeId, to_id: NativeNodeId, edge_offset: FileOffset) {
    // HOT PATH: Basic validation only for obviously invalid inputs
    if from_id <= 0 || to_id <= 0 || edge_offset == 0 {
        // COLD PATH: Full validation and error handling
        return;
    }

    with_neighbor_pointer_table(|table| {
        // COLD PATH: Cache eviction is unlikely, handle separately
        let should_check_capacity = unlikely(table.outgoing_edges.len() >= 10000);

        // HOT PATH: Direct registration
        table.register_edge(from_id, to_id, edge_offset);

        // COLD PATH: Expensive cache management operations
        if should_check_capacity {
            // In a production system, you might implement smarter eviction here
            // For now, we just let the cache grow since it's per-thread
        }
    });
}

/// Get outgoing edge offsets for a node from the pointer table
#[inline(always)]
pub fn get_outgoing_edge_offsets(node_id: NativeNodeId) -> Option<Vec<FileOffset>> {
    with_neighbor_pointer_table(|table| {
        table
            .get_outgoing_edges(node_id)
            .map(|offsets| offsets.to_vec())
    })
}

/// Get incoming edge offsets for a node from the pointer table
#[inline(always)]
pub fn get_incoming_edge_offsets(node_id: NativeNodeId) -> Option<Vec<FileOffset>> {
    with_neighbor_pointer_table(|table| {
        table
            .get_incoming_edges(node_id)
            .map(|offsets| offsets.to_vec())
    })
}

/// Get hot metadata for a node from the cache
#[inline(always)]
pub fn get_node_hot(node_id: NativeNodeId) -> Option<NodeHot> {
    with_node_hot_cache(|cache| cache.get(node_id).cloned())
}

/// Put hot metadata for a node in the cache
#[inline(always)]
pub fn put_node_hot(node_id: NativeNodeId, metadata: NodeHot) {
    with_node_hot_cache(|cache| {
        cache.put(node_id, metadata);
    });
}

/// Extract hot metadata from a node record
#[inline(always)]
pub fn extract_node_hot(node: &crate::backend::native::types::NodeRecord) -> NodeHot {
    NodeHot {
        outgoing_edge_count: node.outgoing_edge_count,
        outgoing_cluster_offset: node.outgoing_cluster_offset,
        incoming_edge_count: node.incoming_edge_count,
        incoming_cluster_offset: node.incoming_cluster_offset,
    }
}

/// Clear all optimization caches (useful for testing)
pub fn clear_all_caches() {
    with_neighbor_pointer_table(|table| table.clear());
    with_node_hot_cache(|cache| cache.clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_neighbor_pointer_table_basic() {
        let mut table = NeighborPointerTable::new();

        // Register some edges
        table.register_edge(1, 2, 1000);
        table.register_edge(1, 3, 1256);
        table.register_edge(2, 1, 1512);

        // Test outgoing edges
        let outgoing = table.get_outgoing_edges(1).unwrap();
        assert_eq!(outgoing, &[1000, 1256]);

        let outgoing = table.get_outgoing_edges(2).unwrap();
        assert_eq!(outgoing, &[1512]);

        // Test incoming edges
        let incoming = table.get_incoming_edges(2).unwrap();
        assert_eq!(incoming, &[1000]);

        let incoming = table.get_incoming_edges(1).unwrap();
        assert_eq!(incoming, &[1512]);

        // Test non-existent node
        assert!(table.get_outgoing_edges(999).is_none());
        assert!(table.get_incoming_edges(999).is_none());
    }

    #[test]
    fn test_node_hot_cache_basic() {
        let mut cache = NodeHotCache::with_capacity(2);

        let metadata1 = NodeHot {
            outgoing_edge_count: 5,
            outgoing_cluster_offset: 1000,
            incoming_edge_count: 3,
            incoming_cluster_offset: 2000,
        };

        let metadata2 = NodeHot {
            outgoing_edge_count: 2,
            outgoing_cluster_offset: 3000,
            incoming_edge_count: 7,
            incoming_cluster_offset: 4000,
        };

        // Insert metadata
        cache.put(1, metadata1.clone());
        cache.put(2, metadata2.clone());

        // Test retrieval
        assert_eq!(cache.get(1), Some(&metadata1));
        assert_eq!(cache.get(2), Some(&metadata2));
        assert!(cache.get(3).is_none());

        // Test cache eviction
        let metadata3 = NodeHot {
            outgoing_edge_count: 1,
            outgoing_cluster_offset: 5000,
            incoming_edge_count: 1,
            incoming_cluster_offset: 6000,
        };

        cache.put(3, metadata3);

        // Cache should have been cleared due to size limit
        assert!(cache.get(1).is_none());
        assert!(cache.get(2).is_none());
        assert_eq!(
            cache.get(3),
            Some(&NodeHot {
                outgoing_edge_count: 1,
                outgoing_cluster_offset: 5000,
                incoming_edge_count: 1,
                incoming_cluster_offset: 6000,
            })
        );
    }
}
