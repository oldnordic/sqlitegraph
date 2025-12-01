use serde_json::json;
use sqlitegraph::{
    backend::{BackendDirection, SqliteGraphBackend},
    graph::{GraphEdge, GraphEntity, SqliteGraph},
    pattern::{NodeConstraint, PatternLeg, PatternQuery},
    pipeline::{ReasoningPipeline, ReasoningScoreConfig, ReasoningStep, run_pipeline},
};

fn sample_graph() -> (SqliteGraphBackend, Vec<i64>) {
    let graph = SqliteGraph::open_in_memory().expect("graph");
    let f1 = graph.insert_entity(&GraphEntity { id: 0, kind: "Fn".into(), name: "A".into(), file_path: None, data: json!({}) }).unwrap();
    let f2 = graph.insert_entity(&GraphEntity { id: 0, kind: "Fn".into(), name: "B".into(), file_path: None, data: json!({}) }).unwrap();
    let f3 = graph.insert_entity(&GraphEntity { id: 0, kind: "Fn".into(), name: "C".into(), file_path: None, data: json!({}) }).unwrap();
    let t1 = graph.insert_entity(&GraphEntity { id: 0, kind: "Type".into(), name: "T1".into(), file_path: None, data: json!({}) }).unwrap();
    let t2 = graph.insert_entity(&GraphEntity { id: 0, kind: "Type".into(), name: "T2".into(), file_path: None, data: json!({}) }).unwrap();
    graph.insert_edge(&GraphEdge { id: 0, from_id: f1, to_id: f2, edge_type: "CALLS".into(), data: json!({}) }).unwrap();
    graph.insert_edge(&GraphEdge { id: 0, from_id: f2, to_id: f3, edge_type: "CALLS".into(), data: json!({}) }).unwrap();
    graph.insert_edge(&GraphEdge { id: 0, from_id: f3, to_id: t1, edge_type: "USES".into(), data: json!({}) }).unwrap();
    graph.insert_edge(&GraphEdge { id: 0, from_id: t1, to_id: t2, edge_type: "USES".into(), data: json!({}) }).unwrap();
    (SqliteGraphBackend::from_graph(graph), vec![f1, f2, f3, t1, t2])
}

fn pattern() -> PatternQuery {
    PatternQuery {
        root: Some(NodeConstraint::kind("Fn")),
        legs: vec![
            PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some("CALLS".into()),
                constraint: Some(NodeConstraint::kind("Fn")),
            },
        ],
    }
}

#[test]
fn test_pipeline_pattern_chain_order() {
    let (backend, ids) = sample_graph();
    let pipeline = ReasoningPipeline {
        steps: vec![ReasoningStep::Pattern(pattern())],
    };
    let result = run_pipeline(&backend, &pipeline).expect("pipeline");
    assert_eq!(result.nodes, vec![ids[0], ids[1], ids[2]]);
}

#[test]
fn test_pipeline_khop_chain_order() {
    let (backend, ids) = sample_graph();
    let pipeline = ReasoningPipeline {
        steps: vec![ReasoningStep::Pattern(pattern()), ReasoningStep::KHops(1)],
    };
    let result = run_pipeline(&backend, &pipeline).expect("pipeline");
    assert_eq!(result.nodes, vec![ids[0], ids[1], ids[2], ids[3]]);
}

#[test]
fn test_pipeline_filter_application() {
    let (backend, ids) = sample_graph();
    let pipeline = ReasoningPipeline {
        steps: vec![
            ReasoningStep::Pattern(pattern()),
            ReasoningStep::KHops(1),
            ReasoningStep::Filter(NodeConstraint::kind("Fn")),
        ],
    };
    let result = run_pipeline(&backend, &pipeline).expect("pipeline");
    assert_eq!(result.nodes, vec![ids[0], ids[1], ids[2]]);
}

#[test]
fn test_pipeline_scoring_application() {
    let (backend, _) = sample_graph();
    let pipeline = ReasoningPipeline {
        steps: vec![
            ReasoningStep::Pattern(pattern()),
            ReasoningStep::Score(ReasoningScoreConfig { hop_depth: 1, degree_weight: 1.0 }),
        ],
    };
    let result = run_pipeline(&backend, &pipeline).expect("pipeline");
    assert_eq!(result.scores.len(), result.nodes.len());
    assert!(result.scores.iter().all(|(_, score)| *score >= 1.0));
}

#[test]
fn test_pipeline_deterministic_output() {
    let (backend, _) = sample_graph();
    let pipeline = ReasoningPipeline {
        steps: vec![ReasoningStep::Pattern(pattern()), ReasoningStep::KHops(2)],
    };
    let first = run_pipeline(&backend, &pipeline).expect("pipeline");
    let second = run_pipeline(&backend, &pipeline).expect("pipeline");
    assert_eq!(first.nodes, second.nodes);
}
