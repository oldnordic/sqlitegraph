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
use crate::hnsw::{HnswConfigBuilder, HnswIndex};
use crate::snapshot::SnapshotId;
use parking_lot::{Mutex, RwLock};
use rusqlite::{Connection, ToSql};
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

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

/// Metadata wrapper for HNSW index with turbovec integration
///
/// Tracks embedding count and turbovec compression state.
/// Automatically activates turbovec (4-bit quantization) at 1K vectors.
#[derive(Clone)]
struct HnswIndexMetadata {
    /// HNSW index (always available, works for small datasets)
    hnsw_index: Arc<std::sync::Mutex<HnswIndex>>,
    /// Turbovec compressed index (feature-gated, activates at 1K vectors)
    #[cfg(feature = "turbovec")]
    turbovec_index: Arc<std::sync::Mutex<Option<turbovec::IdMapIndex>>>,
    /// Embedding count tracker (triggers turbovec at threshold)
    #[cfg(feature = "turbovec")]
    embedding_count: Arc<std::sync::Mutex<usize>>,
    /// Vector dimension (must match all insertions)
    dimension: usize,
    /// Turbovec bit width (2-4, default 4 for best precision)
    #[cfg(feature = "turbovec")]
    bit_width: usize,
}

struct GraphTransactionFrame {
    backup_dir: PathBuf,
    snapshot_version: u64,
    snapshot_registry: HashMap<String, u64>,
}

#[derive(Default)]
struct GraphTransactionState {
    frames: Vec<GraphTransactionFrame>,
}

