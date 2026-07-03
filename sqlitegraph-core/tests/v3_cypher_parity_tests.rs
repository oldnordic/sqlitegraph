#![cfg(feature = "native-v3")]

use sqlitegraph::{
    SnapshotId,
    backend::native::v3::V3Backend,
    backend::{EdgeSpec, GraphBackend, NodeSpec},
    cypher,
};
use tempfile::TempDir;

fn create_v3_backend() -> (TempDir, V3Backend) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("v3_cypher_parity.graph");
    let backend = V3Backend::create(&db_path).unwrap();
    (temp_dir, backend)
}

fn build_test_graph() -> (TempDir, V3Backend) {
    let (temp_dir, backend) = create_v3_backend();

    let main_id = backend
        .insert_node(NodeSpec {
            kind: "Function".into(),
            name: "main".into(),
            file_path: None,
            data: serde_json::json!({"lang": "rust"}),
        })
        .unwrap();

    let helper_id = backend
        .insert_node(NodeSpec {
            kind: "Function".into(),
            name: "helper".into(),
            file_path: None,
            data: serde_json::json!({"lang": "rust"}),
        })
        .unwrap();

    let util_id = backend
        .insert_node(NodeSpec {
            kind: "Function".into(),
            name: "util".into(),
            file_path: None,
            data: serde_json::json!({"lang": "python"}),
        })
        .unwrap();

    let file_id = backend
        .insert_node(NodeSpec {
            kind: "File".into(),
            name: "main.rs".into(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_edge(EdgeSpec {
            from: main_id,
            to: helper_id,
            edge_type: "CALLS".into(),
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_edge(EdgeSpec {
            from: helper_id,
            to: util_id,
            edge_type: "CALLS".into(),
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_edge(EdgeSpec {
            from: file_id,
            to: main_id,
            edge_type: "CONTAINS".into(),
            data: serde_json::json!({}),
        })
        .unwrap();

    (temp_dir, backend)
}

fn sorted_name_pairs(result: &serde_json::Value, left: &str, right: &str) -> Vec<(String, String)> {
    let mut rows: Vec<(String, String)> = result["results"]
        .as_array()
        .expect("results array")
        .iter()
        .map(|row| {
            (
                row[left].as_str().expect("left value").to_string(),
                row[right].as_str().expect("right value").to_string(),
            )
        })
        .collect();
    rows.sort();
    rows
}

#[test]
fn test_v3_execute_edge_pattern() {
    let (_temp_dir, backend) = build_test_graph();
    let query = cypher::parse("MATCH (a)-[:CALLS]->(b) RETURN a.name, b.name").expect("parse");
    let result = cypher::execute(&backend, &query).expect("execute");

    assert_eq!(
        sorted_name_pairs(&result, "a.name", "b.name"),
        vec![
            ("helper".to_string(), "util".to_string()),
            ("main".to_string(), "helper".to_string()),
        ]
    );
}

#[test]
fn test_v3_execute_edge_pattern_with_label_filter() {
    let (_temp_dir, backend) = build_test_graph();
    let query = cypher::parse("MATCH (a:Function)-[:CALLS]->(b:Function) RETURN a.name, b.name")
        .expect("parse");
    let result = cypher::execute(&backend, &query).expect("execute");

    assert_eq!(
        sorted_name_pairs(&result, "a.name", "b.name"),
        vec![
            ("helper".to_string(), "util".to_string()),
            ("main".to_string(), "helper".to_string()),
        ]
    );
}

#[test]
fn test_v3_execute_node_match_with_label_and_name_filters() {
    let (_temp_dir, backend) = build_test_graph();
    let query =
        cypher::parse("MATCH (n:Function {name: \"main\"}) RETURN n.name, n.kind").expect("parse");
    let result = cypher::execute(&backend, &query).expect("execute");

    let rows = result["results"].as_array().expect("results array");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["n.name"].as_str(), Some("main"));
    assert_eq!(rows[0]["n.kind"].as_str(), Some("Function"));
}

#[test]
fn test_v3_execute_edge_pattern_with_name_filters() {
    let (_temp_dir, backend) = build_test_graph();
    let query = cypher::parse(
        "MATCH (a:Function {name: \"main\"})-[:CALLS]->(b:Function {name: \"helper\"}) RETURN a.name, b.name",
    )
    .expect("parse");
    let result = cypher::execute(&backend, &query).expect("execute");

    assert_eq!(
        sorted_name_pairs(&result, "a.name", "b.name"),
        vec![("main".to_string(), "helper".to_string())],
    );
}

#[test]
fn test_v3_query_nodes_by_name_pattern_uses_substring_semantics() {
    let (_temp_dir, backend) = build_test_graph();
    let node_ids = backend
        .query_nodes_by_name_pattern(SnapshotId::current(), "main")
        .expect("query nodes by name pattern");

    let mut names: Vec<String> = node_ids
        .into_iter()
        .map(|id| {
            backend
                .get_node(SnapshotId::current(), id)
                .expect("get node")
                .name
        })
        .collect();
    names.sort();

    assert_eq!(names, vec!["main".to_string(), "main.rs".to_string()]);
}

#[test]
fn test_v3_execute_variable_depth() {
    let (_temp_dir, backend) = build_test_graph();
    let query =
        cypher::parse("MATCH (a:Function {name: \"main\"})-[:CALLS*1..2]->(b) RETURN b.name")
            .expect("parse");
    let result = cypher::execute(&backend, &query).expect("execute");

    let results = result["results"].as_array().expect("results array");
    assert_eq!(results.len(), 2);
}

#[test]
fn test_v3_execute_multi_hop_reports_current_gap() {
    let (_temp_dir, backend) = build_test_graph();
    let query = cypher::parse("MATCH (a)-[:CALLS]->(b)-[:CALLS]->(c) RETURN a.name, c.name")
        .expect("parse");
    let err = cypher::execute(&backend, &query).expect_err("multi-hop should fail on V3 today");

    assert!(
        err.contains("Graph introspection failed"),
        "unexpected V3 multi-hop error: {err}"
    );
}
