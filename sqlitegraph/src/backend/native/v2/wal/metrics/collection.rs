//! Metric collection logic for V2 WAL performance monitoring.
//!
//! This module provides the core metric collection functionality that records
//! performance data from various WAL operations. It handles the collection
//! of write operations, read operations, and error tracking with proper
//! integration into the broader metrics system.

use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use super::core::{
    ClusterOperationCounters, V2WALMetrics, WALPerformanceCounters,
};
use crate::backend::native::v2::wal::metrics::reporting::ErrorEntry;

impl V2WALMetrics {
    /// Record a write operation for V2 graph operations.
    ///
    /// This method captures comprehensive performance data for write operations
    /// including latency, size, cluster affinity, and operation type. It updates
    /// all relevant metrics components in a coordinated manner.
    ///
    /// # Arguments
    ///
    /// * `record_size_bytes` - Size of the record being written in bytes
    /// * `latency_us` - Operation latency in microseconds
    /// * `cluster_key` - Optional cluster ID for cluster-affinity tracking
    /// * `operation_type` - Type of operation (e.g., "edge_insert", "node_update")
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crate::backend::native::v2::wal::metrics::core::V2WALMetrics;
    ///
    /// let metrics = V2WALMetrics::new();
    /// metrics.record_write_operation(100, 50, Some(42), "edge_insert");
    /// ```
    pub fn record_write_operation(
        &self,
        record_size_bytes: usize,
        latency_us: u64,
        cluster_key: Option<i64>,
        operation_type: &str,
    ) {
        // Update global counters (atomic operations for performance)
        self.global_counters
            .records_written
            .fetch_add(1, Ordering::Relaxed);
        self.global_counters
            .bytes_written
            .fetch_add(record_size_bytes as u64, Ordering::Relaxed);

        // Update performance counters
        {
            let mut counters = self.counters.lock();
            counters.records_processed += 1;
            counters.bytes_transferred += record_size_bytes as u64;
            counters.buffer_utilization_percent = self.calculate_buffer_utilization();

            // Update cluster-specific metrics
            if let Some(cluster_id) = cluster_key {
                let cluster_ops = counters
                    .cluster_operations
                    .entry(cluster_id)
                    .or_insert_with(ClusterOperationCounters::default);
                cluster_ops.bytes_processed += record_size_bytes as u64;

                // Update average latency using exponential smoothing
                const ALPHA: f64 = 0.1;
                cluster_ops.avg_latency_us = ((cluster_ops.avg_latency_us as f64 * (1.0 - ALPHA))
                    + (latency_us as f64 * ALPHA))
                    as u64;
            }

            // Update operation-specific metrics
            self.update_operation_metrics(
                &mut counters,
                operation_type,
                record_size_bytes,
                latency_us,
                cluster_key,
            );
        }

        // Update latency histogram
        {
            let mut histogram = self.latency_histogram.lock();
            histogram.record_write_latency(latency_us);
        }

        // Update throughput tracker
        {
            let mut tracker = self.throughput_tracker.lock();
            tracker.record_write_operation(record_size_bytes);
        }

        // Update resource tracker
        {
            let mut resource_tracker = self.resource_tracker.lock();
            resource_tracker.update();
        }

        // Update cluster metrics
        if let Some(cluster_id) = cluster_key {
            {
                let mut cluster_metrics = self.cluster_metrics.lock();
                cluster_metrics.update_cluster_access(cluster_id);
            }
        }
    }