/// Turbovec activation threshold: 1K vectors triggers compression
#[cfg(feature = "turbovec")]
const TURBOVEC_THRESHOLD: usize = 1_000;

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

    /// Insert a node with a manually specified ID without syncing (accumulated in batch)
    pub fn insert_node_with_id(
        &mut self,
        mut node: NodeSpec,
        node_id: i64,
    ) -> Result<i64, SqliteGraphError> {
        if let Some(obj) = node.data.as_object_mut() {
            obj.insert("id".to_string(), serde_json::Value::Number(node_id.into()));
        } else {
            let mut obj = serde_json::Map::new();
            obj.insert("id".to_string(), serde_json::Value::Number(node_id.into()));
            node.data = serde_json::Value::Object(obj);
        }
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
    /// Check if snapshot is valid for V3 backend.
    ///
    /// V3 now supports MVCC snapshots:
    /// - SnapshotId(0) = current state (sees all committed data)
    /// - SnapshotId(lsn) = historical state at specific LSN (time travel)
    /// - SnapshotId(u64::MAX) = invalid snapshot
    fn require_current_snapshot(snapshot_id: SnapshotId) -> Result<(), SqliteGraphError> {
        // SnapshotId(0) is always valid (current state)
        if snapshot_id.0 == 0 {
            return Ok(());
        }

        // Invalid sentinel
        if snapshot_id.0 == u64::MAX {
            return Err(SqliteGraphError::query(
                "Invalid snapshot ID (u64::MAX sentinel)".to_string(),
            ));
        }

        // Historical snapshots must be positive LSNs
        if snapshot_id.0 > 0 {
            return Ok(());
        }

        Err(SqliteGraphError::query(format!(
            "Invalid snapshot ID: {}",
            snapshot_id.0
        )))
    }

    fn current_graph_version(&self) -> u64 {
        *self.current_snapshot_version.read()
    }

    fn next_graph_version(&self) -> u64 {
        self.current_graph_version().saturating_add(1)
    }

    fn set_graph_version(&self, version: u64) {
        *self.current_snapshot_version.write() = version;
    }

    fn set_snapshot_registry(&self, registry: HashMap<String, u64>) {
        *self.snapshot_registry.write() = registry;
    }

    fn graph_transaction_active(&self) -> bool {
        !self.graph_tx_state.lock().frames.is_empty()
    }

    fn ensure_hnsw_not_in_graph_transaction(
        &self,
        operation: &str,
    ) -> Result<(), SqliteGraphError> {
        if self.graph_transaction_active() {
            return Err(SqliteGraphError::unsupported(format!(
                "{} is not supported while a native-v3 transaction/savepoint is active",
                operation
            )));
        }
        Ok(())
    }

    fn transaction_timestamp() -> u128 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0)
    }

    fn graph_backup_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "sqlitegraph-v3-tx-{}-{}",
            std::process::id(),
            Self::transaction_timestamp()
        ))
    }

    fn index_sidecar_path(db_path: &Path) -> PathBuf {
        db_path.with_extension("v3index")
    }

    fn edge_metadata_path(db_path: &Path) -> PathBuf {
        db_path.with_extension("v3edgemeta")
    }

    fn copy_if_exists(from: &Path, to: &Path) -> Result<(), SqliteGraphError> {
        if from.exists() {
            fs::copy(from, to).map_err(|e| {
                SqliteGraphError::connection(format!(
                    "Failed to copy {} to {}: {}",
                    from.display(),
                    to.display(),
                    e
                ))
            })?;
        }
        Ok(())
    }

    fn restore_optional_file(backup: &Path, target: &Path) -> Result<(), SqliteGraphError> {
        if backup.exists() {
            fs::copy(backup, target).map_err(|e| {
                SqliteGraphError::connection(format!(
                    "Failed to restore {} from {}: {}",
                    target.display(),
                    backup.display(),
                    e
                ))
            })?;
        } else if target.exists() {
            fs::remove_file(target).map_err(|e| {
                SqliteGraphError::connection(format!(
                    "Failed to remove stale sidecar {}: {}",
                    target.display(),
                    e
                ))
            })?;
        }
        Ok(())
    }

    fn begin_graph_transaction_frame(&self) -> Result<(), SqliteGraphError> {
        self.flush_to_disk()?;

        let backup_dir = Self::graph_backup_dir();
        fs::create_dir_all(&backup_dir).map_err(|e| {
            SqliteGraphError::connection(format!(
                "Failed to create native-v3 transaction backup dir {}: {}",
                backup_dir.display(),
                e
            ))
        })?;

        Self::copy_if_exists(&self.db_path, &backup_dir.join("graph.snapshot"))?;
        Self::copy_if_exists(
            &Self::index_sidecar_path(&self.db_path),
            &backup_dir.join("index.snapshot"),
        )?;
        Self::copy_if_exists(
            &Self::edge_metadata_path(&self.db_path),
            &backup_dir.join("edge.snapshot"),
        )?;

        let frame = GraphTransactionFrame {
            backup_dir,
            snapshot_version: self.current_graph_version(),
            snapshot_registry: self.snapshot_registry.read().clone(),
        };

        self.graph_tx_state.lock().frames.push(frame);
        Ok(())
    }

    fn discard_graph_transaction_frame(&self) -> Result<(), SqliteGraphError> {
        let frame = self.graph_tx_state.lock().frames.pop().ok_or_else(|| {
            SqliteGraphError::connection("No native-v3 transaction frame to discard".to_string())
        })?;

        if frame.backup_dir.exists() {
            fs::remove_dir_all(&frame.backup_dir).map_err(|e| {
                SqliteGraphError::connection(format!(
                    "Failed to remove native-v3 transaction backup dir {}: {}",
                    frame.backup_dir.display(),
                    e
                ))
            })?;
        }

        Ok(())
    }

    fn rollback_graph_transaction_frame(&self) -> Result<(), SqliteGraphError> {
        let frame = self.graph_tx_state.lock().frames.pop().ok_or_else(|| {
            SqliteGraphError::connection("No native-v3 transaction frame to roll back".to_string())
        })?;

        Self::restore_optional_file(&frame.backup_dir.join("graph.snapshot"), &self.db_path)?;
        Self::restore_optional_file(
            &frame.backup_dir.join("index.snapshot"),
            &Self::index_sidecar_path(&self.db_path),
        )?;
        Self::restore_optional_file(
            &frame.backup_dir.join("edge.snapshot"),
            &Self::edge_metadata_path(&self.db_path),
        )?;

        self.reload_graph_runtime_from_disk()?;
        self.set_graph_version(frame.snapshot_version);
        self.set_snapshot_registry(frame.snapshot_registry);

        if frame.backup_dir.exists() {
            fs::remove_dir_all(&frame.backup_dir).map_err(|e| {
                SqliteGraphError::connection(format!(
                    "Failed to remove native-v3 transaction backup dir {}: {}",
                    frame.backup_dir.display(),
                    e
                ))
            })?;
        }

        Ok(())
    }

    fn reload_graph_runtime_from_disk(&self) -> Result<(), SqliteGraphError> {
        let mut file = File::open(&self.db_path).map_err(|e| {
            SqliteGraphError::connection(format!(
                "Failed to reopen native-v3 database file {}: {}",
                self.db_path.display(),
                e
            ))
        })?;

        let mut header_bytes = vec![0u8; V3_HEADER_SIZE as usize];
        file.read_exact(&mut header_bytes)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to read header: {}", e)))?;
        let header = PersistentHeaderV3::from_bytes(&header_bytes).map_err(map_v3_error)?;
        header.validate().map_err(map_v3_error)?;

        *self.allocator.write() = PageAllocator::new(&header);

        let btree = BTreeManager::with_root(
            Arc::clone(&self.allocator),
            None,
            header.root_index_page,
            header.btree_height,
            self.db_path.clone(),
        );
        let mut node_store = NodeStore::new(&header, self.db_path.clone());
        node_store.initialize(
            BTreeManager::with_root(
                Arc::clone(&self.allocator),
                None,
                header.root_index_page,
                header.btree_height,
                self.db_path.clone(),
            ),
            Arc::clone(&self.allocator),
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
                Arc::clone(&self.allocator),
                None,
                header.edge_data_offset,
                header.reserved,
                self.db_path.clone(),
            ),
            None,
            self.db_path.clone(),
            Arc::clone(&self.allocator),
            header.page_size,
        );
        if let Some(ref wal_arc) = self.wal {
            edge_store.set_wal(Arc::clone(wal_arc));
        }
        let _ = edge_store.restore_btree_from_metadata();

        *self.btree.write() = btree;
        *self.node_store.write() = node_store;
        *self.edge_store.write() = edge_store;
        *self.header.write() = header.clone();

        self.node_cache.clear();
        self.kind_index.clear();
        self.name_index.clear();
        self.rebuild_indexes();

        let _ = crate::backend::native::v3::index_persistence::persist_indexes(
            &self.db_path,
            &self.kind_index,
            &self.name_index,
            header.node_count,
        );

        Ok(())
    }

    /// Create a named snapshot with current LSN as version
    ///
    /// # Arguments
    ///
    /// * `name` - Snapshot name/identifier
    ///
    /// # Returns
    ///
    /// * `Ok(u64)` - LSN assigned to this snapshot
    /// * `Err(SqliteGraphError)` - If snapshot creation fails
    pub fn create_snapshot(&self, name: &str) -> Result<u64, SqliteGraphError> {
        let current_lsn = if let Some(ref wal) = self.wal {
            wal.read().committed_lsn()
        } else {
            self.current_graph_version()
        };

        // Store snapshot metadata
        let mut registry = self.snapshot_registry.write();
        registry.insert(name.to_string(), current_lsn);

        Ok(current_lsn)
    }

    /// Get the current snapshot version (LSN)
    pub fn current_snapshot_version(&self) -> u64 {
        if let Some(ref wal) = self.wal {
            wal.read().committed_lsn()
        } else {
            self.current_graph_version()
        }
    }

    /// Increment snapshot version (called after commits)
    fn advance_snapshot_version(&self) -> u64 {
        let version = self.next_graph_version();
        self.set_graph_version(version);
        version
    }

    /// Get snapshot LSN by name
    pub fn get_snapshot_lsn(&self, name: &str) -> Option<u64> {
        let registry = self.snapshot_registry.read();
        registry.get(name).copied()
    }

    /// List all snapshots
    pub fn list_snapshots(&self) -> Vec<(String, u64)> {
        let registry = self.snapshot_registry.read();
        registry.iter().map(|(k, v)| (k.clone(), *v)).collect()
    }

    /// Delete a snapshot by name
    pub fn delete_snapshot(&self, name: &str) -> Result<(), SqliteGraphError> {
        let mut registry = self.snapshot_registry.write();
        if registry.remove(name).is_none() {
            return Err(SqliteGraphError::query(format!(
                "Snapshot not found: {}",
                name
            )));
        }
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

        // Initialize SQLite connection for property storage
        let sqlite_path = db_path.with_extension("sqlite");
        let sqlite_conn = Connection::open(sqlite_path).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to open SQLite connection: {}", e))
        })?;

        // Create schema tables
        Self::init_sqlite_schema(&sqlite_conn)?;

        Ok(Self {
            db_path,
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
        let sqlite_path = db_path.with_extension("sqlite");
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

    /// Create or get an HNSW index for vector similarity search
    ///
    /// # Arguments
    ///
    /// * `index_name` - Name of the index
    /// * `dimension` - Vector dimension
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Index created or retrieved successfully
    /// * `Err(SqliteGraphError)` - Error during index creation
    pub fn create_hnsw_index(
        &self,
        index_name: &str,
        dimension: usize,
        bit_width: usize, // 2, 3, or 4 (default 4 for best precision)
    ) -> Result<(), SqliteGraphError> {
        self.ensure_hnsw_not_in_graph_transaction("create_hnsw_index")?;

        // Validate bit_width (turbovec only supports 2, 3, or 4)
        #[cfg(feature = "turbovec")]
        if ![2usize, 3, 4].contains(&bit_width) {
            return Err(SqliteGraphError::validation(format!(
                "Invalid bit_width: {}. Must be 2, 3, or 4",
                bit_width
            )));
        }
        #[cfg(not(feature = "turbovec"))]
        let _ = bit_width;

        // Check if index already exists
        {
            let indexes = self.hnsw_indexes.read();
            if indexes.contains_key(index_name) {
                return Ok(()); // Already exists
            }
        }

        // Create HNSW config with default parameters
        let config = HnswConfigBuilder::new()
            .dimension(dimension)
            .distance_metric(crate::hnsw::DistanceMetric::Euclidean)
            .build()
            .map_err(|e| SqliteGraphError::validation(format!("HNSW config error: {:?}", e)))?;

        // Create HnswIndex with appropriate storage backend
        #[cfg(feature = "native-v3")]
        let hnsw_index = {
            // Use V3VectorStorageHandle for persistence
            let storage_handle =
                crate::hnsw::v3_storage::V3VectorStorageHandle::new(self, index_name);
            HnswIndex::with_storage(index_name, config, Box::new(storage_handle)).map_err(|e| {
                SqliteGraphError::validation(format!("HNSW index creation error: {:?}", e))
            })?
        };

        #[cfg(not(feature = "native-v3"))]
        let hnsw_index = {
            // Use in-memory storage when native-v3 feature is not enabled
            HnswIndex::new(index_name, config).map_err(|e| {
                SqliteGraphError::validation(format!("HNSW index creation error: {:?}", e))
            })?
        };

        // Store index metadata in KV store
        let metadata_key = format!("hnsw:{}:metadata", index_name);
        let metadata_value = serde_json::json!({
            "name": index_name,
            "dimension": dimension,
            "created_at": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
        });
        self.kv_set_v3(
            metadata_key.into_bytes(),
            KvValue::Json(metadata_value),
            None,
        );

        // Add to indexes map with metadata
        let mut indexes = self.hnsw_indexes.write();
        indexes.insert(
            index_name.to_string(),
            HnswIndexMetadata {
                hnsw_index: Arc::new(std::sync::Mutex::new(hnsw_index)),
                #[cfg(feature = "turbovec")]
                turbovec_index: Arc::new(std::sync::Mutex::new(None)),
                #[cfg(feature = "turbovec")]
                embedding_count: Arc::new(std::sync::Mutex::new(0)),
                dimension,
                #[cfg(feature = "turbovec")]
                bit_width,
            },
        );

        Ok(())
    }

    /// Check if an HNSW index exists
    ///
    /// # Arguments
    ///
    /// * `index_name` - Name of the index
    ///
    /// # Returns
    ///
    /// * `true` - Index exists
    /// * `false` - Index does not exist
    pub fn has_hnsw_index(&self, index_name: &str) -> bool {
        let indexes = self.hnsw_indexes.read();
        indexes.contains_key(index_name)
    }

    /// Return the number of embeddings tracked for an HNSW index.
    pub fn hnsw_embedding_count(&self, index_name: &str) -> Result<usize, SqliteGraphError> {
        let indexes = self.hnsw_indexes.read();
        let metadata = indexes.get(index_name).ok_or_else(|| {
            SqliteGraphError::validation(format!("HNSW index not found: {}", index_name))
        })?;

        #[cfg(feature = "turbovec")]
        {
            return Ok(*metadata.embedding_count.lock().unwrap());
        }

        #[cfg(not(feature = "turbovec"))]
        {
            let hnsw = metadata.hnsw_index.lock().unwrap();
            Ok(hnsw.vector_count())
        }
    }

    /// Return whether the turbovec sidecar is already built for an HNSW index.
    pub fn hnsw_turbovec_ready(&self, index_name: &str) -> Result<bool, SqliteGraphError> {
        let indexes = self.hnsw_indexes.read();
        let metadata = indexes.get(index_name).ok_or_else(|| {
            SqliteGraphError::validation(format!("HNSW index not found: {}", index_name))
        })?;

        #[cfg(feature = "turbovec")]
        {
            Ok(metadata.turbovec_index.lock().unwrap().is_some())
        }

        #[cfg(not(feature = "turbovec"))]
        {
            let _ = metadata;
            Ok(false)
        }
    }

    /// Get an HNSW index by name (returns Arc clone for thread safety)
    ///
    /// # Arguments
    ///
    /// * `index_name` - Name of the index
    ///
    /// # Returns
    ///
    /// * `Ok(Some(Arc<Mutex<HnswIndex>>))` - Index found
    /// * `Ok(None)` - Index not found
    pub fn get_hnsw_index(
        &self,
        index_name: &str,
    ) -> Result<Option<Arc<std::sync::Mutex<HnswIndex>>>, SqliteGraphError> {
        self.ensure_hnsw_not_in_graph_transaction("get_hnsw_index")?;
        let indexes = self.hnsw_indexes.read();
        Ok(indexes
            .get(index_name)
            .map(|metadata| metadata.hnsw_index.clone()))
    }

    /// Insert a vector into HNSW index with turbovec tracking
    ///
    /// # Arguments
    ///
    /// * `index_name` - Name of the HNSW index
    /// * `vector` - Vector data (must match index dimension)
    /// * `metadata` - Optional JSON metadata (e.g., node_id for reverse mapping)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Insert succeeded
    /// * `Err(SqliteGraphError)` - Dimension mismatch or HNSW error
    ///
    /// # Turbovec Integration
    ///
    /// Tracks embedding count and automatically activates turbovec
    /// compression at 1K vectors (16x memory savings, SIMD search).
    #[cfg(feature = "turbovec")]
    pub fn insert_hnsw_vector(
        &self,
        index_name: &str,
        vector: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<(), SqliteGraphError> {
        self.ensure_hnsw_not_in_graph_transaction("insert_hnsw_vector")?;
        // Get index metadata
        let metadata_arc = {
            let indexes = self.hnsw_indexes.read();
            indexes.get(index_name).cloned().ok_or_else(|| {
                SqliteGraphError::validation(format!("HNSW index not found: {}", index_name))
            })?
        };

        // Validate dimension
        if vector.len() != metadata_arc.dimension {
            return Err(SqliteGraphError::validation(format!(
                "Vector dimension mismatch: expected {}, got {}",
                metadata_arc.dimension,
                vector.len()
            )));
        }

        // Insert into HNSW
        let mut hnsw = metadata_arc.hnsw_index.lock().unwrap();
        hnsw.insert_vector(vector, metadata)
            .map_err(|e| SqliteGraphError::validation(format!("HNSW insert failed: {:?}", e)))?;
        drop(hnsw);

        // Update embedding count
        let mut count = metadata_arc.embedding_count.lock().unwrap();
        *count += 1;
        let current_count = *count;
        drop(count);

        // Build turbovec index once when threshold is crossed
        if current_count == TURBOVEC_THRESHOLD + 1 {
            self.build_turbovec_index(index_name)?;
        }

        Ok(())
    }

    /// Insert a vector into HNSW index (without turbovec)
    #[cfg(not(feature = "turbovec"))]
    pub fn insert_hnsw_vector(
        &self,
        index_name: &str,
        vector: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<(), SqliteGraphError> {
        self.ensure_hnsw_not_in_graph_transaction("insert_hnsw_vector")?;
        // Get index metadata
        let metadata_arc = {
            let indexes = self.hnsw_indexes.read();
            indexes.get(index_name).cloned().ok_or_else(|| {
                SqliteGraphError::validation(format!("HNSW index not found: {}", index_name))
            })?
        };

        // Validate dimension
        if vector.len() != metadata_arc.dimension {
            return Err(SqliteGraphError::validation(format!(
                "Vector dimension mismatch: expected {}, got {}",
                metadata_arc.dimension,
                vector.len()
            )));
        }

        // Insert into HNSW
        let mut hnsw = metadata_arc.hnsw_index.lock().unwrap();
        hnsw.insert_vector(vector, metadata)
            .map_err(|e| SqliteGraphError::validation(format!("HNSW insert failed: {:?}", e)))?;

        Ok(())
    }

    /// Delete an HNSW index
    ///
    /// # Arguments
    ///
    /// * `index_name` - Name of the index
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Index deleted successfully
    /// * `Err(SqliteGraphError)` - Error during deletion
    pub fn delete_hnsw_index(&self, index_name: &str) -> Result<(), SqliteGraphError> {
        self.ensure_hnsw_not_in_graph_transaction("delete_hnsw_index")?;
        // Remove from memory
        let mut indexes = self.hnsw_indexes.write();
        indexes.remove(index_name);

        // Remove metadata from KV store
        let metadata_key = format!("hnsw:{}:metadata", index_name);
        self.kv_delete_v3(metadata_key.as_bytes());

        // Remove all vector data for this index
        let vector_prefix = format!("hnsw:{}:vector:", index_name);
        let snapshot_id = SnapshotId::current();
        let vectors = self.kv_prefix_scan_v3(snapshot_id, vector_prefix.as_bytes());
        for (key, _) in vectors {
            self.kv_delete_v3(&key);
        }

        Ok(())
    }

    /// Perform vector similarity search using HNSW index
    ///
    /// # Arguments
    ///
    /// * `index_name` - Name of the HNSW index
    /// * `query_vector` - Query vector
    /// * `k` - Number of nearest neighbors to return
    ///
    /// # Returns
    ///
    /// * `Ok(Vec<(i64, f32)>)` - Vector of (vector_id, distance) pairs
    /// * `Err(SqliteGraphError)` - Error during search

    /// Build turbovec index from existing HNSW embeddings
    ///
    /// Extracts all vectors from HNSW and builds compressed turbovec index
    /// with 4-bit quantization (16x memory savings, SIMD search).
    ///
    /// # Arguments
    ///
    /// * `index_name` - Name of the HNSW index
    #[cfg(feature = "turbovec")]
    fn build_turbovec_index(&self, index_name: &str) -> Result<(), SqliteGraphError> {
        let metadata_arc = {
            let indexes = self.hnsw_indexes.read();
            indexes.get(index_name).cloned().ok_or_else(|| {
                SqliteGraphError::validation(format!("HNSW index not found: {}", index_name))
            })?
        };

        // Check if already built (guard against duplicate builds)
        #[cfg(feature = "turbovec")]
        {
            let turbovec = metadata_arc.turbovec_index.lock().unwrap();
            if turbovec.is_some() {
                return Ok(()); // Already built
            }
            drop(turbovec);
        }

        let hnsw = metadata_arc.hnsw_index.lock().unwrap();
        let count = hnsw.vector_count();

        if count == 0 {
            return Ok(()); // No embeddings to index
        }

        // Collect all embeddings and IDs from HNSW
        let mut embeddings: Vec<f32> = Vec::with_capacity(count * metadata_arc.dimension);
        let mut ids: Vec<u64> = Vec::with_capacity(count);

        for i in 1..=count {
            if let Ok(Some((vector, metadata))) = hnsw.get_vector(i as u64) {
                // Extract node_id from metadata if available
                let id = metadata
                    .get("node_id")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(i as u64); // Fallback to vector_id if no node_id

                embeddings.extend_from_slice(&vector);
                ids.push(id);
            }
        }

        drop(hnsw);

        // Debug: verify vector collection
        eprintln!(
            "DEBUG: Building turbovec from {} HNSW vectors (dimension: {})",
            ids.len(),
            metadata_arc.dimension
        );

        // Build turbovec index with configured bit width (2-4, default 4 for best precision)
        let bit_width = metadata_arc.bit_width;
        let mut turbovec_index = turbovec::IdMapIndex::new(metadata_arc.dimension, bit_width)
            .map_err(|e| {
                SqliteGraphError::validation(format!("Turbovec construction failed: {}", e))
            })?;

        eprintln!(
            "DEBUG: Adding {} vectors to turbovec index ({}-bit quantization)",
            ids.len(),
            bit_width
        );
        turbovec_index
            .add_with_ids(&embeddings, &ids)
            .map_err(|e| SqliteGraphError::validation(format!("Turbovec add failed: {}", e)))?;

        eprintln!("DEBUG: Turbovec index built successfully");
        // Store the index
        let mut turbovec = metadata_arc.turbovec_index.lock().unwrap();
        *turbovec = Some(turbovec_index);

        Ok(())
    }

    /// Ensure turbovec index is built
    ///
    /// Called during search when turbovec is needed but not available.
    /// Should only happen once at threshold crossing.
    #[cfg(feature = "turbovec")]
    fn ensure_turbovec_index(&self, index_name: &str) {
        let metadata_arc = {
            let indexes = self.hnsw_indexes.read();
            match indexes.get(index_name) {
                Some(m) => m.clone(),
                None => return,
            }
        };

        // Check if already built
        let turbovec = metadata_arc.turbovec_index.lock().unwrap();
        if turbovec.is_some() {
            return; // Already built
        }
        drop(turbovec);

        // Build if not built (should only happen once at threshold)
        eprintln!("DEBUG: ensure_turbovec_index called for {}", index_name);
        if let Err(e) = self.build_turbovec_index(index_name) {
            eprintln!("ERROR: Failed to build turbovec index: {}", e);
        }
    }

    /// Search using turbovec index (for large datasets)
    ///
    /// # Arguments
    ///
    /// * `index` - Turbovec index (must be built)
    /// * `query_vector` - Query vector
    /// * `k` - Number of neighbors to return
    ///
    /// # Returns
    ///
    /// Top-K nearest neighbors sorted by distance (closest first)
    #[cfg(feature = "turbovec")]
    fn turbovec_search(
        &self,
        index: &turbovec::IdMapIndex,
        query_vector: &[f32],
        k: usize,
    ) -> Vec<(i64, f32)> {
        // Turbovec search returns (scores, ids)
        let (scores, ids) = index.search(query_vector, k);

        // Convert to (node_id, distance) format
        scores
            .into_iter()
            .zip(ids.into_iter())
            .map(|(distance, node_id)| (node_id as i64, distance))
            .collect()
    }

    pub fn hnsw_vector_search(
        &self,
        index_name: &str,
        query_vector: &[f32],
        k: usize,
    ) -> Result<Vec<(i64, f32)>, SqliteGraphError> {
        // Get index metadata
        let metadata_arc = {
            let indexes = self.hnsw_indexes.read();
            indexes.get(index_name).cloned().ok_or_else(|| {
                SqliteGraphError::validation(format!("HNSW index not found: {}", index_name))
            })?
        };

        // Validate dimension
        if query_vector.len() != metadata_arc.dimension {
            return Err(SqliteGraphError::validation(format!(
                "Query vector dimension mismatch: expected {}, got {}",
                metadata_arc.dimension,
                query_vector.len()
            )));
        }

        // Check if we should use turbovec (large dataset)
        #[cfg(feature = "turbovec")]
        {
            let count = *metadata_arc.embedding_count.lock().unwrap();
            if count > TURBOVEC_THRESHOLD {
                // Ensure turbovec index is built
                self.ensure_turbovec_index(index_name);

                // Try turbovec search first
                let turbovec = metadata_arc.turbovec_index.lock().unwrap();
                if let Some(ref index) = *turbovec {
                    return Ok(self.turbovec_search(index, query_vector, k));
                }
            }
        }

        // Fall back to HNSW search for small datasets or if turbovec unavailable
        let hnsw = metadata_arc.hnsw_index.lock().unwrap();
        let results = hnsw
            .search(query_vector, k)
            .map_err(|e| SqliteGraphError::validation(format!("HNSW search error: {:?}", e)))?;

        // Convert u64 IDs to i64 and return
        Ok(results
            .into_iter()
            .map(|(id, dist)| (id as i64, dist))
            .collect())
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

    /// Execute raw SQL query against V3Backend's SQL Layer
    ///
    /// Returns result as vector of rows (each row = vector of columns)
    /// Column order matches SELECT clause
    ///
    /// # Safety
    ///
    /// This method accepts raw SQL strings. Caller must sanitize input.
    /// For parameterized queries, use `execute_sql_params`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let rows = backend.execute_sql("SELECT node_id, kind FROM node_properties LIMIT 10")?;
    /// for row in rows {
    ///     let node_id: i64 = row[0].as_i64()?;
    ///     let kind: String = row[1].as_str()?;
    /// }
    /// ```
    pub fn execute_sql(&self, query: &str) -> Result<Vec<SqlRow>, SqliteGraphError> {
        let conn = self.sqlite_conn.lock();

        let mut stmt = conn
            .prepare(query)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to prepare SQL: {}", e)))?;

        let column_count = stmt.column_count();
        let mut rows = Vec::new();

        let mut query_rows = stmt
            .query([])
            .map_err(|e| SqliteGraphError::connection(format!("Failed to execute SQL: {}", e)))?;

        while let Some(row) = query_rows
            .next()
            .map_err(|e| SqliteGraphError::connection(format!("Failed to fetch row: {}", e)))?
        {
            let mut sql_row = Vec::with_capacity(column_count);
            for i in 0..column_count {
                let value_ref = row.get_ref(i).map_err(|e| {
                    SqliteGraphError::connection(format!("Failed to get column: {}", e))
                })?;
                let value = SqlValue::from_sqlite_value_ref(&value_ref);
                sql_row.push(value);
            }
            rows.push(sql_row);
        }

        Ok(rows)
    }

    /// Execute parameterized SQL query safely
    ///
    /// Prevents SQL injection via parameter binding
    ///
    /// # Example
    ///
    /// ```ignore
    /// let rows = backend.execute_sql_params(
    ///     "SELECT * FROM node_properties WHERE kind = ?",
    ///     &[&"User".to_string()]
    /// )?;
    /// ```
    pub fn execute_sql_params(
        &self,
        query: &str,
        params: &[&dyn ToSql],
    ) -> Result<Vec<SqlRow>, SqliteGraphError> {
        let conn = self.sqlite_conn.lock();

        let mut stmt = conn
            .prepare(query)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to prepare SQL: {}", e)))?;

        let column_count = stmt.column_count();
        let mut rows = Vec::new();

        let query_params: Vec<&dyn ToSql> = params.to_vec();
        let mut query_rows = stmt
            .query(&*query_params)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to execute SQL: {}", e)))?;

        while let Some(row) = query_rows
            .next()
            .map_err(|e| SqliteGraphError::connection(format!("Failed to fetch row: {}", e)))?
        {
            let mut sql_row = Vec::with_capacity(column_count);
            for i in 0..column_count {
                let value_ref = row.get_ref(i).map_err(|e| {
                    SqliteGraphError::connection(format!("Failed to get column: {}", e))
                })?;
                let value = SqlValue::from_sqlite_value_ref(&value_ref);
                sql_row.push(value);
            }
            rows.push(sql_row);
        }

        Ok(rows)
    }

    /// Execute SQL statement that doesn't return rows (INSERT/UPDATE/DELETE)
    ///
    /// Returns number of rows affected
    pub fn execute_sql_update(&self, query: &str) -> Result<usize, SqliteGraphError> {
        let conn = self.sqlite_conn.lock();

        conn.execute(query, []).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to execute SQL update: {}", e))
        })
    }

    /// Execute parameterized SQL update safely
    pub fn execute_sql_update_params(
        &self,
        query: &str,
        params: &[&dyn ToSql],
    ) -> Result<usize, SqliteGraphError> {
        let conn = self.sqlite_conn.lock();

        let query_params: Vec<&dyn ToSql> = params.to_vec();
        conn.execute(query, &*query_params).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to execute SQL update: {}", e))
        })
    }

    /// Begin a new transaction
    ///
    /// Returns a transaction guard that auto-rolls back on drop unless explicitly committed.
    /// Uses DEFERRED behavior (transaction starts on first SQL statement).
    ///
    /// # Example
    ///
    /// ```ignore
    /// let tx = backend.begin_transaction()?;
    /// backend.execute_sql("INSERT INTO node_properties (node_id, kind, name, data, created_at, created_version) VALUES (1, 'User', 'alice', '{}', 0, 1)")?;
    /// backend.execute_sql("INSERT INTO edge_attributes (src, dst, attr_name, attr_value, created_version) VALUES (1, 2, 'LINKS', '{}', 1)")?;
    /// tx.commit()?; // Commits both inserts atomically
    /// ```
    pub fn begin_transaction(&self) -> Result<V3TransactionGuard<'_>, SqliteGraphError> {
        let conn = self.sqlite_conn.lock();

        conn.execute("BEGIN DEFERRED", []).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to begin transaction: {}", e))
        })?;
        drop(conn);

        if let Err(err) = self.begin_graph_transaction_frame() {
            let conn = self.sqlite_conn.lock();
            let _ = conn.execute("ROLLBACK", []);
            return Err(err);
        }

        Ok(V3TransactionGuard::new(self))
    }

    /// Create a nested transaction savepoint
    ///
    /// Savepoints allow nested transactions within a parent transaction.
    /// Auto-rolls back on drop unless explicitly committed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let tx = backend.begin_transaction()?;
    /// {
    ///     let sp = backend.savepoint("sp1")?;
    ///     // ... operations ...
    ///     sp.commit()?; // Commits savepoint but not transaction
    /// }
    /// tx.commit()?; // Commits entire transaction
    /// ```
    pub fn savepoint(&self, name: &str) -> Result<V3SavepointGuard<'_>, SqliteGraphError> {
        let conn = self.sqlite_conn.lock();

        let savepoint_sql = format!("SAVEPOINT {}", name);
        conn.execute(&savepoint_sql, []).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to create savepoint: {}", e))
        })?;
        drop(conn);

        if let Err(err) = self.begin_graph_transaction_frame() {
            let conn = self.sqlite_conn.lock();
            let rollback_sql = format!("ROLLBACK TO SAVEPOINT {}", name);
            let _ = conn.execute(&rollback_sql, []);
            let release_sql = format!("RELEASE SAVEPOINT {}", name);
            let _ = conn.execute(&release_sql, []);
            return Err(err);
        }

        Ok(V3SavepointGuard::new(self, name.to_string()))
    }

    /// Explicitly commit current transaction (guard-based preferred)
    ///
    /// This low-level method commits the current SQLite transaction.
    /// Prefer using transaction guards for automatic rollback.
    pub fn commit_transaction(&self) -> Result<(), SqliteGraphError> {
        self.flush_to_disk()?;
        let conn = self.sqlite_conn.lock();

        conn.execute("COMMIT", []).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to commit transaction: {}", e))
        })?;
        drop(conn);
        self.discard_graph_transaction_frame()?;
        Ok(())
    }

    /// Explicitly rollback current transaction (guard-based preferred)
    ///
    /// This low-level method rolls back the current SQLite transaction.
    /// Prefer using transaction guards for automatic rollback.
    pub fn rollback_transaction(&self) -> Result<(), SqliteGraphError> {
        let conn = self.sqlite_conn.lock();

        conn.execute("ROLLBACK", []).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to rollback transaction: {}", e))
        })?;
        drop(conn);
        self.rollback_graph_transaction_frame()?;
        Ok(())
    }

    fn release_savepoint(&self, name: &str) -> Result<(), SqliteGraphError> {
        let conn = self.sqlite_conn.lock();
        let release_sql = format!("RELEASE SAVEPOINT {}", name);
        conn.execute(&release_sql, []).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to release savepoint: {}", e))
        })?;
        drop(conn);
        self.discard_graph_transaction_frame()?;
        Ok(())
    }

    fn rollback_savepoint(&self, name: &str) -> Result<(), SqliteGraphError> {
        let conn = self.sqlite_conn.lock();
        let rollback_sql = format!("ROLLBACK TO SAVEPOINT {}", name);
        conn.execute(&rollback_sql, []).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to rollback to savepoint: {}", e))
        })?;
        let release_sql = format!("RELEASE SAVEPOINT {}", name);
        conn.execute(&release_sql, []).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to release rolled back savepoint: {}", e))
        })?;
        drop(conn);
        self.rollback_graph_transaction_frame()?;
        Ok(())
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

        let requested_id = node
            .data
            .get("id")
            .or_else(|| node.data.get("node_id"))
            .and_then(|v| v.as_i64());

        let node_record = if total_len <= MAX_INLINE_DATA {
            // Small data: store inline
            let mut inline_data = Vec::with_capacity(total_len);
            inline_data.push(kind_bytes.len() as u8);
            inline_data.extend_from_slice(kind_bytes);
            inline_data.push(name_bytes.len() as u8);
            inline_data.extend_from_slice(name_bytes);
            inline_data.extend_from_slice(&data_bytes);

            NodeRecordV3::new_inline(
                requested_id.unwrap_or(0),
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
            let pages_needed = data_len.div_ceil(page_size); // Ceiling division

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
                requested_id.unwrap_or(0),
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
        let node_id = node_store
            .insert_node(node_record, requested_id)
            .map_err(map_v3_error)?;

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

        // Emit NodeChanged event (only if there are subscribers)
        {
            let pub_guard = self.publisher.read();
            if let Some(ref publisher) = *pub_guard
                && publisher.has_subscribers()
            {
                let current_lsn = if let Some(ref wal) = self.wal {
                    wal.read().committed_lsn()
                } else {
                    *self.current_snapshot_version.read()
                };
                publisher.emit(
                    crate::backend::native::v3::pubsub::types::PubSubEvent::NodeChanged {
                        node_id,
                        snapshot_id: current_lsn,
                    },
                );
            }
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
    fn init_sqlite_schema(conn: &Connection) -> Result<(), SqliteGraphError> {
        // Create node_properties table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS node_properties (
                node_id INTEGER PRIMARY KEY,
                kind TEXT NOT NULL,
                name TEXT,
                data TEXT,
                created_at INTEGER NOT NULL,
                created_version INTEGER NOT NULL DEFAULT 1,
                updated_version INTEGER,
                deleted_version INTEGER
            )",
            [],
        )
        .map_err(|e| {
            SqliteGraphError::connection(format!("Failed to create node_properties table: {}", e))
        })?;

        // Create index on node_properties.kind
        conn.execute(
            "CREATE INDEX IF NOT EXISTS node_props_kind ON node_properties(kind)",
            [],
        )
        .map_err(|e| SqliteGraphError::connection(format!("Failed to create kind index: {}", e)))?;

        // Create edge_attributes table
        conn.execute(
            "CREATE TABLE IF NOT EXISTS edge_attributes (
                src INTEGER NOT NULL,
                dst INTEGER NOT NULL,
                attr_name TEXT NOT NULL,
                attr_value TEXT,
                created_version INTEGER NOT NULL DEFAULT 1,
                deleted_version INTEGER,
                PRIMARY KEY (src, dst, attr_name)
            )",
            [],
        )
        .map_err(|e| {
            SqliteGraphError::connection(format!("Failed to create edge_attributes table: {}", e))
        })?;

        // Create layout_graph table (for physical layout metadata)
        conn.execute(
            "CREATE TABLE IF NOT EXISTS layout_graph (
                table_name TEXT NOT NULL,
                column_name TEXT NOT NULL,
                rowgroup_id INTEGER,
                page_id INTEGER,
                parent_page_id INTEGER,
                PRIMARY KEY (table_name, column_name, rowgroup_id, page_id)
            )",
            [],
        )
        .map_err(|e| {
            SqliteGraphError::connection(format!("Failed to create layout_graph table: {}", e))
        })?;

        // Create index on layout_graph.parent_page_id
        conn.execute(
            "CREATE INDEX IF NOT EXISTS layout_parent ON layout_graph(parent_page_id)",
            [],
        )
        .map_err(|e| {
            SqliteGraphError::connection(format!("Failed to create parent index: {}", e))
        })?;

        Self::ensure_sqlite_column(
            conn,
            "node_properties",
            "created_version",
            "INTEGER NOT NULL DEFAULT 1",
        )?;
        Self::ensure_sqlite_column(conn, "node_properties", "updated_version", "INTEGER")?;
        Self::ensure_sqlite_column(conn, "node_properties", "deleted_version", "INTEGER")?;
        Self::ensure_sqlite_column(
            conn,
            "edge_attributes",
            "created_version",
            "INTEGER NOT NULL DEFAULT 1",
        )?;
        Self::ensure_sqlite_column(conn, "edge_attributes", "deleted_version", "INTEGER")?;

        conn.execute(
            "UPDATE node_properties SET created_version = 1 WHERE created_version IS NULL",
            [],
        )
        .map_err(|e| {
            SqliteGraphError::connection(format!("Failed to backfill node created_version: {}", e))
        })?;
        conn.execute(
            "UPDATE edge_attributes SET created_version = 1 WHERE created_version IS NULL",
            [],
        )
        .map_err(|e| {
            SqliteGraphError::connection(format!("Failed to backfill edge created_version: {}", e))
        })?;

        Ok(())
    }

    fn ensure_sqlite_column(
        conn: &Connection,
        table: &str,
        column: &str,
        definition: &str,
    ) -> Result<(), SqliteGraphError> {
        let pragma = format!("PRAGMA table_info({})", table);
        let mut stmt = conn.prepare(&pragma).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to inspect table {}: {}", table, e))
        })?;

        let mut rows = stmt.query([]).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to read table info for {}: {}", table, e))
        })?;

        while let Some(row) = rows.next().map_err(|e| {
            SqliteGraphError::connection(format!(
                "Failed to iterate table info for {}: {}",
                table, e
            ))
        })? {
            let existing: String = row.get(1).map_err(|e| {
                SqliteGraphError::connection(format!(
                    "Failed to decode column info for {}: {}",
                    table, e
                ))
            })?;
            if existing == column {
                return Ok(());
            }
        }

        let alter = format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, definition);
        conn.execute(&alter, []).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to migrate {}.{}: {}", table, column, e))
        })?;
        Ok(())
    }

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
                    if let Ok(mut file) = OpenOptions::new().read(true).open(&self.db_path)
                        && file.seek(SeekFrom::Start(offset)).is_ok()
                    {
                        let _ = file.read_exact(&mut buffer);
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
        // Validate that both endpoints exist before inserting the edge
        let from_exists = self.get_node_internal(edge.from)?.is_some();
        let to_exists = self.get_node_internal(edge.to)?.is_some();
        if !from_exists || !to_exists {
            return Err(SqliteGraphError::invalid_input(
                "edge endpoints must reference existing entities",
            ));
        }

        let edge_store = self.edge_store.write();

        let edge_type = if edge.edge_type.is_empty() {
            None
        } else {
            Some(edge.edge_type.clone())
        };

        let weight = edge
            .data
            .get("weight")
            .and_then(|v| v.as_f64())
            .map(|w| w as f32)
            .unwrap_or(1.0);

        edge_store
            .insert_edge_weighted(
                edge.from,
                edge.to,
                EdgeDirection::Outgoing,
                edge_type.clone(),
                weight,
            )
            .map_err(map_v3_error)?;
        edge_store
            .insert_edge_weighted(
                edge.to,
                edge.from,
                EdgeDirection::Incoming,
                edge_type,
                weight,
            )
            .map_err(map_v3_error)?;

        // Update header edge count (but don't sync yet)
        let mut header = self.header.write();
        header.edge_count += 1;

        let edge_id = header.edge_count as i64;

        // Emit EdgeChanged event (only if there are subscribers)
        {
            let pub_guard = self.publisher.read();
            if let Some(ref publisher) = *pub_guard
                && publisher.has_subscribers()
            {
                let current_lsn = if let Some(ref wal) = self.wal {
                    wal.read().committed_lsn()
                } else {
                    *self.current_snapshot_version.read()
                };
                publisher.emit(
                    crate::backend::native::v3::pubsub::types::PubSubEvent::EdgeChanged {
                        edge_id,
                        from_node: edge.from,
                        to_node: edge.to,
                        snapshot_id: current_lsn,
                    },
                );
            }
        }

        // Return a synthetic edge ID (edge store doesn't assign IDs yet)
        Ok(edge_id)
    }

    /// Helper: Get all node IDs in the graph (expensive, use with caution)
    fn get_all_node_ids(&self) -> Result<Vec<i64>, crate::SqliteGraphError> {
        // Collect all node IDs from the kind_index
        let all_kinds = self.kind_index.all_kinds();
        let mut all_ids = Vec::new();
        for kind in all_kinds {
            let kind_ids = self.kind_index.get(&kind);
            all_ids.extend(kind_ids);
        }

        // Remove duplicates and sort for consistency
        all_ids.sort();
        all_ids.dedup();

        Ok(all_ids)
    }

    /// Helper: Check property filters for start and end nodes
    fn check_property_filters(
        &self,
        start_id: i64,
        end_id: i64,
        pattern: &crate::pattern_engine::PatternTriple,
    ) -> Result<bool, crate::SqliteGraphError> {
        use crate::snapshot::SnapshotId;

        // Check start node properties
        if !pattern.start_props.is_empty() {
            let start_node = self.get_node(SnapshotId::current(), start_id)?;
            if !self.node_matches_properties(&start_node, &pattern.start_props) {
                return Ok(false);
            }
        }

        // Check end node properties
        if !pattern.end_props.is_empty() {
            let end_node = self.get_node(SnapshotId::current(), end_id)?;
            if !self.node_matches_properties(&end_node, &pattern.end_props) {
                return Ok(false);
            }
        }

        Ok(true)
    }

    /// Helper: Check if a node's properties match the required filters
    fn node_matches_properties(
        &self,
        node: &GraphEntity,
        required_props: &std::collections::HashMap<String, String>,
    ) -> bool {
        use serde_json::Value;

        // If node.data is not an object, can't match properties
        let data_obj = match &node.data {
            Value::Object(obj) if !obj.is_empty() => obj,
            _ => return required_props.is_empty(), // No properties to check
        };

        // Check each required property
        for (key, expected_value) in required_props {
            match data_obj.get(key) {
                Some(Value::String(actual_value)) if actual_value == expected_value => {
                    // Property matches
                }
                Some(Value::Number(n)) if n.to_string() == *expected_value => {
                    // Numeric property matches (when expected as string)
                }
                Some(Value::Bool(b)) if b.to_string() == *expected_value => {
                    // Boolean property matches (when expected as string)
                }
                _ => return false, // Property missing or doesn't match
            }
        }

        true
    }

    /// Helper: Generate synthetic edge_id for triple matching
    ///
    /// V3 edge store uses (src, dst, dir) tuples, not traditional edge_id.
    /// This generates a deterministic edge_id for compatibility.
    fn generate_edge_id(
        &self,
        src: i64,
        dst: i64,
        direction: crate::backend::native::v3::edge_compat::Direction,
        edge_type: &str,
    ) -> i64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Create a deterministic hash-based edge_id
        let mut hasher = DefaultHasher::new();
        src.hash(&mut hasher);
        dst.hash(&mut hasher);
        direction.hash(&mut hasher);
        edge_type.hash(&mut hasher);

        // Use absolute value of hash to ensure positive edge_id
        let hash = hasher.finish();
        let edge_id = (hash % i64::MAX as u64) as i64;

        // Ensure edge_id is at least 1 (positive IDs required)
        edge_id.max(1)
    }

    /// Get node properties from SQLite
    pub fn get_node_properties(
        &self,
        node_id: i64,
    ) -> Result<Option<(String, String, serde_json::Value)>, SqliteGraphError> {
        let conn = self.sqlite_conn.lock();

        // First check if node exists
        let mut stmt = conn
            .prepare("SELECT kind, name, data FROM node_properties WHERE node_id = ?1")
            .map_err(|e| SqliteGraphError::connection(format!("Failed to prepare query: {}", e)))?;

        let mut rows = stmt
            .query([node_id])
            .map_err(|e| SqliteGraphError::connection(format!("Failed to query: {}", e)))?;

        let result = rows
            .next()
            .map_err(|e| SqliteGraphError::connection(format!("Failed to fetch row: {}", e)))?;

        match result {
            Some(row) => {
                let kind: String = row.get(0).map_err(|e| {
                    SqliteGraphError::connection(format!("Failed to get kind: {}", e))
                })?;
                let name: String = row.get(1).map_err(|e| {
                    SqliteGraphError::connection(format!("Failed to get name: {}", e))
                })?;
                let data_str: String = row.get(2).map_err(|e| {
                    SqliteGraphError::connection(format!("Failed to get data: {}", e))
                })?;
                let data = serde_json::from_str(&data_str).unwrap_or(serde_json::Value::Null);
                Ok(Some((kind, name, data)))
            }
            None => Ok(None),
        }
    }

    /// Get edge attributes from SQLite
    pub fn get_edge_attributes(
        &self,
        src: i64,
        dst: i64,
    ) -> Result<Vec<(String, serde_json::Value)>, SqliteGraphError> {
        let conn = self.sqlite_conn.lock();
        let mut stmt = conn
            .prepare(
                "SELECT attr_name, attr_value FROM edge_attributes WHERE src = ?1 AND dst = ?2",
            )
            .map_err(|e| {
                SqliteGraphError::connection(format!(
                    "Failed to prepare edge attributes query: {}",
                    e
                ))
            })?;

        let mut attributes = Vec::new();
        let mut rows = stmt.query([src, dst]).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to query edge attributes: {}", e))
        })?;

        loop {
            let result = rows
                .next()
                .map_err(|e| SqliteGraphError::connection(format!("Failed to fetch row: {}", e)))?;
            match result {
                Some(row) => {
                    let attr_name: String = row.get(0).map_err(|e| {
                        SqliteGraphError::connection(format!("Failed to get attr_name: {}", e))
                    })?;
                    let attr_value_str: String = row.get(1).map_err(|e| {
                        SqliteGraphError::connection(format!("Failed to get attr_value: {}", e))
                    })?;
                    let attr_value =
                        serde_json::from_str(&attr_value_str).unwrap_or(serde_json::Value::Null);
                    attributes.push((attr_name, attr_value));
                }
                None => break,
            }
        }

        Ok(attributes)
    }
}

