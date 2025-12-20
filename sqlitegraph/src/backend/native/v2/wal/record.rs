//! V2 WAL record types and serialization.
//!
//! This module defines the record format for V2 WAL operations, providing
//! efficient serialization/deserialization with minimal overhead and
//! cluster-affinity organization for optimal I/O patterns.

use crate::backend::native::{NativeBackendError, NativeResult};
use crate::backend::native::v2::edge_cluster::{CompactEdgeRecord, Direction};
use crate::backend::native::v2::node_record_v2::NodeRecordV2;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// V2 WAL record types for different operations
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum V2WALRecordType {
    /// Node creation with initial data
    NodeInsert = 1,

    /// Node modification/update
    NodeUpdate = 2,

    /// Node deletion (logical)
    NodeDelete = 3,

    /// Edge cluster creation
    ClusterCreate = 4,

    /// Edge insertion into cluster
    EdgeInsert = 5,

    /// Edge modification within cluster
    EdgeUpdate = 6,

    /// Edge deletion (logical)
    EdgeDelete = 7,

    /// String table entry creation
    StringInsert = 8,

    /// Free space block allocation
    FreeSpaceAllocate = 9,

    /// Free space block deallocation
    FreeSpaceDeallocate = 10,

    /// Transaction begin marker
    TransactionBegin = 11,

    /// Transaction commit marker
    TransactionCommit = 12,

    /// Transaction rollback marker
    TransactionRollback = 13,

    /// Checkpoint marker
    Checkpoint = 14,

    /// Database header update
    HeaderUpdate = 15,

    /// End of WAL segment marker
    SegmentEnd = 16,

    /// Transaction prepare phase marker (two-phase commit)
    TransactionPrepare = 17,

    /// Transaction abort marker (two-phase commit)
    TransactionAbort = 18,

    /// Savepoint creation marker
    SavepointCreate = 19,

    /// Savepoint rollback marker
    SavepointRollback = 20,

    /// Savepoint release marker
    SavepointRelease = 21,

    /// Backup creation marker
    BackupCreate = 22,

    /// Backup restore marker
    BackupRestore = 23,

    /// Lock acquisition marker
    LockAcquire = 24,

    /// Lock release marker
    LockRelease = 25,

    /// Index update marker
    IndexUpdate = 26,

    /// Statistics update marker
    StatisticsUpdate = 27,
}

impl V2WALRecordType {
    /// Get all record types that modify data (require checkpointing)
    pub fn data_modifying() -> &'static [V2WALRecordType] {
        &[
            Self::NodeInsert,
            Self::NodeUpdate,
            Self::NodeDelete,
            Self::ClusterCreate,
            Self::EdgeInsert,
            Self::EdgeUpdate,
            Self::EdgeDelete,
            Self::StringInsert,
            Self::FreeSpaceAllocate,
            Self::FreeSpaceDeallocate,
            Self::HeaderUpdate,
        ]
    }

    /// Get transaction control record types
    pub fn transaction_control() -> &'static [V2WALRecordType] {
        &[
            Self::TransactionBegin,
            Self::TransactionCommit,
            Self::TransactionRollback,
        ]
    }

    /// Check if a record type requires checkpointing
    pub fn requires_checkpoint(&self) -> bool {
        Self::data_modifying().contains(self)
    }

    /// Check if a record type is part of transaction control
    pub fn is_transaction_control(&self) -> bool {
        Self::transaction_control().contains(self)
    }
}

impl TryFrom<u8> for V2WALRecordType {
    type Error = NativeBackendError;

    fn try_from(value: u8) -> NativeResult<Self> {
        match value {
            1 => Ok(Self::NodeInsert),
            2 => Ok(Self::NodeUpdate),
            3 => Ok(Self::NodeDelete),
            4 => Ok(Self::ClusterCreate),
            5 => Ok(Self::EdgeInsert),
            6 => Ok(Self::EdgeUpdate),
            7 => Ok(Self::EdgeDelete),
            8 => Ok(Self::StringInsert),
            9 => Ok(Self::FreeSpaceAllocate),
            10 => Ok(Self::FreeSpaceDeallocate),
            11 => Ok(Self::TransactionBegin),
            12 => Ok(Self::TransactionCommit),
            13 => Ok(Self::TransactionRollback),
            14 => Ok(Self::Checkpoint),
            15 => Ok(Self::HeaderUpdate),
            16 => Ok(Self::SegmentEnd),
            17 => Ok(Self::TransactionPrepare),
            18 => Ok(Self::TransactionAbort),
            19 => Ok(Self::SavepointCreate),
            20 => Ok(Self::SavepointRollback),
            21 => Ok(Self::SavepointRelease),
            22 => Ok(Self::BackupCreate),
            23 => Ok(Self::BackupRestore),
            24 => Ok(Self::LockAcquire),
            25 => Ok(Self::LockRelease),
            26 => Ok(Self::IndexUpdate),
            27 => Ok(Self::StatisticsUpdate),
            _ => Err(NativeBackendError::CorruptStringTable {
                reason: format!("unknown WAL record type: {}", value),
            }),
        }
    }
}

