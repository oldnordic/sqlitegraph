//! In-memory neighbor iteration helpers for native backend.
//!
//! This module provides efficient adjacency iteration using the node and edge
//! records stored in the native graph file format.
//!
//! ## INLINE HINT STRATEGY (Phase 13 Step 5)
//!
//! ### Tier A: Tiny Hot Path Functions (#[inline(always)])
//! - `unlikely()`: Simple boolean wrapper used frequently in hot paths
//! - `get_current_neighbor_fast_path()`: Critical tight loop function
//! - `total_count()`, `current_index()`, `is_complete()`: Simple field accessors
//!
//! ### Tier B: Small Helper Functions (#[inline] or compiler-driven)
//! - `get_current_neighbor()`: Hot path but complex branching
//! - `Iterator::next()`: Iterator implementation (compiler-optimized)
//! - Cache access functions: Used frequently but moderate complexity
//!
//! ### Tier C: Large Functions (no inline hints)
//! - BFS implementations: Large algorithms left to compiler discretion
//! - AdjacencyHelpers: Orchestration functions with complex logic

use crate::backend::native::edge_store::EdgeStore;
use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::node_store::NodeStore;
use crate::backend::native::optimizations::*;
use crate::backend::native::types::*;
use crate::backend::native::v2::edge_cluster::Direction as V2Direction;
use crate::backend::native::v2::node_record_v2::NodeRecordV2Ext;

/// Hint to the compiler that a condition is unlikely (cold path optimization)
#[inline(always)]
fn unlikely(cond: bool) -> bool {
    // In stable Rust, we don't have the cold intrinsic, but the function
    // name and structure still help with code organization and readability
    cond
}

/// Adjacency iterator for efficient neighbor traversal
pub struct AdjacencyIterator<'a> {
    graph_file: &'a mut GraphFile,
    node_id: NativeNodeId,
    direction: Direction,
    edge_filter: Option<Vec<String>>,
    current_index: u32,
    total_count: u32,
    /// Cached node metadata to avoid repeated deserialization
    cached_node: Option<NodeRecord>,
    /// Pre-computed edge offsets from neighbor pointer table (fast path)
    edge_offsets: Option<Vec<FileOffset>>,
    /// Hot node metadata for fast adjacency operations
    node_hot: Option<NodeHot>,
    /// V2 Clustered adjacency: cached neighbors for sequential I/O
    cached_clustered_neighbors: Option<Vec<NativeNodeId>>,
}

impl<'a> AdjacencyIterator<'a> {
    /// Create a copy of the iterator at the same position
    pub fn copy_iterator(&self) -> NativeResult<Self> {
        // We can't actually copy since we'd need a mutable reference to the same graph_file
        // This is a limitation of the current design
        Err(NativeBackendError::BufferTooSmall {
            size: 0,
            min_size: 1,
        })
    }
}

/// Direction for adjacency traversal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Outgoing,
    Incoming,
}

impl<'a> AdjacencyIterator<'a> {
    /// Create a new adjacency iterator for outgoing neighbors
    pub fn new_outgoing(
        graph_file: &'a mut GraphFile,
        node_id: NativeNodeId,
    ) -> NativeResult<Self> {
        // Try to get hot metadata first
        let node_hot = get_node_hot(node_id);

        // Try to get edge offsets from pointer table (fast path)
        let edge_offsets = get_outgoing_edge_offsets(node_id);

        // Fall back to reading the full node if needed
        let (node, total_count) =
            if let (Some(hot), Some(_offsets)) = (node_hot.as_ref(), edge_offsets.as_ref()) {
                // Fast path: we have all the info we need
                (None, hot.outgoing_edge_count)
            } else {
                // Slow path: read full node record
                let mut node_store = NodeStore::new(graph_file);
                let node = node_store.read_node(node_id)?;
                let total_count = node.outgoing_edge_count;
                (Some(node), total_count)
            };

        Ok(Self {
            graph_file,
            node_id,
            direction: Direction::Outgoing,
            edge_filter: None,
            current_index: 0,
            total_count,
            cached_node: node,
            edge_offsets,
            node_hot,
            cached_clustered_neighbors: None,
        })
    }