/// SQL value representation for query results
#[derive(Debug, Clone)]
pub enum SqlValue {
    Null,
    Integer(i64),
    Real(f64),
    Text(String),
    Blob(Vec<u8>),
}

impl SqlValue {
    fn from_sqlite_value_ref(value_ref: &rusqlite::types::ValueRef) -> Self {
        match value_ref {
            rusqlite::types::ValueRef::Null => SqlValue::Null,
            rusqlite::types::ValueRef::Integer(i) => SqlValue::Integer(*i),
            rusqlite::types::ValueRef::Real(r) => SqlValue::Real(*r),
            rusqlite::types::ValueRef::Text(t) => {
                let text = std::str::from_utf8(t).unwrap_or("<invalid utf8>");
                SqlValue::Text(text.to_owned())
            }
            rusqlite::types::ValueRef::Blob(b) => SqlValue::Blob(b.to_vec()),
        }
    }

    pub fn as_i64(&self) -> Result<i64, SqliteGraphError> {
        match self {
            SqlValue::Integer(i) => Ok(*i),
            _ => Err(SqliteGraphError::validation("Expected integer value")),
        }
    }

    pub fn as_f64(&self) -> Result<f64, SqliteGraphError> {
        match self {
            SqlValue::Real(r) => Ok(*r),
            SqlValue::Integer(i) => Ok(*i as f64),
            _ => Err(SqliteGraphError::validation("Expected numeric value")),
        }
    }

