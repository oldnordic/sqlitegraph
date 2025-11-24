use std::{fs, path::Path};

use serde_json::{Map, Value, json};

use crate::{
    SqliteGraphError,
    backend_client::BackendClient,
    dsl::{DslResult, parse_dsl},
    pipeline::{ReasoningPipeline, ReasoningStep},
    safety::{SafetyReport, run_safety_checks},
    subgraph::{SubgraphRequest, structural_signature},
};

const ERR_PREFIX: &str = "cli";

pub fn handle_command(
    client: &BackendClient,
    command: &str,
    args: &[String],
) -> Result<Option<String>, SqliteGraphError> {
    match command {
        "subgraph" => run_subgraph(client, args).map(Some),
        "pipeline" => run_pipeline(client, args).map(Some),
        "explain-pipeline" => run_explain_pipeline(client, args).map(Some),
        "dsl-parse" => run_dsl_parse(args).map(Some),
        "safety-check" => run_safety_check(client).map(Some),
        _ => Ok(None),
    }
}

fn run_subgraph(client: &BackendClient, args: &[String]) -> Result<String, SqliteGraphError> {
    let root = parse_required_i64(args, "--root")?;
    let depth = parse_optional_u32(args, "--depth").unwrap_or(1);
    let (edge_types, node_types) = parse_type_filters(args)?;
    let request = SubgraphRequest {
        root,
        depth,
        allowed_edge_types: edge_types,
        allowed_node_types: node_types,
    };
    let subgraph = client.subgraph(request)?;
    let edges = subgraph
        .edges
        .iter()
        .map(|(from, to, ty)| json!({"from": from, "to": to, "type": ty}))
        .collect::<Vec<_>>();
    let signature = structural_signature(&subgraph);
    let mut object = Map::new();
    object.insert("command".into(), Value::String("subgraph".into()));
    object.insert("root".into(), json!(root));
    object.insert("depth".into(), json!(depth));
    object.insert("nodes".into(), json!(subgraph.nodes));
    object.insert("edges".into(), Value::Array(edges));
    object.insert("signature".into(), Value::String(signature));
    encode(object)
}

fn run_pipeline(client: &BackendClient, args: &[String]) -> Result<String, SqliteGraphError> {
    let expr = pipeline_expression(args)?;
    let pipeline = pipeline_from_expression(&expr)?;
    let result = client.run_pipeline(pipeline)?;
    let scores = result
        .scores
        .iter()
        .map(|(node, score)| json!({"node": node, "score": score}))
        .collect::<Vec<_>>();
    let mut object = Map::new();
    object.insert("command".into(), Value::String("pipeline".into()));
    object.insert("nodes".into(), json!(result.nodes));
    object.insert("scores".into(), Value::Array(scores));
    encode(object)
}

fn run_explain_pipeline(
    client: &BackendClient,
    args: &[String],
) -> Result<String, SqliteGraphError> {
    let expr = pipeline_expression(args)?;
    let pipeline = pipeline_from_expression(&expr)?;
    let explanation = client.explain_pipeline(pipeline)?;
    let mut object = Map::new();
    object.insert("command".into(), Value::String("explain-pipeline".into()));
    object.insert("steps_summary".into(), json!(explanation.steps_summary));
    object.insert(
        "node_counts".into(),
        json!(explanation.node_counts_per_step),
    );
    object.insert("filters".into(), json!(explanation.filters_applied));
    object.insert("scoring".into(), json!(explanation.scoring_notes));
    encode(object)
}

fn run_safety_check(client: &BackendClient) -> Result<String, SqliteGraphError> {
    let report = run_safety_checks(client.backend().graph())?;
    let mut object = Map::new();
    object.insert("command".into(), Value::String("safety-check".into()));
    object.insert("report".into(), report_to_value(&report));
    encode(object)
}

fn report_to_value(report: &SafetyReport) -> Value {
    let mut inner = Map::new();
    inner.insert("total_nodes".into(), json!(report.total_nodes));
    inner.insert("total_edges".into(), json!(report.total_edges));
    inner.insert("orphan_edges".into(), json!(report.orphan_edges));
    inner.insert("duplicate_edges".into(), json!(report.duplicate_edges));
    inner.insert("invalid_labels".into(), json!(report.invalid_labels));
    inner.insert(
        "invalid_properties".into(),
        json!(report.invalid_properties),
    );
    Value::Object(inner)
}

