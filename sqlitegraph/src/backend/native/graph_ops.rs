//! Core operations and algorithms for native graph backend.

use super::adjacency::{AdjacencyHelpers, Direction};
use super::graph_file::GraphFile;
use super::types::*;
use crate::backend::{BackendDirection, ChainStep, PatternMatch, PatternQuery};

/// Native BFS implementation using adjacency helpers
pub fn native_bfs(
    graph_file: &mut GraphFile,
    start: NativeNodeId,
    depth: u32,
) -> Result<Vec<NativeNodeId>, NativeBackendError> {
    if depth == 0 {
        return Ok(vec![start]);
    }

    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    let mut result = Vec::new();

    visited.insert(start);
    queue.push_back((start, 0));

    while let Some((current_node, current_depth)) = queue.pop_front() {
        if current_depth >= depth {
            continue;
        }

        let neighbors = AdjacencyHelpers::get_outgoing_neighbors(graph_file, current_node)?;
        for neighbor in neighbors {
            if !visited.contains(&neighbor) {
                visited.insert(neighbor);
                result.push(neighbor);
                queue.push_back((neighbor, current_depth + 1));
            }
        }
    }

    Ok(result)
}

/// Native shortest path implementation using BFS
pub fn native_shortest_path(
    graph_file: &mut GraphFile,
    start: NativeNodeId,
    end: NativeNodeId,
) -> Result<Option<Vec<NativeNodeId>>, NativeBackendError> {
    if start == end {
        return Ok(Some(vec![start]));
    }

    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    let mut parent: std::collections::HashMap<NativeNodeId, NativeNodeId> =
        std::collections::HashMap::new();

    visited.insert(start);
    queue.push_back(start);

    while let Some(current_node) = queue.pop_front() {
        if current_node == end {
            // Reconstruct path
            let mut path = vec![end];
            let mut current = end;

            while let Some(&p) = parent.get(&current) {
                path.push(p);
                current = p;
            }

            path.reverse();
            return Ok(Some(path));
        }

        let neighbors = AdjacencyHelpers::get_outgoing_neighbors(graph_file, current_node)?;
        for neighbor in neighbors {
            if !visited.contains(&neighbor) {
                visited.insert(neighbor);
                parent.insert(neighbor, current_node);
                queue.push_back(neighbor);
            }
        }
    }

    Ok(None)
}

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
    graph_file: &mut GraphFile,
    start: NativeNodeId,
    pattern: &PatternQuery,
) -> Result<Vec<PatternMatch>, NativeBackendError> {
    // This is a simplified implementation
    // In a full implementation, this would use the pattern engine
    // For now, return empty matches as the pattern engine is complex
    Ok(vec![])
}

#[cfg(test)]
mod tests {
    use super::super::{EdgeStore, NodeStore};
    use super::*;
    use crate::backend::{EdgeSpec, NodeSpec};
    use tempfile::NamedTempFile;

    fn create_test_graph_file() -> (GraphFile, NamedTempFile) {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let graph_file = GraphFile::create(path).unwrap();
        (graph_file, temp_file)
    }

    #[test]
    fn test_native_bfs_simple() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Create nodes
        let node1 = NodeRecord::new(
            1,
            "Test".to_string(),
            "node1".to_string(),
            serde_json::json!({}),
        );
        let node2 = NodeRecord::new(
            2,
            "Test".to_string(),
            "node2".to_string(),
            serde_json::json!({}),
        );
        let node3 = NodeRecord::new(
            3,
            "Test".to_string(),
            "node3".to_string(),
            serde_json::json!({}),
        );

        {
            let mut node_store = NodeStore::new(&mut graph_file);
            node_store.write_node(&node1).unwrap();
            node_store.write_node(&node2).unwrap();
            node_store.write_node(&node3).unwrap();
        }

        // Create edges: 1 -> 2 -> 3
        let edge1 = EdgeRecord::new(1, 1, 2, "test".to_string(), serde_json::json!({}));
        let edge2 = EdgeRecord::new(2, 2, 3, "test".to_string(), serde_json::json!({}));

        {
            let mut edge_store = EdgeStore::new(&mut graph_file);
            edge_store.write_edge(&edge1).unwrap();
            edge_store.write_edge(&edge2).unwrap();
        }

        let result = native_bfs(&mut graph_file, 1, 2).unwrap();
        assert!(result.contains(&2));
        assert!(result.contains(&3));
    }

    #[test]
    fn test_native_shortest_path() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Create nodes
        let node1 = NodeRecord::new(
            1,
            "Test".to_string(),
            "node1".to_string(),
            serde_json::json!({}),
        );
        let node2 = NodeRecord::new(
            2,
            "Test".to_string(),
            "node2".to_string(),
            serde_json::json!({}),
        );
        let node3 = NodeRecord::new(
            3,
            "Test".to_string(),
            "node3".to_string(),
            serde_json::json!({}),
        );

        {
            let mut node_store = NodeStore::new(&mut graph_file);
            node_store.write_node(&node1).unwrap();
            node_store.write_node(&node2).unwrap();
            node_store.write_node(&node3).unwrap();
        }

        // Create edge: 1 -> 2 -> 3
        let edge1 = EdgeRecord::new(1, 1, 2, "test".to_string(), serde_json::json!({}));
        let edge2 = EdgeRecord::new(2, 2, 3, "test".to_string(), serde_json::json!({}));

        {
            let mut edge_store = EdgeStore::new(&mut graph_file);
            edge_store.write_edge(&edge1).unwrap();
            edge_store.write_edge(&edge2).unwrap();
        }

        let result = native_shortest_path(&mut graph_file, 1, 3).unwrap();
        assert!(result.is_some());
        let path = result.unwrap();
        assert_eq!(path, vec![1, 2, 3]);
    }
}