    pub fn as_str(&self) -> Result<&str, SqliteGraphError> {
        match self {
            SqlValue::Text(s) => Ok(s),
            _ => Err(SqliteGraphError::validation("Expected text value")),
        }
    }

    pub fn as_blob(&self) -> Result<&[u8], SqliteGraphError> {
        match self {
            SqlValue::Blob(b) => Ok(b),
            _ => Err(SqliteGraphError::validation("Expected blob value")),
        }
    }
}

/// SQL query result row
pub type SqlRow = Vec<SqlValue>;

/// Transaction guard for V3Backend
///
/// Automatically rolls back on drop unless explicitly committed.
/// Use with RAII pattern for exception safety.
pub struct V3TransactionGuard<'a> {
    backend: &'a V3Backend,
    committed: bool,
}

impl<'a> V3TransactionGuard<'a> {
    fn new(backend: &'a V3Backend) -> Self {
        Self {
            backend,
            committed: false,
        }
    }

    /// Commit the transaction
    ///
    /// Commits all changes made within this transaction.
    /// Subsequent operations will start a new auto-transaction.
    pub fn commit(mut self) -> Result<(), SqliteGraphError> {
        self.committed = true;
        self.backend.commit_transaction()
    }

    /// Rollback the transaction
    ///
    /// Rolls back all changes made within this transaction.
    /// Transaction is also rolled back on drop if not committed.
    pub fn rollback(mut self) -> Result<(), SqliteGraphError> {
        self.committed = true;
        self.backend.rollback_transaction()
    }
}

