//! Edge record validation utilities
//!
//! Provides validation functions for edge records including field validation,
//? ID checking, and size limits.

use crate::backend::native::constants::edge;
use crate::backend::native::types::{NativeResult, EdgeRecord, NativeBackendError};

/// Edge record validation utilities
pub struct EdgeValidator;

impl EdgeValidator {
    /// Create a new edge validator
    pub fn new() -> Self {
        Self
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
    pub fn validate_edge_fields(&self, edge: &EdgeRecord) -> NativeResult<()> {
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
}

impl Default for EdgeValidator {
    fn default() -> Self {
        Self::new()
    }
}