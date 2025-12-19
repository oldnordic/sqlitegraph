//! Complex deserialization with comprehensive error handling for V2 node records

use crate::backend::native::{NativeBackendError, NativeResult};

use super::NodeRecordV2;

impl NodeRecordV2 {
    /// Deserialize a V2 node record from binary data
    pub fn deserialize(bytes: &[u8]) -> NativeResult<Self> {
        const MIN_HEADER_SIZE: usize = 1 + 4 + 8 + 2 + 2 + 4; // version + flags + id + length fields
        const CLUSTER_METADATA_SIZE: usize = 32; // 16 bytes per direction

        if bytes.len() < MIN_HEADER_SIZE {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: MIN_HEADER_SIZE,
            });
        }

        let mut offset = 0;
        if bytes[offset] != 2 {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: 0,
                reason: format!("Invalid V2 node record version {}", bytes[offset]),
            });
        }
        offset += 1;

        // Check bounds before accessing flags
        if offset + 4 > bytes.len() {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: offset + 4,
            });
        }
        let flags = crate::backend::native::NodeFlags(u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]));
        offset += 4;

        // Check bounds before accessing id
        if offset + 8 > bytes.len() {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: offset + 8,
            });
        }
        let id = i64::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);
        offset += 8;

        // Check bounds before accessing length fields
        if offset + 2 + 2 + 4 > bytes.len() {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: offset + 2 + 2 + 4,
            });
        }
        let kind_len = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        offset += 2;
        let name_len = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]) as usize;
        offset += 2;
        let data_len = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]) as usize;
        offset += 4;

        // Check bounds for variable-length data and cluster metadata
        let required_size = offset + kind_len + name_len + data_len + CLUSTER_METADATA_SIZE;
        if bytes.len() < required_size {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: required_size,
            });
        }

        // Check bounds before accessing kind
        if offset + kind_len > bytes.len() {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: offset + kind_len,
            });
        }
        let kind = std::str::from_utf8(&bytes[offset..offset + kind_len])?.to_string();
        offset += kind_len;

        // Check bounds before accessing name
        if offset + name_len > bytes.len() {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: offset + name_len,
            });
        }
        let name = std::str::from_utf8(&bytes[offset..offset + name_len])?.to_string();
        offset += name_len;

        // Check bounds before accessing data
        if offset + data_len > bytes.len() {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: offset + data_len,
            });
        }
        let data_bytes = &bytes[offset..offset + data_len];
        let data = serde_json::from_slice(data_bytes).unwrap_or(serde_json::Value::Null);
        offset += data_len;

        // Check bounds before accessing outgoing_cluster_offset
        if offset + 8 > bytes.len() {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: offset + 8,
            });
        }
        let outgoing_cluster_offset = u64::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);
        offset += 8;

        // Check bounds before accessing outgoing_cluster_size
        if offset + 4 > bytes.len() {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: offset + 4,
            });
        }
        let outgoing_cluster_size = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);
        offset += 4;

        // Check bounds before accessing outgoing_edge_count
        if offset + 4 > bytes.len() {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: offset + 4,
            });
        }
        let outgoing_edge_count = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);
        offset += 4;

        // Check bounds before accessing incoming_cluster_offset
        if offset + 8 > bytes.len() {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: offset + 8,
            });
        }
        let incoming_cluster_offset = u64::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ]);
        offset += 8;

        // Check bounds before accessing incoming_cluster_size
        if offset + 4 > bytes.len() {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: offset + 4,
            });
        }
        let incoming_cluster_size = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);
        offset += 4;

        // Check bounds before accessing incoming_edge_count (final field)
        if offset + 4 > bytes.len() {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: offset + 4,
            });
        }
        let incoming_edge_count = u32::from_be_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ]);

        Ok(Self {
            id,
            flags,
            kind,
            name,
            data,
            outgoing_cluster_offset,
            outgoing_cluster_size,
            outgoing_edge_count,
            incoming_cluster_offset,
            incoming_cluster_size,
            incoming_edge_count,
        })
    }
}