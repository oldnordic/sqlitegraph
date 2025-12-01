use crate::{
    SqliteGraphError,
    backend::{GraphBackend, NeighborQuery},
    dual_read::{DualReadResult, compare_adjacent},
};

#[derive(Debug, PartialEq, Eq)]
pub enum HarnessDiff {
    Match,
    Mismatch { base: Vec<i64>, other: Vec<i64> },
}

impl From<DualReadResult> for HarnessDiff {
    fn from(value: DualReadResult) -> Self {
        match value {
            DualReadResult::Match => HarnessDiff::Match,
            DualReadResult::Mismatch { base, other } => HarnessDiff::Mismatch { base, other },
        }
    }
}

pub struct DualGraphHarness<B1, B2> {
    base: B1,
    other: B2,
}

impl<B1, B2> DualGraphHarness<B1, B2> {
    pub fn new(base: B1, other: B2) -> Self {
        Self { base, other }
    }
}

impl<B1, B2> DualGraphHarness<B1, B2>
where
    B1: GraphBackend,
    B2: GraphBackend,
{
    pub fn compare_neighbors(
        &self,
        node: i64,
        query: NeighborQuery,
    ) -> Result<HarnessDiff, SqliteGraphError> {
        let base_neighbors = self.base.neighbors(node, query.clone())?;
        let other_neighbors = self.other.neighbors(node, query)?;
        Ok(compare_adjacent(&base_neighbors, &other_neighbors).into())
    }

    pub fn compare_bfs(&self, node: i64, depth: u32) -> Result<HarnessDiff, SqliteGraphError> {
        let base_visit = self.base.bfs(node, depth)?;
        let other_visit = self.other.bfs(node, depth)?;
        Ok(compare_adjacent(&base_visit, &other_visit).into())
    }
}
