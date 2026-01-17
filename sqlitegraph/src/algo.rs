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

/// Computes PageRank scores for all nodes in the graph.
///
/// PageRank measures node importance based on link structure. Nodes with many
/// incoming links from important nodes score higher.
///
/// # Arguments
/// * `graph` - The graph to analyze
/// * `damping` - Damping factor (typically 0.85), representing probability of continuing navigation
/// * `iterations` - Number of power iteration iterations (fixed, not convergence-based)
///
/// # Returns
/// Vector of (node_id, score) tuples sorted by score descending
///
/// # Algorithm
/// Uses power iteration method:
/// 1. Initialize all nodes with equal score (1.0 / node_count)
/// 2. For each iteration:
///    - new_score = (1-d)/n + d * sum(incoming_scores / outgoing_count)
///    - Handle dangling nodes (no outgoing edges) by distributing their score equally
/// 3. Sort results by score descending
pub fn pagerank(
    graph: &SqliteGraph,
    damping: f64,
    iterations: usize,
) -> Result<Vec<(i64, f64)>, SqliteGraphError> {
    let all_ids = graph.all_entity_ids()?;
    let n = all_ids.len();

    if n == 0 {
        return Ok(Vec::new());
    }

    // Initialize all nodes with equal score
    let mut scores: AHashMap<i64, f64> = all_ids.iter().map(|&id| (id, 1.0 / n as f64)).collect();

    // Pre-compute outgoing counts for all nodes
    let mut outgoing_counts: AHashMap<i64, usize> = AHashMap::new();
    for &id in &all_ids {
        let count = graph.fetch_outgoing(id)?.len();
        outgoing_counts.insert(id, count);
    }

    // Power iteration
    for _ in 0..iterations {
        let mut new_scores: AHashMap<i64, f64> = AHashMap::new();

        // Initialize with teleport probability (1-d)/n
        let base_score = (1.0 - damping) / n as f64;
        for &id in &all_ids {
            new_scores.insert(id, base_score);
        }

        // Track total dangling score to redistribute
        let mut dangling_score = 0.0;

        // Distribute scores from outgoing edges
        for &id in &all_ids {
            let score = scores[&id];
            let out_count = outgoing_counts[&id];

            if out_count == 0 {
                // Dangling node - add score to redistribution pool
                dangling_score += score;
            } else {
                // Distribute score evenly to all outgoing neighbors
                let share = score / out_count as f64;
                for &neighbor in &graph.fetch_outgoing(id)? {
                    *new_scores.get_mut(&neighbor).unwrap() += damping * share;
                }
            }
        }

        // Redistribute dangling score equally to all nodes
        let dangling_share = damping * dangling_score / n as f64;
        for (_, score) in new_scores.iter_mut() {
            *score += dangling_share;
        }

        scores = new_scores;
    }

    // Convert to sorted vector
    let mut result: Vec<(i64, f64)> = scores.into_iter().collect();
    result.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    Ok(result)
}

/// Computes betweenness centrality for all nodes in the graph.
///
/// Betweenness centrality measures how often a node appears on shortest paths
/// between other nodes. Bridge nodes (connecting different parts of the graph)
/// score higher.
///
/// # Arguments
/// * `graph` - The graph to analyze
///
/// # Returns
/// Vector of (node_id, centrality) tuples sorted by centrality descending
///
/// # Algorithm
/// Implements Brandes' algorithm for unweighted graphs:
/// 1. For each node s, run BFS to compute shortest paths
/// 2. Track predecessors and path counts during BFS
/// 3. Accumulate dependency values (how much s depends on each node)
/// 4. Sum dependencies across all source nodes
///
/// Handles disconnected components gracefully (pairs with no path are ignored).
pub fn betweenness_centrality(
    graph: &SqliteGraph,
) -> Result<Vec<(i64, f64)>, SqliteGraphError> {
    let all_ids = graph.all_entity_ids()?;
    let n = all_ids.len();

    if n == 0 {
        return Ok(Vec::new());
    }

    // Initialize centrality scores
    let mut centrality: AHashMap<i64, f64> = all_ids.iter().map(|&id| (id, 0.0)).collect();

    // Brandes' algorithm: for each node as source
    for &s in &all_ids {
        // BFS from s
        let mut dist: AHashMap<i64, i64> = AHashMap::new();
        let mut sigma: AHashMap<i64, f64> = AHashMap::new(); // number of shortest paths
        let mut predecessors: AHashMap<i64, Vec<i64>> = AHashMap::new();

        // Initialize source
        dist.insert(s, 0);
        sigma.insert(s, 1.0);

        let mut queue = VecDeque::new();
        queue.push_back(s);

        while let Some(v) = queue.pop_front() {
            for &w in &graph.fetch_outgoing(v)? {
                // First time discovering w
                if !dist.contains_key(&w) {
                    dist.insert(w, dist[&v] + 1);
                    queue.push_back(w);
                }

                // Found another shortest path to w through v
                if dist.get(&w) == Some(&(dist[&v] + 1)) {
                    *sigma.entry(w).or_insert(0.0) += sigma[&v];
                    predecessors.entry(w).or_insert_with(Vec::new).push(v);
                }
            }
        }

        // Accumulate centrality (dependency propagation)
        let mut delta: AHashMap<i64, f64> = all_ids.iter().map(|&id| (id, 0.0)).collect();

        // Process nodes in reverse order of distance from s
        let mut nodes: Vec<i64> = dist.keys().copied().collect();
        nodes.sort_by_key(|&id| std::cmp::Reverse(dist[&id]));

        for w in nodes {
            if w == s {
                continue;
            }

            for &v in predecessors.get(&w).unwrap_or(&vec![]) {
                let contribution = (sigma[&v] / sigma[&w]) * (1.0 + delta[&w]);
                *delta.get_mut(&v).unwrap() += contribution;
            }

            if w != s {
                *centrality.get_mut(&w).unwrap() += delta[&w];
            }
        }
    }

    // Convert to sorted vector
    let mut result: Vec<(i64, f64)> = centrality.into_iter().collect();
    result.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });

    Ok(result)
}

