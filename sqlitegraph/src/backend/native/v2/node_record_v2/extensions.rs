//! Utility functions and trait extensions for V2 node records

use crate::backend::native::types::FileOffset;
use crate::backend::native::{NativeBackendError, NativeResult};

use super::NodeRecordV2;

// Phase 31: V2 is now default - remove feature gating
/// Parse V2 header lengths from binary buffer
pub fn parse_v2_header_lengths(buf: &[u8]) -> NativeResult<(u16, u16, u32)> {
    const MIN_HEADER: usize = 21;
    const CLUSTER_METADATA_SIZE: usize = 32;
    if buf.len() < MIN_HEADER {
        return Err(NativeBackendError::BufferTooSmall {
            size: buf.len(),
            min_size: MIN_HEADER,
        });
    }
    if buf[0] != 2 {
        if buf[0] == 1 || buf[0] == 0 {
            // Version 1 or 0 in a V2 file indicates uninitialized or V1-formatted slot
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: 0,
                reason: format!(
                    "V2 file contains uninitialized slot (version={}) - node may not be properly written",
                    buf[0]
                ),
            });
        } else {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: 0,
                reason: format!("Invalid V2 node version {}", buf[0]),
            });
        }
    }

    let kind_len = u16::from_be_bytes([buf[13], buf[14]]);
    let name_len = u16::from_be_bytes([buf[15], buf[16]]);
    let data_len = u32::from_be_bytes([buf[17], buf[18], buf[19], buf[20]]);

    // Ensure lengths can be represented in usize for later allocations.
    let mut total: usize = 21;
    total = total
        .checked_add(kind_len as usize)
        .and_then(|v| v.checked_add(name_len as usize))
        .and_then(|v| v.checked_add(data_len as usize))
        .and_then(|v| v.checked_add(CLUSTER_METADATA_SIZE))
        .ok_or(NativeBackendError::RecordTooLarge {
            size: u32::MAX,
            max_size: u32::MAX,
        })?;

    let _ = total;

    // The final size check happens when the caller reads the full record.
    Ok((kind_len, name_len, data_len))
}

/// Extension trait for V2 node record operations
pub trait NodeRecordV2Ext {
    fn new(id: i64, kind: String, name: String, data: serde_json::Value) -> Self;
    fn set_outgoing_cluster(&mut self, offset: FileOffset, size: u32, count: u32);
    fn set_incoming_cluster(&mut self, offset: FileOffset, size: u32, count: u32);
    fn has_efficient_adjacency(&self) -> bool;
}

impl NodeRecordV2Ext for NodeRecordV2 {
    fn new(id: i64, kind: String, name: String, data: serde_json::Value) -> Self {
        Self {
            id,
            flags: crate::backend::native::NodeFlags(0),
            kind,
            name,
            data,
            outgoing_edge_count: 0,
            incoming_edge_count: 0,
            outgoing_cluster_offset: 0,
            incoming_cluster_offset: 0,
            outgoing_cluster_size: 0,
            incoming_cluster_size: 0,
        }
    }

    fn set_outgoing_cluster(&mut self, offset: FileOffset, size: u32, count: u32) {
        self.outgoing_edge_count = count;
        self.outgoing_cluster_offset = offset;
        self.outgoing_cluster_size = size;
    }

    fn set_incoming_cluster(&mut self, offset: FileOffset, size: u32, count: u32) {
        self.incoming_edge_count = count;
        self.incoming_cluster_offset = offset;
        self.incoming_cluster_size = size;
    }

    fn has_efficient_adjacency(&self) -> bool {
        self.outgoing_edge_count > 0 && self.outgoing_cluster_offset > 0
    }
}