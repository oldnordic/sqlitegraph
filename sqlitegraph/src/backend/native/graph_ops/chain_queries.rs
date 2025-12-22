//! Chain traversal and pattern matching operations

use crate::backend::native::NativeBackendError;
use crate::backend::native::adjacency::{AdjacencyHelpers, Direction};
use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::types::*;
use crate::backend::{BackendDirection, ChainStep, PatternMatch, PatternQuery};

/// Native chain query implementation
pub fn native_chain_query(
    graph_file: &mut GraphFile,
    start: NativeNodeId,
    chain: &[ChainStep],
) -> Result<Vec<NativeNodeId>, NativeBackendError> {
    let mut current_nodes = vec![start];
    let mut result = current_nodes.clone();

    for step in chain {
        let mut next_nodes = Vec::new();
        let direction = match step.direction {
            BackendDirection::Outgoing => Direction::Outgoing,
            BackendDirection::Incoming => Direction::Incoming,
        };

        for &node in &current_nodes {
            let neighbors = if let Some(edge_type) = &step.edge_type {
                let edge_type_ref = edge_type.as_str();
                match direction {
                    Direction::Outgoing => AdjacencyHelpers::get_outgoing_neighbors_filtered(
                        graph_file,
                        node,
                        &[edge_type_ref],
                    )?,
                    Direction::Incoming => AdjacencyHelpers::get_incoming_neighbors_filtered(
                        graph_file,
                        node,
                        &[edge_type_ref],
                    )?,
                }
            } else {
                match direction {
                    Direction::Outgoing => {
                        AdjacencyHelpers::get_outgoing_neighbors(graph_file, node)?
                    }
                    Direction::Incoming => {
                        AdjacencyHelpers::get_incoming_neighbors(graph_file, node)?
                    }
                }
            };

            next_nodes.extend(neighbors);
        }

        if next_nodes.is_empty() {
            return Ok(vec![]); // Chain broken
        }

        current_nodes = next_nodes;
        result.extend(current_nodes.clone());
    }

    Ok(result)
}

/// Native pattern search implementation (basic version)
pub fn native_pattern_search(
    _graph_file: &mut GraphFile,
    _start: NativeNodeId,
    _pattern: &PatternQuery,
) -> Result<Vec<PatternMatch>, NativeBackendError> {
    // This is a simplified implementation
    // In a full implementation, this would use the pattern engine
    // For now, return empty matches as the pattern engine is complex
    Ok(vec![])
}
