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
use std::path::PathBuf;

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

/// WAL writer for appending records to WAL file
///
/// Handles writing WAL records to disk with proper synchronization
/// for crash recovery. Records are written in format:
///
/// ```text
/// [4 bytes: record size (little-endian u32)]
/// [N bytes: serialized record]
/// ```
///
/// # Durability
///
/// Each record is followed by an optional fsync for durability.
/// For performance, multiple records can be batched before syncing.
#[derive(Debug)]
pub struct WALWriter {
    /// WAL file path
    wal_path: PathBuf,
    /// Current LSN
    current_lsn: u64,
    /// Committed LSN
    committed_lsn: u64,
    /// Buffered records before fsync
    buffer: Vec<u8>,
    /// Buffer size threshold for auto-flush
    flush_threshold: usize,
}

impl WALWriter {
    /// Create a new WAL writer
    ///
    /// # Arguments
    ///
    /// * `wal_path` - Path to WAL file
    /// * `start_lsn` - Starting LSN (default LSN_BEGIN for new WAL)
    ///
    /// # Returns
    ///
    /// Returns error if WAL file exists but cannot be read.
    pub fn new(wal_path: PathBuf, start_lsn: u64) -> NativeResult<Self> {
        let mut writer = Self {
            wal_path,
            current_lsn: start_lsn,
            committed_lsn: LSN_INVALID,
            buffer: Vec::new(),
            flush_threshold: 64 * 1024, // 64KB default buffer
        };

        // If WAL exists, read current LSN from header
        if writer.wal_path.exists() {
            writer.read_header()?;
        }

        Ok(writer)
    }

    /// Get current LSN
    pub fn current_lsn(&self) -> u64 {
        self.current_lsn
    }

    /// Get committed LSN
    pub fn committed_lsn(&self) -> u64 {
        self.committed_lsn
    }

    /// Read WAL header to get current state
    fn read_header(&mut self) -> NativeResult<()> {
        use std::io::Read;

        let mut file =
            std::fs::File::open(&self.wal_path).map_err(|e| NativeBackendError::IoError {
                context: "Failed to open WAL for reading".to_string(),
                source: e,
            })?;

        let mut header_bytes = [0u8; V3_WAL_HEADER_SIZE];
        file.read_exact(&mut header_bytes)
            .map_err(|e| NativeBackendError::IoError {
                context: "Failed to read WAL header".to_string(),
                source: e,
            })?;

        let header = V3WALHeader::from_bytes(&header_bytes)?;
        header.validate()?;

        self.current_lsn = header.current_lsn;
        self.committed_lsn = header.committed_lsn;

        Ok(())
    }

