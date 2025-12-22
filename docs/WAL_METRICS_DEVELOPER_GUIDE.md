# WAL Metrics Developer Guide

**Date**: 2025-12-20
**Focus**: Developer migration and working with the new modular metrics structure
**Status**: ✅ **DEVELOPER GUIDE COMPLETE**

---

## Executive Summary

This guide helps developers work with the newly modularized WAL metrics system. The modularization provides enhanced maintainability and better development experience while maintaining complete backward compatibility - all existing code continues to work without any changes.

---

## 🚀 Quick Start

### For Existing Users (Zero Changes Required)

All existing code continues to work exactly as before:

```rust
// Your existing code continues to work without any changes:
use crate::backend::native::v2::wal::metrics::V2WALMetrics;

fn setup_metrics() -> V2WALMetrics {
    let metrics = V2WALMetrics::new();

    // All existing method calls work identically:
    metrics.record_write_operation(1024, 50, Some(42), "edge_insert");
    metrics.record_read_operation(512, 30, Some(42), "node_select");

    metrics
}
```

**No migration required** - the modularization is transparent to existing users.

### For New Development (Optional Enhancements)

The new modular structure enables enhanced development patterns:

```rust
// Option 1: Use focused imports for better IDE support:
use crate::backend::native::v2::wal::metrics::{
    V2WALMetrics,              // Main coordinator
    WALPerformanceCounters,    // Performance counters
    LatencyHistogram,          // Latency analysis
    ThroughputTracker,         // Throughput monitoring
};

// Option 2: Access specific submodules for very focused work:
use crate::backend::native::v2::wal::metrics::counters::GlobalCounters;
use crate::backend::native::v2::wal::metrics::latency::LatencyHistogram;

// Option 3: Continue using wildcard import (still supported):
use crate::backend::native::v2::wal::metrics::*;
```

---

## 📚 Understanding the New Structure

### Module Architecture

```
sqlitegraph/src/backend/native/v2/wal/metrics/
├── mod.rs              (50 LOC)  - Public API orchestrator
├── core.rs             (150 LOC) - Main metrics coordination
├── counters.rs         (200 LOC) - Performance counters & atomic ops
├── latency.rs          (300 LOC) - Latency histograms & statistics
└── throughput.rs       (300 LOC) - Throughput tracking & resource metrics
```

### Module Responsibilities

| Module | Focus | Main Types | Key Capabilities |
|--------|-------|------------|------------------|
| **core.rs** | Coordination | `V2WALMetrics` | Main API, lifecycle, coordination |
| **counters.rs** | Counting | `WALPerformanceCounters`, `GlobalCounters` | Atomic operations, performance tracking |
| **latency.rs** | Analysis | `LatencyHistogram` | Statistical analysis, percentiles |
| **throughput.rs** | Monitoring | `ThroughputTracker`, `ResourceTracker` | Real-time monitoring, resource tracking |

---

## 🛠️ Working with Specific Components

### 1. Main Metrics Coordination (V2WALMetrics)

The main coordinator provides the primary API for metrics collection:

```rust
use crate::backend::native::v2::wal::metrics::V2WALMetrics;

struct GraphDatabase {
    metrics: V2WALMetrics,
}

impl GraphDatabase {
    fn new() -> Self {
        Self {
            metrics: V2WALMetrics::new(),
        }
    }

    fn insert_edge(&mut self, edge_data: &[u8], cluster_id: i64) -> Result<(), Error> {
        let start = std::time::Instant::now();

        // Your edge insertion logic here...

        let latency = start.elapsed().as_micros() as u64;

        // Record the operation with metrics
        self.metrics.record_write_operation(
            edge_data.len(),
            latency,
            Some(cluster_id),
            "edge_insert"
        );

        Ok(())
    }

    fn get_performance_summary(&self) -> PerformanceSummary {
        let counters = self.metrics.get_counters();
        let histogram = self.metrics.get_latency_histogram();
        let throughput = self.metrics.get_throughput_tracker();

        PerformanceSummary {
            total_operations: counters.records_processed,
            total_bytes: counters.bytes_transferred,
            avg_write_latency_us: counters.avg_write_latency_us,
            p95_write_latency_us: histogram.get_write_percentile(95.0),
            current_throughput: throughput.get_current_throughput(),
        }
    }
}

struct PerformanceSummary {
    total_operations: u64,
    total_bytes: u64,
    avg_write_latency_us: u64,
    p95_write_latency_us: u64,
    current_throughput: (f64, f64, f64),
}
```

