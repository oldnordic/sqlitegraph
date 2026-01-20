//! V2 WAL Recovery Replayer Types and Configuration
//!
//! This module contains all types, configurations, and data structures
//! used by the V2 WAL recovery replayer system.

use crate::backend::native::{NativeNodeId};
use crate::backend::native::v2::edge_cluster::{CompactEdgeRecord, Direction};
use std::sync::atomic::{AtomicU64, Ordering};

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
    /// Maximum number of transactions to replay in parallel (default: 4)
    pub max_parallel_transactions: usize,
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
            max_parallel_transactions: 4, // Default parallelism degree
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
    /// Replay statistics snapshot
    pub statistics: StatisticsSnapshot,
    /// Any warnings encountered
    pub warnings: Vec<String>,
}

/// Detailed replay statistics and performance metrics
///
/// This struct uses AtomicU64 for lock-free concurrent access,
/// reducing contention during parallel WAL replay operations.
pub struct ReplayStatistics {
    /// Total replay duration in milliseconds
    pub total_duration_ms: AtomicU64,
    /// Number of node operations
    pub node_operations: AtomicU64,
    /// Number of edge operations
    pub edge_operations: AtomicU64,
    /// Number of string operations
    pub string_operations: AtomicU64,
    /// Number of free space operations
    pub free_space_operations: AtomicU64,
    /// Maximum operation time in milliseconds
    pub max_operation_time_ms: AtomicU64,
    /// Bytes written to graph file
    pub bytes_written: AtomicU64,
    /// Average operation time in milliseconds (non-atomic, computed on demand)
    avg_operation_time_ms_cache: f64,
}

impl Default for ReplayStatistics {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayStatistics {
    /// Create new empty statistics with all counters initialized to zero
    pub fn new() -> Self {
        Self {
            total_duration_ms: AtomicU64::new(0),
            node_operations: AtomicU64::new(0),
            edge_operations: AtomicU64::new(0),
            string_operations: AtomicU64::new(0),
            free_space_operations: AtomicU64::new(0),
            max_operation_time_ms: AtomicU64::new(0),
            bytes_written: AtomicU64::new(0),
            avg_operation_time_ms_cache: 0.0,
        }
    }

    /// Get the total number of operations performed
    pub fn total_operations(&self) -> u64 {
        self.node_operations.load(Ordering::Relaxed) +
        self.edge_operations.load(Ordering::Relaxed) +
        self.string_operations.load(Ordering::Relaxed) +
        self.free_space_operations.load(Ordering::Relaxed)
    }

