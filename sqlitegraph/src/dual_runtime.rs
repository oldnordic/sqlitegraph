use crate::{
    SqliteGraphError,
    backend::{GraphBackend, NeighborQuery},
    dual_orchestrator::{DualGraphHarness, HarnessDiff},
};

#[derive(Clone, Debug)]
pub struct DualRuntimeJob {
    pub nodes: Vec<i64>,
    pub neighbor_query: NeighborQuery,
    pub depth: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DualDiff {
    Neighbors { base: Vec<i64>, other: Vec<i64> },
    Bfs { base: Vec<i64>, other: Vec<i64> },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DualRuntimeEvent {
    pub node: i64,
    pub diff: DualDiff,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DualRuntimeReport {
    pub total: usize,
    pub matches: usize,
    pub diffs: Vec<DualRuntimeEvent>,
    pub log: Vec<String>,
}

pub struct DualRuntime<B1, B2> {
    harness: DualGraphHarness<B1, B2>,
}

impl<B1, B2> DualRuntime<B1, B2>
where
    B1: GraphBackend,
    B2: GraphBackend,
{
    pub fn new(base: B1, other: B2) -> Self {
        Self {
            harness: DualGraphHarness::new(base, other),
        }
    }

    pub fn run(&self, job: &DualRuntimeJob) -> Result<DualRuntimeReport, SqliteGraphError> {
        let mut diffs = Vec::new();
        let mut log = Vec::new();
        let mut matches = 0;
        for node in &job.nodes {
            let query = job.neighbor_query.clone();
            match self.harness.compare_neighbors(*node, query)? {
                HarnessDiff::Match => {
                    log.push(format!("node {node} neighbors match"));
                    match self.harness.compare_bfs(*node, job.depth)? {
                        HarnessDiff::Match => {
                            log.push(format!("node {node} bfs match"));
                            matches += 1;
                        }
                        HarnessDiff::Mismatch { base, other } => {
                            log.push(format!("node {node} bfs mismatch"));
                            diffs.push(DualRuntimeEvent {
                                node: *node,
                                diff: DualDiff::Bfs { base, other },
                            });
                        }
                    }
                }
                HarnessDiff::Mismatch { base, other } => {
                    log.push(format!("node {node} neighbors mismatch"));
                    diffs.push(DualRuntimeEvent {
                        node: *node,
                        diff: DualDiff::Neighbors { base, other },
                    });
                }
            }
        }
        Ok(DualRuntimeReport {
            total: job.nodes.len(),
            matches,
            diffs,
            log,
        })
    }
}
