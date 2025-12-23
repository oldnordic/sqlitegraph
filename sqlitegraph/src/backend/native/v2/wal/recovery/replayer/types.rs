//! V2 WAL Recovery Replayer Types and Configuration
//!
//! This module contains all types, configurations, and data structures
//! used by the V2 WAL recovery replayer system.

use crate::backend::native::{NativeNodeId};
use crate::backend::native::v2::edge_cluster::Direction;
use std::path::PathBuf;

/// Configuration for V2 transaction replay operations
#[derive(Debug, Clone)]
pub struct ReplayConfig {
    /// Whether to perform strict validation during replay
    pub strict_validation: bool,
    /// Maximum batch size for transaction operations
    pub max_batch_size: usize,
    /// Timeout for individual operations
    pub operation_timeout_ms: u64,
    /// Whether to create backups before modifications
    pub create_backup: bool,
    /// Progress reporting interval (operations)
    pub progress_interval: usize,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        use crate::backend::native::v2::wal::recovery::constants;
        use crate::backend::native::v2::wal::recovery::constants::v2::MAX_NODE_RECORD_OPERATIONS_PER_RECOVERY;
        Self {
            strict_validation: true,
            max_batch_size: MAX_NODE_RECORD_OPERATIONS_PER_RECOVERY,
            operation_timeout_ms: constants::validation::CONSISTENCY_CHECK_TIMEOUT_MS,
            create_backup: false, // Backup handled by recovery core
            progress_interval: constants::RECOVERY_PROGRESS_INTERVAL,
        }
    }
}

/// Replay result with comprehensive statistics
#[derive(Debug, Clone)]
pub struct ReplayResult {
    /// Successfully replayed operations
    pub successful_operations: u64,
    /// Failed operations with details
    pub failed_operations: Vec<(crate::backend::native::v2::wal::V2WALRecord, crate::backend::native::v2::wal::recovery::errors::RecoveryError)>,
    /// Replay statistics
    pub statistics: ReplayStatistics,
    /// Any warnings encountered
    pub warnings: Vec<String>,
}

/// Detailed replay statistics and performance metrics
#[derive(Debug, Clone, Default)]
pub struct ReplayStatistics {
    /// Total replay duration in milliseconds
    pub total_duration_ms: u64,
    /// Number of node operations
    pub node_operations: u64,
    /// Number of edge operations
    pub edge_operations: u64,
    /// Number of string operations
    pub string_operations: u64,
    /// Number of free space operations
    pub free_space_operations: u64,
    /// Average operation time in milliseconds
    pub avg_operation_time_ms: f64,
    /// Maximum operation time in milliseconds
    pub max_operation_time_ms: u64,
    /// Bytes written to graph file
    pub bytes_written: u64,
}

impl ReplayStatistics {
    /// Get the total number of operations performed
    pub fn total_operations(&self) -> u64 {
        self.node_operations + self.edge_operations +
        self.string_operations + self.free_space_operations
    }
}

/// Rollback operation for transaction recovery
///
/// This enum defines operations that can be rolled back during
/// transaction replay failure scenarios.
#[derive(Debug, Clone)]
pub enum RollbackOperation {
    /// Rollback node insertion by deleting the node
    NodeInsert {
        node_id: NativeNodeId,
        node_data: Vec<u8>,
    },
    /// Rollback node update by restoring old data
    NodeUpdate {
        node_id: NativeNodeId,
        old_data: Vec<u8>,
    },
    /// Rollback node deletion by reinserting the node
    NodeDelete {
        node_id: NativeNodeId,
        slot_offset: u64,
    },
    /// Rollback string insertion (NEW: for string table operations)
    StringInsert {
        string_id: u64,
        string_value: String,
    },
    // Edge rollback operations
    EdgeInsert {
        cluster_key: (u64, u64),
        insertion_point: u32,
        edge_record: Vec<u8>,
        cluster_offset: u64,
        cluster_size: u32,
    },
    EdgeUpdate {
        cluster_key: (i64, crate::backend::native::v2::edge_cluster::Direction),
        position: u32,
        old_edge: Vec<u8>,
        new_edge: Vec<u8>,
    },
    EdgeDelete {
        cluster_key: (i64, crate::backend::native::v2::edge_cluster::Direction),
        position: u32,
        old_edge: Vec<u8>,
    },
    //
    // Cluster rollback operations
    ClusterCreate {
        node_id: u64,
        direction: Direction,
        cluster_offset: u64,
        cluster_size: u64,
        cluster_data: Vec<u8>,
    },
    //
    // Free space rollback operations
    FreeSpaceAllocate {
        block_offset: u64,
        block_size: u64,
        block_type: u8,
    },
    FreeSpaceDeallocate {
        block_offset: u64,
        block_size: u64,
        block_type: u8,
    },
}

