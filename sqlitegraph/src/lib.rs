//! SQLite-based graph database with unified backend support.
//!
//! `sqlitegraph` provides a lightweight, deterministic graph database for embedded Rust applications.
//! It supports both SQLite and Native storage backends through a unified API.
//!
//! # Architecture
//!
//! The crate is organized into focused modules:
//!
//! ```text
//! sqlitegraph/
//! ├── graph/           # Core graph database (SqliteGraph, GraphEntity, GraphEdge)
//! ├── backend/         # Unified backend trait (GraphBackend, SqliteGraphBackend, NativeGraphBackend)
//! ├── algo/            # Graph algorithms (PageRank, Betweenness, Louvain, Label Propagation)
//! ├── hnsw/            # Vector similarity search (HNSW index, distance metrics)
//! ├── cache/           # LRU-K adjacency cache for traversal optimization
//! ├── introspection/   # Debugging and observability APIs
//! ├── progress/        # Progress tracking for long-running operations
//! ├── mvcc/            # MVCC-lite snapshot system
//! ├── pattern_engine/  # Triple pattern matching
//! ├── query/           # High-level query interface
//! └── recovery/        # Backup and restore utilities
//! ```
//!
//! # Features
//!
//! - **Dual Backend Support**: Choose between SQLite (feature-rich) and Native (performance-optimized) backends
//! - **Entity and Edge Storage**: Rich metadata support with JSON serialization
//! - **Pattern Matching**: Efficient triple pattern matching with cache-enabled fast-path
//! - **Traversal Algorithms**: Built-in BFS, k-hop, and shortest path algorithms
//! - **Graph Algorithms**: PageRank, Betweenness Centrality, Louvain, Label Propagation
//! - **Vector Search**: HNSW approximate nearest neighbor search with persistence
//! - **MVCC Snapshots**: Read isolation with snapshot consistency
//! - **Bulk Operations**: High-performance batch insertions for large datasets
//! - **Introspection**: Debugging APIs for cache stats, file sizes, edge counts
//! - **Progress Tracking**: Callback-based progress for long-running algorithms
//!
//! # Quick Start
//!
//! ```rust,ignore
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
//! ## Feature Matrix
//!
//! | Feature | SQLite Backend | Native Backend |
//! |---------|----------------|----------------|
//! | **ACID Transactions** | ✅ Full | ✅ WAL-based |
//! | **Graph Algorithms** | ✅ Full support | ✅ Full support |
//! | **HNSW Vector Search** | ✅ With persistence | ✅ In-memory |
//! | **MVCC Snapshots** | ✅ | ✅ |
//! | **Pattern Matching** | ✅ | ✅ |
//! | **Raw SQL Access** | ✅ Native | ❌ Not supported |
//! | **File Format** | SQLite DB | Custom binary |
//! | **Startup Time** | Fast | Faster |
//! | **Dependencies** | libsqlite3 | None (pure Rust) |
//! | **Write Performance** | Good | Better |
//! | **Query Performance** | Good | Better |
//!
//! ## When to Use SQLite Backend
//!
//! Choose SQLite backend when:
//! - **ACID guarantees** are critical for your application
//! - **Raw SQL access** needed for complex queries or joins
//! - **Database compatibility** with SQLite tools (sqlite3, DB Browser)
//! - **Mature ecosystem** with third-party tooling
//! - **HNSW persistence** required (vectors survive restarts)
//!
//! ## When to Use Native Backend
//!
//! Choose Native backend when:
//! - **Performance is critical** (faster reads/writes)
//! - **No external dependencies** desired (pure Rust)
//! - **Fast startup** with large datasets
//! - **Custom binary format** acceptable
//! - **HNSW in-memory only** (vectors persist in separate file)
//!
//! # Thread Safety
//!
//! ## SqliteGraph is NOT Thread-Safe
//!
//! `SqliteGraph` uses interior mutability (`RefCell`) and is **not `Sync`**:
//!
//! ```rust,ignore
//! use sqlitegraph::SqliteGraph;
//! use std::thread;
//!
//! let graph = SqliteGraph::open("test.db")?;
//!
//! // ❌ WRONG: Sharing graph across threads for writes
//! let graph_clone = graph;
//! thread::spawn(move || {
//!     graph_clone.insert_node(...)?; // DATA RACE!
//! });
//!
//! // ✅ CORRECT: Use snapshots for concurrent reads
//! let snapshot = graph.snapshot()?;
//! thread::spawn(move || {
//!     let neighbors = snapshot.neighbors(node_id)?; // Thread-safe
//! });
//! ```
//!
//! ## Concurrent Read Access
//!
//! Use [`GraphSnapshot`] for thread-safe concurrent reads:
//!
//! ```rust,ignore
//! use sqlitegraph::{GraphSnapshot, SqliteGraph};
//!
//! let graph = SqliteGraph::open("my_graph.db")?;
//!
//! // Create multiple snapshots for concurrent reads
//! let snapshot1 = graph.snapshot()?;
//! let snapshot2 = graph.snapshot()?;
//!
//! // Both snapshots can be used concurrently (thread-safe)
//! let handle1 = std::thread::spawn(move || {
//!     snapshot1.neighbors(node_id)
//! });
//!
//! let handle2 = std::thread::spawn(move || {
//!     snapshot2.neighbors(node_id)
//! });
//! ```
//!
//! ## Write Serialization
//!
//! All writes must be serialized:
//!
//! ```rust,ignore
//! // ✅ CORRECT: Single thread for all writes
//! let graph = SqliteGraph::open("my_graph.db")?;
//! for i in 0..1000 {
//!     graph.insert_node(...)?;
//!     graph.insert_edge(...)?;
//! }
//!
//! // ❌ WRONG: Concurrent writes
//! let graph = Arc::new(Mutex::new(graph));
//! let handle1 = thread::spawn(|| {
//!     let g = graph.lock().unwrap();
//!     g.insert_node(...)
//! });
//! let handle2 = thread::spawn(|| {
//!     let g = graph.lock().unwrap();
//!     g.insert_node(...)
//! });
//! // Even with Mutex, this can cause issues due to RefCell
//! ```
//!
//! # Error Handling
//!
//! All operations return [`Result<T, SqliteGraphError>`]:
//!
//! ```rust,ignore
//! use sqlitegraph::{SqliteGraph, SqliteGraphError};
//!
//! let graph = SqliteGraph::open("my_graph.db")?;
//!
//! match graph.insert_node(node_spec) {
//!     Ok(node_id) => println!("Created node {}", node_id),
//!     Err(SqliteGraphError::EntityNotFound) => {
//!         println!("Node not found");
//!     }
//!     Err(SqliteGraphError::DatabaseError(e)) => {
//!         eprintln!("Database error: {}", e);
//!     }
//!     Err(e) => {
//!         eprintln!("Other error: {}", e);
//!     }
//! }
//! ```
//!
//! # Performance Comparison
//!
//! ## Read Performance
//! - **SQLite Backend**: 10-100μs per neighbor lookup (cached: ~100ns)
//! - **Native Backend**: 1-10μs per neighbor lookup (cached: ~100ns)
//! - **Cache hit ratio**: 80-95% for traversal workloads
//!
//! ## Write Performance
//! - **SQLite Backend**: 100-500μs per insert (transaction-batched)
//! - **Native Backend**: 10-100μs per insert (transaction-batched)
//! - **Bulk insert**: 10-100x faster with `bulk_insert_entities()`
//!
//! ## Memory Usage
//! - **Base overhead**: O(V + E) for graph storage
//! - **Cache overhead**: 10-20% additional memory
//! - **HNSW index**: 2-3x vector data size
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
//! - [`bulk_insert_entities()`], [`bulk_insert_edges()`] - Batch operations
//! - [`neighbors()`] - Direct neighbor queries
//! - [`bfs()`], [`k_hop()`], [`shortest_path()`] - Graph traversal algorithms
//! - [`pattern_engine`] - Pattern matching and triple storage
//!
//! ## Graph Algorithms
//! - [`pagerank`] - PageRank centrality
//! - [`betweenness_centrality`] - Betweenness centrality
//! - [`louvain_communities`] - Louvain community detection
//! - [`label_propagation`] - Label propagation algorithm
//!
//! ## Vector Search
//! - [`hnsw::HnswIndex`] - HNSW vector search index
//! - [`hnsw::HnswConfig`] - HNSW configuration
//! - [`hnsw::DistanceMetric`] - Distance metrics (Cosine, Euclidean, etc.)
//!
//! ## Utilities
//! - [`SqliteGraphError`] - Comprehensive error handling
//! - [`GraphSnapshot`] - MVCC snapshot system
//! - [`GraphIntrospection`] - Introspection and debugging APIs
//! - [`ProgressCallback`] - Algorithm progress tracking
//! - [`recovery`] - Database backup and restore utilities

