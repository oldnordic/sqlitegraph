//! Core type definitions for native backend.
//!
//! This module contains all the core structs, enums, and error types needed
//! for the native graph database file format.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Native node identifier (alias for i64 to match existing NodeId)
pub type NativeNodeId = i64;

/// Native edge identifier (alias for i64 to match existing EdgeId)
pub type NativeEdgeId = i64;

/// File offset within the graph database file
pub type FileOffset = u64;

/// Size of variable-length records
pub type RecordSize = u32;

/// Node flags bitfield for marking node state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeFlags(pub u32);

impl NodeFlags {
    /// Create empty flags
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Check if flag is set
    pub const fn contains(&self, flag: Self) -> bool {
        (self.0 & flag.0) != 0
    }

    /// Set a flag
    pub const fn set(&self, flag: Self) -> Self {
        Self(self.0 | flag.0)
    }

    /// Clear a flag
    pub const fn clear(&self, flag: Self) -> Self {
        Self(self.0 & !flag.0)
    }

    /// No flags set
    pub const NONE: Self = Self(0);
}

impl fmt::LowerHex for NodeFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:x}", self.0)
    }
}

/// Edge flags bitfield for marking edge state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EdgeFlags(pub u16);

impl EdgeFlags {
    /// Create empty flags
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Check if flag is set
    pub const fn contains(&self, flag: Self) -> bool {
        (self.0 & flag.0) != 0
    }

    /// Set a flag
    pub const fn set(&self, flag: Self) -> Self {
        Self(self.0 | flag.0)
    }

    /// Clear a flag
    pub const fn clear(&self, flag: Self) -> Self {
        Self(self.0 & !flag.0)
    }

    /// No flags set
    pub const NONE: Self = Self(0);
}

impl fmt::LowerHex for EdgeFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:x}", self.0)
    }
}

/// File header structure for native graph database
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHeader {
    /// Magic number (should be "SQLTGF\0")
    pub magic: [u8; 8],
    /// File format version
    pub version: u32,
    /// Feature flags bitfield
    pub flags: u32,
    /// Total number of nodes in the file
    pub node_count: u64,
    /// Total number of edges in the file
    pub edge_count: u64,
    /// Schema version
    pub schema_version: u64,
    /// Offset to node data section
    pub node_data_offset: u64,
    /// Offset to edge data section
    pub edge_data_offset: u64,
    /// Header checksum
    pub checksum: u64,
}

impl FileHeader {
    /// Create a new header with default values
    pub fn new() -> Self {
        Self {
            magic: super::constants::MAGIC_BYTES,
            version: super::constants::FILE_FORMAT_VERSION,
            flags: super::constants::DEFAULT_FEATURE_FLAGS,
            node_count: 0,
            edge_count: 0,
            schema_version: super::constants::DEFAULT_SCHEMA_VERSION,
            node_data_offset: super::constants::HEADER_SIZE,
            // Start edge data after node data area (reserve space for nodes)
            // Reserve ~1MB for node data: 64 + 4096 * 256 = 1,048,640 bytes
            edge_data_offset: super::constants::HEADER_SIZE + (4096 * 256),
            checksum: 0,
        }
    }

    /// Validate the header for consistency
    pub fn validate(&self) -> Result<(), NativeBackendError> {
        // Check magic number
        if self.magic != super::constants::MAGIC_BYTES {
            return Err(NativeBackendError::InvalidMagic {
                expected: u64::from_be_bytes(super::constants::MAGIC_BYTES),
                found: u64::from_be_bytes(self.magic),
            });
        }

        // Check version
        if self.version != super::constants::FILE_FORMAT_VERSION {
            return Err(NativeBackendError::UnsupportedVersion {
                version: self.version,
            });
        }

        // Check offset ordering
        if self.node_data_offset < super::constants::HEADER_SIZE {
            return Err(NativeBackendError::InvalidHeader {
                field: "node_data_offset".to_string(),
                reason: "must be >= header_size".to_string(),
            });
        }

        if self.edge_data_offset < self.node_data_offset {
            return Err(NativeBackendError::InvalidHeader {
                field: "edge_data_offset".to_string(),
                reason: "must be >= node_data_offset".to_string(),
            });
        }

        Ok(())
    }

    /// Compute checksum for the header
    pub fn compute_checksum(&self) -> u64 {
        let mut checksum = super::constants::checksum::XOR_SEED;

        // Simple XOR checksum over all fields except checksum itself
        checksum ^= u64::from_be_bytes(self.magic);
        checksum ^= self.version as u64;
        checksum ^= self.flags as u64;
        checksum ^= self.node_count;
        checksum ^= self.edge_count;
        checksum ^= self.schema_version;
        checksum ^= self.node_data_offset;
        checksum ^= self.edge_data_offset;

        checksum
    }

    /// Update the checksum field
    pub fn update_checksum(&mut self) {
        self.checksum = self.compute_checksum();
    }