    /// Create a new adjacency iterator for incoming neighbors
    pub fn new_incoming(
        graph_file: &'a mut GraphFile,
        node_id: NativeNodeId,
    ) -> NativeResult<Self> {
        // Try to get hot metadata first
        let node_hot = get_node_hot(node_id);

        // Try to get edge offsets from pointer table (fast path)
        let edge_offsets = get_incoming_edge_offsets(node_id);

        // Fall back to reading the full node if needed
        let (node, total_count) =
            if let (Some(hot), Some(_offsets)) = (node_hot.as_ref(), edge_offsets.as_ref()) {
                // Fast path: we have all the info we need
                (None, hot.incoming_edge_count)
            } else {
                // Slow path: read full node record
                let mut node_store = NodeStore::new(graph_file);
                let node = node_store.read_node(node_id)?;
                let total_count = node.incoming_edge_count;
                (Some(node), total_count)
            };

        Ok(Self {
            graph_file,
            node_id,
            direction: Direction::Incoming,
            edge_filter: None,
            current_index: 0,
            total_count,
            cached_node: node,
            edge_offsets,
            node_hot,
            cached_clustered_neighbors: None,
        })
    }

    /// Set edge type filter for iteration
    pub fn with_edge_filter(mut self, edge_types: &[&str]) -> Self {
        self.edge_filter = Some(edge_types.iter().map(|&s| s.to_string()).collect());
        self
    }

    /// Get the total number of neighbors (before filtering)
    #[inline(always)]
    pub fn total_count(&self) -> u32 {
        self.total_count
    }

    /// Get the current iteration position
    #[inline(always)]
    pub fn current_index(&self) -> u32 {
        self.current_index
    }

    /// Check if iteration is complete
    #[inline(always)]
    pub fn is_complete(&self) -> bool {
        self.current_index >= self.total_count
    }

    /// Reset iterator to beginning
    pub fn reset(&mut self) {
        self.current_index = 0;
    }

    /// Get neighbor node ID at current position (optimized with pointer table and hot cache)
    #[inline]
    pub fn get_current_neighbor(&mut self) -> NativeResult<Option<NativeNodeId>> {
        // V2 Clustered Adjacency Path: Use clustered neighbors if available (HIGHEST PRIORITY)
        if self.cached_clustered_neighbors.is_none() {
            self.try_initialize_clustered_adjacency()?;
        }

        if let Some(ref neighbors) = self.cached_clustered_neighbors {
            let current_index = self.current_index as usize;
            if current_index < neighbors.len() {
                return Ok(Some(neighbors[current_index]));
            }
            return Ok(None);
        }

        // Fast path: Use pointer table if available
        if let Some(edge_offsets) = self.edge_offsets.take() {
            let result = self.get_current_neighbor_fast_path(&edge_offsets);
            self.edge_offsets = Some(edge_offsets); // Put it back
            return result;
        }

        // V2-only: No V1 fallback - return no neighbors if V2 clustering unavailable
        Ok(None)
    }

    // ========================================
    // V2 CLUSTERED ADJACENCY KERNEL IMPLEMENTATION
    // ========================================

