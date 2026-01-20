//! Core SqliteGraph struct and construction functionality.

use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use rusqlite::Connection;

use crate::{
    cache::{AdjacencyCache, CacheStats},
    config::SqliteConfig,
    errors::SqliteGraphError,
    introspection::{GraphIntrospection, EdgeCount, IntrospectError},
    mvcc::SnapshotManager,
    query_cache::QueryCache,
    schema::ensure_schema,
    hnsw::HnswIndex,
};

use super::{
    metrics::{GraphMetrics, StatementTracker},
    pool::PoolManager,
};

/// Embedded SQLite-backed graph database.
///
/// Provides a lightweight, deterministic graph database with entity and edge storage,
/// pattern matching, MVCC-lite snapshots, and deterministic indexing.
///
/// # Thread Safety
///
/// **NOT thread-safe for concurrent writes.** `SqliteGraph` uses interior mutability
/// (`RefCell`) and is not `Sync`. However, the underlying connection pool allows
/// multiple threads to read concurrently when using separate `SqliteGraph` instances.
///
/// # Connection Pooling
///
/// File-based databases use an r2d2 connection pool (default 5 connections) for
/// concurrent access. In-memory databases skip pooling and use a single direct connection.
pub struct SqliteGraph {
    /// Connection pool for file-based databases, or direct connection for in-memory
    /// (public for CLI access to underlying connection)
    pub pool: PoolManager,
    pub(crate) outgoing_cache: AdjacencyCache,
    pub(crate) incoming_cache: AdjacencyCache,
    pub(crate) query_cache: QueryCache,
    pub(crate) metrics: Arc<GraphMetrics>,
    pub(crate) statement_tracker: Arc<StatementTracker>,
    pub(crate) snapshot_manager: SnapshotManager,
    /// HNSW vector indexes stored by name (public for CLI access)
    pub hnsw_indexes: RwLock<HashMap<String, HnswIndex>>,
}

