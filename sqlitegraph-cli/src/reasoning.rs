use serde_json::{json, Map, Value};

use crate::dsl::{parse_dsl, DslResult};
use sqlitegraph::SqliteGraphError;

const ERR_PREFIX: &str = "cli";

/// Simplified command handler for v0.2.5
/// Removed: subgraph, pipeline, explain-pipeline, safety-check (depend on missing modules)
pub fn handle_command(
    _client: &crate::BackendClient,
    command: &str,
    args: &[String],
) -> Result<Option<String>, SqliteGraphError> {
    match command {
        "dsl-parse" => run_dsl_parse(args).map(Some),
        // Removed commands that depend on missing modules:
        // - subgraph (needs subgraph module)
        // - pipeline (needs pipeline module)
        // - explain-pipeline (needs pipeline module)
        // - safety-check (needs safety module)
        // - metrics (needs metrics_snapshot which may not exist)
        _ => Ok(None),
    }
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

fn summarize_dsl(result: DslResult) -> Result<Value, SqliteGraphError> {
    match result {
        DslResult::Pattern(pattern) => Ok(json!({
            "type": "pattern",
            "legs": pattern.legs.len(),
        })),
        DslResult::Error(msg) => Err(invalid(msg)),
    }
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
