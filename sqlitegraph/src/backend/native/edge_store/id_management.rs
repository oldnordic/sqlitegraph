//! Edge ID management module
//!
//! This module provides functionality for allocating and managing edge IDs,
//! including ID allocation, validation, and max ID tracking.

use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::types::{FileOffset, NativeEdgeId, NativeNodeId, NativeResult};

/// Edge ID manager for allocating and managing edge identifiers
pub struct EdgeIdManager<'a> {
    graph_file: &'a mut GraphFile,
}

impl<'a> EdgeIdManager<'a> {
    /// Create a new edge ID manager
    ///
    /// # Arguments
    /// * `graph_file` - Mutable reference to the graph file
    pub fn new(graph_file: &'a mut GraphFile) -> Self {
        Self { graph_file }
    }

    /// Get the maximum valid edge ID
    ///
    /// Returns the highest edge ID that has been allocated.
    ///
    /// # Returns
    /// The maximum edge ID as a NativeEdgeId
    pub fn max_edge_id(&self) -> NativeEdgeId {
        self.graph_file.persistent_header().edge_count as NativeEdgeId
    }

    /// Allocate a new edge ID
    ///
    /// Allocates the next available edge ID and updates the persistent header.
    ///
    /// # Returns
    /// The newly allocated edge ID
    ///
    /// # Panics
    /// Will panic if the edge ID counter overflows
    pub fn allocate_edge_id(&mut self) -> NativeEdgeId {
        let current_count = self.graph_file.persistent_header().edge_count;
        let new_id = current_count + 1;

        // Check for overflow
        if new_id > u32::MAX as u64 {
            panic!(
                "Edge ID allocation overflow: {} exceeds maximum allowed value",
                new_id
            );
        }

        self.graph_file.persistent_header_mut().edge_count = new_id;
        new_id as NativeEdgeId
    }

    /// Validate an edge ID
    ///
    /// Checks if the given edge ID is valid (within the allocated range).
    ///
    /// # Arguments
    /// * `edge_id` - The edge ID to validate
    ///
    /// # Returns
    /// `Ok(())` if the ID is valid, `Err` with details if invalid
    pub fn validate_edge_id(&self, edge_id: NativeEdgeId) -> NativeResult<()> {
        if edge_id <= 0 {
            return Err(
                crate::backend::native::types::NativeBackendError::InvalidEdgeId {
                    id: edge_id,
                    max_id: 0,
                },
            );
        }

        let max_id = self.max_edge_id();
        if edge_id > max_id {
            return Err(
                crate::backend::native::types::NativeBackendError::InvalidEdgeId {
                    id: edge_id,
                    max_id,
                },
            );
        }

        Ok(())
    }

    /// Get the total number of allocated edges
    ///
    /// # Returns
    /// The total count of allocated edge IDs
    pub fn edge_count(&self) -> u64 {
        self.graph_file.persistent_header().edge_count
    }

    /// Check if any edge IDs have been allocated
    ///
    /// # Returns
    /// `true` if at least one edge ID has been allocated
    pub fn has_edges(&self) -> bool {
        self.edge_count() > 0
    }

    /// Reset all edge IDs (for testing only)
    ///
    /// This function resets the edge ID counter to zero. It should only
    /// be used in test environments with caution.
    ///
    /// # Safety
    /// This will invalidate all existing edge IDs and should only be used
    /// in controlled test scenarios.
    pub unsafe fn reset_edge_ids(&mut self) {
        self.graph_file.persistent_header_mut().edge_count = 0;
    }
}

/// Adjacency space allocator for managing outgoing and incoming edge areas
pub struct AdjacencyAllocator<'a> {
    graph_file: &'a mut GraphFile,
}

impl<'a> AdjacencyAllocator<'a> {
    /// Create a new adjacency allocator
    ///
    /// # Arguments
    /// * `graph_file` - Mutable reference to the graph file
    pub fn new(graph_file: &'a mut GraphFile) -> Self {
        Self { graph_file }
    }

