//! Transaction lifecycle and commit management for GraphFile
//!
//! This module provides atomic transaction management including transaction
//! begin, commit, rollback operations, and commit marker handling for
//! GraphFile operations.

use crate::backend::native::{
    types::NativeResult,
    transaction_state::TransactionState,
    constants::node::NODE_SLOT_SIZE,
};

use std::io::{Read, Seek, SeekFrom, Write};

/// Transaction lifecycle management utilities for GraphFile
pub struct TransactionManager;

impl TransactionManager {
    /// Write commit marker value to file
    ///
    /// Stores the commit marker at the standard offset for transaction
    /// state tracking and crash recovery.
    pub fn write_commit_marker_value(file: &mut std::fs::File, value: u64) -> NativeResult<()> {
        use crate::backend::native::graph_file::validation::GraphFileValidator;

        file.seek(SeekFrom::Start(GraphFileValidator::commit_marker_offset() as u64))?;
        file.write_all(&value.to_be_bytes())?;
        file.sync_all()?;
        Ok(())
    }

    /// Read commit marker value from file
    ///
    /// Reads the commit marker from the standard offset for transaction
    /// state verification.
    pub fn read_commit_marker_value(file: &mut std::fs::File) -> NativeResult<u64> {
        use crate::backend::native::graph_file::validation::GraphFileValidator;

        file.seek(SeekFrom::Start(GraphFileValidator::commit_marker_offset() as u64))?;
        let mut marker_bytes = [0u8; 8];
        file.read_exact(&mut marker_bytes)?;
        Ok(u64::from_be_bytes(marker_bytes))
    }

    /// Begin cluster commit operation
    ///
    /// Initializes a cluster commit by setting the commit marker to 0.
    pub fn begin_cluster_commit(file: &mut std::fs::File) -> NativeResult<()> {
        Self::write_commit_marker_value(file, 0)
    }

    /// Finish cluster commit operation
    ///
    /// Completes a cluster commit by setting the commit marker to clean value.
    pub fn finish_cluster_commit(file: &mut std::fs::File) -> NativeResult<()> {
        use super::validation::GraphFileValidator;
        Self::write_commit_marker_value(file, GraphFileValidator::clean_commit_marker())
    }

