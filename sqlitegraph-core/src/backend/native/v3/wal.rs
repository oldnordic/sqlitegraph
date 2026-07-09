//! V3 Write-Ahead Logging (WAL) for B+Tree page operations.
//!
//! This module implements WAL for V3's page-based storage, providing:
//! - Atomic page operations (allocate, free, write)
//! - B+Tree split tracking
//! - Crash recovery from WAL
//! - Checkpoint integration with BTreeManager
//!
//! # Architecture
//!
//! V3 WAL is designed for page-level operations rather than node/edge operations:
//! - **Page Allocation**: Track new page assignments
//! - **Page Freeing**: Track deallocation for reuse
//! - **Page Writes**: Track data modifications with checksums
//! - **B+Tree Splits**: Track tree restructuring during growth
//! - **Checkpoints**: Persist root page and tree state
//!
//! # WAL File Format
//!
//! ```text
//! [V3WALHeader - 64 bytes]
//!   magic: [u8; 8]       // "V3WAL\0\0"
//!   version: u32            // WAL format version
//!   page_size: u32          // Page size (usually 4096)
//!   created_at: u64         // Creation timestamp
//!   current_lsn: u64        // Current log sequence number
//!   committed_lsn: u64       // Last committed LSN
//!   checkpointed_lsn: u64    // Last checkpointed LSN
//!   reserved: [u64; 3]     // Future use
//!
//! [V3WALRecord 1]
//! [V3WALRecord 2]
//! ...
//! ```
//!
//! # Recovery Process
//!
//! 1. Open WAL file and read header
//! 2. Sequential read and apply records:
//!    - PageAllocate: Allocate page via PageAllocator
//!    - PageFree: Free page via PageAllocator
//!    - PageWrite: Write data to page, verify checksum
//!    - BTreeSplit: Update B+Tree structure
//!    - Checkpoint: Persist header state (root page, height, etc.)
//! 3. Skip corrupt/invalid records
//! 4. After replay, truncate WAL at checkpoint point

use crate::backend::native::NativeBackendError;
use crate::backend::native::NativeResult;
use crate::backend::native::v3::constants::checksum;
use crate::backend::native::v3::header::PersistentHeaderV3;
use bincode;
use serde::{Deserialize, Serialize};

mod checkpoint;
mod recovery;
#[cfg(test)]
mod tests;
mod writer;

pub use checkpoint::{read_kv_checkpoint, write_kv_checkpoint};
pub use recovery::{WALRecovery, WALRecoveryStats};
pub use writer::{V3WALPaths, WALWriter};

/// V3 WAL file magic bytes
pub const V3_WAL_MAGIC: [u8; 8] = [b'V', b'3', b'W', b'A', b'L', 0, 0, 0];

/// V3 WAL format version
pub const V3_WAL_VERSION: u32 = 1;

/// V3 WAL header size in bytes
pub const V3_WAL_HEADER_SIZE: usize = 64;

/// Maximum WAL record size (1MB - safety limit)
pub const MAX_RECORD_SIZE: usize = 1024 * 1024;

/// Log Sequence Number (LSN) representing beginning of WAL
pub const LSN_BEGIN: u64 = 1;

/// Log Sequence Number (LSN) representing invalid/uninitialized position
pub const LSN_INVALID: u64 = 0;

/// Check if an LSN is valid
#[inline]
pub fn lsn_is_valid(lsn: u64) -> bool {
    lsn >= LSN_BEGIN
}

/// Get the next LSN
#[inline]
pub fn lsn_next(lsn: u64) -> u64 {
    lsn.wrapping_add(1)
}

/// V3 WAL file header
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct V3WALHeader {
    /// Magic bytes for identification
    pub magic: [u8; 8],
    /// WAL format version
    pub version: u32,
    /// Page size in bytes (usually 4096)
    pub page_size: u32,
    /// Creation timestamp (Unix epoch)
    pub created_at: u64,
    /// Current log sequence number
    pub current_lsn: u64,
    /// Last committed LSN
    pub committed_lsn: u64,
    /// Last checkpointed LSN
    pub checkpointed_lsn: u64,
    /// Reserved for future use
    pub reserved: [u64; 3],
}

impl Default for V3WALHeader {
    fn default() -> Self {
        Self::new()
    }
}