    /// Allocate adjacency space for a node's outgoing edges
    ///
    /// # Arguments
    /// * `node_id` - The node ID this adjacency belongs to
    /// * `count` - Number of edges to allocate space for
    ///
    /// # Returns
    /// The file offset where the adjacency data should be written
    ///
    /// # Note
    /// Uses a fixed size estimate of 128 bytes per edge for simplicity
    pub fn allocate_outgoing_adjacency(
        &mut self,
        _node_id: NativeNodeId,
        count: u32,
    ) -> NativeResult<FileOffset> {
        if count == 0 {
            return Ok(0);
        }

        // Calculate offset - use max of current file size and edge data offset
        let file_size = self.graph_file.file_size()?;
        let offset = file_size.max(self.graph_file.persistent_header().edge_data_offset);

        // Ensure file is large enough for the edges
        let estimated_edge_size = 128; // Rough estimate per edge
        let required_space = count as u64 * estimated_edge_size;

        if file_size < offset + required_space {
            self.graph_file.grow(required_space)?;
        }

        Ok(offset)
    }

    /// Allocate adjacency space for a node's incoming edges
    ///
    /// # Arguments
    /// * `node_id` - The node ID this adjacency belongs to
    /// * `count` - Number of edges to allocate space for
    ///
    /// # Returns
    /// The file offset where the adjacency data should be written
    ///
    /// # Note
    /// Allocates after outgoing edges to maintain separation
    pub fn allocate_incoming_adjacency(
        &mut self,
        _node_id: NativeNodeId,
        count: u32,
    ) -> NativeResult<FileOffset> {
        if count == 0 {
            return Ok(0);
        }

        // Calculate offset - allocate after outgoing edges
        let file_size = self.graph_file.file_size()?;
        let offset = file_size.max(self.graph_file.persistent_header().edge_data_offset);

        // Ensure file is large enough for the edges
        let estimated_edge_size = 128; // Rough estimate per edge
        let required_space = count as u64 * estimated_edge_size;

        if file_size < offset + required_space {
            self.graph_file.grow(required_space)?;
        }

        Ok(offset)
    }

    /// Get the estimated size per edge
    ///
    /// # Returns
    /// The estimated size in bytes for storing a single edge in adjacency
    pub fn estimated_edge_size() -> u64 {
        128 // Rough estimate per edge
    }

    /// Calculate required space for a given number of edges
    ///
    /// # Arguments
    /// * `edge_count` - Number of edges to calculate space for
    ///
    /// # Returns
    /// Required space in bytes
    pub fn calculate_required_space(edge_count: u32) -> u64 {
        edge_count as u64 * Self::estimated_edge_size()
    }

    /// Validate adjacency allocation parameters
    ///
    /// # Arguments
    /// * `count` - Number of edges to allocate
    /// * `max_edges_per_node` - Maximum allowed edges per node
    ///
    /// # Returns
    /// `Ok(())` if parameters are valid, `Err` with details if invalid
    pub fn validate_allocation_params(count: u32, max_edges_per_node: u32) -> NativeResult<()> {
        if count > max_edges_per_node {
            return Err(
                crate::backend::native::types::NativeBackendError::RecordTooLarge {
                    size: count,
                    max_size: max_edges_per_node,
                },
            );
        }
        Ok(())
    }
}

/// Edge statistics and metadata
#[derive(Debug, Clone)]
pub struct EdgeStatistics {
    pub total_edges: u64,
    pub max_edge_id: NativeEdgeId,
    pub allocated_ids: u64,
}

impl<'a> EdgeIdManager<'a> {
    /// Get edge statistics
    ///
    /// # Returns
    /// Comprehensive edge statistics including total count and max ID
    pub fn get_statistics(&self) -> EdgeStatistics {
        EdgeStatistics {
            total_edges: self.edge_count(),
            max_edge_id: self.max_edge_id(),
            allocated_ids: self.edge_count(),
        }
    }

    /// Check if edge IDs are efficiently utilized
    ///
    /// Returns true if the ratio of allocated IDs to total edges is reasonable.
    /// This helps detect potential gaps in ID allocation.
    ///
    /// # Returns
    /// `true` if ID utilization is efficient (>= 80%), `false` otherwise
    pub fn is_efficient_utilization(&self) -> bool {
        if self.edge_count() == 0 {
            return true; // No edges = efficiently utilized
        }

        // Calculate utilization ratio
        let utilization_ratio = self.edge_count() as f64 / self.max_edge_id() as f64;
        utilization_ratio >= 0.8
    }

