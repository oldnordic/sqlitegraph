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
    fn read_page_from_disk(
        &self,
        file: &mut std::fs::File,
        db_path: &Path,
        page_id: u64,
    ) -> NativeResult<Vec<u8>> {
        use crate::backend::native::v3::constants::V3_HEADER_SIZE;
        use std::io::{Read, Seek, SeekFrom};

        let offset = V3_HEADER_SIZE + (page_id - 1) * (self.page_size as u64);
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to seek to edge page {} offset {}", page_id, offset),
                source: e,
            })?;

        let mut buffer = vec![0u8; self.page_size as usize];
        file.read_exact(&mut buffer)
            .map_err(|e| NativeBackendError::IoError {
                context: format!(
                    "Failed to read edge page {} from {}",
                    page_id,
                    db_path.display()
                ),
                source: e,
            })?;
        Ok(buffer)
    }

    fn weighted_neighbors_from_cluster(
        &self,
        src: i64,
        dir: Direction,
        cluster: &V3EdgeCluster,
    ) -> Arc<[(i64, f32)]> {
        let mut edge_types = self.edge_types.write();
        let mut neighbors = Vec::with_capacity(cluster.edges.len());
        for e in &cluster.edges {
            let edge_type = V3EdgeCluster::extract_edge_type(&e.edge_data);
            if let Some(et) = edge_type {
                edge_types.insert((src, e.neighbor_id, dir), et);
            }
            let weight = V3EdgeCluster::extract_edge_weight(&e.edge_data);
            neighbors.push((e.neighbor_id, weight));
        }
        neighbors.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        Arc::from(neighbors.into_boxed_slice())
    }

    fn unweighted_neighbors_from_weighted(neighbors: &[(i64, f32)]) -> Arc<[i64]> {
        let dsts: Vec<i64> = neighbors.iter().map(|(dst, _)| *dst).collect();
        Arc::from(dsts.into_boxed_slice())
    }

    pub fn btree_lock(&self) -> &Arc<parking_lot::RwLock<BTreeManager>> {
        &self.btree
    }

    pub fn cache_lock(&self) -> &Arc<RwLock<HashMap<(i64, Direction), Arc<[i64]>>>> {
        &self.cache
    }

    pub fn cache_weighted_lock(
        &self,
    ) -> &Arc<RwLock<HashMap<(i64, Direction), Arc<[(i64, f32)]>>>> {
        &self.cache_weighted
    }

    pub fn edge_types_lock(&self) -> &Arc<RwLock<HashMap<(i64, i64, Direction), String>>> {
        &self.edge_types
    }

    pub fn page_size(&self) -> u64 {
        self.page_size as u64
    }

    /// Create new edge store (in-memory only)
    /// NOTE: Prefer with_path_and_allocator() for database-backed edge stores
    pub fn new(
        btree: BTreeManager,
        wal: Option<WALWriter>,
        allocator: Arc<RwLock<PageAllocator>>,
        page_size: u32,
    ) -> Self {
        Self {
            btree: Arc::new(parking_lot::RwLock::new(btree)),
            wal: wal.map(|w| Arc::new(RwLock::new(w))),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_weighted: Arc::new(RwLock::new(HashMap::new())),
            edge_types: Arc::new(RwLock::new(HashMap::new())),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            hit_time_ns: AtomicU64::new(0),
            miss_time_ns: AtomicU64::new(0),
            dirty_clusters: RwLock::new(HashMap::new()),
            db_path: None,
            allocator,
            page_size,
            file_coordinator: None,
        }
    }

    /// Create new edge store with database path and allocator
    /// This is the preferred constructor for database-backed edge stores
    pub fn with_path_and_allocator(
        btree: BTreeManager,
        wal: Option<WALWriter>,
        db_path: PathBuf,
        allocator: Arc<RwLock<PageAllocator>>,
        page_size: u32,
    ) -> Self {
        Self {
            btree: Arc::new(parking_lot::RwLock::new(btree)),
            wal: wal.map(|w| Arc::new(RwLock::new(w))),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_weighted: Arc::new(RwLock::new(HashMap::new())),
            edge_types: Arc::new(RwLock::new(HashMap::new())),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            hit_time_ns: AtomicU64::new(0),
            miss_time_ns: AtomicU64::new(0),
            dirty_clusters: RwLock::new(HashMap::new()),
            db_path: Some(db_path),
            allocator,
            page_size,
            file_coordinator: None,
        }
    }

    /// Create new edge store with disk persistence path
    /// NOTE: This creates a standalone allocator for backward compatibility.
    /// For proper page allocation, use with_path_and_allocator() instead.
    pub fn with_path(btree: BTreeManager, wal: Option<WALWriter>, db_path: PathBuf) -> Self {
        // Create a standalone allocator for backward compatibility
        // WARNING: This allocator is not shared with NodeStore, so page IDs
        // may collide. Always use with_path_and_allocator() in production.
        let header = PersistentHeaderV3::new_v3();
        Self {
            btree: Arc::new(parking_lot::RwLock::new(btree)),
            wal: wal.map(|w| Arc::new(RwLock::new(w))),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_weighted: Arc::new(RwLock::new(HashMap::new())),
            edge_types: Arc::new(RwLock::new(HashMap::new())),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            hit_time_ns: AtomicU64::new(0),
            miss_time_ns: AtomicU64::new(0),
            dirty_clusters: RwLock::new(HashMap::new()),
            db_path: Some(db_path),
            allocator: Arc::new(RwLock::new(PageAllocator::new(&header))),
            page_size: header.page_size,
            file_coordinator: None,
        }
    }

    /// Set the file coordinator for coordinated I/O
    ///
    /// When set, all file reads/writes go through this coordinator to prevent
    /// race conditions and avoid per-cache-miss file open/close overhead.
    /// The coordinator is also propagated to the internal BTreeManager so
    /// edge-cluster lookups share the same persistent handle.
    pub fn set_file_coordinator(&mut self, coordinator: Arc<FileCoordinator>) {
        self.btree.write().set_file_coordinator(coordinator.clone());
        self.file_coordinator = Some(coordinator);
    }

    /// Get neighbors from cache - returns Arc<[i64]> for zero-copy!
    ///
    /// IMPROVED: On cache miss, attempts to load from disk if db_path is set.
    /// This enables recovery after reopening the edge store.
    pub fn neighbors(&self, src: i64, dir: Direction) -> NativeResult<Arc<[i64]>> {
        let key = (src, dir);

        #[cfg(feature = "v3-forensics")]
        FORENSIC_COUNTERS
            .logical_neighbors_calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // First check in-memory cache
        {
            let cache = self.cache.read();
            if let Some(neighbors) = cache.get(&key) {
                self.cache_hits.fetch_add(1, Ordering::Relaxed);
                #[cfg(feature = "v3-forensics")]
                FORENSIC_COUNTERS
                    .edge_cache_hit_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Ok(neighbors.clone()); // Arc clone is just pointer bump, no data copy
            }
        }

        self.cache_misses.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "v3-forensics")]
        FORENSIC_COUNTERS
            .edge_cache_miss_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Cache miss - try to load from disk if we have a db_path
        if let Some(ref db_path) = self.db_path
            && let Ok(neighbors) = self.load_neighbors_from_disk(src, dir, db_path)
        {
            #[cfg(feature = "v3-forensics")]
            if !neighbors.is_empty() {
                FORENSIC_COUNTERS
                    .edge_page_read_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            if !neighbors.is_empty() {
                // Cache the loaded neighbors
                let mut cache = self.cache.write();
                cache.insert(key, neighbors.clone());
                return Ok(neighbors);
            }
        }

        Ok(Arc::from([])) // Empty slice, no allocation
    }

    /// Load neighbors from disk for recovery
    /// IMPORTANT: Also rebuilds edge_types HashMap from deserialized edge records
    /// CRITICAL FIX: Query B+Tree for page_id instead of using formula
    fn load_neighbors_from_disk(
        &self,
        src: i64,
        dir: Direction,
        db_path: &Path,
    ) -> NativeResult<Arc<[i64]>> {
        // CRITICAL FIX: Query B+Tree for page_id instead of calculating it
        // This prevents page ID collision with node storage
        let key = edge_key(src, dir);
        let btree = self.btree.read();

        // Try to get page_id from B+Tree
        let page_id = match btree.lookup(key) {
            Ok(Some(pid)) => pid,
            Ok(None) => {
                // No entry in B+Tree means no edges for this (src, dir)
                return Ok(Arc::from([]));
            }
            Err(_) => {
                // B+Tree lookup error - treat as no edges
                return Ok(Arc::from([]));
            }
        };
        drop(btree);

        match self.load_cluster_from_disk(page_id, src, dir, db_path) {
            Ok(cluster) => {
                let edges_with_types = cluster.edges_with_types();
                let mut edge_types = self.edge_types.write();
                for (dst, edge_type) in edges_with_types {
                    if let Some(et) = edge_type {
                        edge_types.insert((src, dst, dir), et);
                    }
                }

                let neighbors: Vec<i64> = cluster.dsts();
                Ok(Arc::from(neighbors.into_boxed_slice()))
            }
            Err(_) => Ok(Arc::from([])),
        }
    }

    /// Get outgoing neighbors
    pub fn outgoing(&self, src: i64) -> NativeResult<Arc<[i64]>> {
        self.neighbors(src, Direction::Outgoing)
    }

    /// Get incoming neighbors
    pub fn incoming(&self, src: i64) -> NativeResult<Arc<[i64]>> {
        self.neighbors(src, Direction::Incoming)
    }

    /// Get neighbors filtered by edge type
    /// Returns only neighbors connected by edges matching the specified edge_type
    pub fn neighbors_filtered(
        &self,
        src: i64,
        dir: Direction,
        edge_type: &str,
    ) -> NativeResult<Arc<[i64]>> {
        // Get all neighbors first
        let all_neighbors = self.neighbors(src, dir)?;

        // Filter by edge type
        let edge_types = self.edge_types.read();
        let filtered: Vec<i64> = all_neighbors
            .iter()
            .filter(|&&dst| {
                edge_types
                    .get(&(src, dst, dir))
                    .map(|stored_type| stored_type == edge_type)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        Ok(Arc::from(filtered.into_boxed_slice()))
    }

    /// Get the edge type for a specific edge
    pub fn get_edge_type(&self, src: i64, dst: i64, dir: Direction) -> Option<String> {
        let edge_types = self.edge_types.read();
        edge_types.get(&(src, dst, dir)).cloned()
    }

    /// Get or create a dirty cluster, loading from disk if it already exists to avoid dropping previous edges.
    fn get_or_create_dirty_cluster<'m>(
        &self,
        dirty: &'m mut std::collections::HashMap<(i64, Direction), V3EdgeCluster>,
        src: i64,
        dir: Direction,
    ) -> NativeResult<&'m mut V3EdgeCluster> {
        let cache_key = (src, dir);
        if dirty.contains_key(&cache_key) {
            return dirty
                .get_mut(&cache_key)
                .ok_or_else(|| NativeBackendError::InvalidOperation {
                    context: format!(
                        "dirty edge cluster missing despite existing key for src={} dir={:?}",
                        src, dir
                    ),
                });
        }

        let key = edge_key(src, dir);
        let lookup_res = {
            let btree = self.btree.read();
            btree.lookup(key)
        };

        let mut existing_cluster = None;
        let page_id_to_use = match lookup_res {
            Ok(Some(pid)) => {
                // Try to load existing cluster from disk to avoid dropping previous edges
                if let Some(ref db_path) = self.db_path
                    && let Ok(mut cluster) = self.load_cluster_from_disk(pid, src, dir, db_path)
                {
                    cluster.page_id = 0;
                    existing_cluster = Some(cluster);
                }
                pid
            }
            Ok(None) | Err(_) => 0,
        };

        let cluster =
            existing_cluster.unwrap_or_else(|| V3EdgeCluster::new(src, dir, page_id_to_use));
        dirty.insert(cache_key, cluster);
        dirty
            .get_mut(&cache_key)
            .ok_or_else(|| NativeBackendError::InvalidOperation {
                context: format!(
                    "dirty edge cluster missing after insertion for src={} dir={:?}",
                    src, dir
                ),
            })
    }

    /// Insert an edge - uses interior mutability via RwLock, takes &self!
    ///
    /// # SEMANTIC CONSTRAINT
    /// The edge_types HashMap is keyed by (src, dst, dir). This means:
    /// - Only ONE edge type can exist between a given (src, dst, dir) tuple
    /// - Inserting a second edge between same endpoints with different type will OVERWRITE
    /// - This is intentional for V3's simple tuple-key model
    /// - If multi-edge support is needed, the key model must change to use edge_id
    pub fn insert_edge(
        &self,
        src: i64,
        dst: i64,
        dir: Direction,
        edge_type: Option<String>,
    ) -> NativeResult<()> {
        let cache_key = (src, dir);
        let mut cache = self.cache.write();

        // Get or update entry in cache
        if let Some(neighbors) = cache.get_mut(&cache_key) {
            // Existing entry - need to convert Arc back to Vec, modify, then re-Arc
            let mut vec: Vec<i64> = neighbors.to_vec();
            if !vec.contains(&dst) {
                vec.push(dst);
                *neighbors = Arc::from(vec);
            }
        } else {
            // Cache miss/cold cache or new node
            // Load existing from disk if available to avoid caching a partial list
            let mut vec = Vec::new();
            if let Some(ref db_path) = self.db_path {
                if let Ok(existing) = self.load_neighbors_from_disk(src, dir, db_path) {
                    vec = existing.to_vec();
                }
            }
            if !vec.contains(&dst) {
                vec.push(dst);
            }
            cache.insert(cache_key, Arc::from(vec));
        }

        // Also update weighted cache with default weight 1.0
        {
            let mut cache_weighted = self.cache_weighted.write();
            if let Some(neighbors) = cache_weighted.get_mut(&cache_key) {
                let mut vec = neighbors.to_vec();
                if !vec.iter().any(|(n, _)| *n == dst) {
                    vec.push((dst, 1.0));
                }
                vec.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.0.cmp(&b.0))
                });
                *neighbors = Arc::from(vec);
            } else {
                let mut vec = Vec::new();
                if let Some(ref db_path) = self.db_path {
                    if let Ok(existing) = self.load_neighbors_weighted_from_disk(src, dir, db_path)
                    {
                        vec = existing.to_vec();
                    }
                }
                if !vec.iter().any(|(n, _)| *n == dst) {
                    vec.push((dst, 1.0));
                }
                vec.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.0.cmp(&b.0))
                });
                cache_weighted.insert(cache_key, Arc::from(vec));
            }
        }

        // Store edge type in HashMap AND pass to cluster for durable storage
        // NOTE: If an edge already exists between (src, dst, dir) with a different type,
        // this will overwrite the previous type. This is a known semantic limitation.
        if let Some(ref edge_type_str) = edge_type {
            let mut edge_types = self.edge_types.write();

            // DETECT POTENTIAL ALIASING: Check if we're overwriting a different type
            let key = (src, dst, dir);
            if let Some(existing_type) = edge_types.get(&key)
                && existing_type != edge_type_str
            {
                // SEMANTIC WARNING: Overwriting different edge type for same tuple
                // This is logged but not an error - the caller's responsibility
                eprintln!(
                    "WARNING: V3EdgeStore inserting edge_type '{}' for ({}, {}, {:?}), overwriting existing type '{}'. This is a known limitation of tuple-key model.",
                    edge_type_str, src, dst, dir, existing_type
                );
            }

            edge_types.insert(key, edge_type_str.clone());
        } else {
            // If edge_type is None, remove any existing entry to clear it
            let mut edge_types = self.edge_types.write();
            edge_types.remove(&(src, dst, dir));
        }

        // Mark cluster as dirty for later flush
        let page_id = {
            let mut dirty = self.dirty_clusters.write();
            let cluster = self.get_or_create_dirty_cluster(&mut dirty, src, dir)?;
            let cluster_page_id = cluster.page_id;
            // CRITICAL: Pass edge_type to add_edge() so it gets serialized into edge_data
            cluster.add_edge(dst, edge_type);
            cluster_page_id
        };

        // Log to WAL if configured
        if let Some(ref wal) = self.wal {
            let mut wal_guard = wal.write();
            // Write EdgeInsert WAL record for crash recovery
            let _ = wal_guard.edge_insert(src, dst, dir as u8, page_id);
        }

        Ok(())
    }

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

    /// Get weighted neighbors from cache - returns Arc<[(i64, f32)]> for zero-copy!
    pub fn neighbors_weighted(&self, src: i64, dir: Direction) -> NativeResult<Arc<[(i64, f32)]>> {
        let key = (src, dir);

        // First check in-memory cache
        {
            let cache = self.cache_weighted.read();
            if let Some(neighbors) = cache.get(&key) {
                self.cache_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(neighbors.clone());
            }
        }

        self.cache_misses.fetch_add(1, Ordering::Relaxed);

        // Cache miss - try to load from disk if we have a db_path
        if let Some(ref db_path) = self.db_path
            && let Ok(neighbors) = self.load_neighbors_weighted_from_disk(src, dir, db_path)
        {
            if !neighbors.is_empty() {
                // Cache the loaded neighbors
                let mut cache = self.cache_weighted.write();
                cache.insert(key, neighbors.clone());
                return Ok(neighbors);
            }
        }

        Ok(Arc::from([]))
    }

    /// Load weighted neighbors from disk
    fn load_neighbors_weighted_from_disk(
        &self,
        src: i64,
        dir: Direction,
        db_path: &Path,
    ) -> NativeResult<Arc<[(i64, f32)]>> {
        let key = edge_key(src, dir);
        let btree = self.btree.read();

        let page_id = match btree.lookup(key) {
            Ok(Some(pid)) => pid,
            Ok(None) | Err(_) => return Ok(Arc::from([])),
        };
        drop(btree);

        match self.load_cluster_from_disk(page_id, src, dir, db_path) {
            Ok(cluster) => Ok(self.weighted_neighbors_from_cluster(src, dir, &cluster)),
            Err(_) => Ok(Arc::from([])),
        }
    }

    pub fn warm_weighted_neighbors(&self, sources: &[i64], dir: Direction) -> NativeResult<usize> {
        use std::collections::BTreeMap;

        let db_path = match &self.db_path {
            Some(path) => path,
            None => return Ok(0),
        };
        if sources.is_empty() {
            return Ok(0);
        }

        let misses: Vec<i64> = {
            let cache = self.cache_weighted.read();
            sources
                .iter()
                .copied()
                .filter(|src| !cache.contains_key(&(*src, dir)))
                .collect()
        };
        if misses.is_empty() {
            return Ok(0);
        }

        let lookups: Vec<(i64, u64)> = {
            let btree = self.btree.read();
            misses
                .into_iter()
                .filter_map(|src| match btree.lookup(edge_key(src, dir)) {
                    Ok(Some(page_id)) => Some((src, page_id)),
                    Ok(None) | Err(_) => None,
                })
                .collect()
        };
        if lookups.is_empty() {
            return Ok(0);
        }

        let mut page_groups: BTreeMap<u64, Vec<i64>> = BTreeMap::new();
        for (src, page_id) in lookups {
            page_groups.entry(page_id).or_default().push(src);
        }

        let mut file = std::fs::File::open(db_path).map_err(|e| NativeBackendError::IoError {
            context: format!(
                "Failed to open db file for edge warm read: {}",
                db_path.display()
            ),
            source: e,
        })?;

        let mut warmed = Vec::new();
        for (page_id, srcs) in page_groups {
            let buffer = self.read_page_from_disk(&mut file, db_path, page_id)?;
            if buffer.len() >= PACKED_EDGE_PAGE_HEADER_SIZE
                && buffer[0..4] == PACKED_EDGE_PAGE_MAGIC
            {
                for src in srcs {
                    if let Some(cluster_bytes) = decode_packed_edge_page(&buffer, src, dir)? {
                        let mut cluster = V3EdgeCluster::deserialize(&cluster_bytes, page_id)?;
                        cluster.page_id = 0;
                        cluster.sort_for_weighted_queries();
                        let weighted = self.weighted_neighbors_from_cluster(src, dir, &cluster);
                        let unweighted = Self::unweighted_neighbors_from_weighted(&weighted);
                        warmed.push((src, weighted, unweighted));
                    }
                }
                continue;
            }

            for src in srcs {
                let mut cluster = self.load_cluster_from_open_file(
                    &mut file,
                    page_id,
                    src,
                    dir,
                    db_path,
                    Some(buffer.clone()),
                )?;
                cluster.sort_for_weighted_queries();
                let weighted = self.weighted_neighbors_from_cluster(src, dir, &cluster);
                let unweighted = Self::unweighted_neighbors_from_weighted(&weighted);
                warmed.push((src, weighted, unweighted));
            }
        }

        if warmed.is_empty() {
            return Ok(0);
        }

        let warmed_count = warmed.len();
        let mut weighted_cache = self.cache_weighted.write();
        let mut unweighted_cache = self.cache.write();
        for (src, weighted, unweighted) in warmed {
            weighted_cache.insert((src, dir), weighted);
            unweighted_cache.insert((src, dir), unweighted);
        }
        Ok(warmed_count)
    }

    /// Get weighted neighbors filtered by edge type
    pub fn neighbors_weighted_filtered(
        &self,
        src: i64,
        dir: Direction,
        edge_type: &str,
    ) -> NativeResult<Arc<[(i64, f32)]>> {
        let all_neighbors = self.neighbors_weighted(src, dir)?;

        let edge_types = self.edge_types.read();
        let filtered: Vec<(i64, f32)> = all_neighbors
            .iter()
            .filter(|&&(dst, _)| {
                edge_types
                    .get(&(src, dst, dir))
                    .map(|stored_type| stored_type == edge_type)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        Ok(Arc::from(filtered.into_boxed_slice()))
    }

    /// Insert a weighted edge
    pub fn insert_edge_weighted(
        &self,
        src: i64,
        dst: i64,
        dir: Direction,
        edge_type: Option<String>,
        weight: f32,
    ) -> NativeResult<()> {
        let cache_key = (src, dir);

        // Update unweighted cache
        {
            let mut cache = self.cache.write();
            if let Some(neighbors) = cache.get_mut(&cache_key) {
                let mut vec: Vec<i64> = neighbors.to_vec();
                if !vec.contains(&dst) {
                    vec.push(dst);
                    *neighbors = Arc::from(vec);
                }
            } else {
                let mut vec = Vec::new();
                if let Some(ref db_path) = self.db_path {
                    if let Ok(existing) = self.load_neighbors_from_disk(src, dir, db_path) {
                        vec = existing.to_vec();
                    }
                }
                if !vec.contains(&dst) {
                    vec.push(dst);
                }
                cache.insert(cache_key, Arc::from(vec));
            }
        }

        // Update weighted cache
        {
            let mut cache_weighted = self.cache_weighted.write();
            if let Some(neighbors) = cache_weighted.get_mut(&cache_key) {
                let mut vec = neighbors.to_vec();
                if let Some(pos) = vec.iter().position(|(n, _)| *n == dst) {
                    vec[pos].1 = weight;
                } else {
                    vec.push((dst, weight));
                }
                vec.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.0.cmp(&b.0))
                });
                *neighbors = Arc::from(vec);
            } else {
                let mut vec = Vec::new();
                if let Some(ref db_path) = self.db_path {
                    if let Ok(existing) = self.load_neighbors_weighted_from_disk(src, dir, db_path)
                    {
                        vec = existing.to_vec();
                    }
                }
                if let Some(pos) = vec.iter().position(|(n, _)| *n == dst) {
                    vec[pos].1 = weight;
                } else {
                    vec.push((dst, weight));
                }
                vec.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.0.cmp(&b.0))
                });
                cache_weighted.insert(cache_key, Arc::from(vec));
            }
        }

        if let Some(ref edge_type_str) = edge_type {
            let mut edge_types = self.edge_types.write();
            edge_types.insert((src, dst, dir), edge_type_str.clone());
        } else {
            let mut edge_types = self.edge_types.write();
            edge_types.remove(&(src, dst, dir));
        }

        // Mark cluster as dirty
        let page_id = {
            let mut dirty = self.dirty_clusters.write();
            let cluster = self.get_or_create_dirty_cluster(&mut dirty, src, dir)?;
            let cluster_page_id = cluster.page_id;
            cluster.add_edge_weighted(dst, edge_type, weight);
            cluster_page_id
        };

        if let Some(ref wal) = self.wal {
            let mut wal_guard = wal.write();
            let _ = wal_guard.edge_insert(src, dst, dir as u8, page_id);
        }

        Ok(())
    }
}

#[cfg(test)]
#[path = "edge_compat_tests.rs"]
mod tests;