impl V3WALHeader {
    /// Create a new WAL header with defaults
    pub fn new() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            magic: V3_WAL_MAGIC,
            version: V3_WAL_VERSION,
            page_size: 4096,
            created_at: now,
            current_lsn: LSN_BEGIN,
            committed_lsn: LSN_INVALID,
            checkpointed_lsn: LSN_INVALID,
            reserved: [0; 3],
        }
    }

    /// Validate the WAL header
    pub fn validate(&self) -> NativeResult<()> {
        if self.magic != V3_WAL_MAGIC {
            return Err(NativeBackendError::InvalidHeader {
                field: "magic".to_string(),
                reason: format!("expected {:?}, found {:?}", V3_WAL_MAGIC, self.magic),
            });
        }

        if self.version != V3_WAL_VERSION {
            return Err(NativeBackendError::UnsupportedVersion {
                version: self.version,
                supported_version: V3_WAL_VERSION,
            });
        }

        if self.page_size != 4096 && self.page_size != 8192 && self.page_size != 16384 {
            return Err(NativeBackendError::InvalidHeader {
                field: "page_size".to_string(),
                reason: "must be 4096, 8192, or 16384".to_string(),
            });
        }

        if !lsn_is_valid(self.current_lsn) {
            return Err(NativeBackendError::InvalidHeader {
                field: "current_lsn".to_string(),
                reason: "must be >= LSN_BEGIN".to_string(),
            });
        }

        if self.committed_lsn > self.current_lsn {
            return Err(NativeBackendError::InvalidHeader {
                field: "committed_lsn".to_string(),
                reason: "cannot be greater than current_lsn".to_string(),
            });
        }

        if self.checkpointed_lsn > self.committed_lsn {
            return Err(NativeBackendError::InvalidHeader {
                field: "checkpointed_lsn".to_string(),
                reason: "cannot be greater than committed_lsn".to_string(),
            });
        }

        Ok(())
    }

    /// Serialize header to bytes
    pub fn to_bytes(&self) -> [u8; V3_WAL_HEADER_SIZE] {
        let mut bytes = [0u8; V3_WAL_HEADER_SIZE];

        bytes[0..8].copy_from_slice(&self.magic);
        bytes[8..12].copy_from_slice(&self.version.to_le_bytes());
        bytes[12..16].copy_from_slice(&self.page_size.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.created_at.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.current_lsn.to_le_bytes());
        bytes[32..40].copy_from_slice(&self.committed_lsn.to_le_bytes());
        bytes[40..48].copy_from_slice(&self.checkpointed_lsn.to_le_bytes());
        // reserved[0] at 48..56
        // reserved[1] at 56..64
        bytes[48..56].copy_from_slice(&self.reserved[0].to_le_bytes());
        bytes[56..64].copy_from_slice(&self.reserved[1].to_le_bytes());

        bytes
    }

    /// Deserialize header from bytes
    pub fn from_bytes(bytes: &[u8]) -> NativeResult<Self> {
        if bytes.len() < V3_WAL_HEADER_SIZE {
            return Err(NativeBackendError::InvalidHeader {
                field: "bytes".to_string(),
                reason: format!(
                    "expected {} bytes, found {}",
                    V3_WAL_HEADER_SIZE,
                    bytes.len()
                ),
            });
        }

        let mut magic = [0u8; 8];
        magic.copy_from_slice(&bytes[0..8]);

        // Helper to safely convert byte slices to fixed-size arrays
        let extract_u32 = |offset: usize| -> NativeResult<u32> {
            let slice =
                bytes
                    .get(offset..offset + 4)
                    .ok_or_else(|| NativeBackendError::InvalidHeader {
                        field: format!("offset_{}", offset),
                        reason: format!("expected 4 bytes at offset {}", offset),
                    })?;
            let arr: [u8; 4] = slice
                .try_into()
                .map_err(|_| NativeBackendError::InvalidHeader {
                    field: format!("offset_{}", offset),
                    reason: "failed to convert to u32 array".to_string(),
                })?;
            Ok(u32::from_le_bytes(arr))
        };

        let extract_u64 = |offset: usize| -> NativeResult<u64> {
            let slice =
                bytes
                    .get(offset..offset + 8)
                    .ok_or_else(|| NativeBackendError::InvalidHeader {
                        field: format!("offset_{}", offset),
                        reason: format!("expected 8 bytes at offset {}", offset),
                    })?;
            let arr: [u8; 8] = slice
                .try_into()
                .map_err(|_| NativeBackendError::InvalidHeader {
                    field: format!("offset_{}", offset),
                    reason: "failed to convert to u64 array".to_string(),
                })?;
            Ok(u64::from_le_bytes(arr))
        };

        let version = extract_u32(8)?;
        let page_size = extract_u32(12)?;
        let created_at = extract_u64(16)?;
        let current_lsn = extract_u64(24)?;
        let committed_lsn = extract_u64(32)?;
        let checkpointed_lsn = extract_u64(40)?;
        let reserved0 = extract_u64(48)?;
        let reserved1 = extract_u64(56)?;

        Ok(Self {
            magic,
            version,
            page_size,
            created_at,
            current_lsn,
            committed_lsn,
            checkpointed_lsn,
            reserved: [reserved0, reserved1, 0],
        })
    }
}

