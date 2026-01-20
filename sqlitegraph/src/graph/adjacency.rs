//! Adjacency queries and cache management for SqliteGraph.

use rusqlite::{OptionalExtension, params};

use crate::{cache::AdjacencyCache, errors::SqliteGraphError};

use super::{
    SqliteGraph,
    metrics::{InstrumentedConnection, PooledInstrumentedConnection},
};

/// Unified connection wrapper that works for both pooled and direct connections.
pub enum ConnectionWrapper<'a> {
    Borrowed(InstrumentedConnection<'a>),
    Pooled(PooledInstrumentedConnection),
}

impl<'a> ConnectionWrapper<'a> {
    pub fn execute<P>(&self, sql: &str, params: P) -> Result<usize, rusqlite::Error>
    where
        P: rusqlite::Params,
    {
        match self {
            ConnectionWrapper::Borrowed(conn) => conn.execute(sql, params),
            ConnectionWrapper::Pooled(conn) => conn.execute(sql, params),
        }
    }

    pub fn prepare_cached(
        &self,
        sql: &str,
    ) -> Result<StatementWrapper<'_>, rusqlite::Error> {
        match self {
            ConnectionWrapper::Borrowed(conn) => {
                conn.prepare_cached(sql).map(StatementWrapper::Borrowed)
            }
            ConnectionWrapper::Pooled(conn) => {
                conn.prepare_cached(sql).map(StatementWrapper::Pooled)
            }
        }
    }

    pub fn query_row<P, F, R>(
        &self,
        sql: &str,
        params: P,
        f: F,
    ) -> Result<R, rusqlite::Error>
    where
        P: rusqlite::Params,
        F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<R>,
    {
        match self {
            ConnectionWrapper::Borrowed(conn) => conn.query_row(sql, params, f),
            ConnectionWrapper::Pooled(conn) => conn.query_row(sql, params, f),
        }
    }

    pub fn last_insert_rowid(&self) -> i64 {
        match self {
            ConnectionWrapper::Borrowed(conn) => conn.last_insert_rowid(),
            ConnectionWrapper::Pooled(conn) => conn.last_insert_rowid(),
        }
    }

    /// Get access to the underlying connection for advanced operations.
    ///
    /// This provides direct access to the Connection for operations that
    /// cannot be expressed through the wrapper methods.
    pub fn underlying(&self) -> &rusqlite::Connection {
        match self {
            ConnectionWrapper::Borrowed(conn) => conn.inner(),
            ConnectionWrapper::Pooled(conn) => conn.inner(),
        }
    }
}

/// Unified statement wrapper that works for both borrowed and pooled statements.
pub enum StatementWrapper<'a> {
    Borrowed(super::metrics::InstrumentedCachedStatement<'a>),
    Pooled(super::metrics::PooledInstrumentedCachedStatement<'a>),
}

impl<'a> StatementWrapper<'a> {
    pub fn execute<P>(&mut self, params: P) -> Result<usize, rusqlite::Error>
    where
        P: rusqlite::Params,
    {
        match self {
            StatementWrapper::Borrowed(stmt) => stmt.execute(params),
            StatementWrapper::Pooled(stmt) => stmt.execute(params),
        }
    }

    pub fn query_map<P, F, T>(
        &mut self,
        params: P,
        f: F,
    ) -> Result<rusqlite::MappedRows<'_, F>, rusqlite::Error>
    where
        P: rusqlite::Params,
        F: FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>,
    {
        match self {
            StatementWrapper::Borrowed(stmt) => stmt.query_map(params, f),
            StatementWrapper::Pooled(stmt) => stmt.query_map(params, f),
        }
    }

    pub fn query_row<P, F, R>(&mut self, params: P, f: F) -> Result<R, rusqlite::Error>
    where
        P: rusqlite::Params,
        F: FnOnce(&rusqlite::Row<'_>) -> rusqlite::Result<R>,
    {
        match self {
            StatementWrapper::Borrowed(stmt) => stmt.query_row(params, f),
            StatementWrapper::Pooled(stmt) => stmt.query_row(params, f),
        }
    }
}

impl SqliteGraph {
    pub(crate) fn connection(&self) -> ConnectionWrapper<'_> {
        // Check if we have a direct connection (in-memory mode)
        if let Some(conn) = self.pool.direct_connection() {
            return ConnectionWrapper::Borrowed(InstrumentedConnection::new(
                conn,
                &self.metrics,
                &self.statement_tracker,
            ));
        }

        // Otherwise, get a pooled connection
        let conn = self.pool.get()
            .expect("Failed to get connection from pool");
        ConnectionWrapper::Pooled(PooledInstrumentedConnection::new(
            conn,
            self.metrics.clone(),
            self.statement_tracker.clone(),
        ))
    }

    /// Get access to the underlying SQLite connection for advanced operations
    pub(crate) fn underlying_connection(&self) -> Option<&rusqlite::Connection> {
        self.pool.direct_connection()
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
        self.query_cache.invalidate_all();
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