    /// Verify the checksum field
    pub fn verify_checksum(&self) -> Result<(), NativeBackendError> {
        let expected_checksum = self.compute_checksum();
        if self.checksum != expected_checksum {
            return Err(NativeBackendError::InvalidChecksum {
                expected: expected_checksum,
                found: self.checksum,
            });
        }
        Ok(())
    }
}

/// Node record structure for storage
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRecord {
    /// Unique node identifier
    pub id: NativeNodeId,
    /// Node flags bitfield
    pub flags: NodeFlags,
    /// Node type/kind (e.g., "Function", "Variable", etc.)
    pub kind: String,
    /// Human-readable node name
    pub name: String,
    /// JSON metadata for the node
    pub data: serde_json::Value,
    /// Offset to first outgoing edge in edge file
    pub outgoing_offset: FileOffset,
    /// Number of outgoing edges
    pub outgoing_count: u32,
    /// Offset to first incoming edge in edge file
    pub incoming_offset: FileOffset,
    /// Number of incoming edges
    pub incoming_count: u32,
}

impl NodeRecord {
    /// Create a new node record
    pub fn new(id: NativeNodeId, kind: String, name: String, data: serde_json::Value) -> Self {
        Self {
            id,
            flags: NodeFlags::NONE,
            kind,
            name,
            data,
            outgoing_offset: 0,
            outgoing_count: 0,
            incoming_offset: 0,
            incoming_count: 0,
        }
    }

    /// Validate the node record
    pub fn validate(&self, max_node_id: NativeNodeId) -> Result<(), NativeBackendError> {
        if self.id <= 0 || self.id > max_node_id {
            return Err(NativeBackendError::InvalidNodeId {
                id: self.id,
                max_id: max_node_id,
            });
        }

        if self.kind.len() > super::constants::node::MAX_STRING_LENGTH as usize {
            return Err(NativeBackendError::RecordTooLarge {
                size: self.kind.len() as u32,
                max_size: super::constants::node::MAX_STRING_LENGTH as u32,
            });
        }

        if self.name.len() > super::constants::node::MAX_STRING_LENGTH as usize {
            return Err(NativeBackendError::RecordTooLarge {
                size: self.name.len() as u32,
                max_size: super::constants::node::MAX_STRING_LENGTH as u32,
            });
        }

        Ok(())
    }

    /// Get total degree (incoming + outgoing)
    pub fn total_degree(&self) -> u32 {
        self.outgoing_count + self.incoming_count
    }
}

/// Edge record structure for storage
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EdgeRecord {
    /// Unique edge identifier
    pub id: NativeEdgeId,
    /// Source node identifier
    pub from_id: NativeNodeId,
    /// Target node identifier
    pub to_id: NativeNodeId,
    /// Edge type (e.g., "calls", "defines", etc.)
    pub edge_type: String,
    /// Edge flags bitfield
    pub flags: EdgeFlags,
    /// JSON metadata for the edge
    pub data: serde_json::Value,
}

impl EdgeRecord {
    /// Create a new edge record
    pub fn new(
        id: NativeEdgeId,
        from_id: NativeNodeId,
        to_id: NativeNodeId,
        edge_type: String,
        data: serde_json::Value,
    ) -> Self {
        Self {
            id,
            from_id,
            to_id,
            edge_type,
            flags: EdgeFlags::NONE,
            data,
        }
    }

    /// Validate the edge record
    pub fn validate(
        &self,
        max_node_id: NativeNodeId,
        max_edge_id: NativeEdgeId,
    ) -> Result<(), NativeBackendError> {
        if self.id <= 0 || self.id > max_edge_id {
            return Err(NativeBackendError::InvalidEdgeId {
                id: self.id,
                max_id: max_edge_id,
            });
        }

        if self.from_id <= 0 || self.from_id > max_node_id {
            return Err(NativeBackendError::InvalidNodeId {
                id: self.from_id,
                max_id: max_node_id,
            });
        }

        if self.to_id <= 0 || self.to_id > max_node_id {
            return Err(NativeBackendError::InvalidNodeId {
                id: self.to_id,
                max_id: max_node_id,
            });
        }

        if self.edge_type.len() > super::constants::edge::MAX_STRING_LENGTH as usize {
            return Err(NativeBackendError::RecordTooLarge {
                size: self.edge_type.len() as u32,
                max_size: super::constants::edge::MAX_STRING_LENGTH as u32,
            });
        }

        Ok(())
    }
}

/// Comprehensive error type for native backend operations
#[derive(Debug, thiserror::Error)]
pub enum NativeBackendError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid magic number: expected {expected:08x}, found {found:08x}")]
    InvalidMagic { expected: u64, found: u64 },

    #[error("Unsupported version: {version} (supported: 1)")]
    UnsupportedVersion { version: u32 },

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

    #[error("Invalid UTF-8 string: {0}")]
    InvalidUtf8(#[from] std::str::Utf8Error),

    #[error("Buffer too small: {size} bytes (need at least {min_size} bytes)")]
    BufferTooSmall { size: usize, min_size: usize },
}

/// Result type alias for native backend operations
pub type NativeResult<T> = Result<T, NativeBackendError>;
