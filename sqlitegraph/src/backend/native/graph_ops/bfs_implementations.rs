//! Multiple BFS implementations with CPU-specific optimizations

use crate::backend::native::NativeBackendError;
use crate::backend::native::adjacency::AdjacencyHelpers;
use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::optimizations;
use crate::backend::native::types::*;

/// Generic scalar BFS implementation (baseline for all CPUs/large graphs)
pub fn bfs_generic_scalar(
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

/// Optimized BFS with pointer table (medium graphs, SIMD-capable CPUs)
pub fn bfs_pointer_table_optimized(
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

        // Use pointer table for fast adjacency lookup
        let neighbors =
            if let Some(offsets) = optimizations::get_outgoing_edge_offsets(current_node) {
                // Fast path: use pointer table to avoid edge scanning
                let mut neighbor_ids = Vec::with_capacity(offsets.len());
                for &offset in &offsets {
                    if let Ok(edge_record) = graph_file.read_edge_at_offset(offset) {
                        neighbor_ids.push(edge_record.to_id);
                    }
                }
                neighbor_ids
            } else {
                // Fallback to standard adjacency lookup
                AdjacencyHelpers::get_outgoing_neighbors(graph_file, current_node)?
            };

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

/// Fully optimized BFS with pointer table and hot cache (small graphs, high-end CPUs)
pub fn bfs_fully_optimized(
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

        // Use both pointer table and hot cache for maximum performance
        let neighbors =
            if let Some(offsets) = optimizations::get_outgoing_edge_offsets(current_node) {
                let mut neighbor_ids = Vec::with_capacity(offsets.len());

                // Check hot cache for node metadata first
                if let Some(_hot_metadata) = optimizations::get_node_hot(current_node) {
                    // Hot cache hit - use optimized path
                    for &offset in &offsets {
                        if let Ok(edge_record) = graph_file.read_edge_at_offset(offset) {
                            neighbor_ids.push(edge_record.to_id);
                        }
                    }
                } else {
                    // Cold cache path - still use pointer table but extract hot metadata
                    for &offset in &offsets {
                        if let Ok(edge_record) = graph_file.read_edge_at_offset(offset) {
                            neighbor_ids.push(edge_record.to_id);
                        }
                    }

                    // Extract and cache hot metadata for future use
                    if let Ok(node_record) = graph_file.read_node_at(current_node) {
                        let hot_metadata = optimizations::extract_node_hot(&node_record);
                        optimizations::put_node_hot(current_node, hot_metadata);
                    }
                }

                neighbor_ids
            } else {
                // Fallback to standard adjacency lookup
                AdjacencyHelpers::get_outgoing_neighbors(graph_file, current_node)?
            };

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
