//! Compact on-disk representation for edges inside a cluster.

use super::cluster_trace::Direction;
use crate::backend::native::v2::string_table::StringTable;
use crate::backend::native::{EdgeRecord, NativeBackendError, NativeResult};

/// Packed edge header for compressed edge representation.
///
/// Layout: delta (32 bits) + type_offset (16 bits) + data_len (12 bits) + flags (4 bits)
/// Total: 64 bits (8 bytes) instead of ~24 bytes in separate fields.
///
/// Flag bits:
/// - bit 0: has_data (data payload present)
/// - bit 1: large_delta (delta > u16, use extended encoding)
/// - bit 2: null_data (data is JSON null)
/// - bit 3: reserved
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PackedEdgeHeader {
    bits: u64,
}

impl PackedEdgeHeader {
    /// Flag bit positions
    pub const FLAG_HAS_DATA: u8 = 0;
    pub const FLAG_LARGE_DELTA: u8 = 1;
    pub const FLAG_NULL_DATA: u8 = 2;
    pub const FLAG_RESERVED: u8 = 3;

    /// Bit field positions and sizes
    const DELTA_SHIFT: u32 = 32;
    const TYPE_OFFSET_SHIFT: u32 = 16;
    const DATA_LEN_SHIFT: u32 = 4;
    const FLAGS_MASK: u64 = 0xF;

    /// Pack fields into a 64-bit header.
    ///
    /// # Arguments
    /// * `delta` - Delta value (u32)
    /// * `type_offset` - Edge type offset (u16, must be <= u16::MAX)
    /// * `data_len` - Data payload length (u16, must be < 4096 for packing)
    /// * `flags` - 4-bit flags value
    ///
    /// # Panics
    /// Panics if data_len >= 4096 or flags >= 16
    pub fn pack(delta: u32, type_offset: u16, data_len: u16, flags: u8) -> Self {
        assert!(
            data_len < 4096,
            "data_len too large for bit-packing: {} (max 4095)",
            data_len
        );
        assert!(flags < 16, "flags too large: {} (max 15)", flags);

        let bits = ((delta as u64) << Self::DELTA_SHIFT)
            | ((type_offset as u64) << Self::TYPE_OFFSET_SHIFT)
            | ((data_len as u64 & 0xFFF) << Self::DATA_LEN_SHIFT)
            | ((flags as u64) & Self::FLAGS_MASK);

        Self { bits }
    }

    /// Unpack the delta field.
    pub fn unpack_delta(&self) -> u32 {
        (self.bits >> Self::DELTA_SHIFT) as u32
    }

    /// Unpack the type_offset field.
    pub fn unpack_type_offset(&self) -> u16 {
        ((self.bits >> Self::TYPE_OFFSET_SHIFT) & 0xFFFF) as u16
    }

    /// Unpack the data_len field.
    pub fn unpack_data_len(&self) -> u16 {
        ((self.bits >> Self::DATA_LEN_SHIFT) & 0xFFF) as u16
    }

    /// Unpack the flags field.
    pub fn unpack_flags(&self) -> u8 {
        (self.bits & Self::FLAGS_MASK) as u8
    }

    /// Check if a specific flag is set.
    pub fn has_flag(&self, flag: u8) -> bool {
        (self.unpack_flags() & (1 << flag)) != 0
    }

    /// Get the raw bits.
    pub fn as_bits(&self) -> u64 {
        self.bits
    }

