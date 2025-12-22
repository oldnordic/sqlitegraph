//! GraphFile node and edge access operations
//!
//! This module contains methods for reading and writing node and edge records,
//! calculating offsets, and managing record statistics.

use crate::backend::native::{
    constants::edge::EDGE_SLOT_SIZE,
    graph_file::NodeEdgeAccessManager,
    graph_file::header::HeaderManager,
    types::{EdgeRecord, NativeEdgeId, NativeNodeId, NodeRecord},
};

use super::GraphFile;

impl GraphFile {
    /// Calculate file offset for edge
    pub fn calculate_edge_offset(&self, edge_id: NativeEdgeId) -> u64 {
        let base_offset = self.persistent_header().edge_data_offset;
        base_offset + ((edge_id - 1) as u64 * EDGE_SLOT_SIZE)
    }

    /// Read node record at specific node ID
    pub fn read_node_at(
        &mut self,
        node_id: NativeNodeId,
    ) -> crate::backend::native::types::NativeResult<NodeRecord> {
        NodeEdgeAccessManager::read_node_at(self, node_id)
    }

    /// Write node record at specific node ID
    pub fn write_node_at(
        &mut self,
        node_id: NativeNodeId,
        node: &NodeRecord,
    ) -> crate::backend::native::types::NativeResult<()> {
        use crate::backend::native::constants::node::NODE_SLOT_SIZE;

        if node_id == 0 {
            return Err(
                crate::backend::native::types::NativeBackendError::InvalidNodeId {
                    id: node_id,
                    max_id: 0,
                },
            );
        }

        let offset = self.calculate_node_offset(node_id);
        let node_bytes = node.serialize();

        if node_bytes.len() > NODE_SLOT_SIZE as usize {
            return Err(
                crate::backend::native::types::NativeBackendError::RecordTooLarge {
                    size: node_bytes.len() as u32,
                    max_size: NODE_SLOT_SIZE as u32,
                },
            );
        }

        self.write_bytes(offset, &node_bytes)?;
        Ok(())
    }

    /// Read edge record at specific offset
    pub fn read_edge_at_offset(
        &mut self,
        offset: u64,
    ) -> crate::backend::native::types::NativeResult<EdgeRecord> {
        NodeEdgeAccessManager::read_edge_at_offset(self, offset)
    }

    /// Read edge record at specific edge ID
    pub fn read_edge_at(
        &mut self,
        edge_id: NativeEdgeId,
    ) -> crate::backend::native::types::NativeResult<EdgeRecord> {
        let offset = self.calculate_edge_offset(edge_id);
        self.read_edge_at_offset(offset)
    }

    /// Write edge record at specific edge ID
    pub fn write_edge_at(
        &mut self,
        edge_id: NativeEdgeId,
        edge: &EdgeRecord,
    ) -> crate::backend::native::types::NativeResult<()> {
        NodeEdgeAccessManager::write_edge_at(self, edge_id, edge)
    }

    /// Check if node exists
    pub fn node_exists(
        &mut self,
        node_id: NativeNodeId,
    ) -> crate::backend::native::types::NativeResult<bool> {
        NodeEdgeAccessManager::node_exists(self, node_id)
    }

    /// Get node statistics
    pub fn get_node_statistics(
        &self,
    ) -> crate::backend::native::types::NativeResult<
        crate::backend::native::graph_file::header::ClusterUtilization,
    > {
        HeaderManager::get_node_statistics(&self.persistent_header)
    }

    /// Get edge statistics
    pub fn get_edge_statistics(
        &self,
    ) -> crate::backend::native::types::NativeResult<
        crate::backend::native::graph_file::header::ClusterUtilization,
    > {
        HeaderManager::get_edge_statistics(&self.persistent_header)
    }

    /// Get total allocated nodes
    pub fn allocated_node_count(&self) -> u32 {
        self.persistent_header().node_count as u32
    }

    /// Get total allocated edges
    pub fn allocated_edge_count(&self) -> u32 {
        self.persistent_header().edge_count as u32
    }

    /// Get free space offset
    pub fn free_space_offset(&self) -> u64 {
        self.persistent_header().free_space_offset
    }

    /// Get node data offset
    pub fn node_data_offset(&self) -> u64 {
        self.persistent_header().node_data_offset
    }

    /// Get edge data offset
    pub fn edge_data_offset(&self) -> u64 {
        self.persistent_header().edge_data_offset
    }

    /// Get outgoing cluster offset
    pub fn outgoing_cluster_offset(&self) -> u64 {
        self.persistent_header().outgoing_cluster_offset
    }

    /// Get incoming cluster offset
    pub fn incoming_cluster_offset(&self) -> u64 {
        self.persistent_header().incoming_cluster_offset
    }

    /// Calculate node slot offset
    pub fn calculate_node_offset(&self, node_id: NativeNodeId) -> u64 {
        NodeEdgeAccessManager::calculate_node_offset(self, node_id)
    }

    /// Validate node record
    pub fn validate_node_record(&self, node: &NodeRecord) -> bool {
        NodeEdgeAccessManager::validate_node_record(node)
    }

    /// Validate edge record
    pub fn validate_edge_record(&self, edge: &EdgeRecord) -> bool {
        NodeEdgeAccessManager::validate_edge_record(edge)
    }

    /// Get node record size for serialization
    pub fn get_node_record_size(
        &self,
        node: &NodeRecord,
    ) -> crate::backend::native::types::NativeResult<usize> {
        NodeEdgeAccessManager::get_node_record_size(node)
    }

    /// Get edge record size for serialization
    pub fn get_edge_record_size(
        &self,
        edge: &EdgeRecord,
    ) -> crate::backend::native::types::NativeResult<usize> {
        NodeEdgeAccessManager::get_edge_record_size(edge)
    }

    /// Check if edge slot is allocated
    pub fn is_edge_slot_allocated(&self, edge_id: NativeEdgeId) -> bool {
        NodeEdgeAccessManager::is_edge_slot_allocated(&self.persistent_header, edge_id)
    }

    /// Check if node slot is allocated
    pub fn is_node_slot_allocated(&self, node_id: NativeNodeId) -> bool {
        NodeEdgeAccessManager::is_node_slot_allocated(&self.persistent_header, node_id)
    }

    /// Get next available node ID
    pub fn get_next_node_id(&self) -> NativeNodeId {
        (self.persistent_header().node_count + 1) as NativeNodeId
    }

    /// Get next available edge ID
    pub fn get_next_edge_id(&self) -> NativeEdgeId {
        (self.persistent_header().edge_count + 1) as NativeEdgeId
    }

    /// Reserve node slots
    pub fn reserve_node_slots(
        &mut self,
        count: u32,
    ) -> crate::backend::native::types::NativeResult<()> {
        NodeEdgeAccessManager::reserve_node_slots(self, count)
    }

    /// Reserve edge slots
    pub fn reserve_edge_slots(
        &mut self,
        count: u32,
    ) -> crate::backend::native::types::NativeResult<()> {
        NodeEdgeAccessManager::reserve_edge_slots(self, count)
    }
}
