use serde_json::json;
use sqlitegraph::backend::GraphBackend;
use sqlitegraph::{
    backend::{BackendDirection, EdgeSpec, NodeSpec, SqliteGraphBackend},
    explain_pipeline,
    pattern::{NodeConstraint, PatternLeg, PatternQuery},
    pipeline::{ReasoningPipeline, ReasoningScoreConfig, ReasoningStep, run_pipeline},
};

#[test]
fn pattern_only_pipeline_collects_nodes() {
    let (backend, nodes) = pipeline_fixture();
    let pipeline = ReasoningPipeline {
        steps: vec![ReasoningStep::Pattern(demo_pattern())],
    };
    let result = run_pipeline(&backend, &pipeline).unwrap();
    assert_eq!(
        result.nodes,
        vec![
            nodes.entry,
            nodes.helper,
            nodes.aux,
            nodes.module_a,
            nodes.module_b
        ]
    );
    assert!(result.scores.is_empty());
}

#[test]
fn khop_only_pipeline_returns_empty() {
    let (backend, _) = pipeline_fixture();
    let pipeline = ReasoningPipeline {
        steps: vec![ReasoningStep::KHops(2)],
    };
    let result = run_pipeline(&backend, &pipeline).unwrap();
    assert!(result.nodes.is_empty());
    assert!(result.scores.is_empty());
}

#[test]
fn pattern_then_filter_limits_candidates() {
    let (backend, nodes) = pipeline_fixture();
    let pipeline = ReasoningPipeline {
        steps: vec![
            ReasoningStep::Pattern(demo_pattern()),
            ReasoningStep::Filter(NodeConstraint::kind("Fn")),
        ],
    };
    let result = run_pipeline(&backend, &pipeline).unwrap();
    assert_eq!(result.nodes, vec![nodes.entry, nodes.helper, nodes.aux]);
    assert!(result.scores.is_empty());
}

#[test]
fn filter_then_score_orders_by_rank() {
    let (backend, nodes) = pipeline_fixture();
    let pipeline = ReasoningPipeline {
        steps: vec![
            ReasoningStep::Pattern(demo_pattern()),
            ReasoningStep::Filter(NodeConstraint::kind("Fn")),
            ReasoningStep::Score(ReasoningScoreConfig {
                hop_depth: 1,
                degree_weight: 0.5,
            }),
        ],
    };
    let result = run_pipeline(&backend, &pipeline).unwrap();
    assert_eq!(result.nodes, vec![nodes.helper, nodes.entry, nodes.aux]);
    assert_eq!(
        result.scores,
        vec![(nodes.helper, 3.5), (nodes.entry, 3.0), (nodes.aux, 2.0),],
    );
}

#[test]
fn full_pipeline_reports_explanation() {
    let (backend, nodes) = pipeline_fixture();
    let pipeline = ReasoningPipeline {
        steps: vec![
            ReasoningStep::Pattern(demo_pattern()),
            ReasoningStep::KHops(1),
            ReasoningStep::Filter(NodeConstraint::kind("Module")),
            ReasoningStep::Score(ReasoningScoreConfig {
                hop_depth: 1,
                degree_weight: 1.0,
            }),
        ],
    };
    let result = run_pipeline(&backend, &pipeline).unwrap();
    assert_eq!(result.nodes, vec![nodes.module_a, nodes.module_b]);
    assert_eq!(
        result.scores,
        vec![(nodes.module_a, 3.0), (nodes.module_b, 3.0)],
    );

    let explanation = explain_pipeline(&backend, &pipeline).unwrap();
    assert_eq!(
        explanation.steps_summary,
        vec!["pattern 2 legs", "khop depth=1", "filter", "score"],
    );
    assert_eq!(explanation.node_counts_per_step, vec![5, 7, 2, 2]);
    assert_eq!(
        explanation.filters_applied,
        vec!["filter kind=Some(\"Module\")".to_string()],
    );
    assert_eq!(
        explanation.scoring_notes,
        vec!["score hop_depth=1 degree_weight=1".to_string()],
    );
}

struct PipelineNodes {
    entry: i64,
    helper: i64,
    aux: i64,
    module_a: i64,
    module_b: i64,
    _stray: i64,
    _type_leaf: i64,
}

fn pipeline_fixture() -> (SqliteGraphBackend, PipelineNodes) {
    let backend = SqliteGraphBackend::in_memory().unwrap();
    let entry = backend.insert_node(fn_node("entry")).unwrap();
    let helper = backend.insert_node(fn_node("helper")).unwrap();
    let aux = backend.insert_node(fn_node("aux")).unwrap();
    let module_a = backend.insert_node(module_node("alpha")).unwrap();
    let module_b = backend.insert_node(module_node("beta")).unwrap();
    let stray = backend.insert_node(fn_node("stray")).unwrap();
    let type_leaf = backend.insert_node(type_node("leaf")).unwrap();

    backend
        .insert_edge(EdgeSpec {
            from: entry,
            to: helper,
            edge_type: "CALLS".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: helper,
            to: module_a,
            edge_type: "USES".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: entry,
            to: aux,
            edge_type: "CALLS".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: aux,
            to: module_b,
            edge_type: "USES".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: module_a,
            to: type_leaf,
            edge_type: "DEPENDS".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: module_b,
            to: type_leaf,
            edge_type: "DEPENDS".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: helper,
            to: stray,
            edge_type: "CALLS".into(),
            data: json!({}),
        })
        .unwrap();

    (
        backend,
        PipelineNodes {
            entry,
            helper,
            aux,
            module_a,
            module_b,
            _stray: stray,
            _type_leaf: type_leaf,
        },
    )
}

fn fn_node(name: &str) -> NodeSpec {
    NodeSpec {
        kind: "Fn".into(),
        name: name.into(),
        file_path: None,
        data: json!({}),
    }
}

fn module_node(name: &str) -> NodeSpec {
    NodeSpec {
        kind: "Module".into(),
        name: name.into(),
        file_path: None,
        data: json!({}),
    }
}

fn type_node(name: &str) -> NodeSpec {
    NodeSpec {
        kind: "Type".into(),
        name: name.into(),
        file_path: None,
        data: json!({}),
    }
}

fn demo_pattern() -> PatternQuery {
    PatternQuery {
        root: Some(NodeConstraint::kind("Fn")),
        legs: vec![
            PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some("CALLS".into()),
                constraint: Some(NodeConstraint::kind("Fn")),
            },
            PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some("USES".into()),
                constraint: Some(NodeConstraint::kind("Module")),
            },
        ],
    }
}