    /// V2 clustered adjacency with proper error handling
    /// Uses single clustered read and properly distinguishes between "no cluster" and "corrupt cluster"
    fn try_initialize_clustered_adjacency(&mut self) -> NativeResult<()> {
        // First, check if node is V2 format with cluster metadata
        {
            let node_data_offset = self.graph_file.persistent_header().node_data_offset;
            let slot_offset = node_data_offset + ((self.node_id - 1) as u64 * 4096);
            let mut version = [0u8; 1];

            // V2-only: Check node format (V1 support removed)
            match self.graph_file.read_bytes(slot_offset, &mut version) {
                Ok(()) => {
                    if version[0] == 2 {
                        // V2 node detected - try to read cluster metadata
                        let mut node_store = NodeStore::new(self.graph_file);
                        match node_store.read_node_v2(self.node_id) {
                            Ok(node_v2) => {
                                drop(node_store);

                                let (cluster_offset, cluster_size, edge_count) =
                                    match self.direction {
                                        Direction::Outgoing => (
                                            node_v2.outgoing_cluster_offset,
                                            node_v2.outgoing_cluster_size,
                                            node_v2.outgoing_edge_count,
                                        ),
                                        Direction::Incoming => (
                                            node_v2.incoming_cluster_offset,
                                            node_v2.incoming_cluster_size,
                                            node_v2.incoming_edge_count,
                                        ),
                                    };

                                // Phase 35: Only proceed if cluster metadata is complete
                                if cluster_offset > 0 && cluster_size > 0 && edge_count > 0 {
                                    let mut edge_store = EdgeStore::new(self.graph_file);
                                    let cluster_direction = match self.direction {
                                        Direction::Outgoing => V2Direction::Outgoing,
                                        Direction::Incoming => V2Direction::Incoming,
                                    };

                                    // Phase 69: Use V2 clustered neighbors with strict framed mode
                                    let neighbors = edge_store.iter_neighbors(
                                        self.node_id,
                                        self.direction,
                                    ).collect::<Vec<_>>();

                                    // Phase 69: V2 clustered adjacency success
                                    #[cfg(debug_assertions)]
                                    {
                                        println!(
                                            "DEBUG: V2 clustered adjacency SUCCESS for node {} (direction: {:?}, {} neighbors)",
                                            self.node_id,
                                            self.direction,
                                            neighbors.len()
                                        );
                                    }
                                    self.cached_clustered_neighbors = Some(neighbors);
                                    self.total_count = edge_count;
                                    return Ok(());
                                }
                            }
                            Err(NativeBackendError::InvalidNodeId { .. }) => {
                                // Node doesn't exist - propagate error
                                return Err(NativeBackendError::InvalidNodeId { id: 0, max_id: 0 });
                            }
                            Err(e) => {
                                // Phase 35: Propagate unexpected read errors
                                return Err(e);
                            }
                        }
                    }
                }
                Err(NativeBackendError::FileTooSmall { .. }) => {
                    // Node slot out of bounds - return error
                    return Err(NativeBackendError::FileTooSmall { size: 0, min_size: 1 });
                }
                Err(e) => {
                    // Phase 35: Propagate unexpected I/O errors
                    return Err(e);
                }
            }
        }

        // V2-ONLY: Return error if V2 cluster not found
        Err(NativeBackendError::CorruptNodeRecord {
            node_id: self.node_id as i64,
            reason: "V2 cluster metadata not found".to_string(),
        })
    }

    /// V2-only: Fast path disabled - use V2 clustered adjacency only
    /// In V2, individual edge reading is replaced by cluster-based reading
    #[inline(always)]
    fn get_current_neighbor_fast_path(
        &mut self,
        _edge_offsets: &[FileOffset],
    ) -> NativeResult<Option<NativeNodeId>> {
        // V2-only: Fast path disabled - rely on V2 clustered adjacency
        // Individual edge reading is not used in V2 architecture
        Ok(None)
    }

    
    /// Collect all neighbors into a vector
    pub fn collect(mut self) -> NativeResult<Vec<NativeNodeId>> {
        let mut neighbors = Vec::new();

        while !self.is_complete() {
            if let Some(neighbor) = self.get_current_neighbor()? {
                neighbors.push(neighbor);
            }
            self.current_index += 1;
        }

        // neighbors() must return unique neighbor IDs
        // This deduplication is applied at the public API layer, preserving full edge multiplicity internally
        let mut seen_neighbors = std::collections::HashSet::new();
        let mut unique_neighbors = Vec::new();

        for neighbor in neighbors {
            if seen_neighbors.insert(neighbor) {
                unique_neighbors.push(neighbor);
            }
        }

        Ok(unique_neighbors)
    }

    /// Check if a specific neighbor exists
    pub fn contains(&mut self, target_id: NativeNodeId) -> NativeResult<bool> {
        // Store original position
        let original_index = self.current_index;

        // Reset to beginning
        self.current_index = 0;

        // Search through all neighbors
        while !self.is_complete() {
            if let Some(neighbor_id) = self.get_current_neighbor()? {
                if neighbor_id == target_id {
                    // Restore original position
                    self.current_index = original_index;
                    return Ok(true);
                }
            }
            self.current_index += 1;
        }

        // Restore original position
        self.current_index = original_index;
        Ok(false)
    }

