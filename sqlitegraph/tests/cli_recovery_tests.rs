use assert_cmd::Command;
use serde_json::json;
use sqlitegraph::graph::{GraphEdge, GraphEntity, SqliteGraph};
use std::{fs, path::PathBuf};

#[test]
fn dump_graph_command_writes_jsonl() {
    let db_path = temp_db_path("cli_dump.db");
    prepare_db(&db_path);
    let output = temp_db_path("cli_dump.jsonl");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sqlitegraph"));
    cmd.args([
        "--db",
        db_path.to_str().unwrap(),
        "--command",
        "dump-graph",
        "--output",
        output.to_str().unwrap(),
    ]);
    cmd.assert().success();
    let contents = fs::read_to_string(&output).expect("dump contents");
    assert!(contents.contains("\"type\":\"entity\""));
    assert!(contents.contains("\"type\":\"edge\""));
}

#[test]
fn load_graph_command_restores_dump() {
    let source = temp_db_path("cli_dump_src.db");
    prepare_db(&source);
    let dump_file = temp_db_path("cli_dump_roundtrip.jsonl");
    Command::new(env!("CARGO_BIN_EXE_sqlitegraph"))
        .args([
            "--db",
            source.to_str().unwrap(),
            "--command",
            "dump-graph",
            "--output",
            dump_file.to_str().unwrap(),
        ])
        .assert()
        .success();

    let target = temp_db_path("cli_load_target.db");
    Command::new(env!("CARGO_BIN_EXE_sqlitegraph"))
        .args([
            "--db",
            target.to_str().unwrap(),
            "--command",
            "load-graph",
            "--input",
            dump_file.to_str().unwrap(),
        ])
        .assert()
        .success();

    let graph = SqliteGraph::open(&target).unwrap();
    let ids = graph.list_entity_ids().unwrap();
    assert_eq!(ids.len(), 2);
}

fn prepare_db(path: &PathBuf) {
    let graph = SqliteGraph::open(path).expect("graph");
    let a = graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Module".into(),
            name: "core".into(),
            file_path: Some("core.rs".into()),
            data: json!({}),
        })
        .unwrap();
    let b = graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Fn".into(),
            name: "run".into(),
            file_path: Some("core.rs".into()),
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
}

fn temp_db_path(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(name);
    let _ = fs::remove_file(&path);
    path
}
