use serde_json::json;
use sqlitegraph::{
    backend::{BackendDirection, EdgeSpec, NeighborQuery, NodeSpec, SqliteGraphBackend},
    dual_runtime::DualRuntimeJob,
    migration::MigrationManager,
};

fn main() {
    if let Err(err) = run() {
        eprintln!("migration_flow error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let base = SqliteGraphBackend::in_memory()?;
    let shadow = SqliteGraphBackend::in_memory()?;
    let mut manager = MigrationManager::new(base, shadow);

    let entry = manager.insert_node(node("Entry"))?;
    let helper = manager.insert_node(node("Helper"))?;
    manager.insert_edge(edge(entry.base, helper.base, "CALLS"))?;

    let job = DualRuntimeJob {
        nodes: vec![entry.base, helper.base],
        neighbor_query: NeighborQuery {
            direction: BackendDirection::Outgoing,
            edge_type: None,
        },
        depth: 1,
    };
    let report = manager.shadow_read(&job)?;
    println!(
        "shadow_read matches={} diffs={} stats={:?}",
        report.matches,
        report.diffs.len(),
        manager.stats()
    );

    manager.cutover();
    println!(
        "cutover active={} active_nodes={}",
        manager.is_cutover(),
        manager.active_backend().entity_ids()?.len()
    );
    Ok(())
}

fn node(name: &str) -> NodeSpec {
    NodeSpec {
        kind: "Item".into(),
        name: name.into(),
        file_path: None,
        data: json!({"name": name}),
    }
}

fn edge(from: i64, to: i64, edge_type: &str) -> EdgeSpec {
    EdgeSpec {
        from,
        to,
        edge_type: edge_type.into(),
        data: json!({}),
    }
}
