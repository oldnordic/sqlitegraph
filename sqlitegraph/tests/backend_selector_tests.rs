use serde_json::json;
use sqlitegraph::backend::{GraphBackend, NodeSpec};
use sqlitegraph::backend_selector::{BackendKind, GraphBackendFactory};

#[test]
fn test_backend_kind_from_env_defaults_to_sqlite() {
    unsafe {
        std::env::remove_var("GRAPH_BACKEND");
    }
    assert_eq!(BackendKind::from_env(), BackendKind::Sqlite);
}

#[test]
fn test_backend_kind_selects_sqlite_backend() {
    unsafe {
        std::env::set_var("GRAPH_BACKEND", "sqlite");
    }
    let backend = GraphBackendFactory::from_env().new_sqlite().unwrap();
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