    /// Record a node operation (lock-free)
    pub fn record_node_operation(&self) {
        self.node_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record an edge operation (lock-free)
    pub fn record_edge_operation(&self) {
        self.edge_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a string operation (lock-free)
    pub fn record_string_operation(&self) {
        self.string_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a free space operation (lock-free)
    pub fn record_free_space_operation(&self) {
        self.free_space_operations.fetch_add(1, Ordering::Relaxed);
    }

    /// Record bytes written to graph file (lock-free)
    pub fn record_bytes_written(&self, bytes: u64) {
        self.bytes_written.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Update operation timing statistics (lock-free for max, computed for avg)
    pub fn update_timing(&self, operation_time_ms: u64) {
        // Update max operation time (lock-free)
        let mut current_max = self.max_operation_time_ms.load(Ordering::Relaxed);
        while operation_time_ms > current_max {
            match self.max_operation_time_ms.compare_exchange_weak(
                current_max,
                operation_time_ms,
                Ordering::Relaxed,
                Ordering::Relaxed
            ) {
                Ok(_) => break,
                Err(new_max) => current_max = new_max,
            }
        }
        // Note: avg_operation_time_ms is computed on-demand in snapshot()
    }

    /// Set total duration (called once at end of replay)
    pub fn set_total_duration(&self, duration_ms: u64) {
        self.total_duration_ms.store(duration_ms, Ordering::Relaxed);
    }

    /// Create a consistent snapshot of all statistics
    ///
    /// This provides a point-in-time view of all counters, useful for
    /// reporting and analysis. The snapshot may not be perfectly consistent
    /// across all fields due to concurrent updates, but this is acceptable
    /// for statistics reporting.
    pub fn snapshot(&self) -> StatisticsSnapshot {
        let total_ops = self.total_operations();
        let duration = self.total_duration_ms.load(Ordering::Relaxed);
        let avg_time = if total_ops > 0 {
            duration as f64 / total_ops as f64
        } else {
            0.0
        };

        StatisticsSnapshot {
            total_duration_ms: duration,
            node_operations: self.node_operations.load(Ordering::Relaxed),
            edge_operations: self.edge_operations.load(Ordering::Relaxed),
            string_operations: self.string_operations.load(Ordering::Relaxed),
            free_space_operations: self.free_space_operations.load(Ordering::Relaxed),
            avg_operation_time_ms: avg_time,
            max_operation_time_ms: self.max_operation_time_ms.load(Ordering::Relaxed),
            bytes_written: self.bytes_written.load(Ordering::Relaxed),
        }
    }
}

/// Immutable snapshot of replay statistics
///
/// This struct provides a consistent, immutable view of statistics
/// at a point in time, suitable for reporting and analysis.
#[derive(Debug, Clone, Default)]
pub struct StatisticsSnapshot {
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

impl StatisticsSnapshot {
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
    /// Rollback node deletion by reinserting the node with all edges
    NodeDelete {
        node_id: NativeNodeId,
        slot_offset: u64,
        old_data: Vec<u8>,
        outgoing_edges: Vec<CompactEdgeRecord>,
        incoming_edges: Vec<CompactEdgeRecord>,
    },
    /// Rollback string insertion (NEW: for string table operations)
    StringInsert {
        string_id: u64,
        string_value: String,
    },
    /// Rollback header update by restoring old data
    HeaderUpdate {
        header_offset: u64,
        new_data: Vec<u8>,
        old_data: Vec<u8>,
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

impl RollbackOperation {
    /// Get a descriptive name for the rollback operation
    pub fn operation_name(&self) -> &'static str {
        match self {
            RollbackOperation::NodeInsert { .. } => "NodeInsert",
            RollbackOperation::NodeUpdate { .. } => "NodeUpdate",
            RollbackOperation::NodeDelete { .. } => "NodeDelete",
            RollbackOperation::StringInsert { .. } => "StringInsert",
            RollbackOperation::HeaderUpdate { .. } => "HeaderUpdate",
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
        assert_eq!(config.max_parallel_transactions, 4);
    }

    #[test]
    fn test_replay_statistics_default() {
        let stats = ReplayStatistics::default();
        assert_eq!(stats.total_duration_ms.load(Ordering::Relaxed), 0);
        assert_eq!(stats.node_operations.load(Ordering::Relaxed), 0);
        assert_eq!(stats.edge_operations.load(Ordering::Relaxed), 0);
        assert_eq!(stats.string_operations.load(Ordering::Relaxed), 0);
        assert_eq!(stats.free_space_operations.load(Ordering::Relaxed), 0);
        assert_eq!(stats.bytes_written.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn test_replay_statistics_recording() {
        let stats = ReplayStatistics::new();

        stats.record_node_operation();
        stats.record_string_operation();
        stats.record_bytes_written(1024);

        assert_eq!(stats.node_operations.load(Ordering::Relaxed), 1);
        assert_eq!(stats.string_operations.load(Ordering::Relaxed), 1);
        assert_eq!(stats.bytes_written.load(Ordering::Relaxed), 1024);
    }

    #[test]
    fn test_replay_statistics_snapshot() {
        let stats = ReplayStatistics::new();

        stats.record_node_operation();
        stats.record_string_operation();
        stats.record_bytes_written(1024);
        stats.set_total_duration(100);

        let snapshot = stats.snapshot();

        assert_eq!(snapshot.node_operations, 1);
        assert_eq!(snapshot.string_operations, 1);
        assert_eq!(snapshot.bytes_written, 1024);
        assert_eq!(snapshot.total_duration_ms, 100);
        assert_eq!(snapshot.total_operations(), 2);
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