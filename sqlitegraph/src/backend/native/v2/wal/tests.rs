//! Comprehensive tests for enhanced V2 WAL system integration.
//!
//! This test module validates the complete V2 WAL system including:
//! - Enhanced transaction coordination
//! - Group commit optimization
//! - Cluster-affinity logging
//! - Performance optimizations
//! - V2 graph file integration

use crate::backend::native::{NativeResult, NativeNodeId};
use crate::backend::native::v2::wal::{
    V2WALConfig, V2WALManager, TransactionIsolation, WALManagerMetrics,
    V2GraphWALIntegrator, GraphWALIntegrationConfig, GraphOperationResult,
    PerformanceConfig, CompressionAlgorithm, WALRecordCompressor,
    IOBatcher, ClusterAffinityOptimizer,
};
use crate::backend::native::v2::{NodeRecordV2, CompactEdgeRecord, Direction};
use tempfile::tempdir;
use std::time::Duration;

#[cfg(test)]
mod enhanced_wal_tests {
    use super::*;

    /// Test enhanced WAL manager creation and basic operations
    #[test]
    fn test_enhanced_wal_manager_creation() -> NativeResult<()> {
        let temp_dir = tempdir()?;
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("test.wal"),
            checkpoint_path: temp_dir.path().join("test.checkpoint"),
            max_wal_size: 1024 * 1024, // 1MB
            buffer_size: 64 * 1024,     // 64KB
            checkpoint_interval: 100,
            group_commit_timeout_ms: 5,
            max_group_commit_size: 10,
            enable_compression: false,
            compression_level: 3,
        };

        let manager = V2WALManager::create(config)?;
        assert_eq!(manager.get_active_transaction_count(), 0);

        // Test metrics
        let metrics = manager.get_metrics();
        assert_eq!(metrics.total_transactions, 0);
        assert_eq!(metrics.committed_transactions, 0);
        assert_eq!(metrics.rolled_back_transactions, 0);

        // Test checkpoint requirement check
        assert!(!manager.requires_checkpoint());