/// V3 WAL record types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum V3WALRecordType {
    /// Page allocation - new page assigned from PageAllocator
    PageAllocate = 1,

    /// Page deallocation - page returned to free list
    PageFree = 2,

    /// Page write - data written to page with checksum
    PageWrite = 3,

    /// B+Tree split - page split during growth
    BTreeSplit = 4,

    /// Checkpoint - persist tree root and header state
    Checkpoint = 5,

    /// Transaction begin marker
    TransactionBegin = 6,

    /// Transaction commit marker
    TransactionCommit = 7,

    /// Transaction rollback marker
    TransactionRollback = 8,

    /// KV Set - store a key-value pair
    KvSet = 9,

    /// KV Delete - delete a key-value pair
    KvDelete = 10,

    /// KV Tombstone - mark key as deleted (for MVCC)
    KvTombstone = 11,

    /// Edge Insert - insert edge into edge cluster
    EdgeInsert = 12,
}

impl TryFrom<u8> for V3WALRecordType {
    type Error = NativeBackendError;

    fn try_from(value: u8) -> NativeResult<Self> {
        match value {
            1 => Ok(Self::PageAllocate),
            2 => Ok(Self::PageFree),
            3 => Ok(Self::PageWrite),
            4 => Ok(Self::BTreeSplit),
            5 => Ok(Self::Checkpoint),
            6 => Ok(Self::TransactionBegin),
            7 => Ok(Self::TransactionCommit),
            8 => Ok(Self::TransactionRollback),
            9 => Ok(Self::KvSet),
            10 => Ok(Self::KvDelete),
            11 => Ok(Self::KvTombstone),
            12 => Ok(Self::EdgeInsert),
            _ => Err(NativeBackendError::InvalidHeader {
                field: "record_type".to_string(),
                reason: format!("unknown record type: {}", value),
            }),
        }
    }
}

