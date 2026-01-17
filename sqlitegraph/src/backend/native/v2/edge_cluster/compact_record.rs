//! Compact on-disk representation for edges inside a cluster.

use super::cluster_trace::Direction;
use crate::backend::native::v2::string_table::StringTable;
use crate::backend::native::{EdgeRecord, NativeBackendError, NativeResult};

/// Delta-encoded edge for compressed representation.
///
/// Stores the difference from the previous neighbor ID instead of the full ID.
/// This compression is effective when neighbor IDs are sequentially allocated
/// or clustered together (common in many graph patterns).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeltaEncodedEdge {
    /// Difference from previous neighbor_id (u32::MAX indicates overflow > 2^32)
    pub delta: u32,
    /// Edge type offset inside the shared string table.
    pub type_offset: u16,
    /// Length of data payload.
    pub data_len: u16,
}

impl DeltaEncodedEdge {
    /// Maximum delta value that can be stored (used for overflow indication).
    pub const MAX_DELTA: u32 = u32::MAX;

    /// Encode the difference between two neighbor IDs.
    ///
    /// Returns `None` if the gap exceeds u32::MAX - 1 (we reserve MAX for overflow).
    /// Returns `Some(MAX_DELTA)` if the gap exceeds u32::MAX.
    pub fn encode_delta(previous_id: i64, current_id: i64) -> Option<u32> {
        if previous_id < 0 || current_id < 0 {
            return None;
        }

        let diff = current_id.abs_diff(previous_id);
        if diff >= Self::MAX_DELTA as u64 {
            Some(Self::MAX_DELTA)
        } else {
            Some(diff as u32)
        }
    }

    /// Decode a delta value to get the current neighbor ID.
    ///
    /// Returns `None` if the delta is MAX_DELTA (overflow case).
    pub fn decode_delta(previous_id: i64, delta: u32) -> Option<i64> {
        if delta == Self::MAX_DELTA {
            return None; // Overflow case, full ID needed
        }

        let current = previous_id.wrapping_add(delta as i64);
        if current < 0 {
            return None;
        }

        Some(current)
    }

    /// Check if this delta represents an overflow.
    pub fn is_overflow(&self) -> bool {
        self.delta == Self::MAX_DELTA
    }
}

