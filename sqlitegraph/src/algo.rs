use std::collections::VecDeque;

use ahash::{AHashMap, AHashSet};

use crate::{errors::SqliteGraphError, graph::SqliteGraph};

pub fn connected_components(graph: &SqliteGraph) -> Result<Vec<Vec<i64>>, SqliteGraphError> {
    let mut components = Vec::new();
    let mut visited = AHashSet::new();
    for id in graph.all_entity_ids()? {
        if !visited.insert(id) {
            continue;
        }
        let mut queue = VecDeque::new();
        queue.push_back(id);
        let mut component = Vec::new();
        while let Some(node) = queue.pop_front() {
            component.push(node);
            for next in graph.fetch_outgoing(node)? {
                if visited.insert(next) {
                    queue.push_back(next);
                }
            }
            for prev in graph.fetch_incoming(node)? {
                if visited.insert(prev) {
                    queue.push_back(prev);
                }
            }
        }
        component.sort();
        components.push(component);
    }
    components.sort_by(|a, b| a[0].cmp(&b[0]));
    Ok(components)
}

pub fn find_cycles_limited(
    graph: &SqliteGraph,
    limit: usize,
) -> Result<Vec<Vec<i64>>, SqliteGraphError> {
    if limit == 0 {
        return Ok(Vec::new());
    }
    let all_ids = graph.all_entity_ids()?;
    let max_len = all_ids.len();
    let mut cycles = Vec::new();
    for &start in &all_ids {
        let mut stack = vec![(start, vec![start])];
        while let Some((node, path)) = stack.pop() {
            for next in graph.fetch_outgoing(node)? {
                if next == start && path.len() > 1 {
                    let mut cycle = path.clone();
                    cycle.push(start);
                    cycles.push(cycle);
                    if cycles.len() >= limit {
                        normalize_cycles(&mut cycles);
                        return Ok(cycles);
                    }
                    continue;
                }
                if path.contains(&next) {
                    continue;
                }
                let mut new_path = path.clone();
                new_path.push(next);
                if new_path.len() > max_len {
                    continue;
                }
                stack.push((next, new_path));
            }
        }
    }
    normalize_cycles(&mut cycles);
    Ok(cycles)
}

pub fn nodes_by_degree(
    graph: &SqliteGraph,
    descending: bool,
) -> Result<Vec<(i64, usize)>, SqliteGraphError> {
    let mut degrees = Vec::new();
    for id in graph.all_entity_ids()? {
        let outgoing = graph.fetch_outgoing(id)?.len();
        let incoming = graph.fetch_incoming(id)?.len();
        degrees.push((id, outgoing + incoming));
    }
    degrees.sort_by(|a, b| {
        if descending {
            b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0))
        } else {
            a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0))
        }
    });
    Ok(degrees)
}

fn normalize_cycles(cycles: &mut [Vec<i64>]) {
    for cycle in cycles.iter_mut() {
        // rotate so smallest node first for determinism
        if let Some((min_idx, _)) = cycle.iter().enumerate().min_by_key(|(_, value)| *value) {
            cycle.rotate_left(min_idx);
        }
    }
    cycles.sort();
}

/// Label Propagation algorithm for community detection.
///
/// Each node starts with its own label, then iteratively adopts the most frequent
/// label among its neighbors. Converges when no labels change or max_iterations reached.
///
/// # Arguments
/// * `graph` - The graph to analyze
/// * `max_iterations` - Maximum number of iterations to prevent infinite loops
///
/// # Returns
/// Communities as vectors of node IDs, sorted by smallest node ID in each community
pub fn label_propagation(
    graph: &SqliteGraph,
    max_iterations: usize,
) -> Result<Vec<Vec<i64>>, SqliteGraphError> {
    let all_ids = graph.all_entity_ids()?;

    if all_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Initialize: each node gets its own label
    let mut labels: AHashMap<i64, i64> = all_ids.iter().map(|&id| (id, id)).collect();

    // For deterministic results, process nodes in sorted order
    let mut node_order: Vec<i64> = all_ids.clone();
    node_order.sort();

    // Iterative label propagation
    for _iteration in 0..max_iterations {
        let mut any_changed = false;

        for &node in &node_order {
            // Count neighbor labels
            let mut label_counts: AHashMap<i64, usize> = AHashMap::new();

            // Count outgoing neighbors
            for &neighbor in &graph.fetch_outgoing(node)? {
                let neighbor_label = labels.get(&neighbor).unwrap_or(&neighbor);
                *label_counts.entry(*neighbor_label).or_insert(0) += 1;
            }

            // Count incoming neighbors
            for &neighbor in &graph.fetch_incoming(node)? {
                let neighbor_label = labels.get(&neighbor).unwrap_or(&neighbor);
                *label_counts.entry(*neighbor_label).or_insert(0) += 1;
            }

            // Find most frequent label (deterministic tiebreak: smallest label)
            if let Some((&most_frequent_label, _)) = label_counts
                .iter()
                .max_by_key(|(_, &count)| count)
                .map(|(label, count)| (label, *count))
            {
                // In case of ties, max_by_key returns arbitrary one
                // So we need to find all with max count and take smallest label
                let max_count = *label_counts.values().max().unwrap_or(&0);
                let best_label = label_counts
                    .iter()
                    .filter(|(_, &count)| count == max_count)
                    .map(|(&label, _)| label)
                    .min()
                    .unwrap_or(node);

                if let Some(current_label) = labels.get(&node) {
                    if *current_label != best_label {
                        labels.insert(node, best_label);
                        any_changed = true;
                    }
                }
            }
        }

        if !any_changed {
            break;
        }
    }

    // Group nodes by final label
    let mut communities_map: AHashMap<i64, Vec<i64>> = AHashMap::new();
    for (node, label) in &labels {
        communities_map
            .entry(*label)
            .or_insert_with(Vec::new)
            .push(*node);
    }

    // Convert to sorted vector of communities
    let mut communities: Vec<Vec<i64>> = communities_map.into_values().collect();
    for community in &mut communities {
        community.sort();
    }
    communities.sort_by(|a, b| a.first().cmp(&b.first()));

    Ok(communities)
}
