//! Core operations and algorithms for native graph backend.
//!
//! ## INLINE HINT STRATEGY (Phase 13 Step 5)
//!
//! ### Tier A: Tiny Hot Path Functions (#[inline(always)])
//! - `estimate_graph_size_category()`: Simple match statement for BFS dispatch
//! - `select_bfs_strategy()`: Strategy selection logic called for every BFS
//!
//! ### Tier B: Small Helper Functions (#[inline] or compiler-driven)
//! - Public API functions: Moderate complexity but good inline candidates
//!
//! ### Tier C: Large Functions (no inline hints)
//! - BFS implementations: Large algorithms left to compiler discretion
//! - `native_shortest_path()`, `native_k_hop()`: Complex algorithms
//!
//! ## Modular Architecture
//!
//! - `strategy.rs`: CPU profiling and graph size categorization
//! - `bfs_implementations.rs`: Multiple BFS implementations with optimizations
//! - `pathfinding.rs`: Shortest path algorithms using BFS
//! - `k_hop.rs`: K-hop neighbor exploration operations
//! - `chain_queries.rs`: Chain traversal and pattern matching
//! - `tests.rs`: Comprehensive test suite

use super::cpu_tuning::resolve_cpu_profile;
use super::graph_file::GraphFile;
use super::types::CpuProfile;
use super::types::*;
use crate::backend::{BackendDirection, ChainStep, PatternMatch, PatternQuery};

// Module declarations
mod strategy;
mod bfs_implementations;
mod pathfinding;
mod k_hop;
mod chain_queries;

// Re-export all public functionality
pub use strategy::*;
pub use bfs_implementations::*;
pub use pathfinding::*;
pub use k_hop::*;
pub use chain_queries::*;

/// Native BFS implementation using adjacency helpers
pub fn native_bfs(
    graph_file: &mut GraphFile,
    start: NativeNodeId,
    depth: u32,
) -> Result<Vec<NativeNodeId>, NativeBackendError> {
    // Default to Auto CPU profile for backwards compatibility
    native_bfs_with_cpu_profile(graph_file, start, depth, CpuProfile::Auto)
}

/// Native BFS implementation with explicit CPU profile
pub fn native_bfs_with_cpu_profile(
    graph_file: &mut GraphFile,
    start: NativeNodeId,
    depth: u32,
    cpu_profile: CpuProfile,
) -> Result<Vec<NativeNodeId>, NativeBackendError> {
    if depth == 0 {
        return Ok(vec![start]);
    }

    // Get node count from header for graph size estimation
    let node_count = graph_file.persistent_header().node_count as usize;

    // Select optimal strategy based on CPU profile and graph size
    let strategy = select_bfs_strategy(cpu_profile, node_count);

    match strategy {
        "simd512_optimized" | "avx2_optimized" => bfs_fully_optimized(graph_file, start, depth),
        "simd512_pointer_table" | "avx2_pointer_table" => {
            bfs_pointer_table_optimized(graph_file, start, depth)
        }
        _ => bfs_generic_scalar(graph_file, start, depth),
    }
}

#[cfg(test)]
mod tests;