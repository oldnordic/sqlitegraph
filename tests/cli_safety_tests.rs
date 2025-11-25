use assert_cmd::Command;
use serde_json::Value;
use sqlitegraph::{
    graph::{GraphEdge, GraphEntity, SqliteGraph},
    safety::run_safety_checks,
};
use std::path::PathBuf;

#[test]
fn safety_cli_matches_library_report() {
    let path = temp_db_path("cli_safety.db");
    let graph = SqliteGraph::open(&path).expect("graph");
    let a = graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Fn".into(),
            name: "root".into(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let b = graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Fn".into(),
            name: "child".into(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    graph
        .insert_edge(&GraphEdge {
            id: 0,
            from_id: a,
            to_id: b,
            edge_type: "CALLS".into(),
            data: serde_json::json!({}),
        })
        .unwrap();

    let expected = run_safety_checks(&graph).unwrap();

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sqlitegraph"));
    cmd.args(["--db", path.to_str().unwrap(), "--command", "safety-check"]);
    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    let value: Value = serde_json::from_str(stdout.trim()).expect("json");

    assert_eq!(value["command"], Value::String("safety-check".into()));
    assert_eq!(
        value["report"]["total_nodes"],
        Value::from(expected.total_nodes)
    );
    assert_eq!(
        value["report"]["total_edges"],
        Value::from(expected.total_edges)
    );
    assert_eq!(
        value["report"]["orphan_edges"],
        Value::from(expected.orphan_edges)
    );
    assert_eq!(
        value["report"]["duplicate_edges"],
        Value::from(expected.duplicate_edges)
    );
    assert_eq!(
        value["report"]["invalid_labels"],
        Value::from(expected.invalid_labels)
    );
    assert_eq!(
        value["report"]["invalid_properties"],
        Value::from(expected.invalid_properties)
    );
}

#[test]
fn safety_cli_strict_mode_fails_on_issues() {
    let path = temp_db_path("cli_safety_strict.db");
    let graph = SqliteGraph::open(&path).expect("graph");
    let a = graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Fn".into(),
            name: "root".into(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let b = graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Fn".into(),
            name: "child".into(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    // duplicate edge to trigger failure
    for _ in 0..2 {
        graph
            .insert_edge(&GraphEdge {
                id: 0,
                from_id: a,
                to_id: b,
                edge_type: "CALLS".into(),
                data: serde_json::json!({}),
            })
            .unwrap();
    }

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sqlitegraph"));
    cmd.args([
        "--db",
        path.to_str().unwrap(),
        "--command",
        "safety-check",
        "--strict",
    ]);
    let assert = cmd.assert().failure();
    let stderr = String::from_utf8(assert.get_output().stderr.clone()).expect("utf8");
    assert!(stderr.contains("safety violations"));
}

fn temp_db_path(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(name);
    let _ = std::fs::remove_file(&path);
    path
}