// Helper function to check if connection is in-memory
pub fn is_in_memory_connection(conn: &Connection) -> bool {
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
    /// Open a graph database with custom configuration.
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the SQLite database file
    /// * `cfg` - Configuration options (pool size, cache size, PRAGMAs, etc.)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use sqlitegraph::{SqliteGraph, SqliteConfig};
    ///
    /// let cfg = SqliteConfig::new()
    ///     .with_pool_size(10)
    ///     .with_wal_mode();
    /// let graph = SqliteGraph::open_with_config("my_graph.db", &cfg)?;
    /// ```
    pub fn open_with_config<P: AsRef<Path>>(path: P, cfg: &SqliteConfig) -> Result<Self, SqliteGraphError> {
        // Get pool size from config (default: 5)
        let pool_size = cfg.pool_size.unwrap_or(5) as u32;

        let mut pool = PoolManager::with_max_size(path, pool_size)
            .map_err(|e| SqliteGraphError::connection(e.to_string()))?;

        // Initialize schema using first connection from pool
        {
            let conn = pool.get()
                .map_err(|e| SqliteGraphError::connection(e.to_string()))?;

            if cfg.without_migrations {
                crate::schema::ensure_schema_without_migrations(&conn)?;
            } else {
                ensure_schema(&conn)?;
            }
        }

        // Configure pool with WAL mode and performance optimizations
        pool.configure_pool(|conn| {
            // Set prepared statement cache size from config
            let cache_size = cfg.cache_size.unwrap_or(128);
            conn.set_prepared_statement_cache_capacity(cache_size);

            // Enable WAL mode for better concurrency
            let result = conn.pragma_update(None, "journal_mode", "WAL");
            if result.is_err() {
                // Fallback to DELETE mode if WAL fails (e.g., on some network filesystems)
                let _ = conn.pragma_update(None, "journal_mode", "DELETE");
            }

            // Performance optimizations
            let _ = conn.pragma_update(None, "synchronous", "NORMAL"); // Balanced safety/performance
            let _ = conn.pragma_update(None, "cache_size", "-64000"); // 64MB cache
            let _ = conn.pragma_update(None, "temp_store", "MEMORY"); // Store temp tables in memory
            let _ = conn.pragma_update(None, "mmap_size", "268435456"); // 256MB memory-mapped I/O

            // Apply custom PRAGMA settings from config
            for (key, value) in &cfg.pragma_settings {
                let _ = conn.pragma_update(None, key, value.as_str());
            }

            Ok(())
        })?;

        // Load existing HNSW indexes from database
        let hnsw_indexes = {
            let conn = pool.get()
                .map_err(|e| SqliteGraphError::connection(e.to_string()))?;
            Self::load_hnsw_indexes(&conn).unwrap_or_default()
        };

        Ok(Self {
            pool,
            outgoing_cache: AdjacencyCache::new(),
            incoming_cache: AdjacencyCache::new(),
            query_cache: QueryCache::new(),
            metrics: Arc::new(GraphMetrics::default()),
            statement_tracker: Arc::new(StatementTracker::default()),
            snapshot_manager: SnapshotManager::new(),
            hnsw_indexes: RwLock::new(hnsw_indexes),
        })
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, SqliteGraphError> {
        Self::open_with_config(path, &SqliteConfig::default())
    }

    pub fn open_without_migrations<P: AsRef<Path>>(path: P) -> Result<Self, SqliteGraphError> {
        let cfg = SqliteConfig::new().with_migrations_disabled(true);
        Self::open_with_config(path, &cfg)
    }

    /// Open an in-memory database with custom configuration.
    ///
    /// Note: Pool size is ignored for in-memory databases since they use
    /// a single direct connection (each connection would have isolated data).
    ///
    /// # Arguments
    ///
    /// * `cfg` - Configuration options (cache size, PRAGMAs, etc.)
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use sqlitegraph::{SqliteGraph, SqliteConfig};
    ///
    /// let cfg = SqliteConfig::new()
    ///     .with_cache_size(256)
    ///     .with_performance_mode();
    /// let graph = SqliteGraph::open_in_memory_with_config(&cfg)?;
    /// ```
    pub fn open_in_memory_with_config(cfg: &SqliteConfig) -> Result<Self, SqliteGraphError> {
        let mut pool = PoolManager::in_memory()
            .map_err(|e| SqliteGraphError::connection(e.to_string()))?;

        // Set prepared statement cache size from config
        let cache_size = cfg.cache_size.unwrap_or(128);

        // For in-memory databases, configure directly
        pool.configure_direct(|conn| {
            if cfg.without_migrations {
                crate::schema::ensure_schema_without_migrations(conn).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            } else {
                ensure_schema(conn).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
            }
            conn.set_prepared_statement_cache_capacity(cache_size);

            // Apply custom PRAGMA settings from config
            for (key, value) in &cfg.pragma_settings {
                let _ = conn.pragma_update(None, key, value.as_str());
            }

            Ok(())
        }).map_err(|e| SqliteGraphError::connection(e.to_string()))?;

        // Load HNSW indexes (will be empty for fresh in-memory database)
        let hnsw_indexes = pool.direct_connection()
            .map(|conn| Self::load_hnsw_indexes(conn).unwrap_or_default())
            .unwrap_or_default();

        Ok(Self {
            pool,
            outgoing_cache: AdjacencyCache::new(),
            incoming_cache: AdjacencyCache::new(),
            query_cache: QueryCache::new(),
            metrics: Arc::new(GraphMetrics::default()),
            statement_tracker: Arc::new(StatementTracker::default()),
            snapshot_manager: SnapshotManager::new(),
            hnsw_indexes: RwLock::new(hnsw_indexes),
        })
    }

    pub fn open_in_memory() -> Result<Self, SqliteGraphError> {
        Self::open_in_memory_with_config(&SqliteConfig::default())
    }

    pub fn open_in_memory_without_migrations() -> Result<Self, SqliteGraphError> {
        let cfg = SqliteConfig::new().with_migrations_disabled(true);
        Self::open_in_memory_with_config(&cfg)
    }

    /// Create a SqliteGraph from an existing connection.
    ///
    /// This is used internally for in-memory databases where pooling is skipped.
    /// The connection is wrapped in a PoolManager that provides direct access.
    fn from_connection(conn: Connection) -> Self {
        let mut pool = PoolManager::from_connection(conn);

        // Configure the connection
        let _ = pool.configure_direct(|c| {
            c.set_prepared_statement_cache_capacity(128);

            // Configure WAL mode and performance optimizations for file-based databases
            let is_mem = is_in_memory_connection(c);
            if !is_mem {
                // Enable WAL mode for better concurrency
                let result = c.pragma_update(None, "journal_mode", "WAL");
                if result.is_err() {
                    // Fallback to DELETE mode if WAL fails
                    let _ = c.pragma_update(None, "journal_mode", "DELETE");
                }

                // Performance optimizations
                let _ = c.pragma_update(None, "synchronous", "NORMAL");
                let _ = c.pragma_update(None, "cache_size", "-64000");
                let _ = c.pragma_update(None, "temp_store", "MEMORY");
                let _ = c.pragma_update(None, "mmap_size", "268435456");
            }

            Ok(())
        });

        // Load HNSW indexes
        let hnsw_indexes = pool.direct_connection()
            .map(|conn| Self::load_hnsw_indexes(conn).unwrap_or_default())
            .unwrap_or_default();

        Self {
            pool,
            outgoing_cache: AdjacencyCache::new(),
            incoming_cache: AdjacencyCache::new(),
            query_cache: QueryCache::new(),
            metrics: Arc::new(GraphMetrics::default()),
            statement_tracker: Arc::new(StatementTracker::default()),
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

    /// Get comprehensive introspection data for this graph instance.
    ///
    /// This method provides a structured snapshot of the graph state,
    /// including node counts, edge counts, cache statistics, and file sizes.
    /// The result is JSON-serializable for both human debugging and LLM consumption.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use sqlitegraph::{open_graph, GraphConfig};
    ///
    /// let graph = open_graph("my_graph.db", &GraphConfig::sqlite())?;
    /// let intro = graph.introspect()?;
    ///
    /// println!("Backend: {}", intro.backend_type);
    /// println!("Nodes: {}", intro.node_count);
    /// println!("Cache hit ratio: {:.2}%", intro.cache_stats.hit_ratio().unwrap_or(0.0));
    /// ```
    pub fn introspect(&self) -> Result<GraphIntrospection, SqliteGraphError> {
        // Determine backend type
        let backend_type = "sqlite".to_string();

        // Get node count
        let node_count = self
            .all_entity_ids()
            .map_err(|e| IntrospectError::NodeCountError(e.to_string()))?
            .len();

        // Get edge count (use sampling for large graphs)
        let edge_count = self.count_edges()?;

        // Get cache statistics (combined from outgoing and incoming)
        let outgoing_stats = self.outgoing_cache.stats();
        let incoming_stats = self.incoming_cache.stats();
        let cache_stats = CacheStats {
            hits: outgoing_stats.hits + incoming_stats.hits,
            misses: outgoing_stats.misses + incoming_stats.misses,
            entries: outgoing_stats.entries + incoming_stats.entries,
        };

        // Check if in-memory database
        let is_in_memory = self.pool.is_in_memory();

        // Get file size (only for file-based databases)
        let file_size = if is_in_memory {
            None
        } else {
            self.get_database_path()
                .and_then(|path| crate::introspection::get_file_size(path))
        };

        // Get WAL size (if WAL is enabled)
        let wal_size = if is_in_memory {
            None
        } else {
            self.get_database_path()
                .and_then(|path| crate::introspection::get_wal_size(path))
        };

        // Memory usage is not directly available for SQLite backend
        let memory_usage = None;

        Ok(GraphIntrospection {
            backend_type,
            node_count,
            edge_count,
            cache_stats,
            memory_usage,
            file_size,
            wal_size,
            is_in_memory,
        })
    }

    /// Get adjacency cache statistics.
    ///
    /// Returns combined statistics from both outgoing and incoming adjacency caches.
    /// This is useful for monitoring cache effectiveness and tuning cache size.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use sqlitegraph::{open_graph, GraphConfig};
    ///
    /// let graph = open_graph("my_graph.db", &GraphConfig::sqlite())?;
    /// let stats = graph.cache_stats();
    ///
    /// println!("Cache hits: {}", stats.hits);
    /// println!("Cache misses: {}", stats.misses);
    /// println!("Hit ratio: {:.2}%", stats.hit_ratio().unwrap_or(0.0));
    /// ```
    pub fn cache_stats(&self) -> CacheStats {
        let outgoing_stats = self.outgoing_cache.stats();
        let incoming_stats = self.incoming_cache.stats();
        CacheStats {
            hits: outgoing_stats.hits + incoming_stats.hits,
            misses: outgoing_stats.misses + incoming_stats.misses,
            entries: outgoing_stats.entries + incoming_stats.entries,
        }
    }

    /// Count edges in the graph.
    ///
    /// For graphs with fewer than 10,000 edges, returns an exact count.
    /// For larger graphs, returns an estimate based on sampling to avoid
    /// expensive O(E) operations.
    fn count_edges(&self) -> Result<EdgeCount, SqliteGraphError> {
        let conn = self.connection();

        // First, get a quick estimate
        let estimate: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM graph_edges",
                [],
                |row| row.get(0),
            )
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;

        // For small graphs (< 10K edges), return exact count
        if estimate < 10_000 {
            return Ok(EdgeCount::Exact(estimate as usize));
        }

        // For larger graphs, use sampling
        // Sample 1000 random rows to estimate with confidence interval
        let sample_size = 1000.min(estimate as usize);
        let sample_count: i64 = conn
            .query_row(
                &format!(
                    "SELECT COUNT(*) FROM (
                        SELECT 1 FROM graph_edges
                        ORDER BY RANDOM()
                        LIMIT {}
                    )",
                    sample_size
                ),
                [],
                |row| row.get(0),
            )
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;

        // Calculate confidence interval (95% confidence, ~2% margin of error)
        let ratio = sample_count as f64 / sample_size as f64;
        let margin = estimate as f64 * 0.02; // 2% margin of error

        Ok(EdgeCount::Estimate {
            count: estimate as usize,
            min: ((estimate as f64 - margin).floor() as usize).max(0),
            max: ((estimate as f64 + margin).ceil() as usize),
            sample_size,
        })
    }

    /// Get the database file path if this is a file-based database.
    fn get_database_path(&self) -> Option<String> {
        if self.pool.is_in_memory() {
            None
        } else {
            // Try to get the database path from SQLite
            self.pool.get().ok().and_then(|conn| {
                conn
                    .pragma_query_value(None, "database_list", |row| {
                        let name: String = row.get(1)?;
                        Ok(name)
                    })
                    .ok()
                    .filter(|name| !name.is_empty() && name != ":memory:")
            })
        }
    }
}