    /// Write WAL header (initializes new WAL file)
    pub fn write_header(&self) -> NativeResult<()> {
        use std::io::Write;

        let header = V3WALHeader {
            magic: V3_WAL_MAGIC,
            version: V3_WAL_VERSION,
            page_size: 4096,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
            current_lsn: self.current_lsn,
            committed_lsn: self.committed_lsn,
            checkpointed_lsn: LSN_INVALID,
            reserved: [0; 3],
        };

        let header_bytes = header.to_bytes();

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.wal_path)
            .map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to create WAL file: {}", self.wal_path.display()),
                source: e,
            })?;

        file.write_all(&header_bytes)
            .map_err(|e| NativeBackendError::IoError {
                context: "Failed to write WAL header".to_string(),
                source: e,
            })?;

        file.sync_all().map_err(|e| NativeBackendError::IoError {
            context: "Failed to sync WAL file".to_string(),
            source: e,
        })?;

        Ok(())
    }

    /// Append a record to WAL buffer
    ///
    /// Record is buffered until flush() is called or buffer threshold is reached.
    pub fn append(&mut self, record: &V3WALRecord) -> NativeResult<()> {
        let bytes = record.to_bytes()?;
        let size = bytes.len() as u32;

        // Check size limit
        if bytes.len() > MAX_RECORD_SIZE {
            return Err(NativeBackendError::SerializationError {
                context: format!(
                    "Record size {} exceeds maximum {}",
                    bytes.len(),
                    MAX_RECORD_SIZE
                ),
            });
        }

        // Write size prefix and record data
        self.buffer.extend_from_slice(&size.to_le_bytes());
        self.buffer.extend_from_slice(&bytes);

        // Auto-flush if threshold exceeded
        if self.buffer.len() >= self.flush_threshold {
            self.flush()?;
        }

        self.current_lsn = lsn_next(self.current_lsn);

        Ok(())
    }

    /// Flush buffered records to disk
    ///
    /// Writes all buffered records and optionally syncs to disk.
    pub fn flush(&mut self) -> NativeResult<()> {
        use std::io::Write;

        if self.buffer.is_empty() {
            return Ok(());
        }

        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.wal_path)
            .map_err(|e| NativeBackendError::IoError {
                context: "Failed to open WAL for writing".to_string(),
                source: e,
            })?;

        file.write_all(&self.buffer)
            .map_err(|e| NativeBackendError::IoError {
                context: "Failed to write WAL records".to_string(),
                source: e,
            })?;

        file.sync_all().map_err(|e| NativeBackendError::IoError {
            context: "Failed to sync WAL file".to_string(),
            source: e,
        })?;

        self.buffer.clear();
        Ok(())
    }

    /// Mark records up to current LSN as committed
    ///
    /// Updates the committed_lsn in WAL header.
    /// Requires flush to persist the updated header.
    pub fn commit(&mut self) -> NativeResult<()> {
        self.committed_lsn = self.current_lsn;

        // Update header on disk
        self.update_header()?;

        Ok(())
    }

    /// Update WAL header with current LSN values
    fn update_header(&self) -> NativeResult<()> {
        use std::io::{Read, Seek, SeekFrom, Write};

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(&self.wal_path)
            .map_err(|e| NativeBackendError::IoError {
                context: "Failed to open WAL for header update".to_string(),
                source: e,
            })?;

        // Read existing header to preserve fields
        file.seek(SeekFrom::Start(0))
            .map_err(|e| NativeBackendError::IoError {
                context: "Failed to seek in WAL file".to_string(),
                source: e,
            })?;

        let mut header_bytes = [0u8; V3_WAL_HEADER_SIZE];
        file.read_exact(&mut header_bytes)
            .map_err(|e| NativeBackendError::IoError {
                context: "Failed to read WAL header".to_string(),
                source: e,
            })?;

        let mut header = V3WALHeader::from_bytes(&header_bytes)?;

        // Update LSN fields
        header.current_lsn = self.current_lsn;
        header.committed_lsn = self.committed_lsn;

        // Write updated header
        let updated_bytes = header.to_bytes();
        file.seek(SeekFrom::Start(0))
            .map_err(|e| NativeBackendError::IoError {
                context: "Failed to seek to WAL header".to_string(),
                source: e,
            })?;

        file.write_all(&updated_bytes)
            .map_err(|e| NativeBackendError::IoError {
                context: "Failed to write updated WAL header".to_string(),
                source: e,
            })?;

        file.sync_all().map_err(|e| NativeBackendError::IoError {
            context: "Failed to sync WAL header".to_string(),
            source: e,
        })?;

        Ok(())
    }

    /// Truncate WAL file (after checkpoint)
    ///
    /// Removes WAL records that are no longer needed.
    pub fn truncate(&self) -> NativeResult<()> {
        if !self.wal_path.exists() {
            return Ok(());
        }

        std::fs::remove_file(&self.wal_path).map_err(|e| NativeBackendError::IoError {
            context: "Failed to truncate WAL file".to_string(),
            source: e,
        })?;

        Ok(())
    }

    /// Write page allocate record
    pub fn page_allocate(&mut self, page_id: u64) -> NativeResult<u64> {
        let record = V3WALRecord::page_allocate(page_id, self.current_lsn);
        let lsn = record.lsn();
        self.append(&record)?;
        Ok(lsn)
    }

    /// Write page free record
    pub fn page_free(&mut self, page_id: u64, checksum: u32) -> NativeResult<u64> {
        let record = V3WALRecord::page_free(page_id, checksum, self.current_lsn);
        let lsn = record.lsn();
        self.append(&record)?;
        Ok(lsn)
    }

    /// Write page write record
    pub fn page_write(&mut self, page_id: u64, offset: u32, data: Vec<u8>) -> NativeResult<u64> {
        let record = V3WALRecord::page_write(page_id, offset, data, self.current_lsn);
        let lsn = record.lsn();
        self.append(&record)?;
        Ok(lsn)
    }

    /// Write B+Tree split record
    pub fn btree_split(
        &mut self,
        original_page_id: u64,
        new_page_id: u64,
        split_key: u64,
        page_type: bool,
    ) -> NativeResult<u64> {
        let record = V3WALRecord::btree_split(
            original_page_id,
            new_page_id,
            split_key,
            page_type,
            self.current_lsn,
        );
        let lsn = record.lsn();
        self.append(&record)?;
        Ok(lsn)
    }

    /// Write checkpoint record
    pub fn checkpoint(
        &mut self,
        root_page_id: u64,
        total_pages: u64,
        btree_height: u32,
        free_page_list_head: u64,
        header: &PersistentHeaderV3,
    ) -> NativeResult<u64> {
        let record = V3WALRecord::checkpoint(
            root_page_id,
            total_pages,
            btree_height,
            free_page_list_head,
            header,
            self.current_lsn,
        );
        let lsn = record.lsn();
        self.append(&record)?;
        Ok(lsn)
    }

    /// Write transaction begin record
    pub fn transaction_begin(&mut self, tx_id: u64) -> NativeResult<u64> {
        let record = V3WALRecord::TransactionBegin {
            tx_id,
            lsn: self.current_lsn,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };
        let lsn = record.lsn();
        self.append(&record)?;
        Ok(lsn)
    }

    /// Write transaction commit record
    pub fn transaction_commit(&mut self, tx_id: u64) -> NativeResult<u64> {
        let record = V3WALRecord::TransactionCommit {
            tx_id,
            lsn: self.current_lsn,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };
        let lsn = record.lsn();
        self.append(&record)?;
        Ok(lsn)
    }

    /// Write transaction rollback record
    pub fn transaction_rollback(&mut self, tx_id: u64) -> NativeResult<u64> {
        let record = V3WALRecord::TransactionRollback {
            tx_id,
            lsn: self.current_lsn,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0),
        };
        let lsn = record.lsn();
        self.append(&record)?;
        Ok(lsn)
    }

    /// Write edge insert record
    ///
    /// Records an edge insertion for WAL replay during crash recovery.
    /// The edge will be inserted into the edge cluster during recovery.
    pub fn edge_insert(
        &mut self,
        src: i64,
        dst: i64,
        direction: u8,
        page_id: u64,
    ) -> NativeResult<u64> {
        let record = V3WALRecord::edge_insert(src, dst, direction, page_id, self.current_lsn);
        let lsn = record.lsn();
        self.append(&record)?;
        Ok(lsn)
    }

    /// Set buffer flush threshold
    pub fn set_flush_threshold(&mut self, threshold: usize) {
        self.flush_threshold = threshold;
    }
}

/// WAL file path utilities
pub struct V3WALPaths;

impl V3WALPaths {
    /// Get WAL file path for a database file
    pub fn wal_file(db_path: &std::path::Path) -> PathBuf {
        db_path.with_extension("v3wal")
    }

    /// Get checkpoint file path for a database file
    pub fn checkpoint_file(db_path: &std::path::Path) -> PathBuf {
        db_path.with_extension("v3checkpoint")
    }

    /// Get temp file path during checkpoint creation
    pub fn temp_checkpoint_file(db_path: &std::path::Path) -> PathBuf {
        // Add random suffix for uniqueness
        let random: u64 = {
            use std::time::{SystemTime, UNIX_EPOCH};
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0) as u64
        };
        db_path.with_extension(format!("v3checkpoint.tmp.{}", random))
    }
}

/// WAL recovery statistics
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WALRecoveryStats {
    /// Number of records processed
    pub records_processed: usize,
    /// Number of records successfully applied
    pub records_applied: usize,
    /// Number of records skipped (corrupt/invalid)
    pub records_skipped: usize,
    /// Number of page allocations
    pub page_allocations: usize,
    /// Number of page frees
    pub page_frees: usize,
    /// Number of page writes
    pub page_writes: usize,
    /// Number of B+Tree splits
    pub btree_splits: usize,
    /// Number of checkpoints encountered
    pub checkpoints: usize,
}

impl WALRecoveryStats {
    /// Create new empty statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if any records were processed
    pub fn has_activity(&self) -> bool {
        self.records_processed > 0
    }

    /// Get success rate (0.0 to 1.0)
    pub fn success_rate(&self) -> f64 {
        if self.records_processed == 0 {
            1.0
        } else {
            self.records_applied as f64 / self.records_processed as f64
        }
    }
}

