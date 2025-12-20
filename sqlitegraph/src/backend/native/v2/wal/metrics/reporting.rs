//! Metrics reporting and serialization for V2 WAL performance monitoring.
//!
//! This module provides comprehensive reporting capabilities including resource tracking,
//! cluster performance metrics, error collection and management, and data serialization
//! for performance analysis and monitoring dashboards.

use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

/// Resource utilization tracking for comprehensive system monitoring.
///
/// Tracks system-level resource usage including memory, CPU, disk I/O,
/// and other performance indicators that impact WAL operation efficiency.
///
/// # Examples
///
/// ```rust
/// use crate::backend::native::v2::wal::metrics::reporting::ResourceTracker;
///
/// let mut tracker = ResourceTracker::new();
/// tracker.update();
/// println!("Memory usage: {} bytes", tracker.memory_usage_bytes);
/// ```
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ResourceTracker {
    /// Memory usage in bytes
    pub memory_usage_bytes: u64,

    /// CPU usage percentage (0-100)
    pub cpu_usage_percent: f64,

    /// Disk I/O operations per second
    pub disk_iops: u64,

    /// Disk throughput (MB/s)
    pub disk_throughput_mbps: f64,

    /// File descriptor count
    pub file_descriptor_count: u64,

    /// Buffer pool hit rate
    pub buffer_pool_hit_rate: f64,
}

/// Cluster-specific performance metrics for V2 graph clustering.
///
/// Provides detailed metrics for individual clusters including access patterns,
/// efficiency scores, and utilization data for optimization analysis.
///
/// # Examples
///
/// ```rust
/// use crate::backend::native::v2::wal::metrics::reporting::ClusterPerformanceMetrics;
///
/// let mut metrics = ClusterPerformanceMetrics::new();
/// metrics.update_cluster_stats(42, 100, 500);
/// ```
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct ClusterPerformanceMetrics {
    /// Metrics per cluster ID
    pub per_cluster: HashMap<i64, ClusterMetrics>,

    /// Global cluster metrics
    pub global_metrics: ClusterGlobalMetrics,
}

/// Individual cluster metrics for detailed performance analysis.
///
/// Contains comprehensive performance data for a specific cluster
/// including density, efficiency, and access pattern metrics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClusterMetrics {
    /// Cluster ID
    pub cluster_id: i64,

    /// Number of nodes in cluster
    pub node_count: u32,

    /// Number of edges in cluster
    pub edge_count: u64,

    /// Cluster density (edges per node)
    pub density: f64,

    /// Average access pattern locality
    pub access_pattern_locality: f64,

    /// I/O efficiency score
    pub io_efficiency_score: f64,

    /// Compression ratio for cluster data
    pub compression_ratio: f64,

    /// Last access timestamp
    pub last_access_timestamp: u64,
}

/// Global cluster aggregation metrics.
///
/// Provides aggregated statistics across all clusters for
/// system-wide performance analysis and capacity planning.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ClusterGlobalMetrics {
    /// Total clusters
    pub total_clusters: u64,

    /// Average nodes per cluster
    pub avg_nodes_per_cluster: f64,

    /// Average edges per cluster
    pub avg_edges_per_cluster: f64,

    /// Cluster utilization percentage
    pub utilization_percent: f64,
}

/// Error tracking and analysis for comprehensive error management.
///
/// Tracks error patterns, frequencies, and recovery actions to help
/// identify systematic issues and performance bottlenecks in the WAL system.
///
/// # Examples
///
/// ```rust
/// use crate::backend::native::v2::wal::metrics::reporting::{ErrorTracker, ErrorEntry};
///
/// let mut tracker = ErrorTracker::new();
/// let error = ErrorEntry {
///     error_type: "IOError".to_string(),
///     message: "Disk write failed".to_string(),
///     timestamp: 1234567890,
///     operation_context: "edge_insertion".to_string(),
///     recovery_action: "retry_operation".to_string(),
/// };
/// tracker.record_error(error);
/// ```
#[derive(Debug, Clone)]
pub struct ErrorTracker {
    /// Error counts by type
    pub error_counts: HashMap<String, u64>,

