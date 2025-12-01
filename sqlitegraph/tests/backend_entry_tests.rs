use serde_json::json;
use sqlitegraph::backend::{BackendDirection, EdgeSpec, GraphBackend, NeighborQuery, NodeSpec};
use sqlitegraph::backend_selector::{BackendKind, GraphBackendFactory};
use sqlitegraph::dual_orchestrator::DualGraphHarness;

#[test]
fn test_backend_factory_returns_sqlite_backend() {
    let factory = GraphBackendFactory::new(BackendKind::Sqlite);
    let backend = factory.new_sqlite().expect("backend");
    let node = backend
        .insert_node(NodeSpec {
            kind: "Node".into(),
            name: "A".into(),
            file_path: None,
            data: json!({}),
        })
        .unwrap();
    assert!(node > 0);
}

#[test]
fn test_harness_entrypoint_uses_factory_backends() {
    let factory = GraphBackendFactory::new(BackendKind::Sqlite);
    let base = factory.new_sqlite().unwrap();
    let other = factory.new_sqlite().unwrap();
    let a = base
        .insert_node(NodeSpec {
            kind: "Node".into(),
            name: "A".into(),
            file_path: None,
            data: json!({}),
        })
        .unwrap();
    let b = base
        .insert_node(NodeSpec {
            kind: "Node".into(),
            name: "B".into(),
            file_path: None,
            data: json!({}),
        })
        .unwrap();
    base.insert_edge(EdgeSpec {
        from: a,
        to: b,
        edge_type: "LINK".into(),
        data: json!({}),
    })
    .unwrap();
    let other_a = other
        .insert_node(NodeSpec {
            kind: "Node".into(),
            name: "A".into(),
            file_path: None,
            data: json!({}),
        })
        .unwrap();
    let other_b = other
        .insert_node(NodeSpec {
            kind: "Node".into(),
            name: "B".into(),
            file_path: None,
            data: json!({}),
        })
        .unwrap();
    other
        .insert_edge(EdgeSpec {
            from: other_a,
            to: other_b,
            edge_type: "LINK".into(),
            data: json!({}),
        })
        .unwrap();

    let harness = DualGraphHarness::new(base, other);
    let diff = harness
        .compare_neighbors(
            a,
            NeighborQuery {
                direction: BackendDirection::Outgoing,
                edge_type: Some("LINK".into()),
            },
        )
        .unwrap();
    assert!(matches!(
        diff,
        sqlitegraph::dual_orchestrator::HarnessDiff::Match
    ));
}
