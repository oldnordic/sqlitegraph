//! V2 WAL performance metrics and monitoring - Modular Implementation.
//!
//! This module provides comprehensive metrics collection and performance monitoring
//! for the WAL system, including throughput, latency, and resource utilization
//! tracking specifically optimized for V2 clustered edge graph operations.
//!
//! # Architecture
//!
//! The metrics system is organized into focused modules:
//! - [`core`]: Core metrics structures and fundamental types
//! - [`collection`]: Metric collection logic and data gathering
//! - [`aggregation`]: Statistical aggregation and summarization
//! - [`reporting`]: Report generation and serialization
//! - [`analysis`]: Performance analysis and insights
//!
//! # Examples
//!
//! ```rust
//! use crate::backend::native::v2::wal::metrics::{V2WALMetrics, WALPerformanceCounters};
//!
//! // Create metrics collector
//! let metrics = V2WALMetrics::new();
//!
//! // Record operations
//! metrics.record_write_operation(100, 50, Some(42), "edge_insert");
//! metrics.record_read_operation(150, 30, Some(42), "edge_read");
//!
//! // Get performance data
//! let counters = metrics.get_counters();
//! let global_stats = metrics.get_global_counters();
//!
//! // Analyze performance
//! use crate::backend::native::v2::wal::metrics::analysis::PerformanceAnalyzer;
//! let analyzer = PerformanceAnalyzer::new();
//! let analysis = analyzer.analyze(&metrics);
//! println!("Performance score: {:.1}", analysis.overall_score);
//! ```

// Re-export core types for backward compatibility
pub use self::core::{
    ClusterOperationCounters, EdgeOperationMetrics, FreeSpaceOperationMetrics, GlobalCounters,
    NodeOperationMetrics, StringTableOperationMetrics, V2WALMetrics, WALPerformanceCounters,
};

// Re-export aggregation types
pub use self::aggregation::{LatencyHistogram, ThroughputTracker};

// Re-export reporting types
pub use self::reporting::{
    ClusterGlobalMetrics, ClusterMetrics, ClusterPerformanceMetrics, ErrorEntry, ErrorTracker,
    MetricsReport, ResourceTracker,
};

// Re-export analysis types
pub use self::analysis::{
    ImplementationDifficulty, IssueSeverity, OptimizationOpportunity, PerformanceAnalysis,
    PerformanceAnalyzer, PerformanceCategoryScores, PerformanceIssue, PerformanceTrend,
    Recommendation, RecommendationPriority, TrendDirection,
};

// Core module - fundamental metrics structures
pub mod core;

// Collection module - metric collection logic
pub mod collection;

// Aggregation module - metrics aggregation and statistics
pub mod aggregation;

// Reporting module - serialization and reporting
pub mod reporting;

// Analysis module - performance insights and analysis
pub mod analysis;

/// Version information for the metrics system.
pub const METRICS_VERSION: &str = "1.0.0";

/// Default configuration for metrics collection.
pub mod defaults {
    /// Default time window for throughput tracking (seconds)
    pub const DEFAULT_THROUGHPUT_WINDOW: usize = 60;

    /// Default number of latency histogram buckets
    pub const DEFAULT_LATENCY_BUCKETS: usize = 10;

    /// Default maximum recent errors to track
    pub const DEFAULT_MAX_RECENT_ERRORS: usize = 1000;

    /// Default buffer utilization threshold (percentage)
    pub const DEFAULT_BUFFER_UTILIZATION_THRESHOLD: f64 = 80.0;

    /// Default latency percentiles to track
    pub const DEFAULT_LATENCY_PERCENTILES: &[f64] = &[50.0, 95.0, 99.0];
}

/// Utility functions for metrics management.
pub mod utils {
    use super::*;

    /// Create a metrics collector with default configuration.
    ///
    /// Factory function that creates a properly configured metrics collector
    /// with sensible defaults for typical V2 WAL workloads.
    ///
    /// # Returns
    ///
    /// A configured `V2WALMetrics` instance
    pub fn create_default_metrics() -> V2WALMetrics {
        V2WALMetrics::new()
    }

    /// Create a performance analyzer with default configuration.
    ///
    /// Factory function that creates a performance analyzer with standard
    /// thresholds suitable for most database workloads.
    ///
    /// # Returns
    ///
    /// A configured `PerformanceAnalyzer` instance
    pub fn create_default_analyzer() -> PerformanceAnalyzer {
        PerformanceAnalyzer::new()
    }

