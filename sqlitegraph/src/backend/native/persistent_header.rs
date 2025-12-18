//! Persistent header for V2 native backend
//!
//! This module defines the stable, on-disk header format that is persisted to disk.
//! All transaction-related state is handled separately at runtime.

use crate::backend::native::NativeBackendError;
use crate::backend::native::types::NativeResult;

/// V2 Persistent header that is written to disk
///
/// This header contains ONLY the fields that must be persisted across file closes.
/// Transaction state and rollback metadata are handled separately in runtime.
#[derive(Debug, Clone)]
pub struct PersistentHeaderV2 {
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
    /// Offset to edge data section (V1) or outgoing clusters begin (V2)
    pub edge_data_offset: u64,
    /// V2: Offset where outgoing edge clusters begin
    pub outgoing_cluster_offset: u64,
    /// V2: Offset where incoming edge clusters begin
    pub incoming_cluster_offset: u64,
    /// V2: Offset where free space management begins
    pub free_space_offset: u64,
}

/// Byte offset specifications for PersistentHeaderV2
pub mod offset {
    pub const MAGIC: usize = 0; // bytes 0-7
    pub const VERSION: usize = 8; // bytes 8-11
    pub const FLAGS: usize = 12; // bytes 12-15
    pub const NODE_COUNT: usize = 16; // bytes 16-23
    pub const EDGE_COUNT: usize = 24; // bytes 24-31
    pub const SCHEMA_VERSION: usize = 32; // bytes 32-39
    pub const NODE_DATA_OFFSET: usize = 40; // bytes 40-47
    pub const EDGE_DATA_OFFSET: usize = 48; // bytes 48-55
    pub const OUTGOING_CLUSTER_OFFSET: usize = 56; // bytes 56-63
    pub const INCOMING_CLUSTER_OFFSET: usize = 64; // bytes 64-71
    pub const FREE_SPACE_OFFSET: usize = 72; // bytes 72-79
}

/// Size specifications for PersistentHeaderV2
pub mod size {
    pub const MAGIC: usize = 8;
    pub const VERSION: usize = 4;
    pub const FLAGS: usize = 4;
    pub const NODE_COUNT: usize = 8;
    pub const EDGE_COUNT: usize = 8;
    pub const SCHEMA_VERSION: usize = 8;
    pub const NODE_DATA_OFFSET: usize = 8;
    pub const EDGE_DATA_OFFSET: usize = 8;
    pub const OUTGOING_CLUSTER_OFFSET: usize = 8;
    pub const INCOMING_CLUSTER_OFFSET: usize = 8;
    pub const FREE_SPACE_OFFSET: usize = 8;
}

impl PersistentHeaderV2 {
    /// Create a new persistent header with default V2 values
    pub fn new_v2() -> Self {
        use crate::backend::native::v2::{V2_FORMAT_VERSION, V2_MAGIC};
        Self {
            magic: V2_MAGIC,
            version: V2_FORMAT_VERSION,
            flags: crate::backend::native::constants::DEFAULT_FEATURE_FLAGS,
            node_count: 0,
            edge_count: 0,
            schema_version: crate::backend::native::constants::DEFAULT_SCHEMA_VERSION,
            node_data_offset: crate::backend::native::constants::HEADER_SIZE,
            edge_data_offset: crate::backend::native::constants::HEADER_SIZE,
            outgoing_cluster_offset: 0,
            incoming_cluster_offset: 0,
            free_space_offset: 0,
        }
    }

    /// Validate persistent header for consistency
    pub fn validate(&self) -> NativeResult<()> {
        use crate::backend::native::constants;

        // Check magic number
        if self.magic != constants::MAGIC_BYTES {
            return Err(NativeBackendError::InvalidMagic {
                expected: u64::from_be_bytes(constants::MAGIC_BYTES),
                found: u64::from_be_bytes(self.magic),
            });
        }

        // Check version
        if self.version != constants::FILE_FORMAT_VERSION {
            return Err(NativeBackendError::UnsupportedVersion {
                version: self.version,
                supported_version: constants::FILE_FORMAT_VERSION,
            });
        }

        // Check V2 flags
        let required_flags = constants::FLAG_V2_FRAMED_RECORDS | constants::FLAG_V2_ATOMIC_COMMIT;
        if (self.flags & required_flags) != required_flags {
            return Err(NativeBackendError::UnsupportedVersion {
                version: 1, // Unsupported version
                supported_version: constants::FILE_FORMAT_VERSION,
            });
        }

        // Check offset ordering
        if self.node_data_offset < constants::HEADER_SIZE {
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

        // Validate cluster offset ordering for V2
        if self.outgoing_cluster_offset > 0 && self.outgoing_cluster_offset < self.node_data_offset
        {
            return Err(NativeBackendError::InvalidHeader {
                field: "outgoing_cluster_offset".to_string(),
                reason: "must be >= node_data_offset".to_string(),
            });
        }

        if self.incoming_cluster_offset > 0
            && self.incoming_cluster_offset < self.outgoing_cluster_offset
        {
            return Err(NativeBackendError::InvalidHeader {
                field: "incoming_cluster_offset".to_string(),
                reason: "must be >= outgoing_cluster_offset".to_string(),
            });
        }

        if self.free_space_offset > 0 && self.free_space_offset < self.incoming_cluster_offset {
            return Err(NativeBackendError::InvalidHeader {
                field: "free_space_offset".to_string(),
                reason: "must be >= incoming_cluster_offset".to_string(),
            });
        }

        Ok(())
    }
}

/// Size of PersistentHeaderV2 - should match constants::HEADER_SIZE
pub const PERSISTENT_HEADER_SIZE: usize = size::MAGIC
    + size::VERSION
    + size::FLAGS
    + size::NODE_COUNT
    + size::EDGE_COUNT
    + size::SCHEMA_VERSION
    + size::NODE_DATA_OFFSET
    + size::EDGE_DATA_OFFSET
    + size::OUTGOING_CLUSTER_OFFSET
    + size::INCOMING_CLUSTER_OFFSET
    + size::FREE_SPACE_OFFSET;

// Compile-time assertion - ensure persistent header matches global HEADER_SIZE
const _: [(); 80] = [(); PERSISTENT_HEADER_SIZE]; // Size must be exactly 80 bytes
