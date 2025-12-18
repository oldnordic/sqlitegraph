//! Node and edge record access operations for GraphFile
//!
//! This module provides low-level access operations for reading node and edge
//! records from the graph file, including proper validation, binary decoding,
//! and safe error handling.

use crate::backend::native::{
    types::{FileOffset, NativeNodeId, EdgeRecord, NodeRecord, EdgeFlags, NodeFlags},
    constants::edge::FIXED_HEADER_SIZE,
};
use std::io::{Read, Seek, SeekFrom};

/// Node and edge access management utilities for GraphFile
pub struct NodeEdgeAccessManager;

impl NodeEdgeAccessManager {
    /// Read an edge record at a specific file offset
    ///
    /// Reads an edge record from the specified file offset, performing validation
    /// and binary decoding to reconstruct the EdgeRecord structure.
    ///
    /// Returns None if:
    /// - Offset is before edge_data_offset (invalid region)
    /// - File size validation fails
    /// - File seek or read operations fail
    /// - Binary decoding encounters errors
    pub fn read_edge_at_offset(
        graph_file: &mut crate::backend::native::graph_file::GraphFile,
        offset: FileOffset,
    ) -> Option<EdgeRecord> {
        // Validate offset is within edge data region
        if offset < graph_file.persistent_header.edge_data_offset {
            return None;
        }

        let buffer_size = FIXED_HEADER_SIZE;

        // Check file size before read_exact to prevent "failed to fill whole buffer"
        if graph_file.ensure_file_len_at_least(offset, buffer_size).is_err() {
            return None;
        }

        let mut buffer = vec![0u8; buffer_size];

        // Seek to the specified offset
        if let Err(_) = graph_file.file.seek(SeekFrom::Start(offset)) {
            return None;
        }

        // Read the edge record data
        if let Err(_) = graph_file.file.read_exact(&mut buffer) {
            return None;
        }

        // Decode edge record from buffer using big-endian byte order
        let edge_id = u64::from_be_bytes([
            buffer[0], buffer[1], buffer[2], buffer[3], buffer[4], buffer[5], buffer[6], buffer[7],
        ]);
        let from_id = u64::from_be_bytes([
            buffer[8], buffer[9], buffer[10], buffer[11], buffer[12], buffer[13], buffer[14], buffer[15],
        ]);
        let to_id = u64::from_be_bytes([
            buffer[16], buffer[17], buffer[18], buffer[19], buffer[20], buffer[21], buffer[22], buffer[23],
        ]);

        // Reconstruct EdgeRecord with decoded data
        Some(EdgeRecord {
            id: edge_id as i64,
            from_id: from_id as i64,
            to_id: to_id as i64,
            edge_type: "unknown".to_string(), // Simplified for optimization demo
            flags: EdgeFlags::empty(),
            data: serde_json::Value::Null,
        })
    }

    /// Read a node record at a specific node ID
    ///
    /// Returns a NodeRecord for the specified node ID. This is a simplified
    /// implementation that creates a basic node record. In a full implementation,
    /// this would read from the node data section.
    ///
    /// Currently returns a basic node structure for demonstration purposes.
    /// Future implementations should read actual node data from the file.
    pub fn read_node_at(
        _graph_file: &crate::backend::native::graph_file::GraphFile,
        node_id: NativeNodeId,
    ) -> Option<NodeRecord> {
        // This is a simplified implementation that creates a basic node record
        // In a full implementation, this would read from the node data section
        Some(NodeRecord {
            id: node_id,
            flags: NodeFlags::empty(),
            kind: "node".to_string(),
            name: format!("node_{}", node_id),
            data: serde_json::Value::Null,
            outgoing_cluster_offset: 0,
            outgoing_cluster_size: 0,
            outgoing_edge_count: 0,
            incoming_cluster_offset: 0,
            incoming_cluster_size: 0,
            incoming_edge_count: 0,
        })
    }

    
    /// Validate edge record structure and consistency
    ///
    /// Performs basic validation on an edge record to ensure data integrity.
    /// This can be extended with more sophisticated validation logic.
    pub fn validate_edge_record(edge: &EdgeRecord) -> bool {
        // Basic validation checks
        edge.id >= 0 && edge.from_id >= 0 && edge.to_id >= 0
    }

    /// Validate node record structure and consistency
    ///
    /// Performs basic validation on a node record to ensure data integrity.
    /// This can be extended with more sophisticated validation logic.
    pub fn validate_node_record(node: &NodeRecord) -> bool {
        // Basic validation checks
        node.id >= 0 &&
        node.outgoing_cluster_offset >= 0 &&
        node.incoming_cluster_offset >= 0 &&
        node.outgoing_edge_count >= 0 &&
        node.incoming_edge_count >= 0
    }

