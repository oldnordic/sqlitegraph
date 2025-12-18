//! Core type definitions for native backend.
//!
//! This module contains all the core structs, enums, and error types needed
//! for the native graph database file format.

use serde::{Deserialize, Serialize};
use std::fmt;

// Phase 3: V1 Prevention - Top-level compile-time barrier
const _: () = {
    // This will fail if anyone tries to add V1 feature flags
    #[cfg(feature = "v1")]
    compile_error!("V1 FEATURE DETECTED: V1 has been permanently removed. This codebase is V2-ONLY.");

    // This will fail if anyone tries to enable V1 compatibility
    #[cfg(feature = "v1_compatibility")]
    compile_error!("V1 COMPATIBILITY DETECTED: V1 has been permanently removed. Use V2 APIs only.");

    // Intentional guard configs - these will remain noisy as V1 prevention
    #[allow(unexpected_cfgs)]
    #[cfg(feature = "v1")]
    const _: () = compile_error!("V1 FEATURE DETECTED: V1 has been permanently removed. This codebase is V2-ONLY.");

    #[allow(unexpected_cfgs)]
    #[cfg(feature = "v1_compatibility")]
    const _: () = compile_error!("V1 COMPATIBILITY DETECTED: V1 has been permanently removed. Use V2 APIs only.");
};

/// Native node identifier (alias for i64 to match existing NodeId)
pub type NativeNodeId = i64;

/// Native edge identifier (alias for i64 to match existing EdgeId)
pub type NativeEdgeId = i64;

/// File offset within the graph database file
pub type FileOffset = u64;

/// Size of variable-length records
pub type RecordSize = u32;

/// Node flags bitfield for marking node state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
    /// Offset to edge data section or outgoing clusters begin
    pub edge_data_offset: u64,
    /// V2: Offset where outgoing edge clusters begin
    pub outgoing_cluster_offset: u64,
    /// V2: Offset where incoming edge clusters begin
    pub incoming_cluster_offset: u64,
    /// V2: Offset where free space management begins
    pub free_space_offset: u64,
    /// V2 Atomic Commit: Previous outgoing cluster offset for rollback
    pub tx_prev_outgoing_cluster_offset: u64,
    /// V2 Atomic Commit: Previous incoming cluster offset for rollback
    pub tx_prev_incoming_cluster_offset: u64,
    /// V2 Atomic Commit: Previous free space offset for rollback
    pub tx_prev_free_space_offset: u64,
    /// V2 Atomic Commit: Transaction identifier for crash detection
    pub tx_id: u64,
    /// Header checksum
    pub checksum: u64,
}