### 2. Working with Performance Counters

Access detailed performance information for analysis and monitoring:

```rust
use crate::backend::native::v2::wal::metrics::{V2WALMetrics, WALPerformanceCounters};

impl GraphDatabase {
    fn analyze_performance(&self) -> PerformanceAnalysis {
        let counters = self.metrics.get_counters();

        PerformanceAnalysis {
            operation_breakdown: OperationBreakdown {
                edge_inserts: counters.edge_operations.total_inserts,
                edge_updates: counters.edge_operations.total_updates,
                node_inserts: counters.node_operations.total_inserts,
                node_updates: counters.node_operations.total_updates,
                free_space_allocations: counters.free_space_operations.total_allocations,
            },
            cluster_metrics: counters.cluster_operations.iter()
                .map(|(id, ops)| (*id, ClusterMetricSummary {
                    operations: ops.creates + ops.reads + ops.updates,
                    avg_latency_us: ops.avg_latency_us,
                    bytes_processed: ops.bytes_processed,
                }))
                .collect(),
            resource_utilization: ResourceUtilization {
                buffer_utilization: counters.buffer_utilization_percent,
                avg_record_sizes: AverageRecordSizes {
                    edges: counters.edge_operations.avg_record_size,
                    nodes: counters.node_operations.avg_record_size,
                },
            },
        }
    }
}

struct PerformanceAnalysis {
    operation_breakdown: OperationBreakdown,
    cluster_metrics: Vec<(i64, ClusterMetricSummary)>,
    resource_utilization: ResourceUtilization,
}

struct OperationBreakdown {
    edge_inserts: u64,
    edge_updates: u64,
    node_inserts: u64,
    node_updates: u64,
    free_space_allocations: u64,
}

struct ClusterMetricSummary {
    operations: u64,
    avg_latency_us: u64,
    bytes_processed: u64,
}

struct ResourceUtilization {
    buffer_utilization: f64,
    avg_record_sizes: AverageRecordSizes,
}

struct AverageRecordSizes {
    edges: f64,
    nodes: f64,
}
```

### 3. Working with Latency Analysis

Use the latency histogram for detailed performance analysis:

```rust
use crate::backend::native::v2::wal::metrics::{V2WALMetrics, LatencyHistogram};

impl GraphDatabase {
    fn analyze_latency_patterns(&self) -> LatencyAnalysis {
        let histogram = self.metrics.get_latency_histogram();

        LatencyAnalysis {
            write_latency: LatencyMetrics {
                p50: histogram.get_write_percentile(50.0),
                p90: histogram.get_write_percentile(90.0),
                p95: histogram.get_write_percentile(95.0),
                p99: histogram.get_write_percentile(99.0),
            },
            read_latency: LatencyMetrics {
                p50: histogram.get_read_percentile(50.0),
                p90: histogram.get_read_percentile(90.0),
                p95: histogram.get_read_percentile(95.0),
                p99: histogram.get_read_percentile(99.0),
            },
            performance_classification: self.classify_performance(&histogram),
        }
    }

    fn classify_performance(&self, histogram: &LatencyHistogram) -> PerformanceClass {
        let write_p95 = histogram.get_write_percentile(95.0);
        let read_p95 = histogram.get_read_percentile(95.0);

        match (write_p95, read_p95) {
            (w, r) if w <= 100 && r <= 50 => PerformanceClass::Excellent,
            (w, r) if w <= 500 && r <= 200 => PerformanceClass::Good,
            (w, r) if w <= 2000 && r <= 1000 => PerformanceClass::Acceptable,
            _ => PerformanceClass::NeedsOptimization,
        }
    }
}

struct LatencyAnalysis {
    write_latency: LatencyMetrics,
    read_latency: LatencyMetrics,
    performance_classification: PerformanceClass,
}

#[derive(Debug)]
struct LatencyMetrics {
    p50: u64,
    p90: u64,
    p95: u64,
    p99: u64,
}

#[derive(Debug)]
enum PerformanceClass {
    Excellent,
    Good,
    Acceptable,
    NeedsOptimization,
}
```

