use crate::{
    SqliteGraphError,
    backend::{GraphBackend, NeighborQuery},
};

#[derive(Debug, PartialEq, Eq)]
pub enum DualReadResult {
    Match,
    Mismatch { base: Vec<i64>, other: Vec<i64> },
}

pub fn compare_adjacent(base: &[i64], other: &[i64]) -> DualReadResult {
    if base == other {
        DualReadResult::Match
    } else {
        DualReadResult::Mismatch {
            base: base.to_vec(),
            other: other.to_vec(),
        }
    }
}

pub struct DualReader<B1, B2> {
    base: B1,
    other: B2,
}

impl<B1, B2> DualReader<B1, B2> {
    pub fn new(base: B1, other: B2) -> Self {
        Self { base, other }
    }
}

impl<B1, B2> DualReader<B1, B2>
where
    B1: GraphBackend,
    B2: GraphBackend,
{
    pub fn compare_neighbors(
        &self,
        node: i64,
        query: NeighborQuery,
    ) -> Result<DualReadResult, SqliteGraphError> {
        let base_neighbors = self.base.neighbors(node, query.clone())?;
        let other_neighbors = self.other.neighbors(node, query)?;
        Ok(compare_adjacent(&base_neighbors, &other_neighbors))
    }

    pub fn compare_bfs(&self, start: i64, depth: u32) -> Result<DualReadResult, SqliteGraphError> {
        let base_visit = self.base.bfs(start, depth)?;
        let other_visit = self.other.bfs(start, depth)?;
        Ok(compare_adjacent(&base_visit, &other_visit))
    }
}