// Core public modules
pub mod backend;
pub mod config;
pub mod debug;
pub mod errors;
pub mod graph;
pub mod introspection;

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
pub use backend::{BackupResult, EdgeSpec, NativeGraphBackend, NeighborQuery, NodeSpec, SqliteGraphBackend};

// Re-export backup API for convenience
#[cfg(feature = "native-v2")]
pub use backend::native::v2::backup::{BackupConfig, create_backup as database_backup};

// Re-export WAL functionality for native backend
#[cfg(feature = "native-v2")]
pub use backend::native::v2::wal::{
    V2WALConfig, V2WALManager,
    IsolationLevel, WALManagerMetrics,
};

// Re-export WAL integration for advanced usage
#[cfg(feature = "native-v2")]
pub use backend::native::v2::wal::{
    V2GraphWALIntegrator, GraphWALIntegrationConfig,
    GraphOperationResult, OperationMetrics,
};

// Re-export configuration and factory
pub use config::{BackendKind, GraphConfig, NativeConfig, SqliteConfig, open_graph};

// Re-export error types
pub use errors::SqliteGraphError;

// Re-export graph core types
pub use graph::{GraphEdge, GraphEntity, SqliteGraph};

// Re-export graph algorithms
pub use algo::{
    betweenness_centrality, label_propagation, louvain_communities, pagerank,
    betweenness_centrality_with_progress, louvain_communities_with_progress, pagerank_with_progress,
};

