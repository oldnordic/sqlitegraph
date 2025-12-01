//! Adjacency queries and cache management for SqliteGraph.

use rusqlite::{OptionalExtension, params};

use crate::{cache::AdjacencyCache, errors::SqliteGraphError};

use super::{SqliteGraph, metrics::InstrumentedConnection};

impl SqliteGraph {
    pub(crate) fn connection(&self) -> InstrumentedConnection<'_> {
        InstrumentedConnection::new(&self.conn, &self.metrics, &self.statement_tracker)
    }

    /// Get access to the underlying SQLite connection for advanced operations
    pub(crate) fn underlying_connection(&self) -> &rusqlite::Connection {
        &self.conn
    }

    pub(crate) fn fetch_outgoing(&self, id: i64) -> Result<Vec<i64>, SqliteGraphError> {
        if let Some(cached) = self.outgoing_cache.get(id) {
            return Ok(cached);
        }
        let result = self.collect_adjacency(
            "SELECT to_id FROM graph_edges WHERE from_id=?1 ORDER BY to_id, edge_type, id",
            id,
        )?;
        self.outgoing_cache.insert(id, result.clone());
        Ok(result)
    }

    pub(crate) fn fetch_incoming(&self, id: i64) -> Result<Vec<i64>, SqliteGraphError> {
        if let Some(cached) = self.incoming_cache.get(id) {
            return Ok(cached);
        }
        let result = self.collect_adjacency(
            "SELECT from_id FROM graph_edges WHERE to_id=?1 ORDER BY from_id, edge_type, id",
            id,
        )?;
        self.incoming_cache.insert(id, result.clone());
        Ok(result)
    }

    pub(crate) fn invalidate_caches(&self) {
        self.outgoing_cache.clear();
        self.incoming_cache.clear();
    }

    pub fn outgoing_cache_ref(&self) -> &AdjacencyCache {
        &self.outgoing_cache
    }

    pub fn incoming_cache_ref(&self) -> &AdjacencyCache {
        &self.incoming_cache
    }

    pub(crate) fn all_entity_ids(&self) -> Result<Vec<i64>, SqliteGraphError> {
        let conn = self.connection();
        let mut stmt = conn
            .prepare_cached("SELECT id FROM graph_entities ORDER BY id")
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        let rows = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        let mut ids = Vec::new();
        for id in rows {
            ids.push(id.map_err(|e| SqliteGraphError::query(e.to_string()))?);
        }
        Ok(ids)
    }

    fn collect_adjacency(&self, sql: &str, id: i64) -> Result<Vec<i64>, SqliteGraphError> {
        let conn = self.connection();
        let mut stmt = conn
            .prepare_cached(sql)
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        let rows = stmt
            .query_map(params![id], |row| row.get(0))
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        let mut result = Vec::new();
        for item in rows {
            result.push(item.map_err(|e| SqliteGraphError::query(e.to_string()))?);
        }
        Ok(result)
    }

    pub(crate) fn entity_exists(&self, id: i64) -> Result<bool, SqliteGraphError> {
        let exists: Option<i64> = self
            .connection()
            .query_row(
                "SELECT 1 FROM graph_entities WHERE id=?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        Ok(exists.is_some())
    }
}
