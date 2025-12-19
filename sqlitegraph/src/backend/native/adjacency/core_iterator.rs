//! Core adjacency iterator implementation

use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::node_store::NodeStore;
use crate::backend::native::optimizations::*;
use crate::backend::native::types::*;
use crate::backend::native::v2::node_record_v2::NodeRecordV2Ext;

use super::Direction;

// Import instrumentation when in debug mode
#[cfg(debug_assertions)]
use super::instrumentation::convenience::{
    track_iteration, start_timing, get_metrics, validate_state
};

#[cfg(debug_assertions)]
use log::error;

/// Adjacency iterator for efficient neighbor traversal
pub struct AdjacencyIterator<'a> {
    /// Graph file reference for I/O operations
    pub(crate) graph_file: &'a mut GraphFile,
    /// Target node identifier for adjacency traversal
    pub(crate) node_id: NativeNodeId,
    /// Traversal direction (outgoing or incoming edges)
    pub(crate) direction: Direction,
    /// Optional edge type filter for iteration
    pub(crate) edge_filter: Option<Vec<String>>,
    /// Current iteration position index
    pub(crate) current_index: u32,
    /// Total number of neighbors available
    pub(crate) total_count: u32,
    /// Cached node metadata to avoid repeated deserialization
    pub(crate) cached_node: Option<NodeRecord>,
    /// Pre-computed edge offsets from neighbor pointer table (fast path)
    pub(crate) edge_offsets: Option<Vec<FileOffset>>,
    /// Hot node metadata for fast adjacency operations
    pub(crate) node_hot: Option<NodeHot>,
    /// V2 Clustered adjacency: cached neighbors for sequential I/O
    pub(crate) cached_clustered_neighbors: Option<Vec<NativeNodeId>>,
}

impl<'a> AdjacencyIterator<'a> {
    /// Create a copy of the iterator at the same position
    pub fn copy_iterator(&self) -> NativeResult<Self> {
        // We can't actually copy since we'd need a mutable reference to the same graph_file
        // This is a limitation of the current design
        Err(NativeBackendError::BufferTooSmall {
            size: 0,
            min_size: 1,
        })
    }

    /// Create a new adjacency iterator for outgoing neighbors
    pub fn new_outgoing(
        graph_file: &'a mut GraphFile,
        node_id: NativeNodeId,
    ) -> NativeResult<Self> {
        // Try to get hot metadata first
        let node_hot = get_node_hot(node_id);

        // Try to get edge offsets from pointer table (fast path)
        let edge_offsets = get_outgoing_edge_offsets(node_id);

        // Fall back to reading the full node if needed
        let (node, total_count) =
            if let (Some(hot), Some(_offsets)) = (node_hot.as_ref(), edge_offsets.as_ref()) {
                // Fast path: we have all the info we need
                (None, hot.outgoing_edge_count)
            } else {
                // Slow path: read full node record
                let mut node_store = NodeStore::new(graph_file);
                let node = node_store.read_node(node_id)?;
                let total_count = node.outgoing_edge_count;
                (Some(node), total_count)
            };

        Ok(Self {
            graph_file,
            node_id,
            direction: Direction::Outgoing,
            edge_filter: None,
            current_index: 0,
            total_count,
            cached_node: node,
            edge_offsets,
            node_hot,
            cached_clustered_neighbors: None,
        })
    }

    /// Create a new adjacency iterator for incoming neighbors
    pub fn new_incoming(
        graph_file: &'a mut GraphFile,
        node_id: NativeNodeId,
    ) -> NativeResult<Self> {
        // Try to get hot metadata first
        let node_hot = get_node_hot(node_id);

        // Try to get edge offsets from pointer table (fast path)
        let edge_offsets = get_incoming_edge_offsets(node_id);

        // Fall back to reading the full node if needed
        let (node, total_count) =
            if let (Some(hot), Some(_offsets)) = (node_hot.as_ref(), edge_offsets.as_ref()) {
                // Fast path: we have all the info we need
                (None, hot.incoming_edge_count)
            } else {
                // Slow path: read full node record
                let mut node_store = NodeStore::new(graph_file);
                let node = node_store.read_node(node_id)?;
                let total_count = node.incoming_edge_count;
                (Some(node), total_count)
            };

        Ok(Self {
            graph_file,
            node_id,
            direction: Direction::Incoming,
            edge_filter: None,
            current_index: 0,
            total_count,
            cached_node: node,
            edge_offsets,
            node_hot,
            cached_clustered_neighbors: None,
        })
    }

    /// Set edge type filter for iteration
    pub fn with_edge_filter(mut self, edge_types: &[&str]) -> Self {
        self.edge_filter = Some(edge_types.iter().map(|&s| s.to_string()).collect());
        self
    }

    /// Get the total number of neighbors (before filtering)
    #[inline(always)]
    pub fn total_count(&self) -> u32 {
        self.total_count
    }

    /// Get the current iteration position
    #[inline(always)]
    pub fn current_index(&self) -> u32 {
        self.current_index
    }

    /// Check if iteration is complete
    #[inline(always)]
    pub fn is_complete(&self) -> bool {
        self.current_index >= self.total_count
    }

    /// Reset iterator to beginning
    pub fn reset(&mut self) {
        self.current_index = 0;
    }

