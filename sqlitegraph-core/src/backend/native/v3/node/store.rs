//! NodeStore V3 - B+Tree Lookup Integration (Task 66-01)
//!
//! This module implements NodeStore for O(log n) node_id -> page_id lookups
//! using B+Tree index traversal. Includes TraversalCache for per-traversal
//! page caching.
//!
//! # Architecture
//!
//! 1. **NodeStore**: B+Tree-based node lookup with page caching
//! 2. **TraversalCache**: LRU cache for NodePage instances
//! 3. **Page Loading**: Loads NodePage from disk using page_id
//! 4. **Decompression**: Extracts NodeRecordV3 from compressed page data
//!
//! # B+Tree Lookup Algorithm
//!
//! The B+Tree provides O(log n) lookup:
//! - Start at root page
//! - For internal pages: binary search keys, descend to child
//! - For leaf pages: binary search entries for exact match

use crate::backend::native::NativeBackendError;
use crate::backend::native::NativeResult;
use crate::backend::native::types::NodeFlags;
use crate::backend::native::v3::allocator::PageAllocator;
use crate::backend::native::v3::btree::BTreeManager;
use crate::backend::native::v3::constants::{DEFAULT_PAGE_SIZE, V3_HEADER_SIZE};
use crate::backend::native::v3::file_coordinator::FileCoordinator;
#[cfg(feature = "v3-forensics")]
use crate::backend::native::v3::forensics::{FORENSIC_COUNTERS, PageType, Subsystem};
use crate::backend::native::v3::header::PersistentHeaderV3;
use crate::backend::native::v3::index::IndexPage;
use crate::backend::native::v3::node::{NodePage, NodeRecordV3};
use crate::backend::native::v3::wal::WALWriter;
use lru::LruCache;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

mod async_lookup;
mod page_loader;
#[cfg(test)]
mod tests;
mod traversal_cache;

pub use async_lookup::lookup_node_async;
pub use page_loader::PageLoader;
pub use traversal_cache::{
    DEFAULT_CACHE_CAPACITY, MAX_CACHE_CAPACITY, MIN_CACHE_CAPACITY, TraversalCache,
    TraversalCacheBuilder,
};

//=============================================================================
// Constants
//=============================================================================

/// Maximum B+Tree height for safety
const MAX_TREE_HEIGHT: u32 = 10;

/// Page cache size for NodeStore.
///
/// 1024 pages = 4 MiB at the 4 KiB default page size. This holds the working
/// set of graphs up to ~16k nodes without thrashing. The previous value of 64
/// pages (256 KiB) evicted constantly on any graph larger than a few hundred
/// nodes. Eviction is LRU (least-recently-used), so hot pages stay resident
/// even when the working set exceeds capacity.
pub const PAGE_CACHE_SIZE: usize = 1024;

/// Construct a page-cache-sized `LruCache` for `PAGE_CACHE_SIZE` entries.
fn new_page_cache<K: std::hash::Hash + Eq, V>() -> LruCache<K, V> {
    LruCache::new(NonZeroUsize::new(PAGE_CACHE_SIZE).expect("PAGE_CACHE_SIZE > 0"))
}

/// Construct an `LruCache` of an explicit capacity (used by `with_capacity`).
fn new_lru<K: std::hash::Hash + Eq, V>(capacity: usize) -> LruCache<K, V> {
    LruCache::new(NonZeroUsize::new(capacity.max(1)).expect("capacity must be >= 1"))
}

//=============================================================================
// NodeStore: B+Tree Lookup Integration
//=============================================================================

/// NodeStore for B+Tree-based node lookup
pub struct NodeStore {
    db_path: PathBuf,
    /// Coordinated file handle for all main DB I/O (optional for backward compatibility)
    /// When set, all file writes go through this coordinator to prevent race conditions
    file_coordinator: Option<Arc<FileCoordinator>>,
    root_page_id: u64,
    tree_height: u32,
    /// Thread-safe node page cache - accessible from both read and write contexts
    /// This fixes the cache bypass bug where read-only lookups couldn't populate the cache.
    /// Bounded LRU cache: hot pages stay resident, least-recently-used evicted on overflow.
    page_cache: Arc<RwLock<LruCache<u64, Vec<u8>>>>,
    /// Cache of unpacked NodePages - avoids repeated unpacking on cache hits
    /// This is a separate cache from page_cache because unpacked pages are more expensive to reconstruct.
    /// Bounded LRU cache (same capacity as `page_cache`).
    unpacked_page_cache: Arc<RwLock<LruCache<u64, Arc<NodePage>>>>,
    cache_capacity: usize,
    /// Block ID of the most recently accessed page (for block-aware eviction)
    /// PROTOTYPE: Track current access block to prefer retaining same-block pages
    current_access_block: std::sync::atomic::AtomicI64,
    /// Block-to-preferred-pages mapping for physical placement prototype
    /// PROTOTYPE: In-memory only, biases same-block nodes to same pages
    /// Maps block_id → list of page_ids preferred for that block
    block_preferred_pages: HashMap<i64, Vec<u64>>,
    /// Maximum preferred pages to track per block (tunable)
    max_preferred_pages_per_block: usize,
    index_cache: LruCache<u64, IndexPage>,
    /// B+Tree manager for index operations
    btree_manager: Option<BTreeManager>,
    /// Page allocator for page management (shared with BTreeManager)
    page_allocator: Option<Arc<RwLock<PageAllocator>>>,
    /// Optional WAL writer for durability
    wal_writer: Option<WALWriter>,
    /// Next available node ID
    next_node_id: i64,
    /// Dirty page buffer for batch writes (page_id -> NodePage)
    dirty_pages: HashMap<u64, NodePage>,
    /// Whether batch mode is active (defer disk writes)
    batch_mode: bool,
}

impl NodeStore {
    pub fn new(header: &PersistentHeaderV3, db_path: PathBuf) -> Self {
        NodeStore {
            db_path,
            file_coordinator: None,
            root_page_id: header.root_index_page,
            tree_height: header.btree_height,
            page_cache: Arc::new(RwLock::new(new_page_cache())),
            unpacked_page_cache: Arc::new(RwLock::new(new_page_cache())),
            cache_capacity: PAGE_CACHE_SIZE,
            current_access_block: std::sync::atomic::AtomicI64::new(-1),
            block_preferred_pages: HashMap::new(),
            max_preferred_pages_per_block: 3,
            index_cache: new_page_cache(),
            btree_manager: None,
            page_allocator: None,
            wal_writer: None,
            next_node_id: 1, // Start from 1
            dirty_pages: HashMap::new(),
            batch_mode: false,
        }
    }

