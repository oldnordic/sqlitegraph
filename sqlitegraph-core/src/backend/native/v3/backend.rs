//! V3Backend - Native V3 GraphBackend implementation
//!
//! This module implements the GraphBackend trait for V3 storage format with:
//! - B+Tree node index for unlimited capacity
//! - Page-based node storage
//! - Delta/varint compression
//! - Page allocator for dynamic page allocation
//! - Write-Ahead Logging for crash recovery
//!
//! ## Architecture
//!
//! ```text
//! V3Backend {
//!     db_path: PathBuf,           // Database file path
//!     btree: RwLock<BTreeManager>, // B+Tree for node_id → page_id
//!     node_store: RwLock<NodeStore>, // Node storage operations
//!     edge_store: RwLock<V3EdgeStore>, // Edge storage (compat layer)
//!     allocator: RwLock<PageAllocator>, // Page allocation
//!     wal: Option<RwLock<WALWriter>>, // Optional WAL for durability
//!     header: RwLock<PersistentHeaderV3>, // Persistent header
//! }
//! ```

// Note: map_v3_error is defined in graph_validation but not exported.
// We'll define a local mapping function for V3 errors.
use crate::SqliteGraphError;
use crate::backend::native::types::NativeBackendError;
use crate::backend::native::v3::btree::BTreeManager;
use crate::backend::native::v3::edge_compat::Direction as EdgeDirection;
use crate::backend::native::v3::name_index::NameIndex;
use crate::backend::native::v3::storage::AdaptivePageManager;
use crate::backend::native::v3::wal::{V3WALPaths, V3WALRecord, WALWriter};
use crate::backend::native::v3::{
    KindIndex, KvStore, KvValue, NodeCache, NodeRecordV3, NodeStore, PageAllocator,
    PersistentHeaderV3, Publisher, V3_HEADER_SIZE, V3EdgeStore,
};
use crate::backend::{
    BackendDirection, ChainStep, EdgeSpec, GraphBackend, NeighborQuery, NodeSpec, PatternMatch,
    PatternQuery,
};
use crate::graph::GraphEntity;
use crate::snapshot::SnapshotId;
use parking_lot::RwLock;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// V3 Backend implementation with interior mutability
///
/// This struct implements the GraphBackend trait using V3's page-based
/// storage with B+Tree indexing for O(log n) node lookups.
///
/// ## Lazy Initialization
///
/// The KV store and Pub/Sub publisher are lazily initialized:
/// - `kv_store`: Created on first KV operation (get/set/delete)
/// - `publisher`: Created on first subscription
///
/// This reduces memory overhead for use cases that only need graph operations.
pub struct V3Backend {
    /// Database file path
    db_path: PathBuf,
    /// BTreeManager for node_id → page_id lookups
    btree: RwLock<BTreeManager>,
    /// NodeStore for node operations
    node_store: RwLock<NodeStore>,
    /// EdgeStore for edge operations (compat layer)
    edge_store: RwLock<V3EdgeStore>,
    /// Page allocator for dynamic page allocation (shared between BTreeManager and NodeStore)
    allocator: Arc<RwLock<PageAllocator>>,
    /// Optional WAL writer for durability (shared with edge store via Arc)
    wal: Option<Arc<RwLock<WALWriter>>>,
    /// Persistent header
    header: RwLock<PersistentHeaderV3>,
    /// KV store for key-value operations (lazy initialized)
    kv_store: RwLock<Option<KvStore>>,
    /// Pub/Sub publisher for event notification (lazy initialized)
    publisher: RwLock<Option<Publisher>>,
    /// Kind index for O(1) kind-based lookups
    kind_index: KindIndex,
    /// Name index for fast name lookups (exact, prefix, substring)
    name_index: NameIndex,
    /// LRU cache for node records (2-3× point lookup improvement)
    node_cache: NodeCache,
}

/// Write batch guard for amortized durability
///
/// Accumulates node/edge inserts in memory and performs a single
/// fsync at commit, matching SQLite in-transaction semantics.
pub struct WriteBatchGuard<'a> {
    backend: &'a V3Backend,
    node_count: u64,
    edge_count: u64,
    committed: bool,
}

impl<'a> WriteBatchGuard<'a> {
    /// Create a new write batch guard
    fn new(backend: &'a V3Backend) -> Self {
        // Enable batch mode on node_store
        {
            let mut node_store = backend.node_store.write();
            node_store.begin_batch();
        }

        Self {
            backend,
            node_count: 0,
            edge_count: 0,
            committed: false,
        }
    }

    /// Insert a node without syncing (accumulated in batch)
    pub fn insert_node(&mut self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        // Use inner insert that doesn't sync
        let node_id = self.backend.insert_node_inner(node)?;
        self.node_count += 1;
        Ok(node_id)
    }

    /// Insert an edge without syncing (accumulated in batch)
    pub fn insert_edge(&mut self, edge: EdgeSpec) -> Result<i64, SqliteGraphError> {
        // Use inner insert that doesn't sync
        let edge_id = self.backend.insert_edge_inner(edge)?;
        self.edge_count += 1;
        Ok(edge_id)
    }

    /// Commit all accumulated writes with single fsync
    pub fn commit(mut self) -> Result<(), SqliteGraphError> {
        if self.committed {
            return Ok(());
        }

        // Commit node_store batch (single fsync for all dirty pages)
        if self.node_count > 0 {
            let mut node_store = self.backend.node_store.write();
            node_store
                .commit_batch()
                .map_err(|e| SqliteGraphError::connection(format!("Batch commit failed: {}", e)))?;
        }

        // Sync header and WAL once for the entire batch
        if self.node_count > 0 || self.edge_count > 0 {
            self.backend.sync_header()?;
            self.backend.flush_to_disk()?;
        }

        self.committed = true;
        Ok(())
    }

    /// Get number of nodes staged in this batch
    pub fn node_count(&self) -> u64 {
        self.node_count
    }

    /// Get number of edges staged in this batch
    pub fn edge_count(&self) -> u64 {
        self.edge_count
    }
}

impl<'a> Drop for WriteBatchGuard<'a> {
    fn drop(&mut self) {
        if !self.committed {
            // Rollback batch mode
            let mut node_store = self.backend.node_store.write();
            node_store.rollback_batch();
        }
    }
}

impl Drop for V3Backend {
    fn drop(&mut self) {
        // Flush any pending data to disk
        // This ensures data is persisted when the backend is dropped
        let _ = self.flush_to_disk();

        // Sync header to ensure all metadata is written
        let _ = self.sync_header();
    }
}

/// Map NativeBackendError to SqliteGraphError
fn map_v3_error(err: NativeBackendError) -> SqliteGraphError {
    match err {
        NativeBackendError::Io(e) => SqliteGraphError::connection(e.to_string()),
        NativeBackendError::SerializationError { context } => {
            SqliteGraphError::connection(format!("Serialization error: {}", context))
        }
        NativeBackendError::DeserializationError { context } => {
            SqliteGraphError::connection(format!("Deserialization error: {}", context))
        }
        NativeBackendError::InvalidNodeId { id, max_id } => {
            SqliteGraphError::query(format!("Invalid node ID: {} (max: {})", id, max_id))
        }
        NativeBackendError::InvalidEdgeId { id, max_id } => {
            SqliteGraphError::query(format!("Invalid edge ID: {} (max: {})", id, max_id))
        }
        NativeBackendError::CorruptNodeRecord { node_id, reason } => {
            SqliteGraphError::connection(format!("Corrupt node record {}: {}", node_id, reason))
        }
        NativeBackendError::CorruptEdgeRecord { edge_id, reason } => {
            SqliteGraphError::connection(format!("Corrupt edge record {}: {}", edge_id, reason))
        }
        NativeBackendError::InvalidMagic { expected, found } => SqliteGraphError::connection(
            format!("Invalid magic: expected {}, found {}", expected, found),
        ),
        NativeBackendError::UnsupportedVersion {
            version,
            supported_version,
        } => SqliteGraphError::connection(format!(
            "Unsupported version: {} (supported: {})",
            version, supported_version
        )),
        NativeBackendError::InvalidHeader { field, reason } => {
            SqliteGraphError::connection(format!("Invalid header field '{}': {}", field, reason))
        }
        NativeBackendError::InvalidChecksum { expected, found } => SqliteGraphError::connection(
            format!("Checksum mismatch: expected {}, found {}", expected, found),
        ),
        NativeBackendError::RecordTooLarge { size, max_size } => {
            SqliteGraphError::connection(format!("Record too large: {} (max: {})", size, max_size))
        }
        NativeBackendError::BincodeError(e) => {
            SqliteGraphError::connection(format!("Bincode error: {}", e))
        }
        _ => SqliteGraphError::connection(format!("Native backend error: {:?}", err)),
    }
}

