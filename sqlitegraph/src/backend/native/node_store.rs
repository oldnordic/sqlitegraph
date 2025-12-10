//! Node record management for native backend.
//!
//! This module handles node record serialization, deserialization, and O(1)
//! node lookup by ID through direct offset calculation.

use std::io::Read;

use crate::backend::native::constants::*;
use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::types::*;

/// Node store manages node records in the graph file
pub struct NodeStore<'a> {
    graph_file: &'a mut GraphFile,
    /// In-memory index mapping node_id -> actual file offset
    node_index: std::collections::HashMap<NativeNodeId, FileOffset>,
}

impl<'a> NodeStore<'a> {
    /// Create a new node store from a graph file
    pub fn new(graph_file: &'a mut GraphFile) -> Self {
        Self {
            graph_file,
            node_index: std::collections::HashMap::new(),
        }
    }

    /// Write a node record to the file
    pub fn write_node(&mut self, node: &NodeRecord) -> NativeResult<()> {
        // Validate node record basic fields (but not ID range since we're writing it)
        self.validate_node_fields(node)?;

        // Serialize node record
        let serialized = self.serialize_node(node)?;

        // Simple append strategy: always write at the end of the file
        // This avoids circular dependencies and ensures consistency
        let offset = self.graph_file.file_size()?;

        // Ensure file is large enough for this node
        let node_end = offset + serialized.len() as u64;
        self.graph_file.grow(serialized.len() as u64)?;

        // Write to file
        self.graph_file.write_bytes(offset, &serialized)?;

        // Store the actual offset in our index
        self.node_index.insert(node.id, offset);

        // Update header if this is a new node
        if node.id as u64 > self.graph_file.header().node_count {
            self.graph_file.header_mut().node_count = node.id as u64;
            // Persist header changes to disk
            self.graph_file.flush()?;
        }

        Ok(())
    }

    /// Validate node record fields except for ID range (used when writing)
    fn validate_node_fields(&self, node: &NodeRecord) -> NativeResult<()> {
        if node.id <= 0 {
            return Err(NativeBackendError::InvalidNodeId {
                id: node.id,
                max_id: 0,
            });
        }

        if node.kind.len() > super::constants::node::MAX_STRING_LENGTH_U32 as usize {
            return Err(NativeBackendError::RecordTooLarge {
                size: node.kind.len() as u32,
                max_size: super::constants::node::MAX_STRING_LENGTH_U32,
            });
        }

        if node.name.len() > super::constants::node::MAX_STRING_LENGTH_U32 as usize {
            return Err(NativeBackendError::RecordTooLarge {
                size: node.name.len() as u32,
                max_size: super::constants::node::MAX_STRING_LENGTH_U32,
            });
        }

        Ok(())
    }

    /// Read a node record from the file
    pub fn read_node(&mut self, node_id: NativeNodeId) -> NativeResult<NodeRecord> {
        let header = self.graph_file.header();

        if node_id <= 0 || node_id > header.node_count as NativeNodeId {
            return Err(NativeBackendError::InvalidNodeId {
                id: node_id,
                max_id: header.node_count as NativeNodeId,
            });
        }

        // Get actual offset from index, or estimate if not indexed yet
        let offset = if let Some(&stored_offset) = self.node_index.get(&node_id) {
            stored_offset
        } else {
            // Node not in index (e.g., when reading an existing file)
            // Fall back to sequential search from the beginning
            self.rebuild_index_for_node(node_id)?
        };

        self.read_node_internal(node_id, offset)
    }

