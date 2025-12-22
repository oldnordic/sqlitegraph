//! V2 clustered adjacency kernel implementation

use crate::backend::native::edge_store::EdgeStore;
use crate::backend::native::node_store::NodeStore;
use crate::backend::native::types::*;

use super::Direction;

// Import instrumentation when in debug mode
#[cfg(debug_assertions)]
use super::instrumentation::convenience::{get_metrics, start_timing, track_v2_read};

impl super::AdjacencyIterator<'_> {
    // ========================================
    // V2 CLUSTERED ADJACENCY KERNEL IMPLEMENTATION
    // ========================================

    /// V2 clustered adjacency with proper error handling
    /// Uses single clustered read and properly distinguishes between "no cluster" and "corrupt cluster"
    /// FIXED: Cache both success and failure results to prevent repeated initialization attempts
    pub fn try_initialize_clustered_adjacency(&mut self) -> NativeResult<()> {
        // FIXED: Return early if already attempted (prevent infinite loops)
        if self.cached_clustered_neighbors.is_some() {
            return Ok(());
        }

        // First, check if node is V2 format with cluster metadata
        {
            #[cfg(debug_assertions)]
            let _timing = start_timing("v2_cluster_metadata_check");

            let node_data_offset = self.graph_file.persistent_header().node_data_offset;
            let slot_offset = node_data_offset + ((self.node_id - 1) as u64 * 4096);
            let mut version = [0u8; 1];

            // V2-only: Check node format (V1 support removed)
            match self.graph_file.read_bytes(slot_offset, &mut version) {
                Ok(()) => {
                    #[cfg(debug_assertions)]
                    track_v2_read(self.node_id as u32);

                    if version[0] == 2 {
                        // V2 node detected - try to read cluster metadata
                        let mut node_store = NodeStore::new(self.graph_file);
                        match node_store.read_node_v2(self.node_id) {
                            Ok(node_v2) => {
                                drop(node_store);

                                let (cluster_offset, cluster_size, edge_count) =
                                    match self.direction {
                                        Direction::Outgoing => (
                                            node_v2.outgoing_cluster_offset,
                                            node_v2.outgoing_cluster_size,
                                            node_v2.outgoing_edge_count,
                                        ),
                                        Direction::Incoming => (
                                            node_v2.incoming_cluster_offset,
                                            node_v2.incoming_cluster_size,
                                            node_v2.incoming_edge_count,
                                        ),
                                    };

                                // Phase 35: Only proceed if cluster metadata is complete
                                if cluster_offset > 0 && cluster_size > 0 && edge_count > 0 {
                                    #[cfg(debug_assertions)]
                                    let _cluster_timing =
                                        start_timing("v2_cluster_neighbor_iteration");

                                    // Phase 69: Read V2 edge cluster directly (avoid circular dependency)
                                    let neighbors = match self
                                        .read_v2_edge_cluster_directly(&node_v2)
                                    {
                                        Ok(neighbors) => neighbors,
                                        Err(e) => {
                                            #[cfg(debug_assertions)]
                                            println!(
                                                "DEBUG: V2 cluster read failed for node {}: {}, falling back to edge store traversal",
                                                self.node_id, e
                                            );

                                            // Fallback: use edge store to traverse edges directly
                                            let mut edge_store = EdgeStore::new(self.graph_file);
                                            edge_store
                                                .iter_neighbors(self.node_id, self.direction)
                                                .collect::<Vec<_>>()
                                        }
                                    };

                                    // Phase 69: V2 clustered adjacency success
                                    #[cfg(debug_assertions)]
                                    {
                                        println!(
                                            "DEBUG: V2 clustered adjacency SUCCESS for node {} (direction: {:?}, {} neighbors)",
                                            self.node_id,
                                            self.direction,
                                            neighbors.len()
                                        );

                                        // Log metrics snapshot
                                        let metrics = get_metrics();
                                        println!(
                                            "DEBUG: Metrics snapshot - iterations: {}, v2_reads: {}, loop_detections: {}",
                                            metrics.total_iterations,
                                            metrics.total_v2_reads,
                                            metrics.infinite_loop_detections
                                        );
                                    }
                                    self.cached_clustered_neighbors = Some(neighbors);
                                    self.total_count = edge_count;
                                    return Ok(());
                                }
                            }
                            Err(NativeBackendError::InvalidNodeId { .. }) => {
                                // Node doesn't exist - cache empty result and propagate error
                                #[cfg(debug_assertions)]
                                track_v2_read(self.node_id as u32);

                                self.cached_clustered_neighbors = Some(Vec::new());
                                self.total_count = 0; // CRITICAL: Update total_count to match empty result
                                return Err(NativeBackendError::InvalidNodeId { id: 0, max_id: 0 });
                            }
                            Err(e) => {
                                // Phase 35: Propagate unexpected read errors, cache empty result
                                #[cfg(debug_assertions)]
                                track_v2_read(self.node_id as u32);

                                self.cached_clustered_neighbors = Some(Vec::new());
                                self.total_count = 0; // CRITICAL: Update total_count to match empty result
                                return Err(e);
                            }
                        }
                    }
                }
                Err(NativeBackendError::FileTooSmall { .. }) => {
                    // Node slot out of bounds - cache empty result and return error
                    self.cached_clustered_neighbors = Some(Vec::new());
                    self.total_count = 0; // CRITICAL: Update total_count to match empty result
                    return Err(NativeBackendError::FileTooSmall {
                        size: 0,
                        min_size: 1,
                    });
                }
                Err(e) => {
                    // Phase 35: Propagate unexpected I/O errors, cache empty result
                    self.cached_clustered_neighbors = Some(Vec::new());
                    self.total_count = 0; // CRITICAL: Update total_count to match empty result
                    return Err(e);
                }
            }
        }

        // V2-ONLY: Return error if V2 cluster not found
        // FIXED: Cache empty result to prevent repeated initialization attempts
        #[cfg(debug_assertions)]
        {
            println!(
                "DEBUG: V2 clustered adjacency FAILED for node {} - cluster metadata not found",
                self.node_id
            );
            let metrics = get_metrics();
            println!(
                "DEBUG: Final metrics - iterations: {}, v2_reads: {}, loop_detections: {}",
                metrics.total_iterations, metrics.total_v2_reads, metrics.infinite_loop_detections
            );
        }

        let error = NativeBackendError::CorruptNodeRecord {
            node_id: self.node_id as i64,
            reason: "V2 cluster metadata not found".to_string(),
        };
        self.cached_clustered_neighbors = Some(Vec::new()); // Cache empty result
        self.total_count = 0; // CRITICAL: Update total_count to match empty result
        Err(error)
    }

    /// Read V2 edge cluster directly without going through AdjacencyIterator
    /// This avoids the circular dependency where AdjacencyIterator calls edge_store.iter_neighbors()
    /// which creates another AdjacencyIterator
    fn read_v2_edge_cluster_directly(
        &mut self,
        node_v2: &crate::backend::native::v2::node_record_v2::NodeRecordV2,
    ) -> NativeResult<Vec<NativeNodeId>> {
        use crate::backend::native::v2::edge_cluster::EdgeCluster;

        let (cluster_offset, cluster_size) = match self.direction {
            Direction::Outgoing => (
                node_v2.outgoing_cluster_offset,
                node_v2.outgoing_cluster_size,
            ),
            Direction::Incoming => (
                node_v2.incoming_cluster_offset,
                node_v2.incoming_cluster_size,
            ),
        };

        // If no cluster metadata, return empty result
        if cluster_offset == 0 || cluster_size == 0 {
            return Ok(Vec::new());
        }

        #[cfg(debug_assertions)]
        println!(
            "DEBUG: Reading V2 cluster for node {} at offset {}, size {}",
            self.node_id, cluster_offset, cluster_size
        );

        // Read cluster data directly from file
        let mut cluster_data = vec![0u8; cluster_size as usize];
        self.graph_file
            .read_bytes(cluster_offset, &mut cluster_data)?;

        // Check if cluster data contains actual data (not all zeros)
        if cluster_data.iter().all(|&byte| byte == 0) {
            #[cfg(debug_assertions)]
            println!("DEBUG: V2 cluster data is all zeros - no edge cluster was written");
            return Ok(Vec::new());
        }

        // Try to deserialize the cluster
        match EdgeCluster::deserialize(&cluster_data) {
            Ok(cluster) => {
                let neighbors: Vec<NativeNodeId> = cluster
                    .iter_neighbors()
                    .map(|id| id as NativeNodeId)
                    .collect();

                #[cfg(debug_assertions)]
                {
                    println!(
                        "DEBUG: Direct V2 cluster read for node {} (direction: {:?}) - found {} neighbors",
                        self.node_id,
                        self.direction,
                        neighbors.len()
                    );
                }

                Ok(neighbors)
            }
            Err(e) => {
                #[cfg(debug_assertions)]
                println!(
                    "DEBUG: Failed to deserialize V2 cluster for node {}: {}",
                    self.node_id, e
                );

                // This can happen if cluster metadata exists but no actual cluster was written
                // Fall back to edge store traversal
                Err(e)
            }
        }
    }
}
