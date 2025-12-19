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

mod metrics;
mod core;
mod clusters;
mod serialization;
mod deserialization;
mod validation;
mod extensions;

pub use metrics::AdjacencyMetrics;
pub use core::NodeRecordV2;
pub use clusters::*;
pub use serialization::*;
pub use deserialization::*;
pub use validation::*;
pub use extensions::*;

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
}