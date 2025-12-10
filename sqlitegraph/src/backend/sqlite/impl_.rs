//! SQLite backend implementation for GraphBackend trait.
//!
//! This module contains the SqliteGraphBackend implementation that wraps
//! SqliteGraph and provides the concrete implementation of all GraphBackend methods.

use rusqlite::params;

use crate::{
    SqliteGraphError,
    backend::sqlite::types::{BackendDirection, EdgeSpec, NeighborQuery, NodeSpec},
    bfs::{bfs_neighbors, shortest_path},
    graph::{GraphEdge, GraphEntity, SqliteGraph},
    multi_hop,
    pattern::{self, PatternMatch, PatternQuery},
};

/// SQLite-backed implementation of the GraphBackend trait.
///
/// This struct wraps a SqliteGraph instance and implements all GraphBackend methods
/// by delegating to the underlying SQLite-based graph operations.
pub struct SqliteGraphBackend {
    graph: SqliteGraph,
}

impl SqliteGraphBackend {
    /// Create a new SQLite backend with an in-memory database.
    pub fn in_memory() -> Result<Self, SqliteGraphError> {
        Ok(Self {
            graph: SqliteGraph::open_in_memory()?,
        })
    }

    /// Create a new SQLite backend from an existing SqliteGraph instance.
    pub fn from_graph(graph: SqliteGraph) -> Self {
        Self { graph }
    }

    /// Get a reference to the underlying SqliteGraph instance.
    pub fn graph(&self) -> &SqliteGraph {
        &self.graph
    }

    /// Get all entity IDs from the graph.
    pub fn entity_ids(&self) -> Result<Vec<i64>, SqliteGraphError> {
        self.graph.all_entity_ids()
    }

    /// Execute optimized neighbor queries based on direction and edge type filtering.
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
                let conn = self.graph.connection();
                let mut stmt = conn
                    .prepare_cached(
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
                let conn = self.graph.connection();
                let mut stmt = conn
                    .prepare_cached(
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

impl crate::backend::GraphBackend for SqliteGraphBackend {
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        self.graph.insert_entity(&GraphEntity {
            id: 0,
            kind: node.kind,
            name: node.name,
            file_path: node.file_path,
            data: node.data,
        })
    }

    fn get_node(&self, id: i64) -> Result<GraphEntity, SqliteGraphError> {
        self.graph.get_entity(id)
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

    fn chain_query(
        &self,
        start: i64,
        chain: &[crate::multi_hop::ChainStep],
    ) -> Result<Vec<i64>, SqliteGraphError> {
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