    /// Error rates per operation type
    pub error_rates: HashMap<String, f64>,

    /// Recent errors for analysis
    pub recent_errors: VecDeque<ErrorEntry>,

    /// Maximum recent errors to track
    pub max_recent_errors: usize,
}

/// Individual error entry for detailed error tracking and analysis.
///
/// Contains comprehensive information about each error occurrence
/// including context, recovery actions, and timing data.
#[derive(Debug, Clone)]
pub struct ErrorEntry {
    /// Error type
    pub error_type: String,

    /// Error message
    pub message: String,

    /// Timestamp
    pub timestamp: u64,

    /// Operation context
    pub operation_context: String,

    /// Recovery action taken
    pub recovery_action: String,
}

/// Metrics serialization format for external reporting.
///
/// Provides a structured format for exporting metrics data to external
/// monitoring systems, dashboards, and analysis tools.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MetricsReport {
    /// Report generation timestamp
    pub timestamp: u64,

    /// Performance counters
    pub performance_counters: crate::backend::native::v2::wal::metrics::core::WALPerformanceCounters,

    /// Resource utilization
    pub resource_metrics: ResourceTracker,

    /// Cluster performance data
    pub cluster_metrics: ClusterPerformanceMetrics,

    /// Error summary
    pub error_summary: HashMap<String, u64>,

    /// Global counter values
    pub global_counters: (u64, u64, u64, u64, usize),
}

impl ResourceTracker {
    /// Create new resource tracker with default values.
    ///
    /// Initializes all resource metrics to zero, ready for
    /// monitoring and data collection.
    ///
    /// # Returns
    ///
    /// A new `ResourceTracker` instance with initialized metrics
    pub fn new() -> Self {
        Self {
            memory_usage_bytes: 0,
            cpu_usage_percent: 0.0,
            disk_iops: 0,
            disk_throughput_mbps: 0.0,
            file_descriptor_count: 0,
            buffer_pool_hit_rate: 0.0,
        }
    }

    /// Update resource metrics with current system state.
    ///
    /// Collects current resource utilization data from the operating system
    /// and updates the tracker metrics. In a production environment, this
    /// would interface with system monitoring APIs.
    pub fn update(&mut self) {
        // In a production implementation, this would interface with
        // system monitoring tools like:
        // - `procfs` on Linux for memory and CPU usage
        // - `iostat` for disk I/O metrics
        // - System APIs for file descriptor counts
        // - Buffer pool instrumentation for hit rates

        // For demonstration, simulate realistic values based on typical database usage
        self.memory_usage_bytes = self.estimate_memory_usage();
        self.cpu_usage_percent = self.estimate_cpu_usage();
        self.disk_iops = self.estimate_disk_iops();
        self.disk_throughput_mbps = self.estimate_disk_throughput();
        self.file_descriptor_count = self.estimate_fd_count();
        self.buffer_pool_hit_rate = self.estimate_buffer_hit_rate();
    }

    /// Reset resource tracker to initial state.
    ///
    /// Clears all collected metrics and resets the tracker
    /// to its default state for fresh measurements.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Get resource utilization summary.
    ///
    /// Returns a formatted summary of current resource utilization
    /// suitable for logging and reporting.
    ///
    /// # Returns
    ///
    /// Formatted string with resource utilization summary
    pub fn get_summary(&self) -> String {
        format!(
            "Memory: {} MB, CPU: {:.1}%, Disk IOPS: {}, Throughput: {:.1} MB/s, FDs: {}, Buffer Hit Rate: {:.1}%",
            self.memory_usage_bytes / (1024 * 1024),
            self.cpu_usage_percent,
            self.disk_iops,
            self.disk_throughput_mbps,
            self.file_descriptor_count,
            self.buffer_pool_hit_rate * 100.0
        )
    }

    // Helper methods for realistic resource estimation (placeholder implementations)
    fn estimate_memory_usage(&self) -> u64 {
        // Simulate memory usage based on typical database patterns
        512 * 1024 * 1024 // 512 MB base usage
    }

