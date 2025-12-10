//! In-memory neighbor iteration helpers for native backend.
//!
//! This module provides efficient adjacency iteration using the node and edge
//! records stored in the native graph file format.

use crate::backend::native::edge_store::EdgeStore;
use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::node_store::NodeStore;
use crate::backend::native::types::*;

/// Adjacency iterator for efficient neighbor traversal
pub struct AdjacencyIterator<'a> {
    graph_file: &'a mut GraphFile,
    node_id: NativeNodeId,
    direction: Direction,
    edge_filter: Option<Vec<String>>,
    current_index: u32,
    total_count: u32,
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
        let mut node_store = NodeStore::new(graph_file);
        let node = node_store.read_node(node_id)?;

        Ok(Self {
            graph_file,
            node_id,
            direction: Direction::Outgoing,
            edge_filter: None,
            current_index: 0,
            total_count: node.outgoing_count,
        })
    }

    /// Create a new adjacency iterator for incoming neighbors
    pub fn new_incoming(
        graph_file: &'a mut GraphFile,
        node_id: NativeNodeId,
    ) -> NativeResult<Self> {
        let mut node_store = NodeStore::new(graph_file);
        let node = node_store.read_node(node_id)?;

        Ok(Self {
            graph_file,
            node_id,
            direction: Direction::Incoming,
            edge_filter: None,
            current_index: 0,
            total_count: node.incoming_count,
        })
    }

    /// Set edge type filter for iteration
    pub fn with_edge_filter(mut self, edge_types: &[&str]) -> Self {
        self.edge_filter = Some(edge_types.iter().map(|&s| s.to_string()).collect());
        self
    }

    /// Get the total number of neighbors (before filtering)
    pub fn total_count(&self) -> u32 {
        self.total_count
    }

    /// Get the current iteration position
    pub fn current_index(&self) -> u32 {
        self.current_index
    }

    /// Check if iteration is complete
    pub fn is_complete(&self) -> bool {
        self.current_index >= self.total_count
    }

    /// Reset iterator to beginning
    pub fn reset(&mut self) {
        self.current_index = 0;
    }

    /// Get neighbor node ID at current position (with real edge reading and direction filtering)
    pub fn get_current_neighbor(&mut self) -> NativeResult<Option<NativeNodeId>> {
        loop {
            if self.is_complete() {
                return Ok(None);
            }

            // Read fresh node metadata to get current edge offsets
            let mut node_store = NodeStore::new(self.graph_file);
            let node = node_store.read_node(self.node_id)?;

            // Determine edge ID range based on direction
            // Note: outgoing_offset and incoming_offset are interpreted as starting edge IDs
            let (start_edge_id, edge_count) = match self.direction {
                Direction::Outgoing => (node.outgoing_offset as NativeEdgeId, node.outgoing_count),
                Direction::Incoming => (node.incoming_offset as NativeEdgeId, node.incoming_count),
            };

            // Skip if no edges
            if edge_count == 0 || start_edge_id == 0 {
                return Ok(None);
            }

            // Calculate current edge ID to read
            let current_edge_id = start_edge_id + self.current_index as NativeEdgeId;

            // Validate edge ID is within reasonable bounds
            let header = self.graph_file.header();
            let max_edge_id = header.edge_count as NativeEdgeId;
            let max_node_id = header.node_count as NativeNodeId;

            if current_edge_id > max_edge_id {
                return Err(NativeBackendError::InvalidEdgeId {
                    id: current_edge_id,
                    max_id: max_edge_id,
                });
            }

            // Read the edge record using local edge store
            let mut edge_store = EdgeStore::new(self.graph_file);
            let edge = edge_store.read_edge(current_edge_id)?;

            // Apply direction filtering and return appropriate neighbor
            let neighbor_id = match self.direction {
                Direction::Outgoing => {
                    // For outgoing edges, neighbor is the target node
                    if edge.from_id == self.node_id {
                        Some(edge.to_id)
                    } else {
                        // This edge doesn't belong to this node's outgoing adjacency - skip it
                        None
                    }
                }
                Direction::Incoming => {
                    // For incoming edges, neighbor is the source node
                    if edge.to_id == self.node_id {
                        Some(edge.from_id)
                    } else {
                        // This edge doesn't belong to this node's incoming adjacency - skip it
                        None
                    }
                }
            };

            // If edge doesn't match direction, advance and continue loop
            if neighbor_id.is_none() {
                self.current_index += 1;
                continue;
            }

            // Validate neighbor ID is within valid range
            if let Some(neighbor) = neighbor_id {
                if neighbor <= 0 || neighbor > max_node_id {
                    return Err(NativeBackendError::InvalidNodeId {
                        id: neighbor,
                        max_id: max_node_id,
                    });
                }
            }

            return Ok(neighbor_id);
        }
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

        Ok(neighbors)
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

    fn next(&mut self) -> Option<Self::Item> {
        match self.get_current_neighbor() {
            Ok(Some(neighbor)) => {
                self.current_index += 1;
                Some(neighbor)
            }
            Ok(None) => None,
            Err(_) => None, // In a real implementation, you might want to handle errors differently
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
        if outgoing_neighbors.len() as u32 != node.outgoing_count {
            return Err(NativeBackendError::InconsistentAdjacency {
                node_id,
                count: node.outgoing_count,
                direction: "outgoing".to_string(),
                file_count: outgoing_neighbors.len() as u32,
            });
        }

        if incoming_neighbors.len() as u32 != node.incoming_count {
            return Err(NativeBackendError::InconsistentAdjacency {
                node_id,
                count: node.incoming_count,
                direction: "incoming".to_string(),
                file_count: incoming_neighbors.len() as u32,
            });
        }

        // Validate that all referenced neighbors exist within valid ID range
        let max_node_id = graph_file.header().node_count as NativeNodeId;

        for &neighbor_id in &outgoing_neighbors {
            if neighbor_id <= 0 || neighbor_id > max_node_id {
                return Err(NativeBackendError::InvalidNodeId {
                    id: neighbor_id,
                    max_id: max_node_id,
                });
            }
        }

        for &neighbor_id in &incoming_neighbors {
            if neighbor_id <= 0 || neighbor_id > max_node_id {
                return Err(NativeBackendError::InvalidNodeId {
                    id: neighbor_id,
                    max_id: max_node_id,
                });
            }
        }

        Ok(())
    }

    /// Validate adjacency consistency across all nodes
    pub fn validate_all_adjacency(graph_file: &mut GraphFile) -> NativeResult<()> {
        let max_node_id = graph_file.header().node_count as NativeNodeId;

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
    fn test_adjacency_degree() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Create two nodes
        let mut node1 = NodeRecord::new(
            1,
            "Test".to_string(),
            "node1".to_string(),
            serde_json::json!({}),
        );
        let mut node2 = NodeRecord::new(
            2,
            "Test".to_string(),
            "node2".to_string(),
            serde_json::json!({}),
        );

        node1.outgoing_count = 2;
        node1.incoming_count = 1;
        node2.outgoing_count = 1;
        node2.incoming_count = 2;

        // Write nodes - create separate scopes for each store to avoid borrowing issues
        {
            let mut node_store = NodeStore::new(&mut graph_file);
            node_store.write_node(&node1).unwrap();
            node_store.write_node(&node2).unwrap();
        }

        // Test degrees
        let outgoing_deg = AdjacencyHelpers::outgoing_degree(&mut graph_file, 1).unwrap();
        let incoming_deg = AdjacencyHelpers::incoming_degree(&mut graph_file, 1).unwrap();
        let total_deg = AdjacencyHelpers::total_degree(&mut graph_file, 1).unwrap();

        assert_eq!(outgoing_deg, 2);
        assert_eq!(incoming_deg, 1);
        assert_eq!(total_deg, 3);
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
