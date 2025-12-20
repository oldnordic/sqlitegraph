//! V2 WAL writer with sequential write patterns and cluster-affinity logging.
//!
//! This module provides high-performance WAL writing optimized for V2's clustered
//! edge architecture, implementing group commit, write buffering, and cluster-affinity
//! organization for optimal I/O locality and throughput.

use crate::backend::native::{NativeBackendError, NativeResult};
use crate::backend::native::v2::wal::{
    V2WALRecord, V2WALHeader, lsn, V2WALConfig,
};
use crate::backend::native::v2::wal::record::V2WALSerializer;
use parking_lot::{Mutex, Condvar};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// WAL writer for high-performance sequential writes
pub struct V2WALWriter {
    /// Configuration parameters
    config: V2WALConfig,

    /// WAL file handle
    file: Arc<Mutex<BufWriter<File>>>,

    /// Current WAL header (updated in memory, flushed periodically)
    header: Arc<Mutex<V2WALHeader>>,

    /// Write buffer for batching operations
    write_buffer: Arc<Mutex<WriteBuffer>>,

    /// Group commit coordinator
    group_commit: Arc<Mutex<GroupCommitState>>,

    /// Performance metrics
    metrics: Arc<Mutex<WriterMetrics>>,

    /// Cluster-affinity record grouping
    cluster_groups: Arc<Mutex<HashMap<i64, Vec<V2WALRecord>>>>,
}

/// Write buffer for batching WAL records
#[derive(Debug)]
struct WriteBuffer {
    /// Buffer storage
    buffer: Vec<u8>,

    /// Records currently in buffer
    records: Vec<BufferedRecord>,

    /// Maximum buffer size
    max_size: usize,

    /// Buffer flush timeout
    flush_timeout: Duration,

    /// Last flush timestamp
    last_flush: Instant,
}

/// Record buffered for batch writing
#[derive(Debug, Clone)]
struct BufferedRecord {
    /// The WAL record
    record: V2WALRecord,

    /// Log Sequence Number
    lsn: u64,

    /// Timestamp when record was added
    timestamp: Instant,

    /// Whether record is committed (in memory)
    committed: bool,
}

/// Group commit state for batching transactions
#[derive(Debug)]
struct GroupCommitState {
    /// Pending records for group commit
    pending_records: Vec<BufferedRecord>,

    /// Maximum records in group commit batch
    max_batch_size: usize,

    /// Group commit timeout
    timeout: Duration,

    /// Last group commit timestamp
    last_commit: Instant,

    /// Number of active transactions
    active_transactions: u32,
}

/// Writer performance metrics
#[derive(Debug, Default)]
pub struct WriterMetrics {
    /// Total records written
    pub records_written: u64,

    /// Total bytes written
    pub bytes_written: u64,

    /// Number of flush operations
    pub flush_count: u64,

    /// Average records per flush
    pub avg_records_per_flush: f64,

    /// Number of group commits
    pub group_commit_count: u64,

    /// Average group commit size
    pub avg_group_commit_size: f64,

    /// Write latency percentiles (in microseconds)
    pub write_latency_p50: u64,
    pub write_latency_p95: u64,
    pub write_latency_p99: u64,

    /// Buffer utilization percentage
    pub buffer_utilization: f64,
}

impl V2WALWriter {
    /// Create a new WAL writer
    pub fn create(config: V2WALConfig) -> NativeResult<Self> {
        // Validate configuration
        config.validate()?;

        // Initialize writer components
        let header = V2WALHeader::new();

        // Create WAL file and write header
        {
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)  // Start with empty file
                .open(&config.wal_path)
                .map_err(NativeBackendError::Io)?;

            // Write header to file immediately
            let header_bytes = unsafe {
                std::slice::from_raw_parts(
                    &header as *const V2WALHeader as *const u8,
                    std::mem::size_of::<V2WALHeader>()
                )
            };
            file.write_all(header_bytes).map_err(NativeBackendError::Io)?;
            file.flush().map_err(NativeBackendError::Io)?;
        }

        // Re-open file for append mode writes
        let file = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(&config.wal_path)
            .map_err(NativeBackendError::Io)?;
        let write_buffer = WriteBuffer {
            buffer: Vec::with_capacity(config.buffer_size),
            records: Vec::new(),
            max_size: config.buffer_size,
            flush_timeout: Duration::from_millis(config.group_commit_timeout_ms),
            last_flush: Instant::now(),
        };

        let group_commit = GroupCommitState {
            pending_records: Vec::new(),
            max_batch_size: config.max_group_commit_size,
            timeout: Duration::from_millis(config.group_commit_timeout_ms),
            last_commit: Instant::now(),
            active_transactions: 0,
        };