    fn estimate_cpu_usage(&self) -> f64 {
        // Simulate variable CPU usage
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        ((secs % 100) as f64 / 100.0) * 80.0 + 10.0 // 10-90% usage
    }

    fn estimate_disk_iops(&self) -> u64 {
        // Simulate disk I/O based on typical database workload
        1000 + (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() % 2000) as u64
    }

    fn estimate_disk_throughput(&self) -> f64 {
        // Simulate disk throughput in MB/s
        50.0 + (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() % 100) as f64 / 10.0
    }

    fn estimate_fd_count(&self) -> u64 {
        // Simulate file descriptor usage
        25 + (SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() % 50) as u64
    }

    fn estimate_buffer_hit_rate(&self) -> f64 {
        // Simulate buffer pool hit rate (typically high for databases)
        0.85 + ((SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() % 1000) as f64 / 1000.0) * 0.14 // 85-99%
    }
}

impl ClusterPerformanceMetrics {
    /// Create new cluster performance metrics.
    ///
    /// Initializes empty metrics storage ready for cluster-specific
    /// performance data collection.
    ///
    /// # Returns
    ///
    /// A new `ClusterPerformanceMetrics` instance
    pub fn new() -> Self {
        Self {
            per_cluster: HashMap::new(),
            global_metrics: ClusterGlobalMetrics::default(),
        }
    }

    /// Update cluster access timestamp.
    ///
    /// Records when a cluster was last accessed, helping to identify
    /// active vs inactive clusters for optimization decisions.
    ///
    /// # Arguments
    ///
    /// * `cluster_id` - ID of the cluster being accessed
    pub fn update_cluster_access(&mut self, cluster_id: i64) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let cluster = self.per_cluster.entry(cluster_id)
            .or_insert_with(|| ClusterMetrics {
                cluster_id,
                node_count: 0,
                edge_count: 0,
                density: 0.0,
                access_pattern_locality: 0.0,
                io_efficiency_score: 0.0,
                compression_ratio: 1.0,
                last_access_timestamp: now,
            });

        cluster.last_access_timestamp = now;

