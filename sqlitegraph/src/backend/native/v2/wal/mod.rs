//! Write-Ahead Logging (WAL) for V2 clustered edge format.
//!
//! This module implements high-performance WAL specifically designed for SQLiteGraph's
//! V2-native clustered edge file format, providing transactional durability, crash recovery,
//! and 5-10x write throughput improvement through sequential I/O patterns.
//!
//! # Architecture
//!
//! The WAL system is designed around V2's clustered edge architecture:
//! - **Cluster-Affinity Logging**: Groups operations by cluster to maintain I/O locality
//! - **Sequential Write Patterns**: Leverages V2's natural clustering for optimal throughput
//! - **Incremental Checkpointing**: Progressive dirty block flushing for sustained performance
//!
//! # Performance Targets
//!
//! - **Write Throughput**: 5-10x improvement over current V2 format
//! - **Commit Latency**: <1ms for small transactions
//! - **Recovery Time**: <1 second per 100MB WAL
//! - **Space Overhead**: <15% additional storage
//! - **Read Overhead**: <5% performance impact

pub mod bulk_ingest;
#[cfg(test)]
pub mod bulk_ingest_tests;
pub mod checkpoint;
pub mod graph_integration;
pub mod manager;
pub mod metrics;
pub mod performance;
pub mod reader;
pub mod record;
pub mod recovery;
pub mod transaction_coordinator;
pub mod v2_integration;
pub mod writer;

// Re-export core WAL types
pub use bulk_ingest::{BulkIngestConfig, BulkIngestExt, BulkIngestGuard, BulkIngestMetrics};
pub use checkpoint::V2WALCheckpointManager;
pub use graph_integration::{
    GraphOperationResult, GraphWALIntegrationConfig, NodeRecordV2WALExt, OperationMetrics,
    V2GraphWALIntegrator,
};
pub use manager::{TransactionIsolation, V2WALManager, WALManagerMetrics};
pub use metrics::{V2WALMetrics, WALPerformanceCounters};
pub use performance::{
    AdaptivePerformanceTuner, ClusterAffinityOptimizer, ClusterAffinityStats, CompressionAlgorithm,
    CompressionStats, IOBatcher, IOBatcherStats, PerformanceConfig, WALRecordCompressor,
};
pub use reader::V2WALReader;
pub use record::{V2WALRecord, V2WALRecordType, WALSerializationError};
pub use recovery::V2WALRecoveryEngine;
pub use transaction_coordinator::{
    IsolationLevel, LockType, ResourceId, TransactionId, TransactionState, V2TransactionCoordinator,
};
pub use v2_integration::{ChangeTracker, V2IntegrationConfig, V2WALIntegrator};
pub use writer::V2WALWriter;

use crate::backend::native::{NativeBackendError, NativeResult};
use std::path::PathBuf;

/// V2 WAL configuration parameters
#[derive(Debug, Clone)]
pub struct V2WALConfig {
    /// Path to the graph file (can be .db, .v2, or any extension)
    pub graph_path: PathBuf,

    /// Path to the main WAL file
    pub wal_path: PathBuf,

    /// Path to the checkpoint tracking file
    pub checkpoint_path: PathBuf,

    /// Maximum WAL file size before forced checkpoint (default: 1GB)
    pub max_wal_size: u64,

    /// Write buffer size for optimal I/O alignment (default: 1MB)
    pub buffer_size: usize,

    /// Checkpoint interval in number of transactions (default: 1000)
    pub checkpoint_interval: u64,

    /// Group commit timeout in milliseconds (default: 10ms)
    pub group_commit_timeout_ms: u64,

    /// Maximum number of records in group commit batch (default: 100)
    pub max_group_commit_size: usize,

    /// Enable compression for WAL records (default: false)
    pub enable_compression: bool,

    /// Compression level if enabled (1-9, default: 3)
    pub compression_level: u8,

    /// Enable automatic checkpointing after commits (default: true)
    pub auto_checkpoint: bool,

    /// Spawn background thread for periodic checkpoint checks (default: false)
    pub background_checkpoint_thread: bool,

