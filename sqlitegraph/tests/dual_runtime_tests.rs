use serde_json::json;
use sqlitegraph::backend::{
    BackendDirection, EdgeSpec, GraphBackend, NeighborQuery, NodeSpec, SqliteGraphBackend,
};
use sqlitegraph::dual_runtime::{DualDiff, DualRuntime, DualRuntimeJob};
use sqlitegraph::dual_write::DualWriter;

fn sample_node(name: &str) -> NodeSpec {
    NodeSpec {
        kind: "Item".into(),
        name: name.into(),
        file_path: None,
        data: json!({"name": name}),
    }
}

fn setup_backends() -> (SqliteGraphBackend, SqliteGraphBackend, Vec<i64>) {
    let base = SqliteGraphBackend::in_memory().expect("base");
    let mirror = SqliteGraphBackend::in_memory().expect("mirror");
    let mut writer = DualWriter::new(base, mirror);
    let a = writer.insert_node(sample_node("A")).unwrap();
    let b = writer.insert_node(sample_node("B")).unwrap();
    let c = writer.insert_node(sample_node("C")).unwrap();
    writer
        .insert_edge(EdgeSpec {
            from: a.base,
            to: b.base,
            edge_type: "CALLS".into(),
            data: json!({}),
        })
        .unwrap();
    writer
        .insert_edge(EdgeSpec {
            from: b.base,
            to: c.base,
            edge_type: "CALLS".into(),
            data: json!({}),
        })
        .unwrap();
    writer
        .insert_edge(EdgeSpec {
            from: a.base,
            to: c.base,
            edge_type: "USES".into(),
            data: json!({}),
        })
        .unwrap();
    let (base_backend, mirror_backend, _) = writer.into_backends();
    (base_backend, mirror_backend, vec![a.base, b.base, c.base])
}

#[test]
fn test_dual_runtime_reports_all_matches() {
    let (base, mirror, ids) = setup_backends();
    let runtime = DualRuntime::new(base, mirror);
    let job = DualRuntimeJob {
        nodes: vec![ids[0], ids[1]],
        neighbor_query: NeighborQuery {
            direction: BackendDirection::Outgoing,
            edge_type: None,
        },
        depth: 2,
    };
    let report = runtime.run(&job).expect("runtime");
    assert_eq!(report.total, 2);
    assert_eq!(report.matches, 2);
    assert!(report.diffs.is_empty());
    assert!(!report.log.is_empty());
}

#[test]
fn test_dual_runtime_detects_neighbor_mismatch() {
    let (base, mirror, ids) = setup_backends();
    base.insert_edge(EdgeSpec {
        from: ids[1],
        to: ids[0],
        edge_type: "CALLS".into(),
        data: json!({}),
    })
    .unwrap();
    let runtime = DualRuntime::new(base, mirror);
    let job = DualRuntimeJob {
        nodes: vec![ids[1]],
        neighbor_query: NeighborQuery {
            direction: BackendDirection::Outgoing,
            edge_type: None,
        },
        depth: 1,
    };
    let report = runtime.run(&job).expect("runtime");
    assert_eq!(report.matches, 0);
    assert_eq!(report.diffs.len(), 1);
    match &report.diffs[0].diff {
        DualDiff::Neighbors { base, other } => {
            assert_eq!(base, &vec![ids[0], ids[2]]);
            assert_eq!(other, &vec![ids[2]]);
        }
        _ => panic!("expected neighbor diff"),
    }
    assert!(report.log.iter().any(|entry| entry.contains("node")));
}
