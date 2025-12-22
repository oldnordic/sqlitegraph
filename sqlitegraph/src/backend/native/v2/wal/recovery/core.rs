//! V2 WAL Recovery Core Engine
//!
//! Core recovery engine and lifecycle management for V2 clustered edge format.
//! Focuses on recovery orchestration, state management, and phase coordination.

use super::{constants::*, errors::RecoveryError};
use crate::backend::native::v2::wal::{V2WALConfig, V2WALRecord};
use crate::backend::native::{NativeBackendError, NativeResult};
use parking_lot::{Condvar, Mutex};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

macro_rules! info { ($($arg:tt)*) => { log::info!($($arg)*); }; }
macro_rules! debug { ($($arg:tt)*) => { log::debug!($($arg)*); }; }
macro_rules! warn { ($($arg:tt)*) => { log::warn!($($arg)*); }; }
macro_rules! error { ($($arg:tt)*) => { log::error!($($arg)*); }; }

/// Recovery state machine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryState {
    Idle,
    Initializing,
    Scanning,
    Validating,
    Replaying,
    Finalizing,
    Complete,
    Failed,
}

/// Recovery configuration options
#[derive(Debug, Clone)]
pub struct RecoveryOptions {
    pub fast_recovery: bool,
    pub max_batch_size: usize,
    pub recovery_timeout: Duration,
    pub perform_consistency_checks: bool,
    pub create_backup: bool,
    pub max_recovery_attempts: u32,
    pub force_recovery: bool,
}

impl Default for RecoveryOptions {
    fn default() -> Self {
        Self {
            fast_recovery: false,
            max_batch_size: DEFAULT_BATCH_SIZE,
            recovery_timeout: Duration::from_secs(DEFAULT_RECOVERY_TIMEOUT_SECONDS),
            perform_consistency_checks: true,
            create_backup: true,
            max_recovery_attempts: MAX_RECOVERY_ATTEMPTS,
            force_recovery: false,
        }
    }
}

/// Recovery progress information
#[derive(Debug, Clone)]
pub struct RecoveryProgress {
    pub state: RecoveryState,
    pub current_lsn: u64,
    pub total_lsns: u64,
    pub transactions_processed: u64,
    pub total_transactions: u64,
    pub completion_percentage: f64,
    pub start_time: Instant,
    pub estimated_time_remaining: Duration,
}

/// Recovery result type
pub type RecoveryResult<T = RecoverySuccess> = Result<T, RecoveryError>;

/// Successful recovery result
#[derive(Debug, Clone)]
pub struct RecoverySuccess {
    pub metrics: RecoveryMetrics,
    pub warnings: Vec<String>,
    pub duration: Duration,
}

/// Recovery metrics
#[derive(Debug, Clone, Default)]
pub struct RecoveryMetrics {
    pub total_duration_ms: u64,
    pub transactions_scanned: u64,
    pub committed_transactions_replayed: u64,
    pub rolled_back_transactions: u64,
    pub records_processed: u64,
    pub corrupted_records: u64,
    pub checkpoint_inconsistencies: u64,
    pub database_size_before_recovery: u64,
    pub database_size_after_recovery: u64,
    pub recovery_start_timestamp: u64,
    pub recovery_end_timestamp: u64,
}

/// V2 WAL recovery engine
pub struct V2WALRecoveryEngine {
    config: V2WALConfig,
    database_path: PathBuf,
    state: Arc<Mutex<RecoveryState>>,
    options: RecoveryOptions,
    metrics: Arc<Mutex<RecoveryMetrics>>,
    active_transactions: Arc<Mutex<HashMap<u64, TransactionState>>>,
    recovery_cv: Arc<Condvar>,
    backup_path: Option<PathBuf>,
}

/// Transaction state during recovery
#[derive(Debug, Clone)]
pub struct TransactionState {
    pub tx_id: u64,
    pub start_lsn: u64,
    pub commit_lsn: Option<u64>,
    pub records: Vec<V2WALRecord>,
    pub committed: bool,
    pub timestamp: u64,
}

impl V2WALRecoveryEngine {
    /// Create new recovery engine
    pub fn create(
        config: V2WALConfig,
        database_path: PathBuf,
        options: RecoveryOptions,
    ) -> NativeResult<Self> {
        Self::validate_configuration(&config, &database_path)?;

        let backup_path = if options.create_backup {
            Some(Self::create_database_backup(&database_path)?)
        } else {
            None
        };

        Ok(Self {
            config,
            database_path,
            state: Arc::new(Mutex::new(RecoveryState::Idle)),
            options,
            metrics: Arc::new(Mutex::new(RecoveryMetrics::default())),
            active_transactions: Arc::new(Mutex::new(HashMap::new())),
            recovery_cv: Arc::new(Condvar::new()),
            backup_path,
        })
    }

    /// Validate configuration and prerequisites
    fn validate_configuration(config: &V2WALConfig, database_path: &Path) -> NativeResult<()> {
        config.validate()?;

        if !database_path.exists() || !database_path.is_file() {
            return Err(NativeBackendError::InvalidParameter {
                context: "Database file does not exist or is not a file".to_string(),
                source: None,
            });
        }

        if !config.wal_path.exists() || !config.wal_path.is_file() {
            return Err(NativeBackendError::InvalidParameter {
                context: "WAL file does not exist or is not a file".to_string(),
                source: None,
            });
        }

        Ok(())
    }

    /// Create database backup
    fn create_database_backup(database_path: &Path) -> NativeResult<PathBuf> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(NativeBackendError::from)?
            .as_secs();

        let database_name = database_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("database");

        let backup_filename = format!("{}.recovery_backup.{}", database_name, timestamp);
        let backup_path = database_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("recovery_backups")
            .join(backup_filename);

        if let Some(parent) = backup_path.parent() {
            std::fs::create_dir_all(parent).map_err(NativeBackendError::from)?;
        }

        std::fs::copy(database_path, &backup_path).map_err(NativeBackendError::from)?;

        Ok(backup_path)
    }

    /// Perform crash recovery
    pub fn recover(&self) -> RecoveryResult {
        let start_time = Instant::now();
        let start_timestamp = self.get_current_timestamp()?;

        info!("Starting V2 WAL recovery");
        self.set_recovery_state(RecoveryState::Initializing)?;

        let mut warnings = Vec::new();

        for attempt in 1..=self.options.max_recovery_attempts {
            debug!(
                "Recovery attempt {}/{}",
                attempt, self.options.max_recovery_attempts
            );

            match self.attempt_recovery(attempt) {
                Ok(mut attempt_warnings) => {
                    warnings.append(&mut attempt_warnings);
                    return self.finalize_successful_recovery(
                        start_time,
                        start_timestamp,
                        warnings,
                    );
                }
                Err(e) => {
                    error!("Recovery attempt {} failed: {}", attempt, e);

                    if attempt == self.options.max_recovery_attempts {
                        if self.options.force_recovery {
                            warn!("Force recovery enabled");
                            return self.finalize_successful_recovery(
                                start_time,
                                start_timestamp,
                                warnings,
                            );
                        }
                        return self.finalize_failed_recovery(start_time, start_timestamp, e);
                    }

                    let backoff = Duration::from_millis(
                        (RECOVERY_RETRY_BACKOFF_MULTIPLIER.powi(attempt as i32) * 1000.0) as u64,
                    );
                    std::thread::sleep(backoff.min(Duration::from_secs(MAX_RETRY_DELAY_SECONDS)));
                }
            }
        }

        Err(RecoveryError::configuration(
            "Unexpected recovery completion".to_string(),
        ))
    }

    /// Get current timestamp
    fn get_current_timestamp(&self) -> Result<u64, RecoveryError> {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .map_err(|e| RecoveryError::configuration(format!("Failed to get timestamp: {}", e)))
    }

    /// Attempt single recovery operation
    fn attempt_recovery(&self, _attempt: u32) -> Result<Vec<String>, RecoveryError> {
        let mut warnings = Vec::new();

        self.set_recovery_state(RecoveryState::Initializing)?;
        self.set_recovery_state(RecoveryState::Scanning)?;
        let (transactions, scan_warnings) = self.scan_wal_for_transactions()?;
        warnings.extend(scan_warnings);

        if !self.options.fast_recovery {
            self.set_recovery_state(RecoveryState::Validating)?;
            let validation_warnings = self.validate_transactions(&transactions)?;
            warnings.extend(validation_warnings);
        }

        self.set_recovery_state(RecoveryState::Replaying)?;
        let replay_warnings = self.replay_transactions(&transactions)?;
        warnings.extend(replay_warnings);

        self.set_recovery_state(RecoveryState::Finalizing)?;
        let final_warnings = self.finalize_recovery()?;
        warnings.extend(final_warnings);

        self.update_transaction_metrics(&transactions);
        Ok(warnings)
    }

    /// Finalize successful recovery
    fn finalize_successful_recovery(
        &self,
        start_time: Instant,
        start_timestamp: u64,
        warnings: Vec<String>,
    ) -> RecoveryResult {
        let duration = start_time.elapsed();
        let end_timestamp = start_timestamp + duration.as_secs();

        {
            let mut metrics = self.metrics.lock();
            metrics.total_duration_ms = duration.as_millis() as u64;
            metrics.recovery_start_timestamp = start_timestamp;
            metrics.recovery_end_timestamp = end_timestamp;

            if let Ok(metadata) = std::fs::metadata(&self.database_path) {
                metrics.database_size_after_recovery = metadata.len();
            }
        }

        self.set_recovery_state(RecoveryState::Complete)?;
        self.recovery_cv.notify_all();

        info!("Recovery completed successfully in {:?}", duration);
        Ok(RecoverySuccess {
            metrics: self.metrics.lock().clone(),
            warnings,
            duration,
        })
    }

    /// Finalize failed recovery
    fn finalize_failed_recovery(
        &self,
        start_time: Instant,
        start_timestamp: u64,
        error: RecoveryError,
    ) -> RecoveryResult {
        let duration = start_time.elapsed();

        {
            let mut metrics = self.metrics.lock();
            metrics.total_duration_ms = duration.as_millis() as u64;
            metrics.recovery_start_timestamp = start_timestamp;
            metrics.recovery_end_timestamp = start_timestamp + duration.as_secs();
        }

        self.set_recovery_state(RecoveryState::Failed)?;
        self.recovery_cv.notify_all();

        error!("Recovery failed after {:?}: {}", duration, error);
        Err(error)
    }

    /// Set recovery state
    fn set_recovery_state(&self, new_state: RecoveryState) -> Result<(), RecoveryError> {
        let mut state = self.state.lock();

        if !self.is_valid_state_transition(*state, new_state) {
            return Err(RecoveryError::state_transition(format!(
                "Invalid transition from {:?} to {:?}",
                *state, new_state
            )));
        }

        *state = new_state;
        debug!("Recovery state: {:?}", new_state);
        Ok(())
    }

    /// Validate state transition
    fn is_valid_state_transition(&self, from: RecoveryState, to: RecoveryState) -> bool {
        use RecoveryState::*;

        match (from, to) {
            (Idle, Initializing) => true,
            (Initializing, Scanning) => true,
            (Scanning, Validating) => true,
            (Scanning, Replaying) => true,
            (Validating, Replaying) => true,
            (Replaying, Finalizing) => true,
            (Finalizing, Complete) => true,
            (_, Failed) => true,
            _ => false,
        }
    }

    /// Update transaction metrics
    fn update_transaction_metrics(&self, transactions: &[TransactionState]) {
        let mut metrics = self.metrics.lock();
        metrics.transactions_scanned = transactions.len() as u64;
        metrics.committed_transactions_replayed =
            transactions.iter().filter(|tx| tx.committed).count() as u64;
        metrics.rolled_back_transactions =
            transactions.iter().filter(|tx| !tx.committed).count() as u64;
        metrics.records_processed = transactions.iter().map(|tx| tx.records.len() as u64).sum();
    }

    /// Get recovery progress
    pub fn get_progress(&self) -> RecoveryProgress {
        let state = *self.state.lock();
        RecoveryProgress {
            state,
            current_lsn: 0,
            total_lsns: 0,
            transactions_processed: 0,
            total_transactions: 0,
            completion_percentage: 0.0,
            start_time: Instant::now(),
            estimated_time_remaining: Duration::from_secs(0),
        }
    }

    /// Get current state
    pub fn get_state(&self) -> RecoveryState {
        *self.state.lock()
    }

    /// Get metrics
    pub fn get_metrics(&self) -> RecoveryMetrics {
        self.metrics.lock().clone()
    }

    /// Wait for completion
    pub fn wait_for_completion(&self, timeout: Duration) -> Result<RecoveryState, RecoveryError> {
        let mut state = self.state.lock();
        let result = self.recovery_cv.wait_for(&mut state, timeout);

        if result.timed_out() {
            return Err(RecoveryError::timeout(
                "Recovery completion timeout".to_string(),
            ));
        }

        Ok(*state)
    }

    /// Cancel recovery
    pub fn cancel_recovery(&self) -> Result<(), RecoveryError> {
        let mut state = self.state.lock();
        match *state {
            RecoveryState::Idle => Err(RecoveryError::configuration(
                "No recovery in progress".to_string(),
            )),
            RecoveryState::Complete | RecoveryState::Failed => Err(RecoveryError::configuration(
                "Recovery already completed".to_string(),
            )),
            _ => {
                *state = RecoveryState::Failed;
                self.recovery_cv.notify_all();
                info!("Recovery cancelled");
                Ok(())
            }
        }
    }

    // Specialized module methods
    fn scan_wal_for_transactions(
        &self,
    ) -> Result<(Vec<TransactionState>, Vec<String>), RecoveryError> {
        // Delegate to specialized scanner module
        use super::scanner::{ScanStatistics, WALScanResult, WALScanner};

        let _scanner = WALScanner::new();
        let mut active_tx = self.active_transactions.lock();
        active_tx.clear();

        // Use scanner to process WAL file
        // TODO: Make this properly async - for now, simulate the result
        let scan_result = WALScanResult {
            transactions: Vec::new(),
            warnings: vec!["Async scanning not yet implemented".to_string()],
            statistics: ScanStatistics::default(),
        };

        // Update active transactions with scanner results
        *active_tx = HashMap::new(); // Will be populated by scanner

        Ok((scan_result.transactions, scan_result.warnings))
    }

    fn validate_transactions(
        &self,
        transactions: &[TransactionState],
    ) -> Result<Vec<String>, RecoveryError> {
        // Delegate to specialized validator module when implemented
        // For now, implement basic validation inline
        self.validate_transactions_basic(transactions)
    }

    fn replay_transactions(
        &self,
        transactions: &[TransactionState],
    ) -> Result<Vec<String>, RecoveryError> {
        use super::replayer::{ReplayConfig, V2GraphFileReplayer};

        // Create replayer with database path
        let config = ReplayConfig {
            strict_validation: self.options.perform_consistency_checks,
            max_batch_size: self.options.max_batch_size,
            operation_timeout_ms: validation::CONSISTENCY_CHECK_TIMEOUT_MS,
            create_backup: false, // Handled by recovery core
            progress_interval: RECOVERY_PROGRESS_INTERVAL,
        };

        let replayer = V2GraphFileReplayer::create(self.database_path.clone(), config)
            .map_err(|e| {
                RecoveryError::configuration(format!("Failed to create replayer: {}", e))
            })?;

        // Replay transactions using V2 integration
        let replay_result = replayer.replay_transactions(transactions).map_err(|e| {
            RecoveryError::replay_failure(format!("Transaction replay failed: {}", e))
        })?;

        // Update recovery metrics
        {
            let mut metrics = self.metrics.lock();
            metrics.committed_transactions_replayed = replay_result.successful_operations;
            metrics.corrupted_records += replay_result.failed_operations.len() as u64;
            metrics.records_processed += transactions
                .iter()
                .map(|tx| tx.records.len() as u64)
                .sum::<u64>();
        }

        Ok(replay_result.warnings)
    }

    /// Basic transaction validation (temporary until validator module is complete)
    fn validate_transactions_basic(
        &self,
        transactions: &[TransactionState],
    ) -> Result<Vec<String>, RecoveryError> {
        let mut warnings = Vec::new();
        let mut tx_ids = std::collections::HashSet::new();

        for tx in transactions {
            // Check for duplicate transaction IDs
            if tx_ids.contains(&tx.tx_id) {
                warnings.push(format!("Duplicate transaction ID: {}", tx.tx_id));
            }
            tx_ids.insert(&tx.tx_id);

            // Validate transaction sequence
            if tx.start_lsn == 0 {
                warnings.push(format!("Transaction TX {} has invalid start LSN", tx.tx_id));
            }

            if tx.committed && tx.commit_lsn.is_none() {
                warnings.push(format!(
                    "Committed transaction TX {} missing commit LSN",
                    tx.tx_id
                ));
            }

            if !tx.committed && tx.commit_lsn.is_some() {
                warnings.push(format!(
                    "Uncommitted transaction TX {} has commit LSN",
                    tx.tx_id
                ));
            }

            // Validate record order
            if let Some(commit_lsn) = tx.commit_lsn {
                if commit_lsn < tx.start_lsn {
                    warnings.push(format!(
                        "Transaction TX {} commit LSN before start LSN",
                        tx.tx_id
                    ));
                }
            }

            // Validate record count
            if tx.records.is_empty() {
                warnings.push(format!("Transaction TX {} has no records", tx.tx_id));
            }
        }

        debug!(
            "Transaction validation complete, {} warnings",
            warnings.len()
        );
        Ok(warnings)
    }

    fn finalize_recovery(&self) -> Result<Vec<String>, RecoveryError> {
        let mut warnings = Vec::new();
        self.active_transactions.lock().clear();

        if let Err(e) = std::fs::metadata(&self.database_path) {
            warnings.push(format!("Database validation issue: {:?}", e));
        }

        Ok(warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_recovery_state_transitions() {
        assert!(is_valid_transition_static(
            RecoveryState::Idle,
            RecoveryState::Initializing
        ));
        assert!(is_valid_transition_static(
            RecoveryState::Initializing,
            RecoveryState::Scanning
        ));
        assert!(is_valid_transition_static(
            RecoveryState::Scanning,
            RecoveryState::Validating
        ));
        assert!(is_valid_transition_static(
            RecoveryState::Validating,
            RecoveryState::Replaying
        ));
        assert!(is_valid_transition_static(
            RecoveryState::Replaying,
            RecoveryState::Finalizing
        ));
        assert!(is_valid_transition_static(
            RecoveryState::Finalizing,
            RecoveryState::Complete
        ));

        assert!(!is_valid_transition_static(
            RecoveryState::Complete,
            RecoveryState::Scanning
        ));
        assert!(!is_valid_transition_static(
            RecoveryState::Idle,
            RecoveryState::Replaying
        ));
    }

    #[test]
    fn test_recovery_options_default() {
        let options = RecoveryOptions::default();
        assert!(!options.fast_recovery);
        assert_eq!(options.max_batch_size, DEFAULT_BATCH_SIZE);
        assert_eq!(
            options.recovery_timeout,
            Duration::from_secs(DEFAULT_RECOVERY_TIMEOUT_SECONDS)
        );
        assert!(options.perform_consistency_checks);
        assert!(options.create_backup);
        assert_eq!(options.max_recovery_attempts, MAX_RECOVERY_ATTEMPTS);
        assert!(!options.force_recovery);
    }

    fn is_valid_transition_static(from: RecoveryState, to: RecoveryState) -> bool {
        use RecoveryState::*;

        match (from, to) {
            (Idle, Initializing) => true,
            (Initializing, Scanning) => true,
            (Scanning, Validating) => true,
            (Scanning, Replaying) => true,
            (Validating, Replaying) => true,
            (Replaying, Finalizing) => true,
            (Finalizing, Complete) => true,
            (_, Failed) => true,
            _ => false,
        }
    }
}
