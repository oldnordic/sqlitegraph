//! V3 Edge Compatibility Layer
//!
//! This module provides a compatibility layer for using V2 EdgeCluster format
//! within V3's page-based storage system. This compat layer bridges
//! edge layout until NodeStore/B+Tree/allocator/WAL are fully settled.
//! NodeStore/B+Tree/allocator/WAL are still settling.
//!
//! # Design Principles
//!
//! 1. **Logical NodeIDs only**: EdgeCluster references NodeID, not V2 slot assumptions
//!    Resolution is via B+Tree → page.
//!
//! 2. **V3 pages + allocator**: Edge storage lives in V3 pages allocated by V3 allocator.
//!    Only the record format is reused from V2.
//!
//! 3. **Separate PageType**: Edges get their own PageType::EDGE_CLUSTER.
//!    Node pages never embed edge blobs.
//!
//! 4. **WAL-first**: Write path is WAL'd (insert_edge/delete_edge/update adjacency)
//!    before any compaction/relocation.
//!
//! # Edge Type Storage Model
//!
//! ## Durable Storage
//! Edge types are stored durably in the edge_data field of CompactEdgeRecord using
//! an inline encoding format: `[type_len: u8][type_bytes]`. This ensures edge types
//! survive reopen/recovery.
//!
//! ## In-Memory Index
//! The `edge_types: HashMap<(src, dst, dir), String>` field provides O(1) filtering.
//! This HashMap is rebuilt from durable storage on cache miss via `load_neighbors_from_disk()`.
//!
//! ## SEMANTIC CONSTRAINT (Known Limitation)
//!
//! The edge_types HashMap is keyed by `(src, dst, dir)`, NOT by edge_id. This means:
//!
//! - **Only ONE edge type can exist between a given (src, dst, dir) tuple**
//! - Inserting a second edge between same endpoints with a different type OVERWRITES the previous type
//! - This is intentional for V3's simple tuple-key model
//! - If multi-edge support (same endpoints, different types) is needed, the key model must change to use edge_id
//!
//! Example of the aliasing behavior:
//! ```ignore
//! insert_edge(1, 2, Outgoing, "CALLS")  // edge_types: {(1,2,Out) -> "CALLS"}
//! insert_edge(1, 2, Outgoing, "USES")   // edge_types: {(1,2,Out) -> "USES"} ← OVERWRITES!
//! ```
//!
//! # Architecture
//!
//! ```text
//! EdgeCluster { src: NodeId, dsts: Vec<NodeId>, dir: Out|In, metadata }
//!
//! B+Tree index: key = (src, dir) → value = edge_page_id
//!
//! Neighbor query: lookup_edge_page(src) → decode cluster → return iterator
//!
//! Insert edge: load cluster (or create), append, maybe split if page full
//! ```

#![allow(
    clippy::type_complexity,
    clippy::collapsible_if,
    reason = "V3 cache types are complex by design; collapsed let-chains reduce readability"
)]
#[path = "edge_cluster_support.rs"]
mod edge_cluster_support;
#[path = "edge_disk_support.rs"]
mod edge_disk_support;
#[path = "edge_mutation_support.rs"]
mod edge_mutation_support;
#[path = "edge_query_support.rs"]
mod edge_query_support;
#[cfg(feature = "v3-forensics")]
use crate::backend::native::v3::forensics::{
    FORENSIC_COUNTERS, PageType as ForensicPageType, Subsystem,
};
use crate::backend::native::v3::{
    allocator::PageAllocator, btree::BTreeManager, file_coordinator::FileCoordinator,
    header::PersistentHeaderV3, wal::WALWriter,
};
use crate::backend::native::{
    types::{NativeBackendError, NativeResult},
    v3::compact_edge_record::Direction as V2Direction,
};
pub use edge_cluster_support::V3EdgeCluster;
pub(crate) use edge_cluster_support::{decode_edge_cluster_page_header, decode_packed_edge_page};
use edge_cluster_support::{encode_edge_cluster_pages, encode_packed_edge_page};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Page type constants for V3 storage
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PageType {
    /// Free/unallocated page
    Free = 0,
    /// B+Tree index page (node_id → page_id mapping)
    BTreeIndex = 1,
    /// Node data page (contains NodeRecordV3 entries)
    NodeData = 2,
    /// Edge cluster page (contains EdgeCluster entries)
    EdgeCluster = 3,
    /// WAL page (contains WAL records)
    Wal = 4,
    /// Checkpoint page
    Checkpoint = 5,
}