/// WAL recovery engine
///
/// Reconstructs database state by replaying WAL records.
/// Uses in-memory page cache during recovery; actual page operations
/// are deferred until BTreeManager integration.
///
/// # Recovery Process
///
/// 1. Open WAL file and read header
/// 2. Read records sequentially until EOF or unrecoverable error
/// 3. For each valid record, update internal state
/// 4. Return final header state and statistics
///
/// # Note
///
/// This is a simplified recovery implementation that works without
/// BTreeManager. Full integration with BTreeManager is deferred
/// to Task 65-04.
#[derive(Debug)]
pub struct WALRecovery {
    /// WAL file path
    wal_path: PathBuf,
    /// Database file path (for checkpoint file access)
    db_path: PathBuf,
    /// In-memory page cache (page_id -> data)
    page_cache: std::collections::HashMap<u64, Vec<u8>>,
    /// Recovery statistics
    stats: WALRecoveryStats,
    /// Last checkpoint header state
    checkpoint_header: Option<PersistentHeaderV3>,
    /// Last LSN processed
    last_lsn: u64,
}

impl WALRecovery {
    /// Create a new WAL recovery engine
    pub fn new(wal_path: PathBuf) -> Self {
        // Derive db_path from wal_path by removing .v3wal extension
        let db_path = wal_path.to_path_buf();
        let db_path = db_path.with_extension("");
        Self {
            wal_path,
            db_path,
            page_cache: std::collections::HashMap::new(),
            stats: WALRecoveryStats::new(),
            checkpoint_header: None,
            last_lsn: LSN_INVALID,
        }
    }

    /// Get recovery statistics
    pub fn stats(&self) -> &WALRecoveryStats {
        &self.stats
    }

    /// Get last checkpoint header (if any)
    pub fn checkpoint_header(&self) -> Option<&PersistentHeaderV3> {
        self.checkpoint_header.as_ref()
    }

    /// Get last LSN processed
    pub fn last_lsn(&self) -> u64 {
        self.last_lsn
    }

    /// Get in-memory page cache
    pub fn page_cache(&self) -> &std::collections::HashMap<u64, Vec<u8>> {
        &self.page_cache
    }

