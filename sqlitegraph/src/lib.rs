//! SQLite-based graph database with dual backend support.
//!
//! sqlitegraph provides a lightweight, deterministic graph database for embedded Rust applications.
//! It supports both SQLite and Native storage backends through a unified API.
//!
//! # Features
//!
//! - **Dual Backend Support**: Choose between SQLite (feature-rich) and Native (performance-optimized) backends
//! - **Entity and Edge Storage**: Rich metadata support with JSON serialization
//! - **Pattern Matching**: Efficient triple pattern matching with cache-enabled fast-path
//! - **Traversal Algorithms**: Built-in BFS, k-hop, and shortest path algorithms
//! - **MVCC Snapshots**: Read isolation with snapshot consistency
//! - **Bulk Operations**: High-performance batch insertions for large datasets
//! - **Configuration**: Runtime backend selection with fine-grained options
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use sqlitegraph::{open_graph, GraphConfig, BackendKind};
//!
//! // Use SQLite backend (default)
//! let cfg = GraphConfig::sqlite();
//! let graph = open_graph("my_graph.db", &cfg)?;
//!
//! // Or use Native backend
//! let cfg = GraphConfig::native();
//! let graph = open_graph("my_graph.db", &cfg)?;
//!
//! // Both backends support the same operations
//! let node_id = graph.insert_node(/* node spec */)?;
//! let neighbor_ids = graph.neighbors(node_id, /* query */)?;
//! ```
//!
//! # Backend Selection
//!
//! ## SQLite Backend (Default)
//! - ACID transactions with rollback support
//! - Complex queries beyond basic graph operations
//! - Standard SQLite file format and tooling
//! - Mature ecosystem and compatibility
//!
//! ## Native Backend
//! - Optimized for graph operations
//! - Simplified deployment without SQLite dependencies
//! - Fast startup with large datasets
//! - Custom binary format for graph data
//!
//! # Public API Organization
//!
//! This crate exports a clean, stable public API organized as follows:
//!
//! ## Core Types
//! - [`GraphEntity`] - Graph node/vertex representation
//! - [`GraphEdge`] - Graph edge/relationship representation
//! - [`GraphBackend`] - Unified trait for backend implementations
//! - [`SqliteGraphBackend`] - SQLite backend implementation
//! - [`NativeGraphBackend`] - Native backend implementation
//!
//! ## Configuration
//! - [`BackendKind`] - Runtime backend selection enum
//! - [`GraphConfig`] - Unified configuration for both backends
//! - [`SqliteConfig`] - SQLite-specific options
//! - [`NativeConfig`] - Native-specific options
//! - [`open_graph()`] - Unified factory function
//!
//! ## Operations
//! - [`insert_node()`], [`insert_edge()`] - Single entity/edge insertion
//! - [`bulk_insert_entities()`], [`bulk_insert_edges()` - Batch operations
//! - [`neighbors()`] - Direct neighbor queries
//! - [`bfs()`], [`k_hop()`], [`shortest_path()`] - Graph traversal algorithms
//! - [`pattern_engine`] - Pattern matching and triple storage
//!
//! ## Utilities
//! - [`SqliteGraphError`] - Comprehensive error handling
//! - [`GraphSnapshot`] - MVCC snapshot system
//! - [`recovery`] - Database backup and restore utilities
//! - [`query::GraphQuery`] - High-level query interface

// Core public modules
pub mod backend;
pub mod config;
pub mod errors;
pub mod graph;

// Re-export core utilities that are stable public APIs
pub use api_ergonomics::{Label, NodeId, PropertyKey, PropertyValue};
pub use graph_opt::{
    GraphEdgeCreate, GraphEntityCreate, bulk_insert_edges, bulk_insert_entities, cache_stats,
};
pub use index::{add_label, add_property};
pub use mvcc::{GraphSnapshot, SnapshotState};
pub use pattern_engine::{PatternTriple, TripleMatch, match_triples};
pub use pattern_engine_cache::match_triples_fast;
pub use query::GraphQuery;
pub use recovery::{dump_graph_to_path, load_graph_from_path, load_graph_from_reader};

// Re-export backend implementations
pub use backend::{BackendDirection, ChainStep, GraphBackend};
pub use backend::{EdgeSpec, NativeGraphBackend, NeighborQuery, NodeSpec, SqliteGraphBackend};

// Re-export configuration and factory
pub use config::{BackendKind, GraphConfig, NativeConfig, SqliteConfig, open_graph};

// Re-export error types
pub use errors::SqliteGraphError;

// Re-export graph core types
pub use graph::{GraphEdge, GraphEntity, SqliteGraph};

// Internal modules - not part of public API
pub mod algo; // Public for tests
mod api_ergonomics;
pub mod backend_selector;
pub mod bfs; // Public for tests
pub mod cache; // Public for tests
mod client; // Public for binary
pub mod dual_runner; // Public for tests
mod fault_injection; // Public for tests
pub mod graph_opt; // Public for tests
pub mod index; // Public for tests
pub mod multi_hop; // Public for tests
mod pattern_engine_cache; // Already moved to core above
mod reasoning; // Public for binary
pub mod schema; // Public for tests // Public for tests

// Core public modules (these were accidentally removed)
pub mod mvcc; // Already exported above
pub mod pattern_engine; // Already exported above
pub mod query; // Already exported above
pub mod recovery; // Already exported above

// Modules that need to remain public for specific use cases
pub mod bench_gates; // Public for tests
pub mod bench_meta; // Public for tests
pub mod bench_regression; // Public for tests
pub mod bench_utils; // Public for tests
pub mod dsl; // Public for examples
pub mod pattern; // Public for binary

// Re-export cache statistics for benchmarking
pub use cache::CacheStats;
