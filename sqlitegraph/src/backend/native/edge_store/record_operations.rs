//! Edge record operations module
//!
//! This module provides core edge record CRUD operations (Create, Read, Update, Delete),
//! serialization/deserialization functionality, and edge validation for the native backend.
//! It handles the low-level binary format operations for edge storage and retrieval.

use crate::backend::native::constants::edge;
use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::types::{NativeResult, NativeEdgeId, FileOffset, EdgeRecord, EdgeFlags, NativeBackendError};
use super::capacity_coordinator::EdgeCapacityCoordinator;

/// Edge record operations manager for CRUD and serialization
pub struct EdgeRecordOperations<'a> {
    graph_file: &'a mut GraphFile,
}

impl<'a> EdgeRecordOperations<'a> {
    /// Create a new edge record operations manager
    ///
    /// # Arguments
    /// * `graph_file` - Mutable reference to the graph file
    pub fn new(graph_file: &'a mut GraphFile) -> Self {
        Self { graph_file }
    }

    /// Write an edge record to the file
    ///
    /// Serializes and writes an edge record to the appropriate location in the file.
    /// Uses fixed-size slots for simplicity and performance.
    ///
    /// # Arguments
    /// * `edge` - The edge record to write
    ///
    /// # Returns
    /// `Ok(())` if successful, `Err` with details if failed
    pub fn write_edge(&mut self, edge: &EdgeRecord) -> NativeResult<()> {
        // Validate edge fields
        self.validate_edge_fields(edge)?;

        // CRITICAL: Ensure capacity before writing to prevent "beyond end of file" errors
        self.ensure_capacity_for_edge(edge.id)?;

        // Serialize edge record
        let buffer = self.serialize_edge(edge)?;

        // Calculate offset for this edge (fixed-size slot)
        let offset = self.edge_offset(edge.id);
        let fixed_slot_size = 256usize;

        // Ensure buffer fits in fixed slot
        if buffer.len() > fixed_slot_size {
            return Err(NativeBackendError::RecordTooLarge {
                size: buffer.len() as u32,
                max_size: fixed_slot_size as u32,
            });
        }

        // Write to file
        self.graph_file.write_bytes(offset, &buffer)?;

        Ok(())
    }

    /// Read an edge record from the file
    ///
    /// Reads and deserializes an edge record from the file by ID.
    /// Uses fixed-size slots and validates the record format.
    ///
    /// # Arguments
    /// * `edge_id` - The ID of the edge to read
    ///
    /// # Returns
    /// The deserialized edge record if successful
    ///
    /// # Errors
    /// - `InvalidEdgeId` if the edge ID is out of range
    /// - `CorruptEdgeRecord` if the record format is invalid
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

    /// Validate edge record fields
    ///
    /// Checks that all edge fields are within valid limits and formats.
    ///
    /// # Arguments
    /// * `edge` - The edge record to validate
    ///
    /// # Returns
    /// `Ok(())` if valid, `Err` with validation details if invalid
    fn validate_edge_fields(&self, edge: &EdgeRecord) -> NativeResult<()> {
        // Validate edge ID is positive
        if edge.id <= 0 {
            return Err(NativeBackendError::InvalidEdgeId {
                id: edge.id,
                max_id: 0,
            });
        }

        // Validate node IDs are positive
        if edge.from_id <= 0 || edge.to_id <= 0 {
            return Err(NativeBackendError::InvalidNodeId {
                id: if edge.from_id <= 0 { edge.from_id } else { edge.to_id },
                max_id: 0,
            });
        }

        // Validate edge type length
        if edge.edge_type.len() > edge::MAX_STRING_LENGTH_U32 as usize {
            return Err(NativeBackendError::RecordTooLarge {
                size: edge.edge_type.len() as u32,
                max_size: edge::MAX_STRING_LENGTH_U32,
            });
        }

        Ok(())
    }