### 4. Working with Throughput Monitoring

Monitor real-time performance and resource utilization:

```rust
use crate::backend::native::v2::wal::metrics::{V2WALMetrics, ThroughputTracker, ResourceTracker};

impl GraphDatabase {
    fn monitor_realtime_performance(&self) -> RealtimeMetrics {
        let throughput = self.metrics.get_throughput_tracker();
        let resources = self.metrics.get_resource_tracker();

        let (records_per_sec, bytes_per_sec, tx_per_sec) = throughput.get_current_throughput();

        RealtimeMetrics {
            throughput: ThroughputMetrics {
                records_per_second: records_per_sec,
                bytes_per_second: bytes_per_sec,
                transactions_per_second: tx_per_sec,
                mb_per_second: bytes_per_sec / 1_048_576.0,
            },
            resource_usage: ResourceUsage {
                memory_mb: resources.memory_usage_bytes / 1_048_576,
                cpu_percent: resources.cpu_usage_percent,
                disk_iops: resources.disk_iops,
                disk_throughput_mbps: resources.disk_throughput_mbps,
                buffer_pool_hit_rate: resources.buffer_pool_hit_rate,
            },
            health_status: self.assess_system_health(&throughput, &resources),
        }
    }

    fn assess_system_health(&self, throughput: &ThroughputTracker, resources: &ResourceTracker) -> SystemHealth {
        let (records_per_sec, _, _) = throughput.get_current_throughput();
        let memory_usage_percent = (resources.memory_usage_bytes as f64 / (8.0 * 1024.0 * 1024.0 * 1024.0)) * 100.0;

        let issues = vec![];

        if records_per_sec < 100.0 {
            // Note: This would require adding methods to check individual issues
            // For now, this is conceptual
        }

        if memory_usage_percent > 80.0 {
            // Add memory pressure issue
        }

        if resources.cpu_usage_percent > 90.0 {
            // Add CPU pressure issue
        }

        SystemHealth {
            overall_status: if issues.is_empty() { HealthStatus::Healthy } else { HealthStatus::Warning },
            issues,
            recommendations: self.generate_recommendations(&issues),
        }
    }

    fn generate_recommendations(&self, issues: &[HealthIssue]) -> Vec<String> {
        let mut recommendations = vec![];

        for issue in issues {
            match issue {
                HealthIssue::LowThroughput => recommendations.push(
                    "Consider optimizing query patterns or adding more memory".to_string()
                ),
                HealthIssue::HighMemoryUsage => recommendations.push(
                    "Monitor memory leaks or increase available memory".to_string()
                ),
                HealthIssue::HighCpuUsage => recommendations.push(
                    "Profile CPU usage and optimize hot paths".to_string()
                ),
            }
        }

        recommendations
    }
}

struct RealtimeMetrics {
    throughput: ThroughputMetrics,
    resource_usage: ResourceUsage,
    health_status: SystemHealth,
}

struct ThroughputMetrics {
    records_per_second: f64,
    bytes_per_second: f64,
    transactions_per_second: f64,
    mb_per_second: f64,
}

struct ResourceUsage {
    memory_mb: u64,
    cpu_percent: f64,
    disk_iops: u64,
    disk_throughput_mbps: f64,
    buffer_pool_hit_rate: f64,
}

struct SystemHealth {
    overall_status: HealthStatus,
    issues: Vec<HealthIssue>,
    recommendations: Vec<String>,
}

#[derive(Debug)]
enum HealthStatus {
    Healthy,
    Warning,
    Critical,
}

#[derive(Debug)]
enum HealthIssue {
    LowThroughput,
    HighMemoryUsage,
    HighCpuUsage,
}
```