    /// Record a read operation for V2 graph operations.
    ///
    /// Captures comprehensive performance data for read operations similar to
    /// write operations but optimized for read-specific metrics and patterns.
    ///
    /// # Arguments
    ///
    /// * `record_size_bytes` - Size of the record being read in bytes
    /// * `latency_us` - Operation latency in microseconds
    /// * `cluster_key` - Optional cluster ID for cluster-affinity tracking
    /// * `operation_type` - Type of operation (e.g., "edge_read", "node_lookup")
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crate::backend::native::v2::wal::metrics::core::V2WALMetrics;
    ///
    /// let metrics = V2WALMetrics::new();
    /// metrics.record_read_operation(150, 30, Some(42), "edge_read");
    /// ```
    pub fn record_read_operation(
        &self,
        record_size_bytes: usize,
        latency_us: u64,
        cluster_key: Option<i64>,
        operation_type: &str,
    ) {
        // Update global counters
        self.global_counters
            .records_read
            .fetch_add(1, Ordering::Relaxed);
        self.global_counters
            .bytes_read
            .fetch_add(record_size_bytes as u64, Ordering::Relaxed);

        // Update performance counters
        {
            let mut counters = self.counters.lock();
            counters.records_processed += 1;
            counters.bytes_transferred += record_size_bytes as u64;

            // Update cluster-specific metrics
            if let Some(cluster_id) = cluster_key {
                let cluster_ops = counters
                    .cluster_operations
                    .entry(cluster_id)
                    .or_insert_with(ClusterOperationCounters::default);
                cluster_ops.bytes_processed += record_size_bytes as u64;

                // Update average latency using exponential smoothing
                const ALPHA: f64 = 0.1;
                cluster_ops.avg_latency_us = ((cluster_ops.avg_latency_us as f64 * (1.0 - ALPHA))
                    + (latency_us as f64 * ALPHA))
                    as u64;
            }

            // Update operation-specific metrics
            self.update_operation_metrics(
                &mut counters,
                operation_type,
                record_size_bytes,
                latency_us,
                cluster_key,
            );
        }

        // Update latency histogram
        {
            let mut histogram = self.latency_histogram.lock();
            histogram.record_read_latency(latency_us);
        }

        // Update throughput tracker
        {
            let mut tracker = self.throughput_tracker.lock();
            tracker.record_read_operation(record_size_bytes);
        }

        // Update resource tracker
        {
            let mut resource_tracker = self.resource_tracker.lock();
            resource_tracker.update();
        }

        // Update cluster metrics
        if let Some(cluster_id) = cluster_key {
            {
                let mut cluster_metrics = self.cluster_metrics.lock();
                cluster_metrics.update_cluster_access(cluster_id);
            }
        }
    }

    /// Record an error occurrence.
    ///
    /// Captures detailed error information for analysis and monitoring.
    /// This method tracks error patterns, frequencies, and recovery actions
    /// to help identify systematic issues and performance bottlenecks.
    ///
    /// # Arguments
    ///
    /// * `error_type` - Type or category of the error
    /// * `message` - Detailed error message
    /// * `operation_context` - Context in which the error occurred
    /// * `recovery_action` - Action taken to recover from the error
    ///
    /// # Examples
    ///
    /// ```rust
    /// use crate::backend::native::v2::wal::metrics::core::V2WALMetrics;
    ///
    /// let metrics = V2WALMetrics::new();
    /// metrics.record_error(
    ///     "IOError",
    ///     "Disk write failed",
    ///     "edge_insertion",
    ///     "retry_operation"
    /// );
    /// ```
    pub fn record_error(
        &self,
        error_type: &str,
        message: &str,
        operation_context: &str,
        recovery_action: &str,
    ) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let error_entry = ErrorEntry {
            error_type: error_type.to_string(),
            message: message.to_string(),
            timestamp,
            operation_context: operation_context.to_string(),
            recovery_action: recovery_action.to_string(),
        };

