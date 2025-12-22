//! Metrics aggregation and statistical computation for V2 WAL performance monitoring.
//!
//! This module provides comprehensive aggregation capabilities for collecting, processing,
//! and analyzing performance metrics over time windows. It includes latency histograms,
//! throughput tracking, and statistical analysis functions.

use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

/// Latency histogram for performance analysis.
///
/// Provides latency distribution tracking with configurable buckets for
/// different operation types (write, read, flush, checkpoint). Enables
/// percentile calculations and latency pattern analysis.
///
/// # Examples
///
/// ```rust
/// use crate::backend::native::v2::wal::metrics::aggregation::LatencyHistogram;
///
/// let mut histogram = LatencyHistogram::new();
/// histogram.record_write_latency(1000); // 1ms
/// histogram.record_write_latency(5000); // 5ms
///
/// let p95 = histogram.get_write_percentile(95.0);
/// assert!(p95 >= 1000);
/// ```
#[derive(Debug, Clone)]
pub struct LatencyHistogram {
    /// Buckets for latency distribution (in microseconds)
    write_buckets: Vec<u64>,
    read_buckets: Vec<u64>,
    flush_buckets: Vec<u64>,
    checkpoint_buckets: Vec<u64>,

    /// Bucket boundaries (in microseconds)
    bucket_boundaries: Vec<u64>,
}

/// Throughput tracker for monitoring performance over time.
///
/// Provides time-windowed throughput calculations for records, bytes,
/// and transactions. Maintains sliding windows of recent performance
/// data for real-time monitoring and trend analysis.
///
/// # Examples
///
/// ```rust
/// use crate::backend::native::v2::wal::metrics::aggregation::ThroughputTracker;
///
/// let mut tracker = ThroughputTracker::new();
/// tracker.record_write_operation(1024);
/// tracker.record_transaction();
///
/// let (records, bytes, tx) = tracker.get_current_throughput();
/// assert!(records > 0.0);
/// ```
#[derive(Debug, Clone)]
pub struct ThroughputTracker {
    /// Records per second over last N seconds
    records_per_second: VecDeque<(u64, u64)>,

    /// Bytes per second over last N seconds
    bytes_per_second: VecDeque<(u64, u64)>,

    /// Transactions per second over last N seconds
    transactions_per_second: VecDeque<(u64, u64)>,

    /// Time window size in seconds
    time_window_seconds: usize,

    /// Maximum samples to keep
    max_samples: usize,
}

impl LatencyHistogram {
    /// Create new latency histogram with default bucket boundaries.
    ///
    /// Initializes buckets optimized for typical database operation latency
    /// ranges, from microseconds to tens of milliseconds.
    ///
    /// # Returns
    ///
    /// A new `LatencyHistogram` instance with initialized buckets.
    pub fn new() -> Self {
        // Define bucket boundaries: 1, 10, 50, 100, 500, 1000, 5000, 10000, 50000 microseconds
        let bucket_boundaries = vec![1, 10, 50, 100, 500, 1000, 5000, 10000, 50000];
        let bucket_count = bucket_boundaries.len() + 1; // +1 for > last bucket

        Self {
            write_buckets: vec![0; bucket_count],
            read_buckets: vec![0; bucket_count],
            flush_buckets: vec![0; bucket_count],
            checkpoint_buckets: vec![0; bucket_count],
            bucket_boundaries,
        }
    }

    /// Record write latency in appropriate bucket.
    ///
    /// Automatically determines the correct bucket based on the latency
    /// value and increments the corresponding counter.
    ///
    /// # Arguments
    ///
    /// * `latency_us` - Write operation latency in microseconds
    pub fn record_write_latency(&mut self, latency_us: u64) {
        let bucket_index = self.get_bucket_index(latency_us);
        self.write_buckets[bucket_index] += 1;
    }

    /// Record read latency in appropriate bucket.
    ///
    /// Records read operation latency data for performance analysis
    /// and pattern identification.
    ///
    /// # Arguments
    ///
    /// * `latency_us` - Read operation latency in microseconds
    pub fn record_read_latency(&mut self, latency_us: u64) {
        let bucket_index = self.get_bucket_index(latency_us);
        self.read_buckets[bucket_index] += 1;
    }

