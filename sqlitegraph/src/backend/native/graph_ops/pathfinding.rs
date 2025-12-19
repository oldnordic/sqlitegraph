//! Shortest path algorithms using BFS

use crate::backend::native::adjacency::AdjacencyHelpers;
use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::types::*;
use crate::backend::native::NativeBackendError;

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