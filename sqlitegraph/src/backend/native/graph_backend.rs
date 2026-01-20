//! Native GraphBackend implementation with interior mutability.

use super::adjacency::{AdjacencyHelpers, Direction};
use super::edge_store::EdgeStore;
use super::graph_file::GraphFile;
use super::graph_ops::*;
use super::graph_validation::*;
use super::node_store::NodeStore;
use super::types::*;
use crate::SqliteGraphError;
use crate::backend::{
    BackendDirection, ChainStep, EdgeSpec, GraphBackend, NeighborQuery, NodeSpec, PatternMatch,
    PatternQuery,
};
use crate::graph::GraphEntity;
use parking_lot::RwLock;
use std::sync::Arc;

// V2 WAL integration (always available when native-v2 feature is enabled)
#[cfg(feature = "native-v2")]
use crate::backend::native::v2::wal::{
    V2GraphWALIntegrator, V2WALConfig, GraphWALIntegrationConfig,
};

/// Native backend implementation using interior mutability
pub struct NativeGraphBackend {
    graph_file: RwLock<GraphFile>,
    /// WAL integrator for transaction management and checkpointing
    /// Always available when native-v2 feature is enabled (production ready)
    #[cfg(feature = "native-v2")]
    wal_integrator: Option<Arc<V2GraphWALIntegrator>>,
}

impl NativeGraphBackend {
    /// Create a new native backend with an in-memory temporary file
    #[cfg(test)]
    pub fn new_temp() -> Result<Self, SqliteGraphError> {
        use tempfile::NamedTempFile;
        let temp_file =
            NamedTempFile::new().map_err(|e| SqliteGraphError::connection(e.to_string()))?;
        let path = temp_file.path();
        let graph_file = GraphFile::create(path).map_err(map_to_graph_error)?;

        #[cfg(feature = "native-v2")]
        let wal_integrator = Some(Self::create_wal_integrator(path)?);

        Ok(Self {
            graph_file: RwLock::new(graph_file),
            #[cfg(feature = "native-v2")]
            wal_integrator,
        })
    }

    /// Create a new native backend at the specified path
    pub fn new<P: AsRef<std::path::Path>>(path: P) -> Result<Self, SqliteGraphError> {
        let graph_file = GraphFile::create(&path).map_err(map_to_graph_error)?;

        #[cfg(feature = "native-v2")]
        let wal_integrator = Some(Self::create_wal_integrator(&path)?);

        Ok(Self {
            graph_file: RwLock::new(graph_file),
            #[cfg(feature = "native-v2")]
            wal_integrator,
        })
    }

    /// Open an existing native backend from the specified path
    pub fn open<P: AsRef<std::path::Path>>(path: P) -> Result<Self, SqliteGraphError> {
        let graph_file = GraphFile::open(&path).map_err(map_to_graph_error)?;

        #[cfg(feature = "native-v2")]
        let wal_integrator = Some(Self::create_wal_integrator(&path)?);

        Ok(Self {
            graph_file: RwLock::new(graph_file),
            #[cfg(feature = "native-v2")]
            wal_integrator,
        })
    }

    /// Create WAL integrator for the graph
    #[cfg(feature = "native-v2")]
    fn create_wal_integrator<P: AsRef<std::path::Path>>(path: P) -> Result<Arc<V2GraphWALIntegrator>, SqliteGraphError> {
        let path_ref = path.as_ref();

        // Use the helper function to create WAL config with correct paths
        let wal_config = V2WALConfig::for_graph_file(path_ref);

        // Create integration config with default settings
        let integration_config = GraphWALIntegrationConfig::default();

        // Create the integrator
        let integrator = V2GraphWALIntegrator::create(wal_config, integration_config)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to create WAL integrator: {:?}", e)))?;

        Ok(Arc::new(integrator))
    }

    /// Get mutable access to the underlying graph file for internal operations
    fn with_graph_file<R, F>(&self, f: F) -> Result<R, SqliteGraphError>
    where
        F: FnOnce(&mut GraphFile) -> Result<R, NativeBackendError>,
    {
        let mut graph_file = self.graph_file.write();
        f(&mut *graph_file).map_err(map_to_graph_error)
    }

    /// Get WAL metrics (if native-v2 feature is enabled and WAL integrator exists)
    #[cfg(feature = "native-v2")]
    pub fn get_wal_metrics(&self) -> Option<crate::backend::native::v2::wal::WALManagerMetrics> {
        self.wal_integrator.as_ref().map(|integrator| integrator.get_metrics())
    }

    /// Get active transaction count (if native-v2 feature is enabled and WAL integrator exists)
    #[cfg(feature = "native-v2")]
    pub fn get_active_transaction_count(&self) -> Option<usize> {
        self.wal_integrator.as_ref().map(|integrator| integrator.get_active_transaction_count())
    }
}

