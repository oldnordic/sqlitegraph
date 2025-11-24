use serde_json::json;
use sqlitegraph::backend::{
    BackendDirection, EdgeSpec, NeighborQuery, NodeSpec, SqliteGraphBackend,
};
use sqlitegraph::client::{BackendClient, CommandLineConfig};

fn sample_node(name: &str) -> NodeSpec {
    NodeSpec {
        kind: "Item".into(),
        name: name.into(),
        file_path: None,
        data: json!({}),
    }
}

#[test]
fn test_backend_client_executes_sequence() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let client = BackendClient::new(backend);
    let a = client.insert_node(sample_node("A")).unwrap();
    let b = client.insert_node(sample_node("B")).unwrap();
    client
        .insert_edge(EdgeSpec {
            from: a,
            to: b,
            edge_type: "CALLS".into(),
            data: json!({}),
        })
        .unwrap();
    let neighbors = client
        .neighbors(
            a,
            NeighborQuery {
                direction: BackendDirection::Outgoing,
                edge_type: None,
            },
        )
        .unwrap();
    assert_eq!(neighbors, vec![b]);
}

#[test]
fn test_command_line_config_parses_args() {
    let config =
        CommandLineConfig::from_args(&["sqlitegraph", "--backend", "sqlite", "--db", "memory"])
            .unwrap();
    assert_eq!(config.backend, "sqlite");
    assert_eq!(config.database, "memory");
}