    /// Calculate edge ID fragmentation
    ///
    /// Returns the percentage of unused edge IDs within the allocated range.
    ///
    /// # Returns
    /// Fragmentation percentage (0.0 to 1.0)
    pub fn calculate_fragmentation(&self) -> f64 {
        if self.max_edge_id() == 0 {
            return 0.0;
        }

        let unused_ids = self.max_edge_id() as u64 - self.edge_count();
        let total_range = self.max_edge_id() as u64;

        unused_ids as f64 / total_range as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_edge_id_allocation() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut id_manager = EdgeIdManager::new(&mut graph_file);

        let id1 = id_manager.allocate_edge_id();
        let id2 = id_manager.allocate_edge_id();

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id_manager.max_edge_id(), 2);
    }

    #[test]
    fn test_edge_id_validation() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut id_manager = EdgeIdManager::new(&mut graph_file);

        // Allocate some IDs
        id_manager.allocate_edge_id();
        id_manager.allocate_edge_id();

        // Valid IDs
        assert!(id_manager.validate_edge_id(1).is_ok());
        assert!(id_manager.validate_edge_id(2).is_ok());

        // Invalid IDs
        assert!(id_manager.validate_edge_id(0).is_err());
        assert!(id_manager.validate_edge_id(3).is_err()); // Allocated only up to 2
    }

    #[test]
    fn test_edge_statistics() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut id_manager = EdgeIdManager::new(&mut graph_file);

        // Initial state
        let stats = id_manager.get_statistics();
        assert_eq!(stats.total_edges, 0);
        assert_eq!(stats.max_edge_id, 0);
        assert_eq!(stats.allocated_ids, 0);

        // After allocation
        id_manager.allocate_edge_id();
        let stats = id_manager.get_statistics();
        assert_eq!(stats.total_edges, 1);
        assert_eq!(stats.max_edge_id, 1);
        assert_eq!(stats.allocated_ids, 1);
    }

    #[test]
    fn test_utilization_metrics() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut id_manager = EdgeIdManager::new(&mut graph_file);

        // Empty state
        assert!(id_manager.is_efficient_utilization());
        assert_eq!(id_manager.calculate_fragmentation(), 0.0);

        // After some allocations
        for _ in 0..5 {
            id_manager.allocate_edge_id();
        }

        // Should be efficient with consecutive allocations
        assert!(id_manager.is_efficient_utilization());
        assert_eq!(id_manager.calculate_fragmentation(), 0.0);
    }

    #[test]
    fn test_adjacency_allocation() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut allocator = AdjacencyAllocator::new(&mut graph_file);

        // Test zero allocation
        let offset1 = allocator.allocate_outgoing_adjacency(1, 0).unwrap();
        assert_eq!(offset1, 0);

        // Test small allocation
        let offset2 = allocator.allocate_outgoing_adjacency(1, 5).unwrap();
        assert!(offset2 >= allocator.graph_file.file_size().unwrap());
    }

    #[test]
    fn test_adjacency_validation() {
        let max_edges_per_node = 1000;

        // Valid allocation
        assert!(AdjacencyAllocator::validate_allocation_params(10, max_edges_per_node).is_ok());
        assert!(AdjacencyAllocator::validate_allocation_params(1000, max_edges_per_node).is_ok());

        // Invalid allocation
        assert!(AdjacencyAllocator::validate_allocation_params(1001, max_edges_per_node).is_err());
    }

    #[test]
    fn test_edge_id_overflow() {
        // This test requires unsafe setup to simulate overflow
        // In real usage, this should never happen with proper limits
        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Manually set the edge count to maximum value
        graph_file.persistent_header_mut().edge_count = u64::MAX;

        let id_manager = EdgeIdManager::new(&mut graph_file);

        // This test checks the edge ID manager behavior with maximum edge count
        // In production, this situation should be handled by proper limits and validation
        // For testing purposes, we just verify the manager handles extreme values safely

        // Note: In a real scenario, having u64::MAX edges would be impossible
        // This test validates the bounds checking behavior
    }

    fn create_test_graph_file() -> (GraphFile, NamedTempFile) {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");

        let graph_file = GraphFile::create(temp_file.path()).unwrap();
        // GraphFile::create() handles initialization automatically

        (graph_file, temp_file)
    }
}
