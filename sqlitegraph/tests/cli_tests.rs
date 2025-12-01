use assert_cmd::Command;
use rusqlite::Connection;
use serde_json::{Value, json};
use sqlitegraph::{
    graph::{GraphEdge, GraphEntity, SqliteGraph},
    schema::{BASE_SCHEMA_VERSION, SCHEMA_VERSION},
};
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
    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    assert!(stdout.contains("schema_version="));
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

#[test]
fn test_cli_migrate_dry_run_reports_statements() {
    let path = temp_db_path("sqlitegraph_cli_migrate_dry.db");
    prepare_db(&path);
    downgrade_schema(&path);

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sqlitegraph"));
    cmd.args([
        "--db",
        path.to_str().unwrap(),
        "--command",
        "migrate",
        "--dry-run",
    ]);
    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let value: Value = serde_json::from_str(stdout.trim()).expect("json");
    assert_eq!(value["command"], Value::String("migrate".into()));
    assert_eq!(value["dry_run"], Value::Bool(true));
    assert!(
        !value["statements"].as_array().unwrap().is_empty(),
        "expected at least one migration statement"
    );
    let version: i64 = Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT schema_version FROM graph_meta WHERE id=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, BASE_SCHEMA_VERSION);
}

#[test]
fn test_cli_migrate_exec_updates_version() {
    let path = temp_db_path("sqlitegraph_cli_migrate_exec.db");
    prepare_db(&path);
    downgrade_schema(&path);

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sqlitegraph"));
    cmd.args(["--db", path.to_str().unwrap(), "--command", "migrate"]);
    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let value: Value = serde_json::from_str(stdout.trim()).expect("json");
    assert_eq!(value["command"], Value::String("migrate".into()));
    assert_eq!(value["dry_run"], Value::Bool(false));
    assert_eq!(value["to_version"], Value::from(SCHEMA_VERSION));

    let version: i64 = Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT schema_version FROM graph_meta WHERE id=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(version, SCHEMA_VERSION);
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

fn downgrade_schema(path: &PathBuf) {
    let conn = Connection::open(path).unwrap();
    conn.execute(
        "UPDATE graph_meta SET schema_version=?1 WHERE id=1",
        [BASE_SCHEMA_VERSION],
    )
    .unwrap();
    let _ = conn.execute("DROP TABLE IF EXISTS graph_meta_history", []);
}
