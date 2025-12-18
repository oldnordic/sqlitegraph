//! Header management and persistent header operations
//!
//! This module provides header management functionality including initialization,
//! validation, access control, and cluster offset management for GraphFile
//! operations.

use crate::backend::native::{
    types::NativeResult,
    persistent_header::PersistentHeaderV2,
    v2::{V2_MAGIC, V2_FORMAT_VERSION},
    constants::node::NODE_SLOT_SIZE,
};

/// Header management utilities for GraphFile
pub struct HeaderManager;

impl HeaderManager {
    /// Initialize V2 header with proper cluster offset configuration
    ///
    /// Sets up the header with magic bytes, version, and calculates proper
    /// offsets for nodes, edges, and clusters to prevent corruption.
    pub fn initialize_v2_header(
        header: &mut PersistentHeaderV2,
        node_count: u64,
        default_node_data_start: u64,
        reserved_node_region_bytes: u64,
    ) -> NativeResult<()> {
        println!(
            "[CLUSTER_DEBUG] initialize_v2_header() called - fixing cluster offsets to prevent node slot corruption"
        );

        // Set basic V2 header fields
        header.magic = V2_MAGIC;
        header.version = V2_FORMAT_VERSION;
        if header.node_data_offset < default_node_data_start {
            header.node_data_offset = default_node_data_start;
        }

        // V2-ONLY: Position edge data AFTER node region to prevent corruption
        // Reserve space for maximum node capacity (using a reasonable default)
        const MAX_NODE_CAPACITY: u64 = 10000; // Support up to 10K nodes
        let node_region_end = header.node_data_offset + (MAX_NODE_CAPACITY * NODE_SLOT_SIZE);
        header.edge_data_offset = node_region_end;

        // CRITICAL INVARIANT: Ensure edge and node regions never overlap
        debug_assert!(
            header.edge_data_offset >= header.node_data_offset,
            "edge_data_offset ({}) must be >= node_data_offset ({})",
            header.edge_data_offset,
            header.node_data_offset
        );

        // PHASE 42 FIX: Initialize cluster offsets to distinct regions
        // Reserve space for cluster regions: outgoing → incoming → free_space
        let node_region_size = reserved_node_region_bytes;

        // CRITICAL FIX: Calculate base_cluster_start AFTER node_data_offset is finalized
        let base_cluster_start = header.node_data_offset + (node_count as u64 * 4096);

        // MANDATORY INVARIANT: Calculate node region end to prevent cluster overlap
        let node_region_end = header.node_data_offset + (node_count as u64 * 4096);

        // MANDATORY INVARIANT: Calculate cluster floor to ensure clusters are outside node region
        let cluster_floor = std::cmp::max(
            node_region_end,
            header.node_data_offset + reserved_node_region_bytes,
        );

        // DEBUG: Print layout invariants
        Self::print_layout_invariants(header, node_region_end, base_cluster_start, cluster_floor);

        // PHASE 76 CRITICAL FIX: Prevent cluster offset corruption of node slots
        // Ensure cluster offsets are positioned AFTER the entire node region to prevent overwrites
        let node_region_end = header.node_data_offset + node_region_size;

        // Fix outgoing cluster offset if it's inside node region
        if header.outgoing_cluster_offset < node_region_end {
            Self::log_cluster_offset_fix("outgoing", header.outgoing_cluster_offset, node_region_end);
            header.outgoing_cluster_offset = node_region_end;
        }

        // Position incoming clusters after outgoing clusters with reasonable spacing
        let min_incoming_offset = header.outgoing_cluster_offset + (node_count as u64 * 256);
        if header.incoming_cluster_offset < min_incoming_offset {
            Self::log_cluster_offset_fix("incoming", header.incoming_cluster_offset, min_incoming_offset);
            header.incoming_cluster_offset = min_incoming_offset;
        }

        // Ensure free space offset is properly positioned
        if header.free_space_offset < header.node_data_offset + (2 * node_region_size) {
            header.free_space_offset = cluster_floor + (2 * node_region_size);
        }

        // CRITICAL INVARIANT: Cluster offsets must be outside node region
        if header.outgoing_cluster_offset < node_region_end {
            Self::log_cluster_offset_fix("outgoing", header.outgoing_cluster_offset, node_region_end);
            header.outgoing_cluster_offset = node_region_end;
        }

        if header.incoming_cluster_offset < node_region_end {
            let corrected_incoming_offset = node_region_end + (node_count as u64 * 256);
            Self::log_cluster_offset_fix("incoming", header.incoming_cluster_offset, corrected_incoming_offset);
            header.incoming_cluster_offset = corrected_incoming_offset;
        }

        // Print final cluster layout
        Self::print_final_cluster_layout(header);

        Ok(())
    }