    pub fn btree_manager(&self) -> &Option<BTreeManager> {
        &self.btree_manager
    }

    pub fn page_cache_ref(&self) -> &Arc<RwLock<LruCache<u64, Vec<u8>>>> {
        &self.page_cache
    }

    pub fn unpacked_page_cache_ref(&self) -> &Arc<RwLock<LruCache<u64, Arc<NodePage>>>> {
        &self.unpacked_page_cache
    }

    /// Configured page-cache capacity (pages).
    pub fn cache_capacity(&self) -> usize {
        self.cache_capacity
    }

    /// Current number of resident entries in the raw-bytes page cache.
    pub fn page_cache_len(&self) -> usize {
        self.page_cache.read().len()
    }

    /// Current number of resident entries in the unpacked-page cache.
    pub fn unpacked_page_cache_len(&self) -> usize {
        self.unpacked_page_cache.read().len()
    }

    /// Whether `page_id` is currently resident in the raw-bytes page cache.
    /// Uses `peek` (no recency update) so the check itself doesn't perturb LRU order.
    pub fn page_cache_contains(&self, page_id: u64) -> bool {
        self.page_cache.read().peek(&page_id).is_some()
    }

    /// Resolve the page_id for a given node_id via the btree index.
    /// Read-only (no cache side effects). Used by cache-residency tests.
    pub fn page_id_for_node(&self, node_id: i64) -> NativeResult<Option<u64>> {
        self.lookup_page_ro(node_id)
    }

    /// Clear both the raw-bytes page cache and the unpacked-page cache, and the
    /// index cache. Used by tests and on cache invalidation.
    pub fn clear_all_caches(&mut self) {
        self.page_cache.write().clear();
        self.unpacked_page_cache.write().clear();
        self.index_cache.clear();
    }

    pub fn with_capacity(
        header: &PersistentHeaderV3,
        db_path: PathBuf,
        cache_capacity: usize,
    ) -> Self {
        NodeStore {
            db_path,
            file_coordinator: None,
            root_page_id: header.root_index_page,
            tree_height: header.btree_height,
            page_cache: Arc::new(RwLock::new(new_lru(cache_capacity))),
            unpacked_page_cache: Arc::new(RwLock::new(new_lru(cache_capacity))),
            cache_capacity,
            current_access_block: std::sync::atomic::AtomicI64::new(-1),
            block_preferred_pages: HashMap::new(),
            max_preferred_pages_per_block: 3,
            index_cache: new_lru(cache_capacity),
            btree_manager: None,
            page_allocator: None,
            wal_writer: None,
            next_node_id: 1,
            dirty_pages: HashMap::new(),
            batch_mode: false,
        }
    }

    /// Initialize the store with BTreeManager, PageAllocator and optional WAL
    pub fn initialize(
        &mut self,
        btree_manager: BTreeManager,
        page_allocator: Arc<RwLock<PageAllocator>>,
        wal_writer: Option<WALWriter>,
    ) {
        self.btree_manager = Some(btree_manager);
        self.page_allocator = Some(page_allocator);
        self.wal_writer = wal_writer;
    }

    /// Set the WAL writer
    pub fn set_wal_writer(&mut self, wal: WALWriter) {
        self.wal_writer = Some(wal);
    }

    /// Set the file coordinator for coordinated I/O
    ///
    /// When set, all file reads/writes go through this coordinator to prevent
    /// race conditions and avoid per-cache-miss file open/close overhead.
    /// The coordinator is also propagated to the internal BTreeManager so
    /// index-page lookups share the same persistent handle.
    pub fn set_file_coordinator(&mut self, coordinator: Arc<FileCoordinator>) {
        if let Some(ref mut btree) = self.btree_manager {
            btree.set_file_coordinator(Arc::clone(&coordinator));
        }
        self.file_coordinator = Some(coordinator);
    }

    /// Enable batch mode (defer disk writes until commit)
    ///
    /// When batch mode is enabled, page writes are staged in memory
    /// and flushed to disk with a single fsync on commit.
    pub fn begin_batch(&mut self) {
        self.batch_mode = true;
        self.dirty_pages.clear();
    }

    /// Commit all staged dirty pages with single fsync
    ///
    /// Returns the number of pages flushed.
    pub fn commit_batch(&mut self) -> NativeResult<usize> {
        if !self.batch_mode {
            return Ok(0);
        }

        let page_count = self.dirty_pages.len();

        if page_count > 0 {
            // Use file_coordinator if available (FIXES 10K-node bug)
            if let Some(coordinator) = &self.file_coordinator {
                // Write all dirty pages through coordinator
                for (page_id, page) in &self.dirty_pages {
                    let page_bytes = page.pack()?;
                    coordinator.write_page(*page_id, &page_bytes)?;
                    // Update cache - convert array to Vec
                    self.page_cache_insert(*page_id, page_bytes.to_vec());
                }
                // Clear dirty pages
                self.dirty_pages.clear();
            } else {
                // Fallback to original behavior for backward compatibility
                // CRITICAL FIX: Do NOT use create(true) - it truncates the file!
                // Use create(false) to avoid truncating existing data
                let file_exists = self.db_path.exists();
                let mut file = OpenOptions::new()
                    .write(true)
                    .create(!file_exists) // Only create if file doesn't exist
                    .open(&self.db_path)
                    .map_err(|e| NativeBackendError::IoError {
                        context: format!(
                            "Failed to open database file for batch commit: {}",
                            self.db_path.display()
                        ),
                        source: e,
                    })?;

                // CRITICAL: Find the maximum offset needed and extend file once
                // This is more efficient than extending for each page
                let mut required_len = file.metadata().map(|m| m.len()).unwrap_or(0);

                for (page_id, page) in &self.dirty_pages {
                    let page_bytes = page.pack()?;
                    let offset = Self::page_offset(*page_id);
                    let page_end = offset + page_bytes.len() as u64;
                    if page_end > required_len {
                        required_len = page_end;
                    }
                }

                // Extend file if needed
                let current_len = file.metadata().map(|m| m.len()).unwrap_or(0);
                if required_len > current_len {
                    file.set_len(required_len)
                        .map_err(|e| NativeBackendError::IoError {
                            context: format!(
                                "Failed to extend file to {} bytes for batch commit",
                                required_len
                            ),
                            source: e,
                        })?;
                }

                // Write all dirty pages
                for (page_id, page) in &self.dirty_pages {
                    let page_bytes = page.pack()?;
                    let offset = Self::page_offset(*page_id);

                    file.seek(SeekFrom::Start(offset)).map_err(|e| {
                        NativeBackendError::IoError {
                            context: format!(
                                "Failed to seek to page {} offset {}",
                                page_id, offset
                            ),
                            source: e,
                        }
                    })?;

                    file.write_all(&page_bytes)
                        .map_err(|e| NativeBackendError::IoError {
                            context: format!(
                                "Failed to write page {} during batch commit",
                                page_id
                            ),
                            source: e,
                        })?;

                    // Update cache
                    self.page_cache_insert(*page_id, page_bytes.to_vec());
                }

                // Single fsync for all pages
                file.sync_all().map_err(|e| NativeBackendError::IoError {
                    context: "Failed to sync batch commit to disk".to_string(),
                    source: e,
                })?;

                // Clear dirty pages
                self.dirty_pages.clear();
            }
        }

        self.batch_mode = false;
        Ok(page_count)
    }

