//! Reindexing progress information and stages

use std::time::Duration;

/// Reindexing progress information
#[derive(Debug, Clone)]
pub struct ReindexProgress {
    pub stage: ReindexStage,
    pub current: usize,
    pub total: usize,
    pub elapsed: Duration,
    pub estimated_remaining: Option<Duration>,
}

impl ReindexProgress {
    pub fn new(stage: ReindexStage, current: usize, total: usize, elapsed: Duration) -> Self {
        let estimated_remaining = if current > 0 {
            Some(Duration::from_nanos(
                (elapsed.as_nanos() as u64 * total as u64) / current as u64
                    - elapsed.as_nanos() as u64,
            ))
        } else {
            None
        };

        Self {
            stage,
            current,
            total,
            elapsed,
            estimated_remaining,
        }
    }

    pub fn progress_percent(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            (self.current as f64 / self.total as f64) * 100.0
        }
    }
}

/// Reindexing stages
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReindexStage {
    /// Analyzing existing data
    Analyzing,
    /// Rebuilding entity indexes
    EntityIndexes,
    /// Rebuilding edge indexes  
    EdgeIndexes,
    /// Rebuilding label indexes
    LabelIndexes,
    /// Rebuilding property indexes
    PropertyIndexes,
    /// Rebuilding adjacency caches
    AdjacencyCaches,
    /// Validating reindexed data
    Validation,
    /// Completed
    Complete,
}