---

## 🧪 Testing with the Modular Structure

### Unit Testing Individual Components

The modular structure enables focused unit testing:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::v2::wal::metrics::{
        V2WALMetrics, WALPerformanceCounters, LatencyHistogram,
        ThroughputTracker, counters::GlobalCounters
    };

    // Test the main coordinator
    #[test]
    fn test_metrics_coordination() {
        let metrics = V2WALMetrics::new();

        // Test write operations
        metrics.record_write_operation(100, 50, Some(42), "edge_insert");
        metrics.record_write_operation(200, 75, Some(43), "node_insert");

        // Test read operations
        metrics.record_read_operation(50, 25, Some(42), "edge_select");

        // Test error recording
        metrics.record_error("TestError", "Test message", "test context", "test recovery");

        // Verify coordination
        let counters = metrics.get_counters();
        assert_eq!(counters.records_processed, 3);
        assert_eq!(counters.bytes_transferred, 350);
    }

    // Test individual components in isolation
    #[test]
    fn test_performance_counters() {
        let mut counters = WALPerformanceCounters::default();

        counters.records_processed = 100;
        counters.bytes_transferred = 10240;
        counters.edge_operations.total_inserts = 50;

        assert_eq!(counters.records_processed, 100);
        assert_eq!(counters.edge_operations.total_inserts, 50);
    }

    #[test]
    fn test_latency_histogram() {
        let mut histogram = LatencyHistogram::new();

        // Record some latency values
        histogram.record_write_latency(10);
        histogram.record_write_latency(20);
        histogram.record_write_latency(30);
        histogram.record_write_latency(100);
        histogram.record_write_latency(1000);

        // Test percentile calculations
        let p50 = histogram.get_write_percentile(50.0);
        let p95 = histogram.get_write_percentile(95.0);

        assert!(p50 >= 20 && p50 <= 30); // Should be around median
        assert!(p95 >= 100 && p95 <= 1000); // Should be in upper range
    }

    #[test]
    fn test_throughput_tracker() {
        let mut tracker = ThroughputTracker::new();

        // Record some operations
        tracker.record_write_operation(100);
        tracker.record_write_operation(200);
        tracker.record_transaction();
        tracker.record_transaction();

        let (records, bytes, tx) = tracker.get_current_throughput();

        // Should have recorded the operations
        assert!(records > 0.0);
        assert!(bytes > 0.0);
        assert!(tx > 0.0);
    }

    #[test]
    fn test_global_counters() {
        let counters = GlobalCounters::new();

        // Test atomic operations
        counters.increment_records_written(10);
        counters.increment_bytes_written(1024);
        counters.increment_active_operations();

        let (writes, reads, bytes_written, bytes_read, active) = counters.get_snapshot();

        assert_eq!(writes, 10);
        assert_eq!(bytes_written, 1024);
        assert_eq!(active, 1);
        assert_eq!(reads, 0); // Should be 0 since we didn't increment reads
    }

    // Integration tests
    #[test]
    fn test_full_metrics_workflow() {
        let metrics = V2WALMetrics::new();

        // Simulate a realistic workload
        for i in 0..100 {
            let size = 100 + (i % 50); // Variable record sizes
            let latency = 10 + (i % 100); // Variable latencies
            let cluster_id = Some((i % 10) as i64); // Distribute across clusters

            metrics.record_write_operation(size, latency, cluster_id, "edge_insert");

            if i % 5 == 0 {
                metrics.record_read_operation(size / 2, latency / 2, cluster_id, "edge_select");
            }

            if i % 20 == 0 {
                metrics.record_error("SimulatedError", "Test error", "test", "continue");
            }
        }

        // Verify all metrics were recorded
        let counters = metrics.get_counters();
        let histogram = metrics.get_latency_histogram();
        let throughput = metrics.get_throughput_tracker();
        let errors = metrics.get_error_tracker();

        assert_eq!(counters.records_processed, 100);
        assert!(counters.bytes_transferred > 0);
        assert!(histogram.get_write_percentile(50.0) > 0);

        let (records_per_sec, _, _) = throughput.get_current_throughput();
        assert!(records_per_sec > 0.0);
    }
}
```

### Benchmark Testing

```rust
#[cfg(test)]
mod benchmarks {
    use super::*;
    use std::time::Instant;
    use crate::backend::native::v2::wal::metrics::V2WALMetrics;

