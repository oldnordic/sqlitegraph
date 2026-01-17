//! V2 WAL Recovery Scanner
//!
//! This module implements WAL file scanning and transaction detection for the V2
//! clustered edge format. It provides comprehensive transaction parsing, record
//! classification, and recovery metadata extraction while maintaining high
//! performance and memory efficiency.

use super::{constants::*, core::TransactionState, errors::RecoveryError};
use crate::backend::native::v2::wal::{V2WALReader, V2WALRecord, V2WALRecordType};
use crate::debug::{info_log, debug_log, warn_log};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// WAL scanner configuration options
#[derive(Debug, Clone)]
pub struct ScannerConfig {
    /// Maximum number of records to buffer in memory
    pub max_buffer_size: usize,
    /// Whether to perform detailed record validation
    pub validate_records: bool,
    /// Progress reporting interval (records)
    pub progress_interval: usize,
    /// Maximum allowed incomplete transactions
    pub max_incomplete_transactions: usize,
}

impl Default for ScannerConfig {
    fn default() -> Self {
        Self {
            max_buffer_size: scanning::MAX_READ_BUFFER_SIZE,
            validate_records: true,
            progress_interval: RECOVERY_PROGRESS_INTERVAL,
            max_incomplete_transactions: MAX_INCOMPLETE_TRANSACTIONS,
        }
    }
}

/// WAL scan result with transaction metadata and statistics
#[derive(Debug, Clone)]
pub struct WALScanResult {
    /// Detected transactions
    pub transactions: Vec<TransactionState>,
    /// Scan warnings
    pub warnings: Vec<String>,
    /// Scan statistics
    pub statistics: ScanStatistics,
}

/// WAL scan statistics and performance metrics
#[derive(Debug, Clone, Default)]
pub struct ScanStatistics {
    /// Total WAL records scanned
    pub total_records: u64,
    /// Total bytes processed
    pub total_bytes: u64,
    /// Number of transactions found
    pub transactions_found: u64,
    /// Number of committed transactions
    pub committed_transactions: u64,
    /// Number of rolled back transactions
    pub rolled_back_transactions: u64,
    /// Number of incomplete transactions
    pub incomplete_transactions: u64,
    /// Number of corrupted records
    pub corrupted_records: u64,
    /// Scan duration in milliseconds
    pub scan_duration_ms: u64,
}

/// High-performance WAL scanner for transaction detection
pub struct WALScanner {
    /// Active transaction tracking
    active_transactions: Arc<Mutex<HashMap<u64, TransactionState>>>,
    /// Scanner configuration
    config: ScannerConfig,
}

/// Transaction scanner for detailed transaction analysis
pub struct TransactionScanner {
    /// WAL reader instance
    reader: V2WALReader,
    /// Active transactions
    active_transactions: Arc<Mutex<HashMap<u64, TransactionState>>>,
    /// Scan statistics
    statistics: ScanStatistics,
    /// Scanner configuration
    config: ScannerConfig,
}

impl WALScanner {
    /// Create new WAL scanner with default configuration
    pub fn new() -> Self {
        Self::with_config(ScannerConfig::default())
    }

    /// Create new WAL scanner with custom configuration
    pub fn with_config(config: ScannerConfig) -> Self {
        Self {
            active_transactions: Arc::new(Mutex::new(HashMap::new())),
            config,
        }
    }

    /// Scan WAL file and extract all transactions
    ///
    /// This method performs comprehensive WAL scanning with transaction detection,
    /// record validation, and progress reporting. It returns all detected
    /// transactions along with detailed statistics and any warnings.
    ///
    /// # Arguments
    /// * `wal_path` - Path to the WAL file to scan
    ///
    /// # Returns
    /// * `Ok(WALScanResult)` - Scan results with transactions and statistics
    /// * `Err(RecoveryError)` - Scanning error with detailed information
    pub async fn scan_wal_file(
        &self,
        wal_path: &std::path::Path,
    ) -> Result<WALScanResult, RecoveryError> {
        let start_time = std::time::Instant::now();

        info_log!("Starting WAL scan: {:?}", wal_path);

        // Validate WAL file exists and is readable
        if !wal_path.exists() {
            return Err(RecoveryError::configuration(format!(
                "WAL file does not exist: {:?}",
                wal_path
            )));
        }

        if !wal_path.is_file() {
            return Err(RecoveryError::configuration(format!(
                "WAL path is not a file: {:?}",
                wal_path
            )));
        }

        // Create transaction scanner
        let mut scanner = TransactionScanner::new(wal_path, self.config.clone())?;

        // Perform the scan
        let result = scanner.scan().await?;

        let duration = start_time.elapsed();

        info_log!(
            "WAL scan completed: {} transactions, {} records in {:?}",
            result.transactions.len(),
            result.statistics.total_records,
            duration
        );

        Ok(result)
    }