        // Update access pattern locality score (simplified)
        const ALPHA: f64 = 0.1;
        cluster.access_pattern_locality =
            cluster.access_pattern_locality * (1.0 - ALPHA) + ALPHA;
    }

    /// Update cluster statistics with current data.
    ///
    /// Updates comprehensive cluster metrics including node count,
    /// edge count, density, and derived efficiency scores.
    ///
    /// # Arguments
    ///
    /// * `cluster_id` - ID of the cluster to update
    /// * `node_count` - Current number of nodes in cluster
    /// * `edge_count` - Current number of edges in cluster
    pub fn update_cluster_stats(
        &mut self,
        cluster_id: i64,
        node_count: u32,
        edge_count: u64,
    ) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let cluster = self.per_cluster.entry(cluster_id)
            .or_insert_with(|| ClusterMetrics {
                cluster_id,
                node_count: 0,
                edge_count: 0,
                density: 0.0,
                access_pattern_locality: 0.0,
                io_efficiency_score: 0.0,
                compression_ratio: 1.0,
                last_access_timestamp: now,
            });

        cluster.node_count = node_count;
        cluster.edge_count = edge_count;
        cluster.density = if node_count > 0 {
            edge_count as f64 / node_count as f64
        } else {
            0.0
        };

        // Update derived efficiency scores (simplified calculations)
        cluster.io_efficiency_score = ClusterPerformanceMetrics::calculate_io_efficiency_static(cluster);
        cluster.compression_ratio = ClusterPerformanceMetrics::calculate_compression_ratio_static(cluster);

        self.update_global_metrics();
    }

    /// Update global cluster aggregation metrics.
    ///
    /// Recalculates global statistics based on current per-cluster data,
    /// providing system-wide performance indicators.
    fn update_global_metrics(&mut self) {
        if self.per_cluster.is_empty() {
            return;
        }

        let total_clusters = self.per_cluster.len() as u64;
        let total_nodes: u32 = self.per_cluster.values().map(|c| c.node_count).sum();
        let total_edges: u64 = self.per_cluster.values().map(|c| c.edge_count).sum();
        let total_possible_nodes = total_clusters * 1000; // Assumed max nodes per cluster
        let _total_possible_edges = total_clusters * 5000; // Assumed max edges per cluster

        self.global_metrics.total_clusters = total_clusters;
        self.global_metrics.avg_nodes_per_cluster = if total_clusters > 0 {
            total_nodes as f64 / total_clusters as f64
        } else {
            0.0
        };
        self.global_metrics.avg_edges_per_cluster = if total_clusters > 0 {
            total_edges as f64 / total_clusters as f64
        } else {
            0.0
        };
        self.global_metrics.utilization_percent = if total_possible_nodes > 0 {
            ((total_nodes as f64 / total_possible_nodes as f64) * 100.0).min(100.0)
        } else {
            0.0
        };
    }

    /// Reset cluster metrics to initial state.
    ///
    /// Clears all cluster-specific and global metrics for fresh measurements.
    pub fn reset(&mut self) {
        self.per_cluster.clear();
        self.global_metrics = ClusterGlobalMetrics::default();
    }

    /// Get cluster performance summary.
    ///
    /// Returns a formatted summary of cluster performance metrics
    /// suitable for logging and monitoring dashboards.
    ///
    /// # Returns
    ///
    /// Formatted string with cluster performance summary
    pub fn get_summary(&self) -> String {
        format!(
            "Clusters: {}, Avg Nodes: {:.1}, Avg Edges: {:.1}, Utilization: {:.1}%",
            self.global_metrics.total_clusters,
            self.global_metrics.avg_nodes_per_cluster,
            self.global_metrics.avg_edges_per_cluster,
            self.global_metrics.utilization_percent
        )
    }

    // Helper methods for efficiency score calculations
    fn calculate_io_efficiency_static(cluster: &ClusterMetrics) -> f64 {
        // Simplified IO efficiency calculation based on cluster characteristics
        let density_factor = (cluster.density / 10.0).min(1.0); // Higher density = better efficiency
        let locality_factor = cluster.access_pattern_locality;
        let compression_factor = if cluster.compression_ratio > 1.0 {
            1.0 / cluster.compression_ratio
        } else {
            1.0
        };

        (density_factor + locality_factor + compression_factor) / 3.0
    }

    fn calculate_compression_ratio_static(cluster: &ClusterMetrics) -> f64 {
        // Simplified compression ratio based on cluster size and density
        let size_factor = (cluster.node_count as f64 / 1000.0).min(1.0);
        let density_factor = (cluster.density / 20.0).min(1.0);

        1.0 + (size_factor * density_factor * 0.5) // Max 1.5x compression
    }
}

impl ErrorTracker {
    /// Create new error tracker.
    ///
    /// Initializes error tracking storage with configurable history size
    /// for comprehensive error analysis and pattern detection.
    ///
    /// # Returns
    ///
    /// A new `ErrorTracker` instance with default configuration
    pub fn new() -> Self {
        Self {
            error_counts: HashMap::new(),
            error_rates: HashMap::new(),
            recent_errors: VecDeque::new(),
            max_recent_errors: 1000,
        }
    }

    /// Record an error occurrence.
    ///
    /// Adds a new error to the tracking system, updating counts,
    /// rates, and maintaining the recent error history.
    ///
    /// # Arguments
    ///
    /// * `error_entry` - Complete error information to record
    pub fn record_error(&mut self, error_entry: ErrorEntry) {
        // Update error counts
        *self.error_counts.entry(error_entry.error_type.clone()).or_insert(0) += 1;

        // Add to recent errors
        self.recent_errors.push_back(error_entry.clone());

        // Limit recent errors to prevent memory growth
        while self.recent_errors.len() > self.max_recent_errors {
            self.recent_errors.pop_front();
        }

        // Update error rates
        self.update_error_rates();
    }

