use serde_json::json;
use sqlitegraph::{
    backend::{EdgeSpec, NodeSpec, SqliteGraphBackend},
    backend_client::{BackendClient, Constraint},
    pattern::{NodeConstraint, PatternLeg, PatternQuery},
    pipeline::{ReasoningPipeline, ReasoningStep, run_pipeline},
    subgraph::{SubgraphRequest, extract_subgraph},
};

fn sample_node(kind: &str, name: &str) -> NodeSpec {
    NodeSpec {
        kind: kind.into(),
        name: name.into(),
        file_path: None,
        data: json!({}),
    }
}

fn setup_client() -> BackendClient {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let client = BackendClient::new(backend);
    let root = client.insert_node(sample_node("Fn", "A")).unwrap();
    let mid = client.insert_node(sample_node("Fn", "B")).unwrap();
    let leaf = client.insert_node(sample_node("Fn", "C")).unwrap();
    let ty = client.insert_node(sample_node("Type", "T1")).unwrap();
    client
        .insert_edge(EdgeSpec {
            from: root,
            to: mid,
            edge_type: "CALLS".into(),
            data: json!({}),
        })
        .unwrap();
    client
        .insert_edge(EdgeSpec {
            from: mid,
            to: leaf,
            edge_type: "CALLS".into(),
            data: json!({}),
        })
        .unwrap();
    client
        .insert_edge(EdgeSpec {
            from: leaf,
            to: ty,
            edge_type: "USES".into(),
            data: json!({}),
        })
        .unwrap();
    // store properties via entity data for now
    let graph = client.backend().graph();
    sqlitegraph::index::add_label(graph, root, "Fn").unwrap();
    sqlitegraph::index::add_label(graph, mid, "Fn").unwrap();
    sqlitegraph::index::add_label(graph, leaf, "Fn").unwrap();
    sqlitegraph::index::add_property(graph, ty, "label", "DataVec").unwrap();
    client
}

fn pattern() -> PatternQuery {
    PatternQuery {
        root: Some(NodeConstraint::kind("Fn")),
        legs: vec![
            PatternLeg {
                direction: sqlitegraph::backend::BackendDirection::Outgoing,
                edge_type: Some("CALLS".into()),
                constraint: Some(NodeConstraint::kind("Fn")),
            },
            PatternLeg {
                direction: sqlitegraph::backend::BackendDirection::Outgoing,
                edge_type: Some("CALLS".into()),
                constraint: Some(NodeConstraint::kind("Fn")),
            },
        ],
    }
}

#[test]
fn test_pattern_vs_manual_match() {
    let client = setup_client();
    let matches = client.run_pattern(pattern()).unwrap();
    let manual = client
        .backend()
        .graph()
        .query()
        .pattern_matches(client.backend().entity_ids().unwrap()[0], &pattern())
        .unwrap();
    assert!(!matches.is_empty());
    assert_eq!(matches.len(), manual.len());
}

#[test]
fn test_pipeline_vs_manual_chain() {
    let client = setup_client();
    let pipeline = ReasoningPipeline {
        steps: vec![ReasoningStep::Pattern(pattern()), ReasoningStep::KHops(1)],
    };
    let direct = run_pipeline(client.backend(), &pipeline).unwrap();
    let via_client = client.run_pipeline(pipeline).unwrap();
    assert_eq!(direct.nodes, via_client.nodes);
}

#[test]
fn test_subgraph_vs_manual_bfs() {
    let client = setup_client();
    let req = SubgraphRequest {
        root: client.backend().entity_ids().unwrap()[0],
        depth: 2,
        allowed_edge_types: Vec::new(),
        allowed_node_types: Vec::new(),
    };
    let manual = extract_subgraph(client.backend(), req.clone()).unwrap();
    let via_client = client.subgraph(req).unwrap();
    assert_eq!(manual.nodes, via_client.nodes);
}

#[test]
fn test_label_queries() {
    let client = setup_client();
    let fn_entities = client.entity_by_label("Fn").unwrap();
    assert!(fn_entities.iter().all(|e| e.kind == "Fn"));
}

#[test]
fn test_property_queries() {
    let client = setup_client();
    let props = client.find_by_property("label", "DataVec").unwrap();
    assert_eq!(props.len(), 1);
    assert_eq!(props[0].name, "T1");
}

#[test]
fn test_constrained_shortest_path() {
    let client = setup_client();
    let ids = client.backend().entity_ids().unwrap();
    let constraint = Constraint {
        allowed_edge_types: vec!["CALLS".into()],
        allowed_node_types: vec!["Fn".into()],
    };
    let path = client
        .shortest_path_with_constraints(ids[0], ids[2], constraint)
        .unwrap();
    assert_eq!(path, vec![ids[0], ids[1], ids[2]]);
}

#[test]
fn test_deterministic_order() {
    let client = setup_client();
    let pattern_matches = client.run_pattern(pattern()).unwrap();
    let again = client.run_pattern(pattern()).unwrap();
    assert_eq!(pattern_matches, again);
}
