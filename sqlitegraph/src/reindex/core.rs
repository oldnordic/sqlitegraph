//! Core reindexing functionality and orchestration

use std::time::{Duration, Instant};

use crate::{
    SqliteGraphError,
    graph::{InstrumentedConnection, SqliteGraph},
};

use super::{
    cache::CacheRebuilder,
    entity_edge::EntityEdgeRebuilder,
    label_property::LabelPropertyRebuilder,
    progress::{ReindexProgress, ReindexStage},
    validation::{IndexValidator, ReindexResult},
};

/// Reindexing configuration
pub struct ReindexConfig {
    /// Whether to rebuild core syncore indexes
    pub syncore: bool,
    /// Whether to rebuild sync graph indexes and caches
    pub sync_graph: bool,
    /// Whether to validate indexes after rebuilding
    pub validate: bool,
    /// Batch size for processing large datasets
    pub batch_size: usize,
    /// Progress callback (called for each batch)
    pub progress_callback: Option<Box<dyn Fn(ReindexProgress) + Send + Sync>>,
}

impl std::fmt::Debug for ReindexConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ReindexConfig")
            .field("syncore", &self.syncore)
            .field("sync_graph", &self.sync_graph)
            .field("validate", &self.validate)
            .field("batch_size", &self.batch_size)
            .field("progress_callback", &self.progress_callback.is_some())
            .finish()
    }
}

impl Default for ReindexConfig {
    fn default() -> Self {
        Self {
            syncore: true,
            sync_graph: true,
            validate: true,
            batch_size: 1000,
            progress_callback: None,
        }
    }
}

/// Main reindexing functionality
pub struct GraphReindexer<'a> {
    graph: &'a SqliteGraph,
    config: ReindexConfig,
}

impl<'a> GraphReindexer<'a> {
    pub fn new(graph: &'a SqliteGraph, config: ReindexConfig) -> Self {
        Self { graph, config }
    }

    /// Perform complete reindexing according to configuration
    pub fn reindex(&self) -> Result<ReindexResult, SqliteGraphError> {
        let start_time = Instant::now();
        let mut result = ReindexResult {
            success: false,
            total_duration: Duration::ZERO,
            entities_processed: 0,
            edges_processed: 0,
            labels_processed: 0,
            properties_processed: 0,
            indexes_rebuilt: Vec::new(),
            validation_errors: Vec::new(),
        };

        // Use existing connection directly for simplicity
        let conn = self.graph.connection();

        // Stage 1: Analyze existing data
        self.report_progress(ReindexStage::Analyzing, 0, 1, start_time.elapsed());
        let analysis = self.analyze_data(&conn)?;
        self.report_progress(ReindexStage::Analyzing, 1, 1, start_time.elapsed());

        // Stage 2: Rebuild syncore indexes
        if self.config.syncore {
            let entity_rebuilder = EntityEdgeRebuilder::new(self.graph, self.config.batch_size);
            result.entities_processed = entity_rebuilder.reindex_entities(
                &conn,
                &analysis,
                start_time,
                &|stage, current, total, elapsed| {
                    self.report_progress(stage, current, total, elapsed);
                },
            )?;
            result.indexes_rebuilt.push("entities".to_string());

            result.edges_processed = entity_rebuilder.reindex_edges(
                &conn,
                &analysis,
                start_time,
                &|stage, current, total, elapsed| {
                    self.report_progress(stage, current, total, elapsed);
                },
            )?;
            result.indexes_rebuilt.push("edges".to_string());

            let label_rebuilder = LabelPropertyRebuilder::new(self.graph, self.config.batch_size);
            result.labels_processed = label_rebuilder.reindex_labels(
                &conn,
                &analysis,
                start_time,
                &|stage, current, total, elapsed| {
                    self.report_progress(stage, current, total, elapsed);
                },
            )?;
            result.indexes_rebuilt.push("labels".to_string());

            result.properties_processed = label_rebuilder.reindex_properties(
                &conn,
                &analysis,
                start_time,
                &|stage, current, total, elapsed| {
                    self.report_progress(stage, current, total, elapsed);
                },
            )?;
            result.indexes_rebuilt.push("properties".to_string());
        }

        // Stage 3: Rebuild sync graph indexes and caches
        if self.config.sync_graph {
            let cache_rebuilder = CacheRebuilder::new(self.graph, self.config.batch_size);
            cache_rebuilder.rebuild_adjacency_caches(
                &analysis,
                start_time,
                &|stage, current, total, elapsed| {
                    self.report_progress(stage, current, total, elapsed);
                },
            )?;
            result.indexes_rebuilt.push("adjacency_caches".to_string());
        }

        // Stage 3: Rebuild sync graph indexes and caches
        if self.config.sync_graph {
            let rebuilder = CacheRebuilder::new(self.graph, self.config.batch_size);
            rebuilder.rebuild_adjacency_caches(
                &analysis,
                start_time,
                &|stage, current, total, elapsed| {
                    self.report_progress(stage, current, total, elapsed);
                },
            )?;
            result.indexes_rebuilt.push("adjacency_caches".to_string());
        }

        // Stage 4: Validation
        if self.config.validate {
            self.report_progress(ReindexStage::Validation, 0, 1, start_time.elapsed());
            let validator = IndexValidator::new(self.graph);
            result.validation_errors = validator.validate_indexes(&conn)?;
            self.report_progress(ReindexStage::Validation, 1, 1, start_time.elapsed());
        }

        self.report_progress(ReindexStage::Complete, 1, 1, start_time.elapsed());

        result.total_duration = start_time.elapsed();
        result.success = true;
        Ok(result)
    }