        Ok(())
    }

    /// Test transaction lifecycle with enhanced manager
    #[test]
    fn test_enhanced_transaction_lifecycle() -> NativeResult<()> {
        let temp_dir = tempdir()?;
        let config = V2WALConfig::for_graph_file(temp_dir.path().join("test.graph"));
        let manager = V2WALManager::create(config)?;

        // Test different isolation levels
        let tx_read_committed = manager.begin_transaction(TransactionIsolation::ReadCommitted)?;
        let tx_serializable = manager.begin_transaction(TransactionIsolation::Serializable)?;
        let tx_snapshot = manager.begin_transaction(TransactionIsolation::Snapshot)?;

        assert!(tx_read_committed > 0);
        assert!(tx_serializable > tx_read_committed);
        assert!(tx_snapshot > tx_serializable);
        assert_eq!(manager.get_active_transaction_count(), 3);

        // Commit first transaction
        manager.commit_transaction(tx_read_committed)?;
        assert_eq!(manager.get_active_transaction_count(), 2);

        // Rollback second transaction
        manager.rollback_transaction(tx_serializable)?;
        assert_eq!(manager.get_active_transaction_count(), 1);

        // Commit third transaction
        manager.commit_transaction(tx_snapshot)?;
        assert_eq!(manager.get_active_transaction_count(), 0);

        // Verify metrics
        let metrics = manager.get_metrics();
        assert_eq!(metrics.total_transactions, 3);
        assert_eq!(metrics.committed_transactions, 2);
        assert_eq!(metrics.rolled_back_transactions, 1);

        // Graceful shutdown
        manager.shutdown()?;
        Ok(())
    }

    /// Test group commit functionality
    #[test]
    fn test_group_commit_optimization() -> NativeResult<()> {
        let temp_dir = tempdir()?;
        let mut config = V2WALConfig::for_graph_file(temp_dir.path().join("test.graph"));
        config.group_commit_timeout_ms = 1; // Very short timeout for testing
        config.max_group_commit_size = 3;

        let manager = V2WALManager::create(config)?;

        // Start multiple transactions rapidly
        let tx_ids: Vec<u64> = (0..5)
            .map(|_| manager.begin_transaction(TransactionIsolation::ReadCommitted).unwrap())
            .collect();

        // All transactions should be active initially
        assert_eq!(manager.get_active_transaction_count(), 5);

        // Commit all transactions
        for tx_id in tx_ids {
            manager.commit_transaction(tx_id)?;
        }

        // Should be no active transactions
        assert_eq!(manager.get_active_transaction_count(), 0);

        // Check group commit metrics
        let metrics = manager.get_metrics();
        assert_eq!(metrics.committed_transactions, 5);
        // Group commit should have occurred due to short timeout

        manager.shutdown()?;
        Ok(())
    }

    /// Test V2 graph WAL integration
    #[test]
    fn test_v2_graph_wal_integration() -> NativeResult<()> {
        let temp_dir = tempdir()?;
        let wal_config = V2WALConfig::for_graph_file(temp_dir.path().join("test.graph"));
        let integration_config = GraphWALIntegrationConfig {
            auto_checkpoint: true,
            checkpoint_interval: 10,
            cluster_affinity: true,
            enable_compression: false,
            max_batch_size: 5,
            sync_writes: true,
        };

        let integrator = V2GraphWALIntegrator::create(wal_config, integration_config)?;

        // Test transaction-based node insertion
        let tx_id = integrator.begin_transaction(TransactionIsolation::ReadCommitted)?;
        assert_eq!(integrator.get_active_transaction_count(), 1);

        // Create a dummy node record (simplified for testing)
        let node_record = create_test_node_record(42);

        // Insert node within transaction
        let result = integrator.insert_node(Some(tx_id), 42, &node_record)?;
        assert!(result.success);
        assert!(result.lsn.is_some());
        assert_eq!(result.tx_id, Some(tx_id));

        // Commit transaction
        let commit_result = integrator.commit_transaction(tx_id)?;
        assert!(commit_result.success);
        assert_eq!(integrator.get_active_transaction_count(), 0);

        // Test non-transactional operation
        let node_record2 = create_test_node_record(43);
        let direct_result = integrator.insert_node(None, 43, &node_record2)?;
        assert!(direct_result.success);
        assert!(direct_result.lsn.is_some());
        assert!(direct_result.tx_id.is_none());

        integrator.shutdown()?;
        Ok(())
    }

    /// Test compression performance optimization
    #[test]
    fn test_wal_compression() -> NativeResult<()> {
        let test_data = vec![1, 1, 1, 1, 2, 2, 2, 3, 3, 3, 3, 3];

        // Test RLE compression (good for repetitive data)
        let mut compressor = WALRecordCompressor::new(CompressionAlgorithm::RLE, 1)?;
        let compressed = compressor.compress(&test_data)?;
        let decompressed = compressor.decompress(&compressed)?;

        assert_eq!(decompressed, test_data);

        // Check statistics
        let stats = compressor.get_stats();
        assert!(stats.total_records > 0);
        assert!(stats.total_input_bytes > 0);
        assert!(stats.total_output_bytes > 0);

        // Test different algorithms
        for algorithm in [
            CompressionAlgorithm::None,
            CompressionAlgorithm::LZ4,
            CompressionAlgorithm::Zstd,
            CompressionAlgorithm::Snappy,
            CompressionAlgorithm::RLE,
        ] {
            if algorithm.validate_level(algorithm.default_level()) {
                let mut comp = WALRecordCompressor::new(algorithm, algorithm.default_level())?;
                let _ = comp.compress(&test_data)?;
            }
        }

        Ok(())
    }

    /// Test I/O batching optimization
    #[test]
    fn test_io_batching() -> NativeResult<()> {
        let mut batcher = IOBatcher::new(3, Duration::from_millis(10));

        // Add records below batch size
        let result1 = batcher.add_to_batch(vec![1, 2, 3]);
        assert!(result1.is_none()); // No flush yet

        let result2 = batcher.add_to_batch(vec![4, 5, 6]);
        assert!(result2.is_none()); // No flush yet

        // Add record that triggers batch flush
        let result3 = batcher.add_to_batch(vec![7, 8, 9]);
        assert!(result3.is_some()); // Should flush

        let batch = result3.unwrap();
        assert_eq!(batch.len(), 3);

        // Check statistics
        let stats = batcher.get_stats();
        assert_eq!(stats.total_batches, 1);
        assert_eq!(stats.total_records, 3);
        assert!(stats.avg_batch_size > 0.0);

        Ok(())
    }

    /// Test cluster affinity optimization
    #[test]
    fn test_cluster_affinity() -> NativeResult<()> {
        let mut optimizer = ClusterAffinityOptimizer::new(2);

        // Create test records with same cluster affinity
        let record1 = create_test_node_record(42);
        let record2 = create_test_node_record(42); // Same node = same cluster
        let record3 = create_test_node_record(43); // Different node = different cluster

        // Convert to WAL records (simplified)
        let wal_record1 = create_test_wal_record_node_insert(42, &record1);
        let wal_record2 = create_test_wal_record_node_insert(42, &record2);
        let wal_record3 = create_test_wal_record_node_insert(43, &record3);

        optimizer.add_record(wal_record1);
        optimizer.add_record(wal_record2);
        optimizer.add_record(wal_record3);

        // Get records for cluster 42
        let cluster_records = optimizer.get_cluster_records(42);
        assert!(cluster_records.is_some());
        assert_eq!(cluster_records.unwrap().len(), 2);

        // Get statistics
        let stats = optimizer.get_stats();
        assert!(stats.total_records >= 3);
        assert!(stats.total_groups >= 1);

        Ok(())
    }

    /// Test comprehensive WAL performance integration
    #[test]
    fn test_performance_integration() -> NativeResult<()> {
        let temp_dir = tempdir()?;
        let config = V2WALConfig {
            wal_path: temp_dir.path().join("perf_test.wal"),
            checkpoint_path: temp_dir.path().join("perf_test.checkpoint"),
            max_wal_size: 10 * 1024 * 1024, // 10MB
            buffer_size: 256 * 1024,         // 256KB
            checkpoint_interval: 1000,
            group_commit_timeout_ms: 5,
            max_group_commit_size: 20,
            enable_compression: true,
            compression_level: 3,
        };

        let manager = V2WALManager::create(config)?;

        // Perform mixed operations
        let mut tx_ids = Vec::new();

        // Start multiple transactions
        for i in 0..10 {
            let tx_id = manager.begin_transaction(TransactionIsolation::ReadCommitted)?;
            tx_ids.push(tx_id);

            // Write some records in each transaction
            let node_record = create_test_node_record((i + 1) as i64);
            let wal_record = create_test_wal_record_node_insert((i + 1) as i64, &node_record);
            manager.write_transaction_record(tx_id, wal_record)?;
        }

        // Commit half the transactions
        for (i, &tx_id) in tx_ids.iter().take(5).enumerate() {
            manager.commit_transaction(tx_id)?;
        }

        // Rollback the other half
        for &tx_id in tx_ids.iter().skip(5) {
            manager.rollback_transaction(tx_id)?;
        }

        // Check final metrics
        let metrics = manager.get_metrics();
        assert_eq!(metrics.total_transactions, 10);
        assert_eq!(metrics.committed_transactions, 5);
        assert_eq!(metrics.rolled_back_transactions, 5);

        // Force checkpoint
        manager.force_checkpoint()?;

        // Verify checkpoint count increased
        let final_metrics = manager.get_metrics();
        assert!(final_metrics.checkpoint_count > 0);

        manager.shutdown()?;
        Ok(())
    }

    /// Test WAL error handling and recovery scenarios
    #[test]
    fn test_wal_error_handling() -> NativeResult<()> {
        let temp_dir = tempdir()?;
        let config = V2WALConfig::for_graph_file(temp_dir.path().join("test.graph"));

        let manager = V2WALManager::create(config)?;

        // Test invalid transaction handling
        let result = manager.commit_transaction(999999); // Non-existent transaction
        assert!(result.is_err());

        // Test rollback of non-existent transaction
        let result = manager.rollback_transaction(999999); // Non-existent transaction
        assert!(result.is_err());

        // Test operations with no active transactions
        assert_eq!(manager.get_active_transaction_count(), 0);

        // Start and immediately rollback a transaction
        let tx_id = manager.begin_transaction(TransactionIsolation::ReadCommitted)?;
        assert_eq!(manager.get_active_transaction_count(), 1);

        manager.rollback_transaction(tx_id)?;
        assert_eq!(manager.get_active_transaction_count(), 0);

        manager.shutdown()?;
        Ok(())
    }

    // Helper functions for testing

    fn create_test_node_record(node_id: i64) -> NodeRecordV2 {
        // Create a simplified test node record
        // In real implementation, this would use NodeRecordV2 constructors
        NodeRecordV2::default() // Placeholder
    }

    fn create_test_wal_record_node_insert(node_id: i64, _node_record: &NodeRecordV2) -> crate::backend::native::v2::wal::V2WALRecord {
        use crate::backend::native::v2::wal::V2WALRecord;

        V2WALRecord::NodeInsert {
            node_id,
            slot_offset: (node_id as u64) * 1024,
            node_data: vec![1, 2, 3, 4], // Simplified test data
        }
    }
}