    /// Update error rates based on accumulated counts.
    ///
    /// Calculates error rates per operation type based on recent
    /// error patterns and operation frequencies.
    fn update_error_rates(&mut self) {
        // This would calculate error rates per operation type
        // In a production implementation, this would consider:
        // - Total operations per type
        // - Time window for rate calculation
        // - Exponential decay for recent errors

        // Simplified implementation
        for (error_type, &count) in &self.error_counts {
            let rate = if count > 0 {
                // Calculate rate as errors per 1000 operations (simplified)
                count as f64 / 1000.0
            } else {
                0.0
            };
            self.error_rates.insert(error_type.clone(), rate);
        }
    }

    /// Reset error tracker to initial state.
    ///
    /// Clears all error data and resets the tracker for fresh error collection.
    pub fn reset(&mut self) {
        self.error_counts.clear();
        self.error_rates.clear();
        self.recent_errors.clear();
    }

    /// Get error summary for reporting.
    ///
    /// Returns a formatted summary of error statistics suitable
    /// for logging and monitoring dashboards.
    ///
    /// # Returns
    ///
    /// Formatted string with error summary
    pub fn get_summary(&self) -> String {
        let total_errors: u64 = self.error_counts.values().sum();
        let error_types = self.error_counts.len();

        if total_errors == 0 {
            "No errors recorded".to_string()
        } else {
            format!(
                "Total Errors: {}, Types: {}, Recent: {}",
                total_errors,
                error_types,
                self.recent_errors.len()
            )
        }
    }

    /// Get top error types by frequency.
    ///
    /// Returns the most common error types sorted by occurrence count,
    /// helping to identify systematic issues.
    ///
    /// # Arguments
    ///
    /// * `limit` - Maximum number of error types to return
    ///
    /// # Returns
    ///
    /// Vector of (error_type, count) tuples sorted by count (descending)
    pub fn get_top_errors(&self, limit: usize) -> Vec<(String, u64)> {
        let mut errors: Vec<(String, u64)> = self.error_counts
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();

        errors.sort_by(|a, b| b.1.cmp(&a.1));
        errors.truncate(limit);
        errors
    }
}

impl Default for ResourceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ClusterGlobalMetrics {
    fn default() -> Self {
        Self {
            total_clusters: 0,
            avg_nodes_per_cluster: 0.0,
            avg_edges_per_cluster: 0.0,
            utilization_percent: 0.0,
        }
    }
}