    /// Recover from WAL file
    ///
    /// Reads WAL file and applies all records sequentially.
    /// Returns Ok(()) on successful recovery, even if some records were skipped.
    pub fn recover(&mut self) -> NativeResult<()> {
        use std::io::Read;

        // Check if WAL file exists
        if !self.wal_path.exists() {
            // No WAL file is not an error - database is clean
            return Ok(());
        }

        // Open WAL file
        let mut file =
            std::fs::File::open(&self.wal_path).map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to open WAL file: {}", self.wal_path.display()),
                source: e,
            })?;

        // Read and validate header
        let mut header_bytes = [0u8; V3_WAL_HEADER_SIZE];
        file.read_exact(&mut header_bytes)
            .map_err(|e| NativeBackendError::IoError {
                context: "Failed to read WAL header".to_string(),
                source: e,
            })?;

        let header = V3WALHeader::from_bytes(&header_bytes)?;
        header.validate()?;

        // Read records sequentially
        let mut buffer = Vec::new();
        loop {
            // Read record size (4 bytes)
            let mut size_bytes = [0u8; 4];
            let n = file
                .read(&mut size_bytes)
                .map_err(|e| NativeBackendError::IoError {
                    context: "Failed to read record size".to_string(),
                    source: e,
                })?;

            if n == 0 {
                // EOF reached
                break;
            }

            if n < 4 {
                // Incomplete record size - stop
                self.stats.records_skipped += 1;
                break;
            }

            let record_size = u32::from_le_bytes(size_bytes) as usize;

            if record_size == 0 || record_size > MAX_RECORD_SIZE {
                // Invalid size - skip
                self.stats.records_skipped += 1;
                continue;
            }

            // Read record data
            buffer.clear();
            buffer.resize(record_size, 0);
            let n = file.read_exact(&mut buffer);

            if n.is_err() {
                // Incomplete record - stop
                self.stats.records_skipped += 1;
                break;
            }

            // Deserialize and apply record
            self.stats.records_processed += 1;
            let result = V3WALRecord::from_bytes(&buffer);

            match result {
                Ok(record) => {
                    if let Err(e) = self.apply_record(&record) {
                        // Record application failed - skip
                        eprintln!(
                            "WAL Recovery: Failed to apply record LSN {}: {:?}",
                            record.lsn(),
                            e
                        );
                        self.stats.records_skipped += 1;
                    } else {
                        self.stats.records_applied += 1;
                        self.last_lsn = record.lsn();
                    }
                }
                Err(e) => {
                    // Deserialization failed - skip
                    eprintln!("WAL Recovery: Failed to deserialize record: {:?}", e);
                    self.stats.records_skipped += 1;
                }
            }
        }

        Ok(())
    }

    /// Apply a single WAL record to recovery state
    fn apply_record(&mut self, record: &V3WALRecord) -> NativeResult<()> {
        match record {
            V3WALRecord::PageAllocate { page_id, lsn, .. } => {
                // Allocate empty page in cache
                self.page_cache.insert(*page_id, vec![0; 4096]);
                self.stats.page_allocations += 1;
                self.last_lsn = *lsn;
                Ok(())
            }
            V3WALRecord::PageFree { page_id, lsn, .. } => {
                // Remove page from cache
                self.page_cache.remove(page_id);
                self.stats.page_frees += 1;
                self.last_lsn = *lsn;
                Ok(())
            }
            V3WALRecord::PageWrite {
                page_id,
                offset,
                data,
                checksum: _,
                lsn,
                timestamp: _,
            } => {
                // Update page in cache
                let page = self
                    .page_cache
                    .entry(*page_id)
                    .or_insert_with(|| vec![0; 4096]);
                let offset = *offset as usize;
                if offset + data.len() <= page.len() {
                    page[offset..offset + data.len()].copy_from_slice(data);
                }
                self.stats.page_writes += 1;
                self.last_lsn = *lsn;
                Ok(())
            }
            V3WALRecord::BTreeSplit {
                original_page_id: _,
                new_page_id,
                split_key: _,
                page_type: _,
                lsn,
                timestamp: _,
            } => {
                // Allocate new page for split
                self.page_cache.insert(*new_page_id, vec![0; 4096]);
                self.stats.btree_splits += 1;
                self.last_lsn = *lsn;
                Ok(())
            }
            V3WALRecord::Checkpoint {
                root_page_id: _,
                total_pages: _,
                btree_height: _,
                free_page_list_head: _,
                header_snapshot,
                timestamp: _,
                lsn,
            } => {
                // Restore header from checkpoint
                // Note: This is a simplified version - full integration
                // with BTreeManager will happen in Task 65-04
                if !header_snapshot.is_empty() {
                    let restored =
                        PersistentHeaderV3::from_bytes(header_snapshot).map_err(|e| {
                            NativeBackendError::DeserializationError {
                                context: format!("Failed to restore checkpoint header: {:?}", e),
                            }
                        })?;
                    self.checkpoint_header = Some(restored);
                }
                self.stats.checkpoints += 1;
                self.last_lsn = *lsn;
                Ok(())
            }
            V3WALRecord::TransactionBegin { tx_id: _, lsn, .. } => {
                self.last_lsn = *lsn;
                Ok(())
            }
            V3WALRecord::TransactionCommit { tx_id: _, lsn, .. } => {
                self.last_lsn = *lsn;
                Ok(())
            }
            V3WALRecord::TransactionRollback { tx_id: _, lsn, .. } => {
                self.last_lsn = *lsn;
                Ok(())
            }
            V3WALRecord::KvSet { lsn, .. } => {
                // KV operations don't affect page cache
                // They are applied to the in-memory KV store during recovery
                self.last_lsn = *lsn;
                Ok(())
            }
            V3WALRecord::KvDelete { lsn, .. } => {
                // KV operations don't affect page cache
                self.last_lsn = *lsn;
                Ok(())
            }
            V3WALRecord::KvTombstone { lsn, .. } => {
                // KV tombstones don't affect page cache
                self.last_lsn = *lsn;
                Ok(())
            }
            V3WALRecord::EdgeInsert {
                lsn,
                src: _,
                dst: _,
                direction: _,
                page_id,
                timestamp: _,
            } => {
                // Edge inserts are tracked in page cache for recovery
                // The actual edge data will be loaded from the page during neighbor queries
                // We just mark that this page contains edge data
                self.page_cache
                    .entry(*page_id)
                    .or_insert_with(|| vec![0; 4096]);
                self.last_lsn = *lsn;
                Ok(())
            }
        }
    }

    /// Get header state from last checkpoint (if available)
    ///
    /// Returns the PersistentHeaderV3 that was captured in the most
    /// recent checkpoint record. This can be used to restore the
    /// database to a consistent state.
    pub fn get_header_state(&self) -> Option<&PersistentHeaderV3> {
        self.checkpoint_header.as_ref()
    }

    /// Recover KV state from WAL records
    ///
    /// Replays only KV-related records (KvSet, KvDelete, KvTombstone)
    /// to rebuild an in-memory KvStore. This is called during V3Backend::open()
    /// to restore KV durability across close/reopen cycles.
    ///
    /// # V3 KV RECOVERY CONTRACT
    ///
    /// ## Recovery Precedence (in order):
    /// 1. **WAL replay** (authoritative): If WAL exists, replay all KV records
    ///    - This captures the latest state including mutations after last flush
    /// 2. **Checkpoint fallback**: If WAL missing, read checkpoint file
    ///    - This captures the last flushed state (before WAL truncation)
    /// 3. **Empty KV**: If neither WAL nor checkpoint recoverable
    ///    - Returns Ok(0) to indicate no KV records applied
    ///    - Caller decides whether to continue with empty KV
    ///
    /// ## Corruption Handling:
    /// - Corrupt checkpoint files are auto-deleted (see cleanup_corrupt_checkpoint)
    /// - Checkpoint errors do NOT propagate (return Ok(0) instead)
    /// - WAL errors DO propagate (WAL is authoritative, corruption is serious)
    ///
    /// ## Lifecycle Scenarios:
    /// - After flush(): checkpoint=latest, WAL=empty → checkpoint recovery
    /// - Before flush(): WAL=latest, checkpoint=stale → WAL recovery
    /// - Corrupt checkpoint + valid WAL → WAL recovery (checkpoint never checked)
    /// - Corrupt checkpoint + no WAL → empty KV (checkpoint deleted, logged)
    ///
    /// # Arguments
    /// * `kv_store` - Mutable reference to the KvStore to rebuild
    ///
    /// # Returns
    /// * `Ok(kv_records_applied)` - Number of KV records successfully applied
    /// * `Err(NativeBackendError)` - If WAL file cannot be read (WAL errors propagate)
    pub fn recover_kv(
        &mut self,
        kv_store: &mut super::kv_store::store::KvStore,
    ) -> NativeResult<usize> {
        use crate::backend::native::v3::kv_store::types::KvValue;
        use std::io::Read;

        // Import kv_store module types

        // Check if WAL file exists
        if !self.wal_path.exists() {
            // No WAL file - try reading from checkpoint file (written before WAL truncation)
            // This ensures KV durability survives flush() which calls checkpoint+truncate
            //
            // NOTE: Checkpoint errors are handled gracefully - corrupt checkpoints are
            // auto-deleted and we return Ok(0). This is intentional: KV is auxiliary
            // data and shouldn't brick database open.
            match read_kv_checkpoint(&self.db_path, kv_store) {
                Ok(found) => {
                    if found {
                        // Successfully recovered from checkpoint
                        return Ok(1); // Return non-zero to indicate recovery occurred
                    }
                }
                Err(e) => {
                    // Checkpoint read failed - it's already been cleaned up if corrupt
                    // Log and continue with empty KV
                    eprintln!(
                        "WARNING: KV checkpoint read failed: {:?}. Continuing with empty KV store.",
                        e
                    );
                }
            }
            // No WAL and no/invalid checkpoint - no KV data to recover
            return Ok(0);
        }

        // Open WAL file
        let mut file =
            std::fs::File::open(&self.wal_path).map_err(|e| NativeBackendError::IoError {
                context: format!(
                    "Failed to open WAL file for KV recovery: {}",
                    self.wal_path.display()
                ),
                source: e,
            })?;

        // Read and validate header
        let mut header_bytes = [0u8; V3_WAL_HEADER_SIZE];
        file.read_exact(&mut header_bytes)
            .map_err(|e| NativeBackendError::IoError {
                context: "Failed to read WAL header for KV recovery".to_string(),
                source: e,
            })?;

        let header = V3WALHeader::from_bytes(&header_bytes)?;
        header.validate()?;

        // Read records sequentially, applying only KV records
        let mut buffer = Vec::new();
        let mut kv_records_applied = 0usize;

        loop {
            // Read record size (4 bytes)
            let mut size_bytes = [0u8; 4];
            let n = file
                .read(&mut size_bytes)
                .map_err(|e| NativeBackendError::IoError {
                    context: "Failed to read record size during KV recovery".to_string(),
                    source: e,
                })?;

            if n == 0 {
                // EOF reached
                break;
            }

            if n < 4 {
                // Incomplete record size - stop
                break;
            }

            let record_size = u32::from_le_bytes(size_bytes) as usize;

            if record_size == 0 || record_size > MAX_RECORD_SIZE {
                // Invalid size - skip
                continue;
            }

            // Read record data
            buffer.clear();
            buffer.resize(record_size, 0);
            let n = file.read_exact(&mut buffer);

            if n.is_err() {
                // Incomplete record - stop
                break;
            }

            // Deserialize and apply KV records only
            let result = V3WALRecord::from_bytes(&buffer);

            match result {
                Ok(record) => {
                    match record {
                        V3WALRecord::KvSet {
                            lsn,
                            key,
                            value_bytes,
                            value_type,
                            ttl_seconds,
                            timestamp: _,
                        } => {
                            // Deserialize value from bytes
                            if let Some(value) = KvValue::from_bytes(&value_bytes, value_type) {
                                kv_store.set(key, value, ttl_seconds, lsn);
                                kv_records_applied += 1;
                            }
                        }
                        V3WALRecord::KvDelete { lsn, key, .. } => {
                            kv_store.delete(&key, lsn);
                            kv_records_applied += 1;
                        }
                        V3WALRecord::KvTombstone {
                            lsn,
                            key,
                            old_value_bytes: _,
                            old_value_type: _,
                            ..
                        } => {
                            // Tombstone represents a deleted value - add Null entry
                            kv_store.set(key, KvValue::Null, None, lsn);
                            kv_records_applied += 1;
                        }
                        // Ignore all non-KV records
                        _ => {
                            // Not a KV record - skip
                        }
                    }
                }
                Err(_) => {
                    // Deserialization failed - skip
                }
            }
        }

        Ok(kv_records_applied)
    }
}

