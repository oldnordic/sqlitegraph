//! Core SqliteGraph struct and construction functionality.

use std::path::Path;
use std::collections::HashMap;
use std::sync::RwLock;

use rusqlite::Connection;

use crate::{
    cache::AdjacencyCache, errors::SqliteGraphError, mvcc::SnapshotManager,
    query_cache::QueryCache, schema::ensure_schema,
    hnsw::HnswIndex,
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
    pub(crate) query_cache: QueryCache,
    pub(crate) metrics: GraphMetrics,
    pub(crate) statement_tracker: StatementTracker,
    pub(crate) snapshot_manager: SnapshotManager,
    /// HNSW vector indexes stored by name
    pub(crate) hnsw_indexes: RwLock<HashMap<String, HnswIndex>>,
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

        // Load existing HNSW indexes from database
        let hnsw_indexes = Self::load_hnsw_indexes(&conn).unwrap_or_default();

        Self {
            conn,
            outgoing_cache: AdjacencyCache::new(),
            incoming_cache: AdjacencyCache::new(),
            query_cache: QueryCache::new(),
            metrics: GraphMetrics::default(),
            statement_tracker: StatementTracker::default(),
            snapshot_manager: SnapshotManager::new(),
            hnsw_indexes: RwLock::new(hnsw_indexes),
        }
    }

    /// Load HNSW indexes from database
    ///
    /// This is called during SqliteGraph construction to restore any
    /// previously created HNSW indexes with full vector data.
    fn load_hnsw_indexes(conn: &Connection) -> Result<HashMap<String, HnswIndex>, SqliteGraphError> {
        let mut indexes = HashMap::new();

        // Get list of existing indexes
        let index_names = HnswIndex::list_indexes(conn)
            .map_err(|e| SqliteGraphError::invalid_input(format!("Failed to load HNSW indexes: {}", e)))?;

        // Load each index with vectors
        for name in index_names {
            match HnswIndex::load_with_vectors(conn, &name) {
                Ok(hnsw) => {
                    indexes.insert(name, hnsw);
                }
                Err(e) => {
                    // Log warning but continue loading other indexes
                    eprintln!("Warning: Failed to load HNSW index '{}': {}", name, e);
                }
            }
        }

        Ok(indexes)
    }
}
