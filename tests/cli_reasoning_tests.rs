use assert_cmd::Command;
use serde_json::{Value, json};
use sqlitegraph::{
    backend::SqliteGraphBackend,
    backend_client::BackendClient,
    dsl::DslResult,
    graph::{GraphEdge, GraphEntity, SqliteGraph},
    parse_dsl,
    pipeline::{ReasoningPipeline, ReasoningStep},
    subgraph::{SubgraphRequest, extract_subgraph},
};
use std::path::{Path, PathBuf};

#[test]
fn subgraph_cli_matches_library_subgraph() {
    let ctx = test_graph("subgraph");
    let backend = backend_for_path(&ctx.path);
    let request = SubgraphRequest {
        root: ctx.root,
        depth: 2,
        allowed_edge_types: vec![],
        allowed_node_types: vec![],
    };
    let expected = extract_subgraph(&backend, request.clone()).unwrap();
    let expected_value = json!({
        "nodes": expected.nodes,
        "edges": expected
            .edges
            .iter()
            .map(|(from, to, ty)| json!({"from": from, "to": to, "type": ty}))
            .collect::<Vec<_>>(),
        "signature": sqlitegraph::subgraph::structural_signature(&expected),
    });

    let output = run_cli(
        &ctx.path,
        &[
            "--command",
            "subgraph",
            "--root",
            &ctx.root.to_string(),
            "--depth",
            "2",
        ],
    );
    assert_eq!(output["command"], Value::String("subgraph".into()));
    assert_eq!(output["nodes"], expected_value["nodes"]);
    assert_eq!(output["edges"], expected_value["edges"]);
    assert_eq!(output["signature"], expected_value["signature"]);
}

#[test]
fn pipeline_cli_matches_manual_chain() {
    let ctx = test_graph("pipeline");
    let backend = backend_for_path(&ctx.path);
    let client = BackendClient::new(backend);
    let dsl = "pattern CALLS filter type=Fn";
    let pipeline = pipeline_from_dsl(dsl);
    let expected = client.run_pipeline(pipeline.clone()).unwrap();

    let output = run_cli(&ctx.path, &["--command", "pipeline", "--dsl", dsl]);
    assert_eq!(output["command"], Value::String("pipeline".into()));
    assert_eq!(output["nodes"], json!(expected.nodes));
    assert_eq!(
        output["scores"],
        json!(
            expected
                .scores
                .iter()
                .map(|(id, score)| json!({"node": id, "score": score}))
                .collect::<Vec<_>>()
        )
    );
}

#[test]
fn explain_cli_matches_manual_explanation() {
    let ctx = test_graph("explain");
    let backend = backend_for_path(&ctx.path);
    let client = BackendClient::new(backend);
    let dsl = "pattern CALLS filter type=Fn";
    let pipeline = pipeline_from_dsl(dsl);
    let expected = client.explain_pipeline(pipeline.clone()).unwrap();

    let output = run_cli(&ctx.path, &["--command", "explain-pipeline", "--dsl", dsl]);
    assert_eq!(output["command"], Value::String("explain-pipeline".into()));
    assert_eq!(output["steps_summary"], json!(expected.steps_summary));
    assert_eq!(output["node_counts"], json!(expected.node_counts_per_step));
    assert_eq!(output["filters"], json!(expected.filters_applied));
    assert_eq!(output["scoring"], json!(expected.scoring_notes));
}

#[test]
fn dsl_parse_cli_reports_pattern_summary() {
    let ctx = test_graph("dsl");
    let output = run_cli(
        &ctx.path,
        &["--command", "dsl-parse", "--input", "CALLS->USES"],
    );
    assert_eq!(output["command"], Value::String("dsl-parse".into()));
    assert_eq!(output["result"]["type"], Value::String("pattern".into()));
    assert_eq!(output["result"]["legs"], Value::Number(2.into()));
}

struct TestGraph {
    path: PathBuf,
    root: i64,
}

fn test_graph(tag: &str) -> TestGraph {
    let path = temp_db_path(&format!("cli_reasoning_{tag}.db"));
    let graph = SqliteGraph::open(&path).expect("graph");
    let root = graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Fn".into(),
            name: "root".into(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let other = graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Fn".into(),
            name: "child".into(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let tail = graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Module".into(),
            name: "leaf".into(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    graph
        .insert_edge(&GraphEdge {
            id: 0,
            from_id: root,
            to_id: other,
            edge_type: "CALLS".into(),
            data: serde_json::json!({}),
        })
        .unwrap();
    graph
        .insert_edge(&GraphEdge {
            id: 0,
            from_id: other,
            to_id: tail,
            edge_type: "USES".into(),
            data: serde_json::json!({}),
        })
        .unwrap();
    TestGraph { path, root }
}

fn backend_for_path(path: &Path) -> SqliteGraphBackend {
    let graph = SqliteGraph::open(path).expect("graph");
    SqliteGraphBackend::from_graph(graph)
}

fn run_cli(path: &Path, extra: &[&str]) -> Value {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sqlitegraph"));
    cmd.args(["--db", path.to_str().unwrap()]);
    cmd.args(extra);
    let assert = cmd.assert().success();
    let stdout = String::from_utf8(assert.get_output().stdout.clone()).expect("utf8");
    serde_json::from_str(stdout.trim()).expect("json")
}

fn pipeline_from_dsl(expr: &str) -> ReasoningPipeline {
    match parse_dsl(expr) {
        DslResult::Pipeline(p) => p,
        DslResult::Pattern(pattern) => ReasoningPipeline {
            steps: vec![ReasoningStep::Pattern(pattern)],
        },
        other => panic!("unsupported dsl result: {other:?}"),
    }
}

fn temp_db_path(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(name);
    let _ = std::fs::remove_file(&path);
    path
}
