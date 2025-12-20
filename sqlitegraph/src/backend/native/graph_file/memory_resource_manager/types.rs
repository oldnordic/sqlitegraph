//! Memory resource management types and enums
//!
//! This module provides the core types for memory management including
//! I/O modes, statistics, and access pattern hints.

#[derive(Debug, Clone)]
/// Memory management statistics and configuration
pub struct MemoryManagementStatistics {
    /// Current read buffer capacity
    pub read_buffer_capacity: usize,
    /// Current write buffer pending operations
    pub write_buffer_pending_ops: usize,
    /// Whether memory mapping is enabled
    pub mmap_enabled: bool,
    /// Current I/O mode
    pub io_mode: MemoryIOMode,
}

/// Available memory I/O modes for memory management
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryIOMode {
    /// Standard I/O with adaptive buffering
    Standard,
    /// Memory-mapped I/O
    #[cfg(feature = "native-v2")]
    MemoryMapped,
    /// Standard I/O without buffering (exclusive mode)
    #[cfg(feature = "native-v2")]
    ExclusiveStd,
}

/// Hint for memory access pattern optimization
#[derive(Debug, Clone)]
pub enum AccessPatternHint {
    /// Sequential access pattern (large reads, streaming)
    Sequential,
    /// Random access pattern (small reads, scattered)
    Random,
    /// Mixed access pattern (unpredictable)
    Mixed,
}