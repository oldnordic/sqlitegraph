//! File validation and corruption detection utilities
//!
//! This module provides validation functions for ensuring file integrity,
//! detecting corruption, and verifying commit markers in graph files.

use crate::backend::native::{
    constants::HEADER_SIZE,
    types::{NativeBackendError, NativeResult},
};

/// Graph file validation utilities
pub struct GraphFileValidator;

impl GraphFileValidator {
    /// Validate that the file size meets minimum requirements
    ///
    /// Ensures the file is large enough for the header and any declared data.
    /// For native backend, we only require file to be large enough for actual data written.
    pub fn validate_file_size(
        file_size: u64,
        persistent_header: &crate::backend::native::persistent_header::PersistentHeaderV2,
    ) -> NativeResult<()> {
        if file_size < HEADER_SIZE {
            return Err(NativeBackendError::FileTooSmall {
                size: file_size,
                min_size: HEADER_SIZE,
            });
        }

        // Basic sanity check: file should be at least large enough for declared records
        // edge_data_offset is a reservation for future edge data, not a current requirement
        let min_expected_size = if persistent_header.edge_count > 0 {
            // If edges exist, file must be large enough to contain them
            std::cmp::max(
                persistent_header.edge_data_offset,
                persistent_header.node_data_offset,
            )
        } else {
            // If no edges exist, file only needs to be large enough for header and node data
            persistent_header.node_data_offset
        };

        if file_size < min_expected_size {
            return Err(NativeBackendError::FileTooSmall {
                size: file_size,
                min_size: min_expected_size,
            });
        }

        Ok(())
    }

    /// Verify the commit marker indicates a clean commit state
    ///
    /// Detects incomplete clustered commits by checking the commit marker.
    /// Returns error if the marker indicates an incomplete transaction.
    pub fn verify_commit_marker(marker: u64) -> NativeResult<()> {
        const COMMIT_MARKER_CLEAN: u64 = 0x434C45414E5F454F; // "CLEAN_EO" in hex

        if marker != COMMIT_MARKER_CLEAN {
            return Err(NativeBackendError::InvalidHeader {
                field: "commit_marker".to_string(),
                reason: format!(
                    "incomplete clustered commit detected (marker=0x{:016X})",
                    marker
                ),
            });
        }
        Ok(())
    }

    /// Get the clean commit marker value
    pub const fn clean_commit_marker() -> u64 {
        0x434C45414E5F454F // "CLEAN_EO" in hex
    }

    /// Get the commit marker offset in the header
    pub const fn commit_marker_offset() -> u64 {
        72
    }

    /// Calculate minimum expected file size based on header state
    pub fn calculate_minimum_expected_size(
        persistent_header: &crate::backend::native::persistent_header::PersistentHeaderV2,
    ) -> u64 {
        if persistent_header.edge_count > 0 {
            // If edges exist, file must be large enough to contain them
            std::cmp::max(
                persistent_header.edge_data_offset,
                persistent_header.node_data_offset,
            )
        } else {
            // If no edges exist, file only needs to be large enough for header and node data
            persistent_header.node_data_offset
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::persistent_header::PersistentHeaderV2;

    #[test]
    fn test_validate_file_size_minimum_header() {
        let header = PersistentHeaderV2::new_v2();
        let result = GraphFileValidator::validate_file_size(50, &header);
        assert!(result.is_err()); // File too small for header
    }

    #[test]
    fn test_validate_file_size_minimum_with_data() {
        let mut header = PersistentHeaderV2::new_v2();
        header.node_data_offset = 1024;
        header.edge_data_offset = 2048;

        // File large enough for header and declared node data
        let result = GraphFileValidator::validate_file_size(1500, &header);
        assert!(result.is_ok());

        // File too small for declared data
        let result = GraphFileValidator::validate_file_size(1000, &header);
        assert!(result.is_err());
    }

    #[test]
    fn test_verify_commit_marker_clean() {
        let result =
            GraphFileValidator::verify_commit_marker(GraphFileValidator::clean_commit_marker());
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_commit_marker_dirty() {
        let result = GraphFileValidator::verify_commit_marker(0x1234567890ABCDEF);
        assert!(result.is_err());
    }

    #[test]
    fn test_calculate_minimum_expected_size() {
        let mut header = PersistentHeaderV2::new_v2();
        header.node_data_offset = 1024;
        header.edge_data_offset = 2048;

        // No edges - should return node data offset
        let min_size = GraphFileValidator::calculate_minimum_expected_size(&header);
        assert_eq!(min_size, 1024);

        // With edges - should return max of node and edge data offsets
        header.edge_count = 1;
        let min_size = GraphFileValidator::calculate_minimum_expected_size(&header);
        assert_eq!(min_size, 2048);
    }
}