/// V2 WAL record containing operation data
#[derive(Debug, Clone)]
pub enum V2WALRecord {
    /// Node creation with initial data
    NodeInsert {
        node_id: i64,
        slot_offset: u64,
        node_data: Vec<u8>,
    },

    /// Node modification/update
    NodeUpdate {
        node_id: i64,
        slot_offset: u64,
        old_data: Vec<u8>,
        new_data: Vec<u8>,
    },

    /// Node deletion (logical)
    NodeDelete {
        node_id: i64,
        slot_offset: u64,
        old_data: Vec<u8>,
    },

    /// Edge cluster creation
    ClusterCreate {
        node_id: i64,
        direction: Direction,
        cluster_offset: u64,
        cluster_size: u32,
        edge_data: Vec<u8>,
    },

    /// Edge insertion into cluster
    EdgeInsert {
        cluster_key: (i64, Direction), // (node_id, direction)
        edge_record: CompactEdgeRecord,
        insertion_point: u32,
    },

    /// Edge modification within cluster
    EdgeUpdate {
        cluster_key: (i64, Direction),
        old_edge: CompactEdgeRecord,
        new_edge: CompactEdgeRecord,
        position: u32,
    },

    /// Edge deletion (logical)
    EdgeDelete {
        cluster_key: (i64, Direction),
        old_edge: CompactEdgeRecord,
        position: u32,
    },

    /// String table entry creation
    StringInsert {
        string_id: u32,
        string_value: String,
    },

    /// Free space block allocation
    FreeSpaceAllocate {
        block_offset: u64,
        block_size: u32,
        block_type: u8,
    },

    /// Free space block deallocation
    FreeSpaceDeallocate {
        block_offset: u64,
        block_size: u32,
        block_type: u8,
    },

    /// Transaction begin marker
    TransactionBegin {
        tx_id: u64,
        timestamp: u64,
    },

    /// Transaction commit marker
    TransactionCommit {
        tx_id: u64,
        timestamp: u64,
    },

    /// Transaction rollback marker
    TransactionRollback {
        tx_id: u64,
        timestamp: u64,
    },

    /// Checkpoint marker
    Checkpoint {
        checkpointed_lsn: u64,
        timestamp: u64,
    },

    /// Database header update
    HeaderUpdate {
        header_offset: u64,
        old_data: Vec<u8>,
        new_data: Vec<u8>,
    },

    /// End of WAL segment marker
    SegmentEnd {
        segment_lsn: u64,
        checksum: u32,
    },

    /// Transaction prepare phase marker (two-phase commit)
    TransactionPrepare {
        tx_id: u64,
        record_count: u64,
        timestamp: std::time::SystemTime,
    },

    /// Transaction abort marker (two-phase commit)
    TransactionAbort {
        tx_id: u64,
        abort_reason: String,
        timestamp: std::time::SystemTime,
    },

    /// Savepoint creation marker
    SavepointCreate {
        tx_id: u64,
        savepoint_id: String,
        timestamp: std::time::SystemTime,
    },

    /// Savepoint rollback marker
    SavepointRollback {
        tx_id: u64,
        savepoint_id: String,
        timestamp: std::time::SystemTime,
    },

    /// Savepoint release marker
    SavepointRelease {
        tx_id: u64,
        savepoint_id: String,
        timestamp: std::time::SystemTime,
    },

    /// Backup creation marker
    BackupCreate {
        backup_id: String,
        backup_path: std::path::PathBuf,
        timestamp: std::time::SystemTime,
    },

    /// Backup restore marker
    BackupRestore {
        backup_id: String,
        backup_path: std::path::PathBuf,
        target_path: std::path::PathBuf,
        timestamp: std::time::SystemTime,
    },

