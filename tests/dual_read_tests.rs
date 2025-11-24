use sqlitegraph::backend::{
    BackendDirection, EdgeSpec, GraphBackend, NeighborQuery, NodeSpec, SqliteGraphBackend,
};
use sqlitegraph::dual_read::{DualReadResult, DualReader, compare_adjacent};

#[test]
fn test_compare_adjacent_detects_mismatches() {
    let base = vec![1, 2, 3];
    let other = vec![1, 4, 3];
    let diff = compare_adjacent(&base, &other);
    assert!(matches!(diff, DualReadResult::Mismatch { .. }));
}

#[test]
fn test_compare_adjacent_reports_match() {
    let base = vec![1, 2, 3];
    let other = vec![1, 2, 3];
    assert_eq!(compare_adjacent(&base, &other), DualReadResult::Match);
}

fn seed_backend(edges: &[(usize, usize)]) -> (SqliteGraphBackend, Vec<i64>) {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let nodes: Vec<_> = (0..3)
        .map(|idx| {
            backend
                .insert_node(NodeSpec {
                    kind: "Node".into(),
                    name: format!("Node{idx}"),
                    file_path: None,
                    data: serde_json::json!({ "idx": idx }),
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
                data: serde_json::json!({}),
            })
            .unwrap();
    }
    (backend, nodes)
}

#[test]
fn test_dual_reader_neighbors_match() {
    let (base, base_nodes) = seed_backend(&[(0, 1), (0, 2)]);
    let (other, _) = seed_backend(&[(0, 1), (0, 2)]);
    let reader = DualReader::new(base, other);
    let result = reader
        .compare_neighbors(
            base_nodes[0],
            NeighborQuery {
                direction: BackendDirection::Outgoing,
                edge_type: Some("LINK".into()),
            },
        )
        .unwrap();
    assert_eq!(result, DualReadResult::Match);
}

#[test]
fn test_dual_reader_neighbors_detect_mismatch() {
    let (base, base_nodes) = seed_backend(&[(0, 1), (0, 2)]);
    let (other, _) = seed_backend(&[(0, 1)]);
    let reader = DualReader::new(base, other);
    let result = reader
        .compare_neighbors(
            base_nodes[0],
            NeighborQuery {
                direction: BackendDirection::Outgoing,
                edge_type: Some("LINK".into()),
            },
        )
        .unwrap();
    assert!(matches!(result, DualReadResult::Mismatch { .. }));
}

#[test]
fn test_dual_reader_bfs_compare() {
    let (base, base_nodes) = seed_backend(&[(0, 1), (1, 2)]);
    let (other, _) = seed_backend(&[(0, 1)]);
    let reader = DualReader::new(base, other);
    let match_result = reader.compare_bfs(base_nodes[0], 1).unwrap();
    assert_eq!(match_result, DualReadResult::Match);
    let mismatch = reader.compare_bfs(base_nodes[0], 2).unwrap();
    assert!(matches!(mismatch, DualReadResult::Mismatch { .. }));
}