#[cfg(test)]
mod performance_benchmarks {
    use super::*;
    use std::time::Instant;

    /// Benchmark WAL write throughput
    #[test]
    fn test_wal_write_throughput() -> NativeResult<()> {
        let temp_dir = tempdir()?;
        let config = V2WALConfig::for_graph_file(temp_dir.path().join("bench.graph"));
        let manager = V2WALManager::create(config)?;

        let start_time = Instant::now();
        let num_operations = 1000;

        // Perform rapid node insertions
        for i in 0..num_operations {
            let tx_id = manager.begin_transaction(TransactionIsolation::ReadCommitted)?;

            let node_record = create_test_node_record((i + 1) as i64);
            let wal_record = create_test_wal_record_node_insert((i + 1) as i64, &node_record);
            manager.write_transaction_record(tx_id, wal_record)?;

            manager.commit_transaction(tx_id)?;
        }

        let duration = start_time.elapsed();
        let throughput = num_operations as f64 / duration.as_secs_f64();

        println!("WAL Write Throughput: {:.2} ops/sec", throughput);
        println!("Average Latency: {:.2} ms", duration.as_millis() as f64 / num_operations as f64);

        let metrics = manager.get_metrics();
        assert_eq!(metrics.total_transactions, num_operations as u64);
        assert_eq!(metrics.committed_transactions, num_operations as u64);

        // Performance target: should handle >100 ops/sec for this test
        assert!(throughput > 100.0, "WAL throughput too low: {:.2} ops/sec", throughput);

        manager.shutdown()?;
        Ok(())
    }