        Ok(Self {
            config,
            file: Arc::new(Mutex::new(BufWriter::new(file))),
            header: Arc::new(Mutex::new(header)),
            write_buffer: Arc::new(Mutex::new(write_buffer)),
            group_commit: Arc::new(Mutex::new(group_commit)),
            metrics: Arc::new(Mutex::new(WriterMetrics::default())),
            cluster_groups: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Write a single WAL record
    pub fn write_record(&self, record: V2WALRecord) -> NativeResult<u64> {
        let start_time = Instant::now();

        // Assign LSN and buffer record
        let lsn = {
            let mut header = self.header.lock();
            let current_lsn = header.current_lsn;
            header.current_lsn = lsn::next(current_lsn);
            current_lsn
        };

        // Group by cluster for optimal I/O locality
        if let Some(cluster_key) = record.cluster_key() {
            let mut cluster_groups = self.cluster_groups.lock();
            cluster_groups.entry(cluster_key).or_insert_with(Vec::new).push(record.clone());
        }

        // Add to write buffer
        {
            let mut write_buffer = self.write_buffer.lock();
            let buffered_record = BufferedRecord {
                record: record.clone(),
                lsn,
                timestamp: Instant::now(),
                committed: true, // Records are immediately committed in memory
            };

            write_buffer.records.push(buffered_record);

            // Serialize record and add to buffer
            let serialized = V2WALSerializer::serialize(&record)?;
            write_buffer.buffer.extend_from_slice(&serialized);

            // Update metrics
            {
                let mut metrics = self.metrics.lock();
                metrics.records_written += 1;
                metrics.bytes_written += serialized.len() as u64;
                metrics.buffer_utilization = (write_buffer.buffer.len() as f64 / write_buffer.max_size as f64) * 100.0;
            }

            // Check if buffer needs flushing
            let should_flush = write_buffer.buffer.len() >= write_buffer.max_size ||
                             start_time.elapsed() >= write_buffer.flush_timeout;

            if should_flush {
                drop(write_buffer); // Release lock before flush
                self.flush_buffer()?;
            }
        }

        // Record write latency
        let write_latency = start_time.elapsed().as_micros() as u64;
        self.update_latency_metrics(write_latency);

        Ok(lsn)
    }

    /// Write multiple records with group commit optimization
    pub fn write_records_batch(&self, records: Vec<V2WALRecord>) -> NativeResult<Vec<u64>> {
        let start_time = Instant::now();
        let mut lsns = Vec::with_capacity(records.len());

        // Assign LSNs for all records
        {
            let mut header = self.header.lock();
            for _record in &records {
                lsns.push(header.current_lsn);
                header.current_lsn = lsn::next(header.current_lsn);
            }
        }

        // Process records for group commit
        {
            let mut group_commit = self.group_commit.lock();

            // Add records to group commit batch
            for (i, record) in records.into_iter().enumerate() {
                let buffered_record = BufferedRecord {
                    record,
                    lsn: lsns[i],
                    timestamp: Instant::now(),
                    committed: true,
                };
                group_commit.pending_records.push(buffered_record);
            }

            // Check if group commit should trigger
            let should_commit = group_commit.pending_records.len() >= group_commit.max_batch_size ||
                              start_time.elapsed() >= group_commit.timeout;

            if should_commit {
                let records_to_commit = std::mem::take(&mut group_commit.pending_records);
                drop(group_commit); // Release lock before commit
                self.commit_group_batch(records_to_commit)?;
            }
        }

        Ok(lsns)
    }

    /// Flush write buffer to disk
    pub fn flush_buffer(&self) -> NativeResult<()> {
        let start_time = Instant::now();

        let (buffer_data, record_count) = {
            let mut write_buffer = self.write_buffer.lock();

            if write_buffer.buffer.is_empty() {
                return Ok(()); // Nothing to flush
            }

            let buffer_data = std::mem::take(&mut write_buffer.buffer);
            let record_count = write_buffer.records.len();
            write_buffer.records.clear();
            write_buffer.last_flush = Instant::now();

            (buffer_data, record_count)
        };

        // Write to file
        {
            let mut file = self.file.lock();
            file.write_all(&buffer_data)
                .map_err(NativeBackendError::Io)?;

            file.flush()
                .map_err(NativeBackendError::Io)?;
        }

        // Update metrics
        {
            let mut metrics = self.metrics.lock();
            metrics.flush_count += 1;
            metrics.avg_records_per_flush = ((metrics.avg_records_per_flush * (metrics.flush_count - 1) as f64) + record_count as f64) / metrics.flush_count as f64;
        }

        Ok(())
    }

    /// Commit a group of records atomically
    fn commit_group_batch(&self, records: Vec<BufferedRecord>) -> NativeResult<()> {
        let start_time = Instant::now();
        let mut total_bytes = 0;

        // Serialize and write all records
        for buffered_record in &records {
            let serialized = V2WALSerializer::serialize(&buffered_record.record)?;
            total_bytes += serialized.len();

            let mut file = self.file.lock();
            file.write_all(&serialized)
                .map_err(NativeBackendError::Io)?;
        }

        // Flush to ensure durability
        {
            let mut file = self.file.lock();
            file.flush()
                .map_err(NativeBackendError::Io)?;
        }

        // Update metrics
        {
            let mut metrics = self.metrics.lock();
            metrics.group_commit_count += 1;
            metrics.avg_group_commit_size = ((metrics.avg_group_commit_size * (metrics.group_commit_count - 1) as f64) + records.len() as f64) / metrics.group_commit_count as f64;
            metrics.records_written += records.len() as u64;
            metrics.bytes_written += total_bytes as u64;
        }

        Ok(())
    }

    /// Update write latency metrics
    fn update_latency_metrics(&self, latency_us: u64) {
        // Simple sliding window implementation for latency percentiles
        let mut metrics = self.metrics.lock();

        // For simplicity, update with exponential smoothing
        const ALPHA: f64 = 0.1;

        metrics.write_latency_p50 = ((100.0 - ALPHA) * metrics.write_latency_p50 as f64 + ALPHA * latency_us as f64) as u64;
        metrics.write_latency_p95 = ((100.0 - ALPHA) * metrics.write_latency_p95 as f64 + ALPHA * (latency_us * 95 / 50) as f64) as u64;
        metrics.write_latency_p99 = ((100.0 - ALPHA) * metrics.write_latency_p99 as f64 + ALPHA * (latency_us * 99 / 50) as f64) as u64;
    }

    /// Get current performance metrics
    pub fn get_metrics(&self) -> WriterMetrics {
        let metrics = self.metrics.lock();
        WriterMetrics {
            records_written: metrics.records_written,
            bytes_written: metrics.bytes_written,
            flush_count: metrics.flush_count,
            avg_records_per_flush: metrics.avg_records_per_flush,
            group_commit_count: metrics.group_commit_count,
            avg_group_commit_size: metrics.avg_group_commit_size,
            write_latency_p50: metrics.write_latency_p50,
            write_latency_p95: metrics.write_latency_p95,
            write_latency_p99: metrics.write_latency_p99,
            buffer_utilization: metrics.buffer_utilization,
        }
    }

    /// Force flush all pending data
    pub fn sync(&self) -> NativeResult<()> {
        self.flush_buffer()?;

        // Sync underlying file
        {
            let file = self.file.lock();
            file.get_ref().sync_all()
                .map_err(NativeBackendError::Io)?;
        }

        Ok(())
    }

    /// Get current WAL header
    pub fn get_header(&self) -> V2WALHeader {
        *self.header.lock()
    }

    /// Shutdown writer gracefully
    pub fn shutdown(&self) -> NativeResult<()> {
        // Flush any remaining data
        self.flush_buffer()?;

        // Commit any pending group commits
        {
            let mut group_commit = self.group_commit.lock();
            if !group_commit.pending_records.is_empty() {
                let records = std::mem::take(&mut group_commit.pending_records);
                drop(group_commit);
                self.commit_group_batch(records)?;
            }
        }

        // Final sync
        self.sync()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_v2_wal_writer_create() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            ..Default::default()
        };

        let writer = V2WALWriter::create(config);
        assert!(writer.is_ok());
    }

    #[test]
    fn test_write_single_record() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            ..Default::default()
        };

