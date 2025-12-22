//! V2 WAL integration with graph file operations.
//!
//! This module provides the integration layer between the WAL system and V2 graph
//! file operations, ensuring atomicity, consistency, and durability for all V2
//! clustered edge operations with automatic WAL transaction management.

use crate::backend::native::v2::wal::{
    TransactionIsolation, V2WALConfig, V2WALManager, V2WALRecord,
};
use crate::backend::native::v2::{
    CompactEdgeRecord, Direction, NodeRecordV2,
};
use crate::backend::native::{NativeBackendError, NativeResult};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// V2 WAL integration manager for graph file operations
pub struct V2GraphWALIntegrator {
    /// WAL manager instance
    wal_manager: Arc<V2WALManager>,

    /// Active graph transactions
    graph_transactions: Arc<RwLock<HashMap<u64, GraphTransaction>>>,

    /// Integration configuration
    config: GraphWALIntegrationConfig,
}

/// Configuration for V2 WAL graph integration
#[derive(Debug, Clone)]
pub struct GraphWALIntegrationConfig {
    /// Enable automatic WAL checkpointing
    pub auto_checkpoint: bool,

    /// Checkpoint interval in number of transactions
    pub checkpoint_interval: u64,

    /// Enable cluster-affinity optimization
    pub cluster_affinity: bool,

    /// Enable compression for WAL records
    pub enable_compression: bool,

    /// Maximum batch size for group commits
    pub max_batch_size: usize,

    /// Enable synchronous writes for durability
    pub sync_writes: bool,
}

/// Active graph transaction context
#[derive(Debug)]
struct GraphTransaction {
    /// WAL transaction ID
    wal_tx_id: u64,

    /// Transaction isolation level
    isolation_level: TransactionIsolation,

    /// Modified nodes in this transaction
    modified_nodes: Vec<i64>,

    /// Modified clusters in this transaction
    modified_clusters: Vec<(i64, Direction)>,

    /// Transaction start timestamp
    start_time: std::time::Instant,
}

/// Graph operation result with WAL integration
#[derive(Debug, Clone)]
pub struct GraphOperationResult {
    /// Success status
    pub success: bool,

    /// WAL LSN for the operation
    pub lsn: Option<u64>,

    /// Transaction ID (if applicable)
    pub tx_id: Option<u64>,

    /// Operation metrics
    pub metrics: OperationMetrics,
}

/// Operation metrics for performance monitoring
#[derive(Debug, Clone, Default)]
pub struct OperationMetrics {
    /// Operation duration in microseconds
    pub duration_us: u64,

    /// Number of WAL records written
    pub wal_records_written: u64,

    /// Bytes written to WAL
    pub bytes_written: u64,

    /// Number of V2 nodes affected
    pub nodes_affected: u32,

    /// Number of V2 clusters affected
    pub clusters_affected: u32,

    /// Number of edges affected
    pub edges_affected: u32,
}

impl Default for GraphWALIntegrationConfig {
    fn default() -> Self {
        Self {
            auto_checkpoint: true,
            checkpoint_interval: 1000,
            cluster_affinity: true,
            enable_compression: false,
            max_batch_size: 50,
            sync_writes: true,
        }
    }
}

impl V2GraphWALIntegrator {
    /// Create a new V2 graph WAL integrator
    pub fn create(
        wal_config: V2WALConfig,
        integration_config: GraphWALIntegrationConfig,
    ) -> NativeResult<Self> {
        let wal_manager = Arc::new(V2WALManager::create(wal_config)?);

        Ok(Self {
            wal_manager,
            graph_transactions: Arc::new(RwLock::new(HashMap::new())),
            config: integration_config,
        })
    }

    /// Begin a graph operation transaction
    pub fn begin_transaction(&self, isolation_level: TransactionIsolation) -> NativeResult<u64> {
        let wal_tx_id = self.wal_manager.begin_transaction(isolation_level)?;

        let graph_tx = GraphTransaction {
            wal_tx_id,
            isolation_level,
            modified_nodes: Vec::new(),
            modified_clusters: Vec::new(),
            start_time: std::time::Instant::now(),
        };

        {
            let mut transactions = self.graph_transactions.write();
            transactions.insert(wal_tx_id, graph_tx);
        }

        Ok(wal_tx_id)
    }

