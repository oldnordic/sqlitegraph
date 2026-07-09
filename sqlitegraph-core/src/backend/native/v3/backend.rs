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
    clippy::manual_async_fn,
    reason = "AsyncGraphBackend trait requires impl Future"
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
#[path = "kv_pubsub_support.rs"]
mod kv_pubsub_support;
#[path = "lifecycle_support.rs"]
mod lifecycle_support;
#[path = "maintenance_support.rs"]
mod maintenance_support;
#[path = "mutation_support.rs"]
mod mutation_support;
#[path = "neighbor_support.rs"]
mod neighbor_support;
#[path = "pattern_support.rs"]
mod pattern_support;
#[path = "property_support.rs"]
mod property_support;
#[path = "query_support.rs"]
mod query_support;
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
use crate::backend::native::v3::wal::{V3WALPaths, WALWriter};
use crate::backend::native::v3::{
    AsyncFileCoordinator, FileCoordinator, KindIndex, KvStore, NodeCache, NodeRecordV3, NodeStore,
    PageAllocator, PersistentHeaderV3, Publisher, V3EdgeStore,
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
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::sync::Arc;
pub use transaction_guard::{V3SavepointGuard, V3TransactionGuard};
use transaction_state::GraphTransactionState;

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
        self.pattern_search_matches(snapshot_id, start, pattern)
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
        self.query_nodes_by_kind_support(snapshot_id, kind)
    }

    fn query_nodes_by_name_pattern(
        &self,
        snapshot_id: SnapshotId,
        pattern: &str,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        self.query_nodes_by_name_pattern_support(snapshot_id, pattern)
    }

    fn kv_get(
        &self,
        snapshot_id: SnapshotId,
        key: &[u8],
    ) -> Result<Option<crate::backend::native::types::KvValue>, SqliteGraphError> {
        self.kv_get_v2(snapshot_id, key)
    }

    fn kv_set(
        &self,
        key: Vec<u8>,
        value: crate::backend::native::types::KvValue,
        ttl_seconds: Option<u64>,
    ) -> Result<(), SqliteGraphError> {
        self.kv_set_v2(key, value, ttl_seconds)
    }

    fn kv_delete(&self, key: &[u8]) -> Result<(), SqliteGraphError> {
        self.kv_delete_v2(key)
    }

    fn subscribe(
        &self,
        filter: crate::backend::SubscriptionFilter,
    ) -> Result<(u64, std::sync::mpsc::Receiver<crate::backend::PubSubEvent>), SqliteGraphError>
    {
        self.subscribe_pubsub(filter)
    }

    fn unsubscribe(&self, subscriber_id: u64) -> Result<bool, SqliteGraphError> {
        self.unsubscribe_pubsub(subscriber_id)
    }

    fn kv_prefix_scan(
        &self,
        snapshot_id: SnapshotId,
        prefix: &[u8],
    ) -> Result<Vec<(Vec<u8>, crate::backend::native::types::KvValue)>, SqliteGraphError> {
        self.kv_prefix_scan_v2(snapshot_id, prefix)
    }

    fn match_triples(
        &self,
        pattern: &crate::pattern_engine::PatternTriple,
    ) -> Result<Vec<crate::pattern_engine::TripleMatch>, crate::SqliteGraphError> {
        self.match_triples_support(pattern)
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
#[path = "backend_tests.rs"]
mod tests;