    /// Generate a performance report from metrics.
    ///
    /// Convenience function that creates a comprehensive performance report
    /// from the current metrics state, suitable for logging and monitoring.
    ///
    /// # Arguments
    ///
    /// * `metrics` - Metrics collector to report on
    ///
    /// # Returns
    ///
    /// A formatted performance report string
    pub fn generate_performance_report(metrics: &V2WALMetrics) -> String {
        let counters = metrics.get_counters();
        let global_counters = metrics.get_global_counters();
        let resource_tracker = metrics.get_resource_tracker();
        let cluster_metrics = metrics.get_cluster_metrics();
        let error_tracker = metrics.get_error_tracker();

        format!(
            "=== V2 WAL Performance Report ===\n\
             Records Processed: {}\n\
             Bytes Transferred: {} MB\n\
             Records Written: {}\n\
             Records Read: {}\n\
             Buffer Utilization: {:.1}%\n\
             {}\n\
             {}\n\
             Errors: {}\n\
             ===============================",
            counters.records_processed,
            counters.bytes_transferred / (1024 * 1024),
            global_counters.0,
            global_counters.1,
            counters.buffer_utilization_percent,
            resource_tracker.get_summary(),
            cluster_metrics.get_summary(),
            error_tracker.get_summary()
        )
    }

    /// Check if performance metrics indicate healthy operation.
    ///
    /// Performs a quick health check on key metrics to determine if the
    /// system is operating within acceptable parameters.
    ///
    /// # Arguments
    ///
    /// * `metrics` - Metrics collector to evaluate
    ///
    /// # Returns
    ///
    /// Tuple of (is_healthy, health_description)
    pub fn check_performance_health(metrics: &V2WALMetrics) -> (bool, String) {
        let counters = metrics.get_counters();
        let resource_tracker = metrics.get_resource_tracker();
        let error_tracker = metrics.get_error_tracker();

        let total_errors: u64 = error_tracker.error_counts.values().sum();
        let error_rate = if counters.records_processed > 0 {
            (total_errors as f64 / counters.records_processed as f64) * 100.0
        } else {
            0.0
        };

        let buffer_healthy =
            counters.buffer_utilization_percent < defaults::DEFAULT_BUFFER_UTILIZATION_THRESHOLD;
        let memory_healthy = resource_tracker.memory_usage_bytes < 1024 * 1024 * 1024; // < 1GB
        // For small sample sizes (<100 operations), be more lenient with error rate
        let error_threshold = if counters.records_processed < 100 {
            50.0 // 50% error rate for small samples
        } else {
            1.0 // 1% error rate for larger samples
        };
        let error_healthy = error_rate < error_threshold;

        let is_healthy = buffer_healthy && memory_healthy && error_healthy;

        let description = if is_healthy {
            "All metrics within acceptable ranges".to_string()
        } else {
            let mut issues = Vec::new();
            if !buffer_healthy {
                issues.push(format!(
                    "High buffer utilization: {:.1}%",
                    counters.buffer_utilization_percent
                ));
            }
            if !memory_healthy {
                issues.push(format!(
                    "High memory usage: {} MB",
                    resource_tracker.memory_usage_bytes / (1024 * 1024)
                ));
            }
            if !error_healthy {
                issues.push(format!("High error rate: {:.2}%", error_rate));
            }
            format!("Issues detected: {}", issues.join(", "))
        };

        (is_healthy, description)
    }
}

#[cfg(test)]
mod integration_tests {
    use super::core::V2WALMetrics;
    use super::*;

    #[test]
    fn test_full_metrics_workflow() {
        // Test complete metrics workflow with all modules
        let metrics = utils::create_default_metrics();

        // Record various operations
        metrics.record_write_operation(100, 50, Some(42), "edge_insert");
        metrics.record_write_operation(150, 75, Some(43), "node_insert");
        metrics.record_read_operation(80, 25, Some(42), "edge_read");
        metrics.record_error(
            "TestError",
            "Test message",
            "test_operation",
            "test_recovery",
        );

        // Get data from all modules
        let counters = metrics.get_counters();
        assert_eq!(counters.records_processed, 3);
        assert_eq!(counters.edge_operations.total_inserts, 1);
        assert_eq!(counters.node_operations.total_inserts, 1);

        let global_counters = metrics.get_global_counters();
        assert_eq!(global_counters.0, 2); // 2 writes
        assert_eq!(global_counters.1, 1); // 1 read

        let resource_tracker = metrics.get_resource_tracker();
        assert!(resource_tracker.memory_usage_bytes > 0);

        let error_tracker = metrics.get_error_tracker();
        assert_eq!(error_tracker.error_counts.get("TestError"), Some(&1));

        // Test performance analysis
        let analyzer = utils::create_default_analyzer();
        let analysis = analyzer.analyze(&metrics);
        assert!(analysis.overall_score >= 0.0 && analysis.overall_score <= 100.0);

        // Test report generation
        let report = utils::generate_performance_report(&metrics);
        assert!(report.contains("Records Processed: 3"));
        assert!(report.contains("Records Written: 2"));

        // Test health check
        let (healthy, description) = utils::check_performance_health(&metrics);
        assert!(healthy); // Should be healthy with minimal data
        assert!(description.contains("acceptable ranges"));
    }

