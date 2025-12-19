//! Edge record serialization and deserialization
//!
//! Handles binary format conversion for edge records including serialization to
//! bytes, deserialization from bytes, and format validation.

use crate::backend::native::constants::edge;
use crate::backend::native::types::{NativeResult, EdgeRecord, NativeEdgeId, NativeBackendError};

/// Edge record serialization utilities
pub struct EdgeSerializer;

impl EdgeSerializer {
    /// Create a new edge serializer
    pub fn new() -> Self {
        Self
    }

    /// Serialize an edge record to bytes
    ///
    /// Converts an edge record into the binary format for storage.
    /// Includes version header, flags, IDs, and variable-length fields.
    ///
    /// # Arguments
    /// * `edge` - The edge record to serialize
    ///
    /// # Returns
    /// Serialized byte buffer
    ///
    /// # Errors
    /// - `RecordTooLarge` if fields exceed size limits
    /// - `JsonError` if data serialization fails
    pub fn serialize_edge(&self, edge: &EdgeRecord) -> NativeResult<Vec<u8>> {
        let mut buffer = Vec::new();

        // Record header (version + flags)
        buffer.push(1); // Version 1
        buffer.extend_from_slice(&edge.flags.0.to_be_bytes()[..2]);

        // Edge ID (big-endian)
        buffer.extend_from_slice(&edge.id.to_be_bytes());

        // From node ID (big-endian)
        buffer.extend_from_slice(&edge.from_id.to_be_bytes());

        // To node ID (big-endian)
        buffer.extend_from_slice(&edge.to_id.to_be_bytes());

        // Edge type length (big-endian)
        let edge_type_bytes = edge.edge_type.as_bytes();
        if edge_type_bytes.len() > edge::MAX_STRING_LENGTH as usize {
            return Err(NativeBackendError::RecordTooLarge {
                size: edge_type_bytes.len() as u32,
                max_size: edge::MAX_STRING_LENGTH_U32,
            });
        }
        buffer.extend_from_slice(&(edge_type_bytes.len() as u16).to_be_bytes());

        // Data length (big-endian)
        // HOT PATH FIX: Only serialize edge data if it's non-empty/null
        let data_bytes = if edge.data == serde_json::Value::Null {
            Vec::new() // Empty bytes for null data (common case)
        } else {
            serde_json::to_vec(&edge.data)?
        };
        if data_bytes.len() > edge::MAX_DATA_LENGTH as usize {
            return Err(NativeBackendError::RecordTooLarge {
                size: data_bytes.len() as u32,
                max_size: edge::MAX_DATA_LENGTH,
            });
        }
        buffer.extend_from_slice(&(data_bytes.len() as u32).to_be_bytes());

        // Variable-length fields
        buffer.extend_from_slice(edge_type_bytes);
        buffer.extend_from_slice(&data_bytes);

        Ok(buffer)
    }

    /// Deserialize an edge record from bytes
    ///
    /// Converts binary data back into an edge record struct.
    /// Validates format consistency and field integrity.
    ///
    /// # Arguments
    /// * `edge_id` - Expected edge ID for validation
    /// * `buffer` - Binary data to deserialize
    ///
    /// # Returns
    /// Deserialized edge record
    ///
    /// # Errors
    /// - `BufferTooSmall` if buffer doesn't contain complete header
    /// - `CorruptEdgeRecord` if format is invalid or ID doesn't match
    /// - `JsonError` if data deserialization fails
    pub fn deserialize_edge(&self, edge_id: NativeEdgeId, buffer: &[u8]) -> NativeResult<EdgeRecord> {
        if buffer.len() < edge::FIXED_HEADER_SIZE {
            return Err(NativeBackendError::BufferTooSmall {
                size: buffer.len(),
                min_size: edge::FIXED_HEADER_SIZE,
            });
        }

        let mut offset = 0;

        // Skip record header (1 byte)
        offset += 1;

        // Read edge flags
        let flags_bytes = &buffer[offset..offset + 2];
        let flags = crate::backend::native::types::EdgeFlags(u16::from_be_bytes([flags_bytes[0], flags_bytes[1]]));
        offset += 2;

        // Read edge ID and validate
        let id_bytes = &buffer[offset..offset + edge::ID_SIZE];
        let id = i64::from_be_bytes([
            id_bytes[0], id_bytes[1], id_bytes[2], id_bytes[3],
            id_bytes[4], id_bytes[5], id_bytes[6], id_bytes[7],
        ]);
        offset += edge::ID_SIZE;

        if id != edge_id {
            return Err(NativeBackendError::CorruptEdgeRecord {
                edge_id,
                reason: format!("Expected edge ID {}, found {}", edge_id, id),
            });
        }

        // Read from node ID
        let from_bytes = &buffer[offset..offset + edge::FROM_ID_SIZE];
        let from_id = i64::from_be_bytes([
            from_bytes[0], from_bytes[1], from_bytes[2], from_bytes[3],
            from_bytes[4], from_bytes[5], from_bytes[6], from_bytes[7],
        ]);
        offset += edge::FROM_ID_SIZE;

        // Read to node ID
        let to_bytes = &buffer[offset..offset + edge::TO_ID_SIZE];
        let to_id = i64::from_be_bytes([
            to_bytes[0], to_bytes[1], to_bytes[2], to_bytes[3],
            to_bytes[4], to_bytes[5], to_bytes[6], to_bytes[7],
        ]);
        offset += edge::TO_ID_SIZE;

        // Read edge type length
        let type_len_bytes = &buffer[offset..offset + 2];
        let edge_type_len = u16::from_be_bytes([type_len_bytes[0], type_len_bytes[1]]) as usize;
        offset += 2;

        // Read data length
        let data_len_bytes = &buffer[offset..offset + 4];
        let data_len = u32::from_be_bytes([
            data_len_bytes[0], data_len_bytes[1], data_len_bytes[2], data_len_bytes[3],
        ]) as usize;
        offset += 4;

        // Validate we have enough bytes for remaining fields
        if buffer.len() < offset + edge_type_len + data_len {
            return Err(NativeBackendError::BufferTooSmall {
                size: buffer.len(),
                min_size: offset + edge_type_len + data_len,
            });
        }

        // Read edge type
        let edge_type_bytes = &buffer[offset..offset + edge_type_len];
        let edge_type = std::str::from_utf8(edge_type_bytes)?.to_string();
        offset += edge_type_len;

        // Read data
        let data_bytes = &buffer[offset..offset + data_len];
        let data = if data_len == 0 {
            // Empty data represents null
            serde_json::Value::Null
        } else {
            serde_json::from_slice(data_bytes)
                .map_err(|e| NativeBackendError::JsonError(e.into()))?
        };

        Ok(EdgeRecord {
            id,
            from_id,
            to_id,
            edge_type,
            flags,
            data,
        })
    }
}

impl Default for EdgeSerializer {
    fn default() -> Self {
        Self::new()
    }
}