    /// Record flush latency in appropriate bucket.
    ///
    /// Tracks flush operation latencies which are typically longer
    /// than individual read/write operations due to disk sync requirements.
    ///
    /// # Arguments
    ///
    /// * `latency_us` - Flush operation latency in microseconds
    pub fn record_flush_latency(&mut self, latency_us: u64) {
        let bucket_index = self.get_bucket_index(latency_us);
        self.flush_buckets[bucket_index] += 1;
    }

    /// Record checkpoint latency in appropriate bucket.
    ///
    /// Checkpoint operations typically have the longest latencies
    /// due to comprehensive data processing and synchronization.
    ///
    /// # Arguments
    ///
    /// * `latency_us` - Checkpoint operation latency in microseconds
    pub fn record_checkpoint_latency(&mut self, latency_us: u64) {
        let bucket_index = self.get_bucket_index(latency_us);
        self.checkpoint_buckets[bucket_index] += 1;
    }

    /// Get bucket index for latency value.
    ///
    /// Determines the appropriate bucket index for a given latency
    /// value based on the configured bucket boundaries.
    ///
    /// # Arguments
    ///
    /// * `latency_us` - Latency value in microseconds
    ///
    /// # Returns
    ///
    /// Index of the appropriate bucket
    fn get_bucket_index(&self, latency_us: u64) -> usize {
        for (i, &boundary) in self.bucket_boundaries.iter().enumerate() {
            if latency_us <= boundary {
                return i;
            }
        }
        self.bucket_boundaries.len() // Last bucket for latencies > max boundary
    }

    /// Reset histogram to initial state.
    ///
    /// Clears all latency data and resets bucket counters to zero.
    /// Typically used for starting fresh measurements or periodic reset.
    pub fn reset(&mut self) {
        for bucket in &mut self.write_buckets {
            *bucket = 0;
        }
        for bucket in &mut self.read_buckets {
            *bucket = 0;
        }
        for bucket in &mut self.flush_buckets {
            *bucket = 0;
        }
        for bucket in &mut self.checkpoint_buckets {
            *bucket = 0;
        }
    }

    /// Get percentile for write operations.
    ///
    /// Calculates the approximate latency value at the specified percentile
    /// based on the accumulated write operation data.
    ///
    /// # Arguments
    ///
    /// * `percentile` - Percentile to calculate (0.0 to 100.0)
    ///
    /// # Returns
    ///
    /// Approximate latency value at the specified percentile
    pub fn get_write_percentile(&self, percentile: f64) -> u64 {
        self.get_percentile(&self.write_buckets, percentile)
    }

    /// Get percentile for read operations.
    ///
    /// Calculates the approximate read latency value at the specified percentile.
    ///
    /// # Arguments
    ///
    /// * `percentile` - Percentile to calculate (0.0 to 100.0)
    ///
    /// # Returns
    ///
    /// Approximate read latency value at the specified percentile
    pub fn get_read_percentile(&self, percentile: f64) -> u64 {
        self.get_percentile(&self.read_buckets, percentile)
    }

    /// Get percentile for flush operations.
    ///
    /// Calculates the approximate flush latency value at the specified percentile.
    ///
    /// # Arguments
    ///
    /// * `percentile` - Percentile to calculate (0.0 to 100.0)
    ///
    /// # Returns
    ///
    /// Approximate flush latency value at the specified percentile
    pub fn get_flush_percentile(&self, percentile: f64) -> u64 {
        self.get_percentile(&self.flush_buckets, percentile)
    }

    /// Get percentile for checkpoint operations.
    ///
    /// Calculates the approximate checkpoint latency value at the specified percentile.
    ///
    /// # Arguments
    ///
    /// * `percentile` - Percentile to calculate (0.0 to 100.0)
    ///
    /// # Returns
    ///
    /// Approximate checkpoint latency value at the specified percentile
    pub fn get_checkpoint_percentile(&self, percentile: f64) -> u64 {
        self.get_percentile(&self.checkpoint_buckets, percentile)
    }

