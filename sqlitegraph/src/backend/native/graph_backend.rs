//! Native GraphBackend implementation with interior mutability.

use super::adjacency::{AdjacencyHelpers, Direction};
use super::edge_store::EdgeStore;
use super::graph_file::GraphFile;
use super::graph_ops::*;
use super::graph_validation::*;
use super::node_store::NodeStore;
use super::types::*;
use crate::SqliteGraphError;
use crate::backend::{
    BackendDirection, ChainStep, EdgeSpec, GraphBackend, NeighborQuery, NodeSpec, PatternMatch,
    PatternQuery,
};
use crate::graph::GraphEntity;
use parking_lot::RwLock;

/// Native backend implementation using interior mutability
pub struct NativeGraphBackend {
    graph_file: RwLock<GraphFile>,
}

impl NativeGraphBackend {
    /// Create a new native backend with an in-memory temporary file
    #[cfg(test)]
    pub fn new_temp() -> Result<Self, SqliteGraphError> {
        use tempfile::NamedTempFile;
        let temp_file =
            NamedTempFile::new().map_err(|e| SqliteGraphError::connection(e.to_string()))?;
        let path = temp_file.path();
        let graph_file = GraphFile::create(path).map_err(map_to_graph_error)?;

        Ok(Self {
            graph_file: RwLock::new(graph_file),
        })
    }

    /// Create a new native backend at the specified path
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Result<Self, SqliteGraphError> {
        let graph_file = GraphFile::create(path).map_err(map_to_graph_error)?;

        Ok(Self {
            graph_file: RwLock::new(graph_file),
        })
    }

    /// Open an existing native backend from the specified path
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self, SqliteGraphError> {
        let graph_file = GraphFile::open(path).map_err(map_to_graph_error)?;

        Ok(Self {
            graph_file: RwLock::new(graph_file),
        })
    }

    /// Get mutable access to the underlying graph file for internal operations
    fn with_graph_file<R, F>(&self, f: F) -> Result<R, SqliteGraphError>
    where
        F: FnOnce(&mut GraphFile) -> Result<R, NativeBackendError>,
    {
        let mut graph_file = self.graph_file.write();
        f(&mut *graph_file).map_err(map_to_graph_error)
    }
}

impl GraphBackend for NativeGraphBackend {
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let mut node_store = NodeStore::new(graph_file);
            let node_id = node_store.allocate_node_id();
            let record = node_spec_to_record(node, node_id);
            node_store.write_node(&record)?;
            Ok(node_id as i64)
        })
    }

    fn get_node(&self, id: i64) -> Result<GraphEntity, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let mut node_store = NodeStore::new(graph_file);
            let record = node_store.read_node(id as NativeNodeId)?;
            Ok(node_record_to_entity(record))
        })
    }

    fn insert_edge(&self, edge: EdgeSpec) -> Result<i64, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let mut edge_store = EdgeStore::new(graph_file);
            let edge_id = edge_store.allocate_edge_id();
            let record = edge_spec_to_record(edge, edge_id);
            edge_store.write_edge(&record)?;
            Ok(edge_id as i64)
        })
    }

    fn neighbors(&self, node: i64, query: NeighborQuery) -> Result<Vec<i64>, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let node_id = node as NativeNodeId;

            let neighbors = if let Some(edge_type) = &query.edge_type {
                let edge_type_ref = edge_type.as_str();
                match query.direction {
                    BackendDirection::Outgoing => {
                        AdjacencyHelpers::get_outgoing_neighbors_filtered(
                            graph_file,
                            node_id,
                            &[edge_type_ref],
                        )
                    }
                    BackendDirection::Incoming => {
                        AdjacencyHelpers::get_incoming_neighbors_filtered(
                            graph_file,
                            node_id,
                            &[edge_type_ref],
                        )
                    }
                }
            } else {
                match query.direction {
                    BackendDirection::Outgoing => {
                        AdjacencyHelpers::get_outgoing_neighbors(graph_file, node_id)
                    }
                    BackendDirection::Incoming => {
                        AdjacencyHelpers::get_incoming_neighbors(graph_file, node_id)
                    }
                }
            }?;

            Ok(neighbors.into_iter().map(|id| id as i64).collect())
        })
    }

    fn bfs(&self, start: i64, depth: u32) -> Result<Vec<i64>, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let result = native_bfs(graph_file, start as NativeNodeId, depth)?;
            Ok(result.into_iter().map(|id| id as i64).collect())
        })
    }

    fn shortest_path(&self, start: i64, end: i64) -> Result<Option<Vec<i64>>, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let result =
                native_shortest_path(graph_file, start as NativeNodeId, end as NativeNodeId)?;
            Ok(result.map(|path| path.into_iter().map(|id| id as i64).collect()))
        })
    }

    fn node_degree(&self, node: i64) -> Result<(usize, usize), SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let node_id = node as NativeNodeId;
            let outgoing = AdjacencyHelpers::outgoing_degree(graph_file, node_id)?;
            let incoming = AdjacencyHelpers::incoming_degree(graph_file, node_id)?;
            Ok((outgoing as usize, incoming as usize))
        })
    }

    fn k_hop(
        &self,
        start: i64,
        depth: u32,
        direction: BackendDirection,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let result = native_k_hop(
                graph_file,
                start as NativeNodeId,
                depth,
                match direction {
                    BackendDirection::Outgoing => Direction::Outgoing,
                    BackendDirection::Incoming => Direction::Incoming,
                },
            )?;
            Ok(result.into_iter().map(|id| id as i64).collect())
        })
    }

    fn k_hop_filtered(
        &self,
        start: i64,
        depth: u32,
        direction: BackendDirection,
        allowed_edge_types: &[&str],
    ) -> Result<Vec<i64>, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let result = native_k_hop_filtered(
                graph_file,
                start as NativeNodeId,
                depth,
                match direction {
                    BackendDirection::Outgoing => Direction::Outgoing,
                    BackendDirection::Incoming => Direction::Incoming,
                },
                allowed_edge_types,
            )?;
            Ok(result.into_iter().map(|id| id as i64).collect())
        })
    }

    fn chain_query(&self, start: i64, chain: &[ChainStep]) -> Result<Vec<i64>, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let result = native_chain_query(graph_file, start as NativeNodeId, chain)?;
            Ok(result.into_iter().map(|id| id as i64).collect())
        })
    }

    fn pattern_search(
        &self,
        start: i64,
        pattern: &PatternQuery,
    ) -> Result<Vec<PatternMatch>, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            native_pattern_search(graph_file, start as NativeNodeId, pattern)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_backend_creation() {
        let backend = NativeGraphBackend::new_temp().unwrap();
        // Test that backend can be created successfully
        assert!(true);
    }

    #[test]
    fn test_interior_mutability() {
        let backend = NativeGraphBackend::new_temp().unwrap();

        // Test that we can perform multiple operations
        let node_id = backend
            .insert_node(NodeSpec {
                kind: "Test".to_string(),
                name: "node1".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        let node = backend.get_node(node_id).unwrap();
        assert_eq!(node.name, "node1");
        assert_eq!(node.kind, "Test");
    }
}
