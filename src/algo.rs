use std::collections::VecDeque;

use ahash::AHashSet;

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