impl FileHeader {
    /// Create a new header with default values
    pub fn new() -> Self {
        use super::v2::{V2_FORMAT_VERSION, V2_MAGIC};
        Self {
            magic: V2_MAGIC,            // V2 format by default
            version: V2_FORMAT_VERSION, // V2 format by default
            flags: super::constants::DEFAULT_FEATURE_FLAGS,
            node_count: 0,
            edge_count: 0,
            schema_version: super::constants::DEFAULT_SCHEMA_VERSION,
            node_data_offset: super::constants::HEADER_SIZE,
            edge_data_offset: super::constants::HEADER_SIZE,
            outgoing_cluster_offset: 0,
            incoming_cluster_offset: 0,
            free_space_offset: 0,
            tx_prev_outgoing_cluster_offset: 0,
            tx_prev_incoming_cluster_offset: 0,
            tx_prev_free_space_offset: 0,
            tx_id: 0,
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
        if self.version != super::constants::FILE_FORMAT_VERSION && self.version != 2 {
            return Err(NativeBackendError::UnsupportedVersion {
                version: self.version,
                supported_version: super::constants::FILE_FORMAT_VERSION,
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

        // For V2 format, validate additional fields
        if self.version == 2 {
            if self.outgoing_cluster_offset > 0
                && self.outgoing_cluster_offset < self.node_data_offset
            {
                return Err(NativeBackendError::InvalidHeader {
                    field: "outgoing_cluster_offset".to_string(),
                    reason: "must be >= node_data_offset".to_string(),
                });
            }

            if self.incoming_cluster_offset > 0
                && self.incoming_cluster_offset < self.outgoing_cluster_offset
            {
                // HEADER_VALIDATE_DEBUG instrumentation
                if std::env::var("HEADER_VALIDATE_DEBUG").is_ok() {
                    println!(
                        "[HEADER_VALIDATE_DEBUG] FAIL: incoming_cluster_offset ({}) < outgoing_cluster_offset ({})",
                        self.incoming_cluster_offset, self.outgoing_cluster_offset
                    );
                    println!(
                        "[HEADER_VALIDATE_DEBUG] Validation site: {}:{}",
                        file!(),
                        line!()
                    );

                    // Read first 1024 bytes of the file for raw evidence
                    if let Ok(file_path) = std::env::var("HEADER_VALIDATE_DEBUG_FILE") {
                        if let Ok(mut file) = std::fs::File::open(&file_path) {
                            use std::io::Read;
                            let mut buffer = vec![0u8; 1024];
                            if let Ok(_) = file.read_exact(&mut buffer) {
                                println!(
                                    "[HEADER_VALIDATE_DEBUG] First 88 bytes (full header): {:02x?}",
                                    &buffer[..88]
                                );

                                // Extract the offset fields from raw bytes using actual struct layout:
                                // magic: 8 bytes (0-7), version: 4 bytes (8-11), flags: 4 bytes (12-15)
                                // node_count: 8 bytes (16-23), edge_count: 8 bytes (24-31), schema_version: 8 bytes (32-39)
                                // node_data_offset: 8 bytes (40-47), edge_data_offset: 8 bytes (48-55)
                                // outgoing_cluster_offset: 8 bytes (56-63), incoming_cluster_offset: 8 bytes (64-71)
                                let incoming_offset_bytes = &buffer[64..72];
                                let outgoing_offset_bytes = &buffer[56..64];
                                println!(
                                    "[HEADER_VALIDATE_DEBUG] Raw incoming_offset bytes (64-71): {:02x?}",
                                    incoming_offset_bytes
                                );
                                println!(
                                    "[HEADER_VALIDATE_DEBUG] Raw outgoing_offset bytes (56-63): {:02x?}",
                                    outgoing_offset_bytes
                                );

                                // Parse as big-endian u64 for verification
                                let raw_incoming = u64::from_be_bytes([
                                    incoming_offset_bytes[0],
                                    incoming_offset_bytes[1],
                                    incoming_offset_bytes[2],
                                    incoming_offset_bytes[3],
                                    incoming_offset_bytes[4],
                                    incoming_offset_bytes[5],
                                    incoming_offset_bytes[6],
                                    incoming_offset_bytes[7],
                                ]);
                                let raw_outgoing = u64::from_be_bytes([
                                    outgoing_offset_bytes[0],
                                    outgoing_offset_bytes[1],
                                    outgoing_offset_bytes[2],
                                    outgoing_offset_bytes[3],
                                    outgoing_offset_bytes[4],
                                    outgoing_offset_bytes[5],
                                    outgoing_offset_bytes[6],
                                    outgoing_offset_bytes[7],
                                ]);
                                println!(
                                    "[HEADER_VALIDATE_DEBUG] Raw parsed incoming: {}, outgoing: {}",
                                    raw_incoming, raw_outgoing
                                );
                                println!(
                                    "[HEADER_VALIDATE_DEBUG] Comparison result: {} < {} = {}",
                                    raw_incoming,
                                    raw_outgoing,
                                    raw_incoming < raw_outgoing
                                );
                            }
                        }
                    }
                }

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
        checksum ^= self.outgoing_cluster_offset;
        checksum ^= self.incoming_cluster_offset;
        checksum ^= self.free_space_offset;
        checksum ^= self.tx_prev_outgoing_cluster_offset;
        checksum ^= self.tx_prev_incoming_cluster_offset;
        checksum ^= self.tx_prev_free_space_offset;
        checksum ^= self.tx_id;

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

    /// Get transaction state from flags
    pub fn get_tx_state(&self) -> u32 {
        self.flags & super::constants::TX_STATE_MASK
    }

    /// Check if transaction is in progress
    pub fn is_tx_in_progress(&self) -> bool {
        self.get_tx_state() == super::constants::TX_STATE_IN_PROGRESS
    }

    /// Set transaction state
    pub fn set_tx_state(&mut self, state: u32) {
        self.flags = (self.flags & !super::constants::TX_STATE_MASK)
            | (state & super::constants::TX_STATE_MASK);
    }

    /// Begin transaction: save current state and set IN_PROGRESS
    pub fn begin_tx(&mut self, next_tx_id: u64) {
        self.tx_prev_outgoing_cluster_offset = self.outgoing_cluster_offset;
        self.tx_prev_incoming_cluster_offset = self.incoming_cluster_offset;
        self.tx_prev_free_space_offset = self.free_space_offset;
        self.tx_id = next_tx_id;
        self.set_tx_state(super::constants::TX_STATE_IN_PROGRESS);
    }

    /// Commit transaction: clear transaction state
    pub fn commit_tx(&mut self) {
        self.set_tx_state(super::constants::TX_STATE_CLEAN);
    }

    /// Rollback transaction: restore previous offsets
    pub fn rollback_tx(&mut self) {
        self.outgoing_cluster_offset = self.tx_prev_outgoing_cluster_offset;
        self.incoming_cluster_offset = self.tx_prev_incoming_cluster_offset;
        self.free_space_offset = self.tx_prev_free_space_offset;
        self.set_tx_state(super::constants::TX_STATE_CLEAN);
    }
}

/// V2-only node record type alias for backward compatibility
pub type NodeRecord = crate::backend::native::v2::node_record_v2::NodeRecordV2;

/// Edge record structure for API compatibility (V1-style fields for operations)
/// This is converted to CompactEdgeRecord for V2 storage
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

/// CPU Profile for performance optimizations
///
/// This enum allows application developers to choose CPU-specific optimizations
/// while maintaining backwards compatibility. All profiles are safe and will
/// gracefully degrade on unsupported hardware.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum CpuProfile {
    /// Generic profile compatible with all CPUs
    /// Uses portable optimizations without CPU-specific instructions
    Generic,
    /// Auto-detect and use optimal profile
    /// Runtime detection selects the best available profile
    Auto,
    /// Optimized for AMD Zen 4 (Ryzen 7000 series)
    /// Target: AMD Ryzen 7 7800X3D with AVX2, FMA, BMI2
    X86Zen4,
    /// Optimized for Intel CPUs with AVX2 support
    /// Target: Intel Skylake+ with 256-bit vector instructions
    X86Avx2,
    /// Optimized for Intel CPUs with AVX-512 support
    /// Target: Intel Xeon/Server with 512-bit vector instructions
    X86Avx512,
}

impl Default for CpuProfile {
    fn default() -> Self {
        Self::Generic
    }
}

impl std::fmt::Display for CpuProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Generic => write!(f, "generic"),
            Self::Auto => write!(f, "auto"),
            Self::X86Zen4 => write!(f, "x86-zen4"),
            Self::X86Avx2 => write!(f, "x86-avx2"),
            Self::X86Avx512 => write!(f, "x86-avx512"),
        }
    }
}

impl std::str::FromStr for CpuProfile {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "generic" => Ok(Self::Generic),
            "auto" => Ok(Self::Auto),
            "x86-zen4" | "zen4" | "znver4" => Ok(Self::X86Zen4),
            "x86-avx2" | "avx2" => Ok(Self::X86Avx2),
            "x86-avx512" | "avx512" => Ok(Self::X86Avx512),
            _ => Err(format!(
                "Invalid CPU profile '{}'. Valid options: generic, auto, x86-zen4, x86-avx2, x86-avx512",
                s
            )),
        }
    }
}

/// Calculate the file offset for a given node's slot
///
/// This helper centralizes the node slot offset calculation to prevent duplication
/// and ensure consistency across the codebase.
///
/// # Arguments
/// * `node_data_offset` - The base offset where node data begins in the file
/// * `node_id` - The 1-based node identifier
///
/// # Returns
/// The file offset where this node's slot begins
///
/// # Note
/// Node IDs are 1-based, so we subtract 1 to get the correct slot index.
/// Each node slot is 4KB (4096 bytes) in size.
#[inline]
pub fn node_slot_offset(node_data_offset: u64, node_id: NativeNodeId) -> u64 {
    debug_assert!(node_id > 0, "Node IDs must be positive (1-based)");
    node_data_offset + ((node_id - 1) as u64 * super::constants::node::NODE_SLOT_SIZE)
}

/// Result type alias for native backend operations
pub type NativeResult<T> = Result<T, NativeBackendError>;
