use std::collections::VecDeque;

use ahash::{AHashMap, AHashSet};

use crate::{errors::SqliteGraphError, graph::SqliteGraph};

pub fn bfs_neighbors(
    graph: &SqliteGraph,
    start: i64,
    max_depth: u32,
) -> Result<Vec<i64>, SqliteGraphError> {
    graph.get_entity(start)?;
    let mut visited = Vec::new();
    let mut seen = AHashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((start, 0));
    seen.insert(start);
    while let Some((node, depth)) = queue.pop_front() {
        visited.push(node);
        if depth >= max_depth {
            continue;
        }
        for next in graph.fetch_outgoing(node)? {
            if seen.insert(next) {
                queue.push_back((next, depth + 1));
            }
        }
    }
    Ok(visited)
}

pub fn shortest_path(
    graph: &SqliteGraph,
    start: i64,
    end: i64,
) -> Result<Option<Vec<i64>>, SqliteGraphError> {
    graph.get_entity(start)?;
    graph.get_entity(end)?;
    if start == end {
        return Ok(Some(vec![start]));
    }
    let mut queue = VecDeque::new();
    let mut parents = AHashMap::new();
    let mut seen = AHashSet::new();
    queue.push_back(start);
    seen.insert(start);
    let mut found = false;
    while let Some(node) = queue.pop_front() {
        for next in graph.fetch_outgoing(node)? {
            if seen.insert(next) {
                parents.insert(next, node);
                if next == end {
                    found = true;
                    break;
                }
                queue.push_back(next);
            }
        }
        if found {
            break;
        }
    }
    if !found {
        return Ok(None);
    }
    let mut path = vec![end];
    let mut current = end;
    while let Some(&parent) = parents.get(&current) {
        path.push(parent);
        if parent == start {
            break;
        }
        current = parent;
    }
    if *path.last().unwrap() != start {
        return Ok(None);
    }
    path.reverse();
    Ok(Some(path))
}