impl PageType {
    /// Convert from u8 to PageType
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(PageType::Free),
            1 => Some(PageType::BTreeIndex),
            2 => Some(PageType::NodeData),
            3 => Some(PageType::EdgeCluster),
            4 => Some(PageType::Wal),
            5 => Some(PageType::Checkpoint),
            _ => None,
        }
    }
}

/// Direction for edge traversal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Outgoing,
    Incoming,
}

const EDGE_CLUSTER_PAGE_MAGIC: [u8; 4] = *b"V3EC";
pub(crate) const EDGE_CLUSTER_PAGE_HEADER_SIZE: usize = 16;
const PACKED_EDGE_PAGE_MAGIC: [u8; 4] = *b"V3EP";
const PACKED_EDGE_PAGE_HEADER_SIZE: usize = 8;
const PACKED_EDGE_PAGE_SLOT_SIZE: usize = 16;

impl Direction {
    /// Convert to V2 Direction for EdgeCluster compatibility
    pub fn to_v2(&self) -> V2Direction {
        match self {
            Direction::Outgoing => V2Direction::Outgoing,
            Direction::Incoming => V2Direction::Incoming,
        }
    }
}

pub struct V3EdgeStore {
    /// B+Tree index: (src, dir) → page_id
    #[cfg(test)]
    pub btree: Arc<parking_lot::RwLock<BTreeManager>>,
    #[cfg(not(test))]
    btree: Arc<parking_lot::RwLock<BTreeManager>>,
    /// Optional WAL writer for durability (shared with V3Backend via Arc)
    #[cfg(test)]
    pub wal: Option<Arc<RwLock<WALWriter>>>,
    #[cfg(not(test))]
    wal: Option<Arc<RwLock<WALWriter>>>,
    /// In-memory cache of neighbor lists - using Arc<[i64]> for zero-copy reads
    /// This matches SQLite's AdjacencyCache pattern but with Arc for zero-copy
    cache: Arc<RwLock<HashMap<(i64, Direction), Arc<[i64]>>>>,
    /// In-memory cache of weighted neighbors
    cache_weighted: Arc<RwLock<HashMap<(i64, Direction), Arc<[(i64, f32)]>>>>,
    /// Edge type storage: (src, dst, dir) -> edge_type string
    ///
    /// SEMANTIC CONSTRAINT: Key is (src, dst, dir), NOT edge_id.
    /// This means only ONE edge type can exist between a given (src, dst, dir) tuple.
    /// Inserting multiple edges between same endpoints with different types will
    /// cause aliasing - the last type wins. See module-level docs for details.
    ///
    /// This HashMap is rebuilt from durable edge_data on cache miss via
    /// load_neighbors_from_disk(), ensuring edge types survive reopen/recovery.
    edge_types: Arc<RwLock<HashMap<(i64, i64, Direction), String>>>,
    /// Performance counters
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    /// Hit time accumulator (nanoseconds) - for profiling
    hit_time_ns: AtomicU64,
    /// Miss time accumulator (nanoseconds) - for profiling
    miss_time_ns: AtomicU64,
    /// Track dirty clusters that need to be flushed
    #[cfg(test)]
    pub dirty_clusters: RwLock<HashMap<(i64, Direction), V3EdgeCluster>>,
    #[cfg(not(test))]
    dirty_clusters: RwLock<HashMap<(i64, Direction), V3EdgeCluster>>,
    /// Path to database file for writing pages
    db_path: Option<PathBuf>,
    /// Page allocator for edge page allocation
    /// CRITICAL: Shared with NodeStore to prevent page ID collisions
    allocator: Arc<RwLock<PageAllocator>>,
    /// Page size for I/O operations (detected from storage media)
    page_size: u32,
    /// Coordinated file handle for all main DB I/O (optional for backward compat)
    /// When set, all file writes go through this coordinator to prevent race conditions
    file_coordinator: Option<Arc<FileCoordinator>>,
}

