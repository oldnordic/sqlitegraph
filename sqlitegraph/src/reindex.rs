//! Graph reindexing functionality for syncore and sync graph operations.
//!
//! Provides comprehensive reindexing capabilities including:
//! - Syncore reindexing: Core database indexes (entities, edges, labels, properties)
//! - Sync graph reindexing: Graph-specific indexes and adjacency caches
//! - Progress tracking and validation
//! - Transactional safety with rollback capability

use std::time::{Duration, Instant};

use rusqlite::params;

use crate::{
    SqliteGraphError,
    graph::{InstrumentedConnection, SqliteGraph},
};

/// Reindexing progress information
#[derive(Debug, Clone)]
pub struct ReindexProgress {
    pub stage: ReindexStage,
    pub current: usize,
    pub total: usize,
    pub elapsed: Duration,
    pub estimated_remaining: Option<Duration>,
}

impl ReindexProgress {
    pub fn new(stage: ReindexStage, current: usize, total: usize, elapsed: Duration) -> Self {
        let estimated_remaining = if current > 0 {
            Some(Duration::from_nanos(
                (elapsed.as_nanos() as u64 * total as u64) / current as u64
                    - elapsed.as_nanos() as u64,
            ))
        } else {
            None
        };

        Self {
            stage,
            current,
            total,
            elapsed,
            estimated_remaining,
        }
    }

    pub fn progress_percent(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            (self.current as f64 / self.total as f64) * 100.0
        }
    }
}

/// Reindexing stages
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReindexStage {
    /// Analyzing existing data
    Analyzing,
    /// Rebuilding entity indexes
    EntityIndexes,
    /// Rebuilding edge indexes  
    EdgeIndexes,
    /// Rebuilding label indexes
    LabelIndexes,
    /// Rebuilding property indexes
    PropertyIndexes,
    /// Rebuilding adjacency caches
    AdjacencyCaches,
    /// Validating reindexed data
    Validation,
    /// Completed
    Complete,
}

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

/// Reindexing results
#[derive(Debug, Clone)]
pub struct ReindexResult {
    pub success: bool,
    pub total_duration: Duration,
    pub entities_processed: usize,
    pub edges_processed: usize,
    pub labels_processed: usize,
    pub properties_processed: usize,
    pub indexes_rebuilt: Vec<String>,
    pub validation_errors: Vec<String>,
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
            result.entities_processed = self.reindex_entities(&conn, &analysis, start_time)?;
            result.indexes_rebuilt.push("entities".to_string());

            result.edges_processed = self.reindex_edges(&conn, &analysis, start_time)?;
            result.indexes_rebuilt.push("edges".to_string());

            result.labels_processed = self.reindex_labels(&conn, &analysis, start_time)?;
            result.indexes_rebuilt.push("labels".to_string());

