//! Core SqliteGraph struct and construction functionality.

use std::path::Path;

use rusqlite::Connection;

use crate::{
    cache::AdjacencyCache, errors::SqliteGraphError, mvcc::SnapshotManager, schema::ensure_schema,
};

use super::metrics::{GraphMetrics, StatementTracker};

/// Embedded SQLite-backed graph database.
///
/// Provides a lightweight, deterministic graph database with entity and edge storage,
/// pattern matching, MVCC-lite snapshots, and deterministic indexing.
pub struct SqliteGraph {
    pub(crate) conn: Connection,
    pub(crate) outgoing_cache: AdjacencyCache,
    pub(crate) incoming_cache: AdjacencyCache,
    pub(crate) metrics: GraphMetrics,
    pub(crate) statement_tracker: StatementTracker,
    pub(crate) snapshot_manager: SnapshotManager,
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