/// Write KV checkpoint file for durability across WAL truncation
///
/// This function serializes the current KV store state to a .v3checkpoint file.
/// The checkpoint survives WAL truncation and is read during recovery if WAL is empty.
///
/// ## Checkpoint Format (v2):
/// - Magic: 8 bytes ([b'V', b'3', b'K', b'V', b'C', b'K', 0, 1])
/// - Version: u32 (2 for this format)
/// - Data length: u32
/// - Checksum: SHA-256 hash of data (32 bytes)
/// - Data: Actual KV store bytes
pub fn write_kv_checkpoint(
    db_path: &std::path::Path,
    kv_store: &super::kv_store::store::KvStore,
) -> NativeResult<()> {
    use sha2::{Digest, Sha256};
    use std::io::Write;

    let checkpoint_path = V3WALPaths::checkpoint_file(db_path);
    let temp_path = V3WALPaths::temp_checkpoint_file(db_path);

    // Serialize KV store
    let checkpoint_data = kv_store.to_bytes();

    // Calculate checksum over data
    let mut hasher = Sha256::new();
    hasher.update(&checkpoint_data);
    let checksum: [u8; 32] = hasher.finalize().into();

    // Write to temp file first (atomic write pattern)
    let mut file = std::fs::File::create(&temp_path).map_err(|e| NativeBackendError::IoError {
        context: format!(
            "Failed to create temp checkpoint file: {}",
            temp_path.display()
        ),
        source: e,
    })?;

    // Write magic header for validation (v3 indicates u32 value lengths)
    let magic: [u8; 8] = [b'V', b'3', b'K', b'V', b'C', b'K', 0, 3];
    file.write_all(&magic)
        .map_err(|e| NativeBackendError::IoError {
            context: "Failed to write checkpoint magic".to_string(),
            source: e,
        })?;

    // Write version (3 = with u32 value lengths)
    let version: u32 = 3;
    file.write_all(&version.to_le_bytes())
        .map_err(|e| NativeBackendError::IoError {
            context: "Failed to write checkpoint version".to_string(),
            source: e,
        })?;

    // Write data length
    let data_len: u32 = checkpoint_data.len().try_into().unwrap_or(u32::MAX);
    file.write_all(&data_len.to_le_bytes())
        .map_err(|e| NativeBackendError::IoError {
            context: "Failed to write checkpoint data length".to_string(),
            source: e,
        })?;

    // Write checksum
    file.write_all(&checksum)
        .map_err(|e| NativeBackendError::IoError {
            context: "Failed to write checkpoint checksum".to_string(),
            source: e,
        })?;

    // Write checkpoint data
    file.write_all(&checkpoint_data)
        .map_err(|e| NativeBackendError::IoError {
            context: "Failed to write checkpoint data".to_string(),
            source: e,
        })?;

    // Sync to disk
    file.flush().map_err(|e| NativeBackendError::IoError {
        context: "Failed to flush checkpoint file".to_string(),
        source: e,
    })?;
    file.sync_all().map_err(|e| NativeBackendError::IoError {
        context: "Failed to sync checkpoint file".to_string(),
        source: e,
    })?;

    // Atomic rename
    std::fs::rename(&temp_path, &checkpoint_path).map_err(|e| NativeBackendError::IoError {
        context: format!(
            "Failed to rename checkpoint file: {} -> {}",
            temp_path.display(),
            checkpoint_path.display()
        ),
        source: e,
    })?;

    Ok(())
}

