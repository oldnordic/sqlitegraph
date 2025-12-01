//! Label and property index rebuilding operations

use std::time::{Duration, Instant};

use crate::{
    SqliteGraphError,
    graph::{InstrumentedConnection, SqliteGraph},
};

use super::{core::ReindexAnalysis, progress::ReindexStage};

/// Label and property index rebuilding operations
pub struct LabelPropertyRebuilder<'a> {
    graph: &'a SqliteGraph,
    batch_size: usize,
}

impl<'a> LabelPropertyRebuilder<'a> {
    pub fn new(graph: &'a SqliteGraph, batch_size: usize) -> Self {
        Self { graph, batch_size }
    }

    /// Rebuild label indexes
    pub fn reindex_labels(
        &self,
        conn: &InstrumentedConnection<'_>,
        analysis: &ReindexAnalysis,
        start_time: Instant,
        report_progress: &dyn Fn(ReindexStage, usize, usize, Duration),
    ) -> Result<usize, SqliteGraphError> {
        report_progress(
            ReindexStage::LabelIndexes,
            0,
            analysis.label_count,
            start_time.elapsed(),
        );

        // Drop and recreate label indexes
        conn.execute("DROP INDEX IF EXISTS idx_labels_label", [])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        conn.execute("DROP INDEX IF EXISTS idx_labels_label_entity_id", [])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        conn.execute("CREATE INDEX idx_labels_label ON graph_labels(label)", [])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        conn.execute(
            "CREATE INDEX idx_labels_label_entity_id ON graph_labels(label, entity_id)",
            [],
        )
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;

        // Process in batches and report progress
        for batch_start in (0..analysis.label_count).step_by(self.batch_size) {
            let batch_end = (batch_start + self.batch_size).min(analysis.label_count);
            report_progress(
                ReindexStage::LabelIndexes,
                batch_end,
                analysis.label_count,
                start_time.elapsed(),
            );
        }

        Ok(analysis.label_count)
    }

    /// Rebuild property indexes
    pub fn reindex_properties(
        &self,
        conn: &InstrumentedConnection<'_>,
        analysis: &ReindexAnalysis,
        start_time: Instant,
        report_progress: &dyn Fn(ReindexStage, usize, usize, Duration),
    ) -> Result<usize, SqliteGraphError> {
        report_progress(
            ReindexStage::PropertyIndexes,
            0,
            analysis.property_count,
            start_time.elapsed(),
        );

        // Drop and recreate property indexes
        conn.execute("DROP INDEX IF EXISTS idx_props_key_value", [])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        conn.execute("DROP INDEX IF EXISTS idx_props_key_value_entity_id", [])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        conn.execute(
            "CREATE INDEX idx_props_key_value ON graph_properties(key, value)",
            [],
        )
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        conn.execute(
            "CREATE INDEX idx_props_key_value_entity_id ON graph_properties(key, value, entity_id)",
            [],
        )
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;

        // Process in batches and report progress
        for batch_start in (0..analysis.property_count).step_by(self.batch_size) {
            let batch_end = (batch_start + self.batch_size).min(analysis.property_count);
            report_progress(
                ReindexStage::PropertyIndexes,
                batch_end,
                analysis.property_count,
                start_time.elapsed(),
            );
        }

        Ok(analysis.property_count)
    }
}
