//! Main configuration for graph construction.

use super::{kinds::BackendKind, native::NativeConfig, sqlite::SqliteConfig};

/// Complete configuration for graph construction.
///
/// This structure combines backend selection with backend-specific
/// configuration options.
#[derive(Clone, Debug)]
pub struct GraphConfig {
    /// Which backend to use for graph storage
    pub backend: BackendKind,
    /// SQLite-specific configuration options
    pub sqlite: SqliteConfig,
    /// Native-specific configuration options
    pub native: NativeConfig,
}

impl GraphConfig {
    /// Create a new configuration with the specified backend.
    pub fn new(backend: BackendKind) -> Self {
        let sqlite_config = match backend {
            BackendKind::SQLite => SqliteConfig::default(),
            BackendKind::Native => SqliteConfig {
                without_migrations: true,
                ..Default::default()
            },
        };

        Self {
            backend,
            sqlite: sqlite_config,
            native: NativeConfig::default(),
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

    /// Set the CPU profile for the Native backend (builder pattern)
    pub fn with_cpu_profile(mut self, profile: crate::backend::native::CpuProfile) -> Self {
        self.native.cpu_profile = Some(profile);
        self
    }

    /// Configure SQLite backend with custom settings (builder pattern)
    pub fn with_sqlite_config<F>(mut self, config_fn: F) -> Self
    where
        F: FnOnce(SqliteConfig) -> SqliteConfig,
    {
        self.sqlite = config_fn(self.sqlite);
        self
    }

    /// Configure Native backend with custom settings (builder pattern)
    pub fn with_native_config<F>(mut self, config_fn: F) -> Self
    where
        F: FnOnce(NativeConfig) -> NativeConfig,
    {
        self.native = config_fn(self.native);
        self
    }
}

impl Default for GraphConfig {
    fn default() -> Self {
        Self::new(BackendKind::SQLite)
    }
}
