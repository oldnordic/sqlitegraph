//! Utility functions for edge store operations
//!
//! This module provides helper functions for edge store operations including
//! cluster overlap detection, offset calculations, and debugging utilities.

use crate::backend::native::persistent_header::PersistentHeaderV2;
use crate::backend::native::types::NativeNodeId;

/// Check for cluster overlap and print diagnostic information
///
/// This function validates that a proposed cluster allocation does not overlap
/// with critical regions (header, node region, or existing clusters).
///
/// # Arguments
/// * `node_id` - The node ID the cluster belongs to
/// * `direction` - Direction string for debug output ("Outgoing" or "Incoming")
/// * `cluster_offset` - Starting offset of the proposed cluster
/// * `cluster_size` - Size of the proposed cluster in bytes
/// * `node_region_end` - End of the node region (for overlap checking)
/// * `header` - Persistent header containing existing cluster information
pub fn check_for_overlap(
    node_id: NativeNodeId,
    direction: &str,
    cluster_offset: u64,
    cluster_size: u64,
    node_region_end: u64,
    header: &PersistentHeaderV2,
) {
    let cluster_end = cluster_offset + cluster_size;

    // Check overlap with node region (critical issue)
    if cluster_offset < node_region_end {
        println!(
            "[V2_ALLOC_DEBUG] 🔥 OVERLAP DETECTED: node_id={}, direction={}, cluster=[{}, {}) OVERLAPS node_region=[0, {})",
            node_id, direction, cluster_offset, cluster_end, node_region_end
        );
    }

    // Check overlap with header region (critical issue - header is at offset 0-1024)
    if cluster_offset < 1024 {
        println!(
            "[V2_ALLOC_DEBUG] 🔥 OVERLAP DETECTED: node_id={}, direction={}, cluster=[{}, {}) OVERLAPS header_region=[0, 1024]",
            node_id, direction, cluster_offset, cluster_end
        );
    }

    // Check overlap between outgoing and incoming clusters (same node)
    if header.outgoing_cluster_offset > 0 && header.incoming_cluster_offset > 0 {
        let outgoing_end = header.outgoing_cluster_offset + cluster_size; // Estimated size
        let incoming_end = header.incoming_cluster_offset + cluster_size; // Estimated size

        if direction == "Incoming"
            && cluster_offset < outgoing_end
            && cluster_end > header.outgoing_cluster_offset
        {
            println!(
                "[V2_ALLOC_DEBUG] 🔥 OVERLAP DETECTED: node_id={}, direction={}, cluster=[{}, {}) OVERLAPS outgoing_cluster=[{}, {})",
                node_id,
                direction,
                cluster_offset,
                cluster_end,
                header.outgoing_cluster_offset,
                outgoing_end
            );
        }

        if direction == "Outgoing"
            && cluster_offset < incoming_end
            && cluster_end > header.incoming_cluster_offset
        {
            println!(
                "[V2_ALLOC_DEBUG] 🔥 OVERLAP DETECTED: node_id={}, direction={}, cluster=[{}, {}) OVERLAPS incoming_cluster=[{}, {})",
                node_id,
                direction,
                cluster_offset,
                cluster_end,
                header.incoming_cluster_offset,
                incoming_end
            );
        }
    }

    // Final allocation summary
    println!(
        "[V2_ALLOC_DEBUG] ALLOCATION: node_id={}, direction={}, cluster=[{}, {}), cluster_size={}",
        node_id, direction, cluster_offset, cluster_end, cluster_size
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::persistent_header::PersistentHeaderV2;

    #[test]
    fn test_check_for_overlap_header_region() {
        let header = PersistentHeaderV2::new_v2();
        let node_id = 1;
        let direction = "Outgoing";
        let cluster_offset = 500; // Within header region (0-1024)
        let cluster_size = 200;
        let node_region_end = 4096;

        // Should detect header region overlap
        check_for_overlap(
            node_id,
            direction,
            cluster_offset,
            cluster_size,
            node_region_end,
            &header,
        );
    }

    #[test]
    fn test_check_for_overlap_node_region() {
        let header = PersistentHeaderV2::new_v2();
        let node_id = 1;
        let direction = "Outgoing";
        let cluster_offset = 2000; // Within node region (1024-4096)
        let cluster_size = 200;
        let node_region_end = 3072;

        // Should detect node region overlap
        check_for_overlap(
            node_id,
            direction,
            cluster_offset,
            cluster_size,
            node_region_end,
            &header,
        );
    }

    #[test]
    fn test_check_for_overlap_no_overlap() {
        let mut header = PersistentHeaderV2::new_v2();
        header.outgoing_cluster_offset = 8192;
        header.incoming_cluster_offset = 12288;
        let node_id = 1;
        let direction = "Outgoing";
        let cluster_offset = 16384; // Safe zone beyond all regions
        let cluster_size = 512;
        let node_region_end = 4096;

        // Should detect no overlaps (only allocation summary)
        check_for_overlap(
            node_id,
            direction,
            cluster_offset,
            cluster_size,
            node_region_end,
            &header,
        );
    }

    #[test]
    fn test_check_for_overlap_cluster_overlap() {
        let mut header = PersistentHeaderV2::new_v2();
        header.outgoing_cluster_offset = 8192;
        header.incoming_cluster_offset = 9216; // Overlaps with outgoing
        let node_id = 1;
        let direction = "Incoming";
        let cluster_offset = 8704; // Overlaps with outgoing cluster
        let cluster_size = 1024;
        let node_region_end = 4096;

        // Should detect cluster overlap
        check_for_overlap(
            node_id,
            direction,
            cluster_offset,
            cluster_size,
            node_region_end,
            &header,
        );
    }
}