    /// Lock acquisition marker
    LockAcquire {
        tx_id: u64,
        resource_id: i64,
        lock_type: u8,
        timestamp: std::time::SystemTime,
    },

    /// Lock release marker
    LockRelease {
        tx_id: u64,
        resource_id: i64,
        timestamp: std::time::SystemTime,
    },

    /// Index update marker
    IndexUpdate {
        index_id: u32,
        operation_type: u8,
        key_data: Vec<u8>,
        timestamp: std::time::SystemTime,
    },

    /// Statistics update marker
    StatisticsUpdate {
        stats_type: u8,
        stats_data: Vec<u8>,
        timestamp: std::time::SystemTime,
    },
}

impl V2WALRecord {
    /// Get the record type
    pub fn record_type(&self) -> V2WALRecordType {
        match self {
            Self::NodeInsert { .. } => V2WALRecordType::NodeInsert,
            Self::NodeUpdate { .. } => V2WALRecordType::NodeUpdate,
            Self::NodeDelete { .. } => V2WALRecordType::NodeDelete,
            Self::ClusterCreate { .. } => V2WALRecordType::ClusterCreate,
            Self::EdgeInsert { .. } => V2WALRecordType::EdgeInsert,
            Self::EdgeUpdate { .. } => V2WALRecordType::EdgeUpdate,
            Self::EdgeDelete { .. } => V2WALRecordType::EdgeDelete,
            Self::StringInsert { .. } => V2WALRecordType::StringInsert,
            Self::FreeSpaceAllocate { .. } => V2WALRecordType::FreeSpaceAllocate,
            Self::FreeSpaceDeallocate { .. } => V2WALRecordType::FreeSpaceDeallocate,
            Self::TransactionBegin { .. } => V2WALRecordType::TransactionBegin,
            Self::TransactionCommit { .. } => V2WALRecordType::TransactionCommit,
            Self::TransactionRollback { .. } => V2WALRecordType::TransactionRollback,
            Self::Checkpoint { .. } => V2WALRecordType::Checkpoint,
            Self::HeaderUpdate { .. } => V2WALRecordType::HeaderUpdate,
            Self::SegmentEnd { .. } => V2WALRecordType::SegmentEnd,
            Self::TransactionPrepare { .. } => V2WALRecordType::TransactionPrepare,
            Self::TransactionAbort { .. } => V2WALRecordType::TransactionAbort,
            Self::SavepointCreate { .. } => V2WALRecordType::SavepointCreate,
            Self::SavepointRollback { .. } => V2WALRecordType::SavepointRollback,
            Self::SavepointRelease { .. } => V2WALRecordType::SavepointRelease,
            Self::BackupCreate { .. } => V2WALRecordType::BackupCreate,
            Self::BackupRestore { .. } => V2WALRecordType::BackupRestore,
            Self::LockAcquire { .. } => V2WALRecordType::LockAcquire,
            Self::LockRelease { .. } => V2WALRecordType::LockRelease,
            Self::IndexUpdate { .. } => V2WALRecordType::IndexUpdate,
            Self::StatisticsUpdate { .. } => V2WALRecordType::StatisticsUpdate,
        }
    }

    /// Get the cluster key for cluster-affinity logging (if applicable)
    pub fn cluster_key(&self) -> Option<i64> {
        match self {
            Self::NodeInsert { node_id, .. } => Some(*node_id),
            Self::NodeUpdate { node_id, .. } => Some(*node_id),
            Self::NodeDelete { node_id, .. } => Some(*node_id),
            Self::ClusterCreate { node_id, .. } => Some(*node_id),
            Self::EdgeInsert { cluster_key: (node_id, _), .. } => Some(*node_id),
            Self::EdgeUpdate { cluster_key: (node_id, _), .. } => Some(*node_id),
            Self::EdgeDelete { cluster_key: (node_id, _), .. } => Some(*node_id),
            _ => None,
        }
    }