impl V3Backend {
    /// Check if snapshot is supported by V3 backend.
    ///
    /// V3 does not implement MVCC/snapshot isolation — all reads see the
    /// current committed state regardless of the snapshot ID passed.
    fn require_current_snapshot(_snapshot_id: SnapshotId) -> Result<(), SqliteGraphError> {
        Ok(())
    }

    /// Parse node data from compact format: [kind_len: u8][kind bytes][name_len: u8][name bytes][json data]
    fn parse_node_data(data: &[u8], id: i64) -> (String, String, serde_json::Value) {
        if data.len() < 2 {
            return (
                "Node".to_string(),
                format!("node_{}", id),
                serde_json::json!({}),
            );
        }

        let kind_len = data[0] as usize;
        if data.len() < 1 + kind_len + 1 {
            return (
                "Node".to_string(),
                format!("node_{}", id),
                serde_json::json!({}),
            );
        }
        let kind = String::from_utf8_lossy(&data[1..1 + kind_len]).to_string();

        let name_len_pos = 1 + kind_len;
        let name_len = data[name_len_pos] as usize;
        if data.len() < name_len_pos + 1 + name_len {
            return (kind, format!("node_{}", id), serde_json::json!({}));
        }
        let name_start = name_len_pos + 1;
        let name = String::from_utf8_lossy(&data[name_start..name_start + name_len]).to_string();

        let data_start = name_start + name_len;
        let json_data = if data_start < data.len() {
            serde_json::from_slice(&data[data_start..]).unwrap_or_else(|_| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        (kind, name, json_data)
    }

    /// Create a new V3 database at the specified path
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the database file will be created
    ///
    /// # Returns
    ///
    /// * `Ok(V3Backend)` - Newly created backend
    /// * `Err(SqliteGraphError)` - If creation fails
    ///
    /// # Example
    ///
    /// ```ignore
    /// let backend = V3Backend::create("/path/to/db.graph")?;
    /// ```
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Self, SqliteGraphError> {
        let db_path = path.as_ref().to_path_buf();

        // Detect optimal page size based on storage media (SSD vs HDD)
        let mut adaptive_manager = AdaptivePageManager::new(&db_path);
        let page_config = adaptive_manager.get_config();

        // Create initial header with detected page size
        let mut header = PersistentHeaderV3::new_v3();
        header.page_size = page_config.page_size;

        // Write header to file
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&db_path)
            .map_err(|e| {
                SqliteGraphError::connection(format!("Failed to create database file: {}", e))
            })?;

        let header_bytes = header.to_bytes();
        file.write_all(&header_bytes)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to write header: {}", e)))?;
        file.sync_all()
            .map_err(|e| SqliteGraphError::connection(format!("Failed to sync file: {}", e)))?;

        // Initialize components with shared allocator
        let allocator = Arc::new(RwLock::new(PageAllocator::new(&header)));
        let btree = BTreeManager::new(Arc::clone(&allocator), None, db_path.clone());
        let mut node_store = NodeStore::new(&header, db_path.clone());
        // Initialize NodeStore with shared BTreeManager and PageAllocator
        node_store.initialize(btree.clone(), Arc::clone(&allocator), None);
        let edge_store = V3EdgeStore::with_path_and_allocator(
            btree.clone(),
            None,
            db_path.clone(),
            Arc::clone(&allocator),
            header.page_size,
        );

        Ok(Self {
            db_path,
            btree: RwLock::new(btree),
            node_store: RwLock::new(node_store),
            edge_store: RwLock::new(edge_store),
            allocator,
            wal: None,
            header: RwLock::new(header),
            kv_store: RwLock::new(None),  // Lazy initialized
            publisher: RwLock::new(None), // Lazy initialized
            kind_index: KindIndex::new(),
            name_index: NameIndex::new(),
            node_cache: NodeCache::new(
                crate::backend::native::v3::constants::node_cache::DEFAULT_CACHE_CAPACITY,
            ),
        })
    }

    /// Create a new V3 database with WAL enabled
    ///
    /// # Arguments
    ///
    /// * `path` - Path where the database file will be created
    /// * `enable_wal` - Whether to enable write-ahead logging
    ///
    /// # Returns
    ///
    /// * `Ok(V3Backend)` - Newly created backend
    /// * `Err(SqliteGraphError)` - If creation fails
    pub fn create_with_wal<P: AsRef<Path>>(
        path: P,
        enable_wal: bool,
    ) -> Result<Self, SqliteGraphError> {
        let mut backend = Self::create(path)?;

        if enable_wal {
            let wal_path = V3WALPaths::wal_file(&backend.db_path);
            let wal_writer = WALWriter::new(wal_path, 1).map_err(|e| {
                SqliteGraphError::connection(format!("Failed to create WAL: {:?}", e))
            })?;
            wal_writer.write_header().map_err(|e| {
                SqliteGraphError::connection(format!("Failed to write WAL header: {:?}", e))
            })?;
            backend.wal = Some(Arc::new(RwLock::new(wal_writer)));
        }

        Ok(backend)
    }

    /// Open an existing V3 database from the specified path
    ///
    /// # Arguments
    ///
    /// * `path` - Path to the existing database file
    ///
    /// # Returns
    ///
    /// * `Ok(V3Backend)` - Opened backend
    /// * `Err(SqliteGraphError)` - If opening fails or file is not a valid V3 database
    ///
    /// # Example
    ///
    /// ```ignore
    /// let backend = V3Backend::open("/path/to/db.graph")?;
    /// ```
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, SqliteGraphError> {
        let db_path = path.as_ref().to_path_buf();

        // Check if file exists
        if !db_path.exists() {
            return Err(SqliteGraphError::connection(format!(
                "Database file does not exist: {}",
                db_path.display()
            )));
        }

        // Read header from file
        let mut file = File::open(&db_path).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to open database file: {}", e))
        })?;

        let mut header_bytes = vec![0u8; V3_HEADER_SIZE as usize];
        file.read_exact(&mut header_bytes)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to read header: {}", e)))?;

        // Parse and validate header
        let header = PersistentHeaderV3::from_bytes(&header_bytes).map_err(map_v3_error)?;
        header.validate().map_err(map_v3_error)?;

        // Initialize components with shared allocator
        let allocator = Arc::new(RwLock::new(PageAllocator::new(&header)));
        let btree = BTreeManager::with_root(
            Arc::clone(&allocator),
            None,
            header.root_index_page,
            header.btree_height,
            db_path.clone(),
        );
        let mut node_store = NodeStore::new(&header, db_path.clone());
        // Initialize NodeStore with shared BTreeManager and PageAllocator
        node_store.initialize(
            BTreeManager::with_root(
                Arc::clone(&allocator),
                None,
                header.root_index_page,
                header.btree_height,
                db_path.clone(),
            ),
            Arc::clone(&allocator),
            None,
        );
        let mut edge_store = V3EdgeStore::with_path_and_allocator(
            BTreeManager::with_root(
                Arc::clone(&allocator),
                None,
                header.edge_data_offset,
                header.btree_height,
                db_path.clone(),
            ),
            None,
            db_path.clone(),
            Arc::clone(&allocator),
            header.page_size,
        );
        // Attempt to recover edge B+Tree from metadata sidecar
        let _ = edge_store.restore_btree_from_metadata();

        // Check for existing WAL
        let wal_path = V3WALPaths::wal_file(&db_path);
        let wal: Option<Arc<RwLock<WALWriter>>> = if wal_path.exists() {
            let wal_writer = WALWriter::new(wal_path, 1).map_err(|e| {
                SqliteGraphError::connection(format!("Failed to open WAL: {:?}", e))
            })?;
            Some(Arc::new(RwLock::new(wal_writer)))
        } else {
            None
        };

        if let Some(ref wal_arc) = wal {
            edge_store.set_wal(Arc::clone(wal_arc));
        }

        // Initialize indexes: try to restore from .v3index sidecar, fall back to rebuild
        let (kind_index, name_index) =
            match crate::backend::native::v3::index_persistence::restore_indexes(
                &db_path,
                header.node_count,
            ) {
                Ok((kind, name)) => (kind, name),
                Err(_) => {
                    // No sidecar or stale — will rebuild below
                    (KindIndex::new(), NameIndex::new())
                }
            };