    /// Validate header invariants and constraints
    ///
    /// Ensures the header configuration maintains all critical invariants
    /// for safe file operations.
    pub fn validate_header_invariants(header: &PersistentHeaderV2) -> NativeResult<()> {
        // Check basic header validity
        if header.magic != V2_MAGIC {
            return Err(crate::backend::native::types::NativeBackendError::InvalidHeader {
                field: "magic".to_string(),
                reason: format!("Invalid magic bytes: expected {:x?}, got {:x?}", V2_MAGIC, header.magic),
            });
        }

        if header.version != V2_FORMAT_VERSION {
            return Err(crate::backend::native::types::NativeBackendError::InvalidHeader {
                field: "version".to_string(),
                reason: format!("Unsupported version: expected {}, got {}", V2_FORMAT_VERSION, header.version),
            });
        }

        // Validate offset ordering
        if header.node_data_offset >= header.edge_data_offset {
            return Err(crate::backend::native::types::NativeBackendError::InvalidHeader {
                field: "offsets".to_string(),
                reason: format!(
                    "node_data_offset ({}) must be < edge_data_offset ({})",
                    header.node_data_offset, header.edge_data_offset
                ),
            });
        }

        // Validate cluster offsets are outside node region
        let node_region_end = header.node_data_offset + (header.node_count as u64 * NODE_SLOT_SIZE);

        if header.outgoing_cluster_offset < node_region_end {
            return Err(crate::backend::native::types::NativeBackendError::InvalidHeader {
                field: "outgoing_cluster_offset".to_string(),
                reason: format!(
                    "outgoing_cluster_offset ({}) must be >= node_region_end ({})",
                    header.outgoing_cluster_offset, node_region_end
                ),
            });
        }

        if header.incoming_cluster_offset < node_region_end {
            return Err(crate::backend::native::types::NativeBackendError::InvalidHeader {
                field: "incoming_cluster_offset".to_string(),
                reason: format!(
                    "incoming_cluster_offset ({}) must be >= node_region_end ({})",
                    header.incoming_cluster_offset, node_region_end
                ),
            });
        }

        Ok(())
    }

    /// Print cluster layout debugging information
    fn print_layout_invariants(
        header: &PersistentHeaderV2,
        node_region_end: u64,
        base_cluster_start: u64,
        cluster_floor: u64,
    ) {
        println!("[CLUSTER_DEBUG] Layout invariants:");
        println!("  node_data_offset = {}", header.node_data_offset);
        println!("  node_count = {}", header.node_count);
        println!("  node_region_end = {}", node_region_end);
        println!("  base_cluster_start = {}", base_cluster_start);
        println!("  cluster_floor = {}", cluster_floor);
        println!(
            "  current outgoing_cluster_offset = {}",
            header.outgoing_cluster_offset
        );
        println!(
            "  current incoming_cluster_offset = {}",
            header.incoming_cluster_offset
        );
    }

    /// Print final cluster layout after corrections
    fn print_final_cluster_layout(header: &PersistentHeaderV2) {
        println!(
            "  final outgoing_cluster_offset = {}",
            header.outgoing_cluster_offset
        );
        println!(
            "  final incoming_cluster_offset = {}",
            header.incoming_cluster_offset
        );
    }

    /// Log critical cluster offset fixes
    fn log_cluster_offset_fix(cluster_type: &str, old_offset: u64, new_offset: u64) {
        println!(
            "CRITICAL FIX: Moving {}_cluster_offset from {} to {} to prevent node slot corruption",
            cluster_type, old_offset, new_offset
        );
    }

    /// Get header statistics for debugging
    pub fn get_header_statistics(header: &PersistentHeaderV2, reserved_node_region_bytes: u64) -> HeaderStatistics {
        let node_region_end = header.node_data_offset + (header.node_count as u64 * NODE_SLOT_SIZE);

        HeaderStatistics {
            node_count: header.node_count,
            edge_count: header.edge_count,
            node_data_offset: header.node_data_offset,
            edge_data_offset: header.edge_data_offset,
            outgoing_cluster_offset: header.outgoing_cluster_offset,
            incoming_cluster_offset: header.incoming_cluster_offset,
            free_space_offset: header.free_space_offset,
            node_region_end,
            total_node_region_size: reserved_node_region_bytes,
        }
    }
}

/// Header statistics for debugging and monitoring
#[derive(Debug, Clone)]
pub struct HeaderStatistics {
    pub node_count: u64,
    pub edge_count: u64,
    pub node_data_offset: u64,
    pub edge_data_offset: u64,
    pub outgoing_cluster_offset: u64,
    pub incoming_cluster_offset: u64,
    pub free_space_offset: u64,
    pub node_region_end: u64,
    pub total_node_region_size: u64,
}

impl HeaderStatistics {
    /// Check if clusters are properly positioned outside node region
    pub fn are_clusters_positioned_correctly(&self) -> bool {
        self.outgoing_cluster_offset >= self.node_region_end
            && self.incoming_cluster_offset >= self.node_region_end
    }

