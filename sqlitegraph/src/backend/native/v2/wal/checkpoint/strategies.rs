//! V2 WAL Checkpoint Strategy Implementations
//!
//! This module provides comprehensive checkpoint strategy implementations for V2 WAL,
//! including size-based, time-based, transaction-based, and adaptive strategies.
//! Each strategy is optimized for V2 clustered edge graph file workloads.

use crate::backend::native::v2::wal::checkpoint::constants::*;
use crate::backend::native::v2::wal::checkpoint::constants::strategies::*;
use crate::backend::native::v2::wal::checkpoint::errors::{CheckpointError, CheckpointResult};
use crate::backend::native::v2::wal::{V2WALConfig, V2WALReader};
use std::time::{Duration, SystemTime};

/// Checkpoint strategy for determining when to trigger checkpoints
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CheckpointStrategy {
    /// Checkpoint when WAL exceeds specified size
    SizeThreshold(u64),

    /// Checkpoint after specified number of transactions
    TransactionCount(u64),

    /// Checkpoint after specified time interval
    TimeInterval(Duration),

    /// Adaptive strategy combining multiple factors
    Adaptive {
        /// Minimum time interval between checkpoints
        min_interval: Duration,
        /// Maximum WAL size before forcing checkpoint
        max_wal_size: u64,
        /// Maximum transaction count before forcing checkpoint
        max_transactions: u64,
    },
}

impl Default for CheckpointStrategy {
    fn default() -> Self {
        // Default to adaptive strategy with reasonable values for V2 workloads
        Self::Adaptive {
            min_interval: Duration::from_secs(DEFAULT_TIME_INTERVAL_SECONDS),
            max_wal_size: DEFAULT_SIZE_THRESHOLD,
            max_transactions: DEFAULT_TRANSACTION_THRESHOLD,
        }
    }
}

/// Checkpoint trigger information for strategy evaluation
#[derive(Debug, Clone)]
pub struct CheckpointTrigger {
    /// Strategy type that triggered the checkpoint
    pub strategy_type: String,

    /// Trigger reason description
    pub reason: String,

    /// Current WAL size when triggered
    pub wal_size: u64,

    /// Transaction count since last checkpoint
    pub transaction_count: u64,

    /// Time since last checkpoint
    pub time_since_last_checkpoint: Duration,

    /// Trigger timestamp
    pub timestamp: SystemTime,
}

/// Strategy validator for checking strategy configuration
pub struct StrategyValidator;

impl StrategyValidator {
    /// Validate checkpoint strategy configuration
    pub fn validate_strategy(strategy: &CheckpointStrategy) -> CheckpointResult<()> {
        match strategy {
            CheckpointStrategy::SizeThreshold(max_size) => {
                if *max_size < MIN_SIZE_THRESHOLD {
                    return Err(CheckpointError::configuration(
                        format!("Size threshold {} below minimum {}", max_size, MIN_SIZE_THRESHOLD)
                    ));
                }
                if *max_size > MAX_SIZE_THRESHOLD {
                    return Err(CheckpointError::configuration(
                        format!("Size threshold {} above maximum {}", max_size, MAX_SIZE_THRESHOLD)
                    ));
                }
                Ok(())
            }

            CheckpointStrategy::TransactionCount(max_tx) => {
                if *max_tx < MIN_TRANSACTION_THRESHOLD {
                    return Err(CheckpointError::configuration(
                        format!("Transaction threshold {} below minimum {}", max_tx, MIN_TRANSACTION_THRESHOLD)
                    ));
                }
                if *max_tx > MAX_TRANSACTION_THRESHOLD {
                    return Err(CheckpointError::configuration(
                        format!("Transaction threshold {} above maximum {}", max_tx, MAX_TRANSACTION_THRESHOLD)
                    ));
                }
                Ok(())
            }

            CheckpointStrategy::TimeInterval(interval) => {
                if *interval < Duration::from_secs(MIN_TIME_INTERVAL_SECONDS) {
                    return Err(CheckpointError::configuration(
                        format!("Time interval {:?} below minimum {:?}", interval, Duration::from_secs(MIN_TIME_INTERVAL_SECONDS))
                    ));
                }
                if *interval > Duration::from_secs(MAX_TIME_INTERVAL_SECONDS) {
                    return Err(CheckpointError::configuration(
                        format!("Time interval {:?} above maximum {:?}", interval, Duration::from_secs(MAX_TIME_INTERVAL_SECONDS))
                    ));
                }
                Ok(())
            }

            CheckpointStrategy::Adaptive { min_interval, max_wal_size, max_transactions } => {
                // Validate min_interval
                if *min_interval < Duration::from_secs(ADAPTIVE_MIN_INTERVAL_SECONDS) {
                    return Err(CheckpointError::configuration(
                        format!("Adaptive min interval {:?} below minimum {:?}", min_interval, Duration::from_secs(ADAPTIVE_MIN_INTERVAL_SECONDS))
                    ));
                }

                // Validate max_wal_size
                if *max_wal_size < MIN_SIZE_THRESHOLD {
                    return Err(CheckpointError::configuration(
                        format!("Adaptive max WAL size {} below minimum {}", max_wal_size, MIN_SIZE_THRESHOLD)
                    ));
                }
                if *max_wal_size > MAX_SIZE_THRESHOLD * ADAPTIVE_MAX_WAL_SIZE_MULTIPLIER as u64 {
                    return Err(CheckpointError::configuration(
                        format!("Adaptive max WAL size {} too large", max_wal_size)
                    ));
                }

                // Validate max_transactions
                if *max_transactions < MIN_TRANSACTION_THRESHOLD {
                    return Err(CheckpointError::configuration(
                        format!("Adaptive max transactions {} below minimum {}", max_transactions, MIN_TRANSACTION_THRESHOLD)
                    ));
                }
                if *max_transactions > MAX_TRANSACTION_THRESHOLD as u64 * ADAPTIVE_MAX_TX_MULTIPLIER as u64 {
                    return Err(CheckpointError::configuration(
                        format!("Adaptive max transactions {} too large", max_transactions)
                    ));
                }

                Ok(())
            }
        }
    }

