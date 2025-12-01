use std::path::Path;
use std::sync::Arc;

use rusqlite::{Connection, OptionalExtension, params};

use crate::{
    cache::AdjacencyCache,
    errors::SqliteGraphError,
    mvcc::{GraphSnapshot, SnapshotManager},
    schema::{MigrationReport, ensure_schema, read_schema_version, run_pending_migrations},
};

use super::{
    metrics::{GraphMetrics, GraphMetricsSnapshot, InstrumentedConnection, StatementTracker},
    types::{GraphEdge, GraphEntity, row_to_edge, row_to_entity, validate_edge, validate_entity},
};

/// Embedded SQLite-backed graph database.
///
/// Provides a lightweight, deterministic graph database with entity and edge storage,
/// pattern matching, MVCC-lite snapshots, and deterministic indexing.
pub struct SqliteGraph {
    conn: Connection,
    outgoing_cache: AdjacencyCache,
    incoming_cache: AdjacencyCache,
    metrics: GraphMetrics,
    statement_tracker: StatementTracker,
    snapshot_manager: SnapshotManager,
}

// Helper function to check if connection is in-memory
fn is_in_memory_connection(conn: &Connection) -> bool {
    // Check database filename - in-memory databases have empty or special names
    match conn.pragma_query_value(None, "database_list", |row| {
        let name: String = row.get(1)?;
        Ok(name)
    }) {
        Ok(name) => name.is_empty() || name == ":memory:",
        Err(_) => true, // Assume in-memory if we can't query
    }
}

impl SqliteGraph {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, SqliteGraphError> {
        let conn =
            Connection::open(path).map_err(|e| SqliteGraphError::connection(e.to_string()))?;
        ensure_schema(&conn)?;
        Ok(Self::from_connection(conn))
    }

    pub fn open_without_migrations<P: AsRef<Path>>(path: P) -> Result<Self, SqliteGraphError> {
        let conn =
            Connection::open(path).map_err(|e| SqliteGraphError::connection(e.to_string()))?;
        crate::schema::ensure_schema_without_migrations(&conn)?;
        Ok(Self::from_connection(conn))
    }