        let backend = Self {
            db_path: db_path.clone(),
            btree: RwLock::new(btree),
            node_store: RwLock::new(node_store),
            edge_store: RwLock::new(edge_store),
            allocator,
            wal,
            header: RwLock::new(header),
            kv_store: RwLock::new(None),  // Lazy initialized
            publisher: RwLock::new(None), // Lazy initialized
            kind_index,
            name_index,
            node_cache: NodeCache::new(
                crate::backend::native::v3::constants::node_cache::DEFAULT_CACHE_CAPACITY,
            ),
        };

        // If restore failed, rebuild indexes by scanning all nodes
        if backend.kind_index.kind_count() == 0 && backend.header.read().node_count > 0 {
            backend.rebuild_indexes();
        }

        Ok(backend)
    }

    /// Check if KV store has been initialized
    pub fn is_kv_initialized(&self) -> bool {
        self.kv_store.read().is_some()
    }

    /// Check if Publisher has been initialized
    pub fn is_pubsub_initialized(&self) -> bool {
        self.publisher.read().is_some()
    }

    // === V3-Native Public API ===

    /// Get a value from the KV store
    ///
    /// Returns None if the key doesn't exist or has been deleted (tombstone).
    pub fn kv_get_v3(&self, snapshot_id: SnapshotId, key: &[u8]) -> Option<KvValue> {
        let kv_guard = self.kv_store.read();
        kv_guard.as_ref().and_then(|kv| {
            kv.get_at_snapshot(key, snapshot_id)
                .filter(|v| !matches!(v, KvValue::Null))
        })
    }

    /// Set a value in the KV store
    pub fn kv_set_v3(&self, key: Vec<u8>, value: KvValue, ttl_seconds: Option<u64>) {
        let version = if let Some(ref wal) = self.wal {
            let wal_guard = wal.read();
            wal_guard.committed_lsn()
        } else {
            1
        };

        let mut kv_guard = self.kv_store.write();
        kv_guard
            .get_or_insert_with(KvStore::new)
            .set(key, value, ttl_seconds, version);
    }

    /// Delete a key from the KV store
    pub fn kv_delete_v3(&self, key: &[u8]) {
        let version = if let Some(ref wal) = self.wal {
            let wal_guard = wal.read();
            wal_guard.committed_lsn()
        } else {
            1
        };

        let mut kv_guard = self.kv_store.write();
        kv_guard
            .get_or_insert_with(KvStore::new)
            .delete(key, version);
    }

    /// Prefix scan for keys in the KV store using V3 types
    ///
    /// Returns all key-value pairs where the key starts with the given prefix.
    /// This method works directly with V3 KvValue types and does not require
    /// the native-v2 feature to be enabled.
    ///
    /// # Arguments
    ///
    /// * `snapshot_id` - The snapshot to read from
    /// * `prefix` - The prefix to match
    ///
    /// # Returns
    ///
    /// A vector of (key, value) pairs where keys match the prefix
    pub fn kv_prefix_scan_v3(
        &self,
        snapshot_id: SnapshotId,
        prefix: &[u8],
    ) -> Vec<(Vec<u8>, KvValue)> {
        let kv_guard = self.kv_store.read();
        kv_guard
            .as_ref()
            .map(|kv| kv.prefix_scan(prefix, snapshot_id))
            .unwrap_or_default()
    }

    /// Get node by ID (internal method)
    ///
    /// Looks up a node record by its ID using the B+Tree index.
    ///
    /// # Arguments
    ///
    /// * `node_id` - The ID of the node to retrieve
    ///
    /// # Returns
    ///
    /// * `Ok(Some(NodeRecordV3))` - Node found
    /// * `Ok(None)` - Node not found
    /// * `Err(SqliteGraphError)` - Error during lookup
    fn get_node_internal(&self, node_id: i64) -> Result<Option<NodeRecordV3>, SqliteGraphError> {
        // Try cache first
        if let Some(record) = self.node_cache.get(node_id) {
            return Ok(Some(record));
        }

        // Cache miss - look up from storage
        let mut node_store = self.node_store.write();
        if let Some(record) = node_store.lookup_node(node_id).map_err(map_v3_error)? {
            // Populate cache for future access
            self.node_cache.insert(node_id, record.clone());
            Ok(Some(record))
        } else {
            Ok(None)
        }
    }

    /// Get a reference to the database path
    pub fn db_path(&self) -> &Path {
        &self.db_path
    }

    /// Check if WAL is enabled
    pub fn is_wal_enabled(&self) -> bool {
        self.wal.is_some()
    }

    /// Get the current header state
    pub fn header(&self) -> PersistentHeaderV3 {
        self.header.read().clone()
    }

    /// Flush any pending writes to disk
    pub fn flush_to_disk(&self) -> Result<(), SqliteGraphError> {
        // Flush edge store dirty clusters to disk
        let edge_store = self.edge_store.write();
        edge_store.flush(None).map_err(map_v3_error)?;
        drop(edge_store);

        // Update header with edge store B+Tree root
        let mut header = self.header.write();
        if let Some(root_page) = self.edge_store.read().btree_root_page_id() {
            header.edge_data_offset = root_page;
        }
        let node_count = header.node_count;
        drop(header);

        // Persist kind and name indexes to .v3index sidecar
        let _ = crate::backend::native::v3::index_persistence::persist_indexes(
            &self.db_path,
            &self.kind_index,
            &self.name_index,
            node_count,
        );

        // Sync header so edge B+Tree root is durable
        self.sync_header()?;
        if let Some(ref wal) = self.wal {
            wal.write()
                .flush()
                .map_err(|e| SqliteGraphError::connection(format!("WAL flush failed: {:?}", e)))?;
        }
        Ok(())
    }

    /// Sync header to disk
    fn sync_header(&self) -> Result<(), SqliteGraphError> {
        let header = self.header.read();
        let header_bytes = header.to_bytes();

        let mut file = OpenOptions::new()
            .write(true)
            .open(&self.db_path)
            .map_err(|e| {
                SqliteGraphError::connection(format!("Failed to open file for header sync: {}", e))
            })?;

        file.seek(SeekFrom::Start(0)).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to seek to header: {}", e))
        })?;
        file.write_all(&header_bytes)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to write header: {}", e)))?;
        file.sync_all()
            .map_err(|e| SqliteGraphError::connection(format!("Failed to sync header: {}", e)))?;

        Ok(())
    }

    /// Begin a write batch for amortized durability
    ///
    /// Returns a WriteBatchGuard that accumulates inserts without syncing.
    /// Call `commit()` on the guard to persist all changes with a single fsync.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let mut batch = backend.begin_batch();
    /// for i in 0..1000 {
    ///     batch.insert_node(NodeSpec { ... })?;
    /// }
    /// batch.commit()?; // Single fsync for all 1000 inserts
    /// ```
    pub fn begin_batch(&self) -> WriteBatchGuard<'_> {
        WriteBatchGuard::new(self)
    }

    /// Insert node without syncing (internal use only)
    ///
    /// Used by WriteBatchGuard to accumulate changes.
    /// Marked pub for benchmark access.
    pub fn insert_node_inner(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        let kind_bytes = node.kind.as_bytes();
        let name_bytes = node.name.as_bytes();
        let data_bytes = serde_json::to_vec(&node.data).unwrap_or_default();

        let total_len = 2 + kind_bytes.len() + name_bytes.len() + data_bytes.len();

        // Check if data fits inline (MAX_INLINE_DATA = 64 bytes)
        const MAX_INLINE_DATA: usize = 64;

        let node_record = if total_len <= MAX_INLINE_DATA {
            // Small data: store inline
            let mut inline_data = Vec::with_capacity(total_len);
            inline_data.push(kind_bytes.len() as u8);
            inline_data.extend_from_slice(kind_bytes);
            inline_data.push(name_bytes.len() as u8);
            inline_data.extend_from_slice(name_bytes);
            inline_data.extend_from_slice(&data_bytes);

            NodeRecordV3::new_inline(
                0,
                crate::backend::native::types::NodeFlags::empty(),
                0,
                0,
                inline_data,
                0,
                0,
                0,
                0,
            )
        } else {
            // Large data: store externally
            // Format: [kind_len:1][kind][name_len:1][name][data...]
            let mut external_data = Vec::with_capacity(total_len);
            external_data.push(kind_bytes.len() as u8);
            external_data.extend_from_slice(kind_bytes);
            external_data.push(name_bytes.len() as u8);
            external_data.extend_from_slice(name_bytes);
            external_data.extend_from_slice(&data_bytes);

            // Allocate page(s) for external data
            let data_len = external_data.len();
            let page_size = crate::backend::native::v3::constants::DEFAULT_PAGE_SIZE as usize;
            let pages_needed = (data_len + page_size - 1) / page_size; // Ceiling division

            let mut allocator = self.allocator.write();
            let start_page_id = allocator
                .allocate()
                .map_err(SqliteGraphError::NativeError)?;

            // Allocate additional pages if needed
            for _ in 1..pages_needed {
                allocator
                    .allocate()
                    .map_err(SqliteGraphError::NativeError)?;
            }

            // Write external data to file
            let offset = Self::page_offset(start_page_id);

            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(&self.db_path)
                .map_err(|e| {
                    SqliteGraphError::ConnectionError(format!("Failed to open file: {}", e))
                })?;

            file.seek(SeekFrom::Start(offset))
                .map_err(|e| SqliteGraphError::ConnectionError(format!("Failed to seek: {}", e)))?;
            file.write_all(&external_data).map_err(|e| {
                SqliteGraphError::ConnectionError(format!("Failed to write: {}", e))
            })?;
            file.sync_all().map_err(|e| {
                SqliteGraphError::ConnectionError(format!("Failed to sync external data: {}", e))
            })?;

            // Create external node record
            // Use offset as the external data reference
            NodeRecordV3::new_external(
                0,
                crate::backend::native::types::NodeFlags::empty(),
                0,
                0,
                offset, // External data offset
                data_len as u16,
                0,
                0,
                0,
                0,
            )
        };

        let mut node_store = self.node_store.write();
        let node_id = node_store.insert_node(node_record).map_err(map_v3_error)?;

        // Update indexes with kind and name from the inserted node
        self.kind_index.insert(node.kind.clone(), node_id);
        self.name_index.insert(node.name.clone(), node_id);

        // Update header node count and B+Tree root info (but don't sync yet)
        let mut header = self.header.write();
        header.node_count += 1;

        // Sync B+Tree root page ID and height from NodeStore's BTreeManager
        if let Some(root_page) = node_store.btree_root_page_id() {
            header.root_index_page = root_page;
        }
        if let Some(tree_height) = node_store.btree_height() {
            header.btree_height = tree_height;
        }

        Ok(node_id)
    }

    /// Calculate file offset for a given page ID
    ///
    /// Page 0 is the header at offset 0.
    /// Data pages start at page 1, which maps to offset V3_HEADER_SIZE.
    fn page_offset(page_id: u64) -> u64 {
        if page_id == 0 {
            return 0;
        }
        let data_page_index = page_id.saturating_sub(1);
        crate::backend::native::v3::constants::V3_HEADER_SIZE
            + data_page_index * crate::backend::native::v3::constants::DEFAULT_PAGE_SIZE
    }

    /// Rebuild kind and name indexes by scanning all nodes
    ///
    /// This is used as a fallback when index restoration from .v3index fails
    /// or when opening a database without a sidecar index file.
    fn rebuild_indexes(&self) {
        self.kind_index.clear();
        self.name_index.clear();

        let header = self.header.read();
        let node_count = header.node_count;
        drop(header);

        for id in 1..=node_count as i64 {
            if let Ok(Some(record)) = self.get_node_internal(id) {
                let data_bytes = if let Some(inline) = record.data_inline {
                    inline
                } else if let Some(offset) = record.data_external_offset {
                    let actual_data_len = record.data_len
                        & crate::backend::native::v3::node::record::constants::MAX_DATA_LEN;
                    let mut buffer = vec![0u8; actual_data_len as usize];
                    if let Ok(mut file) = OpenOptions::new().read(true).open(&self.db_path) {
                        if file.seek(SeekFrom::Start(offset)).is_ok() {
                            let _ = file.read_exact(&mut buffer);
                        }
                    }
                    buffer
                } else {
                    Vec::new()
                };

                let (kind, name, _data) = Self::parse_node_data(&data_bytes, id);
                self.kind_index.insert(kind, id);
                self.name_index.insert(name, id);
            }
        }
    }

    /// Insert edge without syncing (internal use only)
    ///
    /// Used by WriteBatchGuard to accumulate changes.
    fn insert_edge_inner(&self, edge: EdgeSpec) -> Result<i64, SqliteGraphError> {
        let edge_store = self.edge_store.write();

        let edge_type = if edge.edge_type.is_empty() {
            None
        } else {
            Some(edge.edge_type.clone())
        };

        edge_store
            .insert_edge(
                edge.from,
                edge.to,
                EdgeDirection::Outgoing,
                edge_type.clone(),
            )
            .map_err(map_v3_error)?;
        edge_store
            .insert_edge(edge.to, edge.from, EdgeDirection::Incoming, edge_type)
            .map_err(map_v3_error)?;

        // Update header edge count (but don't sync yet)
        let mut header = self.header.write();
        header.edge_count += 1;

        // Return a synthetic edge ID (edge store doesn't assign IDs yet)
        Ok(header.edge_count as i64)
    }
}

