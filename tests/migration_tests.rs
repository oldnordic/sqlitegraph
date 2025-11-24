use serde_json::json;
use sqlitegraph::backend::{
    BackendDirection, EdgeSpec, GraphBackend, NeighborQuery, NodeSpec, SqliteGraphBackend,
};
use sqlitegraph::dual_runtime::DualRuntimeJob;
use sqlitegraph::migration::{MigrationManager, MigrationStats};

fn sample_node(name: &str) -> NodeSpec {
    NodeSpec {
        kind: "Item".into(),
        name: name.into(),
        file_path: None,
        data: json!({"name": name}),
    }
}

fn sample_edge(from: i64, to: i64, edge_type: &str) -> EdgeSpec {
    EdgeSpec {
        from,
        to,
        edge_type: edge_type.into(),
        data: json!({}),
    }
}

fn manager() -> MigrationManager {
    let base = SqliteGraphBackend::in_memory().expect("base");
    let shadow = SqliteGraphBackend::in_memory().expect("shadow");
    MigrationManager::new(base, shadow)
}

#[test]
fn test_migration_manager_dual_write_and_shadow_read() {
    let mut manager = manager();
    let a = manager.insert_node(sample_node("A")).unwrap();
    let b = manager.insert_node(sample_node("B")).unwrap();
    manager
        .insert_edge(sample_edge(a.base, b.base, "CALLS"))
        .unwrap();

    let job = DualRuntimeJob {
        nodes: vec![a.base, b.base],
        neighbor_query: NeighborQuery {
            direction: BackendDirection::Outgoing,
            edge_type: None,
        },
        depth: 1,
    };
    let report = manager.shadow_read(&job).expect("shadow read");
    assert_eq!(report.total, 2);
    assert_eq!(report.matches, 2);
    assert!(report.diffs.is_empty());
    assert_eq!(
        manager.stats(),
        &MigrationStats {
            nodes_written: 2,
            edges_written: 1
        }
    );
}

#[test]
fn test_shadow_read_detects_mismatch() {
    let mut manager = manager();
    let a = manager.insert_node(sample_node("A")).unwrap();
    let b = manager.insert_node(sample_node("B")).unwrap();
    manager
        .insert_edge(sample_edge(a.base, b.base, "CALLS"))
        .unwrap();
    manager
        .shadow_backend()
        .insert_edge(sample_edge(b.shadow, a.shadow, "CALLS"))
        .unwrap();

    let job = DualRuntimeJob {
        nodes: vec![b.base],
        neighbor_query: NeighborQuery {
            direction: BackendDirection::Outgoing,
            edge_type: None,
        },
        depth: 1,
    };
    let report = manager.shadow_read(&job).expect("shadow read");
    assert_eq!(report.matches, 0);
    assert_eq!(report.diffs.len(), 1);
}

#[test]
fn test_cutover_switches_active_backend() {
    let mut manager = manager();
    let ids = manager.insert_node(sample_node("Primary")).unwrap();
    assert!(!manager.is_cutover());
    manager.cutover();
    assert!(manager.is_cutover());
    let active = manager.active_backend();
    let fetched = active.graph().get_entity(ids.shadow).unwrap();
    assert_eq!(fetched.name, "Primary");
}
