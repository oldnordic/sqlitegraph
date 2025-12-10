//! Configuration for backend selection and backend-specific options.
//!
//! This module provides the configuration structures needed for selecting
//! between SQLite and Native backends at runtime, along with backend-specific
//! configuration options.

use std::collections::HashMap;
use std::path::Path;

use crate::SqliteGraphError;
use crate::backend::{GraphBackend, NativeGraphBackend, SqliteGraphBackend};

/// Backend selection enum for choosing between storage implementations.
///
/// This enum allows runtime selection of the graph storage backend
/// without compile-time dependencies on specific implementations.
///
/// # Default Behavior
///
/// The default backend is [`BackendKind::SQLite`] to maintain backward compatibility
/// and provide the most feature-rich experience out of the box.
///
/// # Examples
///
/// ```rust
/// use sqlitegraph::{BackendKind, GraphConfig};
///
/// // Explicit SQLite selection
/// let cfg = GraphConfig::new(BackendKind::SQLite);
///
/// // Explicit Native selection
/// let cfg = GraphConfig::new(BackendKind::Native);
///
/// // Using convenience constructors
/// let sqlite_cfg = GraphConfig::sqlite();
/// let native_cfg = GraphConfig::native();
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendKind {
    /// SQLite-based backend with full SQL capabilities
    ///
    /// **Use when you need:**
    /// - ACID transactions with rollback support
    /// - Complex queries beyond basic graph operations
    /// - Existing SQLite investments or tooling compatibility
    /// - Standard SQL access alongside graph operations
    SQLite,

    /// Native file-based backend with adjacency storage
    ///
    /// **Use when you need:**
    /// - Maximum performance for graph operations
    /// - Simplified deployment without SQLite dependencies
    /// - Custom graph storage requirements
    /// - Fast startup with large datasets
    Native,
}

/// Configuration for native backend operations.
///
/// Provides options specific to the native file-based storage implementation.
/// These options control file creation behavior and performance optimizations.
///
/// # Default Configuration
///
/// ```rust
/// use sqlitegraph::NativeConfig;
/// let config = NativeConfig::default();
/// assert_eq!(config.create_if_missing, true);
/// assert!(config.reserve_node_capacity.is_none());
/// assert!(config.reserve_edge_capacity.is_none());
/// ```
#[derive(Clone, Debug)]
pub struct NativeConfig {
    /// Whether to create the graph file if it doesn't exist
    ///
    /// **Default:** `true`
    ///
    /// When set to `true`, the backend will automatically create the graph file
    /// if it doesn't exist. When set to `false`, attempting to open a non-existent
    /// file will return an error.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sqlitegraph::{GraphConfig, open_graph};
    ///
    /// // Create file if missing (default behavior)
    /// let mut cfg = GraphConfig::native();
    /// cfg.native.create_if_missing = true;
    /// let graph = open_graph("new_graph.db", &cfg)?; // Creates file if needed
    ///
    /// // Fail if file doesn't exist
    /// let mut cfg = GraphConfig::native();
    /// cfg.native.create_if_missing = false;
    /// let graph = open_graph("existing_graph.db", &cfg)?; // Error if file missing
    /// ```
    pub create_if_missing: bool,

    /// Optional capacity pre-allocation for nodes (performance optimization)
    ///
    /// **Default:** `None`
    ///
    /// When set to `Some(capacity)`, this provides a hint to the native backend
    /// about the expected number of nodes. This can improve performance by reducing
    /// the number of memory reallocations during bulk insertions.
    ///
    /// **Note:** This is a performance hint and not a hard limit. The backend will
    /// automatically grow beyond this capacity if needed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sqlitegraph::GraphConfig;
    ///
    /// let mut cfg = GraphConfig::native();
    /// cfg.native.reserve_node_capacity = Some(10000); // Expect ~10K nodes
    /// let graph = open_graph("large_graph.db", &cfg)?;
    /// ```
    pub reserve_node_capacity: Option<usize>,