/// V3 WAL record for page-level operations
///
/// Each record represents a single operation that modifies the database state.
/// Records are written sequentially to the WAL file and can be replayed
/// during recovery to restore database state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum V3WALRecord {
    /// Page allocation - assign new page from PageAllocator
    PageAllocate {
        /// Log sequence number
        lsn: u64,
        /// Newly allocated page ID
        page_id: u64,
        /// Timestamp of allocation
        timestamp: u64,
    },

    /// Page deallocation - return page to free list
    PageFree {
        /// Log sequence number
        lsn: u64,
        /// Page ID being freed
        page_id: u64,
        /// Checksum of page before free (for validation)
        checksum: u32,
        /// Timestamp of deallocation
        timestamp: u64,
    },

    /// Page write - write data to page
    PageWrite {
        /// Log sequence number
        lsn: u64,
        /// Target page ID
        page_id: u64,
        /// Offset within page (0-4095)
        offset: u32,
        /// Data being written
        data: Vec<u8>,
        /// Checksum of data
        checksum: u32,
        /// Timestamp of write
        timestamp: u64,
    },

    /// B+Tree page split
    BTreeSplit {
        /// Log sequence number
        lsn: u64,
        /// Original page ID being split
        original_page_id: u64,
        /// New page ID created from split
        new_page_id: u64,
        /// Split key (first key in new page)
        split_key: u64,
        /// Page type being split (internal or leaf)
        page_type: u8, // 0 = internal, 1 = leaf
        /// Timestamp of split
        timestamp: u64,
    },

    /// Checkpoint - persist database state
    Checkpoint {
        /// Log sequence number
        lsn: u64,
        /// Root B+Tree page ID
        root_page_id: u64,
        /// Total pages in database
        total_pages: u64,
        /// B+Tree height
        btree_height: u32,
        /// Free page list head
        free_page_list_head: u64,
        /// Full header snapshot for recovery
        header_snapshot: Vec<u8>, // Serialized PersistentHeaderV3
        /// Timestamp of checkpoint
        timestamp: u64,
    },

    /// Transaction begin marker
    TransactionBegin {
        /// Transaction ID
        tx_id: u64,
        /// Log sequence number
        lsn: u64,
        /// Timestamp
        timestamp: u64,
    },

    /// Transaction commit marker
    TransactionCommit {
        /// Transaction ID
        tx_id: u64,
        /// Log sequence number
        lsn: u64,
        /// Timestamp
        timestamp: u64,
    },

    /// Transaction rollback marker
    TransactionRollback {
        /// Transaction ID
        tx_id: u64,
        /// Log sequence number
        lsn: u64,
        /// Timestamp
        timestamp: u64,
    },

    /// KV Set operation
    KvSet {
        /// Log sequence number
        lsn: u64,
        /// Key bytes
        key: Vec<u8>,
        /// Value bytes (serialized)
        value_bytes: Vec<u8>,
        /// Value type tag
        value_type: u8,
        /// Optional TTL in seconds
        ttl_seconds: Option<u64>,
        /// Timestamp
        timestamp: u64,
    },

    /// KV Delete operation
    KvDelete {
        /// Log sequence number
        lsn: u64,
        /// Key bytes
        key: Vec<u8>,
        /// Timestamp
        timestamp: u64,
    },

    /// KV Tombstone for MVCC
    KvTombstone {
        /// Log sequence number
        lsn: u64,
        /// Key bytes
        key: Vec<u8>,
        /// Previous value (for rollback)
        old_value_bytes: Option<Vec<u8>>,
        /// Previous value type
        old_value_type: u8,
        /// Timestamp
        timestamp: u64,
    },

    /// Edge Insert - insert edge into edge cluster
    EdgeInsert {
        /// Log sequence number
        lsn: u64,
        /// Source node ID
        src: i64,
        /// Destination node ID
        dst: i64,
        /// Edge direction (0 = Outgoing, 1 = Incoming)
        direction: u8,
        /// Page ID where edge cluster will be stored
        page_id: u64,
        /// Timestamp
        timestamp: u64,
    },
}

impl V3WALRecord {
    /// Get the record type
    pub fn record_type(&self) -> V3WALRecordType {
        match self {
            Self::PageAllocate { .. } => V3WALRecordType::PageAllocate,
            Self::PageFree { .. } => V3WALRecordType::PageFree,
            Self::PageWrite { .. } => V3WALRecordType::PageWrite,
            Self::BTreeSplit { .. } => V3WALRecordType::BTreeSplit,
            Self::Checkpoint { .. } => V3WALRecordType::Checkpoint,
            Self::TransactionBegin { .. } => V3WALRecordType::TransactionBegin,
            Self::TransactionCommit { .. } => V3WALRecordType::TransactionCommit,
            Self::TransactionRollback { .. } => V3WALRecordType::TransactionRollback,
            Self::KvSet { .. } => V3WALRecordType::KvSet,
            Self::KvDelete { .. } => V3WALRecordType::KvDelete,
            Self::KvTombstone { .. } => V3WALRecordType::KvTombstone,
            Self::EdgeInsert { .. } => V3WALRecordType::EdgeInsert,
        }
    }