    /// Rollback batch - discard staged pages without writing
    pub fn rollback_batch(&mut self) {
        self.dirty_pages.clear();
        self.batch_mode = false;
    }

    /// Check if batch mode is active
    pub fn is_batch_mode(&self) -> bool {
        self.batch_mode
    }

    /// Get count of dirty pages staged for commit
    pub fn dirty_page_count(&self) -> usize {
        self.dirty_pages.len()
    }

    /// Get mutable reference to BTreeManager
    fn btree_manager_mut(&mut self) -> NativeResult<&mut BTreeManager> {
        self.btree_manager
            .as_mut()
            .ok_or_else(|| NativeBackendError::InvalidHeader {
                field: "btree_manager".to_string(),
                reason: "BTreeManager not initialized".to_string(),
            })
    }

    /// Get write lock on PageAllocator
    fn page_allocator_mut(&self) -> NativeResult<parking_lot::RwLockWriteGuard<'_, PageAllocator>> {
        self.page_allocator
            .as_ref()
            .ok_or_else(|| NativeBackendError::InvalidHeader {
                field: "page_allocator".to_string(),
                reason: "PageAllocator not initialized".to_string(),
            })
            .map(|arc| arc.write())
    }

    /// Get the next available node ID and increment
    pub fn allocate_node_id(&mut self) -> i64 {
        let id = self.next_node_id;
        self.next_node_id += 1;
        id
    }

    /// Set the next node ID (for recovery/initialization)
    pub fn set_next_node_id(&mut self, next_id: i64) {
        self.next_node_id = next_id;
    }

    /// Get all node IDs in the store from the B+Tree
    pub fn node_ids(&self) -> NativeResult<Vec<i64>> {
        if let Some(ref btree) = self.btree_manager {
            btree.keys()
        } else {
            Ok(Vec::new())
        }
    }

    /// Insert a new node into the store
    ///
    /// # Arguments
    ///
    /// * `node` - The node record to insert (node.id will be assigned)
    ///
    /// # Returns
    ///
    /// * `Ok(node_id)` - The assigned node ID
    /// * `Err(...)` - Error during insert
    pub fn insert_node(
        &mut self,
        mut node: NodeRecordV3,
        manual_id: Option<i64>,
    ) -> NativeResult<i64> {
        #[cfg(feature = "v3-forensics")]
        FORENSIC_COUNTERS
            .node_encode_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // 1. Allocate new node_id
        let node_id = if let Some(id) = manual_id {
            if id >= self.next_node_id {
                self.next_node_id = id + 1;
            }
            id
        } else {
            self.allocate_node_id()
        };
        node.id = node_id;

        // 2-5. Try to add the node to a page, retrying with a new page if the selected page is full
        // This handles the case where page capacity estimation doesn't match actual packed size
        let mut attempts = 0;
        const MAX_ATTEMPTS: usize = 3;

        loop {
            attempts += 1;

            // 2. Find or create a page for the node
            let page_id = if attempts == 1 {
                // First attempt: use the normal page selection logic
                self.find_or_create_page_for_node(&node)?
            } else {
                // Retry attempts: create a brand new page
                let mut allocator = self.page_allocator_mut()?;
                let new_page_id = allocator.allocate()?;
                let new_page = NodePage::new(new_page_id);

                // Write the empty page to disk
                let page_bytes = new_page.pack()?;
                if let Some(coordinator) = &self.file_coordinator {
                    coordinator.write_page(new_page_id, &page_bytes)?;
                } else {
                    // Fallback: write directly to file when no coordinator
                    let offset = Self::page_offset(new_page_id);
                    let file_exists = self.db_path.exists();
                    let mut file = OpenOptions::new()
                        .write(true)
                        .create(!file_exists)
                        .open(&self.db_path)
                        .map_err(|e| NativeBackendError::IoError {
                            context: format!(
                                "Failed to open db file for retry page write: {}",
                                self.db_path.display()
                            ),
                            source: e,
                        })?;

                    let current_len = file.metadata().map(|m| m.len()).unwrap_or(0);
                    let required_len = offset + page_bytes.len() as u64;
                    if required_len > current_len {
                        file.set_len(required_len)
                            .map_err(|e| NativeBackendError::IoError {
                                context: format!(
                                    "Failed to extend file to {} bytes for retry page {}",
                                    required_len, new_page_id
                                ),
                                source: e,
                            })?;
                    }

                    file.seek(SeekFrom::Start(offset)).map_err(|e| {
                        NativeBackendError::IoError {
                            context: format!(
                                "Failed to seek to offset {} for retry page {}",
                                offset, new_page_id
                            ),
                            source: e,
                        }
                    })?;
                    file.write_all(&page_bytes)
                        .map_err(|e| NativeBackendError::IoError {
                            context: format!("Failed to write retry page {} to disk", new_page_id),
                            source: e,
                        })?;
                    file.sync_all().map_err(|e| NativeBackendError::IoError {
                        context: format!("Failed to sync retry page {}", new_page_id),
                        source: e,
                    })?;
                }

                new_page_id
            };

            // 3. Load the page
            let mut page = self.load_node_page(page_id)?;

            // 4. Add node to page
            match page.add_node(node.clone()) {
                Ok(()) => {
                    // 5. Write page back to disk (or stage in batch mode)
                    self.write_node_page(&page)?;

                    // 6. Update B+Tree index (node_id -> page_id)
                    let btree = self.btree_manager_mut()?;
                    btree.insert(node_id, page_id)?;

                    // 6b. Sync NodeStore's root_page_id and tree_height from BTreeManager
                    // This ensures lookups work correctly after the tree structure changes
                    let new_root = btree.root_page_id();
                    let new_height = btree.tree_height();
                    self.root_page_id = new_root;
                    self.tree_height = new_height;

                    // 7. Log to WAL if configured (skip in batch mode - will be handled at commit)
                    if !self.batch_mode {
                        if let Some(ref mut wal) = self.wal_writer {
                            let page_bytes = page.pack()?;
                            wal.page_write(page_id, 0, page_bytes.to_vec())?;
                        }

                        // 8. Update cache (only in immediate mode; batch updates cache at commit)
                        self.page_cache_insert(page_id, page.pack()?.to_vec());
                    }

                    return Ok(node_id);
                }
                Err(NativeBackendError::InvalidHeader { ref field, .. })
                    if field == "node_page" && attempts < MAX_ATTEMPTS =>
                {
                    // Page is full, retry with a new page
                    continue;
                }
                Err(e) => {
                    // Other error, propagate it
                    return Err(e);
                }
            }
        }
    }

    /// Update an existing node
    ///
    /// # Arguments
    ///
    /// * `node_id` - The ID of the node to update
    /// * `node` - The new node record data
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Update successful
    /// * `Err(...)` - Error during update or node not found
    pub fn update_node(&mut self, node_id: i64, node: NodeRecordV3) -> NativeResult<()> {
        // 1. Lookup existing page
        let page_id = match self.lookup_page(node_id)? {
            Some(pid) => pid,
            None => {
                return Err(NativeBackendError::InvalidHeader {
                    field: "update_node".to_string(),
                    reason: format!("Node {} not found", node_id),
                });
            }
        };

        // 2. Load the page
        let mut page = self.load_node_page(page_id)?;

        // 3. Find and replace the node
        let mut found = false;
        for (i, existing_node) in page.nodes.iter().enumerate() {
            if existing_node.id() == node_id {
                page.nodes[i] = node;
                found = true;
                break;
            }
        }

        if !found {
            return Err(NativeBackendError::InvalidHeader {
                field: "update_node".to_string(),
                reason: format!("Node {} not found in page {}", node_id, page_id),
            });
        }

        // 4. Recalculate used_bytes
        page.used_bytes = page
            .nodes
            .iter()
            .map(|n| self.estimate_node_size(n))
            .sum::<NativeResult<u16>>()?;

        // 5. Write page back to disk
        self.write_node_page(&page)?;

        // 6. Log to WAL if configured
        if let Some(ref mut wal) = self.wal_writer {
            let page_bytes = page.pack()?;
            wal.page_write(page_id, 0, page_bytes.to_vec())?;
        }

        // 7. Update cache
        self.page_cache_insert(page_id, page.pack()?.to_vec());

        Ok(())
    }

    /// Delete a node (soft delete with tombstone)
    ///
    /// # Arguments
    ///
    /// * `node_id` - The ID of the node to delete
    ///
    /// # Returns
    ///
    /// * `Ok(true)` - Node was found and deleted
    /// * `Ok(false)` - Node was not found
    /// * `Err(...)` - Error during delete
    pub fn delete_node(&mut self, node_id: i64) -> NativeResult<bool> {
        // 1. Lookup page
        let page_id = match self.lookup_page(node_id)? {
            Some(pid) => pid,
            None => return Ok(false),
        };

        // 2. Load the page
        let mut page = self.load_node_page(page_id)?;

        // 3. Find and mark as deleted (tombstone)
        let mut found = false;
        for node in page.nodes.iter_mut() {
            if node.id() == node_id {
                node.flags = NodeFlags::DELETED;
                found = true;
                break;
            }
        }

        if !found {
            return Ok(false);
        }

        // 4. Write page back to disk
        self.write_node_page(&page)?;

        // 5. Remove from B+Tree index
        let btree = self.btree_manager_mut()?;
        btree.delete(node_id)?;

        // 6. Log to WAL if configured
        if let Some(ref mut wal) = self.wal_writer {
            let page_bytes = page.pack()?;
            wal.page_write(page_id, 0, page_bytes.to_vec())?;
        }

        // 7. Update cache
        self.page_cache_insert(page_id, page.pack()?.to_vec());

        Ok(true)
    }

    /// Find or create a page for a new node
    fn find_or_create_page_for_node(&mut self, node: &NodeRecordV3) -> NativeResult<u64> {
        // Try to find an existing page with space
        let node_size = self.estimate_node_size(node)?;

        // PROTOTYPE: Block-aware placement bias
        // Try this block's preferred pages first
        use super::page::node_id_to_block;
        let block_id = node_id_to_block(node.id);

        if let Some(preferred_pages) = self.block_preferred_pages.get(&block_id) {
            // Check preferred pages in order (most recent first = reverse)
            for &page_id in preferred_pages.iter().rev() {
                // Check dirty_pages first (in-memory modifications)
                if let Some(page) = self.dirty_pages.get(&page_id)
                    && page.capacity() >= node_size
                {
                    return Ok(page_id);
                }

                // Then check page_cache
                if let Some(page_bytes) = self.page_cache_get(page_id)
                    && let Ok(page) = NodePage::unpack(&page_bytes)
                    && page.capacity() >= node_size
                {
                    return Ok(page_id);
                }
            }
        }

        // Fall back to current behavior: check all dirty pages
        for (&page_id, page) in &self.dirty_pages {
            let cap = page.capacity();
            if cap >= node_size {
                return Ok(page_id);
            }
        }

        // Next, check the page cache (skip pages already in dirty_pages)
        // FIX: Don't clone entire cache - iterate with read lock held
        {
            let cache = self.page_cache.read();
            for (&page_id, page_bytes) in cache.iter() {
                // Skip if this page is already in dirty_pages (stale cache entry)
                if self.dirty_pages.contains_key(&page_id) {
                    continue;
                }
                if let Ok(page) = NodePage::unpack(page_bytes) {
                    let cap = page.capacity();
                    if cap >= node_size {
                        return Ok(page_id);
                    }
                }
            }
        } // Read lock released here

        // Allocate a new page
        let new_page_id = {
            let mut allocator = self.page_allocator_mut()?;
            allocator.allocate()?
        };

        // PROTOTYPE: Associate the new page with this block
        self.associate_page_with_block(new_page_id, block_id);

        // Create empty page
        let new_page = NodePage::new(new_page_id);
        let page_bytes = new_page.pack()?;

        // CRITICAL FIX: Write the empty page to disk BEFORE adding to B+Tree!
        // This ensures that when the B+Tree is updated to point to this page,
        // the page actually exists on disk and can be read.
        // Previously, we only added to cache, causing UnexpectedEof when
        // the page was evicted from cache before being written.
        if let Some(coordinator) = &self.file_coordinator {
            coordinator.write_page(new_page_id, &page_bytes)?;
        } else {
            // Fallback for backward compatibility
            let offset = Self::page_offset(new_page_id);
            let _required_len = offset + page_bytes.len() as u64;

            let file_exists = self.db_path.exists();
            let mut file = OpenOptions::new()
                .write(true)
                .create(!file_exists)
                .open(&self.db_path)
                .map_err(|e| NativeBackendError::IoError {
                    context: format!(
                        "Failed to open db file for new page write: {}",
                        self.db_path.display()
                    ),
                    source: e,
                })?;

            file.seek(SeekFrom::Start(offset))
                .map_err(|e| NativeBackendError::IoError {
                    context: format!(
                        "Failed to seek to offset {} for new page {}",
                        offset, new_page_id
                    ),
                    source: e,
                })?;

            file.write_all(&page_bytes)
                .map_err(|e| NativeBackendError::IoError {
                    context: format!("Failed to write new page {} to disk", new_page_id),
                    source: e,
                })?;

            file.sync_all().map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to sync new page {}", new_page_id),
                source: e,
            })?;
        }

        // Now add to cache for fast access
        self.page_cache_insert(new_page_id, page_bytes.to_vec());

        Ok(new_page_id)
    }

    /// Associate a page with a block for physical placement bias
    ///
    /// PROTOTYPE: When a new page is allocated for a block, remember that
    /// pages from this block should prefer this page in the future.
    fn associate_page_with_block(&mut self, page_id: u64, block_id: i64) {
        let pages = self.block_preferred_pages.entry(block_id).or_default();

        // Avoid duplicates
        if !pages.contains(&page_id) {
            pages.push(page_id);

            // Trim if exceeding max
            while pages.len() > self.max_preferred_pages_per_block {
                // Remove oldest (front of vec)
                pages.remove(0);
            }
        }
    }

    /// Load a NodePage from disk
    fn load_node_page(&mut self, page_id: u64) -> NativeResult<NodePage> {
        // Check dirty pages first (read-your-own-writes during batch)
        if let Some(page) = self.dirty_pages.get(&page_id) {
            #[cfg(feature = "v3-forensics")]
            FORENSIC_COUNTERS
                .dirty_page_hit_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(page.clone());
        }

        // Check the unpacked-page cache BEFORE the raw-bytes cache. This avoids
        // re-running NodePage::unpack (varint decode) on every mutable-path hit
        // — the double-decode bug. lookup_node_ro already did this; load_node_page
        // (used by the mutable lookup_node path) previously did not, so every
        // cache hit re-decoded the page.
        if let Some(cached_page) = self.unpacked_page_cache_get(page_id) {
            #[cfg(feature = "v3-forensics")]
            FORENSIC_COUNTERS
                .node_page_cache_hit_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok((*cached_page).clone());
        }

        // Fall back to the raw-bytes page cache. A hit here still requires an
        // unpack, but we then promote the decoded page into the unpacked cache so
        // subsequent reads skip the decode.
        if let Some(cached) = self.page_cache_get(page_id) {
            #[cfg(feature = "v3-forensics")]
            FORENSIC_COUNTERS
                .node_page_cache_hit_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let page = NodePage::unpack(&cached)?;
            self.unpacked_page_cache_insert(page_id, Arc::new(page.clone()));
            return Ok(page);
        }

        // Load from disk (will count page_read_count and track misses)
        let page_bytes = self.load_page_from_disk(page_id)?;
        #[cfg(feature = "v3-forensics")]
        FORENSIC_COUNTERS
            .node_page_cache_miss_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let page = NodePage::unpack(&page_bytes)?;
        // Populate both caches so the next read is an unpacked-cache hit.
        self.page_cache_insert(page_id, page_bytes);
        self.unpacked_page_cache_insert(page_id, Arc::new(page.clone()));
        Ok(page)
    }

    /// Write a NodePage to disk
    fn write_node_page(&mut self, page: &NodePage) -> NativeResult<()> {
        let page_id = page.page_id;

        #[cfg(feature = "v3-forensics")]
        {
            FORENSIC_COUNTERS
                .page_write_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

            // Track page ownership for forensics
            let offset = if self.file_coordinator.is_some() {
                // FileCoordinator uses: V3_HEADER_SIZE + (page_id - 1) * PAGE_SIZE
                V3_HEADER_SIZE + (page_id.saturating_sub(1)) * DEFAULT_PAGE_SIZE
            } else {
                Self::page_offset(page_id)
            };

            // Register the allocation and write
            crate::track_page_alloc!(page_id, Subsystem::NodeStore, PageType::Node);
            crate::track_page_write!(
                page_id,
                Subsystem::NodeStore,
                PageType::Node,
                offset,
                "NodeStore::write_node_page"
            );
        }

        // In batch mode, stage to dirty_pages instead of writing immediately
        if self.batch_mode {
            self.dirty_pages.insert(page_id, page.clone());
            return Ok(());
        }

        // Pack the page
        let page_bytes = page.pack()?;

        // Use file_coordinator if available (FIXES 10K-node bug)
        if let Some(coordinator) = &self.file_coordinator {
            coordinator.write_page(page_id, &page_bytes)?;
            return Ok(());
        }

        // Fallback to original behavior for backward compatibility
        let offset = Self::page_offset(page_id);
        let required_len = offset + page_bytes.len() as u64;

        // CRITICAL FIX: Do NOT use create(true) - it truncates the file!
        // Use write(true) only to modify existing file without truncation
        // IMPORTANT: If file doesn't exist yet (first page write), we need create(true)
        // But only for the very first write, not for subsequent writes!
        let file_exists = self.db_path.exists();
        let mut file = OpenOptions::new()
            .write(true)
            .create(!file_exists) // Only create if file doesn't exist
            .open(&self.db_path)
            .map_err(|e| NativeBackendError::IoError {
                context: format!(
                    "Failed to open database file for writing: {}",
                    self.db_path.display()
                ),
                source: e,
            })?;

        // CRITICAL: Check current file size and extend if needed BEFORE seeking
        // This ensures the file is large enough before we try to write
        let current_len = file.metadata().map(|m| m.len()).unwrap_or(0);
        if required_len > current_len {
            // Extend file to required size
            file.set_len(required_len)
                .map_err(|e| NativeBackendError::IoError {
                    context: format!(
                        "Failed to extend file to {} bytes for page {}",
                        required_len, page_id
                    ),
                    source: e,
                })?;
            // Sync to ensure the file size is actually updated
            file.sync_all().map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to sync after extending file for page {}", page_id),
                source: e,
            })?;
        }

        // Seek to position
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to seek to page {} offset {}", page_id, offset),
                source: e,
            })?;

        // Write page data
        file.write_all(&page_bytes)
            .map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to write page {}", page_id),
                source: e,
            })?;

        // CRITICAL: Flush and sync to ensure data and metadata are written
        // This is necessary because we're opening a new file handle for each write,
        // and other writers might not see the updated file size otherwise.
        file.sync_all().map_err(|e| NativeBackendError::IoError {
            context: format!("Failed to sync page {}", page_id),
            source: e,
        })?;

        Ok(())
    }

    /// Estimate the compressed size of a node record
    fn estimate_node_size(&self, node: &NodeRecordV3) -> NativeResult<u16> {
        use crate::backend::native::v3::compression::delta::encode_id_delta;
        use crate::backend::native::v3::compression::varint::varint_size;

        let mut size: usize = 0;
        let base_id = 0; // Conservative estimate

        // ID delta (varint, usually 1-4 bytes)
        let delta = encode_id_delta(node.id(), base_id);
        size += varint_size(delta as u64);

        // Flags: 4 bytes (fixed)
        size += 4;

        // kind_offset: varint u16 (usually 1-2 bytes)
        size += varint_size(node.kind_offset as u64);

        // name_offset: varint u16 (usually 1-2 bytes)
        size += varint_size(node.name_offset as u64);

        // data_len: varint u16 (usually 1 byte for small data)
        size += varint_size(node.data_len() as u64);

        // outgoing_cluster_offset: varint u64 (1-10 bytes)
        size += varint_size(node.outgoing_cluster_offset);

        // outgoing_edge_count: varint u32 (usually 1-3 bytes)
        size += varint_size(node.outgoing_edge_count as u64);

        // incoming_cluster_offset: varint u64 (1-10 bytes)
        size += varint_size(node.incoming_cluster_offset);

        // incoming_edge_count: varint u32 (usually 1-3 bytes)
        size += varint_size(node.incoming_edge_count as u64);

        // Inline data OR external offset (8 bytes)
        if let Some(ref data) = node.data_inline {
            size += data.len();
        } else if node.data_external_offset.is_some() {
            size += 8; // External offset is u64 (8 bytes)
        }

        // Ensure we don't overflow u16
        if size > u16::MAX as usize {
            return Err(NativeBackendError::InvalidHeader {
                field: "compressed_size".to_string(),
                reason: format!("compressed size {} exceeds u16::MAX", size),
            });
        }

        Ok(size as u16)
    }

    pub fn has_index(&self) -> bool {
        self.root_page_id != 0
    }

    pub fn root_page_id_pub(&self) -> u64 {
        self.root_page_id
    }

    pub fn tree_height_pub(&self) -> u32 {
        self.tree_height
    }

    /// Get the B+Tree root page ID from the BTreeManager
    /// This reflects the actual root after inserts, which may differ from header's root_index_page
    pub fn btree_root_page_id(&self) -> Option<u64> {
        self.btree_manager.as_ref().and_then(|btree| {
            let root = btree.root_page_id();
            // Only return valid root pages (not 0 or EMPTY_TREE_ROOT)
            if root != 0 && root != u64::MAX {
                Some(root)
            } else {
                None
            }
        })
    }

    /// Get the B+Tree height from the BTreeManager
    pub fn btree_height(&self) -> Option<u32> {
        self.btree_manager.as_ref().and_then(|btree| {
            let height = btree.tree_height();
            if height > 0 { Some(height) } else { None }
        })
    }

    pub fn lookup_page(&mut self, node_id: i64) -> NativeResult<Option<u64>> {
        // Use BTreeManager for lookup if available
        if let Some(ref btree) = self.btree_manager {
            return btree.lookup(node_id);
        }

        // Fallback to direct B+Tree traversal (for backward compatibility)
        if self.root_page_id == 0 {
            return Ok(None);
        }

        let search_key = node_id as u64;
        let mut current_page_id = self.root_page_id;
        let mut depth = 0;

        while depth < self.tree_height as usize + MAX_TREE_HEIGHT as usize {
            let index_page = if let Some(cached) = self.index_cache.get(&current_page_id) {
                cached.clone()
            } else {
                let page_bytes = self.load_page_from_disk(current_page_id)?;
                let index = IndexPage::unpack(&page_bytes)?;
                // LruCache::put enforces capacity and evicts the LRU index page.
                self.index_cache.put(current_page_id, index.clone());
                index
            };

            match index_page {
                IndexPage::Leaf {
                    entries, next_leaf, ..
                } => {
                    let result = IndexPage::binary_search_leaf(&entries, search_key);
                    return match result {
                        Ok(idx) => {
                            if let Some((_, page_id)) = entries.get(idx) {
                                Ok(Some(*page_id))
                            } else {
                                Err(NativeBackendError::InvalidHeader {
                                    field: "btree_leaf".to_string(),
                                    reason: "entry index out of bounds".to_string(),
                                })
                            }
                        }
                        Err(_idx) => {
                            if next_leaf == 0 {
                                Ok(None)
                            } else {
                                current_page_id = next_leaf;
                                continue;
                            }
                        }
                    };
                }
                IndexPage::Internal { keys, children, .. } => {
                    let child_idx = IndexPage::find_child_index(&keys, search_key);
                    if child_idx < children.len() {
                        current_page_id = children[child_idx];
                    } else {
                        return Err(NativeBackendError::InvalidHeader {
                            field: "btree_internal".to_string(),
                            reason: format!("child index {} out of bounds", child_idx),
                        });
                    }
                }
            }

            depth += 1;
        }

        Err(NativeBackendError::InvalidHeader {
            field: "btree_depth".to_string(),
            reason: format!("exceeded maximum depth {}", MAX_TREE_HEIGHT),
        })
    }

    pub fn lookup_node(&mut self, node_id: i64) -> NativeResult<Option<NodeRecordV3>> {
        let page_id = match self.lookup_page(node_id)? {
            Some(pid) => pid,
            None => return Ok(None),
        };

        // Use load_node_page to check dirty_pages first (batch mode support)
        let node_page = self.load_node_page(page_id)?;

        for node in &node_page.nodes {
            if node.id() == node_id {
                return Ok(Some(node.clone()));
            }
        }

        Ok(None)
    }

    fn load_page_from_disk(&mut self, page_id: u64) -> NativeResult<Vec<u8>> {
        // Only count actual disk reads, not cache hits
        // Cache hits are now tracked in load_node_page

        if let Some(cached) = self.page_cache_get(page_id) {
            #[cfg(feature = "v3-forensics")]
            FORENSIC_COUNTERS
                .node_cache_hit_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(cached);
        }

        #[cfg(feature = "v3-forensics")]
        {
            FORENSIC_COUNTERS
                .page_read_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            FORENSIC_COUNTERS
                .node_cache_miss_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        let mut buffer = vec![0u8; DEFAULT_PAGE_SIZE as usize];

        // Use file_coordinator if available (FIXES 10K-node bug)
        if let Some(coordinator) = &self.file_coordinator {
            coordinator.read_page(page_id, &mut buffer)?;
        } else {
            // Fallback to original behavior
            let page_offset = Self::page_offset(page_id);

            let mut file = File::open(&self.db_path).map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to open database file: {}", self.db_path.display()),
                source: e,
            })?;

            file.seek(SeekFrom::Start(page_offset))
                .map_err(|e| NativeBackendError::IoError {
                    context: format!("Failed to seek to page {}", page_id),
                    source: e,
                })?;

            file.read_exact(&mut buffer)
                .map_err(|e| NativeBackendError::IoError {
                    context: format!("Failed to read page {}", page_id),
                    source: e,
                })?;
        }

        self.page_cache_insert(page_id, buffer.clone());

        Ok(buffer)
    }

    fn page_offset(page_id: u64) -> u64 {
        if page_id == 0 {
            return 0;
        }
        let data_page_index = page_id.saturating_sub(1);
        V3_HEADER_SIZE + data_page_index * DEFAULT_PAGE_SIZE
    }

    /// Extract block_id from cached page bytes
    ///
    /// Reads the base_id field (offset 20-27) and computes block_id.
    /// Used for block-aware cache eviction decisions.
    #[inline]
    fn extract_block_id_from_page_bytes(page_bytes: &[u8]) -> Option<i64> {
        use super::page::BLOCK_SIZE;

        if page_bytes.len() < 28 {
            return None;
        }

        // Read base_id from offset 20 (8 bytes, i64 big-endian)
        let base_id = i64::from_be_bytes(page_bytes[20..28].try_into().ok()?);

        // Compute block_id: (base_id - 1) / BLOCK_SIZE
        let block_id = if base_id < 1 {
            0
        } else {
            (base_id - 1) / BLOCK_SIZE
        };

        Some(block_id)
    }

    pub fn clear_cache(&mut self) {
        self.page_cache.write().clear();
        self.index_cache.clear();
    }

    pub fn cache_stats(&self) -> (usize, usize) {
        (self.page_cache.read().len(), self.index_cache.len())
    }

    //=========================================================================
    // Thread-safe page cache helpers
    //=========================================================================

    /// Get page from cache. Updates LRU recency on a hit (takes a write lock)
    /// so that frequently-read pages survive eviction.
    fn page_cache_get(&self, page_id: u64) -> Option<Vec<u8>> {
        let mut cache = self.page_cache.write();
        cache.get(&page_id).cloned()
    }

    /// Insert page into cache (read-only access)
    ///
    /// PROTOTYPE: Updates current_access_block when inserting a page,
    /// enabling block-aware eviction in future cache operations.
    fn page_cache_insert(&self, page_id: u64, data: Vec<u8>) {
        // Update current access block from the page being cached
        if let Some(block_id) = Self::extract_block_id_from_page_bytes(&data) {
            self.current_access_block
                .store(block_id, std::sync::atomic::Ordering::Relaxed);
        }

        // Invalidate unpacked page cache since the page has been modified
        self.unpacked_page_cache_invalidate(page_id);

        let mut cache = self.page_cache.write();
        // LruCache::put handles capacity enforcement and eviction (LRU) automatically.
        cache.put(page_id, data);
    }

    /// Insert page into cache only if not already present (avoids write lock on concurrent hits)
    /// Used on read-only path to reduce lock contention.
    fn page_cache_insert_if_absent(&self, page_id: u64, data: Vec<u8>) {
        // Update current access block from the page being cached
        if let Some(block_id) = Self::extract_block_id_from_page_bytes(&data) {
            self.current_access_block
                .store(block_id, std::sync::atomic::Ordering::Relaxed);
        }

        // Check if already in cache (read lock) before acquiring write lock.
        // peek() does not update recency and works under a shared read lock.
        {
            let cache_read = self.page_cache.read();
            if cache_read.peek(&page_id).is_some() {
                // Another thread already inserted this page, skip write lock
                return;
            }
        }

        // Acquire write lock only when needed
        let mut cache = self.page_cache.write();
        // Double-check in case another thread inserted while we were waiting for write lock
        if cache.peek(&page_id).is_some() {
            return;
        }
        // LruCache::put enforces capacity and evicts the least-recently-used entry.
        cache.put(page_id, data);
    }

    /// Get an unpacked NodePage from cache. Updates LRU recency on a hit.
    fn unpacked_page_cache_get(&self, page_id: u64) -> Option<Arc<NodePage>> {
        let mut cache = self.unpacked_page_cache.write();
        cache.get(&page_id).cloned()
    }

    /// Insert an unpacked NodePage into cache
    fn unpacked_page_cache_insert(&self, page_id: u64, page: Arc<NodePage>) {
        let mut cache = self.unpacked_page_cache.write();
        // LruCache::put enforces capacity (same limit as the raw page cache) and
        // evicts the least-recently-used unpacked page automatically.
        cache.put(page_id, page);
    }

    /// Invalidate unpacked page cache entry (call after modifying a page)
    fn unpacked_page_cache_invalidate(&self, page_id: u64) {
        let mut cache = self.unpacked_page_cache.write();
        cache.pop(&page_id);
    }

    pub fn update_root(&mut self, new_root: u64) {
        self.root_page_id = new_root;
        self.index_cache.clear();
    }

    pub fn update_tree_height(&mut self, new_height: u32) {
        self.tree_height = new_height;
    }

    pub fn is_valid_node_id(&self, node_id: i64) -> bool {
        if node_id <= 0 {
            return false;
        }
        if !self.has_index() {
            return false;
        }
        true
    }

    /// Read-only node lookup that doesn't modify caches - for concurrent reads
    /// This performs a direct B+Tree traversal without updating page/index caches
    pub fn lookup_node_ro(&self, node_id: i64) -> NativeResult<Option<NodeRecordV3>> {
        #[cfg(feature = "v3-forensics")]
        FORENSIC_COUNTERS
            .node_decode_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let page_id = match self.lookup_page_ro(node_id)? {
            Some(pid) => pid,
            None => return Ok(None),
        };

        // Check unpacked page cache first - avoids expensive unpack on cache hits
        if let Some(cached_page) = self.unpacked_page_cache_get(page_id) {
            #[cfg(feature = "v3-forensics")]
            {
                FORENSIC_COUNTERS
                    .node_page_cache_hit_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                FORENSIC_COUNTERS.node_linear_scan_steps.fetch_add(
                    (cached_page.nodes.len().ilog2().max(1)) as u64,
                    std::sync::atomic::Ordering::Relaxed,
                );
            }

            return match cached_page.find_node(node_id) {
                Some(node_ref) => Ok(Some(node_ref.clone())),
                None => Ok(None),
            };
        }

        // Cache miss - load page data and unpack it
        #[cfg(feature = "v3-forensics")]
        FORENSIC_COUNTERS
            .node_page_cache_miss_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let page_data = self.load_page_cache_ro(page_id)?;

        // OPTIMIZATION: Unpack the page and cache it for future accesses
        // This avoids repeated varint decoding on subsequent lookups
        let page = NodePage::unpack(&page_data)?;

        // Do the lookup before moving into cache
        let result = match page.find_node(node_id) {
            Some(node_ref) => Ok(Some(node_ref.clone())),
            None => Ok(None),
        };

        // Insert into unpacked cache for future fast access
        // Note: Move page into Arc, don't clone - we own page from unpack()
        self.unpacked_page_cache_insert(page_id, Arc::new(page));

        result
    }

    /// Load a page for read-only access with cache checking
    /// Now populates cache via Arc<RwLock wrapper
    fn load_page_cache_ro(&self, page_id: u64) -> NativeResult<Vec<u8>> {
        #[cfg(feature = "v3-forensics")]
        FORENSIC_COUNTERS
            .page_read_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Check cache first (now works with Arc<RwLock>)
        if let Some(cached) = self.page_cache_get(page_id) {
            #[cfg(feature = "v3-forensics")]
            FORENSIC_COUNTERS
                .node_page_cache_hit_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            return Ok(cached);
        }

        #[cfg(feature = "v3-forensics")]
        FORENSIC_COUNTERS
            .node_page_cache_miss_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let mut buffer = vec![0u8; DEFAULT_PAGE_SIZE as usize];

        // Use file_coordinator if available (FIXES 10K-node bug)
        if let Some(coordinator) = &self.file_coordinator {
            coordinator.read_page(page_id, &mut buffer)?;
        } else {
            // Fallback to original behavior
            let page_offset = Self::page_offset(page_id);

            let mut file = File::open(&self.db_path).map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to open database file: {}", self.db_path.display()),
                source: e,
            })?;

            file.seek(SeekFrom::Start(page_offset))
                .map_err(|e| NativeBackendError::IoError {
                    context: format!("Failed to seek to page {}", page_id),
                    source: e,
                })?;

            file.read_exact(&mut buffer)
                .map_err(|e| NativeBackendError::IoError {
                    context: format!("Failed to read page {}", page_id),
                    source: e,
                })?;
        }

        // Populate cache for subsequent reads
        // OPTIMIZATION: Only insert if not already present (avoid write lock on concurrent hits)
        self.page_cache_insert_if_absent(page_id, buffer.clone());

        Ok(buffer)
    }

    /// Read-only B+Tree page lookup - doesn't update index_cache
    fn lookup_page_ro(&self, node_id: i64) -> NativeResult<Option<u64>> {
        // Use BTreeManager for lookup if available, otherwise return None
        if let Some(ref btree) = self.btree_manager {
            return btree.lookup(node_id);
        }
        // No index available - node cannot be found
        Ok(None)
    }
}