    /// Internal method to read a node record from a specific offset
    fn read_node_internal(
        &mut self,
        node_id: NativeNodeId,
        offset: FileOffset,
    ) -> NativeResult<NodeRecord> {
        // First read the node header to get the record size
        let mut header_buffer = vec![0u8; 32]; // Enough for version + flags + id + length fields
        self.graph_file.read_bytes(offset, &mut header_buffer)?;

        // Parse the header to get string lengths
        if header_buffer[0] != 1 {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id,
                reason: "Invalid node record version".to_string(),
            });
        }

        // Extract flags (4 bytes)
        let _flags_bytes = [
            header_buffer[1],
            header_buffer[2],
            header_buffer[3],
            header_buffer[4],
        ];

        // Node ID comes next (8 bytes), then string lengths
        let kind_len = u16::from_be_bytes([header_buffer[13], header_buffer[14]]) as usize;
        let name_len = u16::from_be_bytes([header_buffer[15], header_buffer[16]]) as usize;
        let data_len = u32::from_be_bytes([
            header_buffer[17],
            header_buffer[18],
            header_buffer[19],
            header_buffer[20],
        ]) as usize;

        // Calculate total record size exactly as serialize_node writes it
        let total_size = 1 + 4 + 8 + 2 + 2 + 4 + kind_len + name_len + data_len + 8 + 4 + 8 + 4; // version + flags + id + kind_len + name_len + data_len + strings + adjacency

        // Read the complete node record
        let mut buffer = vec![0u8; total_size];
        self.graph_file.read_bytes(offset, &mut buffer)?;
        if buffer.len() != total_size {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id,
                reason: format!(
                    "Buffer size mismatch: expected {}, got {}",
                    total_size,
                    buffer.len()
                ),
            });
        }

        // Deserialize node record
        self.deserialize_node(node_id, &buffer)
    }

    /// Rebuild index up to the target node by scanning from the beginning
    fn rebuild_index_for_node(&mut self, target_id: NativeNodeId) -> NativeResult<FileOffset> {
        let mut current_offset = self.graph_file.header().node_data_offset;
        let file_size = self.graph_file.file_size()?;

        for id in 1..=target_id {
            // Stop if we've reached the end of the file
            if current_offset >= file_size {
                return Err(NativeBackendError::InvalidNodeId {
                    id: target_id,
                    max_id: id - 1,
                });
            }

            // Store offset for this node
            self.node_index.insert(id, current_offset);

            // Read the node header to get its size
            if current_offset + 32 > file_size {
                return Err(NativeBackendError::CorruptNodeRecord {
                    node_id: id,
                    reason: "Node header extends beyond file".to_string(),
                });
            }

            let mut header_buffer = vec![0u8; 32];
            self.graph_file
                .read_bytes(current_offset, &mut header_buffer)?;

            if header_buffer[0] != 1 {
                return Err(NativeBackendError::CorruptNodeRecord {
                    node_id: id,
                    reason: "Invalid node record version".to_string(),
                });
            }

            // Extract string lengths to calculate size
            let kind_len = u16::from_be_bytes([header_buffer[13], header_buffer[14]]) as usize;
            let name_len = u16::from_be_bytes([header_buffer[15], header_buffer[16]]) as usize;
            let data_len = u32::from_be_bytes([
                header_buffer[17],
                header_buffer[18],
                header_buffer[19],
                header_buffer[20],
            ]) as usize;

            // Calculate total record size
            let total_size = 1 + 4 + 8 + 2 + 2 + 4 + kind_len + name_len + data_len + 8 + 4 + 8 + 4;
            current_offset += total_size as u64;
        }

        // Return the offset for the target node
        Ok(self.node_index[&target_id])
    }

    /// Serialize a node record to bytes
    fn serialize_node(&self, node: &NodeRecord) -> NativeResult<Vec<u8>> {
        let mut buffer = Vec::new();

        // Record header (version + flags)
        buffer.push(1); // Version 1
        buffer.extend_from_slice(&node.flags.0.to_be_bytes()[..4]);

        // Node ID (big-endian)
        buffer.extend_from_slice(&node.id.to_be_bytes());

        // Kind length (big-endian)
        let kind_bytes = node.kind.as_bytes();
        if kind_bytes.len() > node::MAX_STRING_LENGTH as usize {
            return Err(NativeBackendError::RecordTooLarge {
                size: kind_bytes.len() as u32,
                max_size: node::MAX_STRING_LENGTH_U32,
            });
        }
        buffer.extend_from_slice(&(kind_bytes.len() as u16).to_be_bytes());

        // Name length (big-endian)
        let name_bytes = node.name.as_bytes();
        if name_bytes.len() > node::MAX_STRING_LENGTH as usize {
            return Err(NativeBackendError::RecordTooLarge {
                size: name_bytes.len() as u32,
                max_size: node::MAX_STRING_LENGTH_U32,
            });
        }
        buffer.extend_from_slice(&(name_bytes.len() as u16).to_be_bytes());

        // Data length (big-endian)
        let data_bytes = serde_json::to_vec(&node.data)?;
        if data_bytes.len() > node::MAX_DATA_LENGTH as usize {
            return Err(NativeBackendError::RecordTooLarge {
                size: data_bytes.len() as u32,
                max_size: node::MAX_DATA_LENGTH,
            });
        }
        buffer.extend_from_slice(&(data_bytes.len() as u32).to_be_bytes());

        // Variable-length fields
        buffer.extend_from_slice(kind_bytes);
        buffer.extend_from_slice(name_bytes);
        buffer.extend_from_slice(&data_bytes);

        // Adjacency metadata
        buffer.extend_from_slice(&node.outgoing_offset.to_be_bytes());
        buffer.extend_from_slice(&node.outgoing_count.to_be_bytes());
        buffer.extend_from_slice(&node.incoming_offset.to_be_bytes());
        buffer.extend_from_slice(&node.incoming_count.to_be_bytes());

        Ok(buffer)
    }

    /// Deserialize a node record from bytes
    fn deserialize_node(&self, node_id: NativeNodeId, buffer: &[u8]) -> NativeResult<NodeRecord> {
        if buffer.len() < node::FIXED_HEADER_SIZE {
            return Err(NativeBackendError::BufferTooSmall {
                size: buffer.len(),
                min_size: node::FIXED_HEADER_SIZE,
            });
        }

        let mut offset = 0;

        // Skip record header (1 byte)
        offset += 1;

        // Read node flags
        let flags_bytes = &buffer[offset..offset + 4];
        let flags = NodeFlags(u32::from_be_bytes([
            flags_bytes[0],
            flags_bytes[1],
            flags_bytes[2],
            flags_bytes[3],
        ]));
        offset += 4;

        // Read node ID and validate
        let id_bytes = &buffer[offset..offset + node::ID_SIZE];
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
        offset += node::ID_SIZE;

        if id != node_id {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id,
                reason: format!("Expected node ID {}, found {}", node_id, id),
            });
        }

        // Read kind length
        let kind_len_bytes = &buffer[offset..offset + 2];
        let kind_len = u16::from_be_bytes([kind_len_bytes[0], kind_len_bytes[1]]) as usize;
        offset += 2;

        // Read name length
        let name_len_bytes = &buffer[offset..offset + 2];
        let name_len = u16::from_be_bytes([name_len_bytes[0], name_len_bytes[1]]) as usize;
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
        let required_size = offset + kind_len + name_len + data_len + node::ADJACENCY_METADATA_SIZE;
        if buffer.len() < required_size {
            return Err(NativeBackendError::BufferTooSmall {
                size: buffer.len(),
                min_size: required_size,
            });
        }

        // Read kind
        let kind_bytes = &buffer[offset..offset + kind_len];
        let kind = std::str::from_utf8(kind_bytes)?.to_string();
        offset += kind_len;

        // Read name
        let name_bytes = &buffer[offset..offset + name_len];
        let name = std::str::from_utf8(name_bytes)?.to_string();
        offset += name_len;

        // Read data
        let data_bytes = &buffer[offset..offset + data_len];
        let data = serde_json::from_slice(data_bytes)?;
        offset += data_len;

        // Read adjacency metadata
        let outgoing_offset_bytes = &buffer[offset..offset + 8];
        let outgoing_offset = u64::from_be_bytes([
            outgoing_offset_bytes[0],
            outgoing_offset_bytes[1],
            outgoing_offset_bytes[2],
            outgoing_offset_bytes[3],
            outgoing_offset_bytes[4],
            outgoing_offset_bytes[5],
            outgoing_offset_bytes[6],
            outgoing_offset_bytes[7],
        ]);
        offset += 8;

        let outgoing_count_bytes = &buffer[offset..offset + 4];
        let outgoing_count = u32::from_be_bytes([
            outgoing_count_bytes[0],
            outgoing_count_bytes[1],
            outgoing_count_bytes[2],
            outgoing_count_bytes[3],
        ]);
        offset += 4;

        let incoming_offset_bytes = &buffer[offset..offset + 8];
        let incoming_offset = u64::from_be_bytes([
            incoming_offset_bytes[0],
            incoming_offset_bytes[1],
            incoming_offset_bytes[2],
            incoming_offset_bytes[3],
            incoming_offset_bytes[4],
            incoming_offset_bytes[5],
            incoming_offset_bytes[6],
            incoming_offset_bytes[7],
        ]);
        offset += 8;

        let incoming_count_bytes = &buffer[offset..offset + 4];
        let incoming_count = u32::from_be_bytes([
            incoming_count_bytes[0],
            incoming_count_bytes[1],
            incoming_count_bytes[2],
            incoming_count_bytes[3],
        ]);

        Ok(NodeRecord {
            id,
            flags,
            kind,
            name,
            data,
            outgoing_offset,
            outgoing_count,
            incoming_offset,
            incoming_count,
        })
    }

    /// Get the maximum valid node ID
    pub fn max_node_id(&self) -> NativeNodeId {
        self.graph_file.header().node_count as NativeNodeId
    }

    /// Allocate a new node ID
    pub fn allocate_node_id(&mut self) -> NativeNodeId {
        let current_count = self.graph_file.header().node_count;
        let new_id = current_count + 1;
        self.graph_file.header_mut().node_count = new_id;
        new_id as NativeNodeId
    }

    /// Validate node consistency across the file
    pub fn validate_consistency(&mut self) -> NativeResult<()> {
        let max_id = self.max_node_id();

        for node_id in 1..=max_id {
            match self.read_node(node_id) {
                Ok(node) => {
                    // Validate adjacency metadata consistency
                    if node.outgoing_count > 0 && node.outgoing_offset == 0 {
                        return Err(NativeBackendError::InconsistentAdjacency {
                            node_id,
                            count: node.outgoing_count,
                            direction: "outgoing".to_string(),
                            file_count: 0,
                        });
                    }

                    if node.incoming_count > 0 && node.incoming_offset == 0 {
                        return Err(NativeBackendError::InconsistentAdjacency {
                            node_id,
                            count: node.incoming_count,
                            direction: "incoming".to_string(),
                            file_count: 0,
                        });
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
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_test_graph_file() -> (GraphFile, NamedTempFile) {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let graph_file = GraphFile::create(path).unwrap();
        (graph_file, temp_file)
    }

    #[test]
    fn test_node_roundtrip() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut node_store = NodeStore::new(&mut graph_file);

        // Create test node
        let test_data = serde_json::json!({
            "language": "rust",
            "lines": 42
        });

        let original_node =
            NodeRecord::new(1, "Function".to_string(), "main".to_string(), test_data);

        // Write node
        node_store.write_node(&original_node).unwrap();

        // Read node back
        let read_node = node_store.read_node(1).unwrap();

        assert_eq!(original_node.id, read_node.id);
        assert_eq!(original_node.kind, read_node.kind);
        assert_eq!(original_node.name, read_node.name);
        assert_eq!(original_node.data, read_node.data);
    }

    #[test]
    fn test_node_id_allocation() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut node_store = NodeStore::new(&mut graph_file);

        let node_id1 = node_store.allocate_node_id();
        let node_id2 = node_store.allocate_node_id();

        assert_eq!(node_id1, 1);
        assert_eq!(node_id2, 2);
        assert_eq!(node_store.max_node_id(), 2);
    }

    #[test]
    fn test_invalid_node_id() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut node_store = NodeStore::new(&mut graph_file);

        let result = node_store.read_node(999);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            NativeBackendError::InvalidNodeId { .. }
        ));
    }

    #[test]
    fn test_high_id_node_record_layout() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut node_store = NodeStore::new(&mut graph_file);

        // Create nodes with varying sizes to cause offset drift
        let base_data = serde_json::json!({});

        // Insert nodes up to ID 5000 with different size strings
        for i in 1..=5000 {
            let kind = if i % 3 == 0 { "Function" } else { "Variable" };
            let name = format!("node_{:04}_with_long_name_to_increase_size", i);

            let node = NodeRecord::new(i, kind.to_string(), name, base_data.clone());

            node_store.write_node(&node).unwrap();
        }

        // Now try to read node ID 4097 - this should work with our fix
        let result = node_store.read_node(4097);
        assert!(result.is_ok());

        let node = result.unwrap();
        assert_eq!(node.id, 4097);
        assert!(node.kind == "Function" || node.kind == "Variable");
        assert!(node.name.contains("4097"));
    }
}