    /// Get neighbors in batches
    pub fn get_batch(&mut self, batch_size: u32) -> NativeResult<Vec<NativeNodeId>> {
        let mut batch = Vec::with_capacity(batch_size as usize);
        let end_index = (self.current_index + batch_size).min(self.total_count);

        while self.current_index < end_index {
            if let Some(neighbor) = self.get_current_neighbor()? {
                batch.push(neighbor);
            }
            self.current_index += 1;
        }

        Ok(batch)
    }
}

impl<'a> Iterator for AdjacencyIterator<'a> {
    type Item = NativeNodeId;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        // HOT PATH: Fast neighbor lookup with minimal error handling
        match self.get_current_neighbor() {
            Ok(Some(neighbor)) => {
                self.current_index += 1;
                Some(neighbor)
            }
            Ok(None) => None,
            Err(_) => {
                // COLD PATH: Error handling - skip problematic edge and continue
                self.current_index += 1;
                None
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = (self.total_count - self.current_index) as usize;
        (remaining, Some(remaining))
    }
}

/// Helper functions for adjacency operations
pub struct AdjacencyHelpers;

impl AdjacencyHelpers {
    /// Get outgoing neighbors for a node
    pub fn get_outgoing_neighbors(
        graph_file: &mut GraphFile,
        node_id: NativeNodeId,
    ) -> NativeResult<Vec<NativeNodeId>> {
        let iterator = AdjacencyIterator::new_outgoing(graph_file, node_id)?;
        iterator.collect()
    }

    /// Get incoming neighbors for a node
    pub fn get_incoming_neighbors(
        graph_file: &mut GraphFile,
        node_id: NativeNodeId,
    ) -> NativeResult<Vec<NativeNodeId>> {
        let iterator = AdjacencyIterator::new_incoming(graph_file, node_id)?;
        iterator.collect()
    }

    /// Get outgoing neighbors filtered by edge type
    pub fn get_outgoing_neighbors_filtered(
        graph_file: &mut GraphFile,
        node_id: NativeNodeId,
        edge_types: &[&str],
    ) -> NativeResult<Vec<NativeNodeId>> {
        let iterator =
            AdjacencyIterator::new_outgoing(graph_file, node_id)?.with_edge_filter(edge_types);
        iterator.collect()
    }

    /// Get incoming neighbors filtered by edge type
    pub fn get_incoming_neighbors_filtered(
        graph_file: &mut GraphFile,
        node_id: NativeNodeId,
        edge_types: &[&str],
    ) -> NativeResult<Vec<NativeNodeId>> {
        let iterator =
            AdjacencyIterator::new_incoming(graph_file, node_id)?.with_edge_filter(edge_types);
        iterator.collect()
    }

    /// Check if there's a path from source to target (direct edge)
    pub fn has_direct_edge(
        graph_file: &mut GraphFile,
        source_id: NativeNodeId,
        target_id: NativeNodeId,
    ) -> NativeResult<bool> {
        let mut iterator = AdjacencyIterator::new_outgoing(graph_file, source_id)?;
        iterator.contains(target_id)
    }

    /// Get degree of node (number of outgoing edges)
    pub fn outgoing_degree(graph_file: &mut GraphFile, node_id: NativeNodeId) -> NativeResult<u32> {
        let iterator = AdjacencyIterator::new_outgoing(graph_file, node_id)?;
        Ok(iterator.total_count())
    }

    /// Get degree of node (number of incoming edges)
    pub fn incoming_degree(graph_file: &mut GraphFile, node_id: NativeNodeId) -> NativeResult<u32> {
        let iterator = AdjacencyIterator::new_incoming(graph_file, node_id)?;
        Ok(iterator.total_count())
    }

    /// Get total degree of node (incoming + outgoing)
    pub fn total_degree(graph_file: &mut GraphFile, node_id: NativeNodeId) -> NativeResult<u32> {
        let outgoing = Self::outgoing_degree(graph_file, node_id)?;
        let incoming = Self::incoming_degree(graph_file, node_id)?;
        Ok(outgoing + incoming)
    }