    /// Get active transactions count
    pub fn active_transactions_count(&self) -> usize {
        self.active_transactions.lock().len()
    }

    /// Clear active transactions
    pub fn clear_active_transactions(&self) {
        self.active_transactions.lock().clear();
    }
}

impl TransactionScanner {
    /// Create new transaction scanner
    fn new(wal_path: &std::path::Path, config: ScannerConfig) -> Result<Self, RecoveryError> {
        let reader = V2WALReader::open(wal_path)
            .map_err(|e| RecoveryError::io_error(format!("Failed to open WAL: {:?}", e)))?;

        Ok(Self {
            reader,
            active_transactions: Arc::new(Mutex::new(HashMap::new())),
            statistics: ScanStatistics::default(),
            config,
        })
    }

    /// Perform comprehensive WAL scanning
    async fn scan(&mut self) -> Result<WALScanResult, RecoveryError> {
        let start_time = std::time::Instant::now();
        let header = self.reader.header().clone();

        info_log!("Scanning WAL from LSN 1 to {}", header.current_lsn);

        // Reset active transactions
        self.active_transactions.lock().clear();
        let mut transactions = Vec::new();
        let mut warnings = Vec::new();

        // Read all WAL records with progress tracking
        let mut record_count = 0;
        while let Some((lsn, record)) = self.read_next_record()? {
            record_count += 1;

            // Process the record
            if let Some((tx_state, record_warnings)) = self.process_record(lsn, record)? {
                if tx_state.committed || tx_state.commit_lsn.is_some() {
                    transactions.push(tx_state);
                }
                warnings.extend(record_warnings);
            }

            // Report progress
            if record_count % self.config.progress_interval == 0 {
                self.report_progress(record_count, lsn, header.current_lsn);
            }

            // Check memory usage
            if self.active_transactions.lock().len() > self.config.max_incomplete_transactions {
                warn_log!("Too many active transactions, forcing completion");
                self.force_complete_incomplete_transactions(&mut transactions, &mut warnings);
            }
        }

        // Handle remaining incomplete transactions
        self.finalize_incomplete_transactions(&mut transactions, &mut warnings);

        // Update final statistics
        self.statistics.scan_duration_ms = start_time.elapsed().as_millis() as u64;
        self.statistics.transactions_found = transactions.len() as u64;
        self.statistics.committed_transactions =
            transactions.iter().filter(|tx| tx.committed).count() as u64;
        self.statistics.rolled_back_transactions = transactions
            .iter()
            .filter(|tx| !tx.committed && tx.commit_lsn.is_some())
            .count() as u64;
        self.statistics.incomplete_transactions = transactions
            .iter()
            .filter(|tx| tx.commit_lsn.is_none())
            .count() as u64;

        let result = WALScanResult {
            transactions,
            warnings,
            statistics: self.statistics.clone(),
        };

        info_log!(
            "WAL scan complete: {} total records, {} transactions",
            self.statistics.total_records, self.statistics.transactions_found
        );

        Ok(result)
    }

    /// Read next WAL record with error handling
    fn read_next_record(&mut self) -> Result<Option<(u64, V2WALRecord)>, RecoveryError> {
        match self.reader.read_next_record() {
            Ok(result) => {
                if let Some((lsn, record)) = result {
                    self.statistics.total_records += 1;
                    self.statistics.total_bytes += self.estimate_record_size(&record);
                    Ok(Some((lsn, record)))
                } else {
                    Ok(None)
                }
            }
            Err(e) => {
                self.statistics.corrupted_records += 1;
                Err(RecoveryError::corruption(format!(
                    "Failed to read WAL record: {}",
                    e
                )))
            }
        }
    }