    /// Calculate file offset for an edge record
    ///
    /// Uses fixed-size edge records (256 bytes) for simplicity and performance.
    ///
    /// # Arguments
    /// * `edge_id` - The edge ID to calculate offset for
    ///
    /// # Returns
    /// File offset where the edge record is stored
    fn edge_offset(&self, edge_id: NativeEdgeId) -> FileOffset {
        let base_offset = self.graph_file.persistent_header().edge_data_offset;
        // Use fixed-size edge records for simplicity: 256 bytes per edge
        // This ensures we have enough space for any edge and keeps offset calculation simple
        base_offset + ((edge_id - 1) as u64 * 256)
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

    /// Update an existing edge record
    ///
    /// Validates the updated edge and writes it to the file.
    /// The edge ID must already exist and be valid.
    ///
    /// # Arguments
    /// * `edge` - The updated edge record
    ///
    /// # Returns
    /// `Ok(())` if successful, `Err` with details if failed
    ///
    /// # Errors
    /// - `InvalidEdgeId` if the edge ID doesn't exist
    /// - Validation errors from `validate_edge_fields`
    /// - Write errors from `write_edge`
    pub fn update_edge(&mut self, edge: &EdgeRecord) -> NativeResult<()> {
        // Validate the edge ID exists
        let header = self.graph_file.header();
        if edge.id <= 0 || edge.id > header.edge_count as NativeEdgeId {
            return Err(NativeBackendError::InvalidEdgeId {
                id: edge.id,
                max_id: header.edge_count as NativeEdgeId,
            });
        }

        // Validate and write the updated edge
        self.validate_edge_fields(edge)?;
        self.write_edge(edge)?;

        Ok(())
    }

    /// Delete an edge record by marking it as deleted
    ///
    /// Note: This implementation marks edges as deleted by setting a flag
    /// rather than actually removing the data to maintain fixed offset calculations.
    ///
    /// # Arguments
    /// * `edge_id` - The ID of the edge to delete
    ///
    /// # Returns
    /// `Ok(())` if successful, `Err` with details if failed
    pub fn delete_edge(&mut self, edge_id: NativeEdgeId) -> NativeResult<()> {
        // First read the existing edge
        let mut edge = self.read_edge(edge_id)?;

        // Mark as deleted by setting the deleted flag
        edge.flags.0 |= 0x0001; // Use first bit as deleted flag

        // Write back the updated edge
        self.write_edge(&edge)?;

        Ok(())
    }

    /// Check if an edge is marked as deleted
    ///
    /// # Arguments
    /// * `edge_id` - The ID of the edge to check
    ///
    /// # Returns
    /// `true` if the edge is marked as deleted, `false` otherwise
    pub fn is_edge_deleted(&mut self, edge_id: NativeEdgeId) -> NativeResult<bool> {
        let edge = self.read_edge(edge_id)?;
        Ok((edge.flags.0 & 0x0001) != 0)
    }

    /// Ensure file has capacity for this edge
    ///
    /// This method ensures the underlying file is large enough to store
    /// an edge at the calculated offset, growing the file if necessary.
    ///
    /// # Arguments
    /// * `edge_id` - The edge ID to ensure capacity for
    ///
    /// # Returns
    /// `Ok(())` if capacity is ensured, `Err` with details if failed
    fn ensure_capacity_for_edge(&mut self, edge_id: NativeEdgeId) -> NativeResult<()> {
        let mut coordinator = EdgeCapacityCoordinator::new(self.graph_file);
        coordinator.ensure_capacity_for_edge_id(edge_id as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::constants::edge;
    use tempfile::NamedTempFile;

    fn create_test_graph_file() -> (GraphFile, NamedTempFile) {
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");

        let graph_file = GraphFile::create(temp_file.path()).unwrap();
        // GraphFile::create() handles initialization automatically

        (graph_file, temp_file)
    }

    fn create_test_edge(graph_file: &mut GraphFile, from_id: i64, to_id: i64) -> EdgeRecord {
        use crate::backend::native::edge_store::id_management::EdgeIdManager;

        // Allocate edge ID properly
        let mut id_manager = EdgeIdManager::new(graph_file);
        let edge_id = id_manager.allocate_edge_id();

        EdgeRecord {
            id: edge_id,
            from_id,
            to_id,
            edge_type: "TEST_EDGE".to_string(),
            flags: EdgeFlags(0),
            data: serde_json::json!({"test": "data"}),
        }
    }

    // Helper for validation tests that need specific edge IDs
    fn create_test_edge_with_id(id: NativeEdgeId, from_id: i64, to_id: i64) -> EdgeRecord {
        EdgeRecord {
            id,
            from_id,
            to_id,
            edge_type: "TEST_EDGE".to_string(),
            flags: EdgeFlags(0),
            data: serde_json::json!({"test": "data"}),
        }
    }

    #[test]
    fn test_edge_record_serialization() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        let edge = create_test_edge(&mut graph_file, 100, 200);
        let operations = EdgeRecordOperations::new(&mut graph_file);

        let serialized = operations.serialize_edge(&edge).unwrap();

        assert!(serialized.len() > edge::FIXED_HEADER_SIZE);
        assert_eq!(serialized[0], 1); // Version
    }

    #[test]
    fn test_edge_record_roundtrip() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        let original_edge = create_test_edge(&mut graph_file, 100, 200);
        let allocated_edge_id = original_edge.id;
        let mut operations = EdgeRecordOperations::new(&mut graph_file);

        // Write edge
        operations.write_edge(&original_edge).unwrap();

        // Read edge back
        let read_edge = operations.read_edge(allocated_edge_id).unwrap();

        assert_eq!(original_edge.id, read_edge.id);
        assert_eq!(original_edge.from_id, read_edge.from_id);
        assert_eq!(original_edge.to_id, read_edge.to_id);
        assert_eq!(original_edge.edge_type, read_edge.edge_type);
        assert_eq!(original_edge.flags, read_edge.flags);
        assert_eq!(original_edge.data, read_edge.data);
    }

    #[test]
    fn test_edge_validation() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let operations = EdgeRecordOperations::new(&mut graph_file);

        // Valid edge
        let valid_edge = create_test_edge_with_id(1, 100, 200);
        assert!(operations.validate_edge_fields(&valid_edge).is_ok());

        // Invalid edge ID
        let mut invalid_edge = create_test_edge_with_id(0, 100, 200);
        assert!(operations.validate_edge_fields(&invalid_edge).is_err());

        // Invalid node ID
        invalid_edge.id = 1;
        invalid_edge.from_id = -1;
        assert!(operations.validate_edge_fields(&invalid_edge).is_err());

        // Edge type too long
        invalid_edge.from_id = 100;
        invalid_edge.edge_type = "x".repeat(edge::MAX_STRING_LENGTH_U32 as usize + 1);
        assert!(operations.validate_edge_fields(&invalid_edge).is_err());
    }