    #[test]
    fn benchmark_metrics_recording() {
        let metrics = V2WALMetrics::new();
        let iterations = 1_000_000;

        let start = Instant::now();

        for i in 0..iterations {
            metrics.record_write_operation(
                100 + (i % 200),
                10 + (i % 100),
                Some((i % 100) as i64),
                "edge_insert"
            );
        }

        let duration = start.elapsed();
        let ops_per_sec = iterations as f64 / duration.as_secs_f64();

        println!("Metrics recording: {:.0} ops/sec", ops_per_sec);
        println!("Average time per operation: {:.2} ns", duration.as_nanos() as f64 / iterations as f64);

        // Should be able to record at least 1M ops/sec
        assert!(ops_per_sec > 1_000_000.0);
    }

    #[test]
    fn benchmark_metrics_reading() {
        let metrics = V2WALMetrics::new();

        // Pre-populate with some data
        for i in 0..10_000 {
            metrics.record_write_operation(100, 50, Some(42), "edge_insert");
        }

        let iterations = 100_000;
        let start = Instant::now();

        for _ in 0..iterations {
            let _counters = metrics.get_counters();
            let _histogram = metrics.get_latency_histogram();
            let _throughput = metrics.get_throughput_tracker();
        }

        let duration = start.elapsed();
        let reads_per_sec = (iterations * 3) as f64 / duration.as_secs_f64(); // 3 reads per iteration

        println!("Metrics reading: {:.0} reads/sec", reads_per_sec);

        // Should be able to read at least 10M metrics/sec
        assert!(reads_per_sec > 10_000_000.0);
    }
}
```

---

## 📈 Performance Considerations

### High-Frequency Operations

For operations that need maximum performance:

```rust
use crate::backend::native::v2::wal::metrics::counters::GlobalCounters;
use std::sync::Arc;

struct HighPerformanceGraph {
    // Use global counters directly for maximum performance
    global_counters: Arc<GlobalCounters>,
    metrics: V2WALMetrics, // For detailed metrics when needed
}

impl HighPerformanceGraph {
    fn new() -> Self {
        let metrics = V2WALMetrics::new();

        // Extract global counters for direct access
        let (writes, reads, bytes_written, bytes_read, active) = metrics.get_global_counters();

        // Note: In practice, you'd want to share the actual GlobalCounters instance
        // This is conceptual for demonstrating high-frequency access patterns

        Self {
            global_counters: Arc::new(GlobalCounters::new()),
            metrics,
        }
    }

    fn high_frequency_write(&self, data: &[u8]) {
        // Fast path: use atomic counters directly
        self.global_counters.increment_records_written(1);
        self.global_counters.increment_bytes_written(data.len() as u64);

        // Optional: record detailed metrics less frequently
        // This could be done in a background thread or at intervals
    }

    fn detailed_metrics_update(&self, cluster_id: i64, latency_us: u64) {
        // Slower path: record detailed metrics
        self.metrics.record_write_operation(data.len(), latency_us, Some(cluster_id), "edge_insert");
    }
}
```

### Memory Usage Optimization

```rust
use crate::backend::native::v2::wal::metrics::V2WALMetrics;

impl GraphDatabase {
    fn optimize_memory_usage(&self) {
        // Reset metrics periodically to prevent memory growth
        let metrics = &self.metrics;

        // This resets all metrics while preserving the instances
        metrics.reset();

        // Or reset specific components if needed
        // (This would require additional methods to be added)
        // metrics.reset_throughput_only();
        // metrics.reset_error_history_only();
    }