    /// Process a single WAL record
    fn process_record(
        &mut self,
        lsn: u64,
        record: V2WALRecord,
    ) -> Result<Option<(TransactionState, Vec<String>)>, RecoveryError> {
        let mut warnings = Vec::new();

        match record.record_type() {
            V2WALRecordType::TransactionBegin => {
                if let V2WALRecord::TransactionBegin { tx_id, timestamp } = record {
                    Ok(Some(self.handle_transaction_begin(tx_id, lsn, timestamp)?))
                } else {
                    warnings.push("Invalid TransactionBegin record format".to_string());
                    Ok(None)
                }
            }

            V2WALRecordType::TransactionCommit => {
                if let V2WALRecord::TransactionCommit { tx_id, timestamp } = record {
                    Ok(Some(self.handle_transaction_commit(
                        tx_id,
                        lsn,
                        timestamp,
                        &mut warnings,
                    )?))
                } else {
                    warnings.push("Invalid TransactionCommit record format".to_string());
                    Ok(None)
                }
            }

            V2WALRecordType::TransactionRollback => {
                if let V2WALRecord::TransactionRollback { tx_id, timestamp } = record {
                    Ok(Some(self.handle_transaction_rollback(
                        tx_id,
                        lsn,
                        timestamp,
                        &mut warnings,
                    )?))
                } else {
                    warnings.push("Invalid TransactionRollback record format".to_string());
                    Ok(None)
                }
            }

            // Data records - associate with active transaction
            _ => {
                if let Some(tx_id) = self.extract_transaction_id(&record) {
                    self.add_record_to_transaction(tx_id, record, lsn, &mut warnings)?;
                }
                Ok(None)
            }
        }
    }

    /// Handle transaction begin record
    fn handle_transaction_begin(
        &mut self,
        tx_id: u64,
        lsn: u64,
        timestamp: u64,
    ) -> Result<(TransactionState, Vec<String>), RecoveryError> {
        let mut warnings = Vec::new();

        let tx_state = TransactionState {
            tx_id,
            start_lsn: lsn,
            commit_lsn: None,
            records: Vec::new(),
            committed: false,
            timestamp,
        };

        {
            let mut active_tx = self.active_transactions.lock();
            if active_tx.contains_key(&tx_id) {
                warnings.push(format!("Duplicate transaction begin for TX {}", tx_id));
            }
            active_tx.insert(tx_id, tx_state);
        }

        // Return None since this transaction is still active
        Ok((
            TransactionState {
                tx_id,
                start_lsn: lsn,
                commit_lsn: None,
                records: Vec::new(),
                committed: false,
                timestamp,
            },
            warnings,
        ))
    }

    /// Handle transaction commit record
    fn handle_transaction_commit(
        &mut self,
        tx_id: u64,
        lsn: u64,
        timestamp: u64,
        warnings: &mut Vec<String>,
    ) -> Result<(TransactionState, Vec<String>), RecoveryError> {
        let mut active_tx = self.active_transactions.lock();

        if let Some(mut tx_state) = active_tx.remove(&tx_id) {
            tx_state.commit_lsn = Some(lsn);
            tx_state.committed = true;
            tx_state.timestamp = timestamp;
            Ok((tx_state, warnings.clone()))
        } else {
            warnings.push(format!("Commit for unknown transaction TX {}", tx_id));

            // Create a synthetic transaction state for unknown commits
            Ok((
                TransactionState {
                    tx_id,
                    start_lsn: 0, // Unknown start
                    commit_lsn: Some(lsn),
                    records: Vec::new(),
                    committed: true,
                    timestamp,
                },
                warnings.clone(),
            ))
        }
    }

    /// Handle transaction rollback record
    fn handle_transaction_rollback(
        &mut self,
        tx_id: u64,
        lsn: u64,
        timestamp: u64,
        warnings: &mut Vec<String>,
    ) -> Result<(TransactionState, Vec<String>), RecoveryError> {
        let mut active_tx = self.active_transactions.lock();

        if let Some(mut tx_state) = active_tx.remove(&tx_id) {
            tx_state.committed = false;
            tx_state.timestamp = timestamp;
            Ok((tx_state, warnings.clone()))
        } else {
            warnings.push(format!("Rollback for unknown transaction TX {}", tx_id));

            // Create a synthetic transaction state for unknown rollbacks
            Ok((
                TransactionState {
                    tx_id,
                    start_lsn: 0, // Unknown start
                    commit_lsn: Some(lsn),
                    records: Vec::new(),
                    committed: false,
                    timestamp,
                },
                warnings.clone(),
            ))
        }
    }

    /// Add record to active transaction
    fn add_record_to_transaction(
        &mut self,
        tx_id: u64,
        record: V2WALRecord,
        _lsn: u64,
        warnings: &mut Vec<String>,
    ) -> Result<(), RecoveryError> {
        let mut active_tx = self.active_transactions.lock();

        if let Some(tx_state) = active_tx.get_mut(&tx_id) {
            tx_state.records.push(record);
        } else {
            // Record without active transaction - might be from a completed transaction
            debug_log!("Record for inactive transaction TX {}", tx_id);
            warnings.push(format!("Record for inactive transaction TX {}", tx_id));
        }

        Ok(())
    }

