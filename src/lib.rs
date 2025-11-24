//! SQLite-backed graph primitives for SynCore.
//! Run Criterion benchmarks with `cargo bench` to inspect reports under `target/criterion`.

pub mod algo;
pub mod backend;
pub mod backend_selector;
pub mod bench_meta;
pub mod bench_regression;
pub mod bench_utils;
pub mod bfs;
pub mod cache;
pub mod client;
pub mod dual_orchestrator;
pub mod dual_read;
pub mod dual_runner;
pub mod dual_runtime;
pub mod dual_write;
pub mod errors;
pub mod graph;
pub mod migration;
pub mod multi_hop;
pub mod pattern;
pub mod query;
pub mod reasoning;
pub mod schema;

pub use crate::errors::SqliteGraphError;
pub use crate::graph::{GraphEdge, GraphEntity, SqliteGraph};
pub use crate::migration::{MigrationIds, MigrationManager, MigrationStats};
pub use crate::multi_hop::ChainStep;
pub use crate::pattern::{NodeConstraint, PatternLeg, PatternMatch, PatternQuery};
pub use crate::query::GraphQuery;
pub use crate::reasoning::{GraphReasoner, ReasoningCandidate, ReasoningConfig};