    /// Get cluster region utilization
    pub fn get_cluster_utilization(&self) -> ClusterUtilization {
        let outgoing_size = if self.outgoing_cluster_offset > 0 {
            self.incoming_cluster_offset - self.outgoing_cluster_offset
        } else {
            0
        };

        ClusterUtilization {
            outgoing_cluster_start: self.outgoing_cluster_offset,
            incoming_cluster_start: self.incoming_cluster_offset,
            outgoing_region_size: outgoing_size,
            free_space_start: self.free_space_offset,
        }
    }
}

/// Cluster utilization statistics
#[derive(Debug, Clone)]
pub struct ClusterUtilization {
    pub outgoing_cluster_start: u64,
    pub incoming_cluster_start: u64,
    pub outgoing_region_size: u64,
    pub free_space_start: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initialize_v2_header() {
        let mut header = PersistentHeaderV2::new_v2();
        let node_count = 100;
        let default_node_data_start = 1024;
        let reserved_node_region_bytes = 8 * 1024 * 1024;

        HeaderManager::initialize_v2_header(&mut header, node_count, default_node_data_start, reserved_node_region_bytes).unwrap();

        assert_eq!(header.magic, V2_MAGIC);
        assert_eq!(header.version, V2_FORMAT_VERSION);
        assert!(header.node_data_offset >= default_node_data_start);
        assert!(header.edge_data_offset > header.node_data_offset);
    }

    #[test]
    fn test_validate_header_invariants() {
        let mut header = PersistentHeaderV2::new_v2();
        header.magic = V2_MAGIC;
        header.version = V2_FORMAT_VERSION;
        header.node_data_offset = 1024; // DEFAULT_NODE_DATA_START
        header.edge_data_offset = 1024 + 10000; // DEFAULT_NODE_DATA_START + 10000
        header.node_count = 100;

        // Calculate node region end: 1024 + (100 * 4096) = 410624
        let node_region_end = header.node_data_offset + (header.node_count as u64 * NODE_SLOT_SIZE);

        // Cluster offsets must be outside node region
        header.outgoing_cluster_offset = node_region_end + 10000;
        header.incoming_cluster_offset = node_region_end + 20000;

        // Should pass validation
        assert!(HeaderManager::validate_header_invariants(&header).is_ok());
    }

    #[test]
    fn test_validate_header_invariants_invalid_magic() {
        let mut header = PersistentHeaderV2::new_v2();
        header.magic = [0xFF; 8]; // Invalid magic
        header.version = V2_FORMAT_VERSION;
        header.node_data_offset = 1024; // DEFAULT_NODE_DATA_START
        header.edge_data_offset = 1024 + 10000; // DEFAULT_NODE_DATA_START + 10000

        // Should fail validation
        assert!(HeaderManager::validate_header_invariants(&header).is_err());
    }

    #[test]
    fn test_validate_header_inversions_invalid_offsets() {
        let mut header = PersistentHeaderV2::new_v2();
        header.magic = V2_MAGIC;
        header.version = V2_FORMAT_VERSION;
        header.node_data_offset = 1024; // DEFAULT_NODE_DATA_START
        header.edge_data_offset = 924; // DEFAULT_NODE_DATA_START - 100; // Invalid: before node data

        // Should fail validation
        assert!(HeaderManager::validate_header_invariants(&header).is_err());
    }

    #[test]
    fn test_get_header_statistics() {
        let mut header = PersistentHeaderV2::new_v2();
        header.node_count = 50;
        header.edge_count = 200;
        header.node_data_offset = 1024; // DEFAULT_NODE_DATA_START
        header.edge_data_offset = 11264; // DEFAULT_NODE_DATA_START + 10000

        let reserved_node_region_bytes = 8 * 1024 * 1024; // 8 MiB
        let stats = HeaderManager::get_header_statistics(&header, reserved_node_region_bytes);
        assert_eq!(stats.node_count, 50);
        assert_eq!(stats.edge_count, 200);
        assert_eq!(stats.node_data_offset, 1024); // DEFAULT_NODE_DATA_START
        assert_eq!(stats.edge_data_offset, 11264); // DEFAULT_NODE_DATA_START + 10000
    }

    #[test]
    fn test_cluster_utilization() {
        let mut stats = HeaderStatistics {
            node_count: 100,
            edge_count: 500,
            node_data_offset: 1024,
            edge_data_offset: 11264,
            outgoing_cluster_offset: 50000,
            incoming_cluster_offset: 60000,
            free_space_offset: 70000,
            node_region_end: 45056,
            total_node_region_size: 8388608,
        };

        let utilization = stats.get_cluster_utilization();
        assert!(stats.are_clusters_positioned_correctly());
        assert_eq!(utilization.outgoing_cluster_start, 50000);
        assert_eq!(utilization.incoming_cluster_start, 60000);
        assert_eq!(utilization.free_space_start, 70000);
    }
}