    fn monitor_memory_usage(&self) -> MemoryUsageReport {
        let resource_tracker = self.metrics.get_resource_tracker();
        let error_tracker = self.metrics.get_error_tracker();

        MemoryUsageReport {
            memory_usage_mb: resource_tracker.memory_usage_bytes / 1_048_576,
            recent_error_count: error_tracker.get_recent_errors().len() as u64,
            recommendation: if resource_tracker.memory_usage_bytes > 1_073_741_824 { // 1GB
                "Consider resetting metrics or increasing available memory"
            } else {
                "Memory usage is within acceptable limits"
            }.to_string(),
        }
    }
}

struct MemoryUsageReport {
    memory_usage_mb: u64,
    recent_error_count: u64,
    recommendation: String,
}
```

---

## 🔧 Best Practices

### 1. Appropriate Metric Recording

```rust
impl GraphDatabase {
    fn good_metric_recording(&self) {
        let operation_start = std::time::Instant::now();

        // Your operation logic here
        let result = self.perform_operation();

        let latency = operation_start.elapsed().as_micros() as u64;
        let cluster_id = self.get_operation_cluster_id(&result);

        // Good practice: record meaningful operation types
        self.metrics.record_write_operation(
            result.size_bytes(),
            latency,
            cluster_id,
            self.get_operation_type(&result) // Specific operation type
        );
    }

    fn get_operation_type(&self, result: &OperationResult) -> &'static str {
        match result.operation_kind {
            OperationKind::EdgeInsert => "edge_insert",
            OperationKind::EdgeUpdate => "edge_update",
            OperationKind::NodeInsert => "node_insert",
            OperationKind::NodeUpdate => "node_update",
            OperationKind::FreeSpaceAllocate => "free_space_allocate",
            OperationKind::StringInsert => "string_insert",
        }
    }
}
```

### 2. Error Tracking Best Practices

```rust
impl GraphDatabase {
    fn good_error_tracking(&self, error: &GraphError, operation_context: &str) {
        // Good practice: record structured error information
        self.metrics.record_error(
            error.error_type(),                          // Specific error type
            error.to_string().as_str(),                 // Detailed message
            format!("{}: {}", operation_context, error.context()).as_str(), // Context
            error.recovery_action()                      // What was done to recover
        );
    }
}

trait GraphError {
    fn error_type(&self) -> &'static str;
    fn context(&self) -> String;
    fn recovery_action(&self) -> &'static str;
}
```

### 3. Performance Monitoring Integration

```rust
impl GraphDatabase {
    fn setup_monitoring(&mut self) {
        // Set up periodic performance monitoring
        let metrics_clone = self.metrics.clone(); // If metrics is wrapped in Arc

        std::thread::spawn(move || {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(60));

                let counters = metrics_clone.get_counters();
                let histogram = metrics_clone.get_latency_histogram();
                let (records_per_sec, bytes_per_sec, tx_per_sec) =
                    metrics_clone.get_throughput_tracker().get_current_throughput();

                // Log or alert based on performance metrics
                if histogram.get_write_percentile(95.0) > 10_000 {
                    eprintln!("WARNING: High write latency detected (P95 > 10ms)");
                }

                if records_per_sec < 100.0 {
                    eprintln!("WARNING: Low throughput detected (< 100 ops/sec)");
                }

                // Could send to monitoring system here
            }
        });
    }
}
```

---

## 🎯 Summary

### Key Points for Developers

1. **Zero Migration Required**: All existing code continues to work without changes
2. **Enhanced Development**: New modular structure provides better organization and tooling
3. **Optional Enhancements**: New import patterns and focused testing capabilities available
4. **Performance Preserved**: Zero runtime overhead from modularization
5. **Future Extensibility**: Easy to extend and enhance specific metric areas

### Recommended Adoption Strategy

1. **Immediate**: Continue using existing code - no changes needed
2. **Optional**: Gradually adopt new import patterns for new code
3. **Enhanced**: Use focused testing for new metric components
4. **Advanced**: Leverage individual modules for specialized use cases

The modularization provides immediate benefits for maintainability while enabling future enhancements without disrupting existing functionality.

---

**Document Version**: 1.0
**Created**: 2025-12-20
**Status**: ✅ **DEVELOPER GUIDE COMPLETE**
**Audience**: All developers working with the WAL metrics system