    #[test]
    fn test_edge_offset_calculation() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Get base_offset before creating operations to avoid borrowing issues
        let base_offset = graph_file.persistent_header().edge_data_offset;

        let operations = EdgeRecordOperations::new(&mut graph_file);

        assert_eq!(operations.edge_offset(1), base_offset);
        assert_eq!(operations.edge_offset(2), base_offset + 256);
        assert_eq!(operations.edge_offset(10), base_offset + (9 * 256));
    }

    #[test]
    fn test_edge_update() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Write initial edge
        let mut edge = create_test_edge(&mut graph_file, 100, 200);
        let allocated_edge_id = edge.id;
        let mut operations = EdgeRecordOperations::new(&mut graph_file);
        operations.write_edge(&edge).unwrap();

        // Update edge
        edge.edge_type = "UPDATED_EDGE".to_string();
        edge.data = serde_json::json!({"updated": true});
        operations.update_edge(&edge).unwrap();

        // Read and verify update
        let read_edge = operations.read_edge(allocated_edge_id).unwrap();
        assert_eq!(read_edge.edge_type, "UPDATED_EDGE");
        assert_eq!(read_edge.data, serde_json::json!({"updated": true}));
    }

    #[test]
    fn test_edge_deletion() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Write edge
        let edge = create_test_edge(&mut graph_file, 100, 200);
        let allocated_edge_id = edge.id;
        let mut operations = EdgeRecordOperations::new(&mut graph_file);
        operations.write_edge(&edge).unwrap();

        // Edge should not be deleted initially
        assert!(!operations.is_edge_deleted(allocated_edge_id).unwrap());

        // Delete edge
        operations.delete_edge(allocated_edge_id).unwrap();

        // Edge should now be marked as deleted
        assert!(operations.is_edge_deleted(allocated_edge_id).unwrap());
    }

    #[test]
    fn test_serialization_with_null_data() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        let mut edge = create_test_edge(&mut graph_file, 100, 200);
        let allocated_edge_id = edge.id;
        edge.data = serde_json::Value::Null;
        let mut operations = EdgeRecordOperations::new(&mut graph_file);

        // Test serialization works with null data
        let serialized = operations.serialize_edge(&edge).unwrap();
        assert!(!serialized.is_empty(), "Serialization should not be empty even with null data");

        // Test full edge write/read roundtrip with null data
        operations.write_edge(&edge).unwrap();
        let read_edge = operations.read_edge(allocated_edge_id).unwrap();

        assert_eq!(read_edge.id, allocated_edge_id);
        assert_eq!(read_edge.from_id, 100);
        assert_eq!(read_edge.to_id, 200);
        assert_eq!(read_edge.edge_type, "TEST_EDGE");
        assert_eq!(read_edge.data, serde_json::Value::Null);
    }

    #[test]
    fn test_read_nonexistent_edge() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut operations = EdgeRecordOperations::new(&mut graph_file);

        // Try to read edge that doesn't exist
        let result = operations.read_edge(999);
        assert!(result.is_err());

        match result.unwrap_err() {
            NativeBackendError::InvalidEdgeId { id, max_id } => {
                assert_eq!(id, 999);
                assert_eq!(max_id, 0); // No edges allocated yet
            }
            _ => panic!("Expected InvalidEdgeId error"),
        }
    }
}