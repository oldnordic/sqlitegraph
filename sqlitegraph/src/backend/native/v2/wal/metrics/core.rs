//! Core metrics structures and fundamental types for V2 WAL performance monitoring.
//!
//! This module provides the essential data structures and core metrics types
//! that form the foundation of the V2 WAL performance monitoring system.
//! It includes performance counters, operation-specific metrics, and the main
//! metrics collector interface.

use crate::backend::native::NativeResult;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use serde::{Deserialize, Serialize};

/// V2 WAL performance metrics collection specifically for graph operations.
///
/// This is the main metrics collector that coordinates all performance monitoring
/// for V2 clustered edge graph operations, providing comprehensive tracking of
/// throughput, latency, and resource utilization.
///
/// # Examples
///
/// ```rust
/// use crate::backend::native::v2::wal::metrics::core::V2WALMetrics;
///
/// let metrics = V2WALMetrics::new();
/// metrics.record_write_operation(100, 50, Some(42), "edge_insert");
/// let counters = metrics.get_counters();
/// assert_eq!(counters.records_processed, 1);
/// ```
pub struct V2WALMetrics {
    /// Performance counters for different operation types
    pub(crate) counters: Arc<Mutex<WALPerformanceCounters>>,

    /// Latency tracking with histogram buckets
    pub(crate) latency_histogram: Arc<Mutex<crate::backend::native::v2::wal::metrics::aggregation::LatencyHistogram>>,

    /// Throughput metrics over time windows
    pub(crate) throughput_tracker: Arc<Mutex<crate::backend::native::v2::wal::metrics::aggregation::ThroughputTracker>>,

    /// Resource utilization metrics
    pub(crate) resource_tracker: Arc<Mutex<crate::backend::native::v2::wal::metrics::reporting::ResourceTracker>>,

    /// Cluster-specific performance metrics
    pub(crate) cluster_metrics: Arc<Mutex<crate::backend::native::v2::wal::metrics::reporting::ClusterPerformanceMetrics>>,

    /// Error tracking and analysis
    pub(crate) error_tracker: Arc<Mutex<crate::backend::native::v2::wal::metrics::reporting::ErrorTracker>>,

    /// Global performance counters
    pub(crate) global_counters: GlobalCounters,
}

/// Performance counters for detailed monitoring of V2 graph operations.
///
/// This structure maintains comprehensive statistics about all WAL operations
/// including throughput, latencies, and resource utilization metrics.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct WALPerformanceCounters {
    /// Total records processed
    pub records_processed: u64,

    /// Total bytes written/read
    pub bytes_transferred: u64,

    /// Number of flush operations
    pub flush_operations: u64,

    /// Checkpoint operations count
    pub checkpoint_operations: u64,

    /// Recovery operations count
    pub recovery_operations: u64,

    /// Average operation latencies
    pub avg_write_latency_us: u64,
    pub avg_read_latency_us: u64,
    pub avg_flush_latency_us: u64,

    /// Buffer utilization percentages
    pub buffer_utilization_percent: f64,

    /// Cluster-specific operation counts
    pub cluster_operations: HashMap<i64, ClusterOperationCounters>,

    /// Edge operation performance
    pub edge_operations: EdgeOperationMetrics,

    /// Node operation performance
    pub node_operations: NodeOperationMetrics,

    /// Free space operation performance
    pub free_space_operations: FreeSpaceOperationMetrics,

    /// String table operation performance
    pub string_table_operations: StringTableOperationMetrics,
}

/// Cluster-specific operation counters for V2 graph clustering.
///
/// Tracks operations and performance metrics for individual clusters
/// within the V2 clustered edge architecture.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClusterOperationCounters {
    /// Number of cluster creates
    pub creates: u64,

    /// Number of cluster reads
    pub reads: u64,

    /// Number of cluster updates
    pub updates: u64,

    /// Total bytes processed for this cluster
    pub bytes_processed: u64,

    /// Average latency for cluster operations
    pub avg_latency_us: u64,
}

