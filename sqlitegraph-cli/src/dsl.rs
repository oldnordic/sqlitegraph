use sqlitegraph::{
    backend::BackendDirection,
    pattern::{PatternLeg, PatternQuery},
};

/// Simplified DSL result for v0.2.5 (pipeline/subgraph removed)
#[derive(Debug, Clone)]
pub enum DslResult {
    Pattern(PatternQuery),
    Error(String),
}

pub fn parse_dsl(input: &str) -> DslResult {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return DslResult::Error("empty DSL string".into());
    }
    if trimmed.starts_with("pattern ") {
        return parse_pattern_pipeline(trimmed);
    }
    if trimmed.contains("->") || trimmed.contains('*') {
        return DslResult::Pattern(parse_repetitive_pattern(trimmed));
    }
    DslResult::Error(format!("unsupported DSL form: {trimmed}"))
}

fn parse_pattern_pipeline(input: &str) -> DslResult {
    let rest = input.trim_start_matches("pattern").trim();
    let mut segments = rest.splitn(2, "filter");
    let pattern_part = segments.next().unwrap_or("").trim();
    if pattern_part.is_empty() {
        return DslResult::Error("missing pattern segment".into());
    }
    let _query = parse_repetitive_pattern(pattern_part);
    // Note: filter parsing removed for v0.2.5 compatibility
    DslResult::Pattern(_query)
}

fn parse_repetitive_pattern(segment: &str) -> PatternQuery {
    let mut legs = Vec::new();
    for token in segment.split("->") {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (ty, count) = if let Some((name, times)) = trimmed.split_once('*') {
            let repeats = times.trim().parse::<usize>().unwrap_or(1);
            (name.trim(), repeats)
        } else {
            (trimmed, 1)
        };
        for _ in 0..count {
            legs.push(PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some(ty.to_string()),
                constraint: None,
            });
        }
    }
    PatternQuery { root: None, legs }
}

// parse_hop_command removed - SubgraphRequest not available in v0.2.5
// TODO: Implement simplified hop command using PatternQuery if needed