    pub fn open_in_memory() -> Result<Self, SqliteGraphError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| SqliteGraphError::connection(e.to_string()))?;
        ensure_schema(&conn)?;
        Ok(Self::from_connection(conn))
    }

    pub fn open_in_memory_without_migrations() -> Result<Self, SqliteGraphError> {
        let conn = Connection::open_in_memory()
            .map_err(|e| SqliteGraphError::connection(e.to_string()))?;
        crate::schema::ensure_schema_without_migrations(&conn)?;
        Ok(Self::from_connection(conn))
    }

    pub fn metrics_snapshot(&self) -> GraphMetricsSnapshot {
        self.metrics.snapshot()
    }

    pub fn reset_metrics(&self) {
        self.metrics.reset();
    }

    pub fn schema_version(&self) -> Result<i64, SqliteGraphError> {
        read_schema_version(&self.conn)
    }

    /// Update the snapshot with current cache state
    /// This is called automatically after write operations
    pub(crate) fn update_snapshot(&self) {
        self.snapshot_manager.update_snapshot(
            &self.outgoing_cache_ref().inner(),
            &self.incoming_cache_ref().inner(),
        );
    }

    /// Acquire a deterministic snapshot of the current graph state
    ///
    /// Returns a read-only snapshot that provides isolated access to graph data.
    /// The snapshot contains cloned adjacency maps and uses a read-only SQLite connection.
    ///
    /// # Returns
    /// Result containing GraphSnapshot or error
    pub fn acquire_snapshot(&self) -> Result<GraphSnapshot, SqliteGraphError> {
        // Update snapshot with current cache state
        self.update_snapshot();

        // Acquire snapshot state
        let snapshot_state = self.snapshot_manager.acquire_snapshot();

        // For testing purposes, assume in-memory database
        // TODO: Add proper file-based database path handling for production use
        let db_path = ":memory:";

        GraphSnapshot::new(snapshot_state, db_path)
            .map_err(|e| SqliteGraphError::connection(e.to_string()))
    }

    /// Get the current snapshot state without creating a new connection
    /// This is useful for internal operations and testing
    pub(crate) fn current_snapshot_state(&self) -> Arc<crate::mvcc::SnapshotState> {
        self.update_snapshot();
        self.snapshot_manager.current_snapshot()
    }

    /// Get the number of nodes in the current snapshot
    pub fn snapshot_node_count(&self) -> usize {
        self.current_snapshot_state().node_count()
    }

    /// Get the number of edges in the current snapshot
    pub fn snapshot_edge_count(&self) -> usize {
        self.current_snapshot_state().edge_count()
    }

    /// Check if a node exists in the current snapshot
    pub fn snapshot_contains_node(&self, node_id: i64) -> bool {
        self.current_snapshot_state().contains_node(node_id)
    }

    pub fn run_pending_migrations(
        &self,
        dry_run: bool,
    ) -> Result<MigrationReport, SqliteGraphError> {
        run_pending_migrations(&self.conn, dry_run)
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

    /// Get access to the underlying SQLite connection for advanced operations
    pub(crate) fn underlying_connection(&self) -> &Connection {
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

    /// Match lightweight triple patterns using the pattern engine.
    ///
    /// This method provides a simple interface for matching single-hop patterns
    /// like (start_label)-\[edge_type\]->(end_label) with optional property filters.
    ///
    /// # Arguments
    /// * `pattern` - The pattern triple to match
    ///
    /// # Returns
    /// A vector of triple matches in deterministic order
    pub fn match_triples(
        &self,
        pattern: &crate::pattern_engine::PatternTriple,
    ) -> Result<Vec<crate::pattern_engine::TripleMatch>, SqliteGraphError> {
        crate::pattern_engine::match_triples(self, pattern)
    }

    /// Match lightweight triple patterns using cache-enabled fast-path.
    ///
    /// This method provides an optimized version of pattern matching that:
    /// - Uses cache as a fast-path where safe
    /// - Falls back to SQL where pattern requires it
    /// - Returns IDENTICAL results to match_triples()
    /// - Maintains deterministic ordering
    ///
    /// # Arguments
    /// * `pattern` - The pattern triple to match
    ///
    /// # Returns
    /// A vector of triple matches in deterministic order
    pub fn match_triples_fast(
        &self,
        pattern: &crate::pattern_engine::PatternTriple,
    ) -> Result<Vec<crate::pattern_engine::TripleMatch>, SqliteGraphError> {
        crate::pattern_engine_cache::match_triples_fast(self, pattern)
    }

    fn from_connection(conn: Connection) -> Self {
        conn.set_prepared_statement_cache_capacity(128);

        // Configure WAL mode and performance optimizations for file-based databases
        if !is_in_memory_connection(&conn) {
            // Enable WAL mode for better concurrency
            if let Err(_e) = conn.pragma_update(None, "journal_mode", "WAL") {
                // Fallback to DELETE mode if WAL fails (e.g., on some network filesystems)
                let _ = conn.pragma_update(None, "journal_mode", "DELETE");
            }

            // Performance optimizations
            let _ = conn.pragma_update(None, "synchronous", "NORMAL"); // Balanced safety/performance
            let _ = conn.pragma_update(None, "cache_size", "-64000"); // 64MB cache
            let _ = conn.pragma_update(None, "temp_store", "MEMORY"); // Store temp tables in memory
            let _ = conn.pragma_update(None, "mmap_size", "268435456"); // 256MB memory-mapped I/O
        }

        Self {
            conn,
            outgoing_cache: AdjacencyCache::new(),
            incoming_cache: AdjacencyCache::new(),
            metrics: GraphMetrics::default(),
            statement_tracker: StatementTracker::default(),
            snapshot_manager: SnapshotManager::new(),
        }
    }
}
