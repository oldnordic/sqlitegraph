//! Edge store modular components
//!
//! This module contains the modularized components of the edge store system,
//! broken down into focused, single-responsibility modules for better
//! maintainability and testing.

// Re-export core types from the parent module
pub use crate::backend::native::types::{EdgeRecord, NativeEdgeId, EdgeFlags};

// Import modular components
mod utils;
mod cluster_utils;
mod record_operations;
pub mod id_management;
pub mod capacity_coordinator;

// Re-export utility functions
pub use utils::check_for_overlap;
pub use cluster_utils::{
    calculate_neighbor_offset_in_cluster,
    calculate_edge_data_offset_in_cluster,
    validate_cluster_size,
    calculate_optimal_cluster_size,
};

// Re-export record operations
pub use record_operations::EdgeRecordOperations;

/// Edge store manages edge records and adjacency layout in the graph file
///
/// This implementation delegates to modularized components for clean separation of concerns
/// while preserving the original API signature for compatibility.
pub struct EdgeStore<'a> {
    graph_file: &'a mut crate::backend::native::graph_file::GraphFile,
}

impl<'a> EdgeStore<'a> {
    /// Create a new edge store
    pub fn new(graph_file: &'a mut crate::backend::native::graph_file::GraphFile) -> Self {
        Self { graph_file }
    }

    /// Write an edge record to the store
    pub fn write_edge(&mut self, edge: &crate::backend::native::types::EdgeRecord) -> crate::backend::native::types::NativeResult<()> {
        let mut operations = record_operations::EdgeRecordOperations::new(self.graph_file);
        operations.write_edge(edge)
    }

    /// Read an edge record from the store
    pub fn read_edge(&mut self, edge_id: crate::backend::native::types::NativeEdgeId) -> crate::backend::native::types::NativeResult<crate::backend::native::types::EdgeRecord> {
        let mut operations = record_operations::EdgeRecordOperations::new(self.graph_file);
        operations.read_edge(edge_id)
    }

    /// Get the maximum edge ID
    pub fn max_edge_id(&mut self) -> crate::backend::native::types::NativeEdgeId {
        let id_manager = id_management::EdgeIdManager::new(self.graph_file);
        id_manager.max_edge_id()
    }

    /// Allocate a new edge ID with capacity coordination
    pub fn allocate_edge_id(&mut self) -> crate::backend::native::types::NativeEdgeId {
        let mut coordinator = capacity_coordinator::EdgeCapacityCoordinator::new(self.graph_file);
        coordinator.allocate_edge_id_with_capacity()
            .expect("Failed to allocate edge ID with capacity")
    }

    /// Iterate over neighbors of a node
    pub fn iter_neighbors(&mut self, _node_id: crate::backend::native::types::NativeNodeId, _direction: crate::backend::native::adjacency::Direction) -> Box<dyn Iterator<Item = crate::backend::native::types::NativeEdgeId> + '_> {
        // TODO: This needs the neighbor iteration module which hasn't been extracted yet
        // For now, return empty iterator to maintain compilation
        Box::new(std::iter::empty())
    }

    /// Allocate adjacency space for outgoing edges
    pub fn allocate_outgoing_adjacency(&mut self, node_id: crate::backend::native::types::NativeNodeId, count: u32) -> crate::backend::native::types::NativeResult<crate::backend::native::types::FileOffset> {
        let mut allocator = id_management::AdjacencyAllocator::new(self.graph_file);
        allocator.allocate_outgoing_adjacency(node_id, count)
    }

    /// Allocate adjacency space for incoming edges
    pub fn allocate_incoming_adjacency(&mut self, node_id: crate::backend::native::types::NativeNodeId, count: u32) -> crate::backend::native::types::NativeResult<crate::backend::native::types::FileOffset> {
        let mut allocator = id_management::AdjacencyAllocator::new(self.graph_file);
        allocator.allocate_incoming_adjacency(node_id, count)
    }
}