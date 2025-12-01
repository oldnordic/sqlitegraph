use serde_json::json;
use sqlitegraph::{
    backend::{BackendDirection, EdgeSpec, NodeSpec, SqliteGraphBackend},
    pattern::{NodeConstraint, PatternLeg, PatternQuery},
    pipeline::{ReasoningPipeline, ReasoningStep},
    subgraph::{SubgraphRequest, extract_subgraph},
};

fn backend() -> SqliteGraphBackend {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let insert = |client: &SqliteGraphBackend, kind: &str, name: &str| -> i64 {
        client
            .insert_node(NodeSpec {
                kind: kind.into(),
                name: name.into(),
                file_path: None,
                data: json!({}),
            })
            .unwrap()
    };
    let root = insert(&backend, "Fn", "root");
    let mid = insert(&backend, "Fn", "mid");
    let tail = insert(&backend, "Fn", "tail");
    backend
        .insert_edge(EdgeSpec {
            from: root,
            to: mid,
            edge_type: "CALLS".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: mid,
            to: tail,
            edge_type: "CALLS".into(),
            data: json!({}),
        })
        .unwrap();
    backend
}

#[test]
fn test_pipeline_and_subgraph_align() {
    let backend = backend();
    let pipeline = ReasoningPipeline {
        steps: vec![
            ReasoningStep::Pattern(PatternQuery {
                root: Some(NodeConstraint::kind("Fn")),
                legs: vec![PatternLeg {
                    direction: BackendDirection::Outgoing,
                    edge_type: Some("CALLS".into()),
                    constraint: Some(NodeConstraint::kind("Fn")),
                }],
            }),
            ReasoningStep::KHops(1),
        ],
    };
    let pipeline_result = sqlitegraph::pipeline::run_pipeline(&backend, &pipeline).unwrap();
    let req = SubgraphRequest {
        root: backend.entity_ids().unwrap()[0],
        depth: 2,
        allowed_edge_types: vec!["CALLS".into()],
        allowed_node_types: vec!["Fn".into()],
    };
    let subgraph = extract_subgraph(&backend, req).unwrap();
    for node in pipeline_result.nodes {
        assert!(subgraph.nodes.contains(&node));
    }
}

#[test]
fn test_pipeline_deterministic_with_subgraph_signature() {
    let backend = backend();
    let pipeline = ReasoningPipeline {
        steps: vec![
            ReasoningStep::Pattern(PatternQuery {
                root: None,
                legs: vec![PatternLeg {
                    direction: BackendDirection::Outgoing,
                    edge_type: Some("CALLS".into()),
                    constraint: None,
                }],
            }),
        ],
    };
    let first = sqlitegraph::pipeline::run_pipeline(&backend, &pipeline).unwrap();
    let second = sqlitegraph::pipeline::run_pipeline(&backend, &pipeline).unwrap();
    assert_eq!(first.nodes, second.nodes);
    let req = SubgraphRequest {
        root: backend.entity_ids().unwrap()[0],
        depth: 1,
        allowed_edge_types: Vec::new(),
        allowed_node_types: Vec::new(),
    };
    let sig = sqlitegraph::subgraph::structural_signature(&extract_subgraph(&backend, req).unwrap());
    assert!(!sig.is_empty());
}
