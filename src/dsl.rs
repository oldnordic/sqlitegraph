use crate::{
    backend::BackendDirection,
    pattern::{NodeConstraint, PatternLeg, PatternQuery},
    pipeline::{ReasoningPipeline, ReasoningStep},
    subgraph::SubgraphRequest,
};

#[derive(Debug, Clone)]
pub enum DslResult {
    Pattern(PatternQuery),
    Pipeline(ReasoningPipeline),
    Subgraph(SubgraphRequest),
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
    if let Some(result) = parse_hop_command(trimmed) {
        return result;
    }
    if trimmed.contains("->") {
        return DslResult::Pattern(parse_chain(trimmed));
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
    let query = parse_repetitive_pattern(pattern_part);
    let mut steps = vec![ReasoningStep::Pattern(query)];
    if let Some(filter_part) = segments.next() {
        let text = filter_part.trim();
        if let Some(kind) = text.strip_prefix("type=") {
            steps.push(ReasoningStep::Filter(NodeConstraint::kind(kind.trim())));
        } else {
            return DslResult::Error("unsupported filter clause".into());
        }
    }
    DslResult::Pipeline(ReasoningPipeline { steps })
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

fn parse_chain(input: &str) -> PatternQuery {
    let mut legs = Vec::new();
    for segment in input.split("->") {
        let ty = segment.trim();
        if ty.is_empty() {
            continue;
        }
        legs.push(PatternLeg {
            direction: BackendDirection::Outgoing,
            edge_type: Some(ty.to_string()),
            constraint: None,
        });
    }
    PatternQuery { root: None, legs }
}

fn parse_hop_command(input: &str) -> Option<DslResult> {
    if let Some((prefix, suffix)) = input.split_once("-hop") {
        let depth = prefix.trim().parse::<u32>().ok()?;
        let mut allowed = Vec::new();
        if let Some(filter) = suffix.trim().strip_prefix("type=") {
            allowed.push(filter.trim().to_string());
        }
        return Some(DslResult::Subgraph(SubgraphRequest {
            root: 0,
            depth,
            allowed_edge_types: Vec::new(),
            allowed_node_types: allowed,
        }));
    }
    None
}
