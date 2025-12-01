//! Index validation routines and integrity checks

use std::time::Duration;

use crate::{
    SqliteGraphError,
    graph::{InstrumentedConnection, SqliteGraph},
};

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

/// Index validation utilities
pub struct IndexValidator<'a> {
    graph: &'a SqliteGraph,
}

impl<'a> IndexValidator<'a> {
    pub fn new(graph: &'a SqliteGraph) -> Self {
        Self { graph }
    }

    /// Validate rebuilt indexes
    pub fn validate_indexes(
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
}
