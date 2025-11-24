use crate::{
    SqliteGraphError,
    backend::{EdgeSpec, GraphBackend, NodeSpec},
};

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct MirrorStats {
    pub nodes_written: usize,
    pub edges_written: usize,
}

pub struct DualWriter<B1, B2> {
    base: B1,
    mirror: B2,
    stats: MirrorStats,
}

pub struct DualIds {
    pub base: i64,
    pub mirror: i64,
}

impl<B1, B2> DualWriter<B1, B2>
where
    B1: GraphBackend,
    B2: GraphBackend,
{
    pub fn new(base: B1, mirror: B2) -> Self {
        Self {
            base,
            mirror,
            stats: MirrorStats::default(),
        }
    }

    pub fn insert_node(&mut self, spec: NodeSpec) -> Result<DualIds, SqliteGraphError> {
        let base_id = self.base.insert_node(spec.clone())?;
        let mirror_id = self.mirror.insert_node(spec)?;
        self.stats.nodes_written += 1;
        Ok(DualIds {
            base: base_id,
            mirror: mirror_id,
        })
    }

    pub fn insert_edge(&mut self, spec: EdgeSpec) -> Result<(), SqliteGraphError> {
        self.base.insert_edge(spec.clone())?;
        self.mirror.insert_edge(spec)?;
        self.stats.edges_written += 1;
        Ok(())
    }

    pub fn stats(&self) -> MirrorStats {
        self.stats.clone()
    }

    pub fn into_backends(self) -> (B1, B2, MirrorStats) {
        (self.base, self.mirror, self.stats)
    }
}
