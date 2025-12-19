//! Core V2 node record structure and basic constructor

use crate::backend::native::{NativeBackendError, NativeResult, NodeFlags};

/// V2 node record with direct references to adjacency clusters.
#[derive(Debug, Clone)]
pub struct NodeRecordV2 {
    pub id: i64,
    pub flags: NodeFlags,
    pub kind: String,
    pub name: String,
    pub data: serde_json::Value,
    pub outgoing_cluster_offset: crate::backend::native::types::FileOffset,
    pub outgoing_cluster_size: u32,
    pub outgoing_edge_count: u32,
    pub incoming_cluster_offset: crate::backend::native::types::FileOffset,
    pub incoming_cluster_size: u32,
    pub incoming_edge_count: u32,
}

impl NodeRecordV2 {
    /// Create a new V2 node record with the given basic properties
    pub fn new(id: i64, kind: String, name: String, data: serde_json::Value) -> Self {
        Self {
            id,
            flags: NodeFlags::empty(),
            kind,
            name,
            data,
            outgoing_cluster_offset: 0,
            outgoing_cluster_size: 0,
            outgoing_edge_count: 0,
            incoming_cluster_offset: 0,
            incoming_cluster_size: 0,
            incoming_edge_count: 0,
        }
    }
}