    /// Validate strategy is suitable for V2 workloads
    pub fn validate_v2_suitability(strategy: &CheckpointStrategy) -> CheckpointResult<()> {
        match strategy {
            CheckpointStrategy::SizeThreshold(_) => {
                // Size-based strategies are excellent for V2 clustered edge workloads
                Ok(())
            }

            CheckpointStrategy::TransactionCount(_) => {
                // Transaction-based strategies work well with V2 transaction patterns
                Ok(())
            }

            CheckpointStrategy::TimeInterval(_) => {
                // Time-based strategies are less optimal but acceptable for V2 workloads
                Ok(())
            }

            CheckpointStrategy::Adaptive { .. } => {
                // Adaptive strategies are optimal for V2 workloads due to varying I/O patterns
                Ok(())
            }
        }
    }
}

/// Strategy evaluator for checkpoint trigger detection
pub struct StrategyEvaluator {
    config: V2WALConfig,
}

impl StrategyEvaluator {
    /// Create new strategy evaluator
    pub fn new(config: V2WALConfig) -> Self {
        Self { config }
    }

    /// Evaluate if checkpoint should be triggered based on strategy
    pub fn should_checkpoint(
        &self,
        strategy: &CheckpointStrategy,
        last_checkpoint_time: SystemTime,
        checkpointed_lsn: u64,
    ) -> CheckpointResult<(bool, Option<CheckpointTrigger>)> {
        // Validate strategy first
        StrategyValidator::validate_strategy(strategy)?;

        let trigger = match strategy {
            CheckpointStrategy::SizeThreshold(max_size) => {
                self.evaluate_size_threshold(*max_size)?
            }

            CheckpointStrategy::TransactionCount(max_tx) => {
                self.evaluate_transaction_count(*max_tx, checkpointed_lsn)?
            }

            CheckpointStrategy::TimeInterval(interval) => {
                self.evaluate_time_interval(*interval, last_checkpoint_time)?
            }

            CheckpointStrategy::Adaptive { min_interval, max_wal_size, max_transactions } => {
                self.evaluate_adaptive(*min_interval, *max_wal_size, *max_transactions, last_checkpoint_time, checkpointed_lsn)?
            }
        };

        Ok(trigger)
    }

    /// Evaluate size-based checkpoint trigger
    fn evaluate_size_threshold(&self, max_size: u64) -> CheckpointResult<(bool, Option<CheckpointTrigger>)> {
        let wal_metadata = std::fs::metadata(&self.config.wal_path)
            .map_err(|e| CheckpointError::io(format!("Failed to get WAL file metadata: {}", e)))?;

        let wal_size = wal_metadata.len();
        let should_trigger = wal_size >= max_size;

        if should_trigger {
            let trigger = CheckpointTrigger {
                strategy_type: "SizeThreshold".to_string(),
                reason: format!("WAL size {} bytes exceeded threshold {}", wal_size, max_size),
                wal_size,
                transaction_count: 0,
                time_since_last_checkpoint: Duration::ZERO,
                timestamp: SystemTime::now(),
            };
            Ok((true, Some(trigger)))
        } else {
            Ok((false, None))
        }
    }