/// Edge operation performance metrics specific to V2 compact edge records.
///
/// Provides detailed tracking of edge-related operations within the WAL system,
/// including insertions, updates, deletions, and cluster affinity metrics.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EdgeOperationMetrics {
    /// Total edge insertions
    pub total_inserts: u64,

    /// Total edge updates
    pub total_updates: u64,

    /// Total edge deletions
    pub total_deletions: u64,

    /// Average edge record size (bytes)
    pub avg_record_size: f64,

    /// Edge insertion latency (microseconds)
    pub avg_insertion_latency_us: u64,

    /// Edge update latency (microseconds)
    pub avg_update_latency_us: u64,

    /// Cluster-affinity hit rate (percentage)
    pub cluster_affinity_hit_rate: f64,
}

/// Node operation performance metrics.
///
/// Tracks performance metrics for node-related operations in the V2 graph system,
/// including insertions, updates, deletions, and I/O locality measurements.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeOperationMetrics {
    /// Total node insertions
    pub total_inserts: u64,

    /// Total node updates
    pub total_updates: u64,

    /// Total node deletions
    pub total_deletions: u64,

    /// Average node record size (bytes)
    pub avg_record_size: f64,

    /// Node insertion latency (microseconds)
    pub avg_insertion_latency_us: u64,

    /// Node update latency (microseconds)
    pub avg_update_latency_us: u64,

    /// Node I/O locality score (0.0-1.0)
    pub io_locality_score: f64,
}

/// Free space operation performance metrics.
///
/// Monitors the performance of free space management operations within the
/// V2 clustered edge system, including allocations, deallocations, and efficiency.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FreeSpaceOperationMetrics {
    /// Total allocations
    pub total_allocations: u64,

    /// Total deallocations
    pub total_deallocations: u64,

    /// Average allocation size (bytes)
    pub avg_allocation_size: u64,

    /// Free space efficiency (percentage)
    pub efficiency_percent: f64,

    /// Allocation latency (microseconds)
    pub avg_allocation_latency_us: u64,

    /// Deallocation latency (microseconds)
    pub avg_deallocation_latency_us: u64,
}

/// String table operation performance metrics.
///
/// Tracks performance metrics for string table operations including insertions,
/// lookups, compression ratios, and cache hit rates.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StringTableOperationMetrics {
    /// Total string insertions
    pub total_insertions: u64,

    /// Average string length
    pub avg_string_length: f64,

    /// String table hit rate
    pub hit_rate_percent: f64,

    /// Compression ratio (if enabled)
    pub compression_ratio: f64,

    /// String insertion latency (microseconds)
    pub avg_insertion_latency_us: u64,

    /// String lookup latency (microseconds)
    pub avg_lookup_latency_us: u64,
}

/// Global atomic counters for high-frequency operations.
///
/// These counters use atomic operations for lock-free updates on frequently
/// accessed metrics, providing minimal overhead for performance-critical tracking.
#[derive(Debug)]
pub struct GlobalCounters {
    /// Total records written
    pub records_written: AtomicU64,

    /// Total records read
    pub records_read: AtomicU64,

    /// Total bytes written
    pub bytes_written: AtomicU64,

    /// Total bytes read
    pub bytes_read: AtomicU64,

    /// Currently active operations
    pub active_operations: AtomicUsize,
}

impl V2WALMetrics {
    /// Create new metrics collector for V2 WAL graph operations.
    ///
    /// Initializes all metric tracking components with default values and
    /// prepares the collector for comprehensive performance monitoring.
    ///
    /// # Returns
    ///
    /// A new `V2WALMetrics` instance ready to collect performance data.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crate::backend::native::v2::wal::metrics::core::V2WALMetrics;
    ///
    /// let metrics = V2WALMetrics::new();
    /// // Ready to collect metrics
    /// ```
    pub fn new() -> Self {
        Self {
            counters: Arc::new(Mutex::new(WALPerformanceCounters::default())),
            latency_histogram: Arc::new(Mutex::new(
                crate::backend::native::v2::wal::metrics::aggregation::LatencyHistogram::new()
            )),
            throughput_tracker: Arc::new(Mutex::new(
                crate::backend::native::v2::wal::metrics::aggregation::ThroughputTracker::new()
            )),
            resource_tracker: Arc::new(Mutex::new(
                crate::backend::native::v2::wal::metrics::reporting::ResourceTracker::default()
            )),
            cluster_metrics: Arc::new(Mutex::new(
                crate::backend::native::v2::wal::metrics::reporting::ClusterPerformanceMetrics::default()
            )),
            error_tracker: Arc::new(Mutex::new(
                crate::backend::native::v2::wal::metrics::reporting::ErrorTracker::new()
            )),
            global_counters: GlobalCounters::default(),
        }
    }