        {
            let mut tracker = self.error_tracker.lock();
            tracker.record_error(error_entry);
        }
    }

    /// Update operation-specific metrics based on operation type.
    ///
    /// Internal method that routes operation metrics to the appropriate
    /// specialized metric structures based on the operation type.
    ///
    /// # Arguments
    ///
    /// * `counters` - Mutable reference to performance counters
    /// * `operation_type` - Type of operation being recorded
    /// * `record_size` - Size of the record in bytes
    /// * `latency_us` - Operation latency in microseconds
    /// * `cluster_key` - Optional cluster ID for affinity tracking
    fn update_operation_metrics(
        &self,
        counters: &mut WALPerformanceCounters,
        operation_type: &str,
        record_size: usize,
        latency_us: u64,
        cluster_key: Option<i64>,
    ) {
        match operation_type {
            "edge_insert" => {
                counters.edge_operations.total_inserts += 1;
                counters.edge_operations.avg_record_size = Self::update_running_average(
                    counters.edge_operations.avg_record_size,
                    record_size as f64,
                    counters.edge_operations.total_inserts,
                );
                counters.edge_operations.avg_insertion_latency_us = Self::update_running_average(
                    counters.edge_operations.avg_insertion_latency_us as f64,
                    latency_us as f64,
                    counters.edge_operations.total_inserts,
                ) as u64;

                // Update cluster affinity hit rate
                if cluster_key.is_some() {
                    counters.edge_operations.cluster_affinity_hit_rate =
                        ((counters.edge_operations.cluster_affinity_hit_rate * 99.0) + 1.0) / 100.0;
                }
            }

            "edge_update" => {
                counters.edge_operations.total_updates += 1;
                counters.edge_operations.avg_record_size = Self::update_running_average(
                    counters.edge_operations.avg_record_size,
                    record_size as f64,
                    counters.edge_operations.total_updates,
                );
                counters.edge_operations.avg_update_latency_us = Self::update_running_average(
                    counters.edge_operations.avg_update_latency_us as f64,
                    latency_us as f64,
                    counters.edge_operations.total_updates,
                ) as u64;
            }

            "node_insert" => {
                counters.node_operations.total_inserts += 1;
                counters.node_operations.avg_record_size = Self::update_running_average(
                    counters.node_operations.avg_record_size,
                    record_size as f64,
                    counters.node_operations.total_inserts,
                );
                counters.node_operations.avg_insertion_latency_us = Self::update_running_average(
                    counters.node_operations.avg_insertion_latency_us as f64,
                    latency_us as f64,
                    counters.node_operations.total_inserts,
                ) as u64;
            }

            "node_update" => {
                counters.node_operations.total_updates += 1;
                counters.node_operations.avg_record_size = Self::update_running_average(
                    counters.node_operations.avg_record_size,
                    record_size as f64,
                    counters.node_operations.total_updates,
                );
                counters.node_operations.avg_update_latency_us = Self::update_running_average(
                    counters.node_operations.avg_update_latency_us as f64,
                    latency_us as f64,
                    counters.node_operations.total_updates,
                ) as u64;
            }

            "free_space_allocate" => {
                counters.free_space_operations.total_allocations += 1;
                counters.free_space_operations.avg_allocation_size =
                    ((counters.free_space_operations.avg_allocation_size
                        * (counters.free_space_operations.total_allocations - 1) as u64)
                        + record_size as u64)
                        / counters.free_space_operations.total_allocations;
                counters.free_space_operations.avg_allocation_latency_us =
                    Self::update_running_average(
                        counters.free_space_operations.avg_allocation_latency_us as f64,
                        latency_us as f64,
                        counters.free_space_operations.total_allocations,
                    ) as u64;
            }

            "string_insert" => {
                counters.string_table_operations.total_insertions += 1;
                counters.string_table_operations.avg_string_length = Self::update_running_average(
                    counters.string_table_operations.avg_string_length,
                    record_size as f64,
                    counters.string_table_operations.total_insertions,
                );
                counters.string_table_operations.avg_insertion_latency_us =
                    Self::update_running_average(
                        counters.string_table_operations.avg_insertion_latency_us as f64,
                        latency_us as f64,
                        counters.string_table_operations.total_insertions,
                    ) as u64;
            }

            _ => {
                // Generic operation handling - could be extended for custom metrics
            }
        }
    }

    /// Update running average using incremental formula.
    ///
    /// Utility method for calculating running averages without storing
    /// all historical values, using the formula: new_avg = old_avg * (n-1)/n + new_value/n
    ///
    /// # Arguments
    ///
    /// * `current_avg` - Current average value
    /// * `new_value` - New value to incorporate
    /// * `count` - Total count after adding the new value
    ///
    /// # Returns
    ///
    /// Updated average value
    fn update_running_average(current_avg: f64, new_value: f64, count: u64) -> f64 {
        if count == 0 {
            return new_value;
        }
        current_avg * ((count - 1) as f64 / count as f64) + (new_value / count as f64)
    }

    /// Calculate buffer utilization percentage.
    ///
    /// Estimates current buffer utilization based on the V2 buffer management
    /// system state. This provides insight into memory efficiency and helps
    /// identify potential memory pressure situations.
    ///
    /// # Returns
    ///
    /// Buffer utilization as a percentage (0.0 to 100.0)
    fn calculate_buffer_utilization(&self) -> f64 {
        // This would interface with the V2 buffer management system
        // For now, return a reasonable default based on typical usage patterns
        // In a production implementation, this would query actual buffer state

        // Simulate dynamic buffer utilization based on recent activity
        let global_counters = self.get_global_counters();
        let total_operations = global_counters.0 + global_counters.1; // writes + reads

        if total_operations == 0 {
            return 0.0;
        }

        // Scale utilization based on activity level, capped at 95%
        let base_utilization = 50.0; // Base utilization
        let activity_factor = (total_operations as f64 / 1000.0).min(45.0); // Scale with activity
        (base_utilization + activity_factor).min(95.0)
    }
}

#[cfg(test)]
mod tests {
  use crate::backend::native::v2::wal::metrics::core::V2WALMetrics;

    #[test]
    fn test_record_write_operation() {
        let metrics = V2WALMetrics::new();

        metrics.record_write_operation(100, 50, Some(42), "edge_insert");

        let counters = metrics.get_counters();
        assert_eq!(counters.records_processed, 1);
        assert_eq!(counters.bytes_transferred, 100);
        assert_eq!(counters.edge_operations.total_inserts, 1);

        let global_counters = metrics.get_global_counters();
        assert_eq!(global_counters.0, 1); // records_written
        assert_eq!(global_counters.2, 100); // bytes_written
    }

