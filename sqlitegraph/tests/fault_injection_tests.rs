use serde_json::json;
use sqlitegraph::{
    GraphEntity,
    fault_injection::{FaultPoint, configure_fault, reset_faults},
    graph::SqliteGraph,
    graph_opt::{GraphEntityCreate, bulk_insert_entities},
    load_graph_from_reader, run_safety_checks,
};
use std::sync::{Mutex, OnceLock};

fn sample_entities() -> Vec<GraphEntityCreate> {
    vec![
        GraphEntityCreate {
            kind: "Module".into(),
            name: "core".into(),
            file_path: Some("core.rs".into()),
            data: json!({"size": 42}),
        },
        GraphEntityCreate {
            kind: "Fn".into(),
            name: "run".into(),
            file_path: Some("core.rs".into()),
            data: json!({"entry": true}),
        },
    ]
}

#[test]
fn bulk_insert_entities_rolls_back_on_fault() {
    let _guard = test_lock().lock().unwrap();
    reset_faults();
    configure_fault(FaultPoint::BulkInsertEntitiesBeforeCommit, 1);
    let graph = SqliteGraph::open_in_memory().unwrap();
    let err = bulk_insert_entities(&graph, &sample_entities()).unwrap_err();
    assert!(
        err.to_string().contains("fault injected"),
        "expected fault error, got {err:?}"
    );
    assert_eq!(graph.list_entity_ids().unwrap().len(), 0);

    reset_faults();
    bulk_insert_entities(&graph, &sample_entities()).unwrap();
    assert_eq!(graph.list_entity_ids().unwrap().len(), 2);
}

#[test]
fn recovery_load_preserves_previous_state_on_fault() {
    let _guard = test_lock().lock().unwrap();
    reset_faults();
    configure_fault(FaultPoint::RecoveryLoadBeforeCommit, 1);
    let graph = SqliteGraph::open_in_memory().unwrap();
    let original = graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Fn".into(),
            name: "original".into(),
            file_path: None,
            data: json!({}),
        })
        .unwrap();
    let payload = "{\"type\":\"entity\",\"id\":1,\"kind\":\"Module\",\"name\":\"core\",\"file_path\":null,\"data\":{}}\n".to_string();
    let err = load_graph_from_reader(&graph, payload.as_bytes()).unwrap_err();
    assert!(
        err.to_string().contains("fault injected"),
        "expected fault error, got {err:?}"
    );
    let ids = graph.list_entity_ids().unwrap();
    assert_eq!(ids, vec![original]);

    reset_faults();
    load_graph_from_reader(&graph, payload.as_bytes()).unwrap();
    let ids = graph.list_entity_ids().unwrap();
    assert_eq!(ids.len(), 1);
    let report = run_safety_checks(&graph).unwrap();
    assert_eq!(report.total_nodes, 1);
}

fn test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}
