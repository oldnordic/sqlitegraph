//! SQLite-backed graph primitives for SynCore.
//! Run Criterion benchmarks with `cargo bench` to inspect reports under `target/criterion`.

pub mod algo;
pub mod api_ergonomics;
pub mod backend;
pub mod backend_client;
pub mod backend_selector;
pub mod bench_gates;
pub mod bench_meta;
pub mod bench_regression;
pub mod bench_utils;
pub mod bfs;
pub mod cache;
pub mod cli_reasoning;
pub mod client;
pub mod dsl;
pub mod dual_orchestrator;
pub mod dual_read;
pub mod dual_runner;
pub mod dual_runtime;
pub mod dual_write;
pub mod errors;
pub mod graph;
pub mod graph_opt;
pub mod index;
pub mod migration;
pub mod multi_hop;
pub mod pattern;
pub mod pipeline;
pub mod query;
pub mod reasoning;
pub mod safety;
pub mod schema;
pub mod subgraph;

pub use crate::api_ergonomics::{
    EdgeId, Label, NodeId, PipelineExplanation, PropertyKey, PropertyValue, explain_pipeline,
};
pub use crate::backend_client::{BackendClient, CommandLineConfig, Constraint, MatchResult};
pub use crate::cache::CacheStats;
pub use crate::dsl::{DslResult, parse_dsl};
pub use crate::errors::SqliteGraphError;
pub use crate::graph::{GraphEdge, GraphEntity, GraphMetricsSnapshot, SqliteGraph};
pub use crate::graph_opt::{
    GraphEdgeCreate, GraphEntityCreate, adjacency_fetch_incoming_batch,
    adjacency_fetch_outgoing_batch, bulk_insert_edges, bulk_insert_entities, cache_clear_ranges,
    cache_stats,
};
pub use crate::index::{add_label, add_property, get_entities_by_label, get_entities_by_property};
pub use crate::migration::{MigrationIds, MigrationManager, MigrationStats};
pub use crate::multi_hop::ChainStep;
pub use crate::pattern::{NodeConstraint, PatternLeg, PatternMatch, PatternQuery};
pub use crate::pipeline::{PipelineResult, ReasoningPipeline, ReasoningScoreConfig, ReasoningStep};
pub use crate::query::GraphQuery;
pub use crate::reasoning::{GraphReasoner, ReasoningCandidate, ReasoningConfig};
pub use crate::safety::{
    SafetyError, SafetyReport, run_safety_checks, run_strict_safety_checks,
    validate_labels_properties, validate_no_duplicate_edges, validate_referential_integrity,
};
pub use crate::subgraph::{Subgraph, SubgraphRequest};