impl<'a> Drop for V3TransactionGuard<'a> {
    fn drop(&mut self) {
        if !self.committed {
            // Auto-rollback on drop
            let _ = self.backend.rollback_transaction();
        }
    }
}

/// Savepoint guard for nested transactions
///
/// Automatically rolls back savepoint on drop unless explicitly committed.
pub struct V3SavepointGuard<'a> {
    backend: &'a V3Backend,
    name: String,
    committed: bool,
}

impl<'a> V3SavepointGuard<'a> {
    fn new(backend: &'a V3Backend, name: String) -> Self {
        Self {
            backend,
            name,
            committed: false,
        }
    }

    /// Commit the savepoint
    ///
    /// Commits all changes made within this savepoint.
    /// Parent transaction remains active.
    pub fn commit(mut self) -> Result<(), SqliteGraphError> {
        self.committed = true;
        self.backend.release_savepoint(&self.name)
    }

    /// Rollback the savepoint
    ///
    /// Rolls back all changes made within this savepoint.
    /// Parent transaction remains active.
    pub fn rollback(mut self) -> Result<(), SqliteGraphError> {
        self.committed = true;
        self.backend.rollback_savepoint(&self.name)
    }
}

impl<'a> Drop for V3SavepointGuard<'a> {
    fn drop(&mut self) {
        if !self.committed {
            // Auto-rollback on drop
            let _ = self.backend.rollback_savepoint(&self.name);
        }
    }
}