impl ReplayStatistics {
    /// Create new empty statistics
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a node operation
    pub fn record_node_operation(&mut self) {
        self.node_operations += 1;
    }

    /// Record an edge operation
    pub fn record_edge_operation(&mut self) {
        self.edge_operations += 1;
    }

    /// Record a string operation
    pub fn record_string_operation(&mut self) {
        self.string_operations += 1;
    }

    /// Record a free space operation
    pub fn record_free_space_operation(&mut self) {
        self.free_space_operations += 1;
    }

    /// Update operation timing statistics
    pub fn update_timing(&mut self, operation_time_ms: u64) {
        self.max_operation_time_ms = self.max_operation_time_ms.max(operation_time_ms);

        if self.node_operations + self.edge_operations + self.string_operations + self.free_space_operations > 0 {
            let total_ops = self.node_operations + self.edge_operations + self.string_operations + self.free_space_operations;
            self.avg_operation_time_ms = ((self.total_duration_ms as f64) + (operation_time_ms as f64)) / total_ops as f64;
        }
    }

    /// Record bytes written to graph file
    pub fn record_bytes_written(&mut self, bytes: u64) {
        self.bytes_written += bytes;
    }
}

impl RollbackOperation {
    /// Get a descriptive name for the rollback operation
    pub fn operation_name(&self) -> &'static str {
        match self {
            RollbackOperation::NodeInsert { .. } => "NodeInsert",
            RollbackOperation::NodeUpdate { .. } => "NodeUpdate",
            RollbackOperation::NodeDelete { .. } => "NodeDelete",
            RollbackOperation::StringInsert { .. } => "StringInsert",
            RollbackOperation::EdgeInsert { .. } => "EdgeInsert",
            RollbackOperation::EdgeUpdate { .. } => "EdgeUpdate",
            RollbackOperation::EdgeDelete { .. } => "EdgeDelete",
            RollbackOperation::ClusterCreate { .. } => "ClusterCreate",
            RollbackOperation::FreeSpaceAllocate { .. } => "FreeSpaceAllocate",
            RollbackOperation::FreeSpaceDeallocate { .. } => "FreeSpaceDeallocate",
        }
    }

    /// Check if this operation affects node data
    pub fn affects_nodes(&self) -> bool {
        matches!(self, RollbackOperation::NodeInsert { .. } | RollbackOperation::NodeUpdate { .. } | RollbackOperation::NodeDelete { .. })
    }

    /// Check if this operation affects string data
    pub fn affects_strings(&self) -> bool {
        matches!(self, RollbackOperation::StringInsert { .. })
    }

    /// Check if this operation affects free space
    pub fn affects_free_space(&self) -> bool {
        matches!(self, RollbackOperation::FreeSpaceAllocate { .. } | RollbackOperation::FreeSpaceDeallocate { .. })
    }

    /// Check if this operation affects edge data
    pub fn affects_edges(&self) -> bool {
        matches!(self, RollbackOperation::EdgeInsert { .. } | RollbackOperation::EdgeUpdate { .. } | RollbackOperation::EdgeDelete { .. })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_replay_config_default() {
        let config = ReplayConfig::default();
        assert!(config.strict_validation);
        assert_eq!(config.create_backup, false);
        assert!(config.max_batch_size > 0);
        assert!(config.operation_timeout_ms > 0);
    }

    #[test]
    fn test_replay_statistics_default() {
        let stats = ReplayStatistics::default();
        assert_eq!(stats.total_duration_ms, 0);
        assert_eq!(stats.node_operations, 0);
        assert_eq!(stats.edge_operations, 0);
        assert_eq!(stats.string_operations, 0);
        assert_eq!(stats.free_space_operations, 0);
        assert_eq!(stats.bytes_written, 0);
    }

    #[test]
    fn test_replay_statistics_recording() {
        let mut stats = ReplayStatistics::new();

        stats.record_node_operation();
        stats.record_string_operation();
        stats.record_bytes_written(1024);

        assert_eq!(stats.node_operations, 1);
        assert_eq!(stats.string_operations, 1);
        assert_eq!(stats.bytes_written, 1024);
    }

    #[test]
    fn test_rollback_operation_names() {
        let node_insert = RollbackOperation::NodeInsert {
            node_id: 42,
            node_data: vec![1, 2, 3],
        };
        assert_eq!(node_insert.operation_name(), "NodeInsert");
        assert!(node_insert.affects_nodes());
        assert!(!node_insert.affects_strings());

        let string_insert = RollbackOperation::StringInsert {
            string_id: 100,
            string_value: "test".to_string(),
        };
        assert_eq!(string_insert.operation_name(), "StringInsert");
        assert!(!string_insert.affects_nodes());
        assert!(string_insert.affects_strings());

        let edge_insert = RollbackOperation::EdgeInsert {
            cluster_key: (100, 0),
            insertion_point: 5,
            edge_record: vec![1, 2, 3],
            cluster_offset: 4000,
            cluster_size: 256,
        };
        assert_eq!(edge_insert.operation_name(), "EdgeInsert");
        assert!(!edge_insert.affects_nodes());
        assert!(!edge_insert.affects_strings());
        assert!(edge_insert.affects_edges());
        assert!(!edge_insert.affects_free_space());

        let edge_update = RollbackOperation::EdgeUpdate {
            cluster_key: (100, crate::backend::native::v2::edge_cluster::Direction::Outgoing),
            position: 2,
            old_edge: vec![4, 5, 6],
            new_edge: vec![7, 8, 9],
        };
        assert_eq!(edge_update.operation_name(), "EdgeUpdate");
        assert!(!edge_update.affects_nodes());
        assert!(!edge_update.affects_strings());
        assert!(edge_update.affects_edges());
        assert!(!edge_update.affects_free_space());

        let free_space_allocate = RollbackOperation::FreeSpaceAllocate {
            block_offset: 1000,
            block_size: 512,
            block_type: 1,
        };
        assert_eq!(free_space_allocate.operation_name(), "FreeSpaceAllocate");
        assert!(!free_space_allocate.affects_nodes());
        assert!(!free_space_allocate.affects_strings());
        assert!(free_space_allocate.affects_free_space());

        let free_space_deallocate = RollbackOperation::FreeSpaceDeallocate {
            block_offset: 2000,
            block_size: 1024,
            block_type: 2,
        };
        assert_eq!(free_space_deallocate.operation_name(), "FreeSpaceDeallocate");
        assert!(!free_space_deallocate.affects_nodes());
        assert!(!free_space_deallocate.affects_strings());
        assert!(free_space_deallocate.affects_free_space());
    }

    #[test]
    fn test_rollback_operation_debug() {
        let operation = RollbackOperation::StringInsert {
            string_id: 123,
            string_value: "test_string".to_string(),
        };

        let debug_str = format!("{:?}", operation);
        assert!(debug_str.contains("StringInsert"));
        assert!(debug_str.contains("123"));
        assert!(debug_str.contains("test_string"));
    }
}