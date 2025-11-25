use std::path::Path;

use rusqlite::{Connection, OptionalExtension, params};

use crate::{cache::AdjacencyCache, errors::SqliteGraphError, schema::ensure_schema};

use super::{
    metrics::{GraphMetrics, GraphMetricsSnapshot, InstrumentedConnection, StatementTracker},
    types::{GraphEdge, GraphEntity, row_to_edge, row_to_entity, validate_edge, validate_entity},
};

pub struct SqliteGraph {
    conn: Connection,
    outgoing_cache: AdjacencyCache,
    incoming_cache: AdjacencyCache,
    metrics: GraphMetrics,
    statement_tracker: StatementTracker,
}

impl SqliteGraph {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, SqliteGraphError> {
        let conn =
            Connection::open(path).map_err(|e| SqliteGraphError::connection(e.to_string()))?;
        ensure_schema(&conn)?;
        Ok(Self::from_connection(conn))
    }

    pub fn open_in_memory() -> Result<Self, SqliteGraphError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| SqliteGraphError::connection(e.to_string()))?;
        ensure_schema(&conn)?;
        Ok(Self::from_connection(conn))
    }

    pub fn metrics_snapshot(&self) -> GraphMetricsSnapshot {
        self.metrics.snapshot()
    }

    pub fn reset_metrics(&self) {
        self.metrics.reset();
    }

    pub fn insert_entity(&self, entity: &GraphEntity) -> Result<i64, SqliteGraphError> {
        validate_entity(entity)?;
        let data = serde_json::to_string(&entity.data)
            .map_err(|e| SqliteGraphError::invalid_input(e.to_string()))?;
        self.connection()
            .execute(
                "INSERT INTO graph_entities(kind, name, file_path, data) VALUES(?1, ?2, ?3, ?4)",
                params![
                    entity.kind.as_str(),
                    entity.name.as_str(),
                    entity.file_path.as_deref(),
                    data,
                ],
            )
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_entity(&self, id: i64) -> Result<GraphEntity, SqliteGraphError> {
        self.connection()
            .query_row(
                "SELECT id, kind, name, file_path, data FROM graph_entities WHERE id=?1",
                params![id],
                row_to_entity,
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => {
                    SqliteGraphError::not_found(format!("entity {id}"))
                }
                other => SqliteGraphError::query(other.to_string()),
            })
    }

    pub fn update_entity(&self, entity: &GraphEntity) -> Result<(), SqliteGraphError> {
        if entity.id <= 0 {
            return Err(SqliteGraphError::invalid_input(
                "entity id must be positive for update",
            ));
        }
        validate_entity(entity)?;
        let data = serde_json::to_string(&entity.data)
            .map_err(|e| SqliteGraphError::invalid_input(e.to_string()))?;
        let affected = self
            .connection()
            .execute(
                "UPDATE graph_entities SET kind=?1, name=?2, file_path=?3, data=?4 WHERE id=?5",
                params![
                    entity.kind.as_str(),
                    entity.name.as_str(),
                    entity.file_path.as_deref(),
                    data,
                    entity.id,
                ],
            )
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        if affected == 0 {
            return Err(SqliteGraphError::not_found(format!("entity {}", entity.id)));
        }
        Ok(())
    }

    pub fn delete_entity(&self, id: i64) -> Result<(), SqliteGraphError> {
        let affected = self
            .connection()
            .execute("DELETE FROM graph_entities WHERE id=?1", params![id])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        if affected == 0 {
            return Err(SqliteGraphError::not_found(format!("entity {id}")));
        }
        self.connection()
            .execute(
                "DELETE FROM graph_edges WHERE from_id=?1 OR to_id=?1",
                params![id],
            )
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        self.invalidate_caches();
        Ok(())
    }

    pub fn insert_edge(&self, edge: &GraphEdge) -> Result<i64, SqliteGraphError> {
        validate_edge(edge)?;
        if !self.entity_exists(edge.from_id)? || !self.entity_exists(edge.to_id)? {
            return Err(SqliteGraphError::invalid_input(
                "edge endpoints must reference existing entities",
            ));
        }
        let data = serde_json::to_string(&edge.data)
            .map_err(|e| SqliteGraphError::invalid_input(e.to_string()))?;
        self.connection()
            .execute(
                "INSERT INTO graph_edges(from_id, to_id, edge_type, data) VALUES(?1, ?2, ?3, ?4)",
                params![edge.from_id, edge.to_id, edge.edge_type.as_str(), data],
            )
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        self.invalidate_caches();
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_edge(&self, id: i64) -> Result<GraphEdge, SqliteGraphError> {
        self.connection()
            .query_row(
                "SELECT id, from_id, to_id, edge_type, data FROM graph_edges WHERE id=?1",
                params![id],
                row_to_edge,
            )
            .map_err(|err| match err {
                rusqlite::Error::QueryReturnedNoRows => {
                    SqliteGraphError::not_found(format!("edge {id}"))
                }
                other => SqliteGraphError::query(other.to_string()),
            })
    }

    pub fn delete_edge(&self, id: i64) -> Result<(), SqliteGraphError> {
        let affected = self
            .connection()
            .execute("DELETE FROM graph_edges WHERE id=?1", params![id])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        if affected == 0 {
            return Err(SqliteGraphError::not_found(format!("edge {id}")));
        }
        self.invalidate_caches();
        Ok(())
    }

    pub fn list_entity_ids(&self) -> Result<Vec<i64>, SqliteGraphError> {
        self.all_entity_ids()
    }

    pub(crate) fn connection(&self) -> InstrumentedConnection<'_> {
        InstrumentedConnection::new(&self.conn, &self.metrics, &self.statement_tracker)
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

    pub(crate) fn outgoing_cache_ref(&self) -> &AdjacencyCache {
        &self.outgoing_cache
    }

    pub(crate) fn incoming_cache_ref(&self) -> &AdjacencyCache {
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

    fn entity_exists(&self, id: i64) -> Result<bool, SqliteGraphError> {
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

    fn from_connection(conn: Connection) -> Self {
        conn.set_prepared_statement_cache_capacity(128);
        Self {
            conn,
            outgoing_cache: AdjacencyCache::new(),
            incoming_cache: AdjacencyCache::new(),
            metrics: GraphMetrics::default(),
            statement_tracker: StatementTracker::default(),
        }
    }
}
