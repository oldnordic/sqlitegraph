//! Graph introspection APIs for debugging and observability.
//!
//! This module provides structured introspection capabilities for SQLiteGraph,
//! enabling developers and AI agents to inspect internal graph state for debugging
//! and observability purposes.

use serde::Serialize;
use std::path::Path;

use crate::cache::CacheStats;
use crate::errors::SqliteGraphError;

/// Comprehensive introspection data for a graph instance.
///
/// This struct provides a JSON-serializable snapshot of graph state,
/// designed for both human debugging and LLM consumption.
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
/// println!("Cache hit ratio: {:.2}%", intro.cache_stats.hit_ratio());
///
/// // Serialize to JSON for LLM consumption
/// let json = serde_json::to_string_pretty(&intro)?;
/// ```
#[derive(Debug, Clone, Serialize)]
pub struct GraphIntrospection {
    /// Backend type identifier ("sqlite" or "native-v2")
    pub backend_type: String,

    /// Total number of nodes in the graph
    pub node_count: usize,

    /// Total number of edges in the graph (estimated for large graphs)
    pub edge_count: EdgeCount,

    /// Adjacency cache statistics
    pub cache_stats: CacheStats,

    /// Memory usage estimate in bytes (if available)
    pub memory_usage: Option<usize>,

    /// Database file size in bytes (for file-based backends)
    pub file_size: Option<u64>,

    /// WAL file size in bytes (for backends with WAL enabled)
    pub wal_size: Option<u64>,

    /// Whether this is an in-memory database
    pub is_in_memory: bool,
}

/// Edge count representation.
///
/// Provides either an exact count or an estimate for large graphs
/// where counting would be prohibitively expensive.
#[derive(Debug, Clone, Serialize)]
pub enum EdgeCount {
    /// Exact edge count (for small to medium graphs)
    Exact(usize),

    /// Estimated edge count with confidence interval
    Estimate {
        /// Estimated count
        count: usize,
        /// Lower bound of confidence interval
        min: usize,
        /// Upper bound of confidence interval
        max: usize,
        /// Sample size used for estimation
        sample_size: usize,
    },

    /// Edge counting not available for this backend
    Unavailable,
}

impl EdgeCount {
    /// Get the primary count value (exact or estimated).
    pub fn value(&self) -> Option<usize> {
        match self {
            EdgeCount::Exact(count) => Some(*count),
            EdgeCount::Estimate { count, .. } => Some(*count),
            EdgeCount::Unavailable => None,
        }
    }
}

impl CacheStats {
    /// Calculate cache hit ratio as a percentage.
    ///
    /// Returns None if there have been no cache accesses.
    pub fn hit_ratio(&self) -> Option<f64> {
        let total = self.hits + self.misses;
        if total == 0 {
            None
        } else {
            Some((self.hits as f64 / total as f64) * 100.0)
        }
    }
}

/// Introspection-specific errors.
#[derive(Debug, Clone, thiserror::Error)]
pub enum IntrospectError {
    /// Failed to query node count
    #[error("Failed to query node count: {0}")]
    NodeCountError(String),

    /// Failed to query edge count
    #[error("Failed to query edge count: {0}")]
    EdgeCountError(String),

    /// Failed to query file size
    #[error("Failed to query file size: {0}")]
    FileSizeError(String),

    /// Backend not supported for introspection
    #[error("Backend not supported: {0}")]
    UnsupportedBackend(String),
}

impl From<IntrospectError> for SqliteGraphError {
    fn from(err: IntrospectError) -> Self {
        SqliteGraphError::invalid_input(err.to_string())
    }
}

/// Get file size for a database path.
pub fn get_file_size<P: AsRef<Path>>(path: P) -> Option<u64> {
    std::fs::metadata(path.as_ref())
        .ok()
        .map(|m| m.len())
}

/// Get WAL file size for a database path.
pub fn get_wal_size<P: AsRef<Path>>(path: P) -> Option<u64> {
    let wal_path = path.as_ref().with_extension("wal");
    std::fs::metadata(wal_path)
        .ok()
        .map(|m| m.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_hit_ratio() {
        let stats = CacheStats {
            hits: 80,
            misses: 20,
            entries: 100,
        };

        assert_eq!(stats.hit_ratio(), Some(80.0));
    }

    #[test]
    fn test_cache_hit_ratio_no_accesses() {
        let stats = CacheStats {
            hits: 0,
            misses: 0,
            entries: 0,
        };

        assert_eq!(stats.hit_ratio(), None);
    }

    #[test]
    fn test_edge_count_exact() {
        let count = EdgeCount::Exact(1000);
        assert_eq!(count.value(), Some(1000));
    }

    #[test]
    fn test_edge_count_estimate() {
        let count = EdgeCount::Estimate {
            count: 1000,
            min: 900,
            max: 1100,
            sample_size: 100,
        };

        assert_eq!(count.value(), Some(1000));
    }

    #[test]
    fn test_edge_count_unavailable() {
        let count = EdgeCount::Unavailable;
        assert_eq!(count.value(), None);
    }
}
