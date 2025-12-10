//! Native backend module organization and minimal public interface.
//!
//! This module provides the native storage implementation for SQLiteGraph.
//! It implements file-based adjacency storage without SQLite dependency.
//! NO GraphBackend implementation is provided here - this is storage layer only.

// Module organization
pub mod adjacency;
pub mod constants;
pub mod edge_store;
pub mod graph_file;
pub mod node_store;
pub mod types;

// Include GraphBackend implementation
pub mod graph_backend;
pub mod graph_ops;
pub mod graph_validation;

// Re-export public types for external use
pub use types::{
    EdgeFlags, EdgeRecord, FileHeader, FileOffset, NativeBackendError, NativeEdgeId, NativeNodeId,
    NativeResult, NodeFlags, NodeRecord, RecordSize,
};

pub use adjacency::{AdjacencyHelpers, AdjacencyIterator, Direction};
pub use edge_store::EdgeStore;
pub use graph_backend::NativeGraphBackend;
pub use graph_file::{GraphFile, decode_header, encode_header};
pub use node_store::NodeStore;