    /// Analyze existing data to plan reindexing
    fn analyze_data(
        &self,
        conn: &InstrumentedConnection<'_>,
    ) -> Result<ReindexAnalysis, SqliteGraphError> {
        let entity_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM graph_entities", [], |row| row.get(0))
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;

        let edge_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM graph_edges", [], |row| row.get(0))
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;

        let label_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM graph_labels", [], |row| row.get(0))
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;

        let property_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM graph_properties", [], |row| {
                row.get(0)
            })
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;

        Ok(ReindexAnalysis {
            entity_count: entity_count as usize,
            edge_count: edge_count as usize,
            label_count: label_count as usize,
            property_count: property_count as usize,
        })
    }

    fn report_progress(
        &self,
        stage: ReindexStage,
        current: usize,
        total: usize,
        elapsed: Duration,
    ) {
        if let Some(ref callback) = self.config.progress_callback {
            let progress = ReindexProgress::new(stage, current, total, elapsed);
            callback(progress);
        }
    }
}

/// Data analysis results for reindexing planning
#[derive(Debug, Clone)]
pub struct ReindexAnalysis {
    pub entity_count: usize,
    pub edge_count: usize,
    pub label_count: usize,
    pub property_count: usize,
}

/// Convenience functions for common reindexing operations
impl SqliteGraph {
    /// Rebuild all indexes (syncore + sync graph)
    pub fn reindex_all(&self) -> Result<ReindexResult, SqliteGraphError> {
        let config = ReindexConfig::default();
        let reindexer = GraphReindexer::new(self, config);
        reindexer.reindex()
    }

    /// Rebuild only syncore indexes (entities, edges, labels, properties)
    pub fn reindex_syncore(&self) -> Result<ReindexResult, SqliteGraphError> {
        let config = ReindexConfig {
            syncore: true,
            sync_graph: false,
            validate: true,
            batch_size: 1000,
            progress_callback: None,
        };
        let reindexer = GraphReindexer::new(self, config);
        reindexer.reindex()
    }

    /// Rebuild only sync graph indexes and caches
    pub fn reindex_sync_graph(&self) -> Result<ReindexResult, SqliteGraphError> {
        let config = ReindexConfig {
            syncore: false,
            sync_graph: true,
            validate: true,
            batch_size: 1000,
            progress_callback: None,
        };
        let reindexer = GraphReindexer::new(self, config);
        reindexer.reindex()
    }

    /// Rebuild with custom configuration and progress callback
    pub fn reindex_with_config(
        &self,
        config: ReindexConfig,
    ) -> Result<ReindexResult, SqliteGraphError> {
        let reindexer = GraphReindexer::new(self, config);
        reindexer.reindex()
    }
}
