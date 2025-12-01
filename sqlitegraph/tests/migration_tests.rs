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

#[test]
fn dual_write_nodes_sync_both_backends() {
    let mut manager = manager();
    let ids = manager.insert_node(sample_node("Dual")).unwrap();
    let base_entity = manager.base_backend().graph().get_entity(ids.base).unwrap();
    let shadow_entity = manager
        .shadow_backend()
        .graph()
        .get_entity(ids.shadow)
        .unwrap();
    assert_eq!(base_entity.name, shadow_entity.name);
}

#[test]
fn dual_write_edges_respect_shadow_ids() {
    let mut manager = manager();
    // skew shadow ids so they diverge from base ids
    manager
        .shadow_backend()
        .insert_node(sample_node("seed"))
        .unwrap();
    let a = manager.insert_node(sample_node("A")).unwrap();
    let b = manager.insert_node(sample_node("B")).unwrap();
    manager
        .insert_edge(sample_edge(a.base, b.base, "CALLS"))
        .unwrap();
    let shadow_neighbors = manager
        .shadow_backend()
        .neighbors(
            a.shadow,
            NeighborQuery {
                direction: BackendDirection::Outgoing,
                edge_type: None,
            },
        )
        .unwrap();
    assert_eq!(shadow_neighbors, vec![b.shadow]);
}

#[test]
fn cutover_is_idempotent_and_safe() {
    let mut manager = manager();
    assert!(!manager.is_cutover());
    manager.cutover();
    manager.cutover();
    assert!(manager.is_cutover());
    let stats_before = manager.stats().clone();
    manager.cutover();
    assert_eq!(manager.stats(), &stats_before);
}

#[test]
fn migration_manager_handles_high_load_batches() {
    let mut manager = manager();
    let mut ids = Vec::new();
    for idx in 0..100 {
        let node = manager
            .insert_node(sample_node(&format!("N{idx}")))
            .unwrap();
        ids.push(node);
    }
    for i in 0..100 {
        let a = ids[i].base;
        let b = ids[(i + 1) % ids.len()].base;
        let c = ids[(i + 2) % ids.len()].base;
        let d = ids[(i + 3) % ids.len()].base;
        manager.insert_edge(sample_edge(a, b, "CALLS")).unwrap();
        manager.insert_edge(sample_edge(a, c, "USES")).unwrap();
        manager.insert_edge(sample_edge(a, d, "DEPENDS")).unwrap();
    }
    assert_eq!(manager.stats().nodes_written, 100);
    assert_eq!(manager.stats().edges_written, 300);

    let job = DualRuntimeJob {
        nodes: ids.iter().take(10).map(|id| id.base).collect(),
        neighbor_query: NeighborQuery {
            direction: BackendDirection::Outgoing,
            edge_type: None,
        },
        depth: 2,
    };
    let report = manager.shadow_read(&job).unwrap();
    assert_eq!(report.matches, 10);
    assert!(report.diffs.is_empty());
}
