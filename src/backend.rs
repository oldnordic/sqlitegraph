//! Backend trait bridging sqlitegraph with higher-level graph consumers. Each trait
//! method delegates to the existing sqlitegraph primitives (e.g., [`SqliteGraph::insert_entity`],
//! [`crate::bfs::bfs_neighbors`]), ensuring deterministic behavior while providing a single
//! integration surface. The `sqlite-backend` Cargo feature (enabled by default) keeps this
//! adapter compiled in, simplifying future backend selection.

use rusqlite::params;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    SqliteGraphError,
    bfs::{bfs_neighbors, shortest_path},
    graph::{GraphEdge, GraphEntity, SqliteGraph},
    multi_hop,
    pattern::{self, PatternMatch, PatternQuery},
};

pub use crate::multi_hop::ChainStep;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum BackendDirection {
    Outgoing,
    Incoming,
}

#[derive(Clone, Debug)]
pub struct NeighborQuery {
    pub direction: BackendDirection,
    pub edge_type: Option<String>,
}

impl Default for NeighborQuery {
    fn default() -> Self {
        Self {
            direction: BackendDirection::Outgoing,
            edge_type: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct NodeSpec {
    pub kind: String,
    pub name: String,
    pub file_path: Option<String>,
    pub data: Value,
}

#[derive(Clone, Debug)]
pub struct EdgeSpec {
    pub from: i64,
    pub to: i64,
    pub edge_type: String,
    pub data: Value,
}

pub trait GraphBackend {
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError>;
    fn insert_edge(&self, edge: EdgeSpec) -> Result<i64, SqliteGraphError>;
    fn neighbors(&self, node: i64, query: NeighborQuery) -> Result<Vec<i64>, SqliteGraphError>;
    fn bfs(&self, start: i64, depth: u32) -> Result<Vec<i64>, SqliteGraphError>;
    fn shortest_path(&self, start: i64, end: i64) -> Result<Option<Vec<i64>>, SqliteGraphError>;
    fn node_degree(&self, node: i64) -> Result<(usize, usize), SqliteGraphError>;
    fn k_hop(
        &self,
        start: i64,
        depth: u32,
        direction: BackendDirection,
    ) -> Result<Vec<i64>, SqliteGraphError>;
    fn k_hop_filtered(
        &self,
        start: i64,
        depth: u32,
        direction: BackendDirection,
        allowed_edge_types: &[&str],
    ) -> Result<Vec<i64>, SqliteGraphError>;
    fn chain_query(&self, start: i64, chain: &[ChainStep]) -> Result<Vec<i64>, SqliteGraphError>;
    fn pattern_search(
        &self,
        start: i64,
        pattern: &PatternQuery,
    ) -> Result<Vec<PatternMatch>, SqliteGraphError>;
}

pub struct SqliteGraphBackend {
    graph: SqliteGraph,
}

impl SqliteGraphBackend {
    pub fn in_memory() -> Result<Self, SqliteGraphError> {
        Ok(Self {
            graph: SqliteGraph::open_in_memory()?,
        })
    }

    pub fn from_graph(graph: SqliteGraph) -> Self {
        Self { graph }
    }

    fn query_neighbors(
        &self,
        node: i64,
        direction: BackendDirection,
        edge_type: &Option<String>,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        match (direction, edge_type) {
            (BackendDirection::Outgoing, None) => self.graph.fetch_outgoing(node),
            (BackendDirection::Incoming, None) => self.graph.fetch_incoming(node),
            (BackendDirection::Outgoing, Some(edge_type)) => {
                let mut stmt = self
                    .graph
                    .connection()
                    .prepare(
                        "SELECT to_id FROM graph_edges WHERE from_id=?1 AND edge_type=?2 ORDER BY to_id, id",
                    )
                    .map_err(|e| SqliteGraphError::query(e.to_string()))?;
                let rows = stmt
                    .query_map(params![node, edge_type], |row| row.get(0))
                    .map_err(|e| SqliteGraphError::query(e.to_string()))?;
                let mut values = Vec::new();
                for value in rows {
                    values.push(value.map_err(|e| SqliteGraphError::query(e.to_string()))?);
                }
                Ok(values)
            }
            (BackendDirection::Incoming, Some(edge_type)) => {
                let mut stmt = self
                    .graph
                    .connection()
                    .prepare(
                        "SELECT from_id FROM graph_edges WHERE to_id=?1 AND edge_type=?2 ORDER BY from_id, id",
                    )
                    .map_err(|e| SqliteGraphError::query(e.to_string()))?;
                let rows = stmt
                    .query_map(params![node, edge_type], |row| row.get(0))
                    .map_err(|e| SqliteGraphError::query(e.to_string()))?;
                let mut values = Vec::new();
                for value in rows {
                    values.push(value.map_err(|e| SqliteGraphError::query(e.to_string()))?);
                }
                Ok(values)
            }
        }
    }
}

impl GraphBackend for SqliteGraphBackend {
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        self.graph.insert_entity(&GraphEntity {
            id: 0,
            kind: node.kind,
            name: node.name,
            file_path: node.file_path,
            data: node.data,
        })
    }

    fn insert_edge(&self, edge: EdgeSpec) -> Result<i64, SqliteGraphError> {
        self.graph.insert_edge(&GraphEdge {
            id: 0,
            from_id: edge.from,
            to_id: edge.to,
            edge_type: edge.edge_type,
            data: edge.data,
        })
    }

    fn neighbors(&self, node: i64, query: NeighborQuery) -> Result<Vec<i64>, SqliteGraphError> {
        self.query_neighbors(node, query.direction, &query.edge_type)
    }

    fn bfs(&self, start: i64, depth: u32) -> Result<Vec<i64>, SqliteGraphError> {
        bfs_neighbors(&self.graph, start, depth)
    }

    fn shortest_path(&self, start: i64, end: i64) -> Result<Option<Vec<i64>>, SqliteGraphError> {
        shortest_path(&self.graph, start, end)
    }

    fn node_degree(&self, node: i64) -> Result<(usize, usize), SqliteGraphError> {
        let out = self.graph.fetch_outgoing(node)?.len();
        let incoming = self.graph.fetch_incoming(node)?.len();
        Ok((out, incoming))
    }

    fn k_hop(
        &self,
        start: i64,
        depth: u32,
        direction: BackendDirection,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        multi_hop::k_hop(&self.graph, start, depth, direction)
    }

    fn k_hop_filtered(
        &self,
        start: i64,
        depth: u32,
        direction: BackendDirection,
        allowed_edge_types: &[&str],
    ) -> Result<Vec<i64>, SqliteGraphError> {
        multi_hop::k_hop_filtered(&self.graph, start, depth, direction, allowed_edge_types)
    }

    fn chain_query(&self, start: i64, chain: &[ChainStep]) -> Result<Vec<i64>, SqliteGraphError> {
        multi_hop::chain_query(&self.graph, start, chain)
    }

    fn pattern_search(
        &self,
        start: i64,
        pattern: &PatternQuery,
    ) -> Result<Vec<PatternMatch>, SqliteGraphError> {
        pattern::execute_pattern(&self.graph, start, pattern)
    }
}

impl SqliteGraphBackend {
    pub fn graph(&self) -> &SqliteGraph {
        &self.graph
    }

    pub fn entity_ids(&self) -> Result<Vec<i64>, SqliteGraphError> {
        self.graph.all_entity_ids()
    }
}

impl<'a, B> GraphBackend for &'a B
where
    B: GraphBackend + ?Sized,
{
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        (*self).insert_node(node)
    }

    fn insert_edge(&self, edge: EdgeSpec) -> Result<i64, SqliteGraphError> {
        (*self).insert_edge(edge)
    }

    fn neighbors(&self, node: i64, query: NeighborQuery) -> Result<Vec<i64>, SqliteGraphError> {
        (*self).neighbors(node, query)
    }

    fn bfs(&self, start: i64, depth: u32) -> Result<Vec<i64>, SqliteGraphError> {
        (*self).bfs(start, depth)
    }

    fn shortest_path(&self, start: i64, end: i64) -> Result<Option<Vec<i64>>, SqliteGraphError> {
        (*self).shortest_path(start, end)
    }

    fn node_degree(&self, node: i64) -> Result<(usize, usize), SqliteGraphError> {
        (*self).node_degree(node)
    }

    fn k_hop(
        &self,
        start: i64,
        depth: u32,
        direction: BackendDirection,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        (*self).k_hop(start, depth, direction)
    }

    fn k_hop_filtered(
        &self,
        start: i64,
        depth: u32,
        direction: BackendDirection,
        allowed_edge_types: &[&str],
    ) -> Result<Vec<i64>, SqliteGraphError> {
        (*self).k_hop_filtered(start, depth, direction, allowed_edge_types)
    }

    fn chain_query(&self, start: i64, chain: &[ChainStep]) -> Result<Vec<i64>, SqliteGraphError> {
        (*self).chain_query(start, chain)
    }

    fn pattern_search(
        &self,
        start: i64,
        pattern: &PatternQuery,
    ) -> Result<Vec<PatternMatch>, SqliteGraphError> {
        (*self).pattern_search(start, pattern)
    }
}