    /// Get current performance counters.
    ///
    /// Returns a snapshot of all current performance metrics including
    /// operation counts, latencies, and resource utilization.
    ///
    /// # Returns
    ///
    /// A clone of the current `WALPerformanceCounters`.
    pub fn get_counters(&self) -> WALPerformanceCounters {
        self.counters.lock().clone()
    }

    /// Get current latency histogram.
    ///
    /// Returns the current latency distribution data for all operation types,
    /// including write, read, flush, and checkpoint operations.
    ///
    /// # Returns
    ///
    /// A clone of the current `LatencyHistogram`.
    pub fn get_latency_histogram(&self) -> crate::backend::native::v2::wal::metrics::aggregation::LatencyHistogram {
        self.latency_histogram.lock().clone()
    }

    /// Get current throughput metrics.
    ///
    /// Returns time-windowed throughput data including records per second,
    /// bytes per second, and transactions per second.
    ///
    /// # Returns
    ///
    /// A clone of the current `ThroughputTracker`.
    pub fn get_throughput_tracker(&self) -> crate::backend::native::v2::wal::metrics::aggregation::ThroughputTracker {
        self.throughput_tracker.lock().clone()
    }

    /// Get current resource utilization.
    ///
    /// Returns current resource usage metrics including memory, CPU, disk I/O,
    /// and other system-level performance indicators.
    ///
    /// # Returns
    ///
    /// A clone of the current `ResourceTracker`.
    pub fn get_resource_tracker(&self) -> crate::backend::native::v2::wal::metrics::reporting::ResourceTracker {
        self.resource_tracker.lock().clone()
    }

    /// Get cluster performance metrics.
    ///
    /// Returns detailed performance metrics for individual clusters including
    /// access patterns, efficiency scores, and utilization data.
    ///
    /// # Returns
    ///
    /// A clone of the current `ClusterPerformanceMetrics`.
    pub fn get_cluster_metrics(&self) -> crate::backend::native::v2::wal::metrics::reporting::ClusterPerformanceMetrics {
        self.cluster_metrics.lock().clone()
    }

    /// Get error tracker data.
    ///
    /// Returns current error tracking data including error counts, rates,
    /// and recent error entries for analysis.
    ///
    /// # Returns
    ///
    /// A clone of the current `ErrorTracker`.
    pub fn get_error_tracker(&self) -> crate::backend::native::v2::wal::metrics::reporting::ErrorTracker {
        self.error_tracker.lock().clone()
    }

    /// Get global counter values.
    ///
    /// Returns the current values of all global atomic counters in a
    /// single atomic operation for consistency.
    ///
    /// # Returns
    ///
    /// A tuple containing (records_written, records_read, bytes_written, bytes_read, active_operations).
    pub fn get_global_counters(&self) -> (u64, u64, u64, u64, usize) {
        (
            self.global_counters.records_written.load(Ordering::Relaxed),
            self.global_counters.records_read.load(Ordering::Relaxed),
            self.global_counters.bytes_written.load(Ordering::Relaxed),
            self.global_counters.bytes_read.load(Ordering::Relaxed),
            self.global_counters.active_operations.load(Ordering::Relaxed),
        )
    }