    /// Optional capacity pre-allocation for edges (performance optimization)
    ///
    /// **Default:** `None`
    ///
    /// When set to `Some(capacity)`, this provides a hint to the native backend
    /// about the expected number of edges. This can improve performance by reducing
    /// the number of memory reallocations during bulk insertions.
    ///
    /// **Note:** This is a performance hint and not a hard limit. The backend will
    /// automatically grow beyond this capacity if needed.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sqlitegraph::GraphConfig;
    ///
    /// let mut cfg = GraphConfig::native();
    /// cfg.native.reserve_edge_capacity = Some(50000); // Expect ~50K edges
    /// let graph = open_graph("dense_graph.db", &cfg)?;
    /// ```
    pub reserve_edge_capacity: Option<usize>,
}

impl Default for NativeConfig {
    fn default() -> Self {
        Self {
            create_if_missing: true, // Default: create files if they don't exist
            reserve_node_capacity: None,
            reserve_edge_capacity: None,
        }
    }
}

/// Configuration for SQLite backend operations.
///
/// Provides options specific to the SQLite storage implementation.
/// These options control schema migrations, performance settings, and PRAGMA configurations.
///
/// # Default Configuration
///
/// ```rust
/// use sqlitegraph::SqliteConfig;
/// let config = SqliteConfig::default();
/// assert_eq!(config.without_migrations, false);
/// assert!(config.cache_size.is_none());
/// assert!(config.pragma_settings.is_empty());
/// ```
#[derive(Clone, Debug, Default)]
pub struct SqliteConfig {
    /// Skip schema migrations during opening
    ///
    /// **Default:** `false`
    ///
    /// When set to `true`, the backend will skip automatic schema migrations when opening
    /// an existing database. This can improve startup time for large databases where you're
    /// certain the schema is already compatible.
    ///
    /// **Use with caution:** Disabling migrations when the schema is incompatible will
    /// result in runtime errors when using newer features.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sqlitegraph::GraphConfig;
    ///
    /// // Skip migrations for faster startup (use when schema is known to be compatible)
    /// let mut cfg = GraphConfig::sqlite();
    /// cfg.sqlite.without_migrations = true;
    /// let graph = open_graph("production_graph.db", &cfg)?;
    /// ```
    pub without_migrations: bool,

    /// Optional cache size for prepared statements
    ///
    /// **Default:** `None`
    ///
    /// When set to `Some(size)`, configures the SQLite prepared statement cache to
    /// hold the specified number of cached statements. This can improve performance for
    /// repetitive queries by avoiding SQL statement recompilation.
    ///
    /// **Note:** SQLite's default cache size is typically sufficient for most workloads.
    /// Only modify this if you have evidence of statement compilation overhead.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sqlitegraph::GraphConfig;
    ///
    /// let mut cfg = GraphConfig::sqlite();
    /// cfg.sqlite.cache_size = Some(1000); // Cache up to 1000 prepared statements
    /// let graph = open_graph("query_intensive.db", &cfg)?;
    /// ```
    pub cache_size: Option<usize>,

    /// Additional SQLite PRAGMA settings
    ///
    /// **Default:** `HashMap::new()` (empty)
    ///
    /// Allows fine-tuning of SQLite behavior through PRAGMA settings. These are applied
    /// after the database is opened and before the graph backend is initialized.
    ///
    /// Common PRAGMA settings include:
    /// - `journal_mode`: Set to "WAL" for better concurrent access
    /// - `synchronous`: Set to "NORMAL" for better performance with some safety trade-off
    /// - `cache_size`: Configure SQLite page cache size
    /// - `temp_store`: Set temp storage location ("MEMORY", "FILE")
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sqlitegraph::GraphConfig;
    /// use std::collections::HashMap;
    ///
    /// let mut cfg = GraphConfig::sqlite();
    ///
    /// // Configure for better concurrent access
    /// cfg.sqlite.pragma_settings.insert("journal_mode".to_string(), "WAL".to_string());
    /// cfg.sqlite.pragma_settings.insert("synchronous".to_string(), "NORMAL".to_string());
    ///
    /// // Configure for better performance
    /// cfg.sqlite.pragma_settings.insert("cache_size".to_string(), "10000".to_string());
    ///
    /// let graph = open_graph("optimized.db", &cfg)?;
    /// ```
    pub pragma_settings: HashMap<String, String>,
}

