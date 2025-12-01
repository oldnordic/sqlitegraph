use serde_json::json;
use sqlitegraph::backend::{
    BackendDirection, EdgeSpec, GraphBackend, NeighborQuery, NodeSpec, SqliteGraphBackend,
};
use sqlitegraph::backend_selector::{BackendKind, GraphBackendFactory};
use sqlitegraph::dual_runner::{DualRunConfig, DualRunResult, run_dual_check};

fn seed(factory: &GraphBackendFactory, edges: &[(usize, usize)]) -> (SqliteGraphBackend, Vec<i64>) {
    let backend = factory.new_sqlite().unwrap();
    let nodes: Vec<_> = (0..3)
        .map(|idx| {
            backend
                .insert_node(NodeSpec {
                    kind: "Node".into(),
                    name: format!("Node{idx}"),
                    file_path: None,
                    data: json!({ "idx": idx }),
                })
                .unwrap()
        })
        .collect();
    for &(from, to) in edges {
        backend
            .insert_edge(EdgeSpec {
                from: nodes[from],
                to: nodes[to],
                edge_type: "LINK".into(),
                data: json!({}),
            })
            .unwrap();
    }
    (backend, nodes)
}

#[test]
fn test_dual_run_reports_match() {
    let factory = GraphBackendFactory::new(BackendKind::Sqlite);
    let (base, nodes) = seed(&factory, &[(0, 1)]);
    let (other, _) = seed(&factory, &[(0, 1)]);
    let config = DualRunConfig {
        node: nodes[0],
        query: NeighborQuery {
            direction: BackendDirection::Outgoing,
            edge_type: Some("LINK".into()),
        },
        depth: 1,
    };
    let result = run_dual_check(base, other, config).unwrap();
    assert_eq!(result, DualRunResult::Match);
}

#[test]
fn test_dual_run_reports_mismatch() {
    let factory = GraphBackendFactory::new(BackendKind::Sqlite);
    let (base, nodes) = seed(&factory, &[(0, 1), (0, 2)]);
    let (other, _) = seed(&factory, &[(0, 1)]);
    let config = DualRunConfig {
        node: nodes[0],
        query: NeighborQuery {
            direction: BackendDirection::Outgoing,
            edge_type: Some("LINK".into()),
        },
        depth: 1,
    };
    let result = run_dual_check(base, other, config).unwrap();
    assert!(matches!(result, DualRunResult::Mismatch { .. }));
}