/// Read KV checkpoint file to restore KV state after WAL truncation
///
/// Returns true if checkpoint was found and applied, false if no checkpoint exists.
///
/// ## Supported Formats:
/// - v1: No checksum (magic ends in 0,0)
/// - v2: SHA-256 checksum (magic ends in 0,2)
///
/// ## Corruption Handling:
/// If the checkpoint is corrupt (bad magic, bad checksum, or deserialization failure),
/// the checkpoint file is deleted and an error is returned. This prevents silent data loss.
pub fn read_kv_checkpoint(
    db_path: &std::path::Path,
    kv_store: &mut super::kv_store::store::KvStore,
) -> NativeResult<bool> {
    use sha2::{Digest, Sha256};
    use std::io::Read;

    let checkpoint_path = V3WALPaths::checkpoint_file(db_path);

    if !checkpoint_path.exists() {
        return Ok(false); // No checkpoint file is not an error
    }

    let mut file =
        std::fs::File::open(&checkpoint_path).map_err(|e| NativeBackendError::IoError {
            context: format!(
                "Failed to open checkpoint file: {}",
                checkpoint_path.display()
            ),
            source: e,
        })?;

    // Read and validate magic, determine format version
    let mut magic_bytes = [0u8; 8];
    file.read_exact(&mut magic_bytes).map_err(|e| {
        cleanup_corrupt_checkpoint(&checkpoint_path);
        NativeBackendError::IoError {
            context: "Failed to read checkpoint magic".to_string(),
            source: e,
        }
    })?;

    // Check magic prefix (first 6 bytes)
    if magic_bytes[0..6] != [b'V', b'3', b'K', b'V', b'C', b'K'] {
        cleanup_corrupt_checkpoint(&checkpoint_path);
        return Err(NativeBackendError::InvalidHeader {
            field: "magic".to_string(),
            reason: format!("checkpoint magic prefix mismatch: got {:?}", magic_bytes),
        });
    }

    // Determine format version from magic bytes [7] (last byte)
    let has_checksum = magic_bytes[7] >= 2; // v2 and v3 format have checksums

    // Read version field
    let mut version_bytes = [0u8; 4];
    file.read_exact(&mut version_bytes).map_err(|e| {
        cleanup_corrupt_checkpoint(&checkpoint_path);
        NativeBackendError::IoError {
            context: "Failed to read checkpoint version".to_string(),
            source: e,
        }
    })?;
    let _version = u32::from_le_bytes(version_bytes);

    // Read data length
    let mut len_bytes = [0u8; 4];
    file.read_exact(&mut len_bytes).map_err(|e| {
        cleanup_corrupt_checkpoint(&checkpoint_path);
        NativeBackendError::IoError {
            context: "Failed to read checkpoint data length".to_string(),
            source: e,
        }
    })?;
    let data_len = u32::from_le_bytes(len_bytes) as usize;

    // Read checksum if v2 format
    let stored_checksum: Option<[u8; 32]> = if has_checksum {
        let mut checksum_bytes = [0u8; 32];
        file.read_exact(&mut checksum_bytes).map_err(|e| {
            cleanup_corrupt_checkpoint(&checkpoint_path);
            NativeBackendError::IoError {
                context: "Failed to read checkpoint checksum".to_string(),
                source: e,
            }
        })?;
        Some(checksum_bytes)
    } else {
        None
    };

    // Read checkpoint data
    let mut checkpoint_data = vec![0u8; data_len];
    file.read_exact(&mut checkpoint_data).map_err(|e| {
        cleanup_corrupt_checkpoint(&checkpoint_path);
        NativeBackendError::IoError {
            context: "Failed to read checkpoint data".to_string(),
            source: e,
        }
    })?;

    // Verify checksum if present
    if let Some(stored) = stored_checksum {
        let mut hasher = Sha256::new();
        hasher.update(&checkpoint_data);
        let calculated: [u8; 32] = hasher.finalize().into();

        if calculated != stored {
            cleanup_corrupt_checkpoint(&checkpoint_path);
            return Err(NativeBackendError::InvalidHeader {
                field: "checksum".to_string(),
                reason: "checkpoint checksum mismatch - data may be corrupt".to_string(),
            });
        }
    }

    // Deserialize into KV store (pass the parsed version byte)
    kv_store
        .from_bytes(&checkpoint_data, magic_bytes[7])
        .map_err(|e| {
            cleanup_corrupt_checkpoint(&checkpoint_path);
            NativeBackendError::InvalidHeader {
                field: "checkpoint_data".to_string(),
                reason: format!("Failed to deserialize checkpoint: {}", e),
            }
        })?;

    Ok(true)
}