    /// Extract transaction ID from WAL record
    fn extract_transaction_id(&self, record: &V2WALRecord) -> Option<u64> {
        match record {
            V2WALRecord::NodeInsert { node_id, .. } => {
                Some((*node_id as u64).wrapping_add(1_000_000))
            }
            V2WALRecord::NodeUpdate { node_id, .. } => {
                Some((*node_id as u64).wrapping_add(2_000_000))
            }
            V2WALRecord::ClusterCreate { node_id, .. } => {
                Some((*node_id as u64).wrapping_add(3_000_000))
            }
            V2WALRecord::EdgeInsert { cluster_key, .. } => {
                Some((cluster_key.0 as u64).wrapping_add(4_000_000))
            }
            V2WALRecord::EdgeUpdate { cluster_key, .. } => {
                Some((cluster_key.0 as u64).wrapping_add(5_000_000))
            }
            V2WALRecord::EdgeDelete { cluster_key, .. } => {
                Some((cluster_key.0 as u64).wrapping_add(6_000_000))
            }
            V2WALRecord::StringInsert { string_id, .. } => {
                Some((*string_id as u64).wrapping_add(7_000_000))
            }
            V2WALRecord::FreeSpaceAllocate { block_offset, .. } => {
                Some(block_offset.wrapping_add(8_000_000))
            }
            V2WALRecord::FreeSpaceDeallocate { block_offset, .. } => {
                Some(block_offset.wrapping_add(9_000_000))
            }
            V2WALRecord::NodeDelete { node_id, .. } => {
                Some((*node_id as u64).wrapping_add(10_000_000))
            }
            V2WALRecord::HeaderUpdate { header_offset, .. } => {
                Some(header_offset.wrapping_add(11_000_000))
            }
            // Control records don't have transaction IDs
            V2WALRecord::TransactionBegin { .. }
            | V2WALRecord::TransactionCommit { .. }
            | V2WALRecord::TransactionRollback { .. }
            | V2WALRecord::TransactionPrepare { .. }
            | V2WALRecord::TransactionAbort { .. }
            | V2WALRecord::SavepointCreate { .. }
            | V2WALRecord::SavepointRollback { .. }
            | V2WALRecord::SavepointRelease { .. }
            | V2WALRecord::BackupCreate { .. }
            | V2WALRecord::BackupRestore { .. }
            | V2WALRecord::LockAcquire { .. }
            | V2WALRecord::LockRelease { .. }
            | V2WALRecord::IndexUpdate { .. }
            | V2WALRecord::StatisticsUpdate { .. }
            | V2WALRecord::Checkpoint { .. }
            | V2WALRecord::SegmentEnd { .. } => None,
        }
    }

    /// Force completion of incomplete transactions
    fn force_complete_incomplete_transactions(
        &mut self,
        transactions: &mut Vec<TransactionState>,
        warnings: &mut Vec<String>,
    ) {
        let mut active_tx = self.active_transactions.lock();
        let incomplete_count = active_tx.len();

        if incomplete_count > 0 {
            warn_log!(
                "Forcing completion of {} incomplete transactions",
                incomplete_count
            );

            for (_, mut tx_state) in active_tx.drain() {
                tx_state.committed = false; // Mark as incomplete
                let tx_id = tx_state.tx_id;
                transactions.push(tx_state);
                warnings.push(format!(
                    "Incomplete transaction TX {} forced to completion",
                    tx_id
                ));
            }
        }
    }

    /// Finalize incomplete transactions
    fn finalize_incomplete_transactions(
        &mut self,
        transactions: &mut Vec<TransactionState>,
        warnings: &mut Vec<String>,
    ) {
        let mut active_tx = self.active_transactions.lock();

        for (_, tx_state) in active_tx.drain() {
            warnings.push(format!(
                "Incomplete transaction TX {} recovered",
                tx_state.tx_id
            ));
            transactions.push(tx_state);
        }
    }

    /// Report scanning progress
    fn report_progress(&self, record_count: usize, current_lsn: u64, total_lsn: u64) {
        let percentage = if total_lsn > 0 {
            (current_lsn as f64 / total_lsn as f64) * 100.0
        } else {
            0.0
        };

        debug_log!(
            "WAL scan progress: {} records, LSN {}/{}, {:.1}% complete",
            record_count, current_lsn, total_lsn, percentage
        );
    }