/// Encode (src, dir) into a composite key for B+Tree lookup
/// Format: [dir: 1 bit][src_abs: 62 bits][sign: 1 bit]
///
/// This encoding guarantees the resulting i64 is always positive by placing
/// the direction bit in the MSB position and using only the magnitude of src.
/// Negative src node IDs are encoded with a sign bit in the LSB.
///
/// Ordering: Incoming edges sort before Outgoing edges for the same node.
pub(crate) fn edge_key(src: i64, dir: Direction) -> i64 {
    let dir_bit = if dir == Direction::Outgoing {
        0i64
    } else {
        1i64
    };
    // Use zigzag encoding for src to handle negative node IDs
    // zigzag(n) = (n << 1) ^ (n >> 63) maps negatives to positive even numbers
    let zigzag_src = (src << 1) ^ (src >> 63);
    // Combine: dir in high bit, zigzag_src in lower bits
    // Ensure result is positive: dir_bit is 0 or 1, zigzag_src is non-negative
    // We place zigzag_src in lower 63 bits and dir_bit in bit 63
    // But bit 63 makes it negative! Instead, interleave:
    // key = (dir_bit << 62) | (zigzag_src & 0x3FFF_FFFF_FFFF_FFFF)

    (dir_bit << 62) | (zigzag_src & 0x3FFF_FFFF_FFFF_FFFF)
}

impl V3EdgeStore {
    /// Clear in-memory cache
    /// Also clears edge_types HashMap to ensure consistency
    pub fn clear_cache(&self) {
        self.cache.write().clear();
        self.cache_weighted.write().clear();
        self.edge_types.write().clear();
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
    }

    /// Print cache statistics for debugging/benchmarking
    pub fn print_stats(&self) {
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let cache_size = self.cache.read().len();
        let hit_ns = self.hit_time_ns.load(Ordering::Relaxed);
        let miss_ns = self.miss_time_ns.load(Ordering::Relaxed);
        let total = hits + misses;
        let hit_rate = if total > 0 {
            (hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let avg_hit_ns = hit_ns.checked_div(hits).unwrap_or(0);
        let avg_miss_ns = miss_ns.checked_div(misses).unwrap_or(0);

        println!("Cache stats:");
        println!("  Entries: {}", cache_size);
        println!("  Hits: {} ({:.1}%)", hits, hit_rate);
        println!("  Misses: {}", misses);
        println!("  Avg hit time: {} ns", avg_hit_ns);
        println!("  Avg miss time: {} ns", avg_miss_ns);
    }

    /// Return cache statistics for focused benchmarking and profiling.
    pub fn cache_stats(&self) -> (u64, u64, u64, u64, usize) {
        (
            self.cache_hits.load(Ordering::Relaxed),
            self.cache_misses.load(Ordering::Relaxed),
            self.hit_time_ns.load(Ordering::Relaxed),
            self.miss_time_ns.load(Ordering::Relaxed),
            self.cache.read().len(),
        )
    }

    /// Reset cache timing counters without evicting cached neighbor slices.
    pub fn reset_stats(&self) {
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.hit_time_ns.store(0, Ordering::Relaxed);
        self.miss_time_ns.store(0, Ordering::Relaxed);
    }
}

#[cfg(test)]
#[path = "edge_compat_tests.rs"]
mod tests;