fn run_dsl_parse(args: &[String]) -> Result<String, SqliteGraphError> {
    let input = required_value(args, "--input")?;
    let result = parse_dsl(&input);
    let summary = summarize_dsl(result)?;
    let mut object = Map::new();
    object.insert("command".into(), Value::String("dsl-parse".into()));
    object.insert("result".into(), summary);
    encode(object)
}

fn parse_type_filters(args: &[String]) -> Result<(Vec<String>, Vec<String>), SqliteGraphError> {
    let mut edges = Vec::new();
    let mut nodes = Vec::new();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--types" {
            let value = iter
                .next()
                .ok_or_else(|| invalid("--types requires key=value"))?
                .clone();
            if let Some((key, val)) = value.split_once('=') {
                match key {
                    "edge" => edges.push(val.trim().to_string()),
                    "node" => nodes.push(val.trim().to_string()),
                    _ => return Err(invalid("--types key must be edge or node")),
                }
            } else {
                return Err(invalid("--types expects key=value"));
            }
        }
    }
    Ok((edges, nodes))
}

fn pipeline_expression(args: &[String]) -> Result<String, SqliteGraphError> {
    let dsl = value(args, "--dsl");
    let file = value(args, "--file");
    match (dsl, file) {
        (Some(expr), None) => Ok(expr),
        (None, Some(path)) => read_pipeline_file(&path),
        (Some(_), Some(_)) => Err(invalid("provide only one of --dsl or --file")),
        _ => Err(invalid("pipeline requires --dsl or --file")),
    }
}

fn pipeline_from_expression(expr: &str) -> Result<ReasoningPipeline, SqliteGraphError> {
    match parse_dsl(expr) {
        DslResult::Pipeline(pipeline) => Ok(pipeline),
        DslResult::Pattern(pattern) => Ok(ReasoningPipeline {
            steps: vec![ReasoningStep::Pattern(pattern)],
        }),
        DslResult::Error(msg) => Err(invalid(msg)),
        DslResult::Subgraph(_) => Err(invalid("DSL describes a subgraph, not a pipeline")),
    }
}

fn read_pipeline_file(path: &str) -> Result<String, SqliteGraphError> {
    let contents = fs::read_to_string(Path::new(path))
        .map_err(|e| invalid(format!("unable to read pipeline file: {e}")))?;
    let trimmed = contents.trim();
    if trimmed.starts_with('{') {
        let value: Value = serde_json::from_str(trimmed)
            .map_err(|e| invalid(format!("invalid pipeline json: {e}")))?;
        if let Some(expr) = value.get("dsl").and_then(|v| v.as_str()) {
            return Ok(expr.to_string());
        }
        return Err(invalid("pipeline json must contain a 'dsl' string"));
    }
    if trimmed.is_empty() {
        Err(invalid("pipeline file is empty"))
    } else {
        Ok(trimmed.to_string())
    }
}

fn summarize_dsl(result: DslResult) -> Result<Value, SqliteGraphError> {
    match result {
        DslResult::Pattern(pattern) => Ok(json!({
            "type": "pattern",
            "legs": pattern.legs.len(),
        })),
        DslResult::Pipeline(pipeline) => Ok(json!({
            "type": "pipeline",
            "steps": pipeline.steps.len(),
        })),
        DslResult::Subgraph(request) => Ok(json!({
            "type": "subgraph",
            "depth": request.depth,
            "edge_types": request.allowed_edge_types.len(),
            "node_types": request.allowed_node_types.len(),
        })),
        DslResult::Error(msg) => Err(invalid(msg)),
    }
}

fn parse_required_i64(args: &[String], flag: &str) -> Result<i64, SqliteGraphError> {
    let value = required_value(args, flag)?;
    value
        .parse::<i64>()
        .map_err(|_| invalid(format!("{flag} expects an integer")))
}

fn parse_optional_u32(args: &[String], flag: &str) -> Option<u32> {
    value(args, flag)?.parse::<u32>().ok()
}

fn required_value(args: &[String], flag: &str) -> Result<String, SqliteGraphError> {
    value(args, flag).ok_or_else(|| invalid(format!("missing {flag}")))
}

fn value(args: &[String], flag: &str) -> Option<String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == flag {
            return iter.next().cloned();
        }
    }
    None
}

fn encode(object: Map<String, Value>) -> Result<String, SqliteGraphError> {
    serde_json::to_string(&Value::Object(object))
        .map_err(|e| invalid(format!("{ERR_PREFIX} serialization failed: {e}")))
}

fn invalid<T: Into<String>>(message: T) -> SqliteGraphError {
    SqliteGraphError::invalid_input(message.into())
}
