//! Native backend module organization and minimal public interface.
//!
//! This module provides the native storage implementation for SQLiteGraph.
//! It implements file-based adjacency storage without SQLite dependency.
//! NO GraphBackend implementation is provided here - this is storage layer only.

// Module organization
pub mod adjacency;
pub mod constants;
pub mod cpu_tuning;
pub mod edge_store;
pub mod graph_file;
pub mod node_cache;
pub mod node_store;
pub mod optimizations; // Internal performance optimizations
pub mod types; // CPU detection and optimization mapping

// Phase 10: Header architecture refactor
pub mod persistent_header;
pub mod transaction_state;

// Include GraphBackend implementation
pub mod graph_backend;
pub mod graph_ops;
pub mod graph_validation;

// V2 clustered edge kernel (Phase 14)
pub mod v2;

// Phase 3: V1 Legacy Prevention - Permanent V1 ban
#[path = "v1_prevention.rs"]
pub mod v1_prevention;

// Re-export public types for external use
pub use types::{
    CpuProfile, EdgeFlags, EdgeRecord, FileHeader, FileOffset, NativeBackendError, NativeEdgeId,
    NativeNodeId, NativeResult, NodeFlags, NodeRecord, RecordSize,
};

pub use adjacency::{AdjacencyHelpers, AdjacencyIterator, Direction};
pub use edge_store::EdgeStore;
pub use graph_backend::NativeGraphBackend;
pub use graph_file::{GraphFile, decode_persistent_header, encode_persistent_header, get_slice_safe};
pub use node_cache::NodeRecordCache;
pub use node_store::{NodeStore, clear_node_cache};

// V2 clustered edge kernel re-exports
pub use v2::{EdgeCluster, EdgeRecordCompactExt, FreeSpaceManager, NodeRecordV2, StringTable};