/// Compact edge record for V2 format.
///
/// The layout is deterministic:
/// `[neighbor_id: i64][edge_type_offset: u16][edge_data_len: u16][edge_data: bytes...]`
///
/// This format supports both delta-encoded and non-encoded representations for
/// backward compatibility.
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

    /// Alias for size_bytes() to maintain compatibility.
    pub fn serialized_size(&self) -> usize {
        self.size_bytes()
    }

    /// Estimated size in bytes for WAL record estimation.
    pub fn estimated_size(&self) -> usize {
        self.size_bytes()
    }

    /// Get serialized bytes for this record.
    pub fn as_bytes(&self) -> Vec<u8> {
        self.serialize()
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

    /// Convert this record to a delta-encoded representation.
    ///
    /// Returns `None` if the delta cannot be encoded (gap too large).
    pub fn to_delta_encoded(&self, previous_id: i64) -> Option<DeltaEncodedEdge> {
        let delta = DeltaEncodedEdge::encode_delta(previous_id, self.neighbor_id)?;
        Some(DeltaEncodedEdge {
            delta,
            type_offset: self.edge_type_offset,
            data_len: self.edge_data.len() as u16,
        })
    }

    /// Create a CompactEdgeRecord from a delta-encoded edge.
    ///
    /// Returns `None` if the delta represents an overflow (need full ID).
    pub fn from_delta_encoded(
        previous_id: i64,
        delta_edge: DeltaEncodedEdge,
        edge_data: Vec<u8>,
    ) -> Option<Self> {
        let neighbor_id = DeltaEncodedEdge::decode_delta(previous_id, delta_edge.delta)?;
        Some(Self {
            neighbor_id,
            edge_type_offset: delta_edge.type_offset,
            edge_data,
        })
    }

    /// Analyze a slice of edges to determine if delta encoding is beneficial.
    ///
    /// Returns true if the average gap between neighbor IDs is less than 256,
    /// which indicates good compression potential.
    pub fn should_use_delta_encoding(edges: &[Self]) -> bool {
        if edges.len() < 2 {
            return false;
        }

        let mut total_gap = 0u64;
        let mut previous_id = edges[0].neighbor_id;

        for edge in edges.iter().skip(1) {
            let gap = previous_id.abs_diff(edge.neighbor_id);
            total_gap += gap;
            previous_id = edge.neighbor_id;
        }

        let avg_gap = total_gap / (edges.len() as u64 - 1);
        avg_gap < 256 // Threshold for good compression
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_encoding_small_gap() {
        let prev = 100i64;
        let curr = 105i64;
        let delta = DeltaEncodedEdge::encode_delta(prev, curr).unwrap();
        assert_eq!(delta, 5);

        let decoded = DeltaEncodedEdge::decode_delta(prev, delta).unwrap();
        assert_eq!(decoded, curr);
    }

    #[test]
    fn test_delta_encoding_zero_gap() {
        let prev = 100i64;
        let curr = 100i64;
        let delta = DeltaEncodedEdge::encode_delta(prev, curr).unwrap();
        assert_eq!(delta, 0);

        let decoded = DeltaEncodedEdge::decode_delta(prev, delta).unwrap();
        assert_eq!(decoded, curr);
    }

    #[test]
    fn test_delta_encoding_large_gap() {
        let prev = 100i64;
        let curr = 1000000i64;
        let delta = DeltaEncodedEdge::encode_delta(prev, curr).unwrap();
        assert_eq!(delta, 999900);

        let decoded = DeltaEncodedEdge::decode_delta(prev, delta).unwrap();
        assert_eq!(decoded, curr);
    }

    #[test]
    fn test_delta_encoding_overflow() {
        let prev = 0i64;
        let curr = u32::MAX as i64 + 1000i64;
        let delta = DeltaEncodedEdge::encode_delta(prev, curr).unwrap();
        assert_eq!(delta, DeltaEncodedEdge::MAX_DELTA);

        // Overflow case - decode should return None
        let decoded = DeltaEncodedEdge::decode_delta(prev, delta);
        assert!(decoded.is_none());
    }

    #[test]
    fn test_delta_encoding_negative_ids() {
        let prev = -1i64;
        let curr = 100i64;
        let delta = DeltaEncodedEdge::encode_delta(prev, curr);
        assert!(delta.is_none());

        let prev = 100i64;
        let curr = -1i64;
        let delta = DeltaEncodedEdge::encode_delta(prev, curr);
        assert!(delta.is_none());
    }

    #[test]
    fn test_compact_edge_to_delta_encoded() {
        let edge = CompactEdgeRecord {
            neighbor_id: 105,
            edge_type_offset: 42,
            edge_data: vec![1, 2, 3],
        };

        let prev_id = 100i64;
        let delta_edge = edge.to_delta_encoded(prev_id).unwrap();

        assert_eq!(delta_edge.delta, 5);
        assert_eq!(delta_edge.type_offset, 42);
        assert_eq!(delta_edge.data_len, 3);
        assert!(!delta_edge.is_overflow());
    }

    #[test]
    fn test_compact_edge_from_delta_encoded() {
        let delta_edge = DeltaEncodedEdge {
            delta: 5,
            type_offset: 42,
            data_len: 3,
        };

        let prev_id = 100i64;
        let edge_data = vec![1, 2, 3];

        let edge = CompactEdgeRecord::from_delta_encoded(prev_id, delta_edge, edge_data).unwrap();

        assert_eq!(edge.neighbor_id, 105);
        assert_eq!(edge.edge_type_offset, 42);
        assert_eq!(edge.edge_data, vec![1, 2, 3]);
    }

    #[test]
    fn test_should_use_delta_encoding_sequential() {
        let edges = vec![
            CompactEdgeRecord {
                neighbor_id: 100,
                edge_type_offset: 1,
                edge_data: vec![],
            },
            CompactEdgeRecord {
                neighbor_id: 101,
                edge_type_offset: 1,
                edge_data: vec![],
            },
            CompactEdgeRecord {
                neighbor_id: 102,
                edge_type_offset: 1,
                edge_data: vec![],
            },
        ];

        assert!(CompactEdgeRecord::should_use_delta_encoding(&edges));
    }

    #[test]
    fn test_should_use_delta_encoding_sparse() {
        let edges = vec![
            CompactEdgeRecord {
                neighbor_id: 100,
                edge_type_offset: 1,
                edge_data: vec![],
            },
            CompactEdgeRecord {
                neighbor_id: 10000,
                edge_type_offset: 1,
                edge_data: vec![],
            },
            CompactEdgeRecord {
                neighbor_id: 20000,
                edge_type_offset: 1,
                edge_data: vec![],
            },
        ];

        assert!(!CompactEdgeRecord::should_use_delta_encoding(&edges));
    }

    #[test]
    fn test_should_use_delta_encoding_single_edge() {
        let edges = vec![CompactEdgeRecord {
            neighbor_id: 100,
            edge_type_offset: 1,
            edge_data: vec![],
        }];

        assert!(!CompactEdgeRecord::should_use_delta_encoding(&edges));
    }

    #[test]
    fn test_delta_edge_overflow_check() {
        let overflow_edge = DeltaEncodedEdge {
            delta: DeltaEncodedEdge::MAX_DELTA,
            type_offset: 1,
            data_len: 0,
        };

        assert!(overflow_edge.is_overflow());

        let normal_edge = DeltaEncodedEdge {
            delta: 100,
            type_offset: 1,
            data_len: 0,
        };

        assert!(!normal_edge.is_overflow());
    }
}