/// Louvain method for community detection via modularity optimization.
///
/// Iteratively moves nodes to maximize modularity (how many edges are within
/// communities vs between communities). Simplified single-pass version.
///
/// # Arguments
/// * `graph` - The graph to analyze
/// * `max_iterations` - Maximum number of iterations to prevent infinite loops
///
/// # Returns
/// Communities as vectors of node IDs, sorted by smallest node ID in each community
pub fn louvain_communities(
    graph: &SqliteGraph,
    max_iterations: usize,
) -> Result<Vec<Vec<i64>>, SqliteGraphError> {
    let all_ids = graph.all_entity_ids()?;

    if all_ids.is_empty() {
        return Ok(Vec::new());
    }

    // Calculate total edges (m) and node degrees
    let mut total_edges = 0usize;
    let mut degrees: AHashMap<i64, usize> = AHashMap::new();

    for &id in &all_ids {
        let out_count = graph.fetch_outgoing(id)?.len();
        let in_count = graph.fetch_incoming(id)?.len();
        let degree = out_count + in_count;
        degrees.insert(id, degree);
        total_edges += degree;
    }

    // Total edges m (undirected: each edge counted twice, so m = sum_degrees / 2)
    let m = total_edges as f64 / 2.0;

    if m == 0.0 {
        // No edges - each node is its own community
        let mut communities: Vec<Vec<i64>> = all_ids.iter().map(|&id| vec![id]).collect();
        communities.sort();
        return Ok(communities);
    }

    // Initialize: each node in its own community
    let mut communities: AHashMap<i64, i64> = all_ids.iter().map(|&id| (id, id)).collect();

    // For deterministic results, process nodes in sorted order
    let mut node_order: Vec<i64> = all_ids.clone();
    node_order.sort();

    // Iterative modularity optimization
    for _iteration in 0..max_iterations {
        let mut any_moved = false;

        for &node in &node_order {
            let current_community = *communities.get(&node).unwrap_or(&node);
            let node_degree = *degrees.get(&node).unwrap_or(&0) as f64;

            // Find neighbor communities
            let mut community_connections: AHashMap<i64, f64> = AHashMap::new();

            // Count outgoing edges
            for &neighbor in &graph.fetch_outgoing(node)? {
                let neighbor_community = *communities.get(&neighbor).unwrap_or(&neighbor);
                *community_connections.entry(neighbor_community).or_insert(0.0) += 1.0;
            }

            // Count incoming edges
            for &neighbor in &graph.fetch_incoming(node)? {
                let neighbor_community = *communities.get(&neighbor).unwrap_or(&neighbor);
                *community_connections.entry(neighbor_community).or_insert(0.0) += 1.0;
            }

            // Calculate modularity delta for moving to each neighbor's community
            let mut best_community = current_community;
            let mut best_delta = 0.0f64;

            for (&target_community, &edges_to_community) in &community_connections {
                if target_community == current_community {
                    continue;
                }

                // Calculate sum of degrees in target community
                let community_degree: f64 = communities
                    .iter()
                    .filter(|(_, &comm)| comm == target_community)
                    .map(|(&node, _)| *degrees.get(&node).unwrap_or(&0) as f64)
                    .sum();

                // Modularity delta formula:
                // ΔQ = (edges_in / m) - (edges_total / m)^2
                // Simplified for single node move:
                // ΔQ = [(2*edges_to_community - node_degree*community_degree/m) / (2*m)]

                let delta = (2.0 * edges_to_community
                    - node_degree * community_degree / m)
                    / (2.0 * m);

                if delta > best_delta {
                    best_delta = delta;
                    best_community = target_community;
                }
            }

            // Move node if it improves modularity
            if best_community != current_community {
                communities.insert(node, best_community);
                any_moved = true;
            }
        }

        if !any_moved {
            break;
        }
    }

    // Group nodes by final community
    let mut communities_map: AHashMap<i64, Vec<i64>> = AHashMap::new();
    for (node, community) in &communities {
        communities_map
            .entry(*community)
            .or_insert_with(Vec::new)
            .push(*node);
    }

    // Convert to sorted vector of communities
    let mut result: Vec<Vec<i64>> = communities_map.into_values().collect();
    for community in &mut result {
        community.sort();
    }
    result.sort_by(|a, b| a.first().cmp(&b.first()));

    Ok(result)
}