    /// Interval for background checkpoint checks in seconds (default: 60)
    pub background_checkpoint_interval_secs: u64,
}

impl Default for V2WALConfig {
    fn default() -> Self {
        Self {
            graph_path: PathBuf::from("v2_graph.db"),
            wal_path: PathBuf::from("v2_graph.wal"),
            checkpoint_path: PathBuf::from("v2_graph.checkpoint"),
            max_wal_size: 1024 * 1024 * 1024, // 1GB
            buffer_size: 1024 * 1024,         // 1MB
            checkpoint_interval: 1000,
            group_commit_timeout_ms: 10,
            max_group_commit_size: 100,
            enable_compression: false,
            compression_level: 3,
            auto_checkpoint: true,
            background_checkpoint_thread: false, // Opt-in for now
            background_checkpoint_interval_secs: 60,
        }
    }
}

impl V2WALConfig {
    /// Create WAL config for the given graph file path
    pub fn for_graph_file(graph_path: &std::path::Path) -> Self {
        let base_path = graph_path.with_extension("");
        Self {
            graph_path: graph_path.to_path_buf(),
            wal_path: base_path.with_extension("wal"),
            checkpoint_path: base_path.with_extension("checkpoint"),
            ..Default::default()
        }
    }

    /// Validate configuration parameters
    pub fn validate(&self) -> NativeResult<()> {
        if self.buffer_size == 0 || !self.buffer_size.is_power_of_two() {
            return Err(NativeBackendError::InvalidConfiguration {
                parameter: "buffer_size".to_string(),
                reason: "must be a non-zero power of two".to_string(),
            });
        }

        if self.max_wal_size < 1024 * 1024 {
            return Err(NativeBackendError::InvalidConfiguration {
                parameter: "max_wal_size".to_string(),
                reason: "must be at least 1MB".to_string(),
            });
        }

        if self.checkpoint_interval == 0 {
            return Err(NativeBackendError::InvalidConfiguration {
                parameter: "checkpoint_interval".to_string(),
                reason: "must be greater than zero".to_string(),
            });
        }

        if self.enable_compression && !(1..=9).contains(&self.compression_level) {
            return Err(NativeBackendError::InvalidConfiguration {
                parameter: "compression_level".to_string(),
                reason: "must be between 1 and 9 when compression is enabled".to_string(),
            });
        }

        Ok(())
    }
}

/// WAL file header for format identification and metadata
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct V2WALHeader {
    /// Magic bytes for V2 WAL format identification
    pub magic: [u8; 8],

    /// WAL format version
    pub version: u32,

    /// WAL file creation timestamp (Unix epoch)
    pub created_at: u64,

    /// Current Log Sequence Number (LSN)
    pub current_lsn: u64,

    /// Committed LSN (all records up to this LSN are durable)
    pub committed_lsn: u64,

    /// Checkpointed LSN (all records up to this LSN are in main file)
    pub checkpointed_lsn: u64,

    /// Number of active transactions
    pub active_transactions: u32,

    /// WAL file flags for feature toggles
    pub flags: u32,

    /// Reserved for future use
    pub reserved: [u64; 4],
}

impl V2WALHeader {
    /// Magic bytes for V2 WAL format
    pub const MAGIC: [u8; 8] = [b'V', b'2', b'W', b'A', b'L', 0, 0, 0];

    /// Current WAL format version
    pub const VERSION: u32 = 1;

    /// Flag: compression enabled
    pub const FLAG_COMPRESSION: u32 = 0x00000001;

    /// Flag: cluster affinity logging enabled
    pub const FLAG_CLUSTER_AFFINITY: u32 = 0x00000002;

    /// Flag: group commit enabled
    pub const FLAG_GROUP_COMMIT: u32 = 0x00000004;

