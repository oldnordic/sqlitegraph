//! Comprehensive error type for native backend operations

use super::{NativeNodeId, NativeEdgeId};

/// Comprehensive error type for native backend operations
#[derive(Debug, thiserror::Error)]
pub enum NativeBackendError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid magic number: expected {expected:08x}, found {found:08x}")]
    InvalidMagic { expected: u64, found: u64 },

    #[error("Unsupported version: {version} (supported: {supported_version})")]
    UnsupportedVersion {
        version: u32,
        supported_version: u32,
    },

    #[error("Invalid header field '{field}': {reason}")]
    InvalidHeader { field: String, reason: String },

    #[error("Invalid header checksum: expected {expected:08x}, found {found:08x}")]
    InvalidChecksum { expected: u64, found: u64 },

    #[error("Node ID {id} out of bounds (valid range: 1-{max_id})")]
    InvalidNodeId {
        id: NativeNodeId,
        max_id: NativeNodeId,
    },

    #[error("Edge ID {id} out of bounds (valid range: 1-{max_id})")]
    InvalidEdgeId {
        id: NativeEdgeId,
        max_id: NativeEdgeId,
    },

    #[error("Corrupt node record at node {node_id}: {reason}")]
    CorruptNodeRecord {
        node_id: NativeNodeId,
        reason: String,
    },

    #[error("Corrupt edge record at edge {edge_id}: {reason}")]
    CorruptEdgeRecord {
        edge_id: NativeEdgeId,
        reason: String,
    },

    #[error(
        "Inconsistent adjacency for node {node_id}: {count} {direction} edges in metadata but file indicates {file_count}"
    )]
    InconsistentAdjacency {
        node_id: NativeNodeId,
        count: u32,
        direction: String,
        file_count: u32,
    },

    #[error("File too small: {size} bytes (minimum {min_size} bytes required)")]
    FileTooSmall { size: u64, min_size: u64 },

    #[error("Record too large: {size} bytes (maximum {max_size} bytes)")]
    RecordTooLarge { size: u32, max_size: u32 },

    #[error("UTF-8 encoding error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("JSON serialization error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("Binary serialization error: {0}")]
    BincodeError(#[from] Box<bincode::ErrorKind>),

    #[error("Invalid UTF-8 string: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),

    #[error("Buffer too small: {size} bytes (need at least {min_size} bytes)")]
    BufferTooSmall { size: usize, min_size: usize },

    #[error("Invalid string table offset: {offset}")]
    InvalidStringOffset { offset: u32 },

    #[error("Corrupt string table: {reason}")]
    CorruptStringTable { reason: String },

    #[error("Invalid magic bytes: found {found:?}")]
    InvalidMagicBytes { found: [u8; 8] },

    #[error("Validation failed for metric '{metric}': expected {expected}, found {actual}")]
    ValidationFailed {
        metric: String,
        expected: f64,
        actual: f64,
    },

    #[error("Out of space in file")]
    OutOfSpace,

    #[error("Corrupt free space: {reason}")]
    CorruptFreeSpace { reason: String },

    #[error("Transaction rolled back: {0}")]
    TransactionRolledBack(String),

    #[error("Node {node_id} not found during {operation}")]
    NodeNotFound {
        node_id: NativeNodeId,
        operation: String,
    },
}