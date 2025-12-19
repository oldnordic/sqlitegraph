//! Backend selection for graph storage implementations.

/// Backend selection enum for choosing between storage implementations.
///
/// This enum allows runtime selection of the graph storage backend
/// without compile-time dependencies on specific implementations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendKind {
    /// SQLite-based backend with full SQL capabilities
    SQLite,
    /// Native file-based backend with adjacency storage
    Native,
}

impl Default for BackendKind {
    fn default() -> Self {
        BackendKind::SQLite
    }
}