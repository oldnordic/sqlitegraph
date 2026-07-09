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
#![allow(
    clippy::items_after_test_module,
    clippy::manual_async_fn,
    reason = "test modules are mid-file by design in V3; AsyncGraphBackend trait requires impl Future"
)]
#[path = "async_backend.rs"]
mod async_backend;
#[path = "batch_guard.rs"]
mod batch_guard;
#[path = "chain_support.rs"]
mod chain_support;
#[path = "csr_support.rs"]
mod csr_support;
#[path = "hnsw_support.rs"]
mod hnsw_support;
#[path = "index_rebuild.rs"]
mod index_rebuild;
#[path = "mutation_support.rs"]
mod mutation_support;
#[path = "property_support.rs"]
mod property_support;
#[path = "read_support.rs"]
mod read_support;
#[path = "snapshot_meta.rs"]
mod snapshot_meta;
#[path = "sql_exec.rs"]
mod sql_exec;
#[path = "sql_value.rs"]
mod sql_value;
#[path = "sqlite_schema.rs"]
mod sqlite_schema;
#[path = "transaction_control.rs"]
mod transaction_control;
#[path = "transaction_guard.rs"]
mod transaction_guard;
#[path = "transaction_state.rs"]
mod transaction_state;
#[path = "traversal_support.rs"]
mod traversal_support;

use crate::SqliteGraphError;
use crate::backend::native::types::NativeBackendError;
use crate::backend::native::v3::btree::BTreeManager;
use crate::backend::native::v3::edge_compat::Direction as EdgeDirection;
use crate::backend::native::v3::name_index::NameIndex;
use crate::backend::native::v3::storage::AdaptivePageManager;
use crate::backend::native::v3::wal::{V3WALPaths, V3WALRecord, WALWriter};
use crate::backend::native::v3::{
    AsyncFileCoordinator, FileCoordinator, KindIndex, KvStore, KvValue, NodeCache, NodeRecordV3,
    NodeStore, PageAllocator, PersistentHeaderV3, Publisher, V3_HEADER_SIZE, V3EdgeStore,
};
use crate::backend::{
    BackendDirection, ChainStep, EdgeSpec, GraphBackend, NeighborQuery, NodeSpec, PatternMatch,
    PatternQuery,
};
use crate::graph::GraphEntity;
use crate::snapshot::SnapshotId;
pub use batch_guard::WriteBatchGuard;
use hnsw_support::HnswIndexMetadata;
pub use hnsw_support::HnswSearchConfig;
use parking_lot::{Mutex, RwLock};
use rusqlite::Connection;
pub use sql_value::{SqlRow, SqlValue};
use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
pub use transaction_guard::{V3SavepointGuard, V3TransactionGuard};
use transaction_state::GraphTransactionState;

const CSR_SHARD_OUTGOING: i64 = 0;
const CSR_SHARD_INCOMING: i64 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CsrEdgeTypeLookup {
    MissingTable,
    MissingType,
    Found(u32),
}

fn encode_csr_adjacency(entries: &[(u32, f32, u32)]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(entries.len() * 12);
    for &(dst, weight, flags) in entries {
        buf.extend_from_slice(&dst.to_le_bytes());
        buf.extend_from_slice(&weight.to_le_bytes());
        buf.extend_from_slice(&flags.to_le_bytes());
    }
    buf
}

type CsrRuntimeEntry = (u32, f32, String);
type CsrRuntimeRow = (i64, Vec<CsrRuntimeEntry>);

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
    /// Shared file coordinator — persistent file handle pool that eliminates
    /// open/seek/read/close per cache miss on NodeStore, BTreeManager, and
    /// V3EdgeStore. All three subsystems share the SAME coordinator instance.
    file_coordinator: Option<Arc<FileCoordinator>>,
    /// Async coordinator for non-blocking I/O
    async_coordinator: std::sync::OnceLock<Arc<AsyncFileCoordinator>>,
    /// SQLite connection for property storage (SQL Layer)
    sqlite_conn: Arc<Mutex<Connection>>,
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
    /// Current snapshot version (LSN-based)
    current_snapshot_version: RwLock<u64>,
    /// Snapshot metadata registry (name -> LSN mapping)
    snapshot_registry: RwLock<HashMap<String, u64>>,
    /// HNSW indexes for vector similarity search with turbovec metadata
    /// Each index tracks embedding count and turbovec state for automatic compression
    hnsw_indexes: RwLock<HashMap<String, HnswIndexMetadata>>,
    /// Native graph transaction/savepoint state for rollback parity with SQLite.
    graph_tx_state: Mutex<GraphTransactionState>,
}

impl Drop for V3Backend {
    fn drop(&mut self) {
        // Flush any pending data to disk
        // This ensures data is persisted when the backend is dropped.
        // Errors here are silent because we cannot panic in Drop, but we
        // surface them so callers don't silently lose the last transaction
        // (e.g. on disk-full / read-only-FS).
        if let Err(e) = self.flush_to_disk() {
            eprintln!("[sqlitegraph] V3Backend::drop: flush_to_disk failed: {e}");
        }

        // Sync header to ensure all metadata is written
        if let Err(e) = self.sync_header() {
            eprintln!("[sqlitegraph] V3Backend::drop: sync_header failed: {e}");
        }
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
    /// Returns true if the shared FileCoordinator is wired into this backend.
    ///
    /// When true, cold-path reads reuse a persistent file handle instead of
    /// doing open/seek/read/close per cache miss. Used by tests to verify
    /// the coordinator wiring in open() and create().
    pub fn has_file_coordinator(&self) -> bool {
        self.file_coordinator.is_some()
    }

    /// Page-cache capacity (pages). Instance method delegating to NodeStore.
    pub fn node_page_cache_capacity(&self) -> usize {
        self.node_store.read().cache_capacity()
    }

    /// Check whether the page containing `node_id` is resident in the page
    /// cache. Performs a btree lookup to resolve node_id to page_id, then
    /// peeks the cache (no recency update). Used by LRU eviction tests.
    pub fn node_page_cache_resident_for(&self, node_id: i64) -> bool {
        let node_store = self.node_store.read();
        // lookup_node may populate the cache, so we use a peek-based check
        // on the index. The btree tells us the page_id for this node.
        if let Ok(Some(page_id)) = node_store.page_id_for_node(node_id) {
            return node_store.page_cache_contains(page_id);
        }
        false
    }

    /// Number of entries in the unpacked-page cache. Used by double-decode tests.
    pub fn node_unpacked_cache_len(&self) -> usize {
        self.node_store.read().unpacked_page_cache_len()
    }

    /// Clear all node page caches. Used by tests to start from a known state.
    pub fn clear_node_page_caches(&self) {
        self.node_store.write().clear_all_caches();
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
        Self::validate_base_path(&db_path)?;

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
        let node_btree = BTreeManager::new(Arc::clone(&allocator), None, db_path.clone());
        let edge_btree = BTreeManager::new(Arc::clone(&allocator), None, db_path.clone());
        let mut node_store = NodeStore::new(&header, db_path.clone());
        // Initialize NodeStore with shared BTreeManager and PageAllocator
        node_store.initialize(node_btree.clone(), Arc::clone(&allocator), None);
        let max_node_id = node_store
            .node_ids()
            .map_err(map_v3_error)?
            .into_iter()
            .max()
            .unwrap_or(0);
        node_store.set_next_node_id(max_node_id + 1);
        let edge_store = V3EdgeStore::with_path_and_allocator(
            edge_btree,
            None,
            db_path.clone(),
            Arc::clone(&allocator),
            header.page_size,
        );

        // Wire shared FileCoordinator into all subsystems so cold-path reads
        // reuse one persistent file handle instead of open/seek/read/close
        // per cache miss. All three share the SAME coordinator instance.
        // set_file_coordinator propagates to internal BTreeManagers.
        let coordinator = Arc::new(FileCoordinator::create(&db_path).map_err(map_v3_error)?);
        node_store.set_file_coordinator(Arc::clone(&coordinator));
        let mut edge_store = edge_store;
        edge_store.set_file_coordinator(Arc::clone(&coordinator));

        // Initialize SQLite connection for property storage
        let sqlite_path = Self::sqlite_sidecar_path(&db_path)?;
        let sqlite_conn = Connection::open(sqlite_path).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to open SQLite connection: {}", e))
        })?;

