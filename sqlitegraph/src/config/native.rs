//! Native backend configuration.

use crate::backend::native::CpuProfile;
use crate::backend::native::v2::wal::checkpoint::CheckpointStrategy;
use std::time::Duration;

/// Configuration for native backend operations.
#[derive(Clone, Debug)]
pub struct NativeConfig {
    /// Whether to create the graph file if it doesn't exist
    pub create_if_missing: bool,
    /// Optional capacity pre-allocation for nodes (performance optimization)
    pub reserve_node_capacity: Option<usize>,
    /// Optional capacity pre-allocation for edges (performance optimization)
    pub reserve_edge_capacity: Option<usize>,
    /// CPU Profile for performance optimizations
    pub cpu_profile: Option<CpuProfile>,
    /// Maximum number of parallel WAL recovery transactions (default: 4)
    pub max_parallel_transactions: usize,
    /// Checkpoint strategy (default: Adaptive with sensible defaults)
    pub checkpoint_strategy: Option<CheckpointStrategy>,
}

impl Default for NativeConfig {
    fn default() -> Self {
        Self {
            create_if_missing: true,
            reserve_node_capacity: None,
            reserve_edge_capacity: None,
            cpu_profile: None,
            max_parallel_transactions: 4, // Default parallelism degree
            checkpoint_strategy: None,   // Uses WAL manager default
        }
    }
}

impl NativeConfig {
    /// Get the effective CPU profile, considering environment variables and defaults
    pub fn effective_cpu_profile(&self) -> CpuProfile {
        // Check environment variable first
        if let Ok(env_profile) = std::env::var("SQLITEGRAPH_NATIVE_CPU_PROFILE") {
            if let Ok(profile) = env_profile.parse() {
                return crate::backend::native::cpu_tuning::resolve_cpu_profile(profile);
            }
            eprintln!(
                "Warning: Invalid SQLITEGRAPH_NATIVE_CPU_PROFILE '{}', using default profile",
                env_profile
            );
        }

        // Use config field if set
        if let Some(profile) = self.cpu_profile {
            return crate::backend::native::cpu_tuning::resolve_cpu_profile(profile);
        }

        // Default to Generic
        CpuProfile::Generic
    }

    /// Set the CPU profile (builder pattern)
    pub fn with_cpu_profile(mut self, profile: CpuProfile) -> Self {
        self.cpu_profile = Some(profile);
        self
    }

    /// Set the maximum parallel WAL recovery transactions (builder pattern)
    pub fn with_parallel_recovery(mut self, degree: usize) -> Self {
        self.max_parallel_transactions = degree;
        self
    }

    /// Set the checkpoint strategy (builder pattern)
    pub fn with_checkpoint_strategy(mut self, strategy: CheckpointStrategy) -> Self {
        self.checkpoint_strategy = Some(strategy);
        self
    }

    /// Set transaction-count checkpoint threshold (builder pattern)
    pub fn with_transaction_checkpoint(mut self, threshold: u64) -> Self {
        self.checkpoint_strategy = Some(CheckpointStrategy::TransactionCount(threshold));
        self
    }

    /// Set size-based checkpoint threshold (builder pattern)
    pub fn with_size_checkpoint(mut self, threshold_bytes: u64) -> Self {
        self.checkpoint_strategy = Some(CheckpointStrategy::SizeThreshold(threshold_bytes));
        self
    }

    /// Set time-based checkpoint interval (builder pattern)
    pub fn with_time_checkpoint(mut self, interval_secs: u64) -> Self {
        self.checkpoint_strategy = Some(CheckpointStrategy::TimeInterval(Duration::from_secs(interval_secs)));
        self
    }
}
