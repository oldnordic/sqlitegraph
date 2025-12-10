use crate::{
    SqliteGraphError,
    backend::{GraphBackend, NeighborQuery},
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
    mut base: B1,
    mut other: B2,
    config: DualRunConfig,
) -> Result<DualRunResult, SqliteGraphError>
where
    B1: GraphBackend,
    B2: GraphBackend,
{
    // For neighbor queries with depth, use BFS instead as it's more appropriate
    let base_neighbors = base.bfs(config.node, config.depth)?;
    let other_neighbors = other.bfs(config.node, config.depth)?;

    if base_neighbors == other_neighbors {
        Ok(DualRunResult::Match)
    } else {
        Ok(DualRunResult::Mismatch {
            base: base_neighbors,
            other: other_neighbors,
        })
    }
}
