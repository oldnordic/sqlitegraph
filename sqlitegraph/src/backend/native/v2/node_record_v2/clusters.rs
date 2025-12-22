//! Cluster management and direction operations for V2 node records

use crate::backend::native::types::FileOffset;
use crate::backend::native::v2::edge_cluster::Direction;

use super::NodeRecordV2;

impl NodeRecordV2 {
    /// Set outgoing cluster metadata
    pub fn set_outgoing_cluster(&mut self, offset: FileOffset, size: u32, count: u32) {
        self.outgoing_cluster_offset = offset;
        self.outgoing_cluster_size = size;
        self.outgoing_edge_count = count;
    }

    /// Set incoming cluster metadata
    pub fn set_incoming_cluster(&mut self, offset: FileOffset, size: u32, count: u32) {
        self.incoming_cluster_offset = offset;
        self.incoming_cluster_size = size;
        self.incoming_edge_count = count;
    }

    /// Check if node has outgoing edges
    pub fn has_outgoing_edges(&self) -> bool {
        self.outgoing_edge_count > 0 && self.outgoing_cluster_offset > 0
    }

    /// Check if node has incoming edges
    pub fn has_incoming_edges(&self) -> bool {
        self.incoming_edge_count > 0 && self.incoming_cluster_offset > 0
    }

    /// Get total edge count (outgoing + incoming)
    pub fn total_edge_count(&self) -> u32 {
        self.outgoing_edge_count + self.incoming_edge_count
    }

    /// Check if node has cluster for specific direction
    pub fn has_cluster_for_direction(&self, direction: Direction) -> bool {
        match direction {
            Direction::Outgoing => self.has_outgoing_edges(),
            Direction::Incoming => self.has_incoming_edges(),
        }
    }

    /// Get cluster offset for specific direction
    pub fn cluster_offset(&self, direction: Direction) -> FileOffset {
        match direction {
            Direction::Outgoing => self.outgoing_cluster_offset,
            Direction::Incoming => self.incoming_cluster_offset,
        }
    }

    /// Get cluster size for specific direction
    pub fn cluster_size(&self, direction: Direction) -> u32 {
        match direction {
            Direction::Outgoing => self.outgoing_cluster_size,
            Direction::Incoming => self.incoming_cluster_size,
        }
    }

    /// Set cluster metadata for specific direction
    pub fn set_cluster(&mut self, direction: Direction, offset: FileOffset, size: u32, count: u32) {
        match direction {
            Direction::Outgoing => self.set_outgoing_cluster(offset, size, count),
            Direction::Incoming => self.set_incoming_cluster(offset, size, count),
        }
    }

    /// Estimate cluster size for a given number of edges (V2 estimation)
    pub fn estimate_cluster_size(edge_count: u32) -> u32 {
        // Base header + edges + overhead estimation
        // From test: estimate_cluster_size(1) = 58, estimate_cluster_size(10) = 508
        if edge_count == 0 {
            return 0;
        }

        // Rough calculation: ~50 bytes per edge + overhead
        let base_overhead = 8; // Base overhead
        let per_edge = 50; // ~50 bytes per edge

        base_overhead + (edge_count * per_edge)
    }
}
