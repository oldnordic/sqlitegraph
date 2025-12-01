//! SQLite-backed graph database for embedded use.
//!
//! Provides a lightweight, deterministic graph database with:
//! - Entity and edge storage with metadata
//! - Pattern matching with cache-enabled fast-path
//! - MVCC-lite snapshots for read isolation
//! - Deterministic indexing and querying

// Core modules - public API
pub mod backend_client;
pub mod errors;
pub mod graph;
pub mod mvcc;
pub mod pattern_engine;
pub mod pattern_engine_cache;
pub mod query;
pub mod recovery;
pub mod reindex;

// Internal modules - not part of public API
pub mod algo; // Public for tests
mod api_ergonomics;
pub mod backend; // Public for binary
pub mod backend_selector; // Public for tests
pub mod bench_gates; // Public for tests
pub mod bench_meta; // Public for tests
pub mod bench_regression; // Public for tests
pub mod bench_utils; // Public for tests
pub mod bfs; // Public for tests
pub mod cache; // Public for tests
pub mod cli_reasoning; // Public for binary
pub mod client; // Public for binary
pub mod dsl; // Public for examples
pub mod dual_orchestrator; // Public for tests
pub mod dual_read; // Public for tests
pub mod dual_runner; // Public for tests
pub mod dual_runtime; // Public for tests
pub mod dual_write; // Public for tests
pub mod fault_injection; // Public for tests
pub mod graph_opt; // Public for tests
pub mod index; // Public for tests
pub mod migration; // Public for examples
pub mod multi_hop; // Public for tests
pub mod pattern; // Public for binary
pub mod pipeline; // Public for binary
pub mod reasoning; // Public for binary
pub mod safety; // Public for binary
pub mod schema; // Public for tests
pub mod subgraph; // Public for binary

// Public API exports
pub use api_ergonomics::{Label, NodeId, explain_pipeline}; // Pipeline explanation and ergonomic types for tests
pub use backend::SqliteGraphBackend; // SQLite backend for binary
pub use backend_client::BackendClient; // Client interface
pub use cache::CacheStats; // Cache statistics for tests
pub use cli_reasoning::handle_command; // CLI command handling for binary
pub use dsl::{DslResult, parse_dsl}; // DSL parsing for tests
pub use errors::SqliteGraphError; // Error type for all SQLiteGraph operations
pub use graph::{GraphEdge, GraphEntity, SqliteGraph}; // Core graph types and SQLite backend
pub use graph_opt::{
    GraphEdgeCreate, GraphEntityCreate, bulk_insert_edges, bulk_insert_entities, cache_stats,
}; // Test utilities
pub use index::{add_label, add_property};
pub use mvcc::{GraphSnapshot, SnapshotState}; // MVCC-lite snapshot system
pub use pattern_engine::{PatternTriple, TripleMatch, match_triples}; // Triple pattern matching
pub use pattern_engine_cache::match_triples_fast; // Cached pattern matching
pub use query::GraphQuery; // High-level query interface
pub use reasoning::ReasoningConfig; // Reasoning configuration for examples
pub use recovery::{dump_graph_to_path, load_graph_from_path, load_graph_from_reader}; // Graph backup and restore
pub use reindex::{ReindexConfig, ReindexProgress, ReindexResult, ReindexStage}; // Database reindexing utilities
pub use safety::{run_deep_safety_checks, run_safety_checks}; // Safety checks for tests // Index utilities for tests