    /// Calculate percentile from histogram buckets.
    ///
    /// Internal method that performs the percentile calculation based on
    /// accumulated bucket data. Uses linear interpolation for better accuracy.
    ///
    /// # Arguments
    ///
    /// * `buckets` - Array of bucket counts
    /// * `percentile` - Percentile to calculate (0.0 to 100.0)
    ///
    /// # Returns
    ///
    /// Approximate latency value at the specified percentile
    fn get_percentile(&self, buckets: &[u64], percentile: f64) -> u64 {
        let total: u64 = buckets.iter().sum();
        if total == 0 {
            return 0;
        }

        let target = (total as f64 * percentile / 100.0) as u64;
        let mut cumulative = 0;
        let mut prev_cumulative = 0;
        let mut prev_boundary = 0;

        for (i, &count) in buckets.iter().enumerate() {
            prev_cumulative = cumulative;
            cumulative += count;

            if cumulative >= target {
                // Calculate interpolated value within this bucket
                let bucket_start = prev_boundary;
                let bucket_end = if i < self.bucket_boundaries.len() {
                    self.bucket_boundaries[i]
                } else {
                    self.bucket_boundaries.last().copied().unwrap_or(0) * 2
                };

                // Linear interpolation within the bucket
                if count == 0 {
                    return bucket_end;
                }

                let position_in_bucket = (target - prev_cumulative) as f64 / count as f64;
                let interpolated_value =
                    bucket_start as f64 + (bucket_end - bucket_start) as f64 * position_in_bucket;

                return interpolated_value as u64;
            }

            // Update prev_boundary for next iteration
            if i < self.bucket_boundaries.len() {
                prev_boundary = self.bucket_boundaries[i];
            }
        }

        // If we haven't reached the target, return the maximum boundary
        self.bucket_boundaries.last().copied().unwrap_or(0) * 2
    }

    /// Get comprehensive latency statistics.
    ///
    /// Returns key percentile values for all operation types, providing
    /// a comprehensive view of latency distribution patterns.
    ///
    /// # Returns
    ///
    /// Tuple containing (p50_write, p95_write, p99_write, p50_read, p95_read, p99_read)
    pub fn get_comprehensive_stats(&self) -> (u64, u64, u64, u64, u64, u64) {
        (
            self.get_write_percentile(50.0),
            self.get_write_percentile(95.0),
            self.get_write_percentile(99.0),
            self.get_read_percentile(50.0),
            self.get_read_percentile(95.0),
            self.get_read_percentile(99.0),
        )
    }
}

impl ThroughputTracker {
    /// Create new throughput tracker with default settings.
    ///
    /// Initializes with a 60-second time window and maximum of 300 samples
    /// (5 minutes of historical data) for comprehensive trend analysis.
    ///
    /// # Returns
    ///
    /// A new `ThroughputTracker` instance ready for operation tracking
    pub fn new() -> Self {
        Self {
            records_per_second: VecDeque::new(),
            bytes_per_second: VecDeque::new(),
            transactions_per_second: VecDeque::new(),
            time_window_seconds: 60, // 1 minute window
            max_samples: 300,        // 5 minutes of data
        }
    }

    /// Record a write operation.
    ///
    /// Adds a write operation to the throughput tracking, updating
    /// both record and byte throughput metrics.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Number of bytes written in the operation
    pub fn record_write_operation(&mut self, bytes: usize) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.records_per_second.push_back((now, 1));
        self.bytes_per_second.push_back((now, bytes as u64));

