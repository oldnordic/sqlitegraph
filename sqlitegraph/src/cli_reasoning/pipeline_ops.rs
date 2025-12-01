use crate::{
    SqliteGraphError,
    dsl::{DslResult, parse_dsl},
    pipeline::{ReasoningPipeline, ReasoningStep},
};

use super::{
    cli_utils::{invalid, value},
    file_io::peek_non_whitespace,
};

pub fn pipeline_expression(args: &[String]) -> Result<String, SqliteGraphError> {
    let dsl = value(args, "--dsl");
    let file = value(args, "--file");
    match (dsl, file) {
        (Some(expr), None) => Ok(expr),
        (None, Some(path)) => read_pipeline_file(&path),
        (Some(_), Some(_)) => Err(invalid("provide only one of --dsl or --file")),
        _ => Err(invalid("pipeline requires --dsl or --file")),
    }
}

pub fn pipeline_from_expression(expr: &str) -> Result<ReasoningPipeline, SqliteGraphError> {
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
    let file = std::fs::File::open(std::path::Path::new(path))
        .map_err(|e| invalid(format!("unable to read pipeline file: {e}")))?;
    read_pipeline_reader(file)
}

fn read_pipeline_reader<R: std::io::Read>(reader: R) -> Result<String, SqliteGraphError> {
    let mut buf = std::io::BufReader::new(reader);
    match peek_non_whitespace(&mut buf)? {
        None => Err(invalid("pipeline file is empty")),
        Some(b'{') => read_pipeline_json(buf),
        _ => read_pipeline_plain(buf),
    }
}

fn read_pipeline_json<R: std::io::Read>(reader: R) -> Result<String, SqliteGraphError> {
    let mut stream = serde_json::Deserializer::from_reader(reader).into_iter::<serde_json::Value>();
    let first = stream
        .next()
        .ok_or_else(|| invalid("pipeline json must contain a 'dsl' string"))
        .and_then(|result| result.map_err(|e| invalid(format!("invalid pipeline json: {e}"))))?;
    if let Some(expr) = first.get("dsl").and_then(|v| v.as_str()) {
        Ok(expr.to_string())
    } else {
        Err(invalid("pipeline json must contain a 'dsl' string"))
    }
}

fn read_pipeline_plain<R: std::io::Read>(mut reader: R) -> Result<String, SqliteGraphError> {
    let mut contents = String::new();
    reader
        .read_to_string(&mut contents)
        .map_err(|e| invalid(format!("unable to read pipeline file: {e}")))?;
    let trimmed = contents.trim();
    if trimmed.is_empty() {
        Err(invalid("pipeline file is empty"))
    } else {
        Ok(trimmed.to_string())
    }
}
