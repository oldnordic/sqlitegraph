//! Edge record CRUD operations
//!
//! Provides the core Create, Read, Update, Delete operations for edge records
//! with proper validation, capacity management, and error handling.

use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::types::{NativeResult, NativeEdgeId, EdgeRecord};

use super::validation::EdgeValidator;
use super::serialization::EdgeSerializer;

/// Edge record operations manager for CRUD operations
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
    /// Validates, serializes and writes an edge record to the appropriate location.
    /// Uses fixed-size slots for simplicity and performance.
    ///
    /// # Arguments
    /// * `edge` - The edge record to write
    ///
    /// # Returns
    /// `Ok(())` if successful, `Err` with details if failed
    pub fn write_edge(&mut self, edge: &EdgeRecord) -> NativeResult<()> {
        // Validate edge fields
        let validator = EdgeValidator::new();
        validator.validate_edge_fields(edge)?;

        // Ensure capacity before writing
        self.ensure_capacity_for_edge(edge.id)?;

        // Serialize and write
        let serializer = EdgeSerializer::new();
        let buffer = serializer.serialize_edge(edge)?;
        let offset = self.edge_offset(edge.id);

        // Ensure buffer fits in fixed slot
        let fixed_slot_size = 256usize;
        if buffer.len() > fixed_slot_size {
            return Err(crate::backend::native::types::NativeBackendError::RecordTooLarge {
                size: buffer.len() as u32,
                max_size: fixed_slot_size as u32,
            });
        }

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
            return Err(crate::backend::native::types::NativeBackendError::InvalidEdgeId {
                id: edge_id,
                max_id: header.edge_count as NativeEdgeId,
            });
        }

        // Calculate offset and read fixed slot
        let offset = self.edge_offset(edge_id);
        let fixed_slot_size = 256usize;
        let mut buffer = vec![0u8; fixed_slot_size];
        self.graph_file.read_bytes(offset, &mut buffer)?;

        // Find actual record size
        if buffer.len() < 33 {
            return Err(crate::backend::native::types::NativeBackendError::CorruptEdgeRecord {
                edge_id,
                reason: "Edge record too short".to_string(),
            });
        }

        // Check version
        if buffer[0] != 1 {
            return Err(crate::backend::native::types::NativeBackendError::CorruptEdgeRecord {
                edge_id,
                reason: "Invalid edge record version".to_string(),
            });
        }

        // Extract length fields
        let type_len = u16::from_be_bytes([buffer[27], buffer[28]]) as usize;
        let data_len = u32::from_be_bytes([buffer[29], buffer[30], buffer[31], buffer[32]]) as usize;
        let actual_size = 1 + 2 + 8 + 8 + 8 + 2 + 4 + type_len + data_len;

        if actual_size > fixed_slot_size {
            return Err(crate::backend::native::types::NativeBackendError::CorruptEdgeRecord {
                edge_id,
                reason: "Edge record too large for fixed slot".to_string(),
            });
        }

        buffer.truncate(actual_size);

        // Deserialize
        let serializer = EdgeSerializer::new();
        serializer.deserialize_edge(edge_id, &buffer)
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
    pub fn update_edge(&mut self, edge: &EdgeRecord) -> NativeResult<()> {
        // Validate the edge ID exists
        let header = self.graph_file.header();
        if edge.id <= 0 || edge.id > header.edge_count as NativeEdgeId {
            return Err(crate::backend::native::types::NativeBackendError::InvalidEdgeId {
                id: edge.id,
                max_id: header.edge_count as NativeEdgeId,
            });
        }

        // Validate and write the updated edge
        let validator = EdgeValidator::new();
        validator.validate_edge_fields(edge)?;
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

    /// Calculate file offset for an edge record
    ///
    /// Uses fixed-size edge records (256 bytes) for simplicity and performance.
    ///
    /// # Arguments
    /// * `edge_id` - The edge ID to calculate offset for
    ///
    /// # Returns
    /// File offset where the edge record is stored
    fn edge_offset(&self, edge_id: NativeEdgeId) -> crate::backend::native::types::FileOffset {
        let base_offset = self.graph_file.persistent_header().edge_data_offset;
        base_offset + ((edge_id - 1) as u64 * 256)
    }

    /// Ensure file has capacity for this edge
    ///
    /// Ensures the underlying file is large enough to store an edge at the calculated offset.
    ///
    /// # Arguments
    /// * `edge_id` - The edge ID to ensure capacity for
    ///
    /// # Returns
    /// `Ok(())` if capacity is ensured, `Err` with details if failed
    fn ensure_capacity_for_edge(&mut self, edge_id: NativeEdgeId) -> NativeResult<()> {
        use crate::backend::native::edge_store::capacity_coordinator::EdgeCapacityCoordinator;
        let mut coordinator = EdgeCapacityCoordinator::new(self.graph_file);
        coordinator.ensure_capacity_for_edge_id(edge_id as u64)
    }
}