// Re-export progress tracking
pub use progress::{ConsoleProgress, NoProgress, ProgressCallback, ProgressState};

// Re-export introspection API
pub use introspection::{GraphIntrospection, EdgeCount, IntrospectError};

// Internal modules - not part of public API
pub mod algo; // Public for tests
pub mod progress; // Public for tests and progress API usage
mod api_ergonomics;
pub mod backend_selector;
pub mod bfs; // Public for tests
pub mod cache; // Public for tests
mod client; // Public for binary
mod fault_injection; // Public for tests
pub mod graph_opt; // Public for tests
pub mod index; // Public for tests
pub mod multi_hop; // Public for tests
mod pattern_engine_cache; // Already moved to core above
pub mod query_cache; // Public for internal use and tests
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
pub mod hnsw;
pub mod pattern; // Public for binary // HNSW vector search capabilities

// Re-export cache statistics for benchmarking
pub use cache::CacheStats;

/// Create a backup of a SQLiteGraph Native V2 database
///
/// This is a convenience function for creating database backups.
/// For more control over backup options, use `database_backup` with `BackupConfig`.
///
/// # Arguments
/// * `db_path` - Path to the database file
/// * `backup_dir` - Directory where backup will be stored
///
/// # Example
/// ```no_run
/// use sqlitegraph;
/// use std::path::Path;
///
/// let result = sqlitegraph::create_backup(
///     Path::new("mydb.v2"),
///     Path::new("backups")
/// );
/// # Ok::<(), sqlitegraph::SqliteGraphError>(())
/// ```
#[cfg(feature = "native-v2")]
pub fn create_backup(
    db_path: &std::path::Path,
    backup_dir: &std::path::Path,
) -> Result<BackupResult, SqliteGraphError> {
    database_backup(db_path, crate::backend::native::v2::backup::BackupConfig::new(backup_dir))
        .map_err(|e| SqliteGraphError::Backend(e.to_string()))
}
