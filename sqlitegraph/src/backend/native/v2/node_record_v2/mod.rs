//! V2 node record implementation (V1 conversion removed).
//!
//! Now V2-only with no V1 conversion support:
//! - `core.rs`: core struct definition and constructor
//! - `clusters.rs`: adjacency cluster management and direction operations
//! - `serialization.rs`: binary serialization implementation
//! - `deserialization.rs`: complex deserialization with error handling
//! - `validation.rs`: size calculation and validation logic
//! - `extensions.rs`: utility functions and trait extensions
//! - `metrics.rs`: adjacency analytics helpers

mod clusters;
mod core;
mod deserialization;
mod extensions;
mod metrics;
mod serialization;
mod validation;

pub use core::NodeRecordV2;
pub use extensions::*;
pub use metrics::AdjacencyMetrics;

#[cfg(all(test, feature = "v2_experimental"))]
mod tests {
    use super::*;
    use crate::backend::native::NodeRecord;

    #[test]
    fn test_v2_node_serialization() {
        let data = serde_json::json!({"key": "value"});
        let node = NodeRecordV2::new(42, "Function".to_string(), "test_func".to_string(), data);
        let serialized = node.serialize();
        assert!(serialized.len() < 200);
        assert!(serialized.len() > 50);

        let deserialized = NodeRecordV2::deserialize(&serialized).unwrap();
        assert_eq!(deserialized.id, node.id);
        assert_eq!(deserialized.kind, node.kind);
        assert_eq!(deserialized.name, node.name);
        assert_eq!(deserialized.data, node.data);
    }

    #[test]
    fn test_v2_adjacency_metadata() {
        let mut node = NodeRecordV2::new(
            1,
            "Node".to_string(),
            "test".to_string(),
            serde_json::json!({}),
        );
        node.set_outgoing_cluster(10000, 500, 10);
        node.set_incoming_cluster(10500, 300, 5);

        assert!(node.has_outgoing_edges());
        assert!(node.has_incoming_edges());
        assert_eq!(node.total_edge_count(), 15);
        assert_eq!(node.outgoing_cluster_offset, 10000);
        assert_eq!(node.incoming_cluster_size, 300);
    }

    #[test]
    fn test_cluster_size_estimation() {
        assert_eq!(NodeRecordV2::estimate_cluster_size(0), 0);
        assert_eq!(NodeRecordV2::estimate_cluster_size(1), 58);
        assert_eq!(NodeRecordV2::estimate_cluster_size(10), 508);
    }

    #[test]
    fn test_validate_cluster_overlap_detection() {
        // Create a node with overlapping clusters
        // Outgoing cluster: offset 4096, size 1024 -> occupies [4096, 5120)
        // Incoming cluster: offset 4608, size 512 -> occupies [4608, 5120)
        // Overlap: [4608, 5120) = 512 bytes
        let mut node = NodeRecordV2::new(
            1,
            "Node".to_string(),
            "test".to_string(),
            serde_json::json!({}),
        );
        node.outgoing_cluster_offset = 4096;
        node.outgoing_cluster_size = 1024;
        node.outgoing_edge_count = 10;
        node.incoming_cluster_offset = 4608;  // Starts within outgoing cluster
        node.incoming_cluster_size = 512;
        node.incoming_edge_count = 5;

        let result = node.validate();
        assert!(result.is_err(), "Expected validation error for overlapping clusters");

        let err = result.unwrap_err();
        match err {
            crate::backend::native::NativeBackendError::InconsistentAdjacency { node_id, direction, file_count } => {
                assert_eq!(node_id, 1);
                assert_eq!(direction, "cluster_overlap");
                assert_eq!(file_count, 512);  // Should include overlap size
            }
            _ => panic!("Expected InconsistentAdjacency error with cluster_overlap direction"),
        }
    }

    #[test]
    fn test_validate_non_overlapping_clusters() {
        // Create a node with non-overlapping clusters
        // Outgoing cluster: offset 10000, size 500 -> occupies [10000, 10500)
        // Incoming cluster: offset 10500, size 300 -> occupies [10500, 10800)
        // No overlap - adjacent but not overlapping
        let mut node = NodeRecordV2::new(
            1,
            "Node".to_string(),
            "test".to_string(),
            serde_json::json!({}),
        );
        node.outgoing_cluster_offset = 10000;
        node.outgoing_cluster_size = 500;
        node.outgoing_edge_count = 10;
        node.incoming_cluster_offset = 10500;  // Starts right after outgoing ends (adjacent)
        node.incoming_cluster_size = 300;
        node.incoming_edge_count = 5;

        let result = node.validate();
        assert!(result.is_ok(), "Expected validation to pass for non-overlapping clusters");
    }

    #[test]
    fn test_validate_sequential_allocation_timing() {
        // Create a node where only outgoing cluster is allocated
        // This simulates the sequential allocation timing issue
        let mut node = NodeRecordV2::new(
            1,
            "Node".to_string(),
            "test".to_string(),
            serde_json::json!({}),
        );
        node.outgoing_cluster_offset = 10000;
        node.outgoing_cluster_size = 500;
        node.outgoing_edge_count = 10;
        // Incoming cluster not yet allocated (offset = 0)
        node.incoming_cluster_offset = 0;
        node.incoming_cluster_size = 0;
        node.incoming_edge_count = 0;

        let result = node.validate();
        assert!(result.is_ok(), "Expected validation to pass when only one cluster allocated");
    }
}
