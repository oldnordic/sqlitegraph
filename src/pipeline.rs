use ahash::AHashSet;

use crate::{
    SqliteGraphError,
    backend::{BackendDirection, GraphBackend, SqliteGraphBackend},
    multi_hop,
    pattern::{NodeConstraint, PatternQuery},
    query::GraphQuery,
};

#[derive(Clone, Debug)]
pub struct ReasoningPipeline {
    pub steps: Vec<ReasoningStep>,
}

#[derive(Clone, Debug)]
pub enum ReasoningStep {
    Pattern(PatternQuery),
    KHops(u32),
    Filter(NodeConstraint),
    Score(ReasoningScoreConfig),
}

#[derive(Clone, Debug)]
pub struct ReasoningScoreConfig {
    pub hop_depth: u32,
    pub degree_weight: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PipelineResult {
    pub nodes: Vec<i64>,
    pub scores: Vec<(i64, f64)>,
}

pub fn run_pipeline(
    backend: &SqliteGraphBackend,
    pipeline: &ReasoningPipeline,
) -> Result<PipelineResult, SqliteGraphError> {
    let mut nodes: Vec<i64> = Vec::new();
    let mut scores: Vec<(i64, f64)> = Vec::new();
    for step in &pipeline.steps {
        match step {
            ReasoningStep::Pattern(pattern) => {
                nodes = pattern_nodes(backend, pattern)?;
            }
            ReasoningStep::KHops(depth) => {
                nodes = expand_khops(backend, &nodes, *depth)?;
            }
            ReasoningStep::Filter(constraint) => {
                nodes = apply_filter(backend, &nodes, constraint)?;
            }
            ReasoningStep::Score(config) => {
                scores = score_nodes(backend, &nodes, config)?;
            }
        }
    }
    Ok(PipelineResult { nodes, scores })
}

fn pattern_nodes(
    backend: &SqliteGraphBackend,
    query: &PatternQuery,
) -> Result<Vec<i64>, SqliteGraphError> {
    let graph = backend.graph();
    let mut nodes = AHashSet::new();
    for id in graph.all_entity_ids()? {
        let matches = GraphQuery::new(graph).pattern_matches(id, query)?;
        for m in matches {
            for node in m.nodes {
                nodes.insert(node);
            }
        }
    }
    Ok(sorted(nodes))
}

fn expand_khops(
    backend: &SqliteGraphBackend,
    seeds: &[i64],
    depth: u32,
) -> Result<Vec<i64>, SqliteGraphError> {
    let graph = backend.graph();
    let mut nodes: AHashSet<i64> = seeds.iter().copied().collect();
    for &seed in seeds {
        let hops = multi_hop::k_hop(graph, seed, depth, BackendDirection::Outgoing)?;
        nodes.extend(hops);
    }
    Ok(sorted(nodes))
}

fn apply_filter(
    backend: &SqliteGraphBackend,
    candidates: &[i64],
    constraint: &NodeConstraint,
) -> Result<Vec<i64>, SqliteGraphError> {
    let graph = backend.graph();
    let mut filtered = Vec::new();
    for &node in candidates {
        let entity = graph.get_entity(node)?;
        if constraint.matches(&entity) {
            filtered.push(node);
        }
    }
    filtered.sort_unstable();
    filtered.dedup();
    Ok(filtered)
}

fn score_nodes(
    backend: &SqliteGraphBackend,
    nodes: &[i64],
    config: &ReasoningScoreConfig,
) -> Result<Vec<(i64, f64)>, SqliteGraphError> {
    let graph = backend.graph();
    let mut scored = Vec::new();
    for &node in nodes {
        let hops = if config.hop_depth == 0 {
            0
        } else {
            multi_hop::k_hop(graph, node, config.hop_depth, BackendDirection::Outgoing)?.len()
        };
        let (out, incoming) = backend.node_degree(node)?;
        let degree = (out + incoming) as f64;
        let score = hops as f64 + config.degree_weight * degree;
        scored.push((node, score));
    }
    scored.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    Ok(scored)
}

fn sorted(set: AHashSet<i64>) -> Vec<i64> {
    let mut data: Vec<i64> = set.into_iter().collect();
    data.sort_unstable();
    data
}