impl GraphBackend for V3Backend {
    /// Check if snapshot is supported by V3 backend.
    ///
    /// V3 only supports SnapshotId::current() (which returns 0).
    /// Any historical/non-zero snapshot ID is rejected with a clear error.
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        // Extract properties for SQLite storage
        let kind = node.kind.clone();
        let name = node.name.clone();
        let data = node.data.clone();
        let next_version = self.next_graph_version();

        // Use inner method - do NOT auto-flush to allow batching multiple nodes
        // Caller must call flush() for durability
        let node_id = self.insert_node_inner(node)?;
        self.sync_header()?;

        // Store node properties in SQLite
        let data_json = serde_json::to_string(&data).unwrap_or_default();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let conn = self.sqlite_conn.lock();
        conn.execute(
            "INSERT INTO node_properties (node_id, kind, name, data, created_at, created_version) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            [
                &node_id as &dyn ToSql,
                &kind as &dyn ToSql,
                &name as &dyn ToSql,
                &data_json as &dyn ToSql,
                &timestamp as &dyn ToSql,
                &next_version as &dyn ToSql,
            ],
        ).map_err(|e| SqliteGraphError::connection(format!("Failed to insert node properties: {}", e)))?;
        drop(conn);

        // Increment snapshot version after successful insert
        self.advance_snapshot_version();

        Ok(node_id)
    }

    fn insert_edge(&self, edge: EdgeSpec) -> Result<i64, SqliteGraphError> {
        // Extract edge attributes for SQLite storage
        let from = edge.from;
        let to = edge.to;
        let edge_type = edge.edge_type.clone();
        let data_json = serde_json::to_string(&edge.data).unwrap_or_default();
        let next_version = self.next_graph_version();

        // Use inner method - do NOT auto-flush to allow batching multiple edges
        // Caller must call flush() for durability
        let edge_id = self.insert_edge_inner(edge)?;
        self.sync_header()?;

        // Store edge attributes in SQLite
        let conn = self.sqlite_conn.lock();
        conn.execute(
            "INSERT OR REPLACE INTO edge_attributes (src, dst, attr_name, attr_value, created_version) VALUES (?1, ?2, ?3, ?4, ?5)",
            [
                &from as &dyn ToSql,
                &to as &dyn ToSql,
                &edge_type as &dyn ToSql,
                &data_json as &dyn ToSql,
                &next_version as &dyn ToSql,
            ],
        ).map_err(|e| SqliteGraphError::connection(format!("Failed to insert edge attributes: {}", e)))?;
        drop(conn);

        // Increment snapshot version after successful insert
        self.advance_snapshot_version();

        Ok(edge_id)
    }

    fn update_node(&self, node_id: i64, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        let kind = node.kind.clone();
        let name = node.name.clone();
        let data_json = serde_json::to_string(&node.data).unwrap_or_default();
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

        let conn = self.sqlite_conn.lock();
        conn.execute(
            "UPDATE node_properties SET kind = ?1, name = ?2, data = ?3, updated_version = ?4 WHERE node_id = ?5 AND deleted_version IS NULL",
            [
                &kind as &dyn ToSql,
                &name as &dyn ToSql,
                &data_json as &dyn ToSql,
                &next_version as &dyn ToSql,
                &node_id as &dyn ToSql,
            ],
        ).map_err(|e| SqliteGraphError::connection(format!("Failed to update node properties: {}", e)))?;
        drop(conn);

        self.rebuild_indexes();

        // Increment snapshot version after successful update
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

        let conn = self.sqlite_conn.lock();
        conn.execute(
            "UPDATE node_properties SET deleted_version = ?1 WHERE node_id = ?2 AND deleted_version IS NULL",
            [&next_version as &dyn ToSql, &id as &dyn ToSql],
        ).map_err(|e| SqliteGraphError::connection(format!("Failed to mark node deleted: {}", e)))?;
        drop(conn);

        self.rebuild_indexes();

        // Increment snapshot version after successful delete
        self.advance_snapshot_version();

        Ok(())
    }

    fn entity_ids(&self) -> Result<Vec<i64>, SqliteGraphError> {
        let node_store = self.node_store.read();
        let ids = node_store.node_ids().map_err(map_v3_error)?;
        Ok(ids)
    }

    fn get_node(&self, snapshot_id: SnapshotId, id: i64) -> Result<GraphEntity, SqliteGraphError> {
        let mut deleted_before_present: Option<u64> = None;

        // MVCC check: if snapshot is historical (non-zero LSN), verify node existed at that time
        if snapshot_id.0 != 0 {
            let conn = self.sqlite_conn.lock();
            let mut stmt = conn.prepare(
                "SELECT created_version, updated_version, deleted_version FROM node_properties WHERE node_id = ?1"
            )
                .map_err(|e| SqliteGraphError::connection(format!("Failed to prepare query: {}", e)))?;

            let mut rows = stmt
                .query([id])
                .map_err(|e| SqliteGraphError::connection(format!("Failed to query: {}", e)))?;

            if let Some(row) = rows
                .next()
                .map_err(|e| SqliteGraphError::connection(format!("Failed to fetch row: {}", e)))?
            {
                let created_version: u64 = row.get(0).map_err(|e| {
                    SqliteGraphError::connection(format!("Failed to get created_version: {}", e))
                })?;
                let updated_version: Option<u64> = row.get(1).map_err(|e| {
                    SqliteGraphError::connection(format!("Failed to get updated_version: {}", e))
                })?;
                let deleted_version: Option<u64> = row.get(2).map_err(|e| {
                    SqliteGraphError::connection(format!("Failed to get deleted_version: {}", e))
                })?;

                if created_version > snapshot_id.0 {
                    return Err(SqliteGraphError::query(format!(
                        "Node {} not found in snapshot",
                        id
                    )));
                }

                if let Some(version) = updated_version
                    && snapshot_id.0 < version
                {
                    return Err(SqliteGraphError::query(format!(
                        "Historical version for node {} is unavailable after update",
                        id
                    )));
                }

                if let Some(version) = deleted_version {
                    if snapshot_id.0 >= version {
                        return Err(SqliteGraphError::query(format!(
                            "Node {} not found in snapshot",
                            id
                        )));
                    }
                    deleted_before_present = Some(version);
                }
            } else {
                // Node not in properties table at all - not found
                return Err(SqliteGraphError::query(format!("Node {} not found", id)));
            }
        }

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
        // Edge type filtering not yet wired for V3 backend.
        // neighbors_filtered exists on edge_store but typed-edge traversal
        // requires mapping edge_type strings to internal type IDs.
        // Tracked; for now delegate to unfiltered k_hop.
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
        // Edge type filtering not yet wired for V3 backend.
        // V3's edge_store exposes `neighbors_filtered`, but typed-edge traversal
        // requires mapping edge_type strings to internal type IDs.
        // Tracked; for now delegate to unfiltered bfs to match the stub pattern.
        self.bfs(snapshot_id, start, depth)
    }

    fn shortest_path_filtered(
        &self,
        snapshot_id: SnapshotId,
        start: i64,
        end: i64,
        _allowed_edge_types: &[&str],
    ) -> Result<Option<Vec<i64>>, SqliteGraphError> {
        // Edge type filtering not yet wired for V3 backend.
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
                    // kind filtering deferred: step.target_kind not yet wired to node_store kind index
                    // Nodes currently store kind in inline data only.
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
        _import_dir: &Path,
    ) -> Result<crate::backend::ImportMetadata, SqliteGraphError> {
        Err(SqliteGraphError::Unsupported(
            "snapshot_import is not supported on the V3 backend".to_string(),
        ))
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
            // Substring match (V3 specific design difference from SQLite GLOB)
            Ok(self.name_index.get_substring(pattern))
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
    fn test_snapshot_import_unsupported() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");
        let backend = V3Backend::create(&db_path).unwrap();
        let result = backend.snapshot_import(temp_dir.path());
        assert!(matches!(result, Err(SqliteGraphError::Unsupported(_))));
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
