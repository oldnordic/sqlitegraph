use assert_cmd::Command;
use serde_json::json;
use sqlitegraph::graph::{GraphEdge, GraphEntity, SqliteGraph};
use std::path::PathBuf;

#[test]
fn test_cli_exits_with_success_on_help() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sqlitegraph"));
    cmd.arg("--help");
    cmd.assert().success();
}

#[test]
fn test_cli_status_command() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sqlitegraph"));
    cmd.args(["--command", "status"]);
    cmd.assert().success();
}

#[test]
fn test_cli_subgraph_command_with_db() {
    let path = temp_db_path("sqlitegraph_cli.db");
    let root = prepare_db(&path);
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sqlitegraph"));
    cmd.args([
        "--db",
        path.to_str().unwrap(),
        "--command",
        "subgraph",
        "--root",
        &root.to_string(),
        "--depth",
        "1",
    ]);
    cmd.assert().success();
}

#[test]
fn test_cli_dsl_parse_command() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sqlitegraph"));
    cmd.args(["--command", "dsl-parse", "--input", "CALLS->USES"]);
    cmd.assert().success();
}

fn temp_db_path(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(name);
    let _ = std::fs::remove_file(&path);
    path
}

fn prepare_db(path: &PathBuf) -> i64 {
    let graph = SqliteGraph::open(path).expect("graph");
    let a = graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Fn".into(),
            name: "root".into(),
            file_path: None,
            data: json!({}),
        })
        .unwrap();
    let b = graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Fn".into(),
            name: "child".into(),
            file_path: None,
            data: json!({}),
        })
        .unwrap();
    graph
        .insert_edge(&GraphEdge {
            id: 0,
            from_id: a,
            to_id: b,
            edge_type: "CALLS".into(),
            data: json!({}),
        })
        .unwrap();
    a
}