    /// Insert a node with WAL integration
    pub fn insert_node(
        &self,
        tx_id: Option<u64>,
        node_id: i64,
        node_record: &NodeRecordV2,
    ) -> NativeResult<GraphOperationResult> {
        let start_time = std::time::Instant::now();

        // Serialize node record
        let node_data = node_record.to_bytes()?;

        // Create WAL record
        let wal_record = V2WALRecord::NodeInsert {
            node_id,
            slot_offset: node_record.outgoing_cluster_offset, // Use existing field
            node_data,
        };

        let lsn = if let Some(tx_id) = tx_id {
            self.wal_manager
                .write_transaction_record(tx_id, wal_record)?
        } else {
            self.wal_manager.write_record(wal_record)?
        };

        // Track modifications if in transaction
        if let Some(tx_id) = tx_id {
            let mut transactions = self.graph_transactions.write();
            if let Some(graph_tx) = transactions.get_mut(&tx_id) {
                graph_tx.modified_nodes.push(node_id);
            }
        }

        let duration = start_time.elapsed();
        let result = GraphOperationResult {
            success: true,
            lsn: Some(lsn),
            tx_id,
            metrics: OperationMetrics {
                duration_us: duration.as_micros() as u64,
                wal_records_written: 1,
                bytes_written: node_record.serialized_size() as u64,
                nodes_affected: 1,
                clusters_affected: 0,
                edges_affected: 0,
            },
        };

        // Auto-checkpoint if configured
        if self.config.auto_checkpoint && self.wal_manager.requires_checkpoint() {
            let _ = self.wal_manager.force_checkpoint();
        }

        Ok(result)
    }

    /// Update a node with WAL integration
    pub fn update_node(
        &self,
        tx_id: Option<u64>,
        node_id: i64,
        old_record: &NodeRecordV2,
        new_record: &NodeRecordV2,
    ) -> NativeResult<GraphOperationResult> {
        let start_time = std::time::Instant::now();

        // Serialize node records
        let old_data = old_record.to_bytes()?;
        let new_data = new_record.to_bytes()?;

        // Create WAL record
        let wal_record = V2WALRecord::NodeUpdate {
            node_id,
            slot_offset: new_record.outgoing_cluster_offset, // Use existing field
            old_data,
            new_data,
        };

        let lsn = if let Some(tx_id) = tx_id {
            self.wal_manager
                .write_transaction_record(tx_id, wal_record)?
        } else {
            self.wal_manager.write_record(wal_record)?
        };

        // Track modifications if in transaction
        if let Some(tx_id) = tx_id {
            let mut transactions = self.graph_transactions.write();
            if let Some(graph_tx) = transactions.get_mut(&tx_id) {
                graph_tx.modified_nodes.push(node_id);
            }
        }

        let duration = start_time.elapsed();
        let result = GraphOperationResult {
            success: true,
            lsn: Some(lsn),
            tx_id,
            metrics: OperationMetrics {
                duration_us: duration.as_micros() as u64,
                wal_records_written: 1,
                bytes_written: (old_record.serialized_size() + new_record.serialized_size()) as u64,
                nodes_affected: 1,
                clusters_affected: 0,
                edges_affected: 0,
            },
        };

        Ok(result)
    }

    /// Create an edge cluster with WAL integration
    pub fn create_cluster(
        &self,
        tx_id: Option<u64>,
        node_id: i64,
        direction: Direction,
        cluster_offset: u64,
        cluster_size: u32,
        cluster_data: &[u8],
    ) -> NativeResult<GraphOperationResult> {
        let start_time = std::time::Instant::now();

        // Create WAL record
        let wal_record = V2WALRecord::ClusterCreate {
            node_id,
            direction,
            cluster_offset,
            cluster_size,
            edge_data: cluster_data.to_vec(),
        };

        let lsn = if let Some(tx_id) = tx_id {
            self.wal_manager
                .write_transaction_record(tx_id, wal_record)?
        } else {
            self.wal_manager.write_record(wal_record)?
        };

        // Track modifications if in transaction
        if let Some(tx_id) = tx_id {
            let mut transactions = self.graph_transactions.write();
            if let Some(graph_tx) = transactions.get_mut(&tx_id) {
                graph_tx.modified_clusters.push((node_id, direction));
            }
        }

        let duration = start_time.elapsed();
        let result = GraphOperationResult {
            success: true,
            lsn: Some(lsn),
            tx_id,
            metrics: OperationMetrics {
                duration_us: duration.as_micros() as u64,
                wal_records_written: 1,
                bytes_written: cluster_data.len() as u64,
                nodes_affected: 0,
                clusters_affected: 1,
                edges_affected: 0,
            },
        };

        Ok(result)
    }

