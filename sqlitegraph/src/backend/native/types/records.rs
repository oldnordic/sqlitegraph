//! Node and edge record structures for the native backend

use serde_json;
use super::{NativeNodeId, NativeEdgeId, NativeBackendError, EdgeFlags};

/// V2-only node record type alias for backward compatibility
pub type NodeRecord = crate::backend::native::v2::node_record_v2::NodeRecordV2;

/// Edge record structure for API compatibility (V1-style fields for operations)
/// This is converted to CompactEdgeRecord for V2 storage
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
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

        if self.edge_type.len() > super::super::constants::edge::MAX_STRING_LENGTH as usize {
            return Err(NativeBackendError::RecordTooLarge {
                size: self.edge_type.len() as u32,
                max_size: super::super::constants::edge::MAX_STRING_LENGTH as u32,
            });
        }

        Ok(())
    }
}