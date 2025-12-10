//! Constants for native backend file format.
//!
//! This module contains all magic numbers, version constants, and field sizes
//! for the native graph database file format as defined in Phase 1.

/// Magic number for native graph database files
pub const MAGIC_BYTES: [u8; 8] = [b'S', b'Q', b'L', b'T', b'G', b'F', 0, 0];

/// Current file format version
pub const FILE_FORMAT_VERSION: u32 = 1;

/// Header size in bytes (fixed for all files)
pub const HEADER_SIZE: u64 = 64;

/// Header field offsets
pub mod header_offset {
    use super::HEADER_SIZE;
    pub const MAGIC: u64 = 0;
    pub const VERSION: u64 = 8;
    pub const FLAGS: u64 = 12;
    pub const NODE_COUNT: u64 = 16;
    pub const EDGE_COUNT: u64 = 24;
    pub const SCHEMA_VERSION: u64 = 32;
    pub const NODE_DATA_OFFSET: u64 = 40;
    pub const EDGE_DATA_OFFSET: u64 = 48;
    pub const CHECKSUM: u64 = 56;
}

/// Header field sizes
pub mod header_size {
    pub const MAGIC: usize = 8;
    pub const VERSION: usize = 4;
    pub const FLAGS: usize = 4;
    pub const NODE_COUNT: usize = 8;
    pub const EDGE_COUNT: usize = 8;
    pub const SCHEMA_VERSION: usize = 8;
    pub const NODE_DATA_OFFSET: usize = 8;
    pub const EDGE_DATA_OFFSET: usize = 8;
    pub const CHECKSUM: usize = 8;
}

/// Node record constants
pub mod node {
    pub const ID_SIZE: usize = 8;
    pub const FLAGS_SIZE: usize = 4;
    pub const KIND_LEN_SIZE: usize = 2;
    pub const NAME_LEN_SIZE: usize = 2;
    pub const DATA_LEN_SIZE: usize = 4;
    pub const OUTGOING_OFFSET_SIZE: usize = 8;
    pub const OUTGOING_COUNT_SIZE: usize = 4;
    pub const INCOMING_OFFSET_SIZE: usize = 8;
    pub const INCOMING_COUNT_SIZE: usize = 4;

    /// Fixed size of node record header before variable-length fields
    pub const FIXED_HEADER_SIZE: usize =
        1 + ID_SIZE + FLAGS_SIZE + KIND_LEN_SIZE + NAME_LEN_SIZE + DATA_LEN_SIZE;

    /// Size of adjacency metadata after variable-length fields
    pub const ADJACENCY_METADATA_SIZE: usize =
        OUTGOING_OFFSET_SIZE + OUTGOING_COUNT_SIZE + INCOMING_OFFSET_SIZE + INCOMING_COUNT_SIZE;

    /// Maximum allowed string lengths to prevent allocation attacks
    pub const MAX_STRING_LENGTH: u16 = 65535;
    /// Maximum allowed string lengths as u32 for compatibility with error types
    pub const MAX_STRING_LENGTH_U32: u32 = 65535;
    pub const MAX_DATA_LENGTH: u32 = 1_000_000; // 1MB per node max
}

/// Edge record constants
pub mod edge {
    pub const ID_SIZE: usize = 8;
    pub const FROM_ID_SIZE: usize = 8;
    pub const TO_ID_SIZE: usize = 8;
    pub const TYPE_LEN_SIZE: usize = 2;
    pub const FLAGS_SIZE: usize = 2;
    pub const DATA_LEN_SIZE: usize = 4;

    /// Fixed size of edge record header before variable-length fields
    pub const FIXED_HEADER_SIZE: usize =
        1 + ID_SIZE + FROM_ID_SIZE + TO_ID_SIZE + TYPE_LEN_SIZE + FLAGS_SIZE + DATA_LEN_SIZE;

    /// Maximum allowed string lengths to prevent allocation attacks
    pub const MAX_STRING_LENGTH: u16 = 65535;
    /// Maximum allowed string lengths as u32 for compatibility with error types
    pub const MAX_STRING_LENGTH_U32: u32 = 65535;
    pub const MAX_DATA_LENGTH: u32 = 1_000_000; // 1MB per edge max
}

/// Default feature flags (currently none defined)
pub const DEFAULT_FEATURE_FLAGS: u32 = 0;

/// Default schema version
pub const DEFAULT_SCHEMA_VERSION: u64 = 1;

/// Checksum calculation parameters
pub mod checksum {
    /// Simple XOR checksum algorithm for basic integrity checking
    pub const XOR_SEED: u64 = 0x5A5A5A5A5A5A5A5A;
}

/// File permissions for new graph files
pub const FILE_PERMISSIONS: u32 = 0o644;