    /// Insert an edge into a cluster with WAL integration
    pub fn insert_edge(
        &self,
        tx_id: Option<u64>,
        cluster_key: (i64, Direction),
        edge_record: &CompactEdgeRecord,
        insertion_point: u32,
    ) -> NativeResult<GraphOperationResult> {
        let start_time = std::time::Instant::now();

        // Create WAL record
        let wal_record = V2WALRecord::EdgeInsert {
            cluster_key,
            edge_record: edge_record.clone(),
            insertion_point,
        };

        let lsn = if let Some(tx_id) = tx_id {
            self.wal_manager
                .write_transaction_record(tx_id, wal_record)?
        } else {
            self.wal_manager.write_record(wal_record)?
        };

        // Track modifications if in transaction
        if let Some(tx_id) = tx_id {
            let mut transactions = self.graph_transactions.write();
            if let Some(graph_tx) = transactions.get_mut(&tx_id) {
                graph_tx.modified_clusters.push(cluster_key);
            }
        }

        let duration = start_time.elapsed();
        let result = GraphOperationResult {
            success: true,
            lsn: Some(lsn),
            tx_id,
            metrics: OperationMetrics {
                duration_us: duration.as_micros() as u64,
                wal_records_written: 1,
                bytes_written: edge_record.serialized_size() as u64,
                nodes_affected: 0,
                clusters_affected: 1,
                edges_affected: 1,
            },
        };

        Ok(result)
    }

    /// Commit a graph transaction
    pub fn commit_transaction(&self, tx_id: u64) -> NativeResult<GraphOperationResult> {
        let start_time = std::time::Instant::now();

        // Get transaction details
        let graph_tx = {
            let mut transactions = self.graph_transactions.write();
            transactions.remove(&tx_id)
        };

        let graph_tx = graph_tx.ok_or_else(|| NativeBackendError::InvalidTransaction {
            tx_id,
            reason: "Transaction not found".to_string(),
        })?;

        // Commit WAL transaction
        self.wal_manager.commit_transaction(tx_id)?;

        let duration = start_time.elapsed();
        let result = GraphOperationResult {
            success: true,
            lsn: None, // LSN will be assigned during WAL commit
            tx_id: Some(tx_id),
            metrics: OperationMetrics {
                duration_us: duration.as_micros() as u64,
                wal_records_written: graph_tx.modified_nodes.len() as u64
                    + graph_tx.modified_clusters.len() as u64,
                bytes_written: 0, // Calculated by WAL manager
                nodes_affected: graph_tx.modified_nodes.len() as u32,
                clusters_affected: graph_tx.modified_clusters.len() as u32,
                edges_affected: 0, // Not tracked separately
            },
        };

        // Auto-checkpoint if configured
        if self.config.auto_checkpoint && self.wal_manager.requires_checkpoint() {
            let _ = self.wal_manager.force_checkpoint();
        }

        Ok(result)
    }

    /// Rollback a graph transaction
    pub fn rollback_transaction(&self, tx_id: u64) -> NativeResult<GraphOperationResult> {
        let start_time = std::time::Instant::now();

        // Get transaction details
        let graph_tx = {
            let mut transactions = self.graph_transactions.write();
            transactions.remove(&tx_id)
        };

        let graph_tx = graph_tx.ok_or_else(|| NativeBackendError::InvalidTransaction {
            tx_id,
            reason: "Transaction not found".to_string(),
        })?;

        // Rollback WAL transaction
        self.wal_manager.rollback_transaction(tx_id)?;

        let duration = start_time.elapsed();
        let result = GraphOperationResult {
            success: true,
            lsn: None,
            tx_id: Some(tx_id),
            metrics: OperationMetrics {
                duration_us: duration.as_micros() as u64,
                wal_records_written: 1, // Just the rollback record
                bytes_written: 0,
                nodes_affected: graph_tx.modified_nodes.len() as u32,
                clusters_affected: graph_tx.modified_clusters.len() as u32,
                edges_affected: 0,
            },
        };

        Ok(result)
    }

    /// Force a checkpoint operation
    pub fn force_checkpoint(&self) -> NativeResult<()> {
        self.wal_manager.force_checkpoint()
    }

    /// Get WAL manager metrics
    pub fn get_metrics(&self) -> crate::backend::native::v2::wal::WALManagerMetrics {
        self.wal_manager.get_metrics()
    }

    /// Get active transaction count
    pub fn get_active_transaction_count(&self) -> usize {
        self.wal_manager.get_active_transaction_count()
    }

