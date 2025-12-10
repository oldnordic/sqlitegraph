//! Backend trait bridging sqlitegraph with higher-level graph consumers.
//!
//! This module contains the core GraphBackend trait and redirects to modular
//! backend implementations. SQLite-specific implementations are in the sqlite submodule.

// Include the modular backend structure
mod sqlite;

// Include native backend storage layer (no GraphBackend implementation yet)
pub mod native;

// Re-export from sqlite submodule for backward compatibility
pub use sqlite::SqliteGraphBackend;

// Re-export from native submodule
pub use native::NativeGraphBackend;

// Re-export types for external users
pub use crate::multi_hop::ChainStep;
pub use sqlite::types::{BackendDirection, EdgeSpec, NeighborQuery, NodeSpec};

use crate::{
    SqliteGraphError,
    graph::GraphEntity,
    pattern::{PatternMatch, PatternQuery},
};

/// Backend trait defining the interface for graph database backends.
///
/// Each trait method delegates to backend-specific primitives while ensuring
/// deterministic behavior and a single integration surface for consumers.
pub trait GraphBackend {
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError>;
    fn get_node(&self, id: i64) -> Result<GraphEntity, SqliteGraphError>;
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
    fn chain_query(
        &self,
        start: i64,
        chain: &[crate::multi_hop::ChainStep],
    ) -> Result<Vec<i64>, SqliteGraphError>;
    fn pattern_search(
        &self,
        start: i64,
        pattern: &PatternQuery,
    ) -> Result<Vec<PatternMatch>, SqliteGraphError>;
}

/// Reference implementation for GraphBackend trait that works with references.
impl<B> GraphBackend for &B
where
    B: GraphBackend + ?Sized,
{
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        (*self).insert_node(node)
    }

    fn get_node(&self, id: i64) -> Result<GraphEntity, SqliteGraphError> {
        (*self).get_node(id)
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

    fn chain_query(
        &self,
        start: i64,
        chain: &[crate::multi_hop::ChainStep],
    ) -> Result<Vec<i64>, SqliteGraphError> {
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
