use crate::{SqliteGraphError, dsl::DslResult};

use super::cli_utils::invalid;

pub fn parse_type_filters(args: &[String]) -> Result<(Vec<String>, Vec<String>), SqliteGraphError> {
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

pub fn peek_non_whitespace<R: std::io::BufRead>(
    reader: &mut R,
) -> Result<Option<u8>, SqliteGraphError> {
    loop {
        let buffer_len = {
            let buffer = reader
                .fill_buf()
                .map_err(|e| invalid(format!("unable to read pipeline file: {e}")))?;
            if buffer.is_empty() {
                return Ok(None);
            }
            let mut idx = 0;
            while idx < buffer.len() {
                let byte = buffer[idx];
                if byte.is_ascii_whitespace() {
                    idx += 1;
                    continue;
                }
                reader.consume(idx);
                return Ok(Some(byte));
            }
            buffer.len()
        };
        reader.consume(buffer_len);
    }
}

pub fn summarize_dsl(result: DslResult) -> Result<serde_json::Value, SqliteGraphError> {
    match result {
        DslResult::Pattern(pattern) => Ok(serde_json::json!({
            "type": "pattern",
            "legs": pattern.legs.len(),
        })),
        DslResult::Pipeline(pipeline) => Ok(serde_json::json!({
            "type": "pipeline",
            "steps": pipeline.steps.len(),
        })),
        DslResult::Subgraph(request) => Ok(serde_json::json!({
            "type": "subgraph",
            "depth": request.depth,
            "edge_types": request.allowed_edge_types.len(),
            "node_types": request.allowed_node_types.len(),
        })),
        DslResult::Error(msg) => Err(invalid(msg)),
    }
}
