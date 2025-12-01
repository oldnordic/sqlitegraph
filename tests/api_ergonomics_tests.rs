use serde_json::json;
use sqlitegraph::{
    BackendClient,
    Label,
    NodeId,
    PipelineExplanation,
    PropertyKey,
    PropertyValue,
    api_ergonomics::explain_pipeline,
    backend::{EdgeSpec, NeighborQuery, NodeSpec, SqliteGraphBackend},
    index::{add_label, add_property},
    pattern::{NodeConstraint, PatternLeg, PatternQuery},
    pipeline::{ReasoningPipeline, ReasoningStep, run_pipeline},
};

fn setup_client() -> (BackendClient, NodeId, NodeId) {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let client = BackendClient::new(backend);
    let root = client
        .insert_node(NodeSpec { kind: "Fn".into(), name: "root".into(), file_path: None, data: json!({}) })
        .unwrap();
    let child = client
        .insert_node(NodeSpec { kind: "Fn".into(), name: "child".into(), file_path: None, data: json!({}) })
        .unwrap();
    client
        .insert_edge(EdgeSpec { from: root, to: child, edge_type: "CALLS".into(), data: json!({}) })
        .unwrap();
    add_label(client.backend().graph(), root, "Fn").unwrap();
    add_property(client.backend().graph(), child, "role", "leaf").unwrap();
    (client, NodeId(root), NodeId(child))
}

#[test]
fn test_get_node_matches_graph_access() {
    let (client, root, _) = setup_client();
    let entity = client.get_node(root).unwrap().unwrap();
    let low = client.backend().graph().get_entity(root.0).unwrap();
    assert_eq!(entity.name, low.name);
}

#[test]
fn test_neighbors_of_matches_low_level_query() {
    let (client, root, child) = setup_client();
    let neighbors = client.neighbors_of(root).unwrap();
    assert_eq!(neighbors, vec![child]);
}

#[test]
fn test_labeled_uses_index_layer() {
    let (client, root, _) = setup_client();
    let nodes = client.labeled(Label("Fn".into())).unwrap();
    assert!(nodes.contains(&root));
}

#[test]
fn test_with_property_uses_index_layer() {
    let (client, _, child) = setup_client();
    let nodes = client
        .with_property(PropertyKey("role".into()), PropertyValue("leaf".into()))
        .unwrap();
    assert_eq!(nodes, vec![child]);
}

#[test]
fn test_explain_pipeline_matches_pipeline_counts() {
    let (client, root, _) = setup_client();
    let pipeline = ReasoningPipeline {
        steps: vec![
            ReasoningStep::Pattern(PatternQuery {
                root: Some(NodeConstraint::kind("Fn")),
                legs: vec![PatternLeg {
                    direction: sqlitegraph::backend::BackendDirection::Outgoing,
                    edge_type: Some("CALLS".into()),
                    constraint: Some(NodeConstraint::kind("Fn")),
                }],
            }),
        ],
    };
    let explanation = client.explain_pipeline(pipeline.clone()).unwrap();
    let result = run_pipeline(client.backend(), &pipeline).unwrap();
    assert_eq!(explanation.node_counts_per_step.last().copied().unwrap_or(0), result.nodes.len());
}
