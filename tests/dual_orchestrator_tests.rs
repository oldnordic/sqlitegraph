use serde_json::json;
use sqlitegraph::backend::{
    BackendDirection, EdgeSpec, GraphBackend, NeighborQuery, NodeSpec, SqliteGraphBackend,
};
use sqlitegraph::dual_orchestrator::{DualGraphHarness, HarnessDiff};

fn seed(nodes: usize, edges: &[(usize, usize)]) -> (SqliteGraphBackend, Vec<i64>) {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let created: Vec<_> = (0..nodes)
        .map(|i| {
            backend
                .insert_node(NodeSpec {
                    kind: "Node".into(),
                    name: format!("N{i}"),
                    file_path: None,
                    data: json!({ "i": i }),
                })
                .unwrap()
        })
        .collect();
    for &(from, to) in edges {
        backend
            .insert_edge(EdgeSpec {
                from: created[from],
                to: created[to],
                edge_type: "LINK".into(),
                data: json!({}),
            })
            .unwrap();
    }
    (backend, created)
}

#[test]
fn test_harness_reports_no_diff_when_backends_match() {
    let (base, ids) = seed(3, &[(0, 1), (0, 2)]);
    let (other, _) = seed(3, &[(0, 1), (0, 2)]);
    let harness = DualGraphHarness::new(base, other);
    let diff = harness
        .compare_neighbors(
            ids[0],
            NeighborQuery {
                direction: BackendDirection::Outgoing,
                edge_type: Some("LINK".into()),
            },
        )
        .unwrap();
    assert_eq!(diff, HarnessDiff::Match);
}

#[test]
fn test_harness_reports_diff_when_outputs_differ() {
    let (base, ids) = seed(3, &[(0, 1), (0, 2)]);
    let (other, _) = seed(3, &[(0, 1)]);
    let harness = DualGraphHarness::new(base, other);
    let diff = harness
        .compare_neighbors(
            ids[0],
            NeighborQuery {
                direction: BackendDirection::Outgoing,
                edge_type: Some("LINK".into()),
            },
        )
        .unwrap();
    assert!(matches!(diff, HarnessDiff::Mismatch { .. }));
}
