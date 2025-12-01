use crate::{
    SqliteGraphError,
    backend::{GraphBackend, NeighborQuery},
    dual_orchestrator::{DualGraphHarness, HarnessDiff},
};

#[derive(Clone, Debug)]
pub struct DualRunConfig {
    pub node: i64,
    pub query: NeighborQuery,
    pub depth: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum DualRunResult {
    Match,
    Mismatch { base: Vec<i64>, other: Vec<i64> },
}

pub fn run_dual_check<B1, B2>(
    base: B1,
    other: B2,
    config: DualRunConfig,
) -> Result<DualRunResult, SqliteGraphError>
where
    B1: GraphBackend,
    B2: GraphBackend,
{
    let harness = DualGraphHarness::new(base, other);
    match harness.compare_neighbors(config.node, config.query)? {
        HarnessDiff::Match => match harness.compare_bfs(config.node, config.depth)? {
            HarnessDiff::Match => Ok(DualRunResult::Match),
            HarnessDiff::Mismatch { base, other } => Ok(DualRunResult::Mismatch { base, other }),
        },
        HarnessDiff::Mismatch { base, other } => Ok(DualRunResult::Mismatch { base, other }),
    }
}