        // Remove old samples
        self.cleanup_old_samples();
    }

    /// Record a read operation.
    ///
    /// Adds a read operation to the throughput tracking for performance
    /// monitoring and capacity planning.
    ///
    /// # Arguments
    ///
    /// * `bytes` - Number of bytes read in the operation
    pub fn record_read_operation(&mut self, bytes: usize) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.records_per_second.push_back((now, 1));
        self.bytes_per_second.push_back((now, bytes as u64));

        // Remove old samples
        self.cleanup_old_samples();
    }

    /// Record a transaction.
    ///
    /// Tracks transaction completion rates for monitoring transaction
    /// throughput and system performance under load.
    pub fn record_transaction(&mut self) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        self.transactions_per_second.push_back((now, 1));

        // Remove old samples
        self.cleanup_old_samples();
    }

    /// Clean up old samples beyond time window.
    ///
    /// Removes samples that are older than the configured time window
    /// to maintain memory efficiency and accurate recent performance metrics.
    fn cleanup_old_samples(&mut self) {
        let cutoff = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .saturating_sub(self.time_window_seconds as u64);

        // Clean old records
        while let Some((timestamp, _)) = self.records_per_second.front() {
            if *timestamp < cutoff {
                self.records_per_second.pop_front();
            } else {
                break;
            }
        }

        // Clean old bytes
        while let Some((timestamp, _)) = self.bytes_per_second.front() {
            if *timestamp < cutoff {
                self.bytes_per_second.pop_front();
            } else {
                break;
            }
        }

        // Clean old transactions
        while let Some((timestamp, _)) = self.transactions_per_second.front() {
            if *timestamp < cutoff {
                self.transactions_per_second.pop_front();
            } else {
                break;
            }
        }

        // Limit maximum samples to prevent memory growth
        while self.records_per_second.len() > self.max_samples {
            self.records_per_second.pop_front();
        }
        while self.bytes_per_second.len() > self.max_samples {
            self.bytes_per_second.pop_front();
        }
        while self.transactions_per_second.len() > self.max_samples {
            self.transactions_per_second.pop_front();
        }
    }

    /// Get current throughput metrics.
    ///
    /// Calculates current throughput rates based on the accumulated data
    /// within the configured time window.
    ///
    /// # Returns
    ///
    /// Tuple containing (records_per_sec, bytes_per_sec, transactions_per_sec)
    pub fn get_current_throughput(&self) -> (f64, f64, f64) {
        let records_per_sec = if self.records_per_second.is_empty() {
            0.0
        } else {
            self.records_per_second
                .iter()
                .map(|(_, count)| *count)
                .sum::<u64>() as f64
                / self.time_window_seconds as f64
        };

        let bytes_per_sec = if self.bytes_per_second.is_empty() {
            0.0
        } else {
            self.bytes_per_second
                .iter()
                .map(|(_, bytes)| *bytes)
                .sum::<u64>() as f64
                / self.time_window_seconds as f64
        };

        let tx_per_sec = if self.transactions_per_second.is_empty() {
            0.0
        } else {
            self.transactions_per_second
                .iter()
                .map(|(_, count)| *count)
                .sum::<u64>() as f64
                / self.time_window_seconds as f64
        };

        (records_per_sec, bytes_per_sec, tx_per_sec)
    }

    /// Get peak throughput metrics.
    ///
    /// Calculates the maximum observed throughput rates for capacity
    /// planning and performance benchmarking.
    ///
    /// # Returns
    ///
    /// Tuple containing (peak_records_per_sec, peak_bytes_per_sec, peak_transactions_per_sec)
    pub fn get_peak_throughput(&self) -> (f64, f64, f64) {
        let peak_records_per_sec = self.records_per_second.len() as f64;
        let peak_bytes_per_sec = self
            .bytes_per_second
            .iter()
            .map(|(_, bytes)| *bytes)
            .sum::<u64>() as f64;
        let peak_tx_per_sec = self.transactions_per_second.len() as f64;

        (peak_records_per_sec, peak_bytes_per_sec, peak_tx_per_sec)
    }

    /// Reset throughput tracker.
    ///
    /// Clears all accumulated throughput data and resets the tracker
    /// to its initial state for fresh measurements.
    pub fn reset(&mut self) {
        self.records_per_second.clear();
        self.bytes_per_second.clear();
        self.transactions_per_second.clear();
    }
}

impl Default for LatencyHistogram {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ThroughputTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_latency_histogram_new() {
        let histogram = LatencyHistogram::new();
        assert_eq!(histogram.write_buckets.len(), 10); // 9 boundaries + 1 overflow bucket
        assert_eq!(histogram.read_buckets.len(), 10);
        assert_eq!(histogram.flush_buckets.len(), 10);
        assert_eq!(histogram.checkpoint_buckets.len(), 10);
    }

    #[test]
    fn test_latency_histogram_recording() {
        let mut histogram = LatencyHistogram::new();

        histogram.record_write_latency(5); // Goes in bucket 0 (<=1)
        histogram.record_write_latency(15); // Goes in bucket 1 (<=10)
        histogram.record_write_latency(5000); // Goes in bucket 6 (<=5000)

        let total_samples: u64 = histogram.write_buckets.iter().sum();
        assert_eq!(total_samples, 3);
        assert!(histogram.get_write_percentile(50.0) > 0);
    }

