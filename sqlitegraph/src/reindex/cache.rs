//! Adjacency cache rebuilding operations

use std::time::{Duration, Instant};

use rusqlite::params;

use crate::{SqliteGraphError, graph::SqliteGraph};

use super::{core::ReindexAnalysis, progress::ReindexStage};

/// Adjacency cache rebuilding operations
pub struct CacheRebuilder<'a> {
    graph: &'a SqliteGraph,
    batch_size: usize,
}

impl<'a> CacheRebuilder<'a> {
    pub fn new(graph: &'a SqliteGraph, batch_size: usize) -> Self {
        Self { graph, batch_size }
    }

    /// Rebuild adjacency caches for sync graph operations
    pub fn rebuild_adjacency_caches(
        &self,
        analysis: &ReindexAnalysis,
        start_time: Instant,
        report_progress: &dyn Fn(ReindexStage, usize, usize, Duration),
    ) -> Result<(), SqliteGraphError> {
        report_progress(
            ReindexStage::AdjacencyCaches,
            0,
            analysis.edge_count,
            start_time.elapsed(),
        );

        // Clear and rebuild adjacency caches
        self.graph.outgoing_cache_ref().clear();
        self.graph.incoming_cache_ref().clear();

        // Process edges in batches to rebuild caches
        for batch_start in (0..analysis.edge_count).step_by(self.batch_size) {
            let batch_end = (batch_start + self.batch_size).min(analysis.edge_count);

            // Load edges for this batch and update caches
            let offset = batch_start as i64;
            let limit = (batch_end - batch_start) as i64;

            let conn = self.graph.connection();
            let mut stmt = conn
                .prepare_cached(
                    "SELECT from_id, to_id FROM graph_edges ORDER BY id LIMIT ?1 OFFSET ?2",
                )
                .map_err(|e| SqliteGraphError::query(e.to_string()))?;

            let rows = stmt
                .query_map(params![limit, offset], |row| {
                    Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?))
                })
                .map_err(|e| SqliteGraphError::query(e.to_string()))?;

            for row in rows {
                let (from_id, to_id) = row.map_err(|e| SqliteGraphError::query(e.to_string()))?;

                // Add to outgoing cache
                if let Some(mut outgoing) = self.graph.outgoing_cache_ref().get(from_id) {
                    outgoing.push(to_id);
                    self.graph.outgoing_cache_ref().insert(from_id, outgoing);
                } else {
                    self.graph.outgoing_cache_ref().insert(from_id, vec![to_id]);
                }

                // Add to incoming cache
                if let Some(mut incoming) = self.graph.incoming_cache_ref().get(to_id) {
                    incoming.push(from_id);
                    self.graph.incoming_cache_ref().insert(to_id, incoming);
                } else {
                    self.graph.incoming_cache_ref().insert(to_id, vec![from_id]);
                }
            }

            report_progress(
                ReindexStage::AdjacencyCaches,
                batch_end,
                analysis.edge_count,
                start_time.elapsed(),
            );
        }

        Ok(())
    }
}