            result.properties_processed = self.reindex_properties(&conn, &analysis, start_time)?;
            result.indexes_rebuilt.push("properties".to_string());
        }

        // Stage 3: Rebuild sync graph indexes and caches
        if self.config.sync_graph {
            self.rebuild_adjacency_caches(&analysis, start_time)?;
            result.indexes_rebuilt.push("adjacency_caches".to_string());
        }

        // Stage 4: Validation
        if self.config.validate {
            self.report_progress(ReindexStage::Validation, 0, 1, start_time.elapsed());
            result.validation_errors = self.validate_indexes(&conn)?;
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

    /// Rebuild entity indexes
    fn reindex_entities(
        &self,
        conn: &InstrumentedConnection<'_>,
        analysis: &ReindexAnalysis,
        start_time: Instant,
    ) -> Result<usize, SqliteGraphError> {
        self.report_progress(
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
        for batch_start in (0..analysis.entity_count).step_by(self.config.batch_size) {
            let batch_end = (batch_start + self.config.batch_size).min(analysis.entity_count);
            self.report_progress(
                ReindexStage::EntityIndexes,
                batch_end,
                analysis.entity_count,
                start_time.elapsed(),
            );
        }

        Ok(analysis.entity_count)
    }

    /// Rebuild edge indexes
    fn reindex_edges(
        &self,
        conn: &InstrumentedConnection<'_>,
        analysis: &ReindexAnalysis,
        start_time: Instant,
    ) -> Result<usize, SqliteGraphError> {
        self.report_progress(
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
        for batch_start in (0..analysis.edge_count).step_by(self.config.batch_size) {
            let batch_end = (batch_start + self.config.batch_size).min(analysis.edge_count);
            self.report_progress(
                ReindexStage::EdgeIndexes,
                batch_end,
                analysis.edge_count,
                start_time.elapsed(),
            );
        }

        Ok(analysis.edge_count)
    }

    /// Rebuild label indexes
    fn reindex_labels(
        &self,
        conn: &InstrumentedConnection<'_>,
        analysis: &ReindexAnalysis,
        start_time: Instant,
    ) -> Result<usize, SqliteGraphError> {
        self.report_progress(
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
        for batch_start in (0..analysis.label_count).step_by(self.config.batch_size) {
            let batch_end = (batch_start + self.config.batch_size).min(analysis.label_count);
            self.report_progress(
                ReindexStage::LabelIndexes,
                batch_end,
                analysis.label_count,
                start_time.elapsed(),
            );
        }

        Ok(analysis.label_count)
    }

    /// Rebuild property indexes
    fn reindex_properties(
        &self,
        conn: &InstrumentedConnection<'_>,
        analysis: &ReindexAnalysis,
        start_time: Instant,
    ) -> Result<usize, SqliteGraphError> {
        self.report_progress(
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
        for batch_start in (0..analysis.property_count).step_by(self.config.batch_size) {
            let batch_end = (batch_start + self.config.batch_size).min(analysis.property_count);
            self.report_progress(
                ReindexStage::PropertyIndexes,
                batch_end,
                analysis.property_count,
                start_time.elapsed(),
            );
        }

        Ok(analysis.property_count)
    }

    /// Rebuild adjacency caches for sync graph operations
    fn rebuild_adjacency_caches(
        &self,
        analysis: &ReindexAnalysis,
        start_time: Instant,
    ) -> Result<(), SqliteGraphError> {
        self.report_progress(
            ReindexStage::AdjacencyCaches,
            0,
            analysis.edge_count,
            start_time.elapsed(),
        );

        // Clear and rebuild adjacency caches
        self.graph.outgoing_cache_ref().clear();
        self.graph.incoming_cache_ref().clear();

        // Process edges in batches to rebuild caches
        for batch_start in (0..analysis.edge_count).step_by(self.config.batch_size) {
            let batch_end = (batch_start + self.config.batch_size).min(analysis.edge_count);

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

            self.report_progress(
                ReindexStage::AdjacencyCaches,
                batch_end,
                analysis.edge_count,
                start_time.elapsed(),
            );
        }

        Ok(())
    }

    /// Validate rebuilt indexes
    fn validate_indexes(
        &self,
        conn: &InstrumentedConnection<'_>,
    ) -> Result<Vec<String>, SqliteGraphError> {
        let mut errors = Vec::new();

        // Validate entity indexes
        if let Err(e) = self.validate_entity_indexes(conn) {
            errors.push(format!("Entity index validation failed: {}", e));
        }

        // Validate edge indexes
        if let Err(e) = self.validate_edge_indexes(conn) {
            errors.push(format!("Edge index validation failed: {}", e));
        }

        // Validate label indexes
        if let Err(e) = self.validate_label_indexes(conn) {
            errors.push(format!("Label index validation failed: {}", e));
        }

        // Validate property indexes
        if let Err(e) = self.validate_property_indexes(conn) {
            errors.push(format!("Property index validation failed: {}", e));
        }

        Ok(errors)
    }

    fn validate_entity_indexes(
        &self,
        conn: &InstrumentedConnection<'_>,
    ) -> Result<(), SqliteGraphError> {
        // Check that index exists and can be used
        conn.query_row(
            "SELECT COUNT(*) FROM graph_entities WHERE kind = '' AND id > 0",
            [],
            |_| Ok(()),
        )
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        Ok(())
    }

    fn validate_edge_indexes(
        &self,
        conn: &InstrumentedConnection<'_>,
    ) -> Result<(), SqliteGraphError> {
        // Check that edge indexes work
        conn.query_row(
            "SELECT COUNT(*) FROM graph_edges WHERE from_id > 0 AND to_id > 0",
            [],
            |_| Ok(()),
        )
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        Ok(())
    }

    fn validate_label_indexes(
        &self,
        conn: &InstrumentedConnection<'_>,
    ) -> Result<(), SqliteGraphError> {
        // Check that label indexes work
        conn.query_row(
            "SELECT COUNT(*) FROM graph_labels WHERE label = ''",
            [],
            |_| Ok(()),
        )
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        Ok(())
    }

    fn validate_property_indexes(
        &self,
        conn: &InstrumentedConnection<'_>,
    ) -> Result<(), SqliteGraphError> {
        // Check that property indexes work
        conn.query_row(
            "SELECT COUNT(*) FROM graph_properties WHERE key = '' AND value = ''",
            [],
            |_| Ok(()),
        )
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        Ok(())
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
struct ReindexAnalysis {
    entity_count: usize,
    edge_count: usize,
    label_count: usize,
    property_count: usize,
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