        // Create schema tables
        Self::init_sqlite_schema(&sqlite_conn)?;

        Ok(Self {
            db_path,
            file_coordinator: Some(coordinator),
            async_coordinator: std::sync::OnceLock::new(),
            sqlite_conn: Arc::new(Mutex::new(sqlite_conn)),
            btree: RwLock::new(node_btree),
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
            current_snapshot_version: RwLock::new(1),
            snapshot_registry: RwLock::new(HashMap::new()),
            hnsw_indexes: RwLock::new(HashMap::new()), // Lazy initialized
            graph_tx_state: Mutex::new(GraphTransactionState::default()),
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
        Self::validate_base_path(&db_path)?;

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
        let max_node_id = node_store
            .node_ids()
            .map_err(map_v3_error)?
            .into_iter()
            .max()
            .unwrap_or(0);
        node_store.set_next_node_id(max_node_id + 1);
        let mut edge_store = V3EdgeStore::with_path_and_allocator(
            BTreeManager::with_root(
                Arc::clone(&allocator),
                None,
                header.edge_data_offset,
                header.reserved,
                db_path.clone(),
            ),
            None,
            db_path.clone(),
            Arc::clone(&allocator),
            header.page_size,
        );

        // Initialize SQLite connection for property storage
        let sqlite_path = Self::sqlite_sidecar_path(&db_path)?;
        let sqlite_conn = Connection::open(sqlite_path).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to open SQLite connection: {}", e))
        })?;

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