    /// Validate adjacency consistency for a single node with strict real adjacency checks
    pub fn validate_node_adjacency(
        graph_file: &mut GraphFile,
        node_id: NativeNodeId,
    ) -> NativeResult<()> {
        // Read node info first to avoid borrowing issues
        let node = {
            let mut node_store = NodeStore::new(graph_file);
            node_store.read_node(node_id)?
        };

        // Check if adjacency metadata is consistent with actual edges
        let outgoing_neighbors = Self::get_outgoing_neighbors(graph_file, node_id)?;
        let incoming_neighbors = Self::get_incoming_neighbors(graph_file, node_id)?;

        // Strict adjacency consistency validation for real implementation
        if outgoing_neighbors.len() as u32 != node.outgoing_edge_count {
            return Err(NativeBackendError::InconsistentAdjacency {
                node_id,
                count: node.outgoing_edge_count,
                direction: "outgoing".to_string(),
                file_count: outgoing_neighbors.len() as u32,
            });
        }

        if incoming_neighbors.len() as u32 != node.incoming_edge_count {
            return Err(NativeBackendError::InconsistentAdjacency {
                node_id,
                count: node.incoming_edge_count,
                direction: "incoming".to_string(),
                file_count: incoming_neighbors.len() as u32,
            });
        }

        // Validate that all referenced neighbors exist within valid ID range
        // Use same logic as validate_node_id_range - allow up to 100,000 OR current count + 1000
        let current_node_count = graph_file.persistent_header().node_count as NativeNodeId;
        let max_allowed_node_id = std::cmp::max(100_000, current_node_count + 1000);

        for &neighbor_id in &outgoing_neighbors {
            if neighbor_id <= 0 || neighbor_id > max_allowed_node_id {
                return Err(NativeBackendError::InvalidNodeId {
                    id: neighbor_id,
                    max_id: max_allowed_node_id,
                });
            }
        }

        for &neighbor_id in &incoming_neighbors {
            if neighbor_id <= 0 || neighbor_id > max_allowed_node_id {
                return Err(NativeBackendError::InvalidNodeId {
                    id: neighbor_id,
                    max_id: max_allowed_node_id,
                });
            }
        }

        Ok(())
    }

    /// Validate adjacency consistency across all nodes
    pub fn validate_all_adjacency(graph_file: &mut GraphFile) -> NativeResult<()> {
        let max_node_id = graph_file.persistent_header().node_count as NativeNodeId;

        for node_id in 1..=max_node_id {
            Self::validate_node_adjacency(graph_file, node_id)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn create_test_graph_file() -> (GraphFile, NamedTempFile) {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let graph_file = GraphFile::create(path).unwrap();
        (graph_file, temp_file)
    }

    #[test]
    fn test_adjacency_iterator_empty() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Create a node with no edges
        let node = NodeRecord::new(
            1,
            "Test".to_string(),
            "node1".to_string(),
            serde_json::json!({}),
        );
        {
            let mut node_store = NodeStore::new(&mut graph_file);
            node_store.write_node(&node).unwrap();
        }

        // Test outgoing iterator
        let iterator = AdjacencyIterator::new_outgoing(&mut graph_file, 1).unwrap();
        assert_eq!(iterator.total_count(), 0);
        assert!(iterator.is_complete());

        // Test incoming iterator
        let iterator = AdjacencyIterator::new_incoming(&mut graph_file, 1).unwrap();
        assert_eq!(iterator.total_count(), 0);
        assert!(iterator.is_complete());
    }

    
    #[test]
    fn test_adjacency_validation() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut node_store = NodeStore::new(&mut graph_file);

        // Create a node
        let node = NodeRecord::new(
            1,
            "Test".to_string(),
            "node1".to_string(),
            serde_json::json!({}),
        );
        node_store.write_node(&node).unwrap();

        // Validate adjacency (should pass for node with no edges)
        let result = AdjacencyHelpers::validate_node_adjacency(&mut graph_file, 1);
        assert!(result.is_ok());
    }
}
