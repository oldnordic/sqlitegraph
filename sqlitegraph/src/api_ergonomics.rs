use std::fmt;

use crate::{
    SqliteGraphError,
    backend::{BackendDirection, GraphBackend, SqliteGraphBackend},
    pattern::{NodeConstraint, PatternQuery},
    pipeline::{ReasoningPipeline, ReasoningStep},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub i64);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EdgeId(pub i64);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Label(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PropertyKey(pub String);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PropertyValue(pub String);

#[derive(Clone, Debug, PartialEq)]
pub struct PipelineExplanation {
    pub steps_summary: Vec<String>,
    pub node_counts_per_step: Vec<usize>,
    pub filters_applied: Vec<String>,
    pub scoring_notes: Vec<String>,
}

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl NodeId {
    pub fn as_i64(self) -> i64 {
        self.0
    }
}

impl From<i64> for NodeId {
    fn from(value: i64) -> Self {
        NodeId(value)
    }
}

pub fn explain_pipeline(
    backend: &SqliteGraphBackend,
    pipeline: &ReasoningPipeline,
) -> Result<PipelineExplanation, SqliteGraphError> {
    let mut counts = Vec::new();
    let mut summaries = Vec::new();
    let mut filters = Vec::new();
    let mut scoring = Vec::new();
    let mut nodes: Vec<i64> = Vec::new();
    for step in &pipeline.steps {
        match step {
            ReasoningStep::Pattern(pattern) => {
                nodes = gather_pattern_nodes(backend, pattern)?;
                summaries.push(format!("pattern {} legs", pattern.legs.len()));
            }
            ReasoningStep::KHops(depth) => {
                nodes = gather_khops(backend, &nodes, *depth)?;
                summaries.push(format!("khop depth={depth}"));
            }
            ReasoningStep::Filter(constraint) => {
                nodes = filter_nodes(backend, &nodes, constraint)?;
                filters.push(format!("filter kind={:?}", constraint.kind.as_deref()));
                summaries.push("filter".into());
            }
            ReasoningStep::Score(config) => {
                let notes = format!(
                    "score hop_depth={} degree_weight={}",
                    config.hop_depth, config.degree_weight
                );
                scoring.push(notes.clone());
                summaries.push("score".into());
            }
        }
        counts.push(nodes.len());
    }
    Ok(PipelineExplanation {
        steps_summary: summaries,
        node_counts_per_step: counts,
        filters_applied: filters,
        scoring_notes: scoring,
    })
}

fn gather_pattern_nodes(
    backend: &SqliteGraphBackend,
    pattern: &PatternQuery,
) -> Result<Vec<i64>, SqliteGraphError> {
    let graph = backend.graph();
    let mut nodes = Vec::new();
    for id in graph.all_entity_ids()? {
        let matches = graph.query().pattern_matches(id, pattern)?;
        for m in matches {
            nodes.extend(m.nodes);
        }
    }
    nodes.sort_unstable();
    nodes.dedup();
    Ok(nodes)
}

fn gather_khops(
    backend: &SqliteGraphBackend,
    seeds: &[i64],
    depth: u32,
) -> Result<Vec<i64>, SqliteGraphError> {
    let mut nodes = seeds.to_vec();
    for seed in seeds {
        let hops = backend.k_hop(*seed, depth, BackendDirection::Outgoing)?;
        nodes.extend(hops);
    }
    nodes.sort_unstable();
    nodes.dedup();
    Ok(nodes)
}

fn filter_nodes(
    backend: &SqliteGraphBackend,
    nodes: &[i64],
    constraint: &NodeConstraint,
) -> Result<Vec<i64>, SqliteGraphError> {
    let mut filtered = Vec::new();
    for &id in nodes {
        let entity = backend.graph().get_entity(id)?;
        if constraint.matches(&entity) {
            filtered.push(id);
        }
    }
    Ok(filtered)
}