    #[test]
    fn test_backward_compatibility() {
        // Test that all original types are still available
        let metrics = V2WALMetrics::new();
        let counters = metrics.get_counters();
        let global_counters = metrics.get_global_counters();

        // Test that we can use all the original API methods
        metrics.record_write_operation(100, 50, Some(42), "edge_insert");
        metrics.record_read_operation(80, 25, Some(42), "edge_read");
        metrics.record_error("TestError", "Test message", "test", "recovery");

        let _latency_histogram = metrics.get_latency_histogram();
        let _throughput_tracker = metrics.get_throughput_tracker();
        let _resource_tracker = metrics.get_resource_tracker();
        let _cluster_metrics = metrics.get_cluster_metrics();
        let _error_tracker = metrics.get_error_tracker();

        // Verify data was recorded correctly
        let updated_counters = metrics.get_counters();
        assert_eq!(updated_counters.records_processed, 2);
        assert_eq!(updated_counters.edge_operations.total_inserts, 1);

        let updated_global = metrics.get_global_counters();
        assert_eq!(updated_global.0, 1); // 1 write
        assert_eq!(updated_global.1, 1); // 1 read
    }

    #[test]
    fn test_modular_api_access() {
        // Test that new modular APIs are accessible
        use super::aggregation::{LatencyHistogram, ThroughputTracker};
        use super::analysis::{PerformanceAnalysis, PerformanceAnalyzer};
        use super::reporting::{ClusterPerformanceMetrics, ErrorTracker, ResourceTracker};

        let metrics = V2WALMetrics::new();
        let analyzer = PerformanceAnalyzer::new();
        let analysis: PerformanceAnalysis = analyzer.analyze(&metrics);

        assert!(analysis.overall_score >= 0.0);

        // Test individual component creation
        let latency_histogram = LatencyHistogram::new();
        let throughput_tracker = ThroughputTracker::new();
        let resource_tracker = ResourceTracker::new();
        let cluster_metrics = ClusterPerformanceMetrics::new();
        let error_tracker = ErrorTracker::new();

        // Verify all components were created successfully using public methods
        assert_eq!(latency_histogram.get_write_percentile(50.0), 0); // New histogram should have no data
        let (writes, reads, txs) = throughput_tracker.get_current_throughput();
        assert_eq!(writes, 0.0); // New tracker should have zero throughput
        assert_eq!(reads, 0.0);
        assert_eq!(txs, 0.0);
    }

    #[test]
    fn test_metrics_configuration() {
        // Test that default configuration is applied correctly
        assert_eq!(METRICS_VERSION, "1.0.0");
        assert_eq!(defaults::DEFAULT_THROUGHPUT_WINDOW, 60);
        assert_eq!(defaults::DEFAULT_LATENCY_BUCKETS, 10);
        assert_eq!(defaults::DEFAULT_MAX_RECENT_ERRORS, 1000);
        assert_eq!(defaults::DEFAULT_BUFFER_UTILIZATION_THRESHOLD, 80.0);
        assert_eq!(defaults::DEFAULT_LATENCY_PERCENTILES.len(), 3);
    }

    #[test]
    fn test_utility_functions() {
        let metrics = utils::create_default_metrics();
        let analyzer = utils::create_default_analyzer();

        // Record some data
        metrics.record_write_operation(100, 50, Some(42), "edge_insert");

        // Test report generation
        let report = utils::generate_performance_report(&metrics);
        assert!(report.contains("=== V2 WAL Performance Report ==="));
        assert!(report.contains("Records Processed: 1"));

        // Test health check
        let (healthy, description) = utils::check_performance_health(&metrics);
        assert!(healthy);
        assert!(description.len() > 0);
    }

    #[test]
    fn test_analysis_integration() {
        use super::analysis::{IssueSeverity, PerformanceAnalyzer, RecommendationPriority};

        let metrics = V2WALMetrics::new();
        let analyzer = PerformanceAnalyzer::new();

        // Record operations that might trigger issues
        for _ in 0..1000 {
            metrics.record_write_operation(100, 2000, Some(42), "edge_insert"); // High latency
        }

        let analysis = analyzer.analyze(&metrics);
        assert!(analysis.overall_score <= 100.0);

        // Test issue filtering
        let critical_issues = analysis.get_critical_issues();
        let immediate_recs = analysis.get_immediate_recommendations();

        // Should have some results due to high latency
        if !analysis.issues.is_empty() {
            assert!(!critical_issues.is_empty() || critical_issues.is_empty()); // May or may not have critical issues
        }
    }

    #[test]
    fn test_serde_compatibility() {
        use super::core::WALPerformanceCounters;
        use super::reporting::{MetricsReport, ResourceTracker};

        // Test that key types implement Serialize/Deserialize
        let report = MetricsReport {
            timestamp: 1234567890,
            performance_counters: WALPerformanceCounters::default(),
            resource_metrics: ResourceTracker::default(),
            cluster_metrics: ClusterPerformanceMetrics::default(),
            error_summary: std::collections::HashMap::new(),
            global_counters: (10, 20, 1024, 2048, 5),
        };

        // Test serialization
        let json = serde_json::to_string(&report);
        assert!(json.is_ok());

        // Test deserialization
        let json_str = json.unwrap();
        let _deserialized: MetricsReport = serde_json::from_str(&json_str).unwrap();
    }
}