impl GraphBackend for NativeGraphBackend {
    fn insert_node(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let mut node_store = NodeStore::new(graph_file);
            let node_id = node_store.allocate_node_id()?;

            // Phase 31: V2 is now the default format (no feature gating)
            let record_v2 = crate::backend::native::v2::node_record_v2::NodeRecordV2::new(
                node_id, node.kind, node.name, node.data,
            );
            node_store.write_node_v2(&record_v2)?;

            Ok(node_id as i64)
        })
    }

    fn get_node(&self, id: i64) -> Result<GraphEntity, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let mut node_store = NodeStore::new(graph_file);
            let record = node_store.read_node(id as NativeNodeId)?;
            Ok(node_record_to_entity(record))
        })
    }

    fn insert_edge(&self, edge: EdgeSpec) -> Result<i64, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            // Phase 44.2: Use V2 clustered adjacency when experimental feature is enabled
            // Phase 44.2: Use V2 clustered adjacency when experimental feature is enabled
            #[cfg(feature = "v2_experimental")]
            {
                let mut edge_store = EdgeStore::new(graph_file);
                let edge_id = edge_store.allocate_edge_id();
                let record = edge_spec_to_record(edge, edge_id);

                // write_edge already calls update_v2_clustered_adjacency internally
                edge_store.write_edge(&record)?;
                Ok(edge_id as i64)
            }
            #[cfg(not(feature = "v2_experimental"))]
            {
                let mut edge_store = EdgeStore::new(graph_file);
                let edge_id = edge_store.allocate_edge_id();
                let record = edge_spec_to_record(edge, edge_id);
                edge_store.write_edge(&record)?;
                Ok(edge_id as i64)
            }
        })
    }

    fn neighbors(&self, node: i64, query: NeighborQuery) -> Result<Vec<i64>, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let node_id = node as NativeNodeId;

            let neighbors = if let Some(edge_type) = &query.edge_type {
                let edge_type_ref = edge_type.as_str();
                match query.direction {
                    BackendDirection::Outgoing => {
                        AdjacencyHelpers::get_outgoing_neighbors_filtered(
                            graph_file,
                            node_id,
                            &[edge_type_ref],
                        )
                    }
                    BackendDirection::Incoming => {
                        AdjacencyHelpers::get_incoming_neighbors_filtered(
                            graph_file,
                            node_id,
                            &[edge_type_ref],
                        )
                    }
                }
            } else {
                match query.direction {
                    BackendDirection::Outgoing => {
                        AdjacencyHelpers::get_outgoing_neighbors(graph_file, node_id)
                    }
                    BackendDirection::Incoming => {
                        AdjacencyHelpers::get_incoming_neighbors(graph_file, node_id)
                    }
                }
            }?;

            Ok(neighbors.into_iter().map(|id| id as i64).collect())
        })
    }

    fn bfs(&self, start: i64, depth: u32) -> Result<Vec<i64>, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let result = native_bfs(graph_file, start as NativeNodeId, depth)?;
            Ok(result.into_iter().map(|id| id as i64).collect())
        })
    }

    fn shortest_path(&self, start: i64, end: i64) -> Result<Option<Vec<i64>>, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let result =
                native_shortest_path(graph_file, start as NativeNodeId, end as NativeNodeId)?;
            Ok(result.map(|path| path.into_iter().map(|id| id as i64).collect()))
        })
    }

    fn node_degree(&self, node: i64) -> Result<(usize, usize), SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let node_id = node as NativeNodeId;
            let outgoing = AdjacencyHelpers::outgoing_degree(graph_file, node_id)?;
            let incoming = AdjacencyHelpers::incoming_degree(graph_file, node_id)?;
            Ok((outgoing as usize, incoming as usize))
        })
    }

    fn k_hop(
        &self,
        start: i64,
        depth: u32,
        direction: BackendDirection,
    ) -> Result<Vec<i64>, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let result = native_k_hop(
                graph_file,
                start as NativeNodeId,
                depth,
                match direction {
                    BackendDirection::Outgoing => Direction::Outgoing,
                    BackendDirection::Incoming => Direction::Incoming,
                },
            )?;
            Ok(result.into_iter().map(|id| id as i64).collect())
        })
    }

    fn k_hop_filtered(
        &self,
        start: i64,
        depth: u32,
        direction: BackendDirection,
        allowed_edge_types: &[&str],
    ) -> Result<Vec<i64>, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let result = native_k_hop_filtered(
                graph_file,
                start as NativeNodeId,
                depth,
                match direction {
                    BackendDirection::Outgoing => Direction::Outgoing,
                    BackendDirection::Incoming => Direction::Incoming,
                },
                allowed_edge_types,
            )?;
            Ok(result.into_iter().map(|id| id as i64).collect())
        })
    }

    fn chain_query(&self, start: i64, chain: &[ChainStep]) -> Result<Vec<i64>, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            let result = native_chain_query(graph_file, start as NativeNodeId, chain)?;
            Ok(result.into_iter().map(|id| id as i64).collect())
        })
    }

    fn pattern_search(
        &self,
        start: i64,
        pattern: &PatternQuery,
    ) -> Result<Vec<PatternMatch>, SqliteGraphError> {
        self.with_graph_file(|graph_file| {
            native_pattern_search(graph_file, start as NativeNodeId, pattern)
        })
    }

    fn checkpoint(&self) -> Result<(), SqliteGraphError> {
        #[cfg(feature = "native-v2")]
        {
            if let Some(ref integrator) = self.wal_integrator {
                integrator
                    .force_checkpoint()
                    .map_err(|e| SqliteGraphError::connection(format!("WAL checkpoint failed: {:?}", e)))?;
                return Ok(());
            }
        }

        // If native-v2 feature is not enabled, checkpoint is a no-op
        Ok(())
    }

    fn snapshot_export(&self, export_dir: &std::path::Path) -> Result<crate::backend::SnapshotMetadata, SqliteGraphError> {
        use crate::backend::native::v2::export::SnapshotExporter;
        use crate::backend::native::v2::export::snapshot::SnapshotExportConfig;
        use std::time::{SystemTime, UNIX_EPOCH};

        // Get the graph file path from the GraphFile
        let graph_path = self.with_graph_file(|graph_file| {
            Ok(graph_file.path().to_path_buf())
        })?;

        // Create snapshot exporter with default config
        let snapshot_id = format!("snapshot_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs());

        let config = SnapshotExportConfig {
            export_path: export_dir.to_path_buf(),
            snapshot_id: snapshot_id.clone(),
            include_statistics: true,
            min_stable_duration: std::time::Duration::from_secs(0),
            checksum_validation: true,
        };

        let mut exporter = SnapshotExporter::new(&graph_path, config)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to create snapshot exporter: {:?}", e)))?;

        let result = exporter.export_snapshot()
            .map_err(|e| SqliteGraphError::connection(format!("Snapshot export failed: {:?}", e)))?;

        Ok(crate::backend::SnapshotMetadata {
            snapshot_path: result.snapshot_path,
            size_bytes: result.snapshot_size_bytes,
            entity_count: 0, // Snapshot export doesn't return entity count directly
            edge_count: 0,
        })
    }

    fn backup(&self, backup_dir: &std::path::Path) -> Result<crate::backend::BackupResult, SqliteGraphError> {
        #[cfg(feature = "native-v2")]
        {
            use crate::backend::native::v2::backup;

            // Get the graph file path from the GraphFile
            let graph_path = self.with_graph_file(|graph_file| {
                Ok(graph_file.path().to_path_buf())
            })?;

            // Create backup with default configuration (includes checkpoint)
            let native_result = backup::create_backup(
                &graph_path,
                backup::BackupConfig::new(backup_dir),
            ).map_err(|e| SqliteGraphError::connection(format!("Backup failed: {:?}", e)))?;

            Ok(crate::backend::BackupResult {
                snapshot_path: native_result.snapshot_path,
                manifest_path: native_result.manifest_path,
                size_bytes: native_result.size_bytes,
                checksum: native_result.checksum,
                record_count: native_result.record_count,
                duration_secs: native_result.duration_secs,
                timestamp: native_result.timestamp,
                checkpoint_performed: native_result.checkpoint_performed,
            })
        }

        #[cfg(not(feature = "native-v2"))]
        {
            let _ = backup_dir;
            Err(SqliteGraphError::connection("Backup not available without native-v2 feature".to_string()))
        }
    }

    fn snapshot_import(&self, import_dir: &std::path::Path) -> Result<crate::backend::ImportMetadata, SqliteGraphError> {
        use crate::backend::native::v2::import::SnapshotImporter;
        use crate::backend::native::v2::import::snapshot::SnapshotImportConfig;
        use crate::backend::native::v2::import::ImportMode;

        // Get the graph file path
        let graph_path = self.with_graph_file(|graph_file| {
            Ok(graph_file.path().to_path_buf())
        })?;

        let config = SnapshotImportConfig {
            target_graph_path: graph_path.clone(),
            export_dir_path: import_dir.to_path_buf(),
            import_mode: ImportMode::Fresh,
            validate_manifest: true,
            verify_checksum: true,
            overwrite_existing: true, // Allow overwriting for import
        };

        let importer = SnapshotImporter::from_export_dir(import_dir, &graph_path, config)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to create snapshot importer: {:?}", e)))?;

        let result = importer.import()
            .map_err(|e| SqliteGraphError::connection(format!("Snapshot import failed: {:?}", e)))?;

        Ok(crate::backend::ImportMetadata {
            snapshot_path: import_dir.join("snapshot"), // Approximate path
            entities_imported: result.records_imported,
            edges_imported: 0, // Records include both entities and edges
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_backend_creation() {
        let backend = NativeGraphBackend::new_temp().unwrap();
        // Test that backend can be created successfully
        assert!(true);
    }

    #[test]
    fn test_interior_mutability() {
        let backend = NativeGraphBackend::new_temp().unwrap();

        // Test that we can perform multiple operations
        let node_id = backend
            .insert_node(NodeSpec {
                kind: "Test".to_string(),
                name: "node1".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        let node = backend.get_node(node_id).unwrap();
        assert_eq!(node.name, "node1");
        assert_eq!(node.kind, "Test");
    }
}
