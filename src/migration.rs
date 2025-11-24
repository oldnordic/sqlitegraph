use crate::{
    SqliteGraphError,
    backend::{EdgeSpec, GraphBackend, NodeSpec, SqliteGraphBackend},
    dual_runtime::{DualRuntime, DualRuntimeJob, DualRuntimeReport},
};

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct MigrationStats {
    pub nodes_written: usize,
    pub edges_written: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationIds {
    pub base: i64,
    pub shadow: i64,
}

pub struct MigrationManager {
    base: SqliteGraphBackend,
    shadow: SqliteGraphBackend,
    stats: MigrationStats,
    cutover: bool,
}

impl MigrationManager {
    pub fn new(base: SqliteGraphBackend, shadow: SqliteGraphBackend) -> Self {
        Self {
            base,
            shadow,
            stats: MigrationStats::default(),
            cutover: false,
        }
    }

    pub fn insert_node(&mut self, node: NodeSpec) -> Result<MigrationIds, SqliteGraphError> {
        let base_id = self.base.insert_node(node.clone())?;
        let shadow_id = self.shadow.insert_node(node)?;
        self.stats.nodes_written += 1;
        Ok(MigrationIds {
            base: base_id,
            shadow: shadow_id,
        })
    }

    pub fn insert_edge(&mut self, edge: EdgeSpec) -> Result<(), SqliteGraphError> {
        self.base.insert_edge(edge.clone())?;
        self.shadow.insert_edge(edge)?;
        self.stats.edges_written += 1;
        Ok(())
    }

    pub fn shadow_read(&self, job: &DualRuntimeJob) -> Result<DualRuntimeReport, SqliteGraphError> {
        let runtime = DualRuntime::new(&self.base, &self.shadow);
        runtime.run(job)
    }

    pub fn cutover(&mut self) {
        self.cutover = true;
    }

    pub fn is_cutover(&self) -> bool {
        self.cutover
    }

    pub fn active_backend(&self) -> &SqliteGraphBackend {
        if self.cutover {
            &self.shadow
        } else {
            &self.base
        }
    }

    pub fn base_backend(&self) -> &SqliteGraphBackend {
        &self.base
    }

    pub fn shadow_backend(&self) -> &SqliteGraphBackend {
        &self.shadow
    }

    pub fn stats(&self) -> &MigrationStats {
        &self.stats
    }
}