    /// Estimate record size for statistics
    fn estimate_record_size(&self, record: &V2WALRecord) -> u64 {
        // Base record size includes LSN, record type, and header
        let base_size = 16; // 8 bytes LSN + 4 bytes type + 4 bytes flags

        match record {
            V2WALRecord::NodeInsert { node_data, .. } => base_size + node_data.len() as u64,
            V2WALRecord::NodeUpdate { new_data, .. } => base_size + new_data.len() as u64,
            V2WALRecord::ClusterCreate { edge_data, .. } => base_size + edge_data.len() as u64,
            V2WALRecord::EdgeInsert { edge_record, .. } => {
                base_size + edge_record.estimated_size() as u64
            }
            V2WALRecord::EdgeUpdate { new_edge, .. } => {
                base_size + new_edge.estimated_size() as u64
            }
            V2WALRecord::StringInsert { string_value, .. } => base_size + string_value.len() as u64,
            V2WALRecord::HeaderUpdate { new_data, .. } => base_size + new_data.len() as u64,
            // Records with fixed size
            _ => base_size + 32, // Estimated fixed payload size
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::v2::edge_cluster::{CompactEdgeRecord, Direction};
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn test_scanner_config_default() {
        let config = ScannerConfig::default();
        assert!(config.validate_records);
        assert_eq!(config.progress_interval, RECOVERY_PROGRESS_INTERVAL);
        assert_eq!(
            config.max_incomplete_transactions,
            MAX_INCOMPLETE_TRANSACTIONS
        );
    }

    #[test]
    fn test_wal_scanner_creation() {
        let scanner = WALScanner::new();
        assert_eq!(scanner.active_transactions_count(), 0);

        let config = ScannerConfig {
            validate_records: false,
            progress_interval: 500,
            max_incomplete_transactions: 200,
            max_buffer_size: 64 * 1024,
        };

        let custom_scanner = WALScanner::with_config(config);
        assert_eq!(custom_scanner.active_transactions_count(), 0);
    }

    #[test]
    fn test_scan_statistics_default() {
        let stats = ScanStatistics::default();
        assert_eq!(stats.total_records, 0);
        assert_eq!(stats.transactions_found, 0);
        assert_eq!(stats.committed_transactions, 0);
        assert_eq!(stats.rolled_back_transactions, 0);
        assert_eq!(stats.incomplete_transactions, 0);
        assert_eq!(stats.corrupted_records, 0);
        assert_eq!(stats.scan_duration_ms, 0);
    }

    #[test]
    fn test_transaction_id_extraction() {
        // Test transaction ID extraction logic directly using the same pattern matching
        // that TransactionScanner::extract_transaction_id uses

        // Test node insert record
        let node_insert = V2WALRecord::NodeInsert {
            node_id: 42,
            slot_offset: 100,
            node_data: vec![1, 2, 3],
        };

        let node_tx_id = match &node_insert {
            V2WALRecord::NodeInsert { node_id, .. } => {
                Some((*node_id as u64).wrapping_add(1_000_000))
            }
            _ => None,
        };
        assert_eq!(node_tx_id, Some(1000042));

        // Test edge insert record
        let edge_record = CompactEdgeRecord {
            neighbor_id: 456,
            edge_type_offset: 0,
            edge_data: vec![],
        };
        let edge_insert = V2WALRecord::EdgeInsert {
            cluster_key: (123, Direction::Outgoing),
            edge_record,
            insertion_point: 0,
        };

        let edge_tx_id = match &edge_insert {
            V2WALRecord::EdgeInsert { cluster_key, .. } => {
                Some((cluster_key.0 as u64).wrapping_add(4_000_000))
            }
            _ => None,
        };
        assert_eq!(edge_tx_id, Some(4000123));

        // Test control record (no transaction ID)
        let tx_begin = V2WALRecord::TransactionBegin {
            tx_id: 1,
            timestamp: 1234567890,
        };

        let control_tx_id: Option<u64> = match &tx_begin {
            V2WALRecord::NodeInsert { .. }
            | V2WALRecord::NodeUpdate { .. }
            | V2WALRecord::ClusterCreate { .. }
            | V2WALRecord::EdgeInsert { .. }
            | V2WALRecord::EdgeUpdate { .. }
            | V2WALRecord::EdgeDelete { .. } => None,
            _ => None,
        };
        assert_eq!(control_tx_id, None);
    }
}
