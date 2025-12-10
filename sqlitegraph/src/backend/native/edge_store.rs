//! Edge record management and adjacency layout for native backend.
//!
//! This module handles edge record serialization, deserialization, and manages
//! the adjacency list layout for efficient neighbor lookups.

use super::node_store::NodeStore;
use crate::backend::native::constants::*;
use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::types::*;

/// Edge store manages edge records and adjacency layout in the graph file
pub struct EdgeStore<'a> {
    graph_file: &'a mut GraphFile,
}

impl<'a> EdgeStore<'a> {
    /// Create a new edge store from a graph file
    pub fn new(graph_file: &'a mut GraphFile) -> Self {
        Self { graph_file }
    }

    /// Write an edge record to the file
    pub fn write_edge(&mut self, edge: &EdgeRecord) -> NativeResult<()> {
        // Validate edge record - check node references against current node count
        self.validate_edge_fields(edge)?;

        // Serialize edge record
        let serialized = self.serialize_edge(edge)?;

        // Calculate offset where this edge should be written (fixed-size slot)
        let offset = self.edge_offset(edge.id);
        let fixed_slot_size = 256u64;

        // Ensure file is large enough for the fixed-size edge slot
        let edge_end = offset + fixed_slot_size;
        let current_file_size = self.graph_file.file_size()?;
        if edge_end > current_file_size {
            self.graph_file.grow(edge_end - current_file_size)?;
        }

        // Pad serialized data to fixed size
        let mut buffer = serialized;
        buffer.resize(fixed_slot_size as usize, 0);

        // Write to file
        self.graph_file.write_bytes(offset, &buffer)?;

        // Update node adjacency metadata
        self.update_node_adjacency(&edge)?;

        // Update header if this is a new edge
        if edge.id as u64 > self.graph_file.header().edge_count {
            self.graph_file.header_mut().edge_count = edge.id as u64;
            // Persist header changes to disk
            self.graph_file.flush()?;
        }

        Ok(())
    }

    /// Update node adjacency metadata when an edge is written
    fn update_node_adjacency(&mut self, edge: &EdgeRecord) -> NativeResult<()> {
        let mut node_store = NodeStore::new(self.graph_file);

        // Update source node (outgoing)
        let mut source_node = node_store.read_node(edge.from_id)?;
        if source_node.outgoing_count == 0 {
            source_node.outgoing_offset = edge.id as FileOffset;
        }
        source_node.outgoing_count += 1;
        node_store.write_node(&source_node)?;

        // Update target node (incoming)
        let mut target_node = node_store.read_node(edge.to_id)?;
        if target_node.incoming_count == 0 {
            target_node.incoming_offset = edge.id as FileOffset;
        }
        target_node.incoming_count += 1;
        node_store.write_node(&target_node)?;

        Ok(())
    }

    /// Validate edge record fields except for edge ID range (used when writing)
    fn validate_edge_fields(&self, edge: &EdgeRecord) -> NativeResult<()> {
        // Validate edge ID
        if edge.id <= 0 {
            return Err(NativeBackendError::InvalidEdgeId {
                id: edge.id,
                max_id: 0,
            });
        }

        // Validate node references against current node count
        let max_node_id = self.graph_file.header().node_count as NativeNodeId;
        if edge.from_id <= 0 || edge.from_id > max_node_id {
            return Err(NativeBackendError::InvalidNodeId {
                id: edge.from_id,
                max_id: max_node_id,
            });
        }

        if edge.to_id <= 0 || edge.to_id > max_node_id {
            return Err(NativeBackendError::InvalidNodeId {
                id: edge.to_id,
                max_id: max_node_id,
            });
        }

        if edge.edge_type.len() > super::constants::edge::MAX_STRING_LENGTH_U32 as usize {
            return Err(NativeBackendError::RecordTooLarge {
                size: edge.edge_type.len() as u32,
                max_size: super::constants::edge::MAX_STRING_LENGTH_U32,
            });
        }

        Ok(())
    }

