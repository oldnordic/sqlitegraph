use serde_json::{Map, Value, json};

use crate::{
    SqliteGraphError,
    backend_client::BackendClient,
    dsl::parse_dsl,
    safety::{SafetyReport, run_deep_safety_checks, run_integrity_sweep, run_safety_checks},
    subgraph::{SubgraphRequest, structural_signature},
};

use super::{
    cli_utils::{
        encode, has_flag, invalid, parse_optional_u32, parse_required_i64, required_value,
    },
    file_io::{parse_type_filters, summarize_dsl},
    pipeline_ops::{pipeline_expression, pipeline_from_expression},
};

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
        "safety-check" => run_safety_check(client, args).map(Some),
        "metrics" => run_metrics(client, args).map(Some),
        _ => Ok(None),
    }
}

fn run_subgraph(client: &BackendClient, args: &[String]) -> Result<String, SqliteGraphError> {
    let root = parse_required_i64(args, "--root")?;
    let depth = parse_optional_u32(args, "--depth").unwrap_or(1);
    let (edge_types, node_types) = parse_type_filters(args)?;
    let mut edge_filters = edge_types.clone();
    edge_filters.sort();
    edge_filters.dedup();
    let mut node_filters = node_types.clone();
    node_filters.sort();
    node_filters.dedup();
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
    object.insert("edge_filters".into(), json!(edge_filters));
    object.insert("node_filters".into(), json!(node_filters));
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
    object.insert("dsl".into(), Value::String(expr));
    object.insert("nodes".into(), json!(result.nodes));
    object.insert("scores".into(), Value::Array(scores));
    encode(object)
}

fn run_metrics(client: &BackendClient, args: &[String]) -> Result<String, SqliteGraphError> {
    let graph = client.backend().graph();
    if has_flag(args, "--reset-metrics") {
        graph.reset_metrics();
    }
    let snapshot = graph.metrics_snapshot();
    let mut object = Map::new();
    object.insert("command".into(), Value::String("metrics".into()));
    object.insert("prepare_count".into(), json!(snapshot.prepare_count));
    object.insert("execute_count".into(), json!(snapshot.execute_count));
    object.insert("tx_begin_count".into(), json!(snapshot.tx_begin_count));
    object.insert("tx_commit_count".into(), json!(snapshot.tx_commit_count));
    object.insert(
        "tx_rollback_count".into(),
        json!(snapshot.tx_rollback_count),
    );
    object.insert(
        "prepare_cache_hits".into(),
        json!(snapshot.prepare_cache_hits),
    );
    object.insert(
        "prepare_cache_misses".into(),
        json!(snapshot.prepare_cache_misses),
    );
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
    object.insert("dsl".into(), Value::String(expr));
    object.insert("steps_summary".into(), json!(explanation.steps_summary));
    object.insert(
        "node_counts".into(),
        json!(explanation.node_counts_per_step),
    );
    object.insert("filters".into(), json!(explanation.filters_applied));
    object.insert("scoring".into(), json!(explanation.scoring_notes));
    encode(object)
}

fn run_safety_check(client: &BackendClient, args: &[String]) -> Result<String, SqliteGraphError> {
    let strict = args.iter().any(|arg| arg == "--strict");
    let deep = args.iter().any(|arg| arg == "--deep");
    let sweep = args.iter().any(|arg| arg == "--sweep");
    let report = if deep {
        run_deep_safety_checks(client.backend().graph())?
    } else {
        run_safety_checks(client.backend().graph())?
    };
    let sweep_issues = if sweep {
        run_integrity_sweep(client.backend().graph())?
    } else {
        Vec::new()
    };
    if strict && (report.has_issues() || !sweep_issues.is_empty()) {
        return Err(invalid(format!(
            "safety violations detected: orphan_edges={} duplicate_edges={} invalid_labels={} invalid_properties={} sweep_issues={}",
            report.orphan_edges,
            report.duplicate_edges,
            report.invalid_labels,
            report.invalid_properties,
            sweep_issues.len(),
        )));
    }
    let mut object = Map::new();
    object.insert("command".into(), Value::String("safety-check".into()));
    object.insert("report".into(), report_to_value(&report, &sweep_issues));
    encode(object)
}

fn report_to_value(report: &SafetyReport, sweep_issues: &[String]) -> Value {
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
    inner.insert("integrity_errors".into(), json!(report.integrity_errors));
    inner.insert(
        "integrity_messages".into(),
        json!(report.integrity_messages),
    );
    inner.insert("sweep_issues".into(), json!(sweep_issues));
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
