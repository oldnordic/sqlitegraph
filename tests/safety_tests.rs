use rusqlite::{Connection, params};
use serde_json::json;
use sqlitegraph::{
    add_label, add_property,
    graph::{GraphEdge, GraphEntity, SqliteGraph},
    safety::{
        run_safety_checks, run_strict_safety_checks, validate_labels_properties,
        validate_no_duplicate_edges, validate_referential_integrity,
    },
};
use std::path::{Path, PathBuf};

#[test]
fn report_for_clean_graph_no_issues() {
    let ctx = graph_context("clean");
    let report = run_safety_checks(&ctx.graph).unwrap();
    assert_eq!(report.total_nodes, 2);
    assert_eq!(report.total_edges, 1);
    assert_eq!(report.orphan_edges, 0);
    assert_eq!(report.duplicate_edges, 0);
    assert_eq!(report.invalid_labels, 0);
    assert_eq!(report.invalid_properties, 0);
}

#[test]
fn orphan_edges_detected() {
    let ctx = graph_context("orphan");
    insert_orphan_edge(&ctx.path, ctx.graph_edge_data());
    let report = validate_referential_integrity(&ctx.graph).unwrap();
    assert_eq!(report.orphan_edges, 1);
}

#[test]
fn duplicate_edges_detected() {
    let ctx = graph_context("duplicate");
    insert_duplicate_edge(&ctx.path, ctx.graph_edge_data());
    let report = validate_no_duplicate_edges(&ctx.graph).unwrap();
    assert_eq!(report.duplicate_edges, 1);
}

#[test]
fn invalid_label_property_detected() {
    let ctx = graph_context("invalid_meta");
    insert_invalid_metadata(&ctx.path);
    let report = validate_labels_properties(&ctx.graph).unwrap();
    assert_eq!(report.invalid_labels, 1);
    assert_eq!(report.invalid_properties, 1);
}

#[test]
fn strict_mode_fails_on_issues() {
    let ctx = graph_context("strict");
    insert_duplicate_edge(&ctx.path, ctx.graph_edge_data());
    let err = run_strict_safety_checks(&ctx.graph).unwrap_err();
    assert!(err.report.duplicate_edges > 0);
}

struct GraphContext {
    graph: SqliteGraph,
    path: PathBuf,
    root: i64,
    child: i64,
}

impl GraphContext {
    fn graph_edge_data(&self) -> (i64, i64) {
        (self.root, self.child)
    }
}

fn graph_context(tag: &str) -> GraphContext {
    let path = temp_db_path(&format!("safety_{tag}.db"));
    let graph = SqliteGraph::open(&path).expect("graph");
    let root = graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Fn".into(),
            name: "root".into(),
            file_path: None,
            data: json!({}),
        })
        .unwrap();
    let child = graph
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
            from_id: root,
            to_id: child,
            edge_type: "CALLS".into(),
            data: json!({}),
        })
        .unwrap();
    add_label(&graph, root, "Fn").unwrap();
    add_property(&graph, root, "role", "root").unwrap();
    GraphContext {
        graph,
        path,
        root,
        child,
    }
}

fn insert_orphan_edge(path: &Path, (from, _to): (i64, i64)) {
    let missing = from + 1000;
    raw_conn(path)
        .execute(
            "INSERT INTO graph_edges(from_id, to_id, edge_type, data) VALUES(?1, ?2, 'CALLS', '{}')",
            params![from, missing],
        )
        .unwrap();
}

fn insert_duplicate_edge(path: &Path, (from, to): (i64, i64)) {
    raw_conn(path)
        .execute(
            "INSERT INTO graph_edges(from_id, to_id, edge_type, data) VALUES(?1, ?2, 'CALLS', '{}')",
            params![from, to],
        )
        .unwrap();
}

fn insert_invalid_metadata(path: &Path) {
    let missing = 99_999_i64;
    let conn = raw_conn(path);
    conn.execute(
        "INSERT INTO graph_labels(entity_id, label) VALUES(?1, 'Ghost')",
        params![missing],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO graph_properties(entity_id, key, value) VALUES(?1, 'role', 'ghost')",
        params![missing],
    )
    .unwrap();
}

fn raw_conn(path: &Path) -> Connection {
    Connection::open(path).expect("connection")
}

fn temp_db_path(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(name);
    let _ = std::fs::remove_file(&path);
    path
}