    /// Read an edge record from the file
    pub fn read_edge(&mut self, edge_id: NativeEdgeId) -> NativeResult<EdgeRecord> {
        let header = self.graph_file.header();

        if edge_id <= 0 || edge_id > header.edge_count as NativeEdgeId {
            return Err(NativeBackendError::InvalidEdgeId {
                id: edge_id,
                max_id: header.edge_count as NativeEdgeId,
            });
        }

        // Calculate offset for this edge (fixed-size slot)
        let offset = self.edge_offset(edge_id);
        let fixed_slot_size = 256usize;

        // Read the entire fixed-size slot
        let mut buffer = vec![0u8; fixed_slot_size];
        self.graph_file.read_bytes(offset, &mut buffer)?;

        // Find the actual record size by looking for the end of valid data
        // Read just enough to get the header with length fields
        if buffer.len() < 33 {
            return Err(NativeBackendError::CorruptEdgeRecord {
                edge_id,
                reason: "Edge record too short".to_string(),
            });
        }

        // Check version
        if buffer[0] != 1 {
            return Err(NativeBackendError::CorruptEdgeRecord {
                edge_id,
                reason: "Invalid edge record version".to_string(),
            });
        }

        // Extract type_len and data_len from header
        let type_len = u16::from_be_bytes([buffer[27], buffer[28]]) as usize;
        let data_len =
            u32::from_be_bytes([buffer[29], buffer[30], buffer[31], buffer[32]]) as usize;

        // Calculate actual record size
        let actual_size = 1 + 2 + 8 + 8 + 8 + 2 + 4 + type_len + data_len;

        if actual_size > fixed_slot_size {
            return Err(NativeBackendError::CorruptEdgeRecord {
                edge_id,
                reason: "Edge record too large for fixed slot".to_string(),
            });
        }

        // Truncate buffer to actual size
        buffer.truncate(actual_size);

        // Deserialize edge record
        self.deserialize_edge(edge_id, &buffer)
    }

    /// Calculate file offset for an edge record
    fn edge_offset(&self, edge_id: NativeEdgeId) -> FileOffset {
        let base_offset = self.graph_file.header().edge_data_offset;
        // Use fixed-size edge records for simplicity: 256 bytes per edge
        // This ensures we have enough space for any edge and keeps offset calculation simple
        base_offset + ((edge_id - 1) as u64 * 256)
    }