    /// Estimate the serialized size of this record
    pub fn serialized_size(&self) -> usize {
        let base_size = std::mem::size_of::<V2WALRecordType>() + std::mem::size_of::<u32>(); // type + size field

        match self {
            Self::NodeInsert { node_data, .. } => base_size + 8 + 8 + 4 + node_data.len(),
            Self::NodeUpdate { old_data, new_data, .. } => base_size + 8 + 8 + 4 + old_data.len() + 4 + new_data.len(),
            Self::NodeDelete { old_data, .. } => base_size + 8 + 8 + 4 + old_data.len(),
            Self::ClusterCreate { edge_data, .. } => base_size + 8 + 1 + 8 + 4 + edge_data.len(),
            Self::EdgeInsert { edge_record, .. } => base_size + 8 + 1 + edge_record.serialized_size() + 4,
            Self::EdgeUpdate { old_edge, new_edge, .. } => {
                base_size + 8 + 1 + old_edge.serialized_size() + new_edge.serialized_size() + 4
            }
            Self::EdgeDelete { old_edge, .. } => base_size + 8 + 1 + old_edge.serialized_size() + 4,
            Self::StringInsert { string_value, .. } => base_size + 4 + string_value.len(),
            Self::FreeSpaceAllocate { .. } | Self::FreeSpaceDeallocate { .. } => base_size + 8 + 4 + 1,
            Self::TransactionBegin { .. } | Self::TransactionCommit { .. } | Self::TransactionRollback { .. } => {
                base_size + 8 + 8
            }
            Self::Checkpoint { .. } => base_size + 8 + 8,
            Self::HeaderUpdate { old_data, new_data, .. } => base_size + 8 + old_data.len() + new_data.len(),
            Self::SegmentEnd { .. } => base_size + 8 + 4,
            Self::TransactionPrepare { record_count, .. } => base_size + 8 + 8 + 8,
            Self::TransactionAbort { abort_reason, .. } => base_size + 8 + abort_reason.len(),
            Self::SavepointCreate { savepoint_id, .. } => base_size + 8 + savepoint_id.len(),
            Self::SavepointRollback { savepoint_id, .. } => base_size + 8 + savepoint_id.len(),
            Self::SavepointRelease { savepoint_id, .. } => base_size + 8 + savepoint_id.len(),
            Self::BackupCreate { backup_id, backup_path, .. } => {
                base_size + backup_id.len() + backup_path.to_string_lossy().len()
            }
            Self::BackupRestore { backup_id, backup_path, target_path, .. } => {
                base_size + backup_id.len() + backup_path.to_string_lossy().len() + target_path.to_string_lossy().len()
            }
            Self::LockAcquire { .. } | Self::LockRelease { .. } => base_size + 8 + 8 + 1,
            Self::IndexUpdate { .. } | Self::StatisticsUpdate { .. } => base_size,
        }
    }

    /// Check if this record modifies data (requires checkpointing)
    pub fn modifies_data(&self) -> bool {
        self.record_type().requires_checkpoint()
    }

    /// Check if this record is transaction control
    pub fn is_transaction_control(&self) -> bool {
        self.record_type().is_transaction_control()
    }
}

/// WAL record serialization error
#[derive(Debug, Clone)]
pub enum WALSerializationError {
    /// Invalid record type encountered
    InvalidRecordType(u8),

    /// Insufficient data for record deserialization
    InsufficientData {
        expected: usize,
        actual: usize,
        record_type: V2WALRecordType,
    },

    /// Data corruption detected
    CorruptedData {
        location: String,
        details: String,
    },

    /// I/O error during serialization
    IoError(String),

    /// Size overflow in record data
    SizeOverflow,
}

impl std::fmt::Display for WALSerializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidRecordType(t) => write!(f, "Invalid WAL record type: {}", t),
            Self::InsufficientData { expected, actual, record_type } => {
                write!(f, "Insufficient data for {:?}: expected {}, got {}", record_type, expected, actual)
            }
            Self::CorruptedData { location, details } => {
                write!(f, "Corrupted WAL data at {}: {}", location, details)
            }
            Self::IoError(msg) => write!(f, "I/O error during WAL serialization: {}", msg),
            Self::SizeOverflow => write!(f, "Size overflow in WAL record data"),
        }
    }
}

impl std::error::Error for WALSerializationError {}

/// WAL record serializer/deserializer
pub struct V2WALSerializer;