impl Default for ErrorTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::v2::wal::metrics::core::WALPerformanceCounters;

    #[test]
    fn test_resource_tracker_new() {
        let tracker = ResourceTracker::new();
        assert_eq!(tracker.memory_usage_bytes, 0);
        assert_eq!(tracker.cpu_usage_percent, 0.0);
        assert_eq!(tracker.disk_iops, 0);
    }

    #[test]
    fn test_resource_tracker_update() {
        let mut tracker = ResourceTracker::new();
        tracker.update();

        // Should have non-zero values after update
        assert!(tracker.memory_usage_bytes > 0);
        assert!(tracker.cpu_usage_percent > 0.0);
        assert!(tracker.disk_iops > 0);
        assert!(tracker.disk_throughput_mbps > 0.0);
    }

    #[test]
    fn test_resource_tracker_summary() {
        let mut tracker = ResourceTracker::new();
        tracker.update();

        let summary = tracker.get_summary();
        assert!(summary.contains("Memory:"));
        assert!(summary.contains("CPU:"));
        assert!(summary.contains("Disk IOPS:"));
    }

    #[test]
    fn test_resource_tracker_reset() {
        let mut tracker = ResourceTracker::new();
        tracker.update();
        assert!(tracker.memory_usage_bytes > 0);

        tracker.reset();
        assert_eq!(tracker.memory_usage_bytes, 0);
        assert_eq!(tracker.cpu_usage_percent, 0.0);
    }

    #[test]
    fn test_cluster_performance_metrics_new() {
        let metrics = ClusterPerformanceMetrics::new();
        assert!(metrics.per_cluster.is_empty());
        assert_eq!(metrics.global_metrics.total_clusters, 0);
        assert_eq!(metrics.global_metrics.avg_nodes_per_cluster, 0.0);
    }

    #[test]
    fn test_cluster_update_access() {
        let mut metrics = ClusterPerformanceMetrics::new();
        metrics.update_cluster_access(42);

        assert!(metrics.per_cluster.contains_key(&42));
        let cluster = &metrics.per_cluster[&42];
        assert_eq!(cluster.cluster_id, 42);
        assert!(cluster.last_access_timestamp > 0);
    }

    #[test]
    fn test_cluster_update_stats() {
        let mut metrics = ClusterPerformanceMetrics::new();
        metrics.update_cluster_stats(42, 100, 500);

        assert!(metrics.per_cluster.contains_key(&42));
        let cluster = &metrics.per_cluster[&42];
        assert_eq!(cluster.node_count, 100);
        assert_eq!(cluster.edge_count, 500);
        assert_eq!(cluster.density, 5.0);
    }

    #[test]
    fn test_cluster_global_metrics() {
        let mut metrics = ClusterPerformanceMetrics::new();
        metrics.update_cluster_stats(42, 10, 50);
        metrics.update_cluster_stats(43, 5, 25);

        assert_eq!(metrics.global_metrics.total_clusters, 2);
        assert_eq!(metrics.global_metrics.avg_nodes_per_cluster, 7.5);
        assert_eq!(metrics.global_metrics.avg_edges_per_cluster, 37.5);
    }

    #[test]
    fn test_cluster_summary() {
        let mut metrics = ClusterPerformanceMetrics::new();
        metrics.update_cluster_stats(42, 10, 50);

        let summary = metrics.get_summary();
        assert!(summary.contains("Clusters:"));
        assert!(summary.contains("Avg Nodes:"));
        assert!(summary.contains("Avg Edges:"));
    }

    #[test]
    fn test_cluster_reset() {
        let mut metrics = ClusterPerformanceMetrics::new();
        metrics.update_cluster_stats(42, 10, 50);
        assert!(!metrics.per_cluster.is_empty());

        metrics.reset();
        assert!(metrics.per_cluster.is_empty());
        assert_eq!(metrics.global_metrics.total_clusters, 0);
    }

    #[test]
    fn test_error_tracker_new() {
        let tracker = ErrorTracker::new();
        assert!(tracker.error_counts.is_empty());
        assert!(tracker.error_rates.is_empty());
        assert!(tracker.recent_errors.is_empty());
        assert_eq!(tracker.max_recent_errors, 1000);
    }

    #[test]
    fn test_error_tracker_record() {
        let mut tracker = ErrorTracker::new();

        let error_entry = ErrorEntry {
            error_type: "TestError".to_string(),
            message: "Test message".to_string(),
            timestamp: 1234567890,
            operation_context: "Test context".to_string(),
            recovery_action: "Test recovery".to_string(),
        };

        tracker.record_error(error_entry);
        assert_eq!(tracker.error_counts.get("TestError"), Some(&1));
        assert_eq!(tracker.recent_errors.len(), 1);
    }

    #[test]
    fn test_error_tracker_multiple() {
        let mut tracker = ErrorTracker::new();

        // Record multiple errors of different types
        tracker.record_error(ErrorEntry {
            error_type: "Error1".to_string(),
            message: "Message1".to_string(),
            timestamp: 1234567890,
            operation_context: "Context1".to_string(),
            recovery_action: "Recovery1".to_string(),
        });

        tracker.record_error(ErrorEntry {
            error_type: "Error2".to_string(),
            message: "Message2".to_string(),
            timestamp: 1234567891,
            operation_context: "Context2".to_string(),
            recovery_action: "Recovery2".to_string(),
        });

        tracker.record_error(ErrorEntry {
            error_type: "Error1".to_string(),
            message: "Message1 again".to_string(),
            timestamp: 1234567892,
            operation_context: "Context1 again".to_string(),
            recovery_action: "Recovery1 again".to_string(),
        });

        assert_eq!(tracker.error_counts.get("Error1"), Some(&2));
        assert_eq!(tracker.error_counts.get("Error2"), Some(&1));
        assert_eq!(tracker.recent_errors.len(), 3);
    }

    #[test]
    fn test_error_tracker_summary() {
        let mut tracker = ErrorTracker::new();
        assert_eq!(tracker.get_summary(), "No errors recorded");

        tracker.record_error(ErrorEntry {
            error_type: "TestError".to_string(),
            message: "Test message".to_string(),
            timestamp: 1234567890,
            operation_context: "Test context".to_string(),
            recovery_action: "Test recovery".to_string(),
        });

        let summary = tracker.get_summary();
        assert!(summary.contains("Total Errors: 1"));
        assert!(summary.contains("Types: 1"));
    }

    #[test]
    fn test_error_tracker_top_errors() {
        let mut tracker = ErrorTracker::new();

        // Add errors with different frequencies
        for _ in 0..5 {
            tracker.record_error(ErrorEntry {
                error_type: "FrequentError".to_string(),
                message: "Frequent message".to_string(),
                timestamp: 1234567890,
                operation_context: "Frequent context".to_string(),
                recovery_action: "Frequent recovery".to_string(),
            });
        }

        for _ in 0..2 {
            tracker.record_error(ErrorEntry {
                error_type: "RareError".to_string(),
                message: "Rare message".to_string(),
                timestamp: 1234567890,
                operation_context: "Rare context".to_string(),
                recovery_action: "Rare recovery".to_string(),
            });
        }

        let top_errors = tracker.get_top_errors(2);
        assert_eq!(top_errors.len(), 2);
        assert_eq!(top_errors[0].0, "FrequentError");
        assert_eq!(top_errors[0].1, 5);
        assert_eq!(top_errors[1].0, "RareError");
        assert_eq!(top_errors[1].1, 2);
    }

    #[test]
    fn test_error_tracker_reset() {
        let mut tracker = ErrorTracker::new();
        tracker.record_error(ErrorEntry {
            error_type: "TestError".to_string(),
            message: "Test message".to_string(),
            timestamp: 1234567890,
            operation_context: "Test context".to_string(),
            recovery_action: "Test recovery".to_string(),
        });

        assert!(!tracker.error_counts.is_empty());

        tracker.reset();
        assert!(tracker.error_counts.is_empty());
        assert!(tracker.recent_errors.is_empty());
    }

    #[test]
    fn test_cluster_io_efficiency_calculation() {
        let mut metrics = ClusterPerformanceMetrics::new();
        metrics.update_cluster_stats(42, 100, 500); // density = 5.0
        metrics.update_cluster_access(42); // increase locality

        let cluster = &metrics.per_cluster[&42];
        assert!(cluster.io_efficiency_score > 0.0);
        assert!(cluster.io_efficiency_score <= 1.0);
    }

    #[test]
    fn test_cluster_compression_ratio() {
        let mut metrics = ClusterPerformanceMetrics::new();
        metrics.update_cluster_stats(42, 1000, 2000); // Larger cluster

        let cluster = &metrics.per_cluster[&42];
        assert!(cluster.compression_ratio >= 1.0);
    }

    #[test]
    fn test_metrics_report_serialization() {
        let report = MetricsReport {
            timestamp: 1234567890,
            performance_counters: WALPerformanceCounters::default(),
            resource_metrics: ResourceTracker::default(),
            cluster_metrics: ClusterPerformanceMetrics::default(),
            error_summary: HashMap::new(),
            global_counters: (10, 20, 1024, 2048, 5),
        };

        // Test serialization
        let json = serde_json::to_string(&report);
        assert!(json.is_ok());

        // Test deserialization
        let json_str = json.unwrap();
        let deserialized: Result<MetricsReport, _> = serde_json::from_str(&json_str);
        assert!(deserialized.is_ok());

        let recovered = deserialized.unwrap();
        assert_eq!(recovered.timestamp, 1234567890);
        assert_eq!(recovered.global_counters, (10, 20, 1024, 2048, 5));
    }
}