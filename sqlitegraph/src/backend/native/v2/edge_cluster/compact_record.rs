//! Compact on-disk representation for edges inside a cluster.

use super::cluster_trace::Direction;
use crate::backend::native::v2::string_table::StringTable;
use crate::backend::native::{EdgeRecord, NativeBackendError, NativeResult};

/// Compact edge record for V2 format.
///
/// The layout is deterministic:
/// `[neighbor_id: i64][edge_type_offset: u16][edge_data_len: u16][edge_data: bytes...]`
#[derive(Debug, Clone)]
pub struct CompactEdgeRecord {
    /// Neighbor node ID (target for outgoing, source for incoming).
    pub neighbor_id: i64,
    /// Edge type offset inside the shared string table.
    pub edge_type_offset: u16,
    /// Serialized JSON payload for the edge.
    pub edge_data: Vec<u8>,
}

impl CompactEdgeRecord {
    /// Construct a new compact record from primitive fields.
    pub fn new(neighbor_id: i64, edge_type_offset: u16, edge_data: Vec<u8>) -> Self {
        Self {
            neighbor_id,
            edge_type_offset,
            edge_data,
        }
    }

    /// Serialize the record into the binary cluster layout.
    pub fn serialize(&self) -> Vec<u8> {
        let edge_data_len = self.edge_data.len() as u16;
        let mut buffer = Vec::with_capacity(8 + 2 + 2 + self.edge_data.len());
        buffer.extend_from_slice(&self.neighbor_id.to_be_bytes());
        buffer.extend_from_slice(&self.edge_type_offset.to_be_bytes());
        buffer.extend_from_slice(&edge_data_len.to_be_bytes());
        buffer.extend_from_slice(&self.edge_data);
        buffer
    }

    /// Deserialize a compact record from the provided bytes.
    pub fn deserialize(bytes: &[u8]) -> NativeResult<Self> {
        if bytes.len() < 12 {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: 12,
            });
        }

        let neighbor_id = i64::from_be_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        ]);

        let edge_type_offset = u16::from_be_bytes([bytes[8], bytes[9]]);
        let edge_data_len = u16::from_be_bytes([bytes[10], bytes[11]]) as usize;

        if bytes.len() < 12 + edge_data_len {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: 12 + edge_data_len,
            });
        }

        let edge_data = bytes[12..12 + edge_data_len].to_vec();

        Ok(Self {
            neighbor_id,
            edge_type_offset,
            edge_data,
        })
    }

    /// Total serialized size of this record.
    pub fn size_bytes(&self) -> usize {
        8 + 2 + 2 + self.edge_data.len()
    }

    /// Create compact record directly from EdgeRecord without data loss.
    /// This is the new pipeline method that preserves original edge_type and edge_data.
    pub fn from_edge_record(
        edge: &EdgeRecord,
        direction: Direction,
        string_table: &mut StringTable,
    ) -> NativeResult<Self> {
        let neighbor_id = match direction {
            Direction::Outgoing => edge.to_id,
            Direction::Incoming => edge.from_id,
        };

        if neighbor_id <= 0 {
            return Err(NativeBackendError::InvalidNodeId {
                id: neighbor_id,
                max_id: 0,
            });
        }

        let type_offset = string_table.get_or_add_offset(&edge.edge_type)?;
        // HOT PATH FIX: Only serialize edge data if it's non-empty/null
        // JSON serialization is expensive and unnecessary for neighbor queries
        let data = if edge.data == serde_json::Value::Null {
            Vec::new() // Empty bytes for null data (common case)
        } else {
            serde_json::to_vec(&edge.data)?
        };

        Ok(Self {
            neighbor_id,
            edge_type_offset: type_offset,
            edge_data: data,
        })
    }
}