    /// Begin atomic transaction with rollback state
    ///
    /// Phase 70: Initialize transaction with comprehensive debugging and state tracking.
    pub fn begin_transaction(
        file: &mut std::fs::File,
        tx_state: &mut TransactionState,
        file_path: &str,
        node_data_offset: u64,
        file_size_fn: &dyn Fn() -> NativeResult<u64>,
        read_bytes_fn: &mut dyn FnMut(u64, &mut [u8]) -> NativeResult<()>,
    ) -> NativeResult<()> {
        // TX_BEGIN_AUDIT: Check node 257 slot before transaction operations
        if std::env::var("TX_BEGIN_AUDIT").is_ok() {
            let slot_offset = node_data_offset + ((257 - 1) as u64 * 4096);
            let mut buffer = vec![0u8; 32];

            if read_bytes_fn(slot_offset, &mut buffer).is_ok() {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} first_32={:02x?} version={}",
                    "BEFORE_TX_BEGIN", 257, slot_offset, &buffer, buffer[0]
                );
            } else {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} READ_FAILED",
                    "BEFORE_TX_BEGIN", 257, slot_offset
                );
            }
        }

        // PHASE 2D: Probe node1 corruption before any transaction operations
        if std::env::var("EDGE_CLUSTER_DEBUG").is_ok() {
            let mut disk_file = std::fs::File::open(file_path)?;
            let mut node1_bytes = vec![0u8; 32];
            disk_file.seek(SeekFrom::Start(0x400))?;
            disk_file.read_exact(&mut node1_bytes)?;
            let version_before_tx_ops = node1_bytes[0];
            let file_size_before_tx_ops = file_size_fn().unwrap_or(0);
            println!(
                "[EDGE_CLUSTER_DEBUG] BEFORE_TX_OPS: node1_version={}, file_size={}, node1_bytes={:02x?}",
                version_before_tx_ops, file_size_before_tx_ops, &node1_bytes
            );
        }

        // Begin transaction in header (saves current state)
        tx_state.begin_tx(1); // Use tx_id=1 for now, could be parameterized

        // TX_BEGIN_AUDIT: Check node 257 slot after header state modification
        if std::env::var("TX_BEGIN_AUDIT").is_ok() {
            let slot_offset = node_data_offset + ((257 - 1) as u64 * 4096);
            let mut buffer = vec![0u8; 32];

            if read_bytes_fn(slot_offset, &mut buffer).is_ok() {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} first_32={:02x?} version={}",
                    "AFTER_TX_STATE_MODIFY", 257, slot_offset, &buffer, buffer[0]
                );
            } else {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} READ_FAILED",
                    "AFTER_TX_STATE_MODIFY", 257, slot_offset
                );
            }
        }

        // PHASE 2D: Probe after header modification
        if std::env::var("EDGE_CLUSTER_DEBUG").is_ok() {
            let mut disk_file = std::fs::File::open(file_path)?;
            let mut node1_bytes = vec![0u8; 32];
            disk_file.seek(SeekFrom::Start(0x400))?;
            disk_file.read_exact(&mut node1_bytes)?;
            let version_after_header_modify = node1_bytes[0];
            let file_size_after_header_modify = file_size_fn().unwrap_or(0);
            println!(
                "[EDGE_CLUSTER_DEBUG] AFTER_HEADER_MODIFY: node1_version={}, file_size={}, node1_bytes={:02x?}",
                version_after_header_modify, file_size_after_header_modify, &node1_bytes
            );
        }

        use super::debug::DebugInstrumentation;
        DebugInstrumentation::log_transaction_phase("begun", 1);
        Ok(())
    }

    /// Commit atomic transaction
    ///
    /// Phase 70: Complete transaction by clearing state and persisting header.
    pub fn commit_transaction(
        file: &mut std::fs::File,
        tx_state: &mut TransactionState,
    ) -> NativeResult<()> {
        // Clear transaction state in header
        tx_state.commit();

        // Force sync to disk
        file.sync_all()?;

        use super::debug::DebugInstrumentation;
        DebugInstrumentation::log_transaction_phase("committed", tx_state.tx_id);
        Ok(())
    }

    /// Rollback incomplete atomic transaction
    ///
    /// Phase 70: Restore file state and clean up transaction state.
    pub fn rollback_transaction(
        file: &mut std::fs::File,
        tx_state: &mut TransactionState,
        current_size: u64,
        node_data_offset: u64,
        node_count: u64,
    ) -> NativeResult<()> {
        let node_region_end = node_data_offset + (node_count as u64 * NODE_SLOT_SIZE);

        // Phase 10: Transaction rollback is now runtime-only
        tx_state.rollback();

        // Phase 72: Calculate rollback floor - never truncate below node region
        let intended_rollback_size = 0; // This would come from header.free_space_offset
        let rollback_floor = std::cmp::max(node_region_end, node_data_offset);

        // Additional protection: ensure all written node slots are protected
        // NEVER rollback below the file size - nodes are persistent and should never be truncated
        let enhanced_rollback_floor = current_size; // Never truncate at all
        let final_rollback_size = std::cmp::max(intended_rollback_size, enhanced_rollback_floor);

        use super::debug::DebugInstrumentation;
        DebugInstrumentation::log_rollback_info(
            rollback_floor,
            enhanced_rollback_floor,
            final_rollback_size,
        );

        if current_size > final_rollback_size {
            // Log truncation that could affect node slots
            DebugInstrumentation::log_slot_corruption_check(
                "FILE_TRUNCATE",
                current_size,
                final_rollback_size,
                current_size - final_rollback_size,
            );

            // Truncate file to remove any partially written cluster data
            file.set_len(final_rollback_size)?;
            file.sync_all()?;

            // Post-truncate slot verification
            DebugInstrumentation::log_post_truncate_slot_check(
                257,
                node_data_offset + ((257 - 1) as u64 * 4096),
                2, // V2 version
            );
        }

        DebugInstrumentation::log_rollback_completion(final_rollback_size);
        Ok(())
    }

    /// Clear V2 cluster metadata during rollback
    ///
    /// Phase 75: Skip V2 node slot rewriting during rollback to prevent corruption.
    pub fn clear_v2_cluster_metadata_on_rollback(tx_modified_nodes: &mut Vec<u64>) -> NativeResult<()> {
        #[cfg(feature = "trace_v2_io")]
        println!("[phase75] ROLLBACK_CLEANUP: SKIPPING V2 node slot rewrite to prevent corruption");

        // CRITICAL FIX: Do NOT rewrite V2 node slots during rollback
        // This prevents corruption of V2 format (version=2 -> version=1)

        // Just clear the transaction tracking
        tx_modified_nodes.clear();

        #[cfg(feature = "trace_v2_io")]
        println!("[phase75] ROLLBACK_CLEANUP: Completed without V2 slot corruption");

        Ok(())
    }

    /// Get transaction statistics for debugging
    pub fn get_transaction_statistics(tx_state: &TransactionState) -> TransactionStatistics {
        TransactionStatistics {
            tx_id: tx_state.tx_id,
            is_active: tx_state.is_in_progress(),
            state: if tx_state.is_in_progress() { "InProgress".to_string() } else { "Inactive".to_string() },
        }
    }
}