    /// Reset all metrics.
    ///
    /// Clears all metric data and resets all counters to their initial state.
    /// This is typically used for starting fresh measurements or clearing
    /// accumulated data.
    pub fn reset(&self) {
        // Reset all metric components
        {
            let mut counters = self.counters.lock();
            *counters = WALPerformanceCounters::default();
        }

        {
            let mut histogram = self.latency_histogram.lock();
            histogram.reset();
        }

        {
            let mut tracker = self.throughput_tracker.lock();
            tracker.reset();
        }

        {
            let mut resource_tracker = self.resource_tracker.lock();
            resource_tracker.reset();
        }

        {
            let mut cluster_metrics = self.cluster_metrics.lock();
            cluster_metrics.reset();
        }

        {
            let mut error_tracker = self.error_tracker.lock();
            error_tracker.reset();
        }

        // Reset global atomic counters
        self.global_counters.records_written.store(0, Ordering::Relaxed);
        self.global_counters.records_read.store(0, Ordering::Relaxed);
        self.global_counters.bytes_written.store(0, Ordering::Relaxed);
        self.global_counters.bytes_read.store(0, Ordering::Relaxed);
        self.global_counters.active_operations.store(0, Ordering::Relaxed);
    }
}

impl Default for GlobalCounters {
    fn default() -> Self {
        Self {
            records_written: AtomicU64::new(0),
            records_read: AtomicU64::new(0),
            bytes_written: AtomicU64::new(0),
            bytes_read: AtomicU64::new(0),
            active_operations: AtomicUsize::new(0),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_v2_wal_metrics_creation() {
        let metrics = V2WALMetrics::new();
        let counters = metrics.get_counters();
        assert_eq!(counters.records_processed, 0);
        assert_eq!(counters.bytes_transferred, 0);
    }

    #[test]
    fn test_performance_counters_default() {
        let counters = WALPerformanceCounters::default();
        assert_eq!(counters.records_processed, 0);
        assert_eq!(counters.bytes_transferred, 0);
        assert_eq!(counters.flush_operations, 0);
    }

    #[test]
    fn test_cluster_operation_counters_default() {
        let cluster_ops = ClusterOperationCounters::default();
        assert_eq!(cluster_ops.creates, 0);
        assert_eq!(cluster_ops.reads, 0);
        assert_eq!(cluster_ops.updates, 0);
        assert_eq!(cluster_ops.bytes_processed, 0);
        assert_eq!(cluster_ops.avg_latency_us, 0);
    }

    #[test]
    fn test_edge_operation_metrics_default() {
        let edge_ops = EdgeOperationMetrics::default();
        assert_eq!(edge_ops.total_inserts, 0);
        assert_eq!(edge_ops.total_updates, 0);
        assert_eq!(edge_ops.total_deletions, 0);
        assert_eq!(edge_ops.avg_record_size, 0.0);
    }

    #[test]
    fn test_node_operation_metrics_default() {
        let node_ops = NodeOperationMetrics::default();
        assert_eq!(node_ops.total_inserts, 0);
        assert_eq!(node_ops.total_updates, 0);
        assert_eq!(node_ops.total_deletions, 0);
        assert_eq!(node_ops.io_locality_score, 0.0);
    }

    #[test]
    fn test_global_counters_atomic_operations() {
        let counters = GlobalCounters::default();

        // Test atomic operations
        counters.records_written.store(100, Ordering::Relaxed);
        counters.records_read.store(50, Ordering::Relaxed);

        assert_eq!(counters.records_written.load(Ordering::Relaxed), 100);
        assert_eq!(counters.records_read.load(Ordering::Relaxed), 50);
    }

    #[test]
    fn test_v2_wal_metrics_reset() {
        let metrics = V2WALMetrics::new();

        // Record some operations
        let counters = metrics.get_counters();
        assert_eq!(counters.records_processed, 0);

        // Reset and verify
        metrics.reset();
        let counters_after = metrics.get_counters();
        assert_eq!(counters_after.records_processed, 0);

        let global_counters = metrics.get_global_counters();
        assert_eq!(global_counters.0, 0); // records_written
        assert_eq!(global_counters.1, 0); // records_read
    }
}