use ahash::AHashMap;

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
    id_map: AHashMap<i64, i64>,
}

impl MigrationManager {
    pub fn new(base: SqliteGraphBackend, shadow: SqliteGraphBackend) -> Self {
        Self {
            base,
            shadow,
            stats: MigrationStats::default(),
            cutover: false,
            id_map: AHashMap::new(),
        }
    }

    pub fn insert_node(&mut self, node: NodeSpec) -> Result<MigrationIds, SqliteGraphError> {
        let base_id = self.base.insert_node(node.clone())?;
        let shadow_id = self.shadow.insert_node(node)?;
        self.stats.nodes_written += 1;
        self.id_map.insert(base_id, shadow_id);
        Ok(MigrationIds {
            base: base_id,
            shadow: shadow_id,
        })
    }

    pub fn insert_edge(&mut self, edge: EdgeSpec) -> Result<(), SqliteGraphError> {
        self.base.insert_edge(edge.clone())?;
        let shadow_edge = EdgeSpec {
            from: self.shadow_id(edge.from)?,
            to: self.shadow_id(edge.to)?,
            edge_type: edge.edge_type,
            data: edge.data,
        };
        self.shadow.insert_edge(shadow_edge)?;
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

    fn shadow_id(&self, base_id: i64) -> Result<i64, SqliteGraphError> {
        self.id_map.get(&base_id).copied().ok_or_else(|| {
            SqliteGraphError::invalid_input(format!("unknown shadow id for base node {base_id}"))
        })
    }

    pub fn stats(&self) -> &MigrationStats {
        &self.stats
    }
}