    /// Serialize an edge record to bytes
    fn serialize_edge(&self, edge: &EdgeRecord) -> NativeResult<Vec<u8>> {
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
        let data_bytes = serde_json::to_vec(&edge.data)?;
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
    fn deserialize_edge(&self, edge_id: NativeEdgeId, buffer: &[u8]) -> NativeResult<EdgeRecord> {
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
        let flags = EdgeFlags(u16::from_be_bytes([flags_bytes[0], flags_bytes[1]]));
        offset += 2;

        // Read edge ID and validate
        let id_bytes = &buffer[offset..offset + edge::ID_SIZE];
        let id = i64::from_be_bytes([
            id_bytes[0],
            id_bytes[1],
            id_bytes[2],
            id_bytes[3],
            id_bytes[4],
            id_bytes[5],
            id_bytes[6],
            id_bytes[7],
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
            from_bytes[0],
            from_bytes[1],
            from_bytes[2],
            from_bytes[3],
            from_bytes[4],
            from_bytes[5],
            from_bytes[6],
            from_bytes[7],
        ]);
        offset += edge::FROM_ID_SIZE;

        // Read to node ID
        let to_bytes = &buffer[offset..offset + edge::TO_ID_SIZE];
        let to_id = i64::from_be_bytes([
            to_bytes[0],
            to_bytes[1],
            to_bytes[2],
            to_bytes[3],
            to_bytes[4],
            to_bytes[5],
            to_bytes[6],
            to_bytes[7],
        ]);
        offset += edge::TO_ID_SIZE;

        // Read edge type length
        let type_len_bytes = &buffer[offset..offset + 2];
        let edge_type_len = u16::from_be_bytes([type_len_bytes[0], type_len_bytes[1]]) as usize;
        offset += 2;

        // Read data length
        let data_len_bytes = &buffer[offset..offset + 4];
        let data_len = u32::from_be_bytes([
            data_len_bytes[0],
            data_len_bytes[1],
            data_len_bytes[2],
            data_len_bytes[3],
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
        let data = serde_json::from_slice(data_bytes)?;

        Ok(EdgeRecord {
            id,
            from_id,
            to_id,
            edge_type,
            flags,
            data,
        })
    }

    /// Get the maximum valid edge ID
    pub fn max_edge_id(&self) -> NativeEdgeId {
        self.graph_file.header().edge_count as NativeEdgeId
    }

    /// Allocate a new edge ID
    pub fn allocate_edge_id(&mut self) -> NativeEdgeId {
        let current_count = self.graph_file.header().edge_count;
        let new_id = current_count + 1;
        self.graph_file.header_mut().edge_count = new_id;
        new_id as NativeEdgeId
    }

    /// Allocate adjacency space for a node's outgoing edges
    pub fn allocate_outgoing_adjacency(
        &mut self,
        _node_id: NativeNodeId,
        count: u32,
    ) -> NativeResult<FileOffset> {
        if count == 0 {
            return Ok(0);
        }

        // For simplicity, allocate at the end of the file
        let file_size = self.graph_file.file_size()?;
        let offset = file_size.max(self.graph_file.header().edge_data_offset);

        // Ensure file is large enough for the edges
        let estimated_edge_size = 128; // Rough estimate per edge
        let required_space = count as u64 * estimated_edge_size;

        if file_size < offset + required_space {
            self.graph_file.grow(required_space)?;
        }

        Ok(offset)
    }

    /// Allocate adjacency space for a node's incoming edges
    pub fn allocate_incoming_adjacency(
        &mut self,
        _node_id: NativeNodeId,
        count: u32,
    ) -> NativeResult<FileOffset> {
        if count == 0 {
            return Ok(0);
        }

        // For simplicity, allocate at the end of the file after outgoing edges
        let file_size = self.graph_file.file_size()?;
        let offset = file_size.max(self.graph_file.header().edge_data_offset);

        // Ensure file is large enough for the edges
        let estimated_edge_size = 128; // Rough estimate per edge
        let required_space = count as u64 * estimated_edge_size;

        if file_size < offset + required_space {
            self.graph_file.grow(required_space)?;
        }

        Ok(offset)
    }

    /// Write edges to adjacency area
    pub fn write_adjacency_edges(
        &mut self,
        offset: FileOffset,
        edges: &[EdgeRecord],
    ) -> NativeResult<()> {
        let mut current_offset = offset;

        for edge in edges {
            let serialized = self.serialize_edge(edge)?;
            self.graph_file.write_bytes(current_offset, &serialized)?;
            current_offset += serialized.len() as u64;
        }

        Ok(())
    }

    /// Validate edge consistency across the file
    pub fn validate_consistency(&mut self) -> NativeResult<()> {
        let max_id = self.max_edge_id();
        let max_node_id = self.graph_file.header().node_count as NativeNodeId;

        for edge_id in 1..=max_id {
            match self.read_edge(edge_id) {
                Ok(edge) => {
                    // Validate node references
                    if edge.from_id <= 0 || edge.from_id > max_node_id {
                        return Err(NativeBackendError::InvalidNodeId {
                            id: edge.from_id,
                            max_id: max_node_id,
                        });
                    }

                    if edge.to_id <= 0 || edge.to_id > max_node_id {
                        return Err(NativeBackendError::InvalidNodeId {
                            id: edge.to_id,
                            max_id: max_node_id,
                        });
                    }

                    // Self-loops are allowed but should be flagged for consideration
                    if edge.from_id == edge.to_id {
                        // This is not an error, but could be logged in a real implementation
                    }
                }
                Err(e) => return Err(e),
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::node_store::NodeStore;
    use super::*;
    use crate::backend::native::types::NodeRecord;
    use tempfile::NamedTempFile;

    fn create_test_graph_file() -> (GraphFile, NamedTempFile) {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let graph_file = GraphFile::create(path).unwrap();
        (graph_file, temp_file)
    }

    #[test]
    fn test_edge_roundtrip() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Create test nodes first so edge validation passes
        {
            let mut node_store = NodeStore::new(&mut graph_file);
            let node1 = NodeRecord::new(
                1,
                "Function".to_string(),
                "func1".to_string(),
                serde_json::json!({}),
            );
            let node2 = NodeRecord::new(
                2,
                "Function".to_string(),
                "func2".to_string(),
                serde_json::json!({}),
            );
            node_store.write_node(&node1).unwrap();
            node_store.write_node(&node2).unwrap();
        }

        let mut edge_store = EdgeStore::new(&mut graph_file);

        // Create test edge
        let test_data = serde_json::json!({
            "weight": 1.5,
            "label": "test"
        });

        let original_edge = EdgeRecord::new(1, 1, 2, "calls".to_string(), test_data);

        // Write edge
        edge_store.write_edge(&original_edge).unwrap();

        // Read edge back
        let read_edge = edge_store.read_edge(1).unwrap();

        assert_eq!(original_edge.id, read_edge.id);
        assert_eq!(original_edge.from_id, read_edge.from_id);
        assert_eq!(original_edge.to_id, read_edge.to_id);
        assert_eq!(original_edge.edge_type, read_edge.edge_type);
        assert_eq!(original_edge.data, read_edge.data);
    }

    #[test]
    fn test_edge_id_allocation() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut edge_store = EdgeStore::new(&mut graph_file);

        let edge_id1 = edge_store.allocate_edge_id();
        let edge_id2 = edge_store.allocate_edge_id();

        assert_eq!(edge_id1, 1);
        assert_eq!(edge_id2, 2);
        assert_eq!(edge_store.max_edge_id(), 2);
    }

    #[test]
    fn test_invalid_edge_id() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut edge_store = EdgeStore::new(&mut graph_file);

        let result = edge_store.read_edge(999);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            NativeBackendError::InvalidEdgeId { .. }
        ));
    }

    #[test]
    fn test_edge_invalid_node_reference() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut edge_store = EdgeStore::new(&mut graph_file);

        // Create edge with invalid node reference
        let invalid_edge = EdgeRecord::new(
            1,
            999, // Invalid node ID
            2,
            "calls".to_string(),
            serde_json::json!({}),
        );

        let result = edge_store.write_edge(&invalid_edge);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            NativeBackendError::InvalidNodeId { .. }
        ));
    }
}