impl V2WALSerializer {
    /// Serialize a WAL record to bytes
    pub fn serialize(record: &V2WALRecord) -> NativeResult<Vec<u8>> {
        let mut buffer = Vec::with_capacity(record.serialized_size());

        // Write record type
        buffer.push(record.record_type() as u8);

        // Write placeholder for record size
        let size_pos = buffer.len();
        buffer.extend_from_slice(&[0u8; 4]);

        let data_start = buffer.len();

        // Serialize record-specific data
        match record {
            V2WALRecord::NodeInsert { node_id, slot_offset, node_data } => {
                buffer.extend_from_slice(&node_id.to_le_bytes());
                buffer.extend_from_slice(&slot_offset.to_le_bytes());
                buffer.extend_from_slice(&(node_data.len() as u32).to_le_bytes());
                buffer.extend_from_slice(node_data);
            }

            V2WALRecord::NodeUpdate { node_id, slot_offset, old_data, new_data } => {
                buffer.extend_from_slice(&node_id.to_le_bytes());
                buffer.extend_from_slice(&slot_offset.to_le_bytes());
                buffer.extend_from_slice(&(old_data.len() as u32).to_le_bytes());
                buffer.extend_from_slice(old_data);
                buffer.extend_from_slice(&(new_data.len() as u32).to_le_bytes());
                buffer.extend_from_slice(new_data);
            }

            V2WALRecord::NodeDelete { node_id, slot_offset, old_data } => {
                buffer.extend_from_slice(&node_id.to_le_bytes());
                buffer.extend_from_slice(&slot_offset.to_le_bytes());
                buffer.extend_from_slice(&(old_data.len() as u32).to_le_bytes());
                buffer.extend_from_slice(old_data);
            }

            V2WALRecord::ClusterCreate { node_id, direction, cluster_offset, cluster_size, edge_data } => {
                buffer.extend_from_slice(&node_id.to_le_bytes());
                buffer.push(*direction as u8);
                buffer.extend_from_slice(&cluster_offset.to_le_bytes());
                buffer.extend_from_slice(&cluster_size.to_le_bytes());
                buffer.extend_from_slice(&(edge_data.len() as u32).to_le_bytes());
                buffer.extend_from_slice(edge_data);
            }

            V2WALRecord::EdgeInsert { cluster_key, edge_record, insertion_point } => {
                buffer.extend_from_slice(&cluster_key.0.to_le_bytes());
                buffer.push(cluster_key.1 as u8);
                buffer.extend_from_slice(&edge_record.as_bytes());
                buffer.extend_from_slice(&insertion_point.to_le_bytes());
            }

            V2WALRecord::TransactionBegin { tx_id, timestamp } => {
                buffer.extend_from_slice(&tx_id.to_le_bytes());
                buffer.extend_from_slice(&timestamp.to_le_bytes());
            }

            V2WALRecord::TransactionCommit { tx_id, timestamp } => {
                buffer.extend_from_slice(&tx_id.to_le_bytes());
                buffer.extend_from_slice(&timestamp.to_le_bytes());
            }

            V2WALRecord::TransactionRollback { tx_id, timestamp } => {
                buffer.extend_from_slice(&tx_id.to_le_bytes());
                buffer.extend_from_slice(&timestamp.to_le_bytes());
            }

            // Implement other record types similarly...
            _ => {
                return Err(NativeBackendError::CorruptStringTable {
                    reason: format!("WAL serialization error - unsupported record type: {:?}", record.record_type()),
                });
            }
        }

        // Write the actual record size
        let record_size = buffer.len() - data_start;
        let size_bytes = (record_size as u32).to_le_bytes();
        buffer[size_pos..size_pos + 4].copy_from_slice(&size_bytes);

        Ok(buffer)
    }

