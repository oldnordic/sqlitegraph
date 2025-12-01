use std::{fs, path::PathBuf, time::Instant};

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = temp_db_path();
    if db_path.exists() {
        fs::remove_file(&db_path)?;
    }
    let graph = SqliteGraph::open(&db_path)?;
    let harness = SynCompatHarness::build(graph)?;
    harness.run()?;
    println!("syncompat harness finished successfully");
    Ok(())
}

struct SynCompatHarness {
    client: BackendClient,
    module_id: i64,
    processor_id: i64,
}

impl SynCompatHarness {
    fn build(graph: SqliteGraph) -> Result<Self, Box<dyn std::error::Error>> {
        let file_id = graph.insert_entity(&GraphEntity {
            id: 0,
            kind: "file".into(),
            name: "FileA.rs".into(),
            file_path: Some("src/FileA.rs".into()),
            data: json!({"language": "rust"}),
        })?;
        let module_id = graph.insert_entity(&GraphEntity {
            id: 0,
            kind: "module".into(),
            name: "modA".into(),
            file_path: Some("src/modA".into()),
            data: json!({"visibility": "pub"}),
        })?;
        let processor_id = graph.insert_entity(&GraphEntity {
            id: 0,
            kind: "function".into(),
            name: "func_process".into(),
            file_path: Some("src/modA.rs".into()),
            data: json!({"sig": "fn func_process()"}),
        })?;
        let helper_id = graph.insert_entity(&GraphEntity {
            id: 0,
            kind: "function".into(),
            name: "func_helper".into(),
            file_path: Some("src/modA.rs".into()),
            data: json!({"sig": "fn func_helper()"}),
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

    fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.run_subgraph_checks()?;
        self.run_pipeline_checks()?;
        self.run_dsl_checks()?;
        self.run_safety_checks()?;
        self.run_bfs_checks()?;
        self.run_performance_smoke()?;
        Ok(())
    }

    fn run_subgraph_checks(&self) -> Result<(), Box<dyn std::error::Error>> {
        let request = SubgraphRequest {
            root: self.processor_id,
            depth: 2,
            allowed_edge_types: vec![],
            allowed_node_types: vec![],
        };
        let subgraph = extract_subgraph(self.client.backend(), request.clone())?;
        println!(
            "subgraph nodes={} edges={}",
            subgraph.nodes.len(),
            subgraph.edges.len()
        );
        let signature = structural_signature(&subgraph);
        println!("subgraph signature={signature}");
        Ok(())
    }

    fn run_pipeline_checks(&self) -> Result<(), Box<dyn std::error::Error>> {
        let pattern = PatternQuery {
            root: Some(NodeConstraint::kind("function")),
            legs: vec![PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some("calls".into()),
                constraint: Some(NodeConstraint::kind("function")),
            }],
        };
        let pipeline = ReasoningPipeline {
            steps: vec![ReasoningStep::Pattern(pattern)],
        };
        let result = self.client.run_pipeline(pipeline)?;
        println!("pipeline nodes={:?}", result.nodes);
        Ok(())
    }

    fn run_dsl_checks(&self) -> Result<(), Box<dyn std::error::Error>> {
        let dsl = "pattern calls filter type=function";
        let parsed = match parse_dsl(dsl) {
            DslResult::Pipeline(p) => p,
            other => panic!("expected pipeline, got {other:?}"),
        };
        let result = self.client.run_pipeline(parsed)?;
        println!("dsl pipeline nodes={:?}", result.nodes);
        Ok(())
    }

    fn run_safety_checks(&self) -> Result<(), Box<dyn std::error::Error>> {
        let report = run_safety_checks(self.client.backend().graph())?;
        println!(
            "safety: nodes={} edges={} issues={}",
            report.total_nodes,
            report.total_edges,
            report.orphan_edges
                + report.duplicate_edges
                + report.invalid_labels
                + report.invalid_properties
        );
        Ok(())
    }

    fn run_bfs_checks(&self) -> Result<(), Box<dyn std::error::Error>> {
        let bfs_nodes = bfs_neighbors(self.client.backend().graph(), self.module_id, 2)?;
        println!("bfs nodes={:?}", bfs_nodes);
        Ok(())
    }

    fn run_performance_smoke(&self) -> Result<(), Box<dyn std::error::Error>> {
        let request = SubgraphRequest {
            root: self.module_id,
            depth: 2,
            allowed_edge_types: vec![],
            allowed_node_types: vec![],
        };
        let start = Instant::now();
        for _ in 0..10 {
            let _ = extract_subgraph(self.client.backend(), request.clone())?;
        }
        println!("performance smoke: 10 subgraphs in {:?}", start.elapsed());
        Ok(())
    }
}

fn temp_db_path() -> PathBuf {
    std::env::temp_dir().join("sqlitegraph_syncompat.db")
}
