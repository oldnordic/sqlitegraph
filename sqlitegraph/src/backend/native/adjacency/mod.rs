//! In-memory neighbor iteration helpers for native backend.
//!
//! This module provides efficient adjacency iteration using the node and edge
//! records stored in the native graph file format.
//!
//! ## INSTRUMENTATION & DEBUGGING (RUST SME Investigation)
//!
//! This module includes comprehensive instrumentation for detecting and analyzing
//! infinite loop issues in adjacency iteration.
//!
//! ### Metrics Tracked:
//! - Total adjacency iterations per operation
//! - V2 node read operations count
//! - Performance timing measurements
//! - State consistency validation
//!
//! ## INLINE HINT STRATEGY (Phase 13 Step 5)
//!
//! ### Tier A: Tiny Hot Path Functions (#[inline(always)])
//! - `unlikely()`: Simple boolean wrapper used frequently in hot paths
//! - `get_current_neighbor_fast_path()`: Critical tight loop function
//! - `total_count()`, `current_index()`, `is_complete()`: Simple field accessors
//!
//! ### Tier B: Small Helper Functions (#[inline] or compiler-driven)
//! - `get_current_neighbor()`: Hot path but complex branching
//! - `Iterator::next()`: Iterator implementation (compiler-optimized)
//! - Cache access functions: Used frequently but moderate complexity
//!
//! ### Tier C: Large Functions (no inline hints)
//! - BFS implementations: Large algorithms left to compiler discretion
//! - AdjacencyHelpers: Orchestration functions with complex logic

mod core_iterator;
mod v2_clustered;
mod iterator_impl;

#[cfg(debug_assertions)]
mod instrumentation;
mod helpers;

#[cfg(test)]
mod tests;

pub use core_iterator::AdjacencyIterator;
pub use helpers::AdjacencyHelpers;
// v2_clustered methods are impl blocks on AdjacencyIterator, so no explicit exports needed
// iterator_impl provides Iterator trait implementation for AdjacencyIterator

/// Direction for adjacency traversal
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Outgoing,
    Incoming,
}

/// Hint to the compiler that a condition is unlikely (cold path optimization)
#[inline(always)]
pub(crate) fn unlikely(cond: bool) -> bool {
    // In stable Rust, we don't have the cold intrinsic, but the function
    // name and structure still help with code organization and readability
    cond
}