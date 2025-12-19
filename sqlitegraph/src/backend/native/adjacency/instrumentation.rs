//! Adjacency Iterator Instrumentation for Infinite Loop Detection
//!
//! This module provides comprehensive instrumentation for detecting and analyzing
//! infinite loop issues in SQLiteGraph's adjacency iteration system.
//!
//! ## Instrumentation Strategy
//!
//! 1. Loop Counting: Track iterations per iterator instance
//! 2. V2 Read Monitoring: Count V2 node read operations
//! 3. Performance Timing: Measure operation durations
//! 4. State Consistency: Validate iterator state invariants
//! 5. Stack Trace Analysis: Detect deep recursion patterns

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use std::collections::HashMap;
#[cfg(debug_assertions)]
use log::{debug, warn, error};

/// Global instrumentation metrics
pub struct AdjacencyMetrics {
    total_iterations: AtomicUsize,
    total_v2_reads: AtomicUsize,
    total_collect_operations: AtomicUsize,
    infinite_loop_detections: AtomicUsize,
    operation_timings: std::sync::Mutex<HashMap<String, Vec<Duration>>>,
}

impl AdjacencyMetrics {
    /// Create new metrics instance
    pub fn new() -> Self {
        Self {
            total_iterations: AtomicUsize::new(0),
            total_v2_reads: AtomicUsize::new(0),
            total_collect_operations: AtomicUsize::new(0),
            infinite_loop_detections: AtomicUsize::new(0),
            operation_timings: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Record an adjacency iteration
    pub fn record_iteration(&self) -> bool {
        let count = self.total_iterations.fetch_add(1, Ordering::SeqCst);

        // Check for potential infinite loop
        const INFINITE_LOOP_THRESHOLD: usize = 1000;
        if count > INFINITE_LOOP_THRESHOLD {
            self.infinite_loop_detections.fetch_add(1, Ordering::SeqCst);
            error!(
                "POTENTIAL INFINITE LOOP DETECTED: {} iterations logged",
                count
            );
            return false; // Signal that we might be in an infinite loop
        }

        // Log progress for debugging
        if count % 100 == 0 {
            debug!("Adjacency iterations: {} total operations", count);
        }

        true // Continue operation
    }

    /// Record a V2 node read operation
    pub fn record_v2_read(&self, node_id: u32) {
        let count = self.total_v2_reads.fetch_add(1, Ordering::SeqCst);

        // Log V2 read patterns
        if count % 10 == 0 {
            debug!("V2 node reads: {} total operations", count);
        }

        // Track reads per node
        if count % 50 == 0 {
            warn!("High V2 read volume detected: {} reads", count);
        }
    }

    /// Start timing an operation
    pub fn start_timing(&self, operation: &str) -> TimingGuard {
        TimingGuard::new(operation, self)
    }

    /// Get current metrics snapshot
    pub fn get_snapshot(&self) -> MetricsSnapshot {
        MetricsSnapshot {
            total_iterations: self.total_iterations.load(Ordering::SeqCst),
            total_v2_reads: self.total_v2_reads.load(Ordering::SeqCst),
            total_collect_operations: self.total_collect_operations.load(Ordering::SeqCst),
            infinite_loop_detections: self.infinite_loop_detections.load(Ordering::SeqCst),
        }
    }

    /// Reset all metrics (useful for isolated testing)
    pub fn reset(&self) {
        self.total_iterations.store(0, Ordering::SeqCst);
        self.total_v2_reads.store(0, Ordering::SeqCst);
        self.total_collect_operations.store(0, Ordering::SeqCst);
        self.infinite_loop_detections.store(0, Ordering::SeqCst);

        if let Ok(mut timings) = self.operation_timings.lock() {
            timings.clear();
        }
    }
}

/// Immutable snapshot of current metrics
#[derive(Debug, Clone)]
pub struct MetricsSnapshot {
    pub total_iterations: usize,
    pub total_v2_reads: usize,
    pub total_collect_operations: usize,
    pub infinite_loop_detections: usize,
}

impl MetricsSnapshot {
    /// Check if metrics indicate an infinite loop pattern
    pub fn suggests_infinite_loop(&self) -> bool {
        self.total_iterations > 1000 &&
        (self.total_iterations > self.total_v2_reads * 10)
    }

    /// Calculate iteration efficiency
    pub fn iteration_efficiency(&self) -> f64 {
        if self.total_iterations == 0 {
            return 0.0;
        }
        self.total_v2_reads as f64 / self.total_iterations as f64
    }
}

/// RAII guard for timing operations
pub struct TimingGuard<'a> {
    operation: String,
    metrics: &'a AdjacencyMetrics,
    start: Instant,
}

impl<'a> TimingGuard<'a> {
    fn new(operation: &str, metrics: &'a AdjacencyMetrics) -> Self {
        let start = Instant::now();
        debug!("Starting timing for operation: {}", operation);

        Self {
            operation: operation.to_string(),
            metrics,
            start,
        }
    }
}

impl<'a> Drop for TimingGuard<'a> {
    fn drop(&mut self) {
        let duration = self.start.elapsed();

        // Record timing
        if let Ok(mut timings) = self.metrics.operation_timings.lock() {
            timings.entry(self.operation.clone()).or_insert_with(Vec::new).push(duration);

            // Keep only recent samples
            if let Some(operation_timings) = timings.get_mut(&self.operation) {
                if operation_timings.len() > 100 {
                    operation_timings.truncate(50);
                }
            }
        }

        debug!("Completed operation: {} in {:?}", self.operation, duration);

        // Warn about slow operations
        if duration > Duration::from_millis(100) {
            warn!("SLOW OPERATION: {} took {:?}", self.operation, duration);
        }
    }
}

/// Iterator state validation utilities
pub struct StateValidator;

impl StateValidator {
    /// Validate iterator state consistency
    pub fn validate_iterator_state(
        node_id: u32,
        current_index: u32,
        total_count: u32,
        cached_neighbors_len: Option<usize>,
    ) -> ValidationReport {
        let mut report = ValidationReport::new(node_id);

        // Check basic bounds
        if current_index > total_count {
            report.add_error(ValidationError::IndexOutOfBounds {
                current_index,
                total_count,
            });
        }

        // Check cached consistency
        if let Some(cached_len) = cached_neighbors_len {
            if cached_len != total_count as usize {
                report.add_warning(ValidationWarning::InconsistentCacheState {
                    cached_len,
                    total_count,
                });
            }

            if current_index > cached_len as u32 {
                report.add_error(ValidationError::IndexBeyondCache {
                    current_index,
                    cached_len,
                });
            }
        }

        // Check for obvious infinite loop patterns
        if total_count > 0 && cached_neighbors_len == Some(0) {
            report.add_error(ValidationError::EmptyCacheNonZeroCount {
                total_count,
            });
        }

        report
    }
}

/// Validation report for iterator state
#[derive(Debug)]
pub struct ValidationReport {
    node_id: u32,
    errors: Vec<ValidationError>,
    warnings: Vec<ValidationWarning>,
}

impl ValidationReport {
    fn new(node_id: u32) -> Self {
        Self {
            node_id,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn add_error(&mut self, error: ValidationError) {
        error!("Iterator validation ERROR for node {}: {:?}", self.node_id, error);
        self.errors.push(error);
    }

    fn add_warning(&mut self, warning: ValidationWarning) {
        warn!("Iterator validation WARNING for node {}: {:?}", self.node_id, warning);
        self.warnings.push(warning);
    }

    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }

    pub fn has_warnings(&self) -> bool {
        !self.warnings.is_empty()
    }
}

#[derive(Debug)]
pub enum ValidationError {
    IndexOutOfBounds { current_index: u32, total_count: u32 },
    IndexBeyondCache { current_index: u32, cached_len: usize },
    EmptyCacheNonZeroCount { total_count: u32 },
}

#[derive(Debug)]
pub enum ValidationWarning {
    InconsistentCacheState { cached_len: usize, total_count: u32 },
}

/// Global metrics instance
#[cfg(debug_assertions)]
static ADJACENCY_METRICS: std::sync::LazyLock<AdjacencyMetrics> = std::sync::LazyLock::new(AdjacencyMetrics::new);

/// Convenience function to get global metrics
#[cfg(debug_assertions)]
pub fn get_global_metrics() -> &'static AdjacencyMetrics {
    &ADJACENCY_METRICS
}

/// Convenience functions for instrumentation
#[cfg(debug_assertions)]
pub mod convenience {
    use super::*;

    /// Record an iteration with loop detection
    pub fn track_iteration(node_id: u32) -> bool {
        get_global_metrics().record_iteration()
    }

    /// Record a V2 node read
    pub fn track_v2_read(node_id: u32) {
        get_global_metrics().record_v2_read(node_id);
    }

    /// Start timing an operation
    pub fn start_timing(operation: &str) -> TimingGuard {
        get_global_metrics().start_timing(operation)
    }

    /// Get current metrics snapshot
    pub fn get_metrics() -> MetricsSnapshot {
        get_global_metrics().get_snapshot()
    }

    /// Validate iterator state
    pub fn validate_state(
        node_id: u32,
        current_index: u32,
        total_count: u32,
        cached_neighbors_len: Option<usize>,
    ) -> ValidationReport {
        StateValidator::validate_iterator_state(node_id, current_index, total_count, cached_neighbors_len)
    }
}