    /// Shutdown the integrator gracefully
    pub fn shutdown(self) -> NativeResult<()> {
        // Force checkpoint of any remaining transactions
        let _ = self.force_checkpoint();

        // Shutdown WAL manager
        Arc::try_unwrap(self.wal_manager)
            .map_err(|_| NativeBackendError::InvalidState {
                context: "Cannot shutdown WAL manager - still in use".to_string(),
                source: None,
            })?
            .shutdown()
    }
}

/// Extension trait for NodeRecordV2 to enable WAL integration
pub trait NodeRecordV2WALExt {
    /// Convert node record to bytes for WAL storage
    fn to_bytes(&self) -> NativeResult<Vec<u8>>;

    /// Get serialized size
    fn serialized_size(&self) -> usize;
}

impl NodeRecordV2WALExt for NodeRecordV2 {
    fn to_bytes(&self) -> NativeResult<Vec<u8>> {
        // Use the existing V2 serialization implementation
        Ok(self.serialize())
    }

    fn serialized_size(&self) -> usize {
        // Use the actual serialized size calculation
        self.size_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_graph_wal_integrator_create() {
        let temp_dir = tempdir().unwrap();
        let wal_config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        // Create a minimal V2 graph file for the checkpoint manager
        let v2_graph_path = temp_dir.path().join("test.v2");
        let _graph_file = crate::backend::native::GraphFile::create(&v2_graph_path)
            .expect("Failed to create V2 graph file for test");

        let integration_config = GraphWALIntegrationConfig::default();
        let integrator = V2GraphWALIntegrator::create(wal_config, integration_config);
        assert!(integrator.is_ok());
    }

    #[test]
    fn test_transaction_lifecycle() {
        let temp_dir = tempdir().unwrap();
        let wal_config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        // Create a minimal V2 graph file for the checkpoint manager
        let v2_graph_path = temp_dir.path().join("test.v2");
        let _graph_file = crate::backend::native::GraphFile::create(&v2_graph_path)
            .expect("Failed to create V2 graph file for test");

        let integration_config = GraphWALIntegrationConfig::default();
        let integrator = V2GraphWALIntegrator::create(wal_config, integration_config).unwrap();

        // Begin transaction
        let tx_id = integrator
            .begin_transaction(TransactionIsolation::ReadCommitted)
            .unwrap();
        assert!(tx_id > 0);
        assert_eq!(integrator.get_active_transaction_count(), 1);

        // Commit transaction
        let result = integrator.commit_transaction(tx_id).unwrap();
        assert!(result.success);
        assert_eq!(result.tx_id, Some(tx_id));
        assert_eq!(integrator.get_active_transaction_count(), 0);
    }

    #[test]
    fn test_node_insertion() {
        let temp_dir = tempdir().unwrap();
        let wal_config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        // Create a minimal V2 graph file for the checkpoint manager
        let v2_graph_path = temp_dir.path().join("test.v2");
        let _graph_file = crate::backend::native::GraphFile::create(&v2_graph_path)
            .expect("Failed to create V2 graph file for test");

        let integration_config = GraphWALIntegrationConfig::default();
        let integrator = V2GraphWALIntegrator::create(wal_config, integration_config).unwrap();

        // Create a dummy node record
        let node_record = NodeRecordV2::new(
            42,
            "test".to_string(),
            "test_node".to_string(),
            serde_json::Value::Null,
        );

        // Insert node outside transaction
        let result = integrator.insert_node(None, 42, &node_record).unwrap();
        assert!(result.success);
        assert!(result.lsn.is_some());
        assert!(result.tx_id.is_none());
    }

    #[test]
    fn test_transaction_rollback() {
        let temp_dir = tempdir().unwrap();
        let wal_config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            ..Default::default()
        };

        // Create a minimal V2 graph file for the checkpoint manager
        let v2_graph_path = temp_dir.path().join("test.v2");
        let _graph_file = crate::backend::native::GraphFile::create(&v2_graph_path)
            .expect("Failed to create V2 graph file for test");

        let integration_config = GraphWALIntegrationConfig::default();
        let integrator = V2GraphWALIntegrator::create(wal_config, integration_config).unwrap();

        // Begin transaction
        let tx_id = integrator
            .begin_transaction(TransactionIsolation::Serializable)
            .unwrap();
        assert_eq!(integrator.get_active_transaction_count(), 1);

        // Rollback transaction
        let result = integrator.rollback_transaction(tx_id).unwrap();
        assert!(result.success);
        assert_eq!(result.tx_id, Some(tx_id));
        assert_eq!(integrator.get_active_transaction_count(), 0);
    }
}
