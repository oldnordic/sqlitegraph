//! Minimal BackendClient wrapper for v0.2.5 compatibility
//!
//! This provides a simplified BackendClient that wraps existing v0.2.5 backends
//! without the missing pipeline/subgraph/safety modules.

use sqlitegraph::{SqliteGraphBackend, NativeGraphBackend, GraphBackend, SqliteGraph};

/// Minimal client wrapper for v0.2.5
pub enum BackendClient {
    Sqlite(SqliteGraphBackend),
    Native(NativeGraphBackend),
    /// Dynamic backend for factory-created backends (e.g., from open_graph())
    Dynamic(Box<dyn GraphBackend>),
}

impl BackendClient {
    pub fn new(backend: SqliteGraphBackend) -> Self {
        Self::Sqlite(backend)
    }

    pub fn new_native(backend: NativeGraphBackend) -> Self {
        Self::Native(backend)
    }

    /// Create from a boxed trait object (e.g., from open_graph factory)
    pub fn from_dynamic(backend: Box<dyn GraphBackend>) -> Self {
        Self::Dynamic(backend)
    }

    /// Get the GraphBackend trait object
    pub fn backend(&self) -> &dyn GraphBackend {
        match self {
            BackendClient::Sqlite(b) => b,
            BackendClient::Native(b) => b,
            BackendClient::Dynamic(b) => b.as_ref(),
        }
    }

    /// Get the underlying SqliteGraph if this is a SQLite backend
    pub fn graph(&self) -> Option<&SqliteGraph> {
        match self {
            BackendClient::Sqlite(b) => Some(b.graph()),
            BackendClient::Native(_) => None,
            BackendClient::Dynamic(_) => None,
        }
    }

    /// Get all entity IDs if this is a SQLite backend
    pub fn entity_ids(&self) -> Result<Option<Vec<i64>>, sqlitegraph::SqliteGraphError> {
        match self {
            BackendClient::Sqlite(b) => Ok(Some(b.entity_ids()?)),
            BackendClient::Native(_) => Ok(None),
            BackendClient::Dynamic(_) => Ok(None),
        }
    }

    /// Get backend type name for debugging
    pub fn backend_type(&self) -> &str {
        match self {
            BackendClient::Sqlite(_) => "sqlite",
            BackendClient::Native(_) => "native",
            BackendClient::Dynamic(_) => "dynamic",
        }
    }

    /// Get WAL metrics (only available for Native backend with native-v2 feature)
    #[cfg(feature = "native-v2")]
    pub fn get_wal_metrics(&self) -> Option<sqlitegraph::backend::native::v2::wal::WALManagerMetrics> {
        match self {
            BackendClient::Native(b) => b.get_wal_metrics(),
            BackendClient::Dynamic(_) => None, // Dynamic backends don't expose WAL-specific methods
            BackendClient::Sqlite(_) => None,
        }
    }

    /// Get active transaction count (only available for Native backend with native-v2 feature)
    #[cfg(feature = "native-v2")]
    pub fn get_active_transaction_count(&self) -> Option<usize> {
        match self {
            BackendClient::Native(b) => b.get_active_transaction_count(),
            BackendClient::Dynamic(_) => None,
            BackendClient::Sqlite(_) => None,
        }
    }
}
