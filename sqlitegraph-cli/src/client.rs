use sqlitegraph::backend::GraphBackend;
use sqlitegraph::backend::SqliteGraphBackend;

/// CLI backend client supporting SQLite and V3 backends
///
/// Both variants box their payload so the enum stays pointer-sized
/// (8 bytes) instead of being bloated to ~1.2 KB by the V3 variant,
/// avoiding `clippy::large_enum_variant`. The CLI only ever holds
/// one `CliClient` at a time, so the extra heap indirection has no
/// measurable cost vs. paying for the largest variant on every value.
pub enum CliClient {
    Sqlite(Box<SqliteGraphBackend>),
    #[cfg(feature = "native-v3")]
    V3(Box<sqlitegraph::backend::native::v3::V3Backend>),
}

impl CliClient {
    /// Open database with specified backend
    pub fn open(backend: super::cli::BackendType, path: &std::path::Path) -> anyhow::Result<Self> {
        match backend {
            super::cli::BackendType::Sqlite => {
                let graph = sqlitegraph::SqliteGraph::open(path)?;
                Ok(Self::Sqlite(Box::new(SqliteGraphBackend::from_graph(
                    graph,
                ))))
            }
            #[cfg(feature = "native-v3")]
            super::cli::BackendType::V3 => {
                use sqlitegraph::backend::native::v3::V3Backend;
                let backend = if path.exists() {
                    V3Backend::open(path)?
                } else {
                    V3Backend::create(path)?
                };
                Ok(Self::V3(Box::new(backend)))
            }
        }
    }

    /// Open in-memory database
    pub fn open_in_memory(backend: super::cli::BackendType) -> anyhow::Result<Self> {
        match backend {
            super::cli::BackendType::Sqlite => {
                let graph = sqlitegraph::SqliteGraph::open_in_memory()?;
                Ok(Self::Sqlite(Box::new(SqliteGraphBackend::from_graph(
                    graph,
                ))))
            }
            #[cfg(feature = "native-v3")]
            super::cli::BackendType::V3 => {
                anyhow::bail!("V3 backend does not support in-memory mode")
            }
        }
    }

    /// Get backend trait object
    pub fn backend(&self) -> &dyn GraphBackend {
        match self {
            Self::Sqlite(b) => b.as_ref(),
            #[cfg(feature = "native-v3")]
            Self::V3(b) => b.as_ref(),
        }
    }

    /// Get SQLite graph reference (if SQLite backend)
    pub fn sqlite_graph(&self) -> Option<&sqlitegraph::SqliteGraph> {
        match self {
            Self::Sqlite(b) => Some(b.graph()),
            #[cfg(feature = "native-v3")]
            Self::V3(_) => None,
        }
    }

    /// Get SQLite backend reference (for Cypher queries)
    pub fn sqlite_backend(&self) -> Option<&SqliteGraphBackend> {
        match self {
            Self::Sqlite(b) => Some(b.as_ref()),
            #[cfg(feature = "native-v3")]
            Self::V3(_) => None,
        }
    }

    /// Get backend name
    pub fn backend_name(&self) -> &'static str {
        match self {
            Self::Sqlite(_) => "sqlite",
            #[cfg(feature = "native-v3")]
            Self::V3(_) => "v3",
        }
    }

    /// Get node count
    pub fn node_count(&self) -> anyhow::Result<usize> {
        let ids = self.backend().entity_ids()?;
        Ok(ids.len())
    }

    /// Get backend type enum
    pub fn backend_type(&self) -> super::cli::BackendType {
        match self {
            Self::Sqlite(_) => super::cli::BackendType::Sqlite,
            #[cfg(feature = "native-v3")]
            Self::V3(_) => super::cli::BackendType::V3,
        }
    }

    /// Get V3 backend reference (if V3 backend)
    #[cfg(feature = "native-v3")]
    pub fn v3_backend(&self) -> Option<&sqlitegraph::backend::native::v3::V3Backend> {
        match self {
            Self::Sqlite(_) => None,
            Self::V3(b) => Some(b.as_ref()),
        }
    }
}
