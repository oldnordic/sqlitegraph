//! SQLite-backed graph database implementation.
//!
//! Provides a lightweight, deterministic graph database with entity and edge storage,
//! pattern matching, MVCC-lite snapshots, and deterministic indexing.

pub use self::core::SqliteGraph;

mod adjacency;
mod core;
mod edge_ops;
mod entity_ops;
mod metrics;
mod metrics_schema;
mod pattern_matching;
mod snapshot;
mod types;

pub use metrics::{GraphMetricsSnapshot, InstrumentedConnection};
pub use types::{GraphEdge, GraphEntity};