    /// Create from raw bits.
    pub fn from_bits(bits: u64) -> Self {
        Self { bits }
    }
}

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

    /// Convert to packed header format.
    ///
    /// Returns `None` if data_len >= 4096 (too large for 12-bit field).
    pub fn to_packed_header(&self, flags: u8) -> Option<PackedEdgeHeader> {
        if self.data_len >= 4096 {
            return None; // Too large for 12-bit data_len field
        }

        Some(PackedEdgeHeader::pack(
            self.delta,
            self.type_offset,
            self.data_len,
            flags,
        ))
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

    /// Check if this edge can use small data optimization (data <= 8 bytes).
    ///
    /// Small data can be inlined in the packed header extension, avoiding
    /// separate data payload allocation.
    pub fn can_use_small_data_optimization(&self) -> bool {
        self.edge_data.len() <= 8
    }

    /// Calculate flags for this edge record.
    pub fn calculate_flags(&self) -> u8 {
        let mut flags = 0u8;

        // Flag 0: has_data
        if !self.edge_data.is_empty() {
            flags |= 1 << PackedEdgeHeader::FLAG_HAS_DATA;
        }

        // Flag 1: large_delta (not applicable here, used during encoding)
        // Flag 2: null_data (empty data represents null in our serialization)
        if self.edge_data.is_empty() {
            flags |= 1 << PackedEdgeHeader::FLAG_NULL_DATA;
        }

        flags
    }

    /// Estimate the packed size of this edge record.
    ///
    /// Returns the size in bytes if using packed format.
    pub fn packed_size(&self) -> usize {
        let header_size = 8; // PackedEdgeHeader is 8 bytes

        // If data can be inlined (<= 8 bytes), no extra payload
        // Otherwise, we need the full data payload
        if self.can_use_small_data_optimization() {
            header_size
        } else {
            header_size + self.edge_data.len()
        }
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

    // Bit-packing tests

    #[test]
    fn test_packed_edge_header_pack_unpack() {
        let delta = 12345u32;
        let type_offset = 65535u16;
        let data_len = 4095u16;
        let flags = 15u8;

        let header = PackedEdgeHeader::pack(delta, type_offset, data_len, flags);

        assert_eq!(header.unpack_delta(), delta);
        assert_eq!(header.unpack_type_offset(), type_offset);
        assert_eq!(header.unpack_data_len(), data_len);
        assert_eq!(header.unpack_flags(), flags);
    }

    #[test]
    fn test_packed_edge_header_flags() {
        // 0b0110 means bits 1 and 2 are set (FLAG_LARGE_DELTA and FLAG_NULL_DATA)
        let header = PackedEdgeHeader::pack(0, 0, 0, 0b0110);

        assert!(!header.has_flag(PackedEdgeHeader::FLAG_HAS_DATA));
        assert!(header.has_flag(PackedEdgeHeader::FLAG_LARGE_DELTA));
        assert!(header.has_flag(PackedEdgeHeader::FLAG_NULL_DATA));
        assert!(!header.has_flag(PackedEdgeHeader::FLAG_RESERVED));
    }

    #[test]
    #[should_panic(expected = "data_len too large")]
    fn test_packed_edge_header_data_len_too_large() {
        PackedEdgeHeader::pack(0, 0, 4096, 0);
    }

    #[test]
    fn test_packed_edge_header_boundary_values() {
        // Test minimum values
        let header_min = PackedEdgeHeader::pack(0, 0, 0, 0);
        assert_eq!(header_min.unpack_delta(), 0);
        assert_eq!(header_min.unpack_type_offset(), 0);
        assert_eq!(header_min.unpack_data_len(), 0);
        assert_eq!(header_min.unpack_flags(), 0);

        // Test maximum values
        let header_max = PackedEdgeHeader::pack(u32::MAX, u16::MAX, 4095, 15);
        assert_eq!(header_max.unpack_delta(), u32::MAX);
        assert_eq!(header_max.unpack_type_offset(), u16::MAX);
        assert_eq!(header_max.unpack_data_len(), 4095);
        assert_eq!(header_max.unpack_flags(), 15);
    }

    #[test]
    fn test_packed_edge_header_bits_conversion() {
        let original = PackedEdgeHeader::pack(12345, 678, 123, 5);
        let bits = original.as_bits();
        let restored = PackedEdgeHeader::from_bits(bits);

        assert_eq!(original.unpack_delta(), restored.unpack_delta());
        assert_eq!(original.unpack_type_offset(), restored.unpack_type_offset());
        assert_eq!(original.unpack_data_len(), restored.unpack_data_len());
        assert_eq!(original.unpack_flags(), restored.unpack_flags());
    }

    #[test]
    fn test_delta_edge_to_packed_header() {
        let delta_edge = DeltaEncodedEdge {
            delta: 1000,
            type_offset: 42,
            data_len: 123,
        };

        let flags = 0b0101;
        let packed = delta_edge.to_packed_header(flags).unwrap();

        assert_eq!(packed.unpack_delta(), 1000);
        assert_eq!(packed.unpack_type_offset(), 42);
        assert_eq!(packed.unpack_data_len(), 123);
        assert_eq!(packed.unpack_flags(), flags);
    }

    #[test]
    fn test_delta_edge_to_packed_header_data_too_large() {
        let delta_edge = DeltaEncodedEdge {
            delta: 1000,
            type_offset: 42,
            data_len: 4096, // Too large for 12-bit field
        };

        let result = delta_edge.to_packed_header(0);
        assert!(result.is_none());
    }

    #[test]
    fn test_compact_edge_small_data_optimization() {
        // Edge with small data (<= 8 bytes)
        let edge_small = CompactEdgeRecord {
            neighbor_id: 100,
            edge_type_offset: 1,
            edge_data: vec![1, 2, 3, 4, 5],
        };

        assert!(edge_small.can_use_small_data_optimization());
        assert_eq!(edge_small.packed_size(), 8); // Only header, data inlined

        // Edge with large data (> 8 bytes)
        let edge_large = CompactEdgeRecord {
            neighbor_id: 100,
            edge_type_offset: 1,
            edge_data: vec![1; 100],
        };

        assert!(!edge_large.can_use_small_data_optimization());
        assert_eq!(edge_large.packed_size(), 108); // Header + 100 bytes data
    }

    #[test]
    fn test_compact_edge_calculate_flags() {
        // Edge with data
        let edge_with_data = CompactEdgeRecord {
            neighbor_id: 100,
            edge_type_offset: 1,
            edge_data: vec![1, 2, 3],
        };

        let flags = edge_with_data.calculate_flags();
        assert!(flags & (1 << PackedEdgeHeader::FLAG_HAS_DATA) != 0);
        assert!(flags & (1 << PackedEdgeHeader::FLAG_NULL_DATA) == 0);

        // Edge without data (null)
        let edge_null = CompactEdgeRecord {
            neighbor_id: 100,
            edge_type_offset: 1,
            edge_data: vec![],
        };

        let flags = edge_null.calculate_flags();
        assert!(flags & (1 << PackedEdgeHeader::FLAG_HAS_DATA) == 0);
        assert!(flags & (1 << PackedEdgeHeader::FLAG_NULL_DATA) != 0);
    }

    #[test]
    fn test_compact_edge_packed_size_vs_unpacked() {
        let edge = CompactEdgeRecord {
            neighbor_id: 100,
            edge_type_offset: 42,
            edge_data: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };

        let unpacked_size = edge.size_bytes();
        let packed_size = edge.packed_size();

        // Packed should be smaller or equal
        // Unpacked: 8 (neighbor_id) + 2 (type_offset) + 2 (data_len) + 8 (data) = 20
        // Packed: 8 (header) with data inlined
        assert!(packed_size <= unpacked_size);
        assert_eq!(unpacked_size, 20);
        assert_eq!(packed_size, 8);
    }
}