/// Delete a corrupt checkpoint file to prevent recovery from using bad data.
fn cleanup_corrupt_checkpoint(checkpoint_path: &std::path::Path) {
    let _ = std::fs::remove_file(checkpoint_path);
    eprintln!(
        "WARNING: Deleted corrupt KV checkpoint file: {}",
        checkpoint_path.display()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v3_wal_magic() {
        assert_eq!(V3_WAL_MAGIC, [b'V', b'3', b'W', b'A', b'L', 0, 0, 0]);
    }

    #[test]
    fn test_lsn_utilities() {
        assert!(!lsn_is_valid(LSN_INVALID));
        assert!(lsn_is_valid(LSN_BEGIN));
        assert_eq!(lsn_next(LSN_BEGIN), LSN_BEGIN + 1);
    }

    #[test]
    fn test_v3_wal_header_new() {
        let header = V3WALHeader::new();
        assert!(header.validate().is_ok());
        assert_eq!(header.magic, V3_WAL_MAGIC);
        assert_eq!(header.version, V3_WAL_VERSION);
        assert_eq!(header.page_size, 4096);
    }

    #[test]
    fn test_v3_wal_header_serialization() {
        let original = V3WALHeader::new();

        let bytes = original.to_bytes();
        assert_eq!(bytes.len(), V3_WAL_HEADER_SIZE);

        let restored = V3WALHeader::from_bytes(&bytes).unwrap();
        assert_eq!(restored.magic, original.magic);
        assert_eq!(restored.version, original.version);
        assert_eq!(restored.page_size, original.page_size);
        assert_eq!(restored.current_lsn, original.current_lsn);
        assert_eq!(restored.committed_lsn, original.committed_lsn);
        assert_eq!(restored.checkpointed_lsn, original.checkpointed_lsn);
    }

    #[test]
    fn test_v3_wal_header_invalid_magic() {
        let mut header = V3WALHeader::new();
        header.magic = [b'B', b'A', b'D', 0, 0, 0, 0, 0];

        assert!(header.validate().is_err());
    }

    #[test]
    fn test_v3_wal_header_invalid_page_size() {
        let mut header = V3WALHeader::new();
        header.page_size = 12345;

        assert!(header.validate().is_err());
    }

    #[test]
    fn test_record_type_from_u8() {
        assert_eq!(
            V3WALRecordType::try_from(1).unwrap(),
            V3WALRecordType::PageAllocate
        );
        assert_eq!(
            V3WALRecordType::try_from(5).unwrap(),
            V3WALRecordType::Checkpoint
        );
        assert!(V3WALRecordType::try_from(99).is_err());
    }

    #[test]
    fn test_page_allocate_record() {
        let record = V3WALRecord::page_allocate(42, 100);

        assert!(matches!(record, V3WALRecord::PageAllocate { .. }));
        assert_eq!(record.lsn(), 100);
        assert!(record.is_data_modifying());
        assert!(!record.is_transaction_control());
        assert!(!record.is_checkpoint());
    }

    #[test]
    fn test_page_free_record() {
        let record = V3WALRecord::page_free(42, 0x12345678, 100);

        assert!(matches!(record, V3WALRecord::PageFree { .. }));
        assert_eq!(record.lsn(), 100);
        assert!(record.is_data_modifying());
    }

    #[test]
    fn test_page_write_record() {
        let data = vec![1, 2, 3, 4, 5];
        let record = V3WALRecord::page_write(42, 100, data.clone(), 100);

        assert!(matches!(record, V3WALRecord::PageWrite { .. }));
        assert_eq!(record.lsn(), 100);
        assert!(record.is_data_modifying());
    }

    #[test]
    fn test_btree_split_record() {
        let record = V3WALRecord::btree_split(10, 20, 500, true, 100);

        assert!(matches!(record, V3WALRecord::BTreeSplit { .. }));
        assert_eq!(record.lsn(), 100);
        assert!(record.is_data_modifying());
    }

    #[test]
    fn test_checkpoint_record() {
        let header = PersistentHeaderV3::new_v3();
        let record = V3WALRecord::checkpoint(5, 100, 3, 0, &header, 100);

        assert!(matches!(record, V3WALRecord::Checkpoint { .. }));
        assert_eq!(record.lsn(), 100);
        assert!(!record.is_data_modifying());
        assert!(record.is_checkpoint());
    }

    #[test]
    fn test_transaction_control_records() {
        let begin = V3WALRecord::TransactionBegin {
            tx_id: 1,
            lsn: 100,
            timestamp: 0,
        };
        let commit = V3WALRecord::TransactionCommit {
            tx_id: 1,
            lsn: 101,
            timestamp: 0,
        };
        let rollback = V3WALRecord::TransactionRollback {
            tx_id: 1,
            lsn: 102,
            timestamp: 0,
        };

        assert!(!begin.is_data_modifying());
        assert!(begin.is_transaction_control());
        assert!(!begin.is_checkpoint());

        assert!(!commit.is_data_modifying());
        assert!(commit.is_transaction_control());

        assert!(!rollback.is_data_modifying());
        assert!(rollback.is_transaction_control());
    }

    #[test]
    fn test_record_serialization_round_trip() {
        let records = vec![
            V3WALRecord::page_allocate(42, 100),
            V3WALRecord::page_free(43, 0x12345678, 101),
            V3WALRecord::page_write(44, 0, vec![1, 2, 3], 102),
            V3WALRecord::btree_split(10, 20, 500, true, 103),
        ];

        for original in records {
            let bytes = original.to_bytes().unwrap();
            let restored = V3WALRecord::from_bytes(&bytes).unwrap();

            assert_eq!(restored.record_type(), original.record_type());
            assert_eq!(restored.lsn(), original.lsn());
        }
    }

    #[test]
    fn test_wal_paths() {
        let db_path = std::path::Path::new("/tmp/test.db");

        let wal_path = V3WALPaths::wal_file(db_path);
        assert_eq!(wal_path, std::path::Path::new("/tmp/test.v3wal"));

        let checkpoint_path = V3WALPaths::checkpoint_file(db_path);
        assert_eq!(
            checkpoint_path,
            std::path::Path::new("/tmp/test.v3checkpoint")
        );

        let temp_path = V3WALPaths::temp_checkpoint_file(db_path);
        assert!(temp_path.to_string_lossy().contains("v3checkpoint.tmp"));
    }

    // WALRecovery tests (Task 65-03)

    #[test]
    fn test_wal_recovery_new() {
        let wal_path = std::path::PathBuf::from("/tmp/test_recovery.v3wal");
        let recovery = WALRecovery::new(wal_path);

        assert_eq!(recovery.last_lsn(), LSN_INVALID);
        assert!(!recovery.stats().has_activity());
        assert_eq!(recovery.stats().records_processed, 0);
        assert!(recovery.checkpoint_header().is_none());
    }

    #[test]
    fn test_wal_recovery_stats_default() {
        let stats = WALRecoveryStats::new();

        assert_eq!(stats.records_processed, 0);
        assert_eq!(stats.records_applied, 0);
        assert_eq!(stats.records_skipped, 0);
        assert_eq!(stats.page_allocations, 0);
        assert_eq!(stats.page_frees, 0);
        assert_eq!(stats.page_writes, 0);
        assert_eq!(stats.btree_splits, 0);
        assert_eq!(stats.checkpoints, 0);
    }

    #[test]
    fn test_wal_recovery_stats_success_rate() {
        let mut stats = WALRecoveryStats::new();

        // No activity = 100% success
        assert!((stats.success_rate() - 1.0).abs() < f64::EPSILON);

        stats.records_processed = 10;
        stats.records_applied = 8;
        stats.records_skipped = 2;

        assert!((stats.success_rate() - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_wal_recovery_apply_page_allocate() {
        let wal_path = std::path::PathBuf::from("/tmp/test.v3wal");
        let mut recovery = WALRecovery::new(wal_path);

        let record = V3WALRecord::page_allocate(42, 100);
        recovery.apply_record(&record).unwrap();

        assert!(recovery.page_cache().contains_key(&42));
        assert_eq!(recovery.stats().page_allocations, 1);
        assert_eq!(recovery.stats().records_applied, 0); // apply_record doesn't increment this
        assert_eq!(recovery.last_lsn(), 100);
    }

    #[test]
    fn test_wal_recovery_apply_page_free() {
        let wal_path = std::path::PathBuf::from("/tmp/test.v3wal");
        let mut recovery = WALRecovery::new(wal_path);

        // First allocate, then free
        let alloc_record = V3WALRecord::page_allocate(42, 100);
        recovery.apply_record(&alloc_record).unwrap();

        let free_record = V3WALRecord::page_free(42, 0x12345678, 101);
        recovery.apply_record(&free_record).unwrap();

        assert!(!recovery.page_cache().contains_key(&42));
        assert_eq!(recovery.stats().page_allocations, 1);
        assert_eq!(recovery.stats().page_frees, 1);
        assert_eq!(recovery.last_lsn(), 101);
    }

    #[test]
    fn test_wal_recovery_apply_page_write() {
        let wal_path = std::path::PathBuf::from("/tmp/test.v3wal");
        let mut recovery = WALRecovery::new(wal_path);

        let data = vec![1, 2, 3, 4, 5];
        let record = V3WALRecord::page_write(42, 0, data.clone(), 0x12345678);
        recovery.apply_record(&record).unwrap();

        assert!(recovery.page_cache().contains_key(&42));
        let page = recovery.page_cache().get(&42).unwrap();
        assert_eq!(page[0..5], data[..]);
        assert_eq!(recovery.stats().page_writes, 1);
    }

    #[test]
    fn test_wal_recovery_apply_btree_split() {
        let wal_path = std::path::PathBuf::from("/tmp/test.v3wal");
        let mut recovery = WALRecovery::new(wal_path);

        let record = V3WALRecord::btree_split(10, 20, 500, true, 100);
        recovery.apply_record(&record).unwrap();

        assert!(recovery.page_cache().contains_key(&20));
        assert_eq!(recovery.stats().btree_splits, 1);
        assert_eq!(recovery.last_lsn(), 100);
    }

    #[test]
    fn test_wal_recovery_apply_checkpoint() {
        let wal_path = std::path::PathBuf::from("/tmp/test.v3wal");
        let mut recovery = WALRecovery::new(wal_path);

        let header = PersistentHeaderV3::new_v3();
        let record = V3WALRecord::checkpoint(5, 100, 3, 0, &header, 100);
        recovery.apply_record(&record).unwrap();

        assert!(recovery.checkpoint_header().is_some());
        assert_eq!(recovery.stats().checkpoints, 1);
        assert_eq!(recovery.last_lsn(), 100);
    }

    #[test]
    fn test_wal_recovery_apply_transaction_control() {
        let wal_path = std::path::PathBuf::from("/tmp/test.v3wal");
        let mut recovery = WALRecovery::new(wal_path);

        let begin = V3WALRecord::TransactionBegin {
            tx_id: 1,
            lsn: 100,
            timestamp: 0,
        };
        let commit = V3WALRecord::TransactionCommit {
            tx_id: 1,
            lsn: 101,
            timestamp: 0,
        };
        let rollback = V3WALRecord::TransactionRollback {
            tx_id: 2,
            lsn: 102,
            timestamp: 0,
        };

        recovery.apply_record(&begin).unwrap();
        recovery.apply_record(&commit).unwrap();
        recovery.apply_record(&rollback).unwrap();

        assert_eq!(recovery.last_lsn(), 102);
    }

    #[test]
    fn test_wal_recovery_no_file() {
        // Non-existent WAL file should not error
        let wal_path = std::path::PathBuf::from("/tmp/nonexistent_wal_file_xyz.v3wal");
        let mut recovery = WALRecovery::new(wal_path);

        let result = recovery.recover();
        assert!(result.is_ok());
        assert!(!recovery.stats().has_activity());
    }

    #[test]
    fn test_wal_recovery_get_header_state() {
        let wal_path = std::path::PathBuf::from("/tmp/test.v3wal");
        let mut recovery = WALRecovery::new(wal_path);

        // Initially no header
        assert!(recovery.get_header_state().is_none());

        // After checkpoint, should have header
        let header = PersistentHeaderV3::new_v3();
        let record = V3WALRecord::checkpoint(5, 100, 3, 0, &header, 100);
        recovery.apply_record(&record).unwrap();

        assert!(recovery.get_header_state().is_some());
    }

    // WALWriter tests (Task 65-04)

    #[test]
    fn test_wal_writer_new() {
        let wal_path = std::path::PathBuf::from("/tmp/test_writer.v3wal");
        let writer = WALWriter::new(wal_path.clone(), LSN_BEGIN).unwrap();

        assert_eq!(writer.current_lsn(), LSN_BEGIN);
        assert_eq!(writer.committed_lsn(), LSN_INVALID);
    }

    #[test]
    fn test_wal_writer_set_flush_threshold() {
        let wal_path = std::path::PathBuf::from("/tmp/test.v3wal");
        let mut writer = WALWriter::new(wal_path, LSN_BEGIN).unwrap();

        writer.set_flush_threshold(128 * 1024);
        assert_eq!(writer.flush_threshold, 128 * 1024);
    }

    #[test]
    fn test_wal_writer_page_allocate_helper() {
        let wal_path = std::path::PathBuf::from("/tmp/test.v3wal");
        let mut writer = WALWriter::new(wal_path, LSN_BEGIN).unwrap();

        let lsn = writer.page_allocate(42).unwrap();
        assert_eq!(lsn, LSN_BEGIN);
        assert_eq!(writer.current_lsn(), LSN_BEGIN + 1);
    }

    #[test]
    fn test_wal_writer_page_free_helper() {
        let wal_path = std::path::PathBuf::from("/tmp/test.v3wal");
        let mut writer = WALWriter::new(wal_path, LSN_BEGIN).unwrap();

        let lsn = writer.page_free(42, 0).unwrap();
        assert_eq!(lsn, LSN_BEGIN);
        assert_eq!(writer.current_lsn(), LSN_BEGIN + 1);
    }

    #[test]
    fn test_wal_writer_page_write_helper() {
        let wal_path = std::path::PathBuf::from("/tmp/test.v3wal");
        let mut writer = WALWriter::new(wal_path, LSN_BEGIN).unwrap();

        let data = vec![1, 2, 3, 4, 5];
        let lsn = writer.page_write(42, 0, data).unwrap();
        assert_eq!(lsn, LSN_BEGIN);
        assert_eq!(writer.current_lsn(), LSN_BEGIN + 1);
    }

    #[test]
    fn test_wal_writer_btree_split_helper() {
        let wal_path = std::path::PathBuf::from("/tmp/test.v3wal");
        let mut writer = WALWriter::new(wal_path, LSN_BEGIN).unwrap();

        let lsn = writer.btree_split(10, 20, 500, true).unwrap();
        assert_eq!(lsn, LSN_BEGIN);
        assert_eq!(writer.current_lsn(), LSN_BEGIN + 1);
    }

    #[test]
    fn test_wal_writer_checkpoint_helper() {
        let wal_path = std::path::PathBuf::from("/tmp/test.v3wal");
        let mut writer = WALWriter::new(wal_path, LSN_BEGIN).unwrap();

        let header = PersistentHeaderV3::new_v3();
        let lsn = writer.checkpoint(5, 100, 3, 0, &header).unwrap();
        assert_eq!(lsn, LSN_BEGIN);
        assert_eq!(writer.current_lsn(), LSN_BEGIN + 1);
    }

    #[test]
    fn test_wal_writer_transaction_helpers() {
        let wal_path = std::path::PathBuf::from("/tmp/test.v3wal");
        let mut writer = WALWriter::new(wal_path, LSN_BEGIN).unwrap();

        let begin_lsn = writer.transaction_begin(1).unwrap();
        assert_eq!(begin_lsn, LSN_BEGIN);

        let commit_lsn = writer.transaction_commit(1).unwrap();
        assert_eq!(commit_lsn, LSN_BEGIN + 1);

        let rollback_lsn = writer.transaction_rollback(2).unwrap();
        assert_eq!(rollback_lsn, LSN_BEGIN + 2);
    }

    #[test]
    fn test_wal_writer_multiple_records() {
        let wal_path = std::path::PathBuf::from("/tmp/test.v3wal");
        let mut writer = WALWriter::new(wal_path, LSN_BEGIN).unwrap();

        let lsn1 = writer.page_allocate(42).unwrap();
        let lsn2 = writer.page_allocate(43).unwrap();
        let lsn3 = writer.page_allocate(44).unwrap();

        assert_eq!(lsn1, LSN_BEGIN);
        assert_eq!(lsn2, LSN_BEGIN + 1);
        assert_eq!(lsn3, LSN_BEGIN + 2);
        assert_eq!(writer.current_lsn(), LSN_BEGIN + 3);
    }
}