    /// Benchmark group commit performance
    #[test]
    fn test_group_commit_performance() -> NativeResult<()> {
        let temp_dir = tempdir()?;
        let mut config = V2WALConfig::for_graph_file(temp_dir.path().join("group_bench.graph"));
        config.group_commit_timeout_ms = 10; // 10ms timeout
        config.max_group_commit_size = 50;

        let manager = V2WALManager::create(config)?;

        let start_time = Instant::now();
        let num_transactions = 200;

        // Start many transactions quickly to trigger group commits
        let tx_ids: Vec<u64> = (0..num_transactions)
            .map(|_| manager.begin_transaction(TransactionIsolation::ReadCommitted).unwrap())
            .collect();

        // Commit all transactions rapidly
        for tx_id in tx_ids {
            manager.commit_transaction(tx_id)?;
        }

        let duration = start_time.elapsed();
        let throughput = num_transactions as f64 / duration.as_secs_f64();

        println!("Group Commit Throughput: {:.2} tx/sec", throughput);

        let metrics = manager.get_metrics();
        assert_eq!(metrics.committed_transactions, num_transactions as u64);

        // Group commit should improve throughput
        assert!(throughput > 50.0, "Group commit throughput too low: {:.2} tx/sec", throughput);

        manager.shutdown()?;
        Ok(())
    }

    fn create_test_node_record(node_id: i64) -> NodeRecordV2 {
        NodeRecordV2::default() // Placeholder
    }

    fn create_test_wal_record_node_insert(node_id: i64, _node_record: &NodeRecordV2) -> crate::backend::native::v2::wal::V2WALRecord {
        use crate::backend::native::v2::wal::V2WALRecord;

        V2WALRecord::NodeInsert {
            node_id,
            slot_offset: (node_id as u64) * 1024,
            node_data: vec![1, 2, 3, 4],
        }
    }
}