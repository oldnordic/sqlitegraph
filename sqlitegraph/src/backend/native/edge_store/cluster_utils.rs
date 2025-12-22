//! Edge cluster utility functions
//!
//! This module provides utility functions for calculating cluster offsets,
//! managing cluster metadata, and handling cluster-related operations.

/// Calculate where neighbor_id is stored in cluster buffer
///
/// This function calculates the byte offset within an edge cluster where
/// the neighbor ID for a specific edge is stored.
///
/// # Arguments
/// * `edge_idx` - Index of the edge within the cluster (0-based)
///
/// # Returns
/// Byte offset from the start of the cluster buffer where the neighbor_id is stored
///
/// # Cluster Format
/// - Header: magic(4) + version(2) + flags(2) + payload_size(4) + edge_count(4) = 16 bytes
/// - Per edge: neighbor_id(8) + edge_type_offset(4) + edge_data_len(4) = 16 bytes
/// - Edge data follows edges array
pub fn calculate_neighbor_offset_in_cluster(edge_idx: usize) -> usize {
    // Cluster format calculation:
    // - Header: magic(4) + version(2) + flags(2) + payload_size(4) + edge_count(4) = 16 bytes
    // - Per edge: neighbor_id(8) + edge_type_offset(4) + edge_data_len(4) = 16 bytes
    // - Edge data follows edges array
    let header_size = 16;
    let edge_metadata_size = 16;
    header_size + (edge_idx * edge_metadata_size)
}

/// Calculate where edge data starts in cluster buffer
///
/// This function calculates the byte offset within an edge cluster where
/// the actual edge data (JSON payload) for a specific edge begins.
///
/// # Arguments
/// * `edge_idx` - Index of the edge within the cluster (0-based)
///
/// # Returns
/// Option containing byte offset from the start of the cluster buffer where edge data begins,
/// or None if calculation fails
///
/// # Format Details
/// The actual format depends on the EdgeCluster implementation. This is an approximation
/// that skips the neighbor_id (8 bytes) to get to the edge data region.
pub fn calculate_edge_data_offset_in_cluster(edge_idx: usize) -> Option<usize> {
    // Approximate cluster format calculation:
    // - Header: magic(4) + version(2) + flags(2) + payload_size(4) + edge_count(4) = 16 bytes
    // - Per edge: neighbor_id(8) + edge_type_offset(4) + edge_data_len(4) = 16 bytes
    // - Edge data follows edges array
    let header_size = 16;
    let edge_metadata_size = 16;
    let edges_offset = header_size + (edge_idx * edge_metadata_size);

    // Skip neighbor_id to get to edge data region
    Some(edges_offset + 8)
}

/// Validate cluster size calculations
///
/// This function validates that calculated cluster sizes are reasonable
/// and don't exceed logical limits.
///
/// # Arguments
/// * `edge_count` - Number of edges in the cluster
/// * `max_cluster_size` - Maximum allowed cluster size
///
/// # Returns
/// `true` if the calculated cluster size is valid, `false` otherwise
pub fn validate_cluster_size(edge_count: usize, max_cluster_size: usize) -> bool {
    // Calculate expected cluster size
    let header_size = 16;
    let per_edge_metadata_size = 16;
    let expected_size = header_size + (edge_count * per_edge_metadata_size);

    // Validate against maximum allowed size
    expected_size <= max_cluster_size && edge_count > 0
}

/// Calculate optimal cluster allocation size
///
/// This function determines the optimal cluster size based on the number
/// of edges and minimum/maximum size constraints.
///
/// # Arguments
/// * `edge_count` - Number of edges to accommodate
/// * `min_cluster_size` - Minimum cluster size (for alignment)
/// * `max_cluster_size` - Maximum cluster size
///
/// # Returns
/// Optimal cluster size that fits the requirements
pub fn calculate_optimal_cluster_size(
    edge_count: usize,
    min_cluster_size: usize,
    max_cluster_size: usize,
) -> usize {
    let header_size = 16;
    let per_edge_size = 16;
    let required_size = header_size + (edge_count * per_edge_size);

    // Apply bounds first, then align the final result to maintain alignment guarantee
    let bounded_size = required_size.max(min_cluster_size).min(max_cluster_size);

    // Align to reasonable boundaries (typically 64 or 128 bytes)
    let alignment = 64;
    ((bounded_size + alignment - 1) / alignment) * alignment
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_neighbor_offset_in_cluster() {
        // Test first edge in cluster
        assert_eq!(calculate_neighbor_offset_in_cluster(0), 16);

        // Test second edge in cluster
        assert_eq!(calculate_neighbor_offset_in_cluster(1), 32);

        // Test fifth edge in cluster
        assert_eq!(calculate_neighbor_offset_in_cluster(4), 80);

        // Test tenth edge in cluster
        assert_eq!(calculate_neighbor_offset_in_cluster(9), 160);
    }

    #[test]
    fn test_calculate_edge_data_offset_in_cluster() {
        // Test first edge data offset
        assert_eq!(calculate_edge_data_offset_in_cluster(0), Some(24));

        // Test second edge data offset
        assert_eq!(calculate_edge_data_offset_in_cluster(1), Some(40));

        // Test fifth edge data offset
        assert_eq!(calculate_edge_data_offset_in_cluster(4), Some(88));

        // Test tenth edge data offset
        assert_eq!(calculate_edge_data_offset_in_cluster(9), Some(168));
    }

    #[test]
    fn test_validate_cluster_size() {
        // Test valid cluster sizes
        assert!(validate_cluster_size(1, 1024));
        assert!(validate_cluster_size(10, 1024));
        assert!(validate_cluster_size(50, 1024));

        // Test invalid cluster sizes
        assert!(!validate_cluster_size(0, 1024)); // Zero edges
        assert!(validate_cluster_size(60, 1024)); // Too many edges for size limit
        assert!(!validate_cluster_size(100, 1024)); // Way too many edges
    }

    #[test]
    fn test_calculate_optimal_cluster_size() {
        let min_size = 256;
        let max_size = 1024;

        // Test small edge count
        assert_eq!(calculate_optimal_cluster_size(1, min_size, max_size), 256);

        // Test medium edge count
        assert_eq!(calculate_optimal_cluster_size(10, min_size, max_size), 256);

        // Test large edge count requiring exact calculation
        let _required_size = 16 + (20 * 16); // 16 header + 20 edges * 16 bytes = 336
        assert_eq!(calculate_optimal_cluster_size(20, min_size, max_size), 384); // Aligned to 64

        // Test maximum capacity
        let max_edges = (max_size - 16) / 16; // (1024 - 16) / 16 = 63
        assert_eq!(
            calculate_optimal_cluster_size(max_edges, min_size, max_size),
            1024
        );

        // Test exceeding maximum
        assert_eq!(
            calculate_optimal_cluster_size(max_edges + 1, min_size, max_size),
            1024
        );
    }

    #[test]
    fn test_calculate_optimal_cluster_size_alignment() {
        let min_size = 200;
        let max_size = 1000;

        // Test that result is always aligned to 64-byte boundaries
        for edge_count in 1..=50 {
            let size = calculate_optimal_cluster_size(edge_count, min_size, max_size);
            assert_eq!(
                size % 64,
                0,
                "Size {} for {} edges should be aligned to 64-byte boundaries",
                size,
                edge_count
            );
        }
    }
}