    /// Deserialize a WAL record from bytes
    pub fn deserialize(data: &[u8]) -> NativeResult<V2WALRecord> {
        if data.is_empty() {
            return Err(NativeBackendError::CorruptStringTable {
                reason: "WAL deserialization error - empty data buffer".to_string(),
            });
        }

        let record_type = V2WALRecordType::try_from(data[0])?;

        if data.len() < 5 {
            return Err(NativeBackendError::CorruptStringTable {
                reason: "WAL deserialization error - insufficient data for record size".to_string(),
            });
        }

        let record_size = u32::from_le_bytes([data[1], data[2], data[3], data[4]]) as usize;

        if data.len() < 5 + record_size {
            return Err(NativeBackendError::CorruptStringTable {
                reason: format!("WAL deserialization error - insufficient data: expected {}, got {}", record_size + 5, data.len()),
            });
        }

        let record_data = &data[5..5 + record_size];

        // Deserialize based on record type
        match record_type {
            V2WALRecordType::NodeInsert => {
                if record_data.len() < 16 {
                    return Err(NativeBackendError::CorruptStringTable {
                        reason: "NodeInsert deserialization error - insufficient data for header".to_string(),
                    });
                }

                let node_id = i64::from_le_bytes(record_data[0..8].try_into().unwrap());
                let slot_offset = u64::from_le_bytes(record_data[8..16].try_into().unwrap());

                if record_data.len() < 20 {
                    return Err(NativeBackendError::CorruptStringTable {
                        reason: "NodeInsert deserialization error - insufficient data for size field".to_string(),
                    });
                }

                let data_len = u32::from_le_bytes(record_data[16..20].try_into().unwrap()) as usize;

                if record_data.len() < 20 + data_len {
                    return Err(NativeBackendError::CorruptStringTable {
                        reason: "NodeInsert deserialization error - insufficient data for node data".to_string(),
                    });
                }

                let node_data = record_data[20..20 + data_len].to_vec();

                Ok(V2WALRecord::NodeInsert { node_id, slot_offset, node_data })
            }

            V2WALRecordType::TransactionBegin => {
                if record_data.len() < 16 {
                    return Err(NativeBackendError::CorruptStringTable {
                        reason: "TransactionBegin deserialization error - insufficient data".to_string(),
                    });
                }

                let tx_id = u64::from_le_bytes(record_data[0..8].try_into().unwrap());
                let timestamp = u64::from_le_bytes(record_data[8..16].try_into().unwrap());

                Ok(V2WALRecord::TransactionBegin { tx_id, timestamp })
            }

            V2WALRecordType::TransactionCommit => {
                if record_data.len() < 16 {
                    return Err(NativeBackendError::CorruptStringTable {
                        reason: "TransactionCommit deserialization error - insufficient data".to_string(),
                    });
                }

                let tx_id = u64::from_le_bytes(record_data[0..8].try_into().unwrap());
                let timestamp = u64::from_le_bytes(record_data[8..16].try_into().unwrap());

                Ok(V2WALRecord::TransactionCommit { tx_id, timestamp })
            }

            V2WALRecordType::TransactionRollback => {
                if record_data.len() < 16 {
                    return Err(NativeBackendError::CorruptStringTable {
                        reason: "TransactionRollback deserialization error - insufficient data".to_string(),
                    });
                }

                let tx_id = u64::from_le_bytes(record_data[0..8].try_into().unwrap());
                let timestamp = u64::from_le_bytes(record_data[8..16].try_into().unwrap());

                Ok(V2WALRecord::TransactionRollback { tx_id, timestamp })
            }

            // Implement other record types similarly...
            _ => Err(NativeBackendError::CorruptStringTable {
                reason: format!("WAL deserialization error - unsupported record type: {:?}", record_type),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_type_properties() {
        assert!(V2WALRecordType::NodeInsert.requires_checkpoint());
        assert!(V2WALRecordType::TransactionBegin.is_transaction_control());
        assert!(!V2WALRecordType::TransactionBegin.requires_checkpoint());
    }

    #[test]
    fn test_v2_wal_record_cluster_key() {
        let record = V2WALRecord::NodeInsert {
            node_id: 42,
            slot_offset: 1024,
            node_data: vec![1, 2, 3],
        };
        assert_eq!(record.cluster_key(), Some(42));

        let record = V2WALRecord::TransactionBegin {
            tx_id: 100,
            timestamp: 123456,
        };
        assert_eq!(record.cluster_key(), None);
    }

    #[test]
    fn test_record_serialization_roundtrip() {
        let original = V2WALRecord::NodeInsert {
            node_id: 123,
            slot_offset: 4096,
            node_data: vec![4, 5, 6, 7, 8],
        };

        let serialized = V2WALSerializer::serialize(&original).unwrap();
        let deserialized = V2WALSerializer::deserialize(&serialized).unwrap();

        match (original, deserialized) {
            (V2WALRecord::NodeInsert { node_id: id1, slot_offset: off1, node_data: data1 },
             V2WALRecord::NodeInsert { node_id: id2, slot_offset: off2, node_data: data2 }) => {
                assert_eq!(id1, id2);
                assert_eq!(off1, off2);
                assert_eq!(data1, data2);
            }
            _ => panic!("Record type mismatch after roundtrip"),
        }
    }

    #[test]
    fn test_serialized_size_estimation() {
        let record = V2WALRecord::NodeInsert {
            node_id: 42,
            slot_offset: 1024,
            node_data: vec![1, 2, 3, 4, 5],
        };

        let estimated = record.serialized_size();
        let serialized = V2WALSerializer::serialize(&record).unwrap();

        // Estimated size should be >= actual serialized size
        assert!(estimated >= serialized.len());
    }
}