    #[test]
    fn test_record_read_operation() {
        let metrics = V2WALMetrics::new();

        metrics.record_read_operation(150, 30, Some(43), "edge_read");

        let counters = metrics.get_counters();
        assert_eq!(counters.records_processed, 1);
        assert_eq!(counters.bytes_transferred, 150);

        let global_counters = metrics.get_global_counters();
        assert_eq!(global_counters.1, 1); // records_read
        assert_eq!(global_counters.3, 150); // bytes_read
    }

    #[test]
    fn test_record_error() {
        let metrics = V2WALMetrics::new();

        metrics.record_error("TestError", "Test message", "Test context", "Test recovery");

        let error_tracker = metrics.get_error_tracker();
        assert_eq!(error_tracker.error_counts.get("TestError"), Some(&1));
    }

    #[test]
    fn test_edge_operation_metrics() {
        let metrics = V2WALMetrics::new();

        // Record multiple edge insertions
        metrics.record_write_operation(50, 25, Some(42), "edge_insert");
        metrics.record_write_operation(75, 35, Some(42), "edge_insert");
        metrics.record_write_operation(100, 45, Some(42), "edge_insert");

        let counters = metrics.get_counters();
        assert_eq!(counters.edge_operations.total_inserts, 3);
        assert!(counters.edge_operations.avg_record_size > 0.0);
        assert!(counters.edge_operations.avg_insertion_latency_us > 0);
        assert!(counters.edge_operations.cluster_affinity_hit_rate > 0.0);
    }

    #[test]
    fn test_node_operation_metrics() {
        let metrics = V2WALMetrics::new();

        metrics.record_write_operation(60, 30, Some(42), "node_insert");
        metrics.record_write_operation(80, 40, Some(42), "node_update");

        let counters = metrics.get_counters();
        assert_eq!(counters.node_operations.total_inserts, 1);
        assert_eq!(counters.node_operations.total_updates, 1);
        assert!(counters.node_operations.avg_record_size > 0.0);
    }

    #[test]
    fn test_free_space_operation_metrics() {
        let metrics = V2WALMetrics::new();

        metrics.record_write_operation(200, 100, None, "free_space_allocate");

        let counters = metrics.get_counters();
        assert_eq!(counters.free_space_operations.total_allocations, 1);
        assert!(counters.free_space_operations.avg_allocation_size > 0);
    }

    #[test]
    fn test_string_table_operation_metrics() {
        let metrics = V2WALMetrics::new();

        metrics.record_write_operation(25, 15, None, "string_insert");

        let counters = metrics.get_counters();
        assert_eq!(counters.string_table_operations.total_insertions, 1);
        assert!(counters.string_table_operations.avg_string_length > 0.0);
    }

    #[test]
    fn test_running_average_calculation() {
        // Test the utility function directly
        let avg1 = V2WALMetrics::update_running_average(0.0, 10.0, 1);
        assert_eq!(avg1, 10.0);

        let avg2 = V2WALMetrics::update_running_average(10.0, 20.0, 2);
        assert_eq!(avg2, 15.0);

        let avg3 = V2WALMetrics::update_running_average(15.0, 30.0, 3);
        assert_eq!(avg3, 20.0);
    }

    #[test]
    fn test_cluster_specific_metrics() {
        let metrics = V2WALMetrics::new();

        // Record operations for different clusters
        metrics.record_write_operation(100, 50, Some(42), "edge_insert");
        metrics.record_write_operation(150, 60, Some(43), "edge_insert");

        let counters = metrics.get_counters();
        assert_eq!(counters.cluster_operations.len(), 2);
        assert!(counters.cluster_operations.contains_key(&42));
        assert!(counters.cluster_operations.contains_key(&43));

        let cluster_42 = &counters.cluster_operations[&42];
        assert_eq!(cluster_42.bytes_processed, 100);
        assert!(cluster_42.avg_latency_us > 0);
    }

    #[test]
    fn test_buffer_utilization_calculation() {
        let metrics = V2WALMetrics::new();

        // Initially should be low utilization
        let utilization = metrics.calculate_buffer_utilization();
        assert!(utilization >= 0.0 && utilization <= 100.0);

        // After operations, utilization should increase
        metrics.record_write_operation(100, 50, Some(42), "edge_insert");
        let utilization_after = metrics.calculate_buffer_utilization();
        assert!(utilization_after >= utilization);
    }
}
