use serde_json::json;
use sqlitegraph::{
    SqliteGraph,
    backend::{BackendDirection, SqliteGraphBackend},
    backend_client::BackendClient,
    bfs::bfs_neighbors,
    dsl::{DslResult, parse_dsl},
    graph::{GraphEdge, GraphEntity},
    index::add_property,
    pattern::{NodeConstraint, PatternLeg, PatternQuery},
    pipeline::{ReasoningPipeline, ReasoningStep},
    safety::run_safety_checks,
    subgraph::{SubgraphRequest, extract_subgraph, structural_signature},
};

#[test]
fn syncompat_smoke_test() -> Result<(), Box<dyn std::error::Error>> {
    let graph = SqliteGraph::open_in_memory()?;
    let harness = TestHarness::build(graph)?;
    harness.assert_subgraph_consistency()?;
    harness.assert_pipeline_consistency()?;
    harness.assert_dsl_matching()?;
    harness.assert_safety()?;
    harness.assert_bfs()?;
    Ok(())
}

struct TestHarness {
    client: BackendClient,
    module_id: i64,
    processor_id: i64,
}

impl TestHarness {
    fn build(graph: SqliteGraph) -> Result<Self, Box<dyn std::error::Error>> {
        let file_id = graph.insert_entity(&GraphEntity {
            id: 0,
            kind: "file".into(),
            name: "FileA.rs".into(),
            file_path: Some("src/FileA.rs".into()),
            data: json!({}),
        })?;
        let module_id = graph.insert_entity(&GraphEntity {
            id: 0,
            kind: "module".into(),
            name: "modA".into(),
            file_path: Some("src/modA".into()),
            data: json!({}),
        })?;
        let processor_id = graph.insert_entity(&GraphEntity {
            id: 0,
            kind: "function".into(),
            name: "func_process".into(),
            file_path: Some("src/modA.rs".into()),
            data: json!({}),
        })?;
        let helper_id = graph.insert_entity(&GraphEntity {
            id: 0,
            kind: "function".into(),
            name: "func_helper".into(),
            file_path: Some("src/modA.rs".into()),
            data: json!({}),
        })?;

        graph.insert_edge(&GraphEdge {
            id: 0,
            from_id: file_id,
            to_id: module_id,
            edge_type: "contains".into(),
            data: json!({}),
        })?;
        graph.insert_edge(&GraphEdge {
            id: 0,
            from_id: module_id,
            to_id: processor_id,
            edge_type: "contains".into(),
            data: json!({}),
        })?;
        graph.insert_edge(&GraphEdge {
            id: 0,
            from_id: processor_id,
            to_id: helper_id,
            edge_type: "calls".into(),
            data: json!({}),
        })?;

        add_property(&graph, processor_id, "imports", "std::fmt")?;
        add_property(&graph, helper_id, "imports", "std::io")?;

        let backend = SqliteGraphBackend::from_graph(graph);
        let client = BackendClient::new(backend);
        Ok(Self {
            client,
            module_id,
            processor_id,
        })
    }

    fn subgraph_request(&self) -> SubgraphRequest {
        SubgraphRequest {
            root: self.processor_id,
            depth: 2,
            allowed_edge_types: vec![],
            allowed_node_types: vec![],
        }
    }

    fn assert_subgraph_consistency(&self) -> Result<(), Box<dyn std::error::Error>> {
        let request = self.subgraph_request();
        let first = extract_subgraph(self.client.backend(), request.clone())?;
        let second = extract_subgraph(self.client.backend(), request)?;
        assert!(!first.nodes.is_empty());
        assert_eq!(structural_signature(&first), structural_signature(&second));
        Ok(())
    }

    fn pipeline(&self) -> ReasoningPipeline {
        ReasoningPipeline {
            steps: vec![ReasoningStep::Pattern(PatternQuery {
                root: Some(NodeConstraint::kind("function")),
                legs: vec![PatternLeg {
                    direction: BackendDirection::Outgoing,
                    edge_type: Some("calls".into()),
                    constraint: Some(NodeConstraint::kind("function")),
                }],
            })],
        }
    }

    fn assert_pipeline_consistency(&self) -> Result<(), Box<dyn std::error::Error>> {
        let pipeline = self.pipeline();
        let result = self.client.run_pipeline(pipeline)?;
        assert!(!result.nodes.is_empty());
        Ok(())
    }

    fn assert_dsl_matching(&self) -> Result<(), Box<dyn std::error::Error>> {
        let dsl = "pattern calls filter type=function";
        let parsed = match parse_dsl(dsl) {
            DslResult::Pipeline(p) => p,
            other => panic!("expected pipeline, got {other:?}"),
        };
        let manual = self.client.run_pipeline(self.pipeline())?;
        let via_dsl = self.client.run_pipeline(parsed)?;
        assert_eq!(sorted(&manual.nodes), sorted(&via_dsl.nodes));
        Ok(())
    }

    fn assert_safety(&self) -> Result<(), Box<dyn std::error::Error>> {
        let report = run_safety_checks(self.client.backend().graph())?;
        let issues = report.orphan_edges
            + report.duplicate_edges
            + report.invalid_labels
            + report.invalid_properties;
        assert_eq!(issues, 0);
        Ok(())
    }

    fn assert_bfs(&self) -> Result<(), Box<dyn std::error::Error>> {
        let bfs_nodes = bfs_neighbors(self.client.backend().graph(), self.module_id, 2)?;
        assert!(!bfs_nodes.is_empty());
        assert!(bfs_nodes.contains(&self.module_id));
        assert!(bfs_nodes.contains(&self.processor_id));
        Ok(())
    }
}

fn sorted(values: &[i64]) -> Vec<i64> {
    let mut copy = values.to_vec();
    copy.sort_unstable();
    copy
}