    /// Get neighbor node ID at current position (optimized with pointer table and hot cache)
    #[inline]
    pub fn get_current_neighbor(&mut self) -> NativeResult<Option<NativeNodeId>> {
        // Instrumentation: Track iterations for infinite loop detection
        #[cfg(debug_assertions)]
        {
            if !track_iteration(self.node_id as u32) {
                error!("Stopping iteration due to infinite loop detection for node {}", self.node_id);
                return Ok(None);
            }
        }

        // Validate current state consistency
        #[cfg(debug_assertions)]
        {
            let validation_report = validate_state(
                self.node_id as u32,
                self.current_index,
                self.total_count,
                self.cached_clustered_neighbors.as_ref().map(|n| n.len()),
            );

            if !validation_report.is_valid() {
                error!("Iterator state validation failed for node {}", self.node_id);
            }
        }

        // V2 Clustered Adjacency Path: Use clustered neighbors if available (HIGHEST PRIORITY)
        if self.cached_clustered_neighbors.is_none() {
            #[cfg(debug_assertions)]
            let _timing = start_timing("try_initialize_clustered_adjacency");

            // EVIDENCE-BASED FIX: Ensure initialization errors are cached to prevent repeated attempts
            if let Err(_) = self.try_initialize_clustered_adjacency() {
                // Error has already been cached in try_initialize_clustered_adjacency()
                // The cached_clustered_neighbors should now be Some(Vec::new()) with total_count = 0
                #[cfg(debug_assertions)]
                {
                    println!("DEBUG: V2 cluster initialization failed and cached for node {}", self.node_id);
                }
            }
        }

        if let Some(ref neighbors) = self.cached_clustered_neighbors {
            let current_index = self.current_index as usize;
            if current_index < neighbors.len() {
                return Ok(Some(neighbors[current_index]));
            }
            return Ok(None);
        }

        // Fast path: Use pointer table if available
        if let Some(edge_offsets) = self.edge_offsets.take() {
            let result = self.get_current_neighbor_fast_path(&edge_offsets);
            self.edge_offsets = Some(edge_offsets); // Put it back
            return result;
        }

        // V2-only: No V1 fallback - return no neighbors if V2 clustering unavailable
        Ok(None)
    }

    /// V2-only: Fast path disabled - use V2 clustered adjacency only
    /// In V2, individual edge reading is replaced by cluster-based reading
    #[inline(always)]
    pub(crate) fn get_current_neighbor_fast_path(
        &mut self,
        _edge_offsets: &[FileOffset],
    ) -> NativeResult<Option<NativeNodeId>> {
        // V2-only: Fast path disabled - rely on V2 clustered adjacency
        // Individual edge reading is not used in V2 architecture
        Ok(None)
    }


    /// Collect all neighbors into a vector
    pub fn collect(mut self) -> NativeResult<Vec<NativeNodeId>> {
        #[cfg(debug_assertions)]
        let _timing = start_timing("adjacency_collect_operation");

        #[cfg(debug_assertions)]
        {
            println!(
                "DEBUG: Starting collect operation for node {} (direction: {:?})",
                self.node_id, self.direction
            );
        }

        let mut neighbors = Vec::new();

        while !self.is_complete() {
            match self.get_current_neighbor()? {
                Some(neighbor) => {
                    neighbors.push(neighbor);
                    self.current_index += 1;
                }
                None => {
                    // Inconsistency detected - force termination
                    #[cfg(debug_assertions)]
                    eprintln!("DEBUG: Terminating iteration early - no neighbor found at index {} for node {} (total_count: {})",
                                     self.current_index, self.node_id, self.total_count);
                    break;
                }
            }
        }

        // neighbors() must return unique neighbor IDs
        // This deduplication is applied at the public API layer, preserving full edge multiplicity internally
        let mut seen_neighbors = std::collections::HashSet::new();
        let mut unique_neighbors = Vec::new();

        for neighbor in neighbors.clone() {
            if seen_neighbors.insert(neighbor) {
                unique_neighbors.push(neighbor);
            }
        }

        #[cfg(debug_assertions)]
        {
            println!(
                "DEBUG: Completed collect operation for node {} - {} raw neighbors, {} unique neighbors",
                self.node_id,
                neighbors.len(),
                unique_neighbors.len()
            );

            let metrics = get_metrics();
            println!(
                "DEBUG: Final collect metrics - iterations: {}, v2_reads: {}, loop_detections: {}, efficiency: {:.2}",
                metrics.total_iterations,
                metrics.total_v2_reads,
                metrics.infinite_loop_detections,
                metrics.iteration_efficiency()
            );

            // Warn about potential infinite loop patterns
            if metrics.suggests_infinite_loop() {
                println!(
                    "WARNING: Collect operation shows potential infinite loop pattern for node {}",
                    self.node_id
                );
            }
        }

        Ok(unique_neighbors)
    }

    /// Check if a specific neighbor exists
    pub fn contains(&mut self, target_id: NativeNodeId) -> NativeResult<bool> {
        // Store original position
        let original_index = self.current_index;

        // Reset to beginning
        self.current_index = 0;

        // Search through all neighbors
        while !self.is_complete() {
            if let Some(neighbor_id) = self.get_current_neighbor()? {
                if neighbor_id == target_id {
                    // Restore original position
                    self.current_index = original_index;
                    return Ok(true);
                }
            }
            self.current_index += 1;
        }

        // Restore original position
        self.current_index = original_index;
        Ok(false)
    }

    /// Get neighbors in batches
    pub fn get_batch(&mut self, batch_size: u32) -> NativeResult<Vec<NativeNodeId>> {
        let mut batch = Vec::with_capacity(batch_size as usize);
        let end_index = (self.current_index + batch_size).min(self.total_count);

        while self.current_index < end_index {
            if let Some(neighbor) = self.get_current_neighbor()? {
                batch.push(neighbor);
            }
            self.current_index += 1;
        }

        Ok(batch)
    }
}