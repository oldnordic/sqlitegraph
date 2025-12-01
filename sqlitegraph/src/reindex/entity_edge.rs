//! Entity and edge index rebuilding operations

use std::time::{Duration, Instant};

use crate::{
    SqliteGraphError,
    graph::{InstrumentedConnection, SqliteGraph},
};

use super::{core::ReindexAnalysis, progress::ReindexStage};

/// Entity and edge index rebuilding operations
pub struct EntityEdgeRebuilder<'a> {
    graph: &'a SqliteGraph,
    batch_size: usize,
}

impl<'a> EntityEdgeRebuilder<'a> {
    pub fn new(graph: &'a SqliteGraph, batch_size: usize) -> Self {
        Self { graph, batch_size }
    }

    /// Rebuild entity indexes
    pub fn reindex_entities(
        &self,
        conn: &InstrumentedConnection<'_>,
        analysis: &ReindexAnalysis,
        start_time: Instant,
        report_progress: &dyn Fn(ReindexStage, usize, usize, Duration),
    ) -> Result<usize, SqliteGraphError> {
        report_progress(
            ReindexStage::EntityIndexes,
            0,
            analysis.entity_count,
            start_time.elapsed(),
        );

        // Drop and recreate entity indexes
        conn.execute("DROP INDEX IF EXISTS idx_entities_kind_id", [])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        conn.execute(
            "CREATE INDEX idx_entities_kind_id ON graph_entities(kind, id)",
            [],
        )
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;

        // Process in batches and report progress
        for batch_start in (0..analysis.entity_count).step_by(self.batch_size) {
            let batch_end = (batch_start + self.batch_size).min(analysis.entity_count);
            report_progress(
                ReindexStage::EntityIndexes,
                batch_end,
                analysis.entity_count,
                start_time.elapsed(),
            );
        }

        Ok(analysis.entity_count)
    }

    /// Rebuild edge indexes
    pub fn reindex_edges(
        &self,
        conn: &InstrumentedConnection<'_>,
        analysis: &ReindexAnalysis,
        start_time: Instant,
        report_progress: &dyn Fn(ReindexStage, usize, usize, Duration),
    ) -> Result<usize, SqliteGraphError> {
        report_progress(
            ReindexStage::EdgeIndexes,
            0,
            analysis.edge_count,
            start_time.elapsed(),
        );

        // Drop and recreate edge indexes
        conn.execute("DROP INDEX IF EXISTS idx_edges_from", [])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        conn.execute("DROP INDEX IF EXISTS idx_edges_to", [])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        conn.execute("DROP INDEX IF EXISTS idx_edges_type", [])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        conn.execute("CREATE INDEX idx_edges_from ON graph_edges(from_id)", [])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        conn.execute("CREATE INDEX idx_edges_to ON graph_edges(to_id)", [])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        conn.execute("CREATE INDEX idx_edges_type ON graph_edges(edge_type)", [])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;

        // Process in batches and report progress
        for batch_start in (0..analysis.edge_count).step_by(self.batch_size) {
            let batch_end = (batch_start + self.batch_size).min(analysis.edge_count);
            report_progress(
                ReindexStage::EdgeIndexes,
                batch_end,
                analysis.edge_count,
                start_time.elapsed(),
            );
        }

        Ok(analysis.edge_count)
    }
}