        // Wire shared FileCoordinator into all subsystems so cold-path reads
        // reuse one persistent file handle. Same pattern as create().
        // set_file_coordinator propagates to internal BTreeManagers.
        let coordinator = Arc::new(FileCoordinator::create(&db_path).map_err(map_v3_error)?);
        node_store.set_file_coordinator(Arc::clone(&coordinator));
        edge_store.set_file_coordinator(Arc::clone(&coordinator));

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
            file_coordinator: Some(coordinator),
            async_coordinator: std::sync::OnceLock::new(),
            sqlite_conn: Arc::new(Mutex::new(sqlite_conn)),
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
            current_snapshot_version: RwLock::new(1),
            snapshot_registry: RwLock::new(HashMap::new()),
            hnsw_indexes: RwLock::new(HashMap::new()), // Lazy initialized
            graph_tx_state: Mutex::new(GraphTransactionState::default()),
        };

        // If restore failed, rebuild indexes by scanning all nodes
        if backend.kind_index.kind_count() == 0 && backend.header.read().node_count > 0 {
            backend.rebuild_indexes();
        }

        // Recover KV store from WAL or checkpoint
        let wal_path = V3WALPaths::wal_file(&db_path);
        let mut kv_store = KvStore::new();
        let mut recovered = false;
        if wal_path.exists() {
            let mut recovery = crate::backend::native::v3::wal::WALRecovery::new(wal_path);
            if let Ok(count) = recovery.recover_kv(&mut kv_store) {
                recovered = count > 0;
            }
        }
        if !recovered {
            // No WAL or empty WAL - try reading from checkpoint file
            if let Ok(found) =
                crate::backend::native::v3::wal::read_kv_checkpoint(&db_path, &mut kv_store)
            {
                recovered = found;
            }
        }
        if recovered {
            let mut kv_guard = backend.kv_store.write();
            *kv_guard = Some(kv_store);
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

    /// Return a shared neighbor slice without cloning into a `Vec`.
    ///
    /// This is intended for focused benchmarking of the edge-store hot path.
    pub fn neighbors_shared(
        &self,
        snapshot_id: SnapshotId,
        node: i64,
        query: NeighborQuery,
    ) -> Result<Arc<[i64]>, SqliteGraphError> {
        Self::require_current_snapshot(snapshot_id)?;
        if let Some(csr_neighbors) = self.csr_neighbors_shared(snapshot_id, node, &query)? {
            return Ok(csr_neighbors);
        }
        let edge_store = self.edge_store.read();

        if let Some(ref edge_type) = query.edge_type {
            let dir = match query.direction {
                BackendDirection::Outgoing => EdgeDirection::Outgoing,
                BackendDirection::Incoming => EdgeDirection::Incoming,
            };
            edge_store
                .neighbors_filtered(node, dir, edge_type)
                .map_err(map_v3_error)
        } else {
            match query.direction {
                BackendDirection::Outgoing => edge_store.outgoing(node).map_err(map_v3_error),
                BackendDirection::Incoming => edge_store.incoming(node).map_err(map_v3_error),
            }
        }
    }

    /// Return shared (neighbor_id, weight) pairs without allocating per call.
    ///
    /// For unfiltered queries, neighbors are returned in descending weight
    /// order with neighbor ID as a deterministic tie-breaker.
    pub fn neighbors_weighted_shared(
        &self,
        snapshot_id: SnapshotId,
        node: i64,
        query: NeighborQuery,
    ) -> Result<Arc<[(i64, f32)]>, SqliteGraphError> {
        Self::require_current_snapshot(snapshot_id)?;
        if let Some(csr_neighbors) =
            self.csr_neighbors_weighted_shared(snapshot_id, node, &query)?
        {
            return Ok(csr_neighbors);
        }
        let edge_store = self.edge_store.read();

        let dir = match query.direction {
            BackendDirection::Outgoing => EdgeDirection::Outgoing,
            BackendDirection::Incoming => EdgeDirection::Incoming,
        };

        if let Some(ref edge_type) = query.edge_type {
            edge_store
                .neighbors_weighted_filtered(node, dir, edge_type)
                .map_err(map_v3_error)
        } else {
            edge_store
                .neighbors_weighted(node, dir)
                .map_err(map_v3_error)
        }
    }

    /// Return (neighbor_id, weight) pairs for the given node.
    pub fn neighbors_weighted(
        &self,
        snapshot_id: SnapshotId,
        node: i64,
        query: NeighborQuery,
    ) -> Result<Vec<(i64, f32)>, SqliteGraphError> {
        let shared = self.neighbors_weighted_shared(snapshot_id, node, query)?;
        Ok(shared.to_vec())
    }

    /// Warm the weighted-neighbor cache for a set of source nodes.
    ///
    /// This preloads edge clusters for the requested direction so the first
    /// `neighbors_weighted_shared` call can hit in-memory cache.
    pub fn warm_neighbors_for_sources(
        &self,
        snapshot_id: SnapshotId,
        sources: &[i64],
        query: NeighborQuery,
    ) -> Result<usize, SqliteGraphError> {
        Self::require_current_snapshot(snapshot_id)?;
        let edge_store = self.edge_store.read();
        let dir = match query.direction {
            BackendDirection::Outgoing => EdgeDirection::Outgoing,
            BackendDirection::Incoming => EdgeDirection::Incoming,
        };
        edge_store
            .warm_weighted_neighbors(sources, dir)
            .map_err(map_v3_error)
    }

    /// Return current edge-cache hit and miss counters.
    pub fn edge_cache_stats(&self) -> (u64, u64, u64, u64, usize) {
        self.edge_store.read().cache_stats()
    }

    /// Reset edge-cache counters without evicting cached rows.
    pub fn reset_edge_cache_stats(&self) {
        self.edge_store.read().reset_stats();
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

        // Update header with edge store B+Tree root and allocator state
        let mut header = self.header.write();
        if let Some(root_page) = self.edge_store.read().btree_root_page_id() {
            header.edge_data_offset = root_page;
            header.reserved = self.edge_store.read().btree_height();
        } else {
            header.edge_data_offset = 0;
            header.reserved = 0;
        }
        let allocator = self.allocator.read();
        header.total_pages = allocator.total_pages();
        drop(allocator);
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

        // Write KV checkpoint before WAL flush so it survives truncation
        let kv_guard = self.kv_store.read();
        if let Some(ref kv) = *kv_guard {
            let _ = crate::backend::native::v3::wal::write_kv_checkpoint(&self.db_path, kv);
        }
        drop(kv_guard);

        // Write WAL checkpoint record, flush, then truncate
        self.checkpoint()?;
        if let Some(ref wal) = self.wal {
            wal.write()
                .flush()
                .map_err(|e| SqliteGraphError::connection(format!("WAL flush failed: {:?}", e)))?;
            wal.write().truncate().map_err(|e| {
                SqliteGraphError::connection(format!("WAL truncate failed: {:?}", e))
            })?;
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

    /// Bulk insert a list of weighted edges within a single write batch transaction
    pub fn batch_insert_edges_with_weights(
        &self,
        edges: Vec<(i64, i64, f32, Option<String>)>,
    ) -> Result<(), SqliteGraphError> {
        let mut batch = self.begin_batch();
        for (src, dst, weight, edge_type) in edges {
            let spec = EdgeSpec {
                from: src,
                to: dst,
                edge_type: edge_type.unwrap_or_default(),
                data: serde_json::json!({ "weight": weight }),
            };
            batch.insert_edge(spec)?;
        }
        batch.commit()
    }
}

impl GraphBackend for V3Backend {
    /// Check if snapshot is supported by V3 backend.
    ///
    /// V3 only supports SnapshotId::current() (which returns 0).
    /// Any historical/non-zero snapshot ID is rejected with a clear error.
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        let kind = node.kind.clone();
        let name = node.name.clone();
        let data = node.data.clone();
        let next_version = self.next_graph_version();

        let node_id = self.insert_node_inner(node)?;
        self.sync_header()?;
        self.persist_inserted_node_properties(node_id, &kind, &name, &data, next_version)?;

        self.advance_snapshot_version();

        Ok(node_id)
    }

    fn insert_edge(&self, edge: EdgeSpec) -> Result<i64, SqliteGraphError> {
        let from = edge.from;
        let to = edge.to;
        let edge_type = edge.edge_type.clone();
        let data = edge.data.clone();
        let next_version = self.next_graph_version();

        let edge_id = self.insert_edge_inner(edge)?;
        self.sync_header()?;
        self.persist_inserted_edge_attributes(from, to, &edge_type, &data, next_version)?;
        self.rebuild_csr_runtime_views(next_version)?;
        self.advance_snapshot_version();

        Ok(edge_id)
    }

    fn update_node(&self, node_id: i64, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        let kind = node.kind.clone();
        let name = node.name.clone();
        let next_version = self.next_graph_version();

        // Create updated node record
        let updated_record = NodeRecordV3::new_inline(
            node_id,
            crate::backend::native::types::NodeFlags::empty(),
            0, // kind_offset reserved for future index into kind_string_table
            0, // name_offset reserved for future index into name_string_table
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
        drop(node_store);

        // Invalidate cache entry
        self.node_cache.invalidate(node_id);

        self.flush_to_disk()?;
        self.persist_updated_node_properties(node_id, &kind, &name, &node.data, next_version)?;
        self.rebuild_indexes();
        self.advance_snapshot_version();

        Ok(node_id)
    }

    fn delete_entity(&self, id: i64) -> Result<(), SqliteGraphError> {
        let next_version = self.next_graph_version();
        let mut node_store = self.node_store.write();
        node_store.delete_node(id).map_err(map_v3_error)?;
        drop(node_store);

        // Invalidate cache entry
        self.node_cache.invalidate(id);

        // Update header
        {
            let mut header = self.header.write();
            header.node_count = header.node_count.saturating_sub(1);
        }
        self.sync_header()?;

        self.flush_to_disk()?;
        self.persist_deleted_node_marker(id, next_version)?;
        self.rebuild_indexes();
        self.advance_snapshot_version();

        Ok(())
    }

    fn entity_ids(&self) -> Result<Vec<i64>, SqliteGraphError> {
        let node_store = self.node_store.read();
        let ids = node_store.node_ids().map_err(map_v3_error)?;
        Ok(ids)
    }

    fn get_node(&self, snapshot_id: SnapshotId, id: i64) -> Result<GraphEntity, SqliteGraphError> {
        let deleted_before_present = self.validate_node_snapshot_visibility(snapshot_id, id)?;

        match self.get_node_internal(id)? {
            Some(record) => {
                let data_bytes = self.load_node_record_bytes(&record)?;
                let (kind, name, data) = Self::parse_node_data(&data_bytes, id);

                Ok(GraphEntity {
                    id,
                    kind,
                    name,
                    file_path: None, // V3 compact format does not persist file_path; use GraphEntity.metadata if needed
                    data,
                })
            }
            None => {
                if deleted_before_present.is_some() {
                    Err(SqliteGraphError::query(format!(
                        "Historical version for node {} is unavailable after delete",
                        id
                    )))
                } else {
                    Err(SqliteGraphError::query(format!("Node {} not found", id)))
                }
            }
        }
    }

    fn neighbors(
        &self,
        snapshot_id: SnapshotId,
        node: i64,
        query: NeighborQuery,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        Ok(self.neighbors_shared(snapshot_id, node, query)?.to_vec())
    }

    fn bfs(
        &self,
        snapshot_id: SnapshotId,
        start: i64,
        depth: u32,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        self.bfs_traversal(snapshot_id, start, depth)
    }

    fn shortest_path(
        &self,
        snapshot_id: SnapshotId,
        start: i64,
        end: i64,
    ) -> Result<Option<Vec<i64>>, SqliteGraphError> {
        self.shortest_path_traversal(snapshot_id, start, end)
    }

    fn node_degree(
        &self,
        snapshot_id: SnapshotId,
        node: i64,
    ) -> Result<(usize, usize), SqliteGraphError> {
        self.node_degree_counts(snapshot_id, node)
    }

    fn k_hop(
        &self,
        snapshot_id: SnapshotId,
        start: i64,
        depth: u32,
        direction: BackendDirection,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        self.k_hop_traversal(snapshot_id, start, depth, direction)
    }

    fn k_hop_filtered(
        &self,
        snapshot_id: SnapshotId,
        start: i64,
        depth: u32,
        direction: BackendDirection,
        allowed_edge_types: &[&str],
    ) -> Result<Vec<i64>, SqliteGraphError> {
        self.k_hop_filtered_traversal(snapshot_id, start, depth, direction, allowed_edge_types)
    }

    fn bfs_filtered(
        &self,
        snapshot_id: SnapshotId,
        start: i64,
        depth: u32,
        direction: BackendDirection,
        allowed_edge_types: &[&str],
    ) -> Result<Vec<i64>, SqliteGraphError> {
        self.bfs_filtered_traversal(snapshot_id, start, depth, direction, allowed_edge_types)
    }

    fn shortest_path_filtered(
        &self,
        snapshot_id: SnapshotId,
        start: i64,
        end: i64,
        allowed_edge_types: &[&str],
    ) -> Result<Option<Vec<i64>>, SqliteGraphError> {
        self.shortest_path_filtered_traversal(snapshot_id, start, end, allowed_edge_types)
    }

    fn chain_query(
        &self,
        snapshot_id: SnapshotId,
        start: i64,
        chain: &[ChainStep],
    ) -> Result<Vec<i64>, SqliteGraphError> {
        self.chain_query_steps(snapshot_id, start, chain)
    }

    fn pattern_search(
        &self,
        snapshot_id: SnapshotId,
        start: i64,
        pattern: &PatternQuery,
    ) -> Result<Vec<PatternMatch>, SqliteGraphError> {
        Self::require_current_snapshot(snapshot_id)?;

        if let Some(root_constraint) = &pattern.root {
            let root = self.get_node(snapshot_id, start)?;
            if !root_constraint.matches(&root) {
                return Ok(Vec::new());
            }
        }

        let mut cache: HashMap<i64, GraphEntity> = HashMap::new();
        let mut sequences: Vec<Vec<i64>> = vec![vec![start]];

        for leg in &pattern.legs {
            let mut next_sequences = Vec::new();

            for sequence in &sequences {
                let current = *sequence.last().expect("sequence non-empty");
                let neighbors = self.neighbors(
                    snapshot_id,
                    current,
                    NeighborQuery {
                        direction: leg.direction,
                        edge_type: leg.edge_type.clone(),
                    },
                )?;

                for neighbor in neighbors {
                    let matches_constraint = match leg.constraint.as_ref() {
                        None => true,
                        Some(constraint) => {
                            let entity = if let Some(entity) = cache.get(&neighbor) {
                                entity.clone()
                            } else {
                                let entity = self.get_node(snapshot_id, neighbor)?;
                                cache.insert(neighbor, entity.clone());
                                entity
                            };
                            constraint.matches(&entity)
                        }
                    };

                    if matches_constraint {
                        let mut next = sequence.clone();
                        next.push(neighbor);
                        next_sequences.push(next);
                    }
                }
            }

            if next_sequences.is_empty() {
                return Ok(Vec::new());
            }

            next_sequences.sort();
            next_sequences.dedup();
            sequences = next_sequences;
        }

        let mut matches: Vec<PatternMatch> = sequences
            .into_iter()
            .map(|nodes| PatternMatch { nodes })
            .collect();
        matches.sort_by(|a, b| a.nodes.cmp(&b.nodes));
        Ok(matches)
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
            checksum: 0, // V3 backup checksum not yet implemented; use external fs hash
            record_count: self.header.read().node_count,
            duration_secs: 0.0, // backup duration not instrumented; client should time externally
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
        // The JSONL dump format is produced by `recovery::dump_graph_to_path`
        // and consumed by `recovery::load_graph_from_path` on the sqlite-backend.
        // Each line is a JSON object tagged with `type`:
        //   {"type":"entity","id":..,"kind":..,"name":..,"file_path":..,"data":..}
        //   {"type":"edge","id":..,"from_id":..,"to_id":..,"edge_type":..,"data":..}
        //   {"type":"label","entity_id":..,"label":..}
        //   {"type":"property","entity_id":..,"key":..,"value":..}
        //
        // V3 has no separate labels/properties tables: labels become the node
        // `kind` (the first label wins, matching how kind_index is keyed) and
        // properties/labels are merged into the node's `data` JSON object so
        // nothing is lost on a round-trip.
        let snapshot_file = import_dir.join("snapshot.json");
        if !snapshot_file.exists() {
            return Err(SqliteGraphError::connection(format!(
                "Snapshot file not found: {}",
                snapshot_file.display()
            )));
        }

        let file = File::open(&snapshot_file)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to open snapshot: {}", e)))?;
        let reader = BufReader::new(file);

        // First pass: collect pending labels and properties keyed by source
        // entity id so we can fold them into each node's data before insert.
        let mut labels_by_entity: HashMap<i64, Vec<String>> = HashMap::new();
        let mut props_by_entity: HashMap<i64, Vec<(String, serde_json::Value)>> = HashMap::new();
        let mut entity_records: Vec<serde_json::Value> = Vec::new();
        let mut edge_records: Vec<serde_json::Value> = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| {
                SqliteGraphError::invalid_input(format!("Failed to read line: {}", e))
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let record: serde_json::Value = serde_json::from_str(&line).map_err(|e| {
                SqliteGraphError::invalid_input(format!("Failed to parse JSONL record: {}", e))
            })?;
            let rec_type = record.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match rec_type {
                "entity" => entity_records.push(record),
                "edge" => edge_records.push(record),
                "label" => {
                    let entity_id = record
                        .get("entity_id")
                        .and_then(|v| v.as_i64())
                        .ok_or_else(|| {
                            SqliteGraphError::invalid_input(
                                "label record missing entity_id".to_string(),
                            )
                        })?;
                    let label = record
                        .get("label")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            SqliteGraphError::invalid_input(
                                "label record missing label".to_string(),
                            )
                        })?
                        .to_string();
                    labels_by_entity.entry(entity_id).or_default().push(label);
                }
                "property" => {
                    let entity_id = record
                        .get("entity_id")
                        .and_then(|v| v.as_i64())
                        .ok_or_else(|| {
                            SqliteGraphError::invalid_input(
                                "property record missing entity_id".to_string(),
                            )
                        })?;
                    let key = record
                        .get("key")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            SqliteGraphError::invalid_input(
                                "property record missing key".to_string(),
                            )
                        })?
                        .to_string();
                    let raw_value =
                        record
                            .get("value")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                SqliteGraphError::invalid_input(
                                    "property record missing value".to_string(),
                                )
                            })?;
                    // Property values are stored as strings in the JSONL dump;
                    // attempt to parse as JSON to recover typed values, falling
                    // back to the raw string when not valid JSON.
                    let parsed: serde_json::Value = serde_json::from_str(raw_value)
                        .unwrap_or(serde_json::Value::String(raw_value.to_string()));
                    props_by_entity
                        .entry(entity_id)
                        .or_default()
                        .push((key, parsed));
                }
                "" => {
                    return Err(SqliteGraphError::invalid_input(
                        "JSONL record missing `type` field".to_string(),
                    ));
                }
                other => {
                    return Err(SqliteGraphError::invalid_input(format!(
                        "unknown JSONL record type: {other}"
                    )));
                }
            }
        }

        if entity_records.is_empty() && edge_records.is_empty() {
            return Ok(crate::backend::ImportMetadata {
                snapshot_path: snapshot_file,
                entities_imported: 0,
                edges_imported: 0,
            });
        }

        // Track original-id → inserted-id so edges can be remapped. The V3
        // backend assigns its own ids, so we remap edge endpoints to the ids
        // actually handed out during this import.
        let mut id_map: HashMap<i64, i64> = HashMap::new();
        let mut entities_imported: u64 = 0;

        for rec in &entity_records {
            let original_id = rec.get("id").and_then(|v| v.as_i64()).ok_or_else(|| {
                SqliteGraphError::invalid_input("entity record missing id".to_string())
            })?;
            let kind = rec
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("Entity")
                .to_string();
            let name = rec
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let file_path = rec
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let mut data = rec.get("data").cloned().unwrap_or(serde_json::Value::Null);

            // Fold labels and properties into the data object so they survive
            // the round-trip through V3 (which stores a single JSON blob).
            let extra_labels = labels_by_entity.remove(&original_id);
            let extra_props = props_by_entity.remove(&original_id);
            if extra_labels.is_some() || extra_props.is_some() {
                if !data.is_object() {
                    data = serde_json::Value::Object(serde_json::Map::new());
                }
                if let Some(obj) = data.as_object_mut() {
                    if let Some(labels) = extra_labels {
                        obj.insert(
                            "_labels".to_string(),
                            serde_json::Value::Array(
                                labels.into_iter().map(serde_json::Value::String).collect(),
                            ),
                        );
                    }
                    if let Some(props) = extra_props {
                        for (k, v) in props {
                            obj.insert(k, v);
                        }
                    }
                }
            }

            let node_id = self.insert_node(NodeSpec {
                kind,
                name,
                file_path,
                data,
            })?;
            id_map.insert(original_id, node_id);
            entities_imported += 1;
        }

        let mut edges_imported: u64 = 0;
        for rec in &edge_records {
            let from_original = rec.get("from_id").and_then(|v| v.as_i64()).ok_or_else(|| {
                SqliteGraphError::invalid_input("edge record missing from_id".to_string())
            })?;
            let to_original = rec.get("to_id").and_then(|v| v.as_i64()).ok_or_else(|| {
                SqliteGraphError::invalid_input("edge record missing to_id".to_string())
            })?;
            let edge_type = rec
                .get("edge_type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let data = rec.get("data").cloned().unwrap_or(serde_json::Value::Null);

            let from = id_map.get(&from_original).copied().unwrap_or(from_original);
            let to = id_map.get(&to_original).copied().unwrap_or(to_original);

            self.insert_edge(EdgeSpec {
                from,
                to,
                edge_type,
                data,
            })?;
            edges_imported += 1;
        }

        Ok(crate::backend::ImportMetadata {
            snapshot_path: snapshot_file,
            entities_imported,
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

        // GLOB dispatch matching the sqlite-backend (`name GLOB pattern`):
        // - "prefix*" (ends with single star, no leading star) → fast prefix path
        // - any pattern containing '*' or '?' → full GLOB matcher
        //   (handles "*suffix", "*mid*", "a*b", "func?bar", "*user?")
        // - no wildcards → exact match (a bare "User" matches only "User")
        if pattern.ends_with('*') && !pattern.starts_with('*') && !pattern.contains('?') {
            // "prefix*" — prefix search via the ordered index (O(k) matches).
            let prefix = &pattern[..pattern.len() - 1];
            Ok(self.name_index.get_prefix(prefix))
        } else if pattern.contains('*') || pattern.contains('?') {
            // Wildcard GLOB: delegate to the backtracking glob matcher. This
            // also subsumes the old "*suffix"/"*mid*" substring fallback but
            // with correct anchored GLOB semantics (e.g. "*User" matches names
            // *ending* in "User", not names merely containing "User").
            Ok(self.name_index.get_glob(pattern))
        } else {
            // No wildcards: exact match, identical to SQLite GLOB with a
            // wildcard-free pattern.
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

    fn match_triples(
        &self,
        pattern: &crate::pattern_engine::PatternTriple,
    ) -> Result<Vec<crate::pattern_engine::TripleMatch>, crate::SqliteGraphError> {
        use crate::backend::native::v3::edge_compat::Direction;
        use crate::pattern_engine::TripleMatch;

        pattern.validate()?;

        let mut matches = Vec::new();

        // Convert BackendDirection to V3 Direction
        let v3_direction = match pattern.direction {
            crate::backend::BackendDirection::Outgoing => Direction::Outgoing,
            crate::backend::BackendDirection::Incoming => Direction::Incoming,
        };

        // Get candidate start nodes based on start_label filter
        let start_candidates: Vec<i64> = if let Some(ref start_label) = pattern.start_label {
            self.kind_index.get(start_label)
        } else {
            // No label filter - need to scan all nodes
            // This is expensive, but matches the expected behavior
            self.get_all_node_ids()?
        };

        // For each start node, get matching edges and end nodes
        for start_id in start_candidates {
            // Get neighbors filtered by edge_type
            let edge_store = self.edge_store.read();
            let neighbors =
                match edge_store.neighbors_filtered(start_id, v3_direction, &pattern.edge_type) {
                    Ok(neighbors) => neighbors,
                    Err(_) => continue, // Skip nodes where edge lookup fails
                };

            drop(edge_store); // Release read lock before property checks

            // For each neighbor (potential end node), check filters
            for end_id in neighbors.iter() {
                // Check end_label filter if specified
                if let Some(ref end_label) = pattern.end_label {
                    let label_matches = self.kind_index.get(end_label).contains(end_id);
                    if !label_matches {
                        continue;
                    }
                }

                // Check property filters if specified
                if !self.check_property_filters(start_id, *end_id, pattern)? {
                    continue;
                }

                // Generate synthetic edge_id (hash-based for uniqueness)
                let edge_id =
                    self.generate_edge_id(start_id, *end_id, v3_direction, &pattern.edge_type);

                matches.push(TripleMatch::new(start_id, edge_id, *end_id));
            }
        }

        // Sort results for deterministic output
        matches.sort_by(|a, b| {
            a.start_id
                .cmp(&b.start_id)
                .then_with(|| a.edge_id.cmp(&b.edge_id))
                .then_with(|| a.end_id.cmp(&b.end_id))
        });

        Ok(matches)
    }

    fn vector_search(
        &self,
        _snapshot: crate::snapshot::SnapshotId,
        index_name: &str,
        vector: &[f32],
        k: usize,
    ) -> Result<Vec<(i64, f32)>, crate::SqliteGraphError> {
        self.hnsw_vector_search(index_name, vector, k)
    }

    fn get_graph_ref(&self) -> Option<&crate::graph::SqliteGraph> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::v3::{V3_FORMAT_VERSION, V3_MAGIC};
    use tempfile::TempDir;

    fn encode_csr_adjacency(entries: &[(u32, f32, u32)]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(entries.len() * 12);
        for &(dst, weight, flags) in entries {
            buf.extend_from_slice(&dst.to_le_bytes());
            buf.extend_from_slice(&weight.to_le_bytes());
            buf.extend_from_slice(&flags.to_le_bytes());
        }
        buf
    }

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
    fn test_v3_backend_rejects_sqlite_base_path() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("reserved.sqlite");

        let create_error = match V3Backend::create(&db_path) {
            Ok(_) => panic!("expected .sqlite base path rejection"),
            Err(err) => err,
        };
        assert!(
            create_error
                .to_string()
                .contains("Base path must not use .sqlite extension"),
            "unexpected error: {create_error}"
        );

        std::fs::write(&db_path, b"not-a-v3-db").unwrap();
        let open_error = match V3Backend::open(&db_path) {
            Ok(_) => panic!("expected .sqlite base path rejection"),
            Err(err) => err,
        };
        assert!(
            open_error
                .to_string()
                .contains("Base path must not use .sqlite extension"),
            "unexpected error: {open_error}"
        );
    }

    #[cfg(feature = "turbovec")]
    #[test]
    fn test_hnsw_vector_search_normalizes_ids_and_supports_exact_override() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("hnsw.graph");
        let backend = V3Backend::create(&db_path).unwrap();
        let dimension = 128;

        backend.create_hnsw_index("vectors", dimension).unwrap();

        let make_vector = |seed: usize| -> Vec<f32> {
            (0..dimension)
                .map(|j| {
                    let mut x = ((seed as u64) << 32) ^ j as u64 ^ 0x9E37_79B9_7F4A_7C15;
                    x ^= x >> 30;
                    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
                    x ^= x >> 27;
                    x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
                    x ^= x >> 31;
                    if x & 1 == 0 { -1.0 } else { 1.0 }
                })
                .collect()
        };

        for i in 0..100 {
            let node_id = 10_000 + i as i64;
            backend
                .insert_hnsw_vector(
                    "vectors",
                    &make_vector(i),
                    Some(serde_json::json!({"node_id": node_id})),
                )
                .unwrap();
        }

        let small_result = backend
            .hnsw_vector_search("vectors", &make_vector(42), 1)
            .unwrap();
        assert!(
            (10_000..11_002).contains(&small_result[0].0),
            "expected normalized node_id, got {}",
            small_result[0].0
        );

        for i in 100..1002 {
            let node_id = 10_000 + i as i64;
            backend
                .insert_hnsw_vector(
                    "vectors",
                    &make_vector(i),
                    Some(serde_json::json!({"node_id": node_id})),
                )
                .unwrap();
        }

        assert_eq!(backend.hnsw_embedding_count("vectors").unwrap(), 1002);
        assert!(backend.hnsw_turbovec_ready("vectors").unwrap());

        {
            let metadata = backend.hnsw_indexes.read();
            let index = metadata.get("vectors").unwrap();
            let mut turbovec = index.turbovec_index.lock().unwrap();
            *turbovec = None;
        }
        assert!(!backend.hnsw_turbovec_ready("vectors").unwrap());

        let exact_result = backend
            .hnsw_vector_search_with_config(
                "vectors",
                &make_vector(1001),
                1,
                HnswSearchConfig {
                    force_exact: true,
                    ef_search_override: Some(100),
                },
            )
            .unwrap();
        assert!(
            (10_000..11_002).contains(&exact_result[0].0),
            "expected normalized node_id, got {}",
            exact_result[0].0
        );
        assert!(
            !backend.hnsw_turbovec_ready("vectors").unwrap(),
            "force_exact should not rebuild the turbovec cache"
        );

        let turbovec_result = backend
            .hnsw_vector_search("vectors", &make_vector(1001), 1)
            .unwrap();
        assert!(
            (10_000..11_002).contains(&turbovec_result[0].0),
            "expected normalized node_id, got {}",
            turbovec_result[0].0
        );
        assert!(
            backend.hnsw_turbovec_ready("vectors").unwrap(),
            "default search should rebuild the turbovec cache when eligible"
        );
    }

    #[test]
    fn test_hnsw_vector_search_rejects_invalid_ef_override() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("invalid_ef.graph");
        let backend = V3Backend::create(&db_path).unwrap();

        backend.create_hnsw_index("vectors", 64).unwrap();

        let error = backend
            .hnsw_vector_search_with_config(
                "vectors",
                &[0.0; 64],
                1,
                HnswSearchConfig {
                    force_exact: true,
                    ef_search_override: Some(0),
                },
            )
            .unwrap_err();
        assert!(
            error.to_string().contains("Invalid ef_search_override"),
            "unexpected error: {error}"
        );
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

    #[test]
    fn test_v3_backend_batch_insert_node_persists_node_properties() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("batch_node.graph");
        let backend = V3Backend::create(&db_path).unwrap();

        let mut batch = backend.begin_batch();
        let node_id = batch
            .insert_node(NodeSpec {
                kind: "Batch".to_string(),
                name: "batched_node".to_string(),
                file_path: None,
                data: serde_json::json!({"mode": "batch"}),
            })
            .unwrap();
        batch.commit().unwrap();

        let props = backend.get_node_properties(node_id).unwrap();
        let Some((kind, name, data)) = props else {
            panic!("batched node properties missing");
        };
        assert_eq!(kind, "Batch");
        assert_eq!(name, "batched_node");
        assert_eq!(data["mode"], serde_json::json!("batch"));
    }

    #[test]
    fn test_neighbors_shared_uses_csr_for_outgoing_unfiltered() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("csr_neighbors.graph");
        let backend = V3Backend::create(&db_path).unwrap();

        let n1 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n1".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n2 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n2".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n3 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n3".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        backend
            .insert_edge(EdgeSpec {
                from: n1,
                to: n2,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();
        backend
            .insert_edge(EdgeSpec {
                from: n1,
                to: n3,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();

        let manual_version = backend.current_snapshot_version() + 1;
        let csr_blob = encode_csr_adjacency(&[(n3 as u32, 0.7, 0), (n2 as u32, 0.2, 0)]);
        let conn = backend.sqlite_conn.lock();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS csr_shards (
                shard_id INTEGER NOT NULL,
                node_id INTEGER NOT NULL,
                shard_data BLOB,
                version INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                visible_at INTEGER NOT NULL,
                PRIMARY KEY (shard_id, node_id, version)
            )",
            [],
        )
        .unwrap();
        conn.execute(
                "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
                 VALUES (?1, ?2, ?3, ?4, 0, ?4)",
            rusqlite::params![0_i64, n1, csr_blob, manual_version as i64],
        )
        .unwrap();
        drop(conn);
        backend.set_graph_version(manual_version);

        let neighbors = backend
            .neighbors_shared(
                SnapshotId::current(),
                n1,
                NeighborQuery {
                    direction: BackendDirection::Outgoing,
                    edge_type: None,
                },
            )
            .unwrap();
        assert_eq!(
            &*neighbors,
            &[n3, n2],
            "outgoing unfiltered path should prefer CSR"
        );

        let weighted = backend
            .neighbors_weighted_shared(
                SnapshotId::current(),
                n1,
                NeighborQuery {
                    direction: BackendDirection::Outgoing,
                    edge_type: None,
                },
            )
            .unwrap();
        assert_eq!(&*weighted, &[(n3, 0.7), (n2, 0.2)]);
    }

    #[test]
    fn test_neighbors_shared_keeps_filtered_queries_on_edge_store() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("csr_fallback.graph");
        let backend = V3Backend::create(&db_path).unwrap();

        let n1 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n1".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n2 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n2".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n3 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n3".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        backend
            .insert_edge(EdgeSpec {
                from: n1,
                to: n2,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();
        backend
            .insert_edge(EdgeSpec {
                from: n1,
                to: n3,
                edge_type: "USES".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();

        let csr_blob = encode_csr_adjacency(&[(n3 as u32, 0.7, 0), (n2 as u32, 0.2, 0)]);
        let conn = backend.sqlite_conn.lock();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS csr_shards (
                shard_id INTEGER NOT NULL,
                node_id INTEGER NOT NULL,
                shard_data BLOB,
                version INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                visible_at INTEGER NOT NULL,
                PRIMARY KEY (shard_id, node_id, version)
            )",
            [],
        )
        .unwrap();
        conn.execute(
                "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
                 VALUES (?1, ?2, ?3, 1, 0, 0)",
            rusqlite::params![0_i64, n1, csr_blob],
        )
        .unwrap();
        drop(conn);

        let filtered = backend
            .neighbors_shared(
                SnapshotId::current(),
                n1,
                NeighborQuery {
                    direction: BackendDirection::Outgoing,
                    edge_type: Some("CALLS".to_string()),
                },
            )
            .unwrap();
        assert_eq!(
            &*filtered,
            &[n2],
            "typed queries should stay on edge-store filtering"
        );
    }

    #[test]
    fn test_bfs_uses_csr_for_outgoing_unfiltered() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("csr_bfs.graph");
        let backend = V3Backend::create(&db_path).unwrap();

        let n1 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n1".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n2 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n2".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n3 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n3".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n4 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n4".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        backend
            .insert_edge(EdgeSpec {
                from: n1,
                to: n2,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();
        backend
            .insert_edge(EdgeSpec {
                from: n1,
                to: n3,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();
        backend
            .insert_edge(EdgeSpec {
                from: n3,
                to: n4,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();

        let manual_version = backend.current_snapshot_version() + 1;
        let conn = backend.sqlite_conn.lock();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS csr_shards (
                shard_id INTEGER NOT NULL,
                node_id INTEGER NOT NULL,
                shard_data BLOB,
                version INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                visible_at INTEGER NOT NULL,
                PRIMARY KEY (shard_id, node_id, version)
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
            rusqlite::params![
                0_i64,
                n1,
                encode_csr_adjacency(&[(n3 as u32, 0.7, 0), (n2 as u32, 0.2, 0)]),
                manual_version as i64
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
            rusqlite::params![0_i64, n3, encode_csr_adjacency(&[(n4 as u32, 0.9, 0)]), manual_version as i64],
        )
        .unwrap();
        drop(conn);
        backend.set_graph_version(manual_version);

        let bfs = backend.bfs(SnapshotId::current(), n1, 2).unwrap();
        assert_eq!(bfs, vec![n1, n3, n2, n4]);
    }

    #[test]
    fn test_shortest_path_uses_csr_for_outgoing_unfiltered() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("csr_shortest_path.graph");
        let backend = V3Backend::create(&db_path).unwrap();

        let n1 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n1".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n2 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n2".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n3 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n3".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n4 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n4".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        backend
            .insert_edge(EdgeSpec {
                from: n1,
                to: n2,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();
        backend
            .insert_edge(EdgeSpec {
                from: n1,
                to: n3,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();
        backend
            .insert_edge(EdgeSpec {
                from: n3,
                to: n4,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();
        backend
            .insert_edge(EdgeSpec {
                from: n2,
                to: n4,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();

        let manual_version = backend.current_snapshot_version() + 1;
        let conn = backend.sqlite_conn.lock();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS csr_shards (
                shard_id INTEGER NOT NULL,
                node_id INTEGER NOT NULL,
                shard_data BLOB,
                version INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                visible_at INTEGER NOT NULL,
                PRIMARY KEY (shard_id, node_id, version)
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
            rusqlite::params![
                0_i64,
                n1,
                encode_csr_adjacency(&[(n3 as u32, 0.9, 0), (n2 as u32, 0.2, 0)]),
                manual_version as i64
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
            rusqlite::params![0_i64, n3, encode_csr_adjacency(&[(n4 as u32, 0.8, 0)]), manual_version as i64],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
            rusqlite::params![0_i64, n2, encode_csr_adjacency(&[(n4 as u32, 0.1, 0)]), manual_version as i64],
        )
        .unwrap();
        drop(conn);
        backend.set_graph_version(manual_version);

        let path = backend
            .shortest_path(SnapshotId::current(), n1, n4)
            .unwrap()
            .unwrap();
        assert_eq!(path, vec![n1, n3, n4]);
    }

    #[test]
    fn test_k_hop_uses_csr_for_outgoing_unfiltered() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("csr_k_hop.graph");
        let backend = V3Backend::create(&db_path).unwrap();

        let n1 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n1".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n2 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n2".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n3 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n3".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n4 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n4".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        backend
            .insert_edge(EdgeSpec {
                from: n1,
                to: n2,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();
        backend
            .insert_edge(EdgeSpec {
                from: n1,
                to: n3,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();
        backend
            .insert_edge(EdgeSpec {
                from: n3,
                to: n4,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();

        let manual_version = backend.current_snapshot_version() + 1;
        let conn = backend.sqlite_conn.lock();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS csr_shards (
                shard_id INTEGER NOT NULL,
                node_id INTEGER NOT NULL,
                shard_data BLOB,
                version INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                visible_at INTEGER NOT NULL,
                PRIMARY KEY (shard_id, node_id, version)
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
            rusqlite::params![
                0_i64,
                n1,
                encode_csr_adjacency(&[(n3 as u32, 0.7, 0), (n2 as u32, 0.2, 0)]),
                manual_version as i64
            ],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
            rusqlite::params![0_i64, n3, encode_csr_adjacency(&[(n4 as u32, 0.9, 0)]), manual_version as i64],
        )
        .unwrap();
        drop(conn);
        backend.set_graph_version(manual_version);

        let hop = backend
            .k_hop(SnapshotId::current(), n1, 2, BackendDirection::Outgoing)
            .unwrap();
        assert_eq!(hop, vec![n3, n2, n4]);
    }

    #[test]
    fn test_node_degree_uses_csr_for_outgoing_unfiltered() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("csr_node_degree.graph");
        let backend = V3Backend::create(&db_path).unwrap();

        let n1 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n1".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n2 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n2".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n3 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n3".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        backend
            .insert_edge(EdgeSpec {
                from: n1,
                to: n2,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();
        backend
            .insert_edge(EdgeSpec {
                from: n3,
                to: n1,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();

        let manual_version = backend.current_snapshot_version() + 1;
        let conn = backend.sqlite_conn.lock();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS csr_shards (
                shard_id INTEGER NOT NULL,
                node_id INTEGER NOT NULL,
                shard_data BLOB,
                version INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                visible_at INTEGER NOT NULL,
                PRIMARY KEY (shard_id, node_id, version)
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
            rusqlite::params![
                0_i64,
                n1,
                encode_csr_adjacency(&[(n2 as u32, 0.7, 0), (n3 as u32, 0.3, 0)]),
                manual_version as i64
            ],
        )
        .unwrap();
        drop(conn);
        backend.set_graph_version(manual_version);

        let degree = backend.node_degree(SnapshotId::current(), n1).unwrap();
        assert_eq!(degree, (2, 1));
    }

    #[test]
    fn test_neighbors_shared_uses_csr_for_incoming_unfiltered() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("csr_incoming_neighbors.graph");
        let backend = V3Backend::create(&db_path).unwrap();

        let n1 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n1".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n2 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n2".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n3 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n3".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        backend
            .insert_edge(EdgeSpec {
                from: n1,
                to: n3,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();
        backend
            .insert_edge(EdgeSpec {
                from: n2,
                to: n3,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();

        let manual_version = backend.current_snapshot_version() + 1;
        let conn = backend.sqlite_conn.lock();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS csr_shards (
                shard_id INTEGER NOT NULL,
                node_id INTEGER NOT NULL,
                shard_data BLOB,
                version INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                visible_at INTEGER NOT NULL,
                PRIMARY KEY (shard_id, node_id, version)
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
            rusqlite::params![
                1_i64,
                n3,
                encode_csr_adjacency(&[(n2 as u32, 0.9, 0), (n1 as u32, 0.4, 0)]),
                manual_version as i64
            ],
        )
        .unwrap();
        drop(conn);
        backend.set_graph_version(manual_version);

        let incoming = backend
            .neighbors_shared(
                SnapshotId::current(),
                n3,
                NeighborQuery {
                    direction: BackendDirection::Incoming,
                    edge_type: None,
                },
            )
            .unwrap();
        assert_eq!(&*incoming, &[n2, n1]);
    }

    #[test]
    fn test_neighbors_shared_uses_csr_for_typed_outgoing() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("csr_typed_neighbors.graph");
        let backend = V3Backend::create(&db_path).unwrap();

        let n1 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n1".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n2 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n2".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n3 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n3".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        let n4 = backend
            .insert_node(NodeSpec {
                kind: "Fn".to_string(),
                name: "n4".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        backend
            .insert_edge(EdgeSpec {
                from: n1,
                to: n2,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();
        backend
            .insert_edge(EdgeSpec {
                from: n1,
                to: n3,
                edge_type: "USES".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();
        backend
            .insert_edge(EdgeSpec {
                from: n1,
                to: n4,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({}),
            })
            .unwrap();

        let manual_version = backend.current_snapshot_version() + 1;
        let conn = backend.sqlite_conn.lock();
        let calls_type_id: i64 = conn
            .query_row(
                "SELECT type_id FROM csr_edge_types WHERE edge_type = 'CALLS'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        let uses_type_id: i64 = conn
            .query_row(
                "SELECT type_id FROM csr_edge_types WHERE edge_type = 'USES'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS csr_shards (
                shard_id INTEGER NOT NULL,
                node_id INTEGER NOT NULL,
                shard_data BLOB,
                version INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                visible_at INTEGER NOT NULL,
                PRIMARY KEY (shard_id, node_id, version)
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
            rusqlite::params![
                0_i64,
                n1,
                encode_csr_adjacency(&[
                    (n4 as u32, 0.9, calls_type_id as u32),
                    (n3 as u32, 0.7, uses_type_id as u32),
                    (n2 as u32, 0.5, calls_type_id as u32),
                ]),
                manual_version as i64
            ],
        )
        .unwrap();
        drop(conn);
        backend.set_graph_version(manual_version);

        let typed = backend
            .neighbors_shared(
                SnapshotId::current(),
                n1,
                NeighborQuery {
                    direction: BackendDirection::Outgoing,
                    edge_type: Some("CALLS".to_string()),
                },
            )
            .unwrap();
        assert_eq!(&*typed, &[n4, n2]);
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
    fn test_v3_snapshot_import() {
        use std::io::Write;

        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");
        let backend = V3Backend::create(&db_path).unwrap();

        // Build a snapshot.json in the same JSONL format that
        // `recovery::dump_graph_to_path` produces on the sqlite-backend:
        // three entities, two edges, plus label/property records.
        let snapshot_path = temp_dir.path().join("snapshot.json");
        let mut file = std::fs::File::create(&snapshot_path).unwrap();
        writeln!(
            file,
            r#"{{"type":"entity","id":1,"kind":"Class","name":"NodeA","file_path":null,"data":{{"score":42}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"entity","id":2,"kind":"Class","name":"NodeB","file_path":"src/b.rs","data":{{"score":7}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"entity","id":3,"kind":"Method","name":"NodeC","file_path":null,"data":{{}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"edge","id":10,"from_id":1,"to_id":2,"edge_type":"calls","data":{{"weight":2.0}}}}"#
        )
        .unwrap();
        writeln!(
            file,
            r#"{{"type":"edge","id":11,"from_id":2,"to_id":3,"edge_type":"defines","data":{{}}}}"#
        )
        .unwrap();
        writeln!(file, r#"{{"type":"label","entity_id":1,"label":"public"}}"#).unwrap();
        writeln!(
            file,
            r#"{{"type":"property","entity_id":1,"key":"lang","value":"\"rust\""}}"#
        )
        .unwrap();
        drop(file);

        let meta = backend.snapshot_import(temp_dir.path()).unwrap();
        assert_eq!(meta.entities_imported, 3, "three entities should import");
        assert_eq!(meta.edges_imported, 2, "two edges should import");

        // Node and edge counts should match the import.
        let header = backend.header.read();
        assert_eq!(header.node_count, 3, "header node_count should be 3");
        assert_eq!(header.edge_count, 2, "header edge_count should be 2");
        drop(header);

        // Sample node properties should round-trip, including the merged
        // label ("_labels") and property ("lang") from the JSONL dump.
        let ids = backend.entity_ids().unwrap();
        assert_eq!(ids.len(), 3, "entity_ids should report 3 nodes");

        use crate::snapshot::SnapshotId;
        let node = backend.get_node(SnapshotId::current(), ids[0]).unwrap();
        assert_eq!(node.kind, "Class");
        assert_eq!(node.name, "NodeA");
        assert_eq!(node.data.get("score").and_then(|v| v.as_i64()), Some(42));
        assert_eq!(
            node.data
                .get("_labels")
                .and_then(|v| v.as_array())
                .map(|a| a.len()),
            Some(1),
            "merged label should be present in data"
        );
        assert_eq!(
            node.data.get("lang").and_then(|v| v.as_str()),
            Some("rust"),
            "merged property should be present in data"
        );
    }

    #[test]
    fn test_v3_snapshot_import_missing_file() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");
        let backend = V3Backend::create(&db_path).unwrap();
        let result = backend.snapshot_import(temp_dir.path());
        assert!(result.is_err(), "import without snapshot.json should fail");
    }

    #[test]
    fn test_snapshot_creation() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");
        let backend = V3Backend::create(&db_path).unwrap();

        // Create initial snapshot
        let snapshot_name = "snapshot_v1";
        let lsn_v1 = backend.create_snapshot(snapshot_name).unwrap();
        assert!(lsn_v1 > 0, "Snapshot LSN should be positive");

        // Insert a node after snapshot
        backend
            .insert_node(NodeSpec {
                kind: "Agent".to_string(),
                name: "agent1".to_string(),
                file_path: None,
                data: serde_json::json!({"version": 1}),
            })
            .unwrap();

        // Create second snapshot
        let snapshot_name_v2 = "snapshot_v2";
        let lsn_v2 = backend.create_snapshot(snapshot_name_v2).unwrap();
        assert!(lsn_v2 > lsn_v1, "Snapshot LSNs should increment");

        // Insert another node
        backend
            .insert_node(NodeSpec {
                kind: "Agent".to_string(),
                name: "agent2".to_string(),
                file_path: None,
                data: serde_json::json!({"version": 2}),
            })
            .unwrap();

        // Verify snapshot metadata persists
        let retrieved_lsn = backend.get_snapshot_lsn(snapshot_name);
        assert_eq!(retrieved_lsn, Some(lsn_v1), "Should retrieve stored LSN");

        let retrieved_lsn_v2 = backend.get_snapshot_lsn(snapshot_name_v2);
        assert_eq!(retrieved_lsn_v2, Some(lsn_v2), "Should retrieve second LSN");

        // List snapshots
        let snapshots = backend.list_snapshots();
        assert_eq!(snapshots.len(), 2, "Should have 2 snapshots");
        assert!(
            snapshots
                .iter()
                .any(|(name, lsn)| name == snapshot_name && *lsn == lsn_v1)
        );

        // Delete a snapshot
        backend.delete_snapshot(snapshot_name).unwrap();
        let snapshots_after_delete = backend.list_snapshots();
        assert_eq!(
            snapshots_after_delete.len(),
            1,
            "Should have 1 snapshot after delete"
        );
        assert!(
            !snapshots_after_delete
                .iter()
                .any(|(name, _)| name == snapshot_name)
        );

        // Delete non-existent snapshot should fail
        let result = backend.delete_snapshot("nonexistent");
        assert!(
            result.is_err(),
            "Deleting non-existent snapshot should fail"
        );
    }

    #[test]
    fn test_snapshot_version_increments() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");
        let backend = V3Backend::create(&db_path).unwrap();

        // Initial version
        let version_0 = backend.current_snapshot_version();
        assert_eq!(version_0, 1, "Initial version should be 1");

        // Insert node - version should increment
        backend
            .insert_node(NodeSpec {
                kind: "Node".to_string(),
                name: "node1".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        let version_1 = backend.current_snapshot_version();
        assert_eq!(version_1, 2, "Version should increment after insert");

        // Update node - version should increment
        let node_id = backend.entity_ids().unwrap()[0];
        backend
            .update_node(
                node_id,
                NodeSpec {
                    kind: "Node".to_string(),
                    name: "node1".to_string(),
                    file_path: None,
                    data: serde_json::json!({"updated": true}),
                },
            )
            .unwrap();

        let version_2 = backend.current_snapshot_version();
        assert_eq!(version_2, 3, "Version should increment after update");

        // Delete node - version should increment
        backend.delete_entity(node_id).unwrap();

        let version_3 = backend.current_snapshot_version();
        assert_eq!(version_3, 4, "Version should increment after delete");
    }

    #[test]
    fn test_current_snapshot_validation() {
        use crate::SnapshotId;

        // SnapshotId(0) should always be valid (current state)
        let current = SnapshotId::current();
        assert!(V3Backend::require_current_snapshot(current).is_ok());

        // Historical snapshots should be valid
        let historical = SnapshotId::from_lsn(12345);
        assert!(V3Backend::require_current_snapshot(historical).is_ok());

        // Invalid snapshot should fail
        let invalid = SnapshotId::invalid();
        assert!(V3Backend::require_current_snapshot(invalid).is_err());
    }
}