    #[test]
    fn test_latency_histogram_percentiles() {
        let mut histogram = LatencyHistogram::new();

        // Add samples across different latency ranges
        for i in 0..100 {
            histogram.record_write_latency((i + 1) * 100); // 100us to 10ms
        }

        let p50 = histogram.get_write_percentile(50.0);
        let p95 = histogram.get_write_percentile(95.0);
        let p99 = histogram.get_write_percentile(99.0);

        assert!(p50 < p95);
        assert!(p95 < p99);
    }

    #[test]
    fn test_latency_histogram_reset() {
        let mut histogram = LatencyHistogram::new();

        histogram.record_write_latency(1000);
        histogram.record_read_latency(500);

        assert!(histogram.write_buckets.iter().sum::<u64>() > 0);
        assert!(histogram.read_buckets.iter().sum::<u64>() > 0);

        histogram.reset();

        assert_eq!(histogram.write_buckets.iter().sum::<u64>(), 0);
        assert_eq!(histogram.read_buckets.iter().sum::<u64>(), 0);
    }

    #[test]
    fn test_throughput_tracker_new() {
        let tracker = ThroughputTracker::new();
        assert_eq!(tracker.time_window_seconds, 60);
        assert_eq!(tracker.max_samples, 300);
        assert!(tracker.records_per_second.is_empty());
        assert!(tracker.bytes_per_second.is_empty());
        assert!(tracker.transactions_per_second.is_empty());
    }

    #[test]
    fn test_throughput_tracker_recording() {
        let mut tracker = ThroughputTracker::new();

        tracker.record_write_operation(100);
        tracker.record_write_operation(200);
        tracker.record_transaction();

        let (records, bytes, tx) = tracker.get_current_throughput();
        assert!(records > 0.0);
        assert!(bytes > 0.0);
        assert!(tx > 0.0);
    }

    #[test]
    fn test_throughput_tracker_peak() {
        let mut tracker = ThroughputTracker::new();

        tracker.record_write_operation(1024);
        tracker.record_transaction();

        let (peak_records, peak_bytes, peak_tx) = tracker.get_peak_throughput();
        assert!(peak_records >= 0.0);
        assert!(peak_bytes >= 1024.0);
        assert!(peak_tx >= 1.0);
    }

    #[test]
    fn test_throughput_tracker_reset() {
        let mut tracker = ThroughputTracker::new();

        tracker.record_write_operation(100);
        tracker.record_transaction();

        assert!(!tracker.records_per_second.is_empty());
        assert!(!tracker.transactions_per_second.is_empty());

        tracker.reset();

        assert!(tracker.records_per_second.is_empty());
        assert!(tracker.bytes_per_second.is_empty());
        assert!(tracker.transactions_per_second.is_empty());
    }

    #[test]
    fn test_comprehensive_latency_stats() {
        let mut histogram = LatencyHistogram::new();

        // Add sample data
        for i in 1..=50 {
            histogram.record_write_latency(i * 10); // 10us to 500us
            histogram.record_read_latency(i * 5); // 5us to 250us
        }

        let (p50_write, p95_write, p99_write, p50_read, p95_read, p99_read) =
            histogram.get_comprehensive_stats();

        // Verify reasonable percentile relationships
        assert!(p50_write <= p95_write);
        assert!(p95_write <= p99_write);
        assert!(p50_read <= p95_read);
        assert!(p95_read <= p99_read);
    }

    #[test]
    fn test_bucket_index_calculation() {
        let histogram = LatencyHistogram::new();

        // Test boundary conditions
        assert_eq!(histogram.get_bucket_index(0), 0); // Below first boundary
        assert_eq!(histogram.get_bucket_index(1), 0); // Exactly at first boundary
        assert_eq!(histogram.get_bucket_index(10), 1); // Exactly at second boundary
        assert_eq!(histogram.get_bucket_index(1000), 5); // Mid-range boundary
        assert_eq!(histogram.get_bucket_index(100000), 9); // Above maximum boundary
    }
}