impl GraphBackend for V3Backend {
    /// Check if snapshot is supported by V3 backend.
    ///
    /// V3 only supports SnapshotId::current() (which returns 0).
    /// Any historical/non-zero snapshot ID is rejected with a clear error.
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        // Use inner method - do NOT auto-flush to allow batching multiple nodes
        // Caller must call flush() for durability
        let node_id = self.insert_node_inner(node)?;
        self.sync_header()?;
        Ok(node_id)
    }

    fn insert_edge(&self, edge: EdgeSpec) -> Result<i64, SqliteGraphError> {
        // Use inner method - do NOT auto-flush to allow batching multiple edges
        // Caller must call flush() for durability
        let edge_id = self.insert_edge_inner(edge)?;
        self.sync_header()?;
        Ok(edge_id)
    }

    fn update_node(&self, node_id: i64, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        // Create updated node record
        let updated_record = NodeRecordV3::new_inline(
            node_id,
            crate::backend::native::types::NodeFlags::empty(),
            0, // TODO: kind_offset
            0, // TODO: name_offset
            serde_json::to_vec(&node.data).unwrap_or_default(),
            0, // outgoing_cluster_offset
            0, // outgoing_edge_count
            0, // incoming_cluster_offset
            0, // incoming_edge_count
        );

        let mut node_store = self.node_store.write();
        node_store
            .update_node(node_id, updated_record)
            .map_err(map_v3_error)?;

        // Invalidate cache entry
        self.node_cache.invalidate(node_id);

        self.flush_to_disk()?;

        Ok(node_id)
    }

    fn delete_entity(&self, id: i64) -> Result<(), SqliteGraphError> {
        let mut node_store = self.node_store.write();
        node_store.delete_node(id).map_err(map_v3_error)?;

        // Invalidate cache entry
        self.node_cache.invalidate(id);

        // Update header
        {
            let mut header = self.header.write();
            header.node_count = header.node_count.saturating_sub(1);
        }
        self.sync_header()?;

        self.flush_to_disk()?;

        Ok(())
    }

    fn entity_ids(&self) -> Result<Vec<i64>, SqliteGraphError> {
        // For now, scan all possible node IDs
        // In production, this would use a B+Tree range scan
        let header = self.header.read();
        let mut ids = Vec::new();

        for id in 1..=header.node_count as i64 {
            if self.get_node_internal(id)?.is_some() {
                ids.push(id);
            }
        }

        Ok(ids)
    }

    fn get_node(&self, snapshot_id: SnapshotId, id: i64) -> Result<GraphEntity, SqliteGraphError> {
        Self::require_current_snapshot(snapshot_id)?;
        match self.get_node_internal(id)? {
            Some(record) => {
                // Parse compact format: [kind_len: u8][kind bytes][name_len: u8][name bytes][json data]
                let data_bytes = if let Some(inline) = record.data_inline {
                    inline
                } else if let Some(offset) = record.data_external_offset {
                    // Read external data from file
                    // Mask out the external flag to get actual data length
                    let actual_data_len = record.data_len
                        & crate::backend::native::v3::node::record::constants::MAX_DATA_LEN;
                    let mut file =
                        OpenOptions::new()
                            .read(true)
                            .open(&self.db_path)
                            .map_err(|e| {
                                SqliteGraphError::connection(format!("Failed to open file: {}", e))
                            })?;

                    let mut buffer = vec![0u8; actual_data_len as usize];
                    file.seek(SeekFrom::Start(offset)).map_err(|e| {
                        SqliteGraphError::connection(format!("Failed to seek: {}", e))
                    })?;
                    file.read_exact(&mut buffer).map_err(|e| {
                        SqliteGraphError::connection(format!("Failed to read: {}", e))
                    })?;
                    buffer
                } else {
                    Vec::new()
                };

                let (kind, name, data) = Self::parse_node_data(&data_bytes, id);

                Ok(GraphEntity {
                    id,
                    kind,
                    name,
                    file_path: None, // TODO: Add file_path to compact format if needed
                    data,
                })
            }
            None => Err(SqliteGraphError::query(format!("Node {} not found", id))),
        }
    }

    fn neighbors(
        &self,
        snapshot_id: SnapshotId,
        node: i64,
        query: NeighborQuery,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        Self::require_current_snapshot(snapshot_id)?;
        let edge_store = self.edge_store.read();

        let neighbors_arc = if let Some(ref edge_type) = query.edge_type {
            let dir = match query.direction {
                BackendDirection::Outgoing => EdgeDirection::Outgoing,
                BackendDirection::Incoming => EdgeDirection::Incoming,
            };
            edge_store
                .neighbors_filtered(node, dir, edge_type)
                .map_err(map_v3_error)?
        } else {
            match query.direction {
                BackendDirection::Outgoing => edge_store.outgoing(node).map_err(map_v3_error)?,
                BackendDirection::Incoming => edge_store.incoming(node).map_err(map_v3_error)?,
            }
        };

        // Convert Arc<[i64]> to Vec<i64> for the API
        Ok(neighbors_arc.to_vec())
    }

    fn bfs(
        &self,
        _snapshot_id: SnapshotId,
        start: i64,
        depth: u32,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        use std::collections::{HashSet, VecDeque};

        let mut visited = HashSet::new();
        let mut result = Vec::new();
        let mut queue = VecDeque::new();

        visited.insert(start);
        queue.push_back((start, 0));

        while let Some((node_id, current_depth)) = queue.pop_front() {
            if current_depth > depth {
                continue;
            }

            result.push(node_id);

            if current_depth < depth {
                let edge_store = self.edge_store.write();
                let neighbors = edge_store.outgoing(node_id).map_err(map_v3_error)?;

                for neighbor in neighbors.iter() {
                    if visited.insert(*neighbor) {
                        queue.push_back((*neighbor, current_depth + 1));
                    }
                }
            }
        }

        Ok(result)
    }

    fn shortest_path(
        &self,
        _snapshot_id: SnapshotId,
        start: i64,
        end: i64,
    ) -> Result<Option<Vec<i64>>, SqliteGraphError> {
        use std::collections::{HashMap, VecDeque};

        if start == end {
            return Ok(Some(vec![start]));
        }

        let mut visited = HashMap::new();
        let mut queue = VecDeque::new();

        visited.insert(start, None);
        queue.push_back(start);

        while let Some(node_id) = queue.pop_front() {
            let edge_store = self.edge_store.write();
            let neighbors = edge_store.outgoing(node_id).map_err(map_v3_error)?;

            for neighbor in neighbors.iter() {
                if !visited.contains_key(neighbor) {
                    visited.insert(*neighbor, Some(node_id));

                    if *neighbor == end {
                        // Reconstruct path
                        let mut path = vec![end];
                        let mut current = node_id;

                        while let Some(&parent) = visited.get(&current) {
                            path.push(current);
                            match parent {
                                Some(p) => current = p,
                                None => break,
                            }
                        }

                        path.reverse();
                        return Ok(Some(path));
                    }

                    queue.push_back(*neighbor);
                }
            }
        }

        Ok(None)
    }

    fn node_degree(
        &self,
        _snapshot_id: SnapshotId,
        node: i64,
    ) -> Result<(usize, usize), SqliteGraphError> {
        let edge_store = self.edge_store.write();

        let outgoing = edge_store.outgoing(node).map_err(map_v3_error)?.len();
        let incoming = edge_store.incoming(node).map_err(map_v3_error)?.len();

        Ok((outgoing, incoming))
    }

    fn k_hop(
        &self,
        _snapshot_id: SnapshotId,
        start: i64,
        depth: u32,
        direction: BackendDirection,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        // For k_hop, we use BFS with direction filtering
        use std::collections::{HashSet, VecDeque};

        let mut visited = HashSet::new();
        let mut result = Vec::new();
        let mut queue = VecDeque::new();

        visited.insert(start);
        queue.push_back((start, 0));

        while let Some((node_id, current_depth)) = queue.pop_front() {
            if current_depth > depth {
                continue;
            }

            if current_depth > 0 || depth == 0 {
                result.push(node_id);
            }

            if current_depth < depth {
                let neighbors = match direction {
                    BackendDirection::Outgoing => {
                        let edge_store = self.edge_store.write();
                        edge_store.outgoing(node_id).map_err(map_v3_error)?
                    }
                    BackendDirection::Incoming => {
                        let edge_store = self.edge_store.write();
                        edge_store.incoming(node_id).map_err(map_v3_error)?
                    }
                };

                for neighbor in neighbors.iter() {
                    if visited.insert(*neighbor) {
                        queue.push_back((*neighbor, current_depth + 1));
                    }
                }
            }
        }

        Ok(result)
    }

    fn k_hop_filtered(
        &self,
        _snapshot_id: SnapshotId,
        _start: i64,
        _depth: u32,
        _direction: BackendDirection,
        _allowed_edge_types: &[&str],
    ) -> Result<Vec<i64>, SqliteGraphError> {
        // TODO: Implement edge type filtering
        // For now, delegate to unfiltered k_hop
        self.k_hop(_snapshot_id, _start, _depth, _direction)
    }

    fn bfs_filtered(
        &self,
        snapshot_id: SnapshotId,
        start: i64,
        depth: u32,
        _direction: BackendDirection,
        _allowed_edge_types: &[&str],
    ) -> Result<Vec<i64>, SqliteGraphError> {
        // TODO: Implement edge type filtering for V3 backend.
        // V3's edge_store exposes `neighbors_filtered`, but typed-edge traversal
        // is not yet wired into the BFS path. Tracked alongside `k_hop_filtered`.
        // For now, delegate to unfiltered bfs to match the existing stub pattern.
        self.bfs(snapshot_id, start, depth)
    }

    fn shortest_path_filtered(
        &self,
        snapshot_id: SnapshotId,
        start: i64,
        end: i64,
        _allowed_edge_types: &[&str],
    ) -> Result<Option<Vec<i64>>, SqliteGraphError> {
        // TODO: Implement edge type filtering for V3 backend.
        // See note on `bfs_filtered`.
        self.shortest_path(snapshot_id, start, end)
    }

    fn chain_query(
        &self,
        _snapshot_id: SnapshotId,
        start: i64,
        chain: &[ChainStep],
    ) -> Result<Vec<i64>, SqliteGraphError> {
        let mut current_nodes = vec![start];

        for step in chain {
            let mut next_nodes = Vec::new();

            for &node_id in &current_nodes {
                let neighbors = match step.direction {
                    BackendDirection::Outgoing => {
                        let edge_store = self.edge_store.write();
                        edge_store.outgoing(node_id).map_err(map_v3_error)?
                    }
                    BackendDirection::Incoming => {
                        let edge_store = self.edge_store.write();
                        edge_store.incoming(node_id).map_err(map_v3_error)?
                    }
                };

                for neighbor in neighbors.iter() {
                    // TODO: Apply kind filter from step.target_kind
                    next_nodes.push(*neighbor);
                }
            }

            current_nodes = next_nodes;
        }

        Ok(current_nodes)
    }

    fn pattern_search(
        &self,
        snapshot_id: SnapshotId,
        _start: i64,
        _pattern: &PatternQuery,
    ) -> Result<Vec<PatternMatch>, SqliteGraphError> {
        Self::require_current_snapshot(snapshot_id)?;
        Err(SqliteGraphError::Unsupported(
            "V3 backend does not support pattern_search yet".to_string(),
        ))
    }

    fn checkpoint(&self) -> Result<(), SqliteGraphError> {
        if let Some(ref wal) = self.wal {
            let header = self.header.read();
            let btree = self.btree.read();
            let allocator = self.allocator.read();

            wal.write()
                .checkpoint(
                    btree.root_page_id(),
                    allocator.total_pages(),
                    btree.tree_height(),
                    allocator.free_list_head(),
                    &header,
                )
                .map_err(|e| SqliteGraphError::connection(format!("Checkpoint failed: {:?}", e)))?;
        }

        Ok(())
    }

    fn flush(&self) -> Result<(), SqliteGraphError> {
        self.flush_to_disk()
    }

    fn backup(&self, backup_dir: &Path) -> Result<crate::backend::BackupResult, SqliteGraphError> {
        use std::time::{SystemTime, UNIX_EPOCH};

        // Ensure backup directory exists
        std::fs::create_dir_all(backup_dir).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to create backup dir: {}", e))
        })?;

        // Generate backup filename
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let backup_filename = format!("v3_backup_{}.graph", timestamp);
        let backup_path = backup_dir.join(&backup_filename);

        // Copy database file
        std::fs::copy(&self.db_path, &backup_path)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to copy database: {}", e)))?;

        // Copy WAL if exists
        let wal_path = V3WALPaths::wal_file(&self.db_path);
        if wal_path.exists() {
            let backup_wal_path = V3WALPaths::wal_file(&backup_path);
            std::fs::copy(&wal_path, &backup_wal_path)
                .map_err(|e| SqliteGraphError::connection(format!("Failed to copy WAL: {}", e)))?;
        }

        // Get file size
        let metadata = std::fs::metadata(&backup_path).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to get backup metadata: {}", e))
        })?;

        Ok(crate::backend::BackupResult {
            snapshot_path: backup_path,
            manifest_path: backup_dir.join(format!("v3_backup_{}.manifest", timestamp)),
            size_bytes: metadata.len(),
            checksum: 0, // TODO: Calculate checksum
            record_count: self.header.read().node_count,
            duration_secs: 0.0, // TODO: Measure duration
            timestamp,
            checkpoint_performed: self.wal.is_some(),
        })
    }

    fn snapshot_export(
        &self,
        export_dir: &Path,
    ) -> Result<crate::backend::SnapshotMetadata, SqliteGraphError> {
        use std::time::{SystemTime, UNIX_EPOCH};

        // Ensure export directory exists
        std::fs::create_dir_all(export_dir).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to create export dir: {}", e))
        })?;

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let snapshot_filename = format!("v3_snapshot_{}", timestamp);
        let snapshot_path = export_dir.join(&snapshot_filename);

        // Perform checkpoint first if WAL is enabled
        self.checkpoint()?;

        // Copy database file
        std::fs::copy(&self.db_path, &snapshot_path).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to export snapshot: {}", e))
        })?;

        let metadata = std::fs::metadata(&snapshot_path).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to get snapshot metadata: {}", e))
        })?;

        let header = self.header.read();

        Ok(crate::backend::SnapshotMetadata {
            snapshot_path,
            size_bytes: metadata.len(),
            entity_count: header.node_count,
            edge_count: header.edge_count,
        })
    }

    fn snapshot_import(
        &self,
        import_dir: &Path,
    ) -> Result<crate::backend::ImportMetadata, SqliteGraphError> {
        if !import_dir.is_dir() {
            return Err(SqliteGraphError::connection(format!(
                "Import path is not a directory: {}",
                import_dir.display()
            )));
        }

        // Find v3_snapshot_* files in the import directory
        let mut snapshot_files: Vec<std::fs::DirEntry> = std::fs::read_dir(import_dir)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to read import dir: {}", e)))?
            .filter_map(|entry| entry.ok())
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("v3_snapshot_")
            })
            .collect();

        snapshot_files.sort_by_key(|a| a.file_name());

        if snapshot_files.is_empty() {
            return Err(SqliteGraphError::connection(format!(
                "No v3_snapshot_* files found in {}",
                import_dir.display()
            )));
        }

        let latest_snapshot = &snapshot_files[snapshot_files.len() - 1];
        let snapshot_path = latest_snapshot.path();

        // Get entity count before import
        let before_count = self.header.read().node_count;

        // Copy the snapshot file to our db_path
        std::fs::copy(&snapshot_path, &self.db_path)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to copy snapshot: {}", e)))?;

        // Re-read header from the newly copied file
        let mut file = File::open(&self.db_path).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to open imported DB: {}", e))
        })?;
        let mut header_bytes = vec![0u8; V3_HEADER_SIZE as usize];
        file.read_exact(&mut header_bytes).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to read imported header: {}", e))
        })?;
        let imported_header =
            PersistentHeaderV3::from_bytes(&header_bytes).map_err(map_v3_error)?;
        imported_header.validate().map_err(map_v3_error)?;
        drop(file);

        // Reinitialize all in-memory structures from the imported file
        // The file on disk has changed, so our old BTree/page references are invalid
        // Update allocator in place so new structures share the same Arc
        *self.allocator.write() = PageAllocator::new(&imported_header);

        let btree = BTreeManager::with_root(
            Arc::clone(&self.allocator),
            None,
            imported_header.root_index_page,
            imported_header.btree_height,
            self.db_path.clone(),
        );
        let mut node_store = NodeStore::new(&imported_header, self.db_path.clone());
        node_store.initialize(
            BTreeManager::with_root(
                Arc::clone(&self.allocator),
                None,
                imported_header.root_index_page,
                imported_header.btree_height,
                self.db_path.clone(),
            ),
            Arc::clone(&self.allocator),
            None,
        );
        let edge_store = V3EdgeStore::with_path_and_allocator(
            BTreeManager::with_root(
                Arc::clone(&self.allocator),
                None,
                imported_header.edge_data_offset,
                imported_header.btree_height,
                self.db_path.clone(),
            ),
            None,
            self.db_path.clone(),
            Arc::clone(&self.allocator),
            imported_header.page_size,
        );
        let _ = edge_store.restore_btree_from_metadata();

        // Update in-memory state
        *self.btree.write() = btree;
        *self.node_store.write() = node_store;
        *self.edge_store.write() = edge_store;
        // Keep existing WAL (if any) — snapshot doesn't include WAL
        *self.header.write() = imported_header.clone();

        // Clear and rebuild indexes from the imported data
        self.kind_index.clear();
        self.name_index.clear();
        self.rebuild_indexes();

        // Persist the rebuilt indexes
        let _ = crate::backend::native::v3::index_persistence::persist_indexes(
            &self.db_path,
            &self.kind_index,
            &self.name_index,
            imported_header.node_count,
        );

        let after_count = imported_header.node_count;
        let edges_imported = imported_header.edge_count;

        Ok(crate::backend::ImportMetadata {
            snapshot_path,
            entities_imported: after_count.saturating_sub(before_count),
            edges_imported,
        })
    }

    fn query_nodes_by_kind(
        &self,
        snapshot_id: SnapshotId,
        kind: &str,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        Self::require_current_snapshot(snapshot_id)?;
        Ok(self.kind_index.get(kind))
    }

    fn query_nodes_by_name_pattern(
        &self,
        snapshot_id: SnapshotId,
        pattern: &str,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        Self::require_current_snapshot(snapshot_id)?;

        // Pattern dispatch based on wildcard position
        // - "prefix*" → prefix search
        // - "*substring" or "*substring*" → substring search
        // - no wildcards → exact match
        if pattern.ends_with('*') && !pattern.starts_with('*') {
            // prefix* pattern
            let prefix = &pattern[..pattern.len() - 1];
            Ok(self.name_index.get_prefix(prefix))
        } else if pattern.contains('*') {
            // *suffix or *middle* pattern — strip all * and do substring search
            let cleaned: String = pattern.chars().filter(|&c| c != '*').collect();
            if cleaned.is_empty() {
                Ok(Vec::new())
            } else {
                Ok(self.name_index.get_substring(&cleaned))
            }
        } else {
            // Exact match (no wildcards)
            Ok(self.name_index.get_exact(pattern))
        }
    }

    fn kv_get(
        &self,
        snapshot_id: SnapshotId,
        key: &[u8],
    ) -> Result<Option<crate::backend::native::types::KvValue>, SqliteGraphError> {
        use crate::backend::native::types::KvValue as V2KvValue;

        // If KV store not initialized, key doesn't exist
        let kv_guard = self.kv_store.read();
        let v3_value = kv_guard
            .as_ref()
            .and_then(|kv| kv.get_at_snapshot(key, snapshot_id));

        // Convert V3 KvValue to V2 KvValue (V2 doesn't have Null, use Bytes(vec![]) instead)
        let v2_value = v3_value.and_then(|v| match v {
            KvValue::Null => None, // V2 doesn't have Null, treat as not found
            KvValue::Integer(i) => Some(V2KvValue::Integer(i)),
            KvValue::Float(f) => Some(V2KvValue::Float(f)),
            KvValue::String(s) => Some(V2KvValue::String(s)),
            KvValue::Boolean(b) => Some(V2KvValue::Boolean(b)),
            KvValue::Bytes(b) => Some(V2KvValue::Bytes(b)),
            KvValue::Json(j) => Some(V2KvValue::Json(j)),
        });

        Ok(v2_value)
    }

    fn kv_set(
        &self,
        key: Vec<u8>,
        value: crate::backend::native::types::KvValue,
        ttl_seconds: Option<u64>,
    ) -> Result<(), SqliteGraphError> {
        use crate::backend::native::types::KvValue as V2KvValue;

        // Convert V2 KvValue to V3 KvValue (V2 doesn't have Null)
        let v3_value = match &value {
            V2KvValue::Null => KvValue::Null,
            V2KvValue::Integer(i) => KvValue::Integer(*i),
            V2KvValue::Float(f) => KvValue::Float(*f),
            V2KvValue::String(s) => KvValue::String(s.clone()),
            V2KvValue::Boolean(b) => KvValue::Boolean(*b),
            V2KvValue::Bytes(b) => KvValue::Bytes(b.clone()),
            V2KvValue::Json(j) => KvValue::Json(j.clone()),
        };

        // Get LSN for versioning (use 1 if no WAL)
        let version = if let Some(ref wal) = self.wal {
            let wal_guard = wal.read();
            wal_guard.committed_lsn()
        } else {
            1
        };

        // Compute key hash before moving key
        let key_hash = crate::backend::native::v3::kv_store::types::hash_key(&key);

        // Lazy initialize KV store and set value
        {
            let mut kv_guard = self.kv_store.write();
            kv_guard.get_or_insert_with(KvStore::new).set(
                key.clone(),
                v3_value,
                ttl_seconds,
                version,
            );
        }

        // Write to WAL if enabled
        if let Some(ref wal) = self.wal {
            let mut wal_guard = wal.write();
            let value_bytes = match &value {
                V2KvValue::Null => vec![],
                V2KvValue::Integer(i) => i.to_le_bytes().to_vec(),
                V2KvValue::Float(f) => f.to_le_bytes().to_vec(),
                V2KvValue::String(s) => s.clone().into_bytes(),
                V2KvValue::Boolean(b) => vec![if *b { 1 } else { 0 }],
                V2KvValue::Bytes(b) => b.clone(),
                V2KvValue::Json(j) => serde_json::to_vec(j).unwrap_or_default(),
            };
            let value_type = match &value {
                V2KvValue::Null => 0,
                V2KvValue::Integer(_) => 1,
                V2KvValue::Float(_) => 2,
                V2KvValue::String(_) => 3,
                V2KvValue::Boolean(_) => 4,
                V2KvValue::Bytes(_) => 5,
                V2KvValue::Json(_) => 6,
            };

            let record = V3WALRecord::KvSet {
                lsn: version,
                key,
                value_bytes,
                value_type,
                ttl_seconds,
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            };
            wal_guard
                .append(&record)
                .map_err(|e| SqliteGraphError::connection(format!("WAL write failed: {:?}", e)))?;
        }

        // Emit event (lazy initialize publisher)
        {
            let mut pub_guard = self.publisher.write();
            pub_guard.get_or_insert_with(Publisher::new).emit(
                crate::backend::native::v3::pubsub::types::PubSubEvent::KvChanged {
                    key_hash,
                    snapshot_id: version,
                },
            );
        }

        Ok(())
    }

    fn kv_delete(&self, key: &[u8]) -> Result<(), SqliteGraphError> {
        // Get LSN for versioning (use 1 if no WAL)
        let version = if let Some(ref wal) = self.wal {
            let wal_guard = wal.read();
            wal_guard.committed_lsn()
        } else {
            1
        };

        // Lazy initialize KV store and delete
        {
            let mut kv_guard = self.kv_store.write();
            kv_guard
                .get_or_insert_with(KvStore::new)
                .delete(key, version);
        }

        // Write to WAL if enabled
        if let Some(ref wal) = self.wal {
            let mut wal_guard = wal.write();
            let record = V3WALRecord::KvDelete {
                lsn: version,
                key: key.to_vec(),
                timestamp: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
            };
            wal_guard
                .append(&record)
                .map_err(|e| SqliteGraphError::connection(format!("WAL write failed: {:?}", e)))?;
        }

        // Emit event (lazy initialize publisher)
        {
            let mut pub_guard = self.publisher.write();
            pub_guard.get_or_insert_with(Publisher::new).emit(
                crate::backend::native::v3::pubsub::types::PubSubEvent::KvChanged {
                    key_hash: crate::backend::native::v3::kv_store::types::hash_key(key),
                    snapshot_id: version,
                },
            );
        }

        Ok(())
    }

    fn subscribe(
        &self,
        filter: crate::backend::SubscriptionFilter,
    ) -> Result<(u64, std::sync::mpsc::Receiver<crate::backend::PubSubEvent>), SqliteGraphError>
    {
        use crate::backend::PubSubEvent;
        use crate::backend::native::v3::pubsub::types::{
            PubSubEvent as V3Event, SubscriptionFilter as V3Filter,
        };

        // Convert generic filter to V3 filter
        let v3_filter = V3Filter {
            node_changes: filter.node_changes,
            edge_changes: filter.edge_changes,
            kv_changes: filter.kv_changes,
            snapshot_commits: filter.snapshot_commits,
        };

        // Lazy initialize publisher and subscribe
        let (sub_id, v3_rx) = {
            let mut pub_guard = self.publisher.write();
            pub_guard
                .get_or_insert_with(Publisher::new)
                .subscribe(v3_filter)
        };

        // Create a channel adapter that converts V3 events to generic events
        let (tx, rx) = std::sync::mpsc::channel();

        // Spawn a thread to convert events
        std::thread::spawn(move || {
            while let Ok(v3_event) = v3_rx.recv() {
                let event = match v3_event {
                    V3Event::NodeChanged {
                        node_id,
                        snapshot_id,
                    } => PubSubEvent::NodeChanged {
                        node_id,
                        snapshot_id,
                    },
                    V3Event::EdgeChanged {
                        edge_id,
                        from_node,
                        to_node,
                        snapshot_id,
                    } => PubSubEvent::EdgeChanged {
                        edge_id,
                        from_node,
                        to_node,
                        snapshot_id,
                    },
                    V3Event::KvChanged {
                        key_hash,
                        snapshot_id,
                    } => PubSubEvent::KvChanged {
                        key_hash,
                        snapshot_id,
                    },
                    V3Event::SnapshotCommitted { snapshot_id } => {
                        PubSubEvent::SnapshotCommitted { snapshot_id }
                    }
                };
                if tx.send(event).is_err() {
                    break; // Receiver dropped
                }
            }
        });

        Ok((sub_id.as_u64(), rx))
    }

    fn unsubscribe(&self, subscriber_id: u64) -> Result<bool, SqliteGraphError> {
        use crate::backend::native::v3::pubsub::types::SubscriberId;

        // If publisher not initialized, nothing to unsubscribe
        let pub_guard = self.publisher.read();
        let removed = pub_guard
            .as_ref()
            .map(|p| p.unsubscribe(SubscriberId::from_raw(subscriber_id)))
            .unwrap_or(false);
        Ok(removed)
    }

    fn kv_prefix_scan(
        &self,
        snapshot_id: SnapshotId,
        prefix: &[u8],
    ) -> Result<Vec<(Vec<u8>, crate::backend::native::types::KvValue)>, SqliteGraphError> {
        use crate::backend::native::types::KvValue as V2KvValue;

        // If KV not initialized, return empty results
        let kv_guard = self.kv_store.read();
        let v3_results = kv_guard
            .as_ref()
            .map(|kv| kv.prefix_scan(prefix, snapshot_id))
            .unwrap_or_default();

        // Convert V3 KvValue to V2 KvValue (filter out Null)
        let v2_results: Vec<_> = v3_results
            .into_iter()
            .filter_map(|(k, v)| {
                let v2_value = match v {
                    KvValue::Null => return None, // V2 doesn't have Null
                    KvValue::Integer(i) => V2KvValue::Integer(i),
                    KvValue::Float(f) => V2KvValue::Float(f),
                    KvValue::String(s) => V2KvValue::String(s),
                    KvValue::Boolean(b) => V2KvValue::Boolean(b),
                    KvValue::Bytes(b) => V2KvValue::Bytes(b),
                    KvValue::Json(j) => V2KvValue::Json(j),
                };
                Some((k, v2_value))
            })
            .collect();

        Ok(v2_results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::v3::{V3_FORMAT_VERSION, V3_MAGIC};
    use tempfile::TempDir;

    #[test]
    fn test_v3_backend_create() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");

        let backend = V3Backend::create(&db_path);
        assert!(backend.is_ok());
        assert!(db_path.exists());
    }

    #[test]
    fn test_v3_backend_create_and_open() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");

        // Create
        {
            let backend = V3Backend::create(&db_path).unwrap();
            assert!(!backend.is_wal_enabled());
        }

        // Open
        {
            let backend = V3Backend::open(&db_path).unwrap();
            assert_eq!(backend.header().magic, V3_MAGIC);
            assert_eq!(backend.header().version, V3_FORMAT_VERSION);
        }
    }

    #[test]
    fn test_v3_backend_insert_node() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");

        let backend = V3Backend::create(&db_path).unwrap();

        let node_id = backend
            .insert_node(NodeSpec {
                kind: "Test".to_string(),
                name: "test_node".to_string(),
                file_path: None,
                data: serde_json::json!({"key": "value"}),
            })
            .unwrap();

        assert_eq!(node_id, 1);

        // Verify entity count
        let ids = backend.entity_ids().unwrap();
        assert_eq!(ids.len(), 1);
    }

    /// Test inserting a node with large data (>64 bytes) that requires external storage
    /// This test verifies the fix for the bug where large node data would panic
    #[test]
    fn test_v3_backend_insert_node_with_large_data() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test_large.graph");

        let backend = V3Backend::create(&db_path).unwrap();

        // Create node data that exceeds MAX_INLINE_DATA (64 bytes)
        // The inline buffer format is: [kind_len:1][kind][name_len:1][name][data]
        // So we need data that pushes total over 64 bytes
        let large_data = serde_json::json!({
            "path": "src/components/user/authentication/handlers/login.rs",
            "hash": "abcdef1234567890abcdef1234567890abcdef1234567890",
            "last_indexed_at": 1234567890_i64,
            "last_modified": 1234567890_i64,
            "metadata": {
                "language": "rust",
                "lines": 150,
                "size_bytes": 4096
            }
        });

        // This should NOT panic - it should use external storage
        let node_id = backend
            .insert_node(NodeSpec {
                kind: "File".to_string(),
                name: "login.rs".to_string(),
                file_path: Some("src/components/user/authentication/handlers/login.rs".to_string()),
                data: large_data,
            })
            .unwrap();

        assert_eq!(node_id, 1);

        // Verify entity count
        let ids = backend.entity_ids().unwrap();
        assert_eq!(ids.len(), 1);

        // Verify we can retrieve the node
        use crate::SnapshotId;
        let snapshot = SnapshotId::current();
        let node = backend.get_node(snapshot, node_id).unwrap();
        assert_eq!(node.kind, "File");
        assert_eq!(node.name, "login.rs");
    }

    #[test]
    fn test_v3_backend_open_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("nonexistent.graph");

        let result = V3Backend::open(&db_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_query_nodes_by_kind() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");

        let backend = V3Backend::create(&db_path).unwrap();

        // Insert nodes of different kinds
        backend
            .insert_node(NodeSpec {
                kind: "Function".to_string(),
                name: "func_a".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        backend
            .insert_node(NodeSpec {
                kind: "Function".to_string(),
                name: "func_b".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        backend
            .insert_node(NodeSpec {
                kind: "Class".to_string(),
                name: "class_a".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        use crate::SnapshotId;
        let snapshot = SnapshotId::current();

        // Query by kind
        let functions = backend.query_nodes_by_kind(snapshot, "Function").unwrap();
        assert_eq!(functions.len(), 2);

        let classes = backend.query_nodes_by_kind(snapshot, "Class").unwrap();
        assert_eq!(classes.len(), 1);

        let unknown = backend.query_nodes_by_kind(snapshot, "Unknown").unwrap();
        assert!(unknown.is_empty());
    }

    #[test]
    fn test_query_nodes_by_name_pattern_exact() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");

        let backend = V3Backend::create(&db_path).unwrap();

        backend
            .insert_node(NodeSpec {
                kind: "Function".to_string(),
                name: "func_a".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        backend
            .insert_node(NodeSpec {
                kind: "Function".to_string(),
                name: "func_b".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        use crate::SnapshotId;
        let snapshot = SnapshotId::current();

        // Exact match
        let results = backend
            .query_nodes_by_name_pattern(snapshot, "func_a")
            .unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_query_nodes_by_name_pattern_prefix() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");

        let backend = V3Backend::create(&db_path).unwrap();

        backend
            .insert_node(NodeSpec {
                kind: "Function".to_string(),
                name: "func_a".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        backend
            .insert_node(NodeSpec {
                kind: "Function".to_string(),
                name: "func_b".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        backend
            .insert_node(NodeSpec {
                kind: "Class".to_string(),
                name: "class_a".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        use crate::SnapshotId;
        let snapshot = SnapshotId::current();

        // Prefix match
        let results = backend
            .query_nodes_by_name_pattern(snapshot, "func*")
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_query_nodes_by_name_pattern_substring() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");

        let backend = V3Backend::create(&db_path).unwrap();

        backend
            .insert_node(NodeSpec {
                kind: "Function".to_string(),
                name: "my_func_a".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        backend
            .insert_node(NodeSpec {
                kind: "Function".to_string(),
                name: "my_func_b".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        use crate::SnapshotId;
        let snapshot = SnapshotId::current();

        // Substring match
        let results = backend
            .query_nodes_by_name_pattern(snapshot, "*func*")
            .unwrap();
        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_index_persistence_across_open() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");

        // Create and populate
        {
            let backend = V3Backend::create(&db_path).unwrap();
            backend
                .insert_node(NodeSpec {
                    kind: "Function".to_string(),
                    name: "func_a".to_string(),
                    file_path: None,
                    data: serde_json::json!({}),
                })
                .unwrap();
            backend.flush_to_disk().unwrap();
        }

        // Verify .v3index sidecar was created
        let index_path = crate::backend::native::v3::index_persistence::index_path_for_db(&db_path);
        assert!(
            index_path.exists(),
            "Index sidecar should be created on flush"
        );

        // Reopen and query
        {
            let backend = V3Backend::open(&db_path).unwrap();
            use crate::SnapshotId;
            let snapshot = SnapshotId::current();

            let results = backend.query_nodes_by_kind(snapshot, "Function").unwrap();
            assert_eq!(
                results.len(),
                1,
                "Kind index should be restored from sidecar"
            );
        }
    }

    #[test]
    fn test_index_rebuild_on_open_without_sidecar() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");

        // Create and populate
        {
            let backend = V3Backend::create(&db_path).unwrap();
            backend
                .insert_node(NodeSpec {
                    kind: "Class".to_string(),
                    name: "class_a".to_string(),
                    file_path: None,
                    data: serde_json::json!({}),
                })
                .unwrap();
            backend.flush_to_disk().unwrap();
        }

        // Delete the sidecar
        let index_path = crate::backend::native::v3::index_persistence::index_path_for_db(&db_path);
        let _ = std::fs::remove_file(&index_path);

        // Reopen — should rebuild indexes from node scan
        {
            let backend = V3Backend::open(&db_path).unwrap();
            use crate::SnapshotId;
            let snapshot = SnapshotId::current();

            let results = backend.query_nodes_by_kind(snapshot, "Class").unwrap();
            assert_eq!(
                results.len(),
                1,
                "Kind index should be rebuilt from node scan"
            );
        }
    }

    #[test]
    fn test_snapshot_import() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");
        let import_dir = temp_dir.path().join("import");
        std::fs::create_dir(&import_dir).unwrap();

        // Create a snapshot file
        {
            let backend = V3Backend::create(&db_path).unwrap();
            backend
                .insert_node(NodeSpec {
                    kind: "Function".to_string(),
                    name: "original".to_string(),
                    file_path: None,
                    data: serde_json::json!({}),
                })
                .unwrap();
            backend.flush_to_disk().unwrap();

            // Export snapshot
            backend.snapshot_export(&import_dir).unwrap();
        }

        // Create a fresh database and import
        let fresh_db = temp_dir.path().join("fresh.graph");
        let backend = V3Backend::create(&fresh_db).unwrap();

        let metadata = backend.snapshot_import(&import_dir).unwrap();
        assert_eq!(metadata.entities_imported, 1);

        // Verify imported data is queryable
        use crate::SnapshotId;
        let snapshot = SnapshotId::current();
        let results = backend.query_nodes_by_kind(snapshot, "Function").unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_snapshot_import_missing_dir_fails() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");
        let backend = V3Backend::create(&db_path).unwrap();

        let missing_dir = temp_dir.path().join("nonexistent_import");
        let result = backend.snapshot_import(&missing_dir);
        assert!(result.is_err());
    }
}
