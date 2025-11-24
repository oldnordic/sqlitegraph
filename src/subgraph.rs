use std::collections::VecDeque;

use ahash::AHashSet;
use rusqlite::params;

use crate::{SqliteGraphError, backend::SqliteGraphBackend, graph::SqliteGraph};

#[derive(Clone, Debug)]
pub struct SubgraphRequest {
    pub root: i64,
    pub depth: u32,
    pub allowed_edge_types: Vec<String>,
    pub allowed_node_types: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Subgraph {
    pub nodes: Vec<i64>,
    pub edges: Vec<(i64, i64, String)>,
}

pub fn extract_subgraph(
    backend: &SqliteGraphBackend,
    request: SubgraphRequest,
) -> Result<Subgraph, SqliteGraphError> {
    let mut visited = AHashSet::new();
    let mut queue = VecDeque::new();
    queue.push_back((request.root, 0_u32));
    visited.insert(request.root);

    let allowed_edge_types = into_lookup(&request.allowed_edge_types);
    let allowed_node_types = into_lookup(&request.allowed_node_types);

    let graph = backend.graph();
    let mut nodes = Vec::new();
    let mut edges = Vec::new();

    while let Some((node, depth)) = queue.pop_front() {
        nodes.push(node);
        if depth >= request.depth {
            continue;
        }
        for (target, edge_type) in fetch_outgoing(graph, node)? {
            if let Some(filter) = allowed_edge_types.as_ref() {
                if !filter.contains(edge_type.as_str()) {
                    continue;
                }
            }
            let entity = graph.get_entity(target)?;
            if let Some(filter) = allowed_node_types.as_ref() {
                if !filter.contains(entity.kind.as_str()) {
                    continue;
                }
            }
            edges.push((node, target, edge_type.clone()));
            if visited.insert(target) {
                queue.push_back((target, depth + 1));
            }
        }
    }
    nodes.sort_unstable();
    nodes.dedup();
    edges.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    Ok(Subgraph { nodes, edges })
}

pub fn structural_signature(subgraph: &Subgraph) -> String {
    let node_str = subgraph
        .nodes
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let edge_str = subgraph
        .edges
        .iter()
        .map(|(from, to, ty)| format!("{from}->{to}:{ty}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("N[{node_str}]|E[{edge_str}]")
}

fn into_lookup(items: &[String]) -> Option<AHashSet<&str>> {
    if items.is_empty() {
        None
    } else {
        Some(items.iter().map(|s| s.as_str()).collect())
    }
}

fn fetch_outgoing(graph: &SqliteGraph, node: i64) -> Result<Vec<(i64, String)>, SqliteGraphError> {
    let mut stmt = graph
        .connection()
        .prepare(
            "SELECT to_id, edge_type FROM graph_edges \
             WHERE from_id=?1 ORDER BY to_id, edge_type, id",
        )
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let rows = stmt
        .query_map(params![node], |row| {
            let to: i64 = row.get(0)?;
            let ty: String = row.get(1)?;
            Ok((to, ty))
        })
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let mut neighbors = Vec::new();
    for row in rows {
        neighbors.push(row.map_err(|e| SqliteGraphError::query(e.to_string()))?);
    }
    Ok(neighbors)
}