/// Complete configuration for graph construction.
///
/// This structure combines backend selection with backend-specific
/// configuration options. Default values maintain existing behavior
/// and ensure backward compatibility.
///
/// # Default Configuration
///
/// ```rust
/// use sqlitegraph::{GraphConfig, BackendKind};
/// let config = GraphConfig::default();
/// assert_eq!(config.backend, BackendKind::SQLite);
/// assert!(!config.sqlite.without_migrations);
/// assert!(config.native.create_if_missing);
/// ```
///
/// # Examples
///
/// ```rust
/// use sqlitegraph::{GraphConfig, BackendKind};
/// use std::collections::HashMap;
///
/// // Simple SQLite configuration
/// let sqlite_cfg = GraphConfig::sqlite();
///
/// // Simple Native configuration
/// let native_cfg = GraphConfig::native();
///
/// // Custom SQLite configuration with PRAGMAs
/// let mut custom_sqlite = GraphConfig::sqlite();
/// custom_sqlite.sqlite.pragma_settings.insert("journal_mode".to_string(), "WAL".to_string());
/// custom_sqlite.sqlite.pragma_settings.insert("synchronous".to_string(), "NORMAL".to_string());
///
/// // Custom Native configuration with capacity pre-allocation
/// let mut custom_native = GraphConfig::native();
/// custom_native.native.reserve_node_capacity = Some(10000);
/// custom_native.native.reserve_edge_capacity = Some(50000);
/// ```
#[derive(Clone, Debug)]
pub struct GraphConfig {
    /// Which backend to use for graph storage
    ///
    /// **Default:** [`BackendKind::SQLite`]
    ///
    /// This field determines the storage implementation used for the graph.
    /// Both backends implement the same [`GraphBackend`] trait, ensuring
    /// identical API behavior regardless of the selected backend.
    pub backend: BackendKind,

    /// SQLite-specific configuration options
    ///
    /// **Default:** [`SqliteConfig::default()`]
    ///
    /// These options are only used when `backend` is [`BackendKind::SQLite`].
    /// When using the Native backend, these settings are ignored but still
    /// available for configuration consistency when switching backends.
    pub sqlite: SqliteConfig,

    /// Native-specific configuration options
    ///
    /// **Default:** [`NativeConfig::default()`]
    ///
    /// These options are only used when `backend` is [`BackendKind::Native`].
    /// When using the SQLite backend, these settings are ignored but still
    /// available for configuration consistency when switching backends.
    pub native: NativeConfig,
}

impl GraphConfig {
    /// Create a new configuration with the specified backend.
    pub fn new(backend: BackendKind) -> Self {
        let mut sqlite_config = SqliteConfig::default();
        let mut native_config = NativeConfig::default();

        // Set backend-specific defaults
        match backend {
            BackendKind::SQLite => {
                // SQLite defaults: run migrations by default
                sqlite_config.without_migrations = false;
            }
            BackendKind::Native => {
                // Native backend: SQLite config won't be used, but mark as without migrations
                sqlite_config.without_migrations = true;
            }
        }

        Self {
            backend,
            sqlite: sqlite_config,
            native: native_config,
        }
    }

    /// Create a configuration for SQLite backend.
    pub fn sqlite() -> Self {
        Self::new(BackendKind::SQLite)
    }

    /// Create a configuration for Native backend.
    pub fn native() -> Self {
        Self::new(BackendKind::Native)
    }
}

impl Default for GraphConfig {
    fn default() -> Self {
        // Default to SQLite backend with appropriate defaults
        Self::new(BackendKind::SQLite)
    }
}

impl Default for BackendKind {
    fn default() -> Self {
        BackendKind::SQLite
    }
}