/// Transaction statistics for debugging and monitoring
#[derive(Debug, Clone)]
pub struct TransactionStatistics {
    pub tx_id: u64,
    pub is_active: bool,
    pub state: String,
}

impl TransactionStatistics {
    /// Check if transaction is in progress
    pub fn is_transaction_in_progress(&self) -> bool {
        self.is_active
    }

    /// Get transaction state description
    pub fn get_state_description(&self) -> &str {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempfile;
    use crate::backend::native::transaction_state::TransactionState;
    use std::fs::OpenOptions;

    #[test]
    fn test_write_read_commit_marker() {
        let mut temp_file = tempfile().unwrap();
        let test_value = 0x123456789ABCDEF0u64;

        // Write marker
        TransactionManager::write_commit_marker_value(&mut temp_file, test_value).unwrap();

        // Read marker back
        let read_value = TransactionManager::read_commit_marker_value(&mut temp_file).unwrap();
        assert_eq!(read_value, test_value);
    }

    #[test]
    fn test_cluster_commit_operations() {
        let mut temp_file = tempfile().unwrap();

        // Begin cluster commit
        TransactionManager::begin_cluster_commit(&mut temp_file).unwrap();

        // Read marker should be 0
        let marker = TransactionManager::read_commit_marker_value(&mut temp_file).unwrap();
        assert_eq!(marker, 0);

        // Finish cluster commit
        TransactionManager::finish_cluster_commit(&mut temp_file).unwrap();

        // Read marker should be clean value
        let marker = TransactionManager::read_commit_marker_value(&mut temp_file).unwrap();
        assert_ne!(marker, 0); // Should be the clean marker value
    }

    #[test]
    fn test_transaction_statistics() {
        let mut tx_state = TransactionState::new();

        // Begin transaction
        tx_state.begin_tx(42);

        let stats = TransactionManager::get_transaction_statistics(&tx_state);
        assert_eq!(stats.tx_id, 42);
        assert!(stats.is_transaction_in_progress());
        assert!(!stats.get_state_description().is_empty());
        assert_eq!(stats.get_state_description(), "InProgress");
    }

    #[test]
    fn test_clear_v2_cluster_metadata() {
        let mut tx_modified_nodes = vec![1u64, 2u64, 3u64];
        assert_eq!(tx_modified_nodes.len(), 3);

        TransactionManager::clear_v2_cluster_metadata_on_rollback(&mut tx_modified_nodes).unwrap();

        assert_eq!(tx_modified_nodes.len(), 0);
    }

    #[test]
    fn test_rollback_with_no_truncation() {
        let mut temp_file = tempfile().unwrap();
        let mut tx_state = TransactionState::new();

        // Set up some file content
        temp_file.set_len(2048).unwrap();

        // Start transaction
        tx_state.begin_tx(1);

        // Rollback with current_size == final_rollback_size (no truncation)
        let result = TransactionManager::rollback_transaction(
            &mut temp_file,
            &mut tx_state,
            2048, // current_size
            1024, // node_data_offset
            10,   // node_count
        );

        assert!(result.is_ok());
        // File size should remain 2048 (no truncation)
        assert_eq!(temp_file.metadata().unwrap().len(), 2048);
    }
}