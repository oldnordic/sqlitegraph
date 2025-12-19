//! Helper functions for adjacency operations

use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::node_store::NodeStore;
use crate::backend::native::types::*;
use super::AdjacencyIterator;

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