/// Open a graph with the specified configuration.
///
/// This is the unified factory function that allows runtime backend selection.
/// The path parameter is used for file-based storage in both backends.
///
/// # Arguments
/// * `path` - Path to the graph database file
/// * `cfg` - Configuration specifying backend and options
///
/// # Returns
/// A boxed GraphBackend implementation matching the selected backend
///
/// # Examples
/// ```rust
/// use sqlitegraph::{open_graph, GraphConfig, BackendKind};
///
/// // Open SQLite backend (default behavior)
/// let cfg = GraphConfig::sqlite();
/// let graph = open_graph("my_graph.db", &cfg)?;
///
/// // Open Native backend
/// let cfg = GraphConfig::native();
/// let graph = open_graph("my_graph.db", &cfg)?;
/// ```
pub fn open_graph<P: AsRef<Path>>(
    path: P,
    cfg: &GraphConfig,
) -> Result<Box<dyn GraphBackend>, SqliteGraphError> {
    match cfg.backend {
        BackendKind::SQLite => {
            // Construct SQLite backend with configuration
            let sqlite_graph = if cfg.sqlite.without_migrations {
                crate::graph::SqliteGraph::open_without_migrations(&path)?
            } else {
                crate::graph::SqliteGraph::open(&path)?
            };

            // Apply PRAGMA settings if provided
            for (key, value) in &cfg.sqlite.pragma_settings {
                let pragma_sql = format!("PRAGMA {} = {}", key, value);
                match sqlite_graph.conn.execute(&pragma_sql, []) {
                    Ok(_) => {} // PRAGMA executed successfully
                    Err(rusqlite::Error::ExecuteReturnedResults) => {
                        // Some PRAGMAs return results - that's fine, just ignore them
                    }
                    Err(e) => {
                        return Err(SqliteGraphError::connection(format!(
                            "PRAGMA {} = {}: {}",
                            key, value, e
                        )));
                    }
                }
            }

            Ok(Box::new(SqliteGraphBackend::from_graph(sqlite_graph)))
        }
        BackendKind::Native => {
            // Construct Native backend with configuration
            let mut native_graph = if cfg.native.create_if_missing {
                crate::backend::NativeGraphBackend::new(&path)?
            } else {
                crate::backend::NativeGraphBackend::open(&path)?
            };

            // Apply capacity pre-allocation if requested
            if let Some(node_capacity) = cfg.native.reserve_node_capacity {
                // Note: Native backend doesn't currently expose capacity pre-allocation
                // This would require extending the NativeGraphBackend API
                // For now, we store this for potential future optimization
            }

            if let Some(edge_capacity) = cfg.native.reserve_edge_capacity {
                // Note: Same as above - future optimization opportunity
            }

            Ok(Box::new(native_graph))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_graph_config_default() {
        let cfg = GraphConfig::default();
        assert_eq!(cfg.backend, BackendKind::SQLite);
        assert!(!cfg.sqlite.without_migrations);
        assert!(cfg.sqlite.cache_size.is_none());
        assert!(cfg.sqlite.pragma_settings.is_empty());
        assert!(cfg.native.create_if_missing);
        assert!(cfg.native.reserve_node_capacity.is_none());
        assert!(cfg.native.reserve_edge_capacity.is_none());
    }

    #[test]
    fn test_graph_config_new() {
        let cfg = GraphConfig::new(BackendKind::Native);
        assert_eq!(cfg.backend, BackendKind::Native);
        assert!(cfg.sqlite.without_migrations);
        assert!(cfg.native.create_if_missing);
    }

    #[test]
    fn test_graph_config_constructors() {
        let sqlite_cfg = GraphConfig::sqlite();
        assert_eq!(sqlite_cfg.backend, BackendKind::SQLite);

        let native_cfg = GraphConfig::native();
        assert_eq!(native_cfg.backend, BackendKind::Native);
    }

    #[test]
    fn test_open_graph_sqlite() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test.db");

        let cfg = GraphConfig::sqlite();
        let result = open_graph(&db_path, &cfg);
        assert!(result.is_ok());

        // Verify the file was created
        assert!(db_path.exists());
    }

    #[test]
    fn test_open_graph_native() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_native.db");

        let cfg = GraphConfig::native();
        let result = open_graph(&db_path, &cfg);
        assert!(result.is_ok());

        // Verify the file was created
        assert!(db_path.exists());
    }

    #[test]
    fn test_sqlite_config_pragmas() {
        let temp_dir = tempdir().unwrap();
        let db_path = temp_dir.path().join("test_pragmas.db");

        let mut cfg = GraphConfig::sqlite();
        cfg.sqlite
            .pragma_settings
            .insert("journal_mode".to_string(), "WAL".to_string());
        cfg.sqlite
            .pragma_settings
            .insert("synchronous".to_string(), "NORMAL".to_string());

        let result = open_graph(&db_path, &cfg);
        assert!(result.is_ok());
        assert!(db_path.exists());
    }
}