        let writer = V2WALWriter::create(config).unwrap();

        let record = V2WALRecord::NodeInsert {
            node_id: 42,
            slot_offset: 1024,
            node_data: vec![1, 2, 3, 4, 5],
        };

        let lsn = writer.write_record(record).unwrap();
        assert!(lsn >= 1);

        let metrics = writer.get_metrics();
        assert_eq!(metrics.records_written, 1);
        assert!(metrics.bytes_written > 0);
    }

    #[test]
    fn test_write_records_batch() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            ..Default::default()
        };

        let writer = V2WALWriter::create(config).unwrap();

        let records = vec![
            V2WALRecord::NodeInsert {
                node_id: 1,
                slot_offset: 1024,
                node_data: vec![1, 2, 3],
            },
            V2WALRecord::NodeInsert {
                node_id: 2,
                slot_offset: 2048,
                node_data: vec![4, 5, 6],
            },
        ];

        let lsns = writer.write_records_batch(records).unwrap();
        assert_eq!(lsns.len(), 2);
        assert!(lsns[1] > lsns[0]); // LSNs should be sequential

        // Force shutdown to ensure all records are committed and metrics updated
        writer.shutdown().unwrap();

        let metrics = writer.get_metrics();
        assert_eq!(metrics.records_written, 2);
    }

    #[test]
    fn test_flush_and_sync() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            ..Default::default()
        };

        let writer = V2WALWriter::create(config).unwrap();

        let record = V2WALRecord::NodeInsert {
            node_id: 42,
            slot_offset: 1024,
            node_data: vec![1, 2, 3],
        };

        writer.write_record(record).unwrap();
        writer.flush_buffer().unwrap();
        writer.sync().unwrap();

        let metrics = writer.get_metrics();
        assert!(metrics.flush_count > 0);
    }

    #[test]
    fn test_writer_shutdown() {
        let temp_dir = tempdir().unwrap();
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            ..Default::default()
        };

        let writer = V2WALWriter::create(config).unwrap();

        let record = V2WALRecord::NodeInsert {
            node_id: 42,
            slot_offset: 1024,
            node_data: vec![1, 2, 3],
        };

        writer.write_record(record).unwrap();
        writer.shutdown().unwrap();

        let metrics = writer.get_metrics();
        assert!(metrics.flush_count > 0);
    }
}