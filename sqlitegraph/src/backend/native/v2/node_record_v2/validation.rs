//! Size calculation and validation logic for V2 node records

use crate::backend::native::{NativeBackendError, NativeResult};

use super::NodeRecordV2;

impl NodeRecordV2 {
    /// Get serialized length of the node record (experimental feature)
    #[cfg(feature = "v2_experimental")]
    pub fn serialized_len(&self) -> usize {
        let data_len = serde_json::to_vec(&self.data)
            .expect("serializing serde_json::Value should not fail")
            .len();
        21 + self.kind.len() + self.name.len() + data_len + 32
    }

    /// Calculate the size in bytes of this record
    pub fn size_bytes(&self) -> usize {
        1 + 4
            + 8
            + 2
            + 2
            + 4
            + self.kind.len()
            + self.name.len()
            + serde_json::to_vec(&self.data).unwrap_or_default().len()
            + 24
    }

    /// Validate the node record for consistency and correctness
    pub fn validate(&self) -> NativeResult<()> {
        if self.id <= 0 {
            return Err(NativeBackendError::InvalidNodeId {
                id: self.id,
                max_id: 0,
            });
        }

        if self.outgoing_edge_count > 0 {
            if self.outgoing_cluster_offset == 0 || self.outgoing_cluster_size == 0 {
                return Err(NativeBackendError::InconsistentAdjacency {
                    node_id: self.id,
                    count: self.outgoing_edge_count,
                    direction: "outgoing".to_string(),
                    file_count: 0,
                });
            }
        }

        if self.incoming_edge_count > 0 {
            if self.incoming_cluster_offset == 0 || self.incoming_cluster_size == 0 {
                return Err(NativeBackendError::InconsistentAdjacency {
                    node_id: self.id,
                    count: self.incoming_edge_count,
                    direction: "incoming".to_string(),
                    file_count: 0,
                });
            }
        }

        if self.outgoing_cluster_offset > 0 && self.outgoing_cluster_offset < 1024 {
            return Err(NativeBackendError::InconsistentAdjacency {
                node_id: self.id,
                count: self.outgoing_edge_count,
                direction: "outgoing".to_string(),
                file_count: 0,
            });
        }

        if self.incoming_cluster_offset > 0 && self.incoming_cluster_offset < 1024 {
            return Err(NativeBackendError::InconsistentAdjacency {
                node_id: self.id,
                count: self.incoming_edge_count,
                direction: "incoming".to_string(),
                file_count: 0,
            });
        }

        // Cluster overlap validation
        // Only validates when both clusters are allocated (offsets > 0)
        // This accounts for allocation sequencing where clusters are allocated one at a time
        if self.outgoing_cluster_offset > 0 && self.incoming_cluster_offset > 0 {
            let outgoing_end = self.outgoing_cluster_offset + self.outgoing_cluster_size as u64;
            let incoming_end = self.incoming_cluster_offset + self.incoming_cluster_size as u64;

            // DEBUG: Print cluster information
            if std::env::var("CLUSTER_VALIDATION_DEBUG").is_ok() {
                println!("[CLUSTER_VALIDATION_DEBUG] Node {}: checking overlap", self.id);
                println!("  outgoing_cluster_offset: {}", self.outgoing_cluster_offset);
                println!("  outgoing_cluster_size: {}", self.outgoing_cluster_size);
                println!("  outgoing_end: {}", outgoing_end);
                println!("  incoming_cluster_offset: {}", self.incoming_cluster_offset);
                println!("  incoming_cluster_size: {}", self.incoming_cluster_size);
                println!("  incoming_end: {}", incoming_end);
                println!("  overlap_condition: {} < {} && {} < {}",
                    self.incoming_cluster_offset, outgoing_end,
                    self.outgoing_cluster_offset, incoming_end);
            }

            // Bidirectional overlap check: intervals [outgoing_start, outgoing_end) and [incoming_start, incoming_end)
            // Overlap occurs if: incoming_start < outgoing_end AND outgoing_start < incoming_end
            if self.incoming_cluster_offset < outgoing_end
                && self.outgoing_cluster_offset < incoming_end
            {
                // Calculate actual overlap size
                // Allow edge case where clusters are adjacent (overlap_size = 0 is OK)
                let overlap_start = std::cmp::max(self.outgoing_cluster_offset, self.incoming_cluster_offset);
                let overlap_end = std::cmp::min(outgoing_end, incoming_end);
                let overlap_size = overlap_end - overlap_start;

                if std::env::var("CLUSTER_VALIDATION_DEBUG").is_ok() {
                    if overlap_size > 0 {
                        println!("  ❌ OVERLAP DETECTED! overlap_size={}", overlap_size);
                    } else {
                        println!("  ✅ No overlap detected (clusters may be adjacent)");
                    }
                }

                if overlap_size > 0 {
                    return Err(NativeBackendError::InconsistentAdjacency {
                        node_id: self.id,
                        count: self.outgoing_edge_count,
                        direction: "cluster_overlap".to_string(),
                        file_count: overlap_size as u32,
                    });
                }
            } else {
                if std::env::var("CLUSTER_VALIDATION_DEBUG").is_ok() {
                    println!("  ✅ No overlap detected");
                }
            }
        }

        Ok(())
    }
}