    /// Evaluate transaction-based checkpoint trigger
    fn evaluate_transaction_count(&self, max_tx: u64, checkpointed_lsn: u64) -> CheckpointResult<(bool, Option<CheckpointTrigger>)> {
        let reader = V2WALReader::open(&self.config.wal_path)
            .map_err(|e| CheckpointError::v2_integration(format!("Failed to open WAL reader: {}", e)))?;

        let header = reader.header();
        let transaction_count = header.committed_lsn.saturating_sub(checkpointed_lsn);
        let should_trigger = transaction_count >= max_tx;

        if should_trigger {
            let trigger = CheckpointTrigger {
                strategy_type: "TransactionCount".to_string(),
                reason: format!("Transaction count {} exceeded threshold {}", transaction_count, max_tx),
                wal_size: 0,
                transaction_count,
                time_since_last_checkpoint: Duration::ZERO,
                timestamp: SystemTime::now(),
            };
            Ok((true, Some(trigger)))
        } else {
            Ok((false, None))
        }
    }

    /// Evaluate time-based checkpoint trigger
    fn evaluate_time_interval(&self, interval: Duration, last_checkpoint_time: SystemTime) -> CheckpointResult<(bool, Option<CheckpointTrigger>)> {
        let elapsed = last_checkpoint_time
            .elapsed()
            .map_err(|e| CheckpointError::state(format!("Invalid checkpoint time: {}", e)))?;

        let should_trigger = elapsed >= interval;

        if should_trigger {
            let trigger = CheckpointTrigger {
                strategy_type: "TimeInterval".to_string(),
                reason: format!("Time interval {:?} exceeded threshold {:?}", elapsed, interval),
                wal_size: 0,
                transaction_count: 0,
                time_since_last_checkpoint: elapsed,
                timestamp: SystemTime::now(),
            };
            Ok((true, Some(trigger)))
        } else {
            Ok((false, None))
        }
    }

    /// Evaluate adaptive checkpoint trigger
    fn evaluate_adaptive(
        &self,
        min_interval: Duration,
        max_wal_size: u64,
        max_transactions: u64,
        last_checkpoint_time: SystemTime,
        checkpointed_lsn: u64,
    ) -> CheckpointResult<(bool, Option<CheckpointTrigger>)> {
        let mut should_trigger = false;
        let mut reasons = Vec::new();

        // Check minimum time interval
        let elapsed = last_checkpoint_time
            .elapsed()
            .map_err(|e| CheckpointError::state(format!("Invalid checkpoint time: {}", e)))?;

        if elapsed >= min_interval {
            should_trigger = true;
            reasons.push(format!("Time interval {:?} exceeded minimum {:?}", elapsed, min_interval));
        }

        // Check WAL size
        let wal_metadata = std::fs::metadata(&self.config.wal_path)
            .map_err(|e| CheckpointError::io(format!("Failed to get WAL file metadata: {}", e)))?;

        let wal_size = wal_metadata.len();
        if wal_size >= max_wal_size {
            should_trigger = true;
            reasons.push(format!("WAL size {} exceeded maximum {}", wal_size, max_wal_size));
        }

        // Check transaction count
        let reader = V2WALReader::open(&self.config.wal_path)
            .map_err(|e| CheckpointError::v2_integration(format!("Failed to open WAL reader: {}", e)))?;

        let header = reader.header();
        let transaction_count = header.committed_lsn.saturating_sub(checkpointed_lsn);

        if transaction_count >= max_transactions {
            should_trigger = true;
            reasons.push(format!("Transaction count {} exceeded maximum {}", transaction_count, max_transactions));
        }

        if should_trigger {
            let trigger = CheckpointTrigger {
                strategy_type: "Adaptive".to_string(),
                reason: reasons.join("; "),
                wal_size,
                transaction_count,
                time_since_last_checkpoint: elapsed,
                timestamp: SystemTime::now(),
            };
            Ok((true, Some(trigger)))
        } else {
            Ok((false, None))
        }
    }

    /// Get current metrics for strategy monitoring
    pub fn get_strategy_metrics(&self, checkpointed_lsn: u64, last_checkpoint_time: SystemTime) -> CheckpointResult<StrategyMetrics> {
        let wal_metadata = std::fs::metadata(&self.config.wal_path)
            .map_err(|e| CheckpointError::io(format!("Failed to get WAL file metadata: {}", e)))?;

        let reader = V2WALReader::open(&self.config.wal_path)
            .map_err(|e| CheckpointError::v2_integration(format!("Failed to open WAL reader: {}", e)))?;

        let header = reader.header();
        let elapsed = last_checkpoint_time
            .elapsed()
            .map_err(|e| CheckpointError::state(format!("Invalid checkpoint time: {}", e)))?;

        Ok(StrategyMetrics {
            wal_size: wal_metadata.len(),
            committed_lsn: header.committed_lsn,
            checkpointed_lsn,
            pending_transactions: header.committed_lsn.saturating_sub(checkpointed_lsn),
            time_since_last_checkpoint: elapsed,
        })
    }
}