    /// Get the LSN for this record
    pub fn lsn(&self) -> u64 {
        match self {
            Self::PageAllocate { lsn, .. } => *lsn,
            Self::PageFree { lsn, .. } => *lsn,
            Self::PageWrite { lsn, .. } => *lsn,
            Self::BTreeSplit { lsn, .. } => *lsn,
            Self::Checkpoint { lsn, .. } => *lsn,
            Self::TransactionBegin { lsn, .. } => *lsn,
            Self::TransactionCommit { lsn, .. } => *lsn,
            Self::TransactionRollback { lsn, .. } => *lsn,
            Self::KvSet { lsn, .. } => *lsn,
            Self::KvDelete { lsn, .. } => *lsn,
            Self::KvTombstone { lsn, .. } => *lsn,
            Self::EdgeInsert { lsn, .. } => *lsn,
        }
    }

    /// Check if this record modifies page data (requires checkpoint)
    pub fn is_data_modifying(&self) -> bool {
        matches!(
            self,
            Self::PageAllocate { .. }
                | Self::PageFree { .. }
                | Self::PageWrite { .. }
                | Self::BTreeSplit { .. }
        )
    }

    /// Check if this is a transaction control record
    pub fn is_transaction_control(&self) -> bool {
        matches!(
            self,
            Self::TransactionBegin { .. }
                | Self::TransactionCommit { .. }
                | Self::TransactionRollback { .. }
        )
    }

    /// Check if this is a checkpoint record
    pub fn is_checkpoint(&self) -> bool {
        matches!(self, Self::Checkpoint { .. })
    }

    /// Serialize record to bytes using bincode
    pub fn to_bytes(&self) -> NativeResult<Vec<u8>> {
        let bytes: Result<Vec<u8>, _> = bincode::serialize(self);
        bytes
            .map_err(NativeBackendError::BincodeError)
            .and_then(|bytes: Vec<u8>| {
                if bytes.len() > MAX_RECORD_SIZE {
                    Err(NativeBackendError::RecordTooLarge {
                        size: bytes.len() as u32,
                        max_size: MAX_RECORD_SIZE as u32,
                    })
                } else {
                    Ok(bytes)
                }
            })
    }

    /// Deserialize record from bytes using bincode
    pub fn from_bytes(bytes: &[u8]) -> NativeResult<Self> {
        bincode::deserialize(bytes).map_err(NativeBackendError::BincodeError)
    }

    /// Calculate checksum for the serialized record
    pub fn calculate_checksum(&self) -> u64 {
        let bytes = match self.to_bytes() {
            Ok(b) => b,
            Err(_) => return 0, // Should not happen for valid records
        };
        checksum::xor_checksum(&bytes)
    }

    /// Create a PageAllocate record
    pub fn page_allocate(page_id: u64, lsn: u64) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self::PageAllocate {
            lsn,
            page_id,
            timestamp,
        }
    }

    /// Create a PageFree record
    pub fn page_free(page_id: u64, checksum: u32, lsn: u64) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self::PageFree {
            lsn,
            page_id,
            checksum,
            timestamp,
        }
    }

    /// Create a PageWrite record
    pub fn page_write(page_id: u64, offset: u32, data: Vec<u8>, lsn: u64) -> Self {
        let checksum = checksum::xor_checksum(&data) as u32;
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self::PageWrite {
            lsn,
            page_id,
            offset,
            data,
            checksum,
            timestamp,
        }
    }

    /// Create a BTreeSplit record
    pub fn btree_split(
        original_page_id: u64,
        new_page_id: u64,
        split_key: u64,
        is_leaf: bool,
        lsn: u64,
    ) -> Self {
        let page_type = if is_leaf { 1 } else { 0 };
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self::BTreeSplit {
            lsn,
            original_page_id,
            new_page_id,
            split_key,
            page_type,
            timestamp,
        }
    }

    /// Create a Checkpoint record
    pub fn checkpoint(
        root_page_id: u64,
        total_pages: u64,
        btree_height: u32,
        free_page_list_head: u64,
        header: &PersistentHeaderV3,
        lsn: u64,
    ) -> Self {
        let header_snapshot = header.to_bytes().to_vec();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self::Checkpoint {
            lsn,
            root_page_id,
            total_pages,
            btree_height,
            free_page_list_head,
            header_snapshot,
            timestamp,
        }
    }

    /// Create an EdgeInsert record
    pub fn edge_insert(src: i64, dst: i64, direction: u8, page_id: u64, lsn: u64) -> Self {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self::EdgeInsert {
            lsn,
            src,
            dst,
            direction,
            page_id,
            timestamp,
        }
    }
}
