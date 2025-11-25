use std::collections::VecDeque;

use ahash::AHashSet;
use rusqlite::params;

use super::types::{Constraint, MatchResult};
use crate::{
    SqliteGraphError,
    api_ergonomics::{
        Label, NodeId, PipelineExplanation, PropertyKey, PropertyValue,
        explain_pipeline as explain_pipeline_internal,
    },
    backend::{EdgeSpec, GraphBackend, NeighborQuery, NodeSpec, SqliteGraphBackend},
    graph::GraphEntity,
    index::{get_entities_by_label, get_entities_by_property},
    pattern::PatternQuery,
    pipeline::{PipelineResult, ReasoningPipeline, run_pipeline},
    subgraph::{Subgraph, SubgraphRequest, extract_subgraph},
};

pub struct BackendClient {
    backend: SqliteGraphBackend,
}

impl BackendClient {
    pub fn new(backend: SqliteGraphBackend) -> Self {
        Self { backend }
    }

    pub fn backend(&self) -> &SqliteGraphBackend {
        &self.backend
    }

    pub fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        self.backend.insert_node(node)
    }

    pub fn insert_edge(&self, edge: EdgeSpec) -> Result<i64, SqliteGraphError> {
        self.backend.insert_edge(edge)
    }

    pub fn neighbors(&self, node: i64, query: NeighborQuery) -> Result<Vec<i64>, SqliteGraphError> {
        self.backend.neighbors(node, query)
    }

    pub fn bfs(&self, start: i64, depth: u32) -> Result<Vec<i64>, SqliteGraphError> {
        self.backend.bfs(start, depth)
    }

    pub fn shortest_path(
        &self,
        start: i64,
        end: i64,
    ) -> Result<Option<Vec<i64>>, SqliteGraphError> {
        self.backend.shortest_path(start, end)
    }

    /// Returns an entity by [`NodeId`], or `None` if the id is missing.
    ///
    /// ```
    /// # use serde_json::json;
    /// # use sqlitegraph::{BackendClient, NodeId};
    /// # use sqlitegraph::backend::{NodeSpec, SqliteGraphBackend};
    /// let backend = SqliteGraphBackend::in_memory().unwrap();
    /// let client = BackendClient::new(backend);
    /// let node_id = client.insert_node(NodeSpec{kind:"Fn".into(), name:"demo".into(), file_path:None, data: json!({})}).unwrap();
    /// let entity = client.get_node(NodeId(node_id)).unwrap();
    /// assert!(entity.is_some());
    /// ```
    pub fn get_node(&self, node: NodeId) -> Result<Option<GraphEntity>, SqliteGraphError> {
        match self.backend.graph().get_entity(node.0) {
            Ok(entity) => Ok(Some(entity)),
            Err(SqliteGraphError::NotFound(_)) => Ok(None),
            Err(err) => Err(err),
        }
    }

    /// Returns neighbor ids for the given [`NodeId`].
    ///
    /// ```
    /// # use serde_json::json;
    /// # use sqlitegraph::{BackendClient, NodeId};
    /// # use sqlitegraph::backend::{NodeSpec, EdgeSpec, SqliteGraphBackend};
    /// let backend = SqliteGraphBackend::in_memory().unwrap();
    /// let client = BackendClient::new(backend);
    /// let a = client.insert_node(NodeSpec{kind:"Fn".into(), name:"a".into(), file_path:None, data: json!({})}).unwrap();
    /// let b = client.insert_node(NodeSpec{kind:"Fn".into(), name:"b".into(), file_path:None, data: json!({})}).unwrap();
    /// client.insert_edge(EdgeSpec{from:a,to:b,edge_type:"CALLS".into(),data:json!({})}).unwrap();
    /// let neighbors = client.neighbors_of(NodeId(a)).unwrap();
    /// assert_eq!(neighbors, vec![NodeId(b)]);
    /// ```
    pub fn neighbors_of(&self, node: NodeId) -> Result<Vec<NodeId>, SqliteGraphError> {
        let mut ids = Vec::new();
        for neighbor in self.neighbors(node.0, NeighborQuery::default())? {
            ids.push(NodeId(neighbor));
        }
        Ok(ids)
    }

    /// Returns ids tagged with the provided [`Label`].
    ///
    /// ```
    /// # use serde_json::json;
    /// # use sqlitegraph::{BackendClient, Label, NodeId};
    /// # use sqlitegraph::backend::{NodeSpec, SqliteGraphBackend};
    /// let backend = SqliteGraphBackend::in_memory().unwrap();
    /// let client = BackendClient::new(backend);
    /// let node = client.insert_node(NodeSpec{kind:"Fn".into(), name:"lab".into(), file_path:None, data: json!({})}).unwrap();
    /// sqlitegraph::index::add_label(client.backend().graph(), node, "Fn").unwrap();
    /// let nodes = client.labeled(Label("Fn".into())).unwrap();
    /// assert_eq!(nodes, vec![NodeId(node)]);
    /// ```
    pub fn labeled(&self, label: Label) -> Result<Vec<NodeId>, SqliteGraphError> {
        let entities = get_entities_by_label(self.backend.graph(), &label.0)?;
        Ok(entities
            .into_iter()
            .map(|entity| NodeId(entity.id))
            .collect())
    }

    /// Returns ids that match the provided property key/value pair.
    ///
    /// ```
    /// # use serde_json::json;
    /// # use sqlitegraph::{BackendClient, NodeId, PropertyKey, PropertyValue};
    /// # use sqlitegraph::backend::{NodeSpec, SqliteGraphBackend};
    /// let backend = SqliteGraphBackend::in_memory().unwrap();
    /// let client = BackendClient::new(backend);
    /// let node = client.insert_node(NodeSpec{kind:"Fn".into(), name:"prop".into(), file_path:None, data: json!({})}).unwrap();
    /// sqlitegraph::index::add_property(client.backend().graph(), node, "role", "leaf").unwrap();
    /// let nodes = client.with_property(PropertyKey("role".into()), PropertyValue("leaf".into())).unwrap();
    /// assert_eq!(nodes, vec![NodeId(node)]);
    /// ```
    pub fn with_property(
        &self,
        key: PropertyKey,
        value: PropertyValue,
    ) -> Result<Vec<NodeId>, SqliteGraphError> {
        let entities = get_entities_by_property(self.backend.graph(), &key.0, &value.0)?;
        Ok(entities
            .into_iter()
            .map(|entity| NodeId(entity.id))
            .collect())
    }

    /// Provides a deterministic explanation of the supplied pipeline.
    ///
    /// ```
    /// # use serde_json::json;
    /// # use sqlitegraph::{BackendClient};
    /// # use sqlitegraph::backend::{NodeSpec, EdgeSpec, SqliteGraphBackend};
    /// # use sqlitegraph::pattern::{NodeConstraint, PatternLeg, PatternQuery};
    /// # use sqlitegraph::pipeline::{ReasoningPipeline, ReasoningStep};
    /// let backend = SqliteGraphBackend::in_memory().unwrap();
    /// let client = BackendClient::new(backend);
    /// let a = client.insert_node(NodeSpec{kind:"Fn".into(), name:"a".into(), file_path:None, data: json!({})}).unwrap();
    /// let b = client.insert_node(NodeSpec{kind:"Fn".into(), name:"b".into(), file_path:None, data: json!({})}).unwrap();
    /// client.insert_edge(EdgeSpec{from:a,to:b,edge_type:"CALLS".into(),data:json!({})}).unwrap();
    /// let pipeline = ReasoningPipeline{ steps: vec![ReasoningStep::Pattern(PatternQuery{ root: Some(NodeConstraint::kind("Fn")), legs: vec![PatternLeg{ direction: sqlitegraph::backend::BackendDirection::Outgoing, edge_type: Some("CALLS".into()), constraint: Some(NodeConstraint::kind("Fn")) }], })] };
    /// let explanation = client.explain_pipeline(pipeline).unwrap();
    /// assert_eq!(explanation.steps_summary.len(), 1);
    /// ```
    pub fn explain_pipeline(
        &self,
        pipeline: ReasoningPipeline,
    ) -> Result<PipelineExplanation, SqliteGraphError> {
        explain_pipeline_internal(&self.backend, &pipeline)
    }

    pub fn run_pattern(&self, query: PatternQuery) -> Result<Vec<MatchResult>, SqliteGraphError> {
        let mut matches = Vec::new();
        for id in self.backend.entity_ids()? {
            let mut partial = self.backend.graph().query().pattern_matches(id, &query)?;
            matches.append(&mut partial);
        }
        matches.sort_by(|a, b| a.nodes.cmp(&b.nodes));
        Ok(matches)
    }

    pub fn run_pipeline(
        &self,
        pipeline: ReasoningPipeline,
    ) -> Result<PipelineResult, SqliteGraphError> {
        run_pipeline(&self.backend, &pipeline)
    }

    pub fn subgraph(&self, request: SubgraphRequest) -> Result<Subgraph, SqliteGraphError> {
        extract_subgraph(&self.backend, request)
    }

    pub fn entity_by_label(&self, label: &str) -> Result<Vec<GraphEntity>, SqliteGraphError> {
        get_entities_by_label(self.backend.graph(), label)
    }

    pub fn find_by_property(
        &self,
        key: &str,
        value: &str,
    ) -> Result<Vec<GraphEntity>, SqliteGraphError> {
        get_entities_by_property(self.backend.graph(), key, value)
    }

    pub fn shortest_path_with_constraints(
        &self,
        start: i64,
        end: i64,
        constraint: Constraint,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        let graph = self.backend.graph();
        let allowed_edges = into_lookup(&constraint.allowed_edge_types);
        let allowed_nodes = into_lookup(&constraint.allowed_node_types);
        let mut queue = VecDeque::new();
        queue.push_back(vec![start]);
        let mut visited = AHashSet::new();
        visited.insert(start);
        while let Some(path) = queue.pop_front() {
            let &current = path.last().expect("path");
            if current == end {
                return Ok(path);
            }
            for (neighbor, ty) in fetch_outgoing(graph, current)? {
                if let Some(filter) = allowed_edges.as_ref() {
                    if !filter.contains(ty.as_str()) {
                        continue;
                    }
                }
                if let Some(filter) = allowed_nodes.as_ref() {
                    let entity = graph.get_entity(neighbor)?;
                    if !filter.contains(entity.kind.as_str()) {
                        continue;
                    }
                }
                if visited.insert(neighbor) {
                    let mut next = path.clone();
                    next.push(neighbor);
                    queue.push_back(next);
                }
            }
        }
        Ok(Vec::new())
    }
}

fn into_lookup(values: &[String]) -> Option<AHashSet<&str>> {
    if values.is_empty() {
        None
    } else {
        Some(values.iter().map(|s| s.as_str()).collect())
    }
}

fn fetch_outgoing(
    graph: &crate::graph::SqliteGraph,
    node: i64,
) -> Result<Vec<(i64, String)>, SqliteGraphError> {
    let conn = graph.connection();
    let mut stmt = conn
        .prepare_cached(
            "SELECT to_id, edge_type FROM graph_edges WHERE from_id=?1 ORDER BY to_id, edge_type, id",
        )
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let rows = stmt
        .query_map(params![node], |row| {
            let to: i64 = row.get(0)?;
            let ty: String = row.get(1)?;
            Ok((to, ty))
        })
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let mut edges = Vec::new();
    for row in rows {
        edges.push(row.map_err(|e| SqliteGraphError::query(e.to_string()))?);
    }
    Ok(edges)
}