/// Current strategy metrics for monitoring and decision making
#[derive(Debug, Clone)]
pub struct StrategyMetrics {
    /// Current WAL file size in bytes
    pub wal_size: u64,

    /// Current committed LSN in WAL
    pub committed_lsn: u64,

    /// Last checkpointed LSN
    pub checkpointed_lsn: u64,

    /// Number of transactions pending checkpoint
    pub pending_transactions: u64,

    /// Time elapsed since last checkpoint
    pub time_since_last_checkpoint: Duration,
}

impl StrategyMetrics {
    /// Check if metrics indicate urgent checkpoint need
    pub fn is_urgent(&self) -> bool {
        // Urgent if WAL size is very large or many pending transactions
        self.wal_size > DEFAULT_SIZE_THRESHOLD * 2 ||
        self.pending_transactions > DEFAULT_TRANSACTION_THRESHOLD * 2 ||
        self.time_since_last_checkpoint > Duration::from_secs(DEFAULT_TIME_INTERVAL_SECONDS * 2)
    }

    /// Get recommended strategy adjustment
    pub fn recommend_adjustment(&self, current_strategy: &CheckpointStrategy) -> Option<String> {
        match current_strategy {
            CheckpointStrategy::SizeThreshold(max_size) => {
                if self.wal_size > *max_size * 2 {
                    Some("Consider reducing size threshold".to_string())
                } else if self.wal_size < *max_size / 4 {
                    Some("Consider increasing size threshold".to_string())
                } else {
                    None
                }
            }

            CheckpointStrategy::TransactionCount(max_tx) => {
                if self.pending_transactions > *max_tx * 2 {
                    Some("Consider reducing transaction threshold".to_string())
                } else if self.pending_transactions < *max_tx / 4 {
                    Some("Consider increasing transaction threshold".to_string())
                } else {
                    None
                }
            }

            CheckpointStrategy::TimeInterval(interval) => {
                if self.time_since_last_checkpoint > *interval * 2 {
                    Some("Consider reducing time interval".to_string())
                } else if self.time_since_last_checkpoint < *interval / 4 {
                    Some("Consider increasing time interval".to_string())
                } else {
                    None
                }
            }

            CheckpointStrategy::Adaptive { .. } => {
                if self.is_urgent() {
                    Some("Consider adjusting adaptive parameters".to_string())
                } else {
                    None
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::path::PathBuf;

    #[test]
    fn test_checkpoint_strategy_default() {
        let strategy = CheckpointStrategy::default();
        match strategy {
            CheckpointStrategy::Adaptive { min_interval, max_wal_size, max_transactions } => {
                assert_eq!(min_interval, Duration::from_secs(DEFAULT_TIME_INTERVAL_SECONDS));
                assert_eq!(max_wal_size, DEFAULT_SIZE_THRESHOLD);
                assert_eq!(max_transactions, DEFAULT_TRANSACTION_THRESHOLD);
            }
            _ => panic!("Default strategy should be Adaptive"),
        }
    }

    #[test]
    fn test_strategy_validator_size_threshold() -> CheckpointResult<()> {
        // Valid size threshold
        let strategy = CheckpointStrategy::SizeThreshold(DEFAULT_SIZE_THRESHOLD);
        assert!(StrategyValidator::validate_strategy(&strategy).is_ok());

        // Too small
        let strategy = CheckpointStrategy::SizeThreshold(MIN_SIZE_THRESHOLD - 1);
        assert!(StrategyValidator::validate_strategy(&strategy).is_err());

        // Too large
        let strategy = CheckpointStrategy::SizeThreshold(MAX_SIZE_THRESHOLD + 1);
        assert!(StrategyValidator::validate_strategy(&strategy).is_err());

        Ok(())
    }

    #[test]
    fn test_strategy_validator_transaction_count() -> CheckpointResult<()> {
        // Valid transaction count
        let strategy = CheckpointStrategy::TransactionCount(DEFAULT_TRANSACTION_THRESHOLD);
        assert!(StrategyValidator::validate_strategy(&strategy).is_ok());

        // Too small
        let strategy = CheckpointStrategy::TransactionCount(MIN_TRANSACTION_THRESHOLD - 1);
        assert!(StrategyValidator::validate_strategy(&strategy).is_err());

        // Too large
        let strategy = CheckpointStrategy::TransactionCount(MAX_TRANSACTION_THRESHOLD + 1);
        assert!(StrategyValidator::validate_strategy(&strategy).is_err());

        Ok(())
    }

    #[test]
    fn test_strategy_validator_time_interval() -> CheckpointResult<()> {
        // Valid time interval
        let strategy = CheckpointStrategy::TimeInterval(Duration::from_secs(DEFAULT_TIME_INTERVAL_SECONDS));
        assert!(StrategyValidator::validate_strategy(&strategy).is_ok());

        // Too small
        let strategy = CheckpointStrategy::TimeInterval(Duration::from_secs(MIN_TIME_INTERVAL_SECONDS - 1));
        assert!(StrategyValidator::validate_strategy(&strategy).is_err());

        // Too large
        let strategy = CheckpointStrategy::TimeInterval(Duration::from_secs(MAX_TIME_INTERVAL_SECONDS + 1));
        assert!(StrategyValidator::validate_strategy(&strategy).is_err());

        Ok(())
    }

    #[test]
    fn test_strategy_validator_adaptive() -> CheckpointResult<()> {
        // Valid adaptive strategy
        let strategy = CheckpointStrategy::Adaptive {
            min_interval: Duration::from_secs(DEFAULT_TIME_INTERVAL_SECONDS),
            max_wal_size: DEFAULT_SIZE_THRESHOLD,
            max_transactions: DEFAULT_TRANSACTION_THRESHOLD,
        };
        assert!(StrategyValidator::validate_strategy(&strategy).is_ok());

        // Min interval too small
        let strategy = CheckpointStrategy::Adaptive {
            min_interval: Duration::from_secs(ADAPTIVE_MIN_INTERVAL_SECONDS - 1),
            max_wal_size: DEFAULT_SIZE_THRESHOLD,
            max_transactions: DEFAULT_TRANSACTION_THRESHOLD,
        };
        assert!(StrategyValidator::validate_strategy(&strategy).is_err());

        Ok(())
    }

    #[test]
    fn test_strategy_metrics_urgent() {
        let metrics = StrategyMetrics {
            wal_size: DEFAULT_SIZE_THRESHOLD * 3,
            committed_lsn: 1000,
            checkpointed_lsn: 100,
            pending_transactions: DEFAULT_TRANSACTION_THRESHOLD * 3,
            time_since_last_checkpoint: Duration::from_secs(DEFAULT_TIME_INTERVAL_SECONDS * 3),
        };
        assert!(metrics.is_urgent());

        let non_urgent_metrics = StrategyMetrics {
            wal_size: DEFAULT_SIZE_THRESHOLD / 2,
            committed_lsn: 1000,
            checkpointed_lsn: 800,
            pending_transactions: DEFAULT_TRANSACTION_THRESHOLD / 2,
            time_since_last_checkpoint: Duration::from_secs(DEFAULT_TIME_INTERVAL_SECONDS / 2),
        };
        assert!(!non_urgent_metrics.is_urgent());
    }

    #[test]
    fn test_strategy_metrics_recommendations() {
        let large_wal_metrics = StrategyMetrics {
            wal_size: DEFAULT_SIZE_THRESHOLD * 3,
            committed_lsn: 1000,
            checkpointed_lsn: 100,
            pending_transactions: 100,
            time_since_last_checkpoint: Duration::from_secs(60),
        };

        let strategy = CheckpointStrategy::SizeThreshold(DEFAULT_SIZE_THRESHOLD);
        let recommendation = large_wal_metrics.recommend_adjustment(&strategy);
        assert!(recommendation.is_some());
        assert!(recommendation.unwrap().contains("reducing size threshold"));
    }

    #[test]
    fn test_checkpoint_trigger_creation() {
        let trigger = CheckpointTrigger {
            strategy_type: "TestStrategy".to_string(),
            reason: "Test trigger".to_string(),
            wal_size: 1024 * 1024,
            transaction_count: 100,
            time_since_last_checkpoint: Duration::from_secs(300),
            timestamp: SystemTime::now(),
        };

        assert_eq!(trigger.strategy_type, "TestStrategy");
        assert_eq!(trigger.reason, "Test trigger");
        assert_eq!(trigger.wal_size, 1024 * 1024);
        assert_eq!(trigger.transaction_count, 100);
        assert_eq!(trigger.time_since_last_checkpoint, Duration::from_secs(300));
    }
}