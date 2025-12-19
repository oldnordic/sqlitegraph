//! K-hop neighbor exploration operations

use crate::backend::native::adjacency::{AdjacencyHelpers, Direction};
use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::types::*;
use crate::backend::native::NativeBackendError;

/// Native k-hop implementation
pub fn native_k_hop(
    graph_file: &mut GraphFile,
    start: NativeNodeId,
    depth: u32,
    direction: Direction,
) -> Result<Vec<NativeNodeId>, NativeBackendError> {
    if depth == 0 {
        return Ok(vec![start]);
    }

    let mut visited = std::collections::HashSet::new();
    let mut current_level = vec![start];
    visited.insert(start);
    let mut result = Vec::new();

    for _ in 0..depth {
        let mut next_level = Vec::new();

        for node in current_level {
            let neighbors = match direction {
                Direction::Outgoing => AdjacencyHelpers::get_outgoing_neighbors(graph_file, node)?,
                Direction::Incoming => AdjacencyHelpers::get_incoming_neighbors(graph_file, node)?,
            };

            for neighbor in neighbors {
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor);
                    next_level.push(neighbor);
                    result.push(neighbor);
                }
            }
        }

        current_level = next_level;
        if current_level.is_empty() {
            break;
        }
    }

    Ok(result)
}

/// Native k-hop implementation with edge type filtering
pub fn native_k_hop_filtered(
    graph_file: &mut GraphFile,
    start: NativeNodeId,
    depth: u32,
    direction: Direction,
    allowed_edge_types: &[&str],
) -> Result<Vec<NativeNodeId>, NativeBackendError> {
    if depth == 0 {
        return Ok(vec![start]);
    }

    let mut visited = std::collections::HashSet::new();
    let mut current_level = vec![start];
    visited.insert(start);
    let mut result = Vec::new();

    for _ in 0..depth {
        let mut next_level = Vec::new();

        for node in current_level {
            let neighbors = match direction {
                Direction::Outgoing => AdjacencyHelpers::get_outgoing_neighbors_filtered(
                    graph_file,
                    node,
                    allowed_edge_types,
                )?,
                Direction::Incoming => AdjacencyHelpers::get_incoming_neighbors_filtered(
                    graph_file,
                    node,
                    allowed_edge_types,
                )?,
            };

            for neighbor in neighbors {
                if !visited.contains(&neighbor) {
                    visited.insert(neighbor);
                    next_level.push(neighbor);
                    result.push(neighbor);
                }
            }
        }

        current_level = next_level;
        if current_level.is_empty() {
            break;
        }
    }

    Ok(result)
}