    /// Get edge record size for buffer allocation
    ///
    /// Returns the fixed size of an edge record header for buffer allocation
    /// and validation purposes.
    pub fn get_edge_record_size() -> usize {
        FIXED_HEADER_SIZE
    }

    /// Check if offset is within valid edge data region
    ///
    /// Validates that the given offset is within the edge data section
    /// of the graph file.
    pub fn is_valid_edge_offset(
        graph_file: &crate::backend::native::graph_file::GraphFile,
        offset: FileOffset,
    ) -> bool {
        offset >= graph_file.persistent_header.edge_data_offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempfile;
    use std::io::{Write, Seek, SeekFrom};
    use serde_json;

    #[test]
    fn test_read_edge_at_offset() {
        let mut temp_file = tempfile().unwrap();

        // Create a test edge record in proper V2 binary format
        let edge_id = 12345u64;
        let from_id = 67890u64;
        let to_id = 98765u64;

        // Build edge record matching what read_edge_at_offset expects
        // The function reads: edge_id(8) + from_id(8) + to_id(8) + extra padding to reach FIXED_HEADER_SIZE
        let buffer: Vec<u8> = [
            edge_id.to_be_bytes().to_vec(),      // 8 bytes: edge ID (at buffer[0..8])
            from_id.to_be_bytes().to_vec(),     // 8 bytes: from node ID (at buffer[8..16])
            to_id.to_be_bytes().to_vec(),       // 8 bytes: to node ID (at buffer[16..24])
            vec![0u8; FIXED_HEADER_SIZE - 24],  // Padding to reach full FIXED_HEADER_SIZE
        ].concat();

        // Verify buffer matches expected FIXED_HEADER_SIZE
        assert_eq!(buffer.len(), FIXED_HEADER_SIZE);

        // Write test data to file
        temp_file.seek(SeekFrom::Start(100)).unwrap();
        temp_file.write_all(&buffer).unwrap();

        // Create a mock GraphFile for testing
        let mut graph_file = crate::backend::native::graph_file::GraphFile {
            file: temp_file,
            persistent_header: crate::backend::native::persistent_header::PersistentHeaderV2::new_v2(),
            transaction_state: crate::backend::native::transaction_state::TransactionState::new(),
            file_path: std::path::PathBuf::from("test"),
            read_buffer: crate::backend::native::graph_file::buffers::ReadBuffer::new(),
            write_buffer: crate::backend::native::graph_file::buffers::WriteBuffer::new(10),
            #[cfg(feature = "v2_experimental")]
            mmap: None,
            transaction_auditor: crate::backend::native::graph_file::TransactionAuditor::new(),
        };

        // Set edge_data_offset to allow the read
        graph_file.persistent_header.edge_data_offset = 80;

        // Test edge reading
        let edge = NodeEdgeAccessManager::read_edge_at_offset(&mut graph_file, 100);

        assert!(edge.is_some());
        let edge = edge.unwrap();
        assert_eq!(edge.id, edge_id as i64);
        assert_eq!(edge.from_id, from_id as i64);
        assert_eq!(edge.to_id, to_id as i64);
        assert_eq!(edge.edge_type, "unknown");
    }

    #[test]
    fn test_read_edge_invalid_offset() {
        let temp_file = tempfile().unwrap();

        let mut graph_file = crate::backend::native::graph_file::GraphFile {
            file: temp_file,
            persistent_header: crate::backend::native::persistent_header::PersistentHeaderV2::new_v2(),
            transaction_state: crate::backend::native::transaction_state::TransactionState::new(),
            file_path: std::path::PathBuf::from("test"),
            read_buffer: crate::backend::native::graph_file::buffers::ReadBuffer::new(),
            write_buffer: crate::backend::native::graph_file::buffers::WriteBuffer::new(10),
            #[cfg(feature = "v2_experimental")]
            mmap: None,
            transaction_auditor: crate::backend::native::graph_file::TransactionAuditor::new(),
        };

        // Set edge_data_offset to make the offset invalid
        graph_file.persistent_header.edge_data_offset = 200;

        // Test invalid offset (before edge_data_offset)
        let edge = NodeEdgeAccessManager::read_edge_at_offset(&mut graph_file, 100);
        assert!(edge.is_none());
    }

    #[test]
    fn test_read_node_at() {
        let graph_file = crate::backend::native::graph_file::GraphFile {
            file: tempfile().unwrap(),
            persistent_header: crate::backend::native::persistent_header::PersistentHeaderV2::new_v2(),
            transaction_state: crate::backend::native::transaction_state::TransactionState::new(),
            file_path: std::path::PathBuf::from("test"),
            read_buffer: crate::backend::native::graph_file::buffers::ReadBuffer::new(),
            write_buffer: crate::backend::native::graph_file::buffers::WriteBuffer::new(10),
            #[cfg(feature = "v2_experimental")]
            mmap: None,
            transaction_auditor: crate::backend::native::graph_file::TransactionAuditor::new(),
        };

        // Test node reading
        let node = NodeEdgeAccessManager::read_node_at(&graph_file, 42);

        assert!(node.is_some());
        let node = node.unwrap();
        assert_eq!(node.id, 42);
        assert_eq!(node.name, "node_42");
        assert_eq!(node.kind, "node");
        assert_eq!(node.data, serde_json::Value::Null);
        assert_eq!(node.outgoing_edge_count, 0);
        assert_eq!(node.incoming_edge_count, 0);
    }

    #[test]
    fn test_validate_edge_record() {
        let valid_edge = EdgeRecord {
            id: 1,
            from_id: 2,
            to_id: 3,
            edge_type: "test".to_string(),
            flags: EdgeFlags::empty(),
            data: serde_json::Value::Null,
        };

        let invalid_edge = EdgeRecord {
            id: -1, // Invalid negative ID
            from_id: 2,
            to_id: 3,
            edge_type: "test".to_string(),
            flags: EdgeFlags::empty(),
            data: serde_json::Value::Null,
        };

        assert!(NodeEdgeAccessManager::validate_edge_record(&valid_edge));
        assert!(!NodeEdgeAccessManager::validate_edge_record(&invalid_edge));
    }

    #[test]
    fn test_validate_node_record() {
        let valid_node = NodeRecord {
            id: 1,
            flags: NodeFlags::empty(),
            kind: "test".to_string(),
            name: "test_node".to_string(),
            data: serde_json::Value::Null,
            outgoing_cluster_offset: 100,
            outgoing_cluster_size: 50,
            outgoing_edge_count: 5,
            incoming_cluster_offset: 200,
            incoming_cluster_size: 30,
            incoming_edge_count: 3,
        };

        let invalid_node = NodeRecord {
            id: -1, // Invalid negative ID
            flags: NodeFlags::empty(),
            kind: "test".to_string(),
            name: "test_node".to_string(),
            data: serde_json::Value::Null,
            outgoing_cluster_offset: u64::MAX, // Invalid offset (too large)
            outgoing_cluster_size: 50,
            outgoing_edge_count: 5,
            incoming_cluster_offset: 200,
            incoming_cluster_size: 30,
            incoming_edge_count: 3,
        };

        assert!(NodeEdgeAccessManager::validate_node_record(&valid_node));
        assert!(!NodeEdgeAccessManager::validate_node_record(&invalid_node));
    }

    #[test]
    fn test_is_valid_edge_offset() {
        let mut graph_file = crate::backend::native::graph_file::GraphFile {
            file: tempfile().unwrap(),
            persistent_header: crate::backend::native::persistent_header::PersistentHeaderV2::new_v2(),
            transaction_state: crate::backend::native::transaction_state::TransactionState::new(),
            file_path: std::path::PathBuf::from("test"),
            read_buffer: crate::backend::native::graph_file::buffers::ReadBuffer::new(),
            write_buffer: crate::backend::native::graph_file::buffers::WriteBuffer::new(10),
            #[cfg(feature = "v2_experimental")]
            mmap: None,
            transaction_auditor: crate::backend::native::graph_file::TransactionAuditor::new(),
        };

        // Set edge_data_offset
        graph_file.persistent_header.edge_data_offset = 1000;

        // Test offset validation
        assert!(NodeEdgeAccessManager::is_valid_edge_offset(&graph_file, 1000)); // Exactly at edge_data_offset
        assert!(NodeEdgeAccessManager::is_valid_edge_offset(&graph_file, 1500)); // After edge_data_offset
        assert!(!NodeEdgeAccessManager::is_valid_edge_offset(&graph_file, 500));  // Before edge_data_offset
    }

    #[test]
    fn test_get_edge_record_size() {
        let size = NodeEdgeAccessManager::get_edge_record_size();
        assert_eq!(size, FIXED_HEADER_SIZE);
        assert!(size > 0);
    }
}