    /// Initialize a new WAL header
    pub fn new() -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            magic: Self::MAGIC,
            version: Self::VERSION,
            created_at: now,
            current_lsn: 1,
            committed_lsn: 0,
            checkpointed_lsn: 0,
            active_transactions: 0,
            flags: Self::FLAG_CLUSTER_AFFINITY | Self::FLAG_GROUP_COMMIT,
            reserved: [0; 4],
        }
    }

    /// Validate WAL header integrity
    pub fn validate(&self) -> NativeResult<()> {
        if self.magic != Self::MAGIC {
            return Err(NativeBackendError::CorruptionDetected {
                context: format!("WAL header: invalid magic bytes: {:?}", self.magic),
                source: None,
            });
        }

        if self.version != Self::VERSION {
            return Err(NativeBackendError::VersionMismatch {
                expected: Self::VERSION.to_string(),
                found: self.version.to_string(),
                source: None,
            });
        }

        if self.current_lsn == 0 {
            return Err(NativeBackendError::CorruptionDetected {
                context: "WAL header: current_lsn cannot be zero".to_string(),
                source: None,
            });
        }

        if self.committed_lsn > self.current_lsn {
            return Err(NativeBackendError::CorruptionDetected {
                context: "WAL header: committed_lsn cannot be greater than current_lsn".to_string(),
                source: None,
            });
        }

        if self.checkpointed_lsn > self.committed_lsn {
            return Err(NativeBackendError::CorruptionDetected {
                context: "WAL header: checkpointed_lsn cannot be greater than committed_lsn"
                    .to_string(),
                source: None,
            });
        }

        Ok(())
    }

    /// Check if a specific feature flag is enabled
    pub fn has_flag(&self, flag: u32) -> bool {
        (self.flags & flag) != 0
    }

    /// Set or clear a feature flag
    pub fn set_flag(&mut self, flag: u32, enabled: bool) {
        if enabled {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }
}

/// Log Sequence Number (LSN) utilities
pub mod lsn {
    /// LSN representing the beginning of the WAL
    pub const LSN_BEGIN: u64 = 1;

    /// LSN representing invalid/uninitialized position
    pub const LSN_INVALID: u64 = 0;

    /// Check if an LSN is valid
    #[inline]
    pub fn is_valid(lsn: u64) -> bool {
        lsn >= LSN_BEGIN
    }

    /// Get the next LSN
    #[inline]
    pub fn next(lsn: u64) -> u64 {
        lsn.wrapping_add(1)
    }

    /// Get the distance between two LSNs
    #[inline]
    pub fn distance(from: u64, to: u64) -> u64 {
        to.saturating_sub(from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v2_wal_config_default() {
        let config = V2WALConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_v2_wal_config_for_graph_file() {
        let graph_path = std::path::Path::new("/tmp/test.graph");
        let config = V2WALConfig::for_graph_file(graph_path);

        assert_eq!(config.wal_path, std::path::Path::new("/tmp/test.wal"));
        assert_eq!(
            config.checkpoint_path,
            std::path::Path::new("/tmp/test.checkpoint")
        );
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_v2_wal_config_validation() {
        let mut config = V2WALConfig::default();

        // Invalid buffer size
        config.buffer_size = 1023; // Not power of two
        assert!(config.validate().is_err());

        config.buffer_size = 0;
        assert!(config.validate().is_err());

        // Valid
        config.buffer_size = 4096;
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_v2_wal_header() {
        let header = V2WALHeader::new();
        assert!(header.validate().is_ok());
        assert_eq!(header.magic, V2WALHeader::MAGIC);
        assert_eq!(header.version, V2WALHeader::VERSION);
        assert!(header.has_flag(V2WALHeader::FLAG_CLUSTER_AFFINITY));
        assert!(header.has_flag(V2WALHeader::FLAG_GROUP_COMMIT));
        assert!(!header.has_flag(V2WALHeader::FLAG_COMPRESSION));
    }

    #[test]
    fn test_lsn_utilities() {
        assert!(!lsn::is_valid(lsn::LSN_INVALID));
        assert!(lsn::is_valid(lsn::LSN_BEGIN));

        assert_eq!(lsn::next(lsn::LSN_BEGIN), lsn::LSN_BEGIN + 1);
        assert_eq!(lsn::distance(10, 15), 5);
        assert_eq!(lsn::distance(20, 15), 0); // Saturates at 0
    }
}
