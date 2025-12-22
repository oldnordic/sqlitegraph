//! Graph file coordinator module
//!
//! This module provides high-level coordination and workflow management for graph file operations.
//! It handles complex transaction workflows, rollback procedures, and file management coordination.

use crate::backend::native::{
    persistent_header::PersistentHeaderV2,
    transaction_state::TransactionState,
    types::{NativeBackendError, NativeNodeId, NativeResult},
};

/// Graph file coordinator for high-level workflow management and coordination
pub struct GraphFileCoordinator<'a> {
    persistent_header: &'a mut PersistentHeaderV2,
    transaction_state: &'a mut TransactionState,
}

impl<'a> GraphFileCoordinator<'a> {
    /// Create a new graph file coordinator
    ///
    /// # Arguments
    /// * `persistent_header` - Mutable reference to persistent header
    /// * `transaction_state` - Mutable reference to transaction state
    pub fn new(
        persistent_header: &'a mut PersistentHeaderV2,
        transaction_state: &'a mut TransactionState,
    ) -> Self {
        Self {
            persistent_header,
            transaction_state,
        }
    }

    /// Begin a new transaction with the specified ID
    ///
    /// # Arguments
    /// * `tx_id` - The transaction identifier
    pub fn begin_transaction(&mut self, tx_id: u64) -> NativeResult<()> {
        // Begin transaction in state (saves current state)
        self.transaction_state.begin_tx(tx_id);
        Ok(())
    }

    /// Commit the current transaction and update the header
    ///
    /// # Arguments
    /// * `write_header_fn` - Function to write header to storage
    /// * `sync_fn` - Function to sync data to storage
    pub fn commit_transaction<WH, S>(&mut self, write_header_fn: WH, sync_fn: S) -> NativeResult<()>
    where
        WH: FnOnce() -> NativeResult<()>,
        S: FnOnce() -> NativeResult<()>,
    {
        // Mark transaction as committed in header
        self.transaction_state.commit();

        // Write header with transaction state COMMITTED
        write_header_fn()?;

        // Force final header to disk
        sync_fn()?;

        Ok(())
    }

    /// Rollback an incomplete transaction with comprehensive protection
    ///
    /// # Arguments
    /// * `current_file_size` - Current size of the file
    /// * `node_data_offset` - Offset to node data region
    /// * `node_count` - Current number of nodes
    /// * `truncate_file_fn` - Function to truncate file to specified size
    /// * `node_slot_size` - Size of each node slot
    pub fn rollback_transaction<F>(
        &mut self,
        current_file_size: u64,
        node_data_offset: u64,
        node_count: u32,
        truncate_file_fn: F,
        node_slot_size: u64,
    ) -> NativeResult<()>
    where
        F: FnOnce(u64) -> NativeResult<()>,
    {
        // Capture rollback parameters
        let node_region_end = node_data_offset + (node_count as u64 * node_slot_size);

        // Phase 10: Transaction rollback is now runtime-only
        self.transaction_state.rollback();

        // Phase 72: Calculate rollback floor - never truncate below node region
        let intended_rollback_size = self.persistent_header.free_space_offset;
        let rollback_floor = std::cmp::max(node_region_end, node_data_offset);

        // Additional protection: ensure all written node slots are protected
        // NEVER rollback below the node region to protect existing nodes
        let enhanced_rollback_floor = rollback_floor;
        let final_rollback_size = std::cmp::max(intended_rollback_size, enhanced_rollback_floor);

        self.log_rollback_calculation(
            rollback_floor,
            enhanced_rollback_floor,
            final_rollback_size,
        )?;

        // Perform file truncation if necessary
        if current_file_size > final_rollback_size {
            self.perform_safe_truncation(
                current_file_size,
                final_rollback_size,
                intended_rollback_size,
                truncate_file_fn,
            )?;

            // If we clamped the rollback_size, update free_space_offset to match actual file size
            if final_rollback_size > intended_rollback_size {
                self.persistent_header.free_space_offset = final_rollback_size;
            }
        }

        // PHASE 74 FIX: Reset cluster offsets to 0 since clusters were truncated
        self.reset_cluster_offsets();

        Ok(())
    }

    /// Reset cluster offsets after rollback to prevent invalid references
    fn reset_cluster_offsets(&mut self) {
        self.persistent_header.outgoing_cluster_offset = 0;
        self.persistent_header.incoming_cluster_offset = 0;
    }

    /// Log rollback calculation details for debugging
    fn log_rollback_calculation(
        &self,
        rollback_floor: u64,
        enhanced_rollback_floor: u64,
        final_rollback_size: u64,
    ) -> NativeResult<()> {
        println!(
            "PHASE 72: rollback_floor = {}, enhanced_rollback_floor = {}, final_rollback_size = {}",
            rollback_floor, enhanced_rollback_floor, final_rollback_size
        );

        // TRUNC_AUDIT: Log file truncation operations
        if std::env::var("TRUNC_AUDIT").is_ok() {
            println!(
                "[TRUNC_AUDIT] ROLLBACK: intended_rollback_size={}, rollback_floor={}, enhanced_rollback_floor={}, final_rollback_size={}, enhanced_protection_enabled={}",
                self.persistent_header.free_space_offset,
                rollback_floor,
                enhanced_rollback_floor,
                final_rollback_size,
                true
            );
        }

        Ok(())
    }

    /// Perform safe file truncation with comprehensive debugging
    fn perform_safe_truncation<F>(
        &self,
        current_size: u64,
        final_rollback_size: u64,
        _intended_rollback_size: u64,
        truncate_file_fn: F,
    ) -> NativeResult<()>
    where
        F: FnOnce(u64) -> NativeResult<()>,
    {
        // SLOT CORRUPTION DEBUG: Log truncation that could affect node slots
        if std::env::var("SLOT_CORRUPTION_DEBUG").is_ok() {
            println!(
                "[SLOT_CORRUPTION] FILE_TRUNCATE: current_size={}, final_rollback_size={}, difference={} bytes",
                current_size,
                final_rollback_size,
                current_size - final_rollback_size
            );
        }

        // Perform the actual truncation with audit logging
        if std::env::var("TRUNC_AUDIT").is_ok() {
            println!(
                "[TRUNC_AUDIT] BEFORE_TRUNCATE: calling set_len({})",
                final_rollback_size
            );
        }
        truncate_file_fn(final_rollback_size)?;
        if std::env::var("TRUNC_AUDIT").is_ok() {
            println!("[TRUNC_AUDIT] AFTER_TRUNCATE: set_len completed",);
        }

        Ok(())
    }

    /// Get transaction statistics
    pub fn get_transaction_statistics(&self) -> TransactionCoordinatorStatistics {
        TransactionCoordinatorStatistics {
            tx_id: self.transaction_state.tx_id,
            free_space_offset: self.persistent_header.free_space_offset,
            node_count: self.persistent_header.node_count,
            edge_count: self.persistent_header.edge_count,
        }
    }

    /// Validate transaction state consistency
    pub fn validate_transaction_state(&self) -> NativeResult<()> {
        // Check if transaction state is consistent with header
        if self.transaction_state.tx_id > 0 && self.transaction_state.is_in_progress() {
            // Transaction is active, this is expected
        } else if self.transaction_state.tx_id > 0 && !self.transaction_state.is_in_progress() {
            // Transaction is completed but still has ID, this might indicate a state issue
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "Transaction state inconsistency: tx_id={}, should be committed or rolled back",
                    self.transaction_state.tx_id
                ),
            });
        }

        Ok(())
    }

    /// Check if a transaction is currently active
    pub fn is_transaction_active(&self) -> bool {
        self.transaction_state.is_in_progress()
    }

    /// Get the current transaction ID
    pub fn current_transaction_id(&self) -> u64 {
        self.transaction_state.tx_id
    }
}

/// Statistics for the graph file coordinator
#[derive(Debug, Clone)]
pub struct TransactionCoordinatorStatistics {
    /// Current transaction ID
    pub tx_id: u64,
    /// Free space offset in file
    pub free_space_offset: u64,
    /// Number of nodes in file
    pub node_count: u64,
    /// Number of edges in file
    pub edge_count: u64,
}

/// Rollback protection configuration
#[derive(Debug, Clone)]
pub struct RollbackProtectionConfig {
    /// Enable enhanced rollback protection
    pub enable_enhanced_protection: bool,
    /// Minimum rollback floor (absolute minimum file size)
    pub minimum_rollback_size: u64,
    /// Enable node slot verification after truncation
    pub enable_slot_verification: bool,
    /// Enable truncation auditing
    pub enable_truncation_audit: bool,
}

impl Default for RollbackProtectionConfig {
    fn default() -> Self {
        Self {
            enable_enhanced_protection: true,
            minimum_rollback_size: 1024, // 1KB minimum
            enable_slot_verification: false,
            enable_truncation_audit: false,
        }
    }
}

/// Post-transaction validation options
#[derive(Debug, Clone)]
pub struct PostTransactionValidationOptions {
    /// Validate node slots after rollback
    pub validate_node_slots: bool,
    /// Node IDs to validate (range start, range end)
    pub node_validation_range: (NativeNodeId, NativeNodeId),
    /// Verify file size consistency
    pub verify_file_size: bool,
}

impl Default for PostTransactionValidationOptions {
    fn default() -> Self {
        Self {
            validate_node_slots: false,
            node_validation_range: (256, 258),
            verify_file_size: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::{
        persistent_header::PersistentHeaderV2, transaction_state::TransactionState,
    };

    #[test]
    fn test_coordinator_creation() {
        let mut header = PersistentHeaderV2::new_v2();
        let mut tx_state = TransactionState::new();
        let coordinator = GraphFileCoordinator::new(&mut header, &mut tx_state);

        assert_eq!(coordinator.current_transaction_id(), 0);
        assert!(!coordinator.is_transaction_active());
    }

    #[test]
    fn test_begin_transaction() {
        let mut header = PersistentHeaderV2::new_v2();
        let mut tx_state = TransactionState::new();
        let mut coordinator = GraphFileCoordinator::new(&mut header, &mut tx_state);

        coordinator.begin_transaction(123).unwrap();

        assert_eq!(coordinator.current_transaction_id(), 123);
        assert!(coordinator.is_transaction_active());
    }

    #[test]
    fn test_commit_transaction() {
        let mut header = PersistentHeaderV2::new_v2();
        let mut tx_state = TransactionState::new();
        let mut coordinator = GraphFileCoordinator::new(&mut header, &mut tx_state);

        coordinator.begin_transaction(456).unwrap();

        let header_written = false;
        let synced = false;
        let mut write_header_calls = 0;
        let mut sync_calls = 0;

        coordinator
            .commit_transaction(
                || {
                    write_header_calls += 1;
                    Ok(())
                },
                || {
                    sync_calls += 1;
                    Ok(())
                },
            )
            .unwrap();

        assert_eq!(write_header_calls, 1);
        assert_eq!(sync_calls, 1);
        assert_eq!(coordinator.current_transaction_id(), 0); // Commit resets tx_id to 0
        assert!(!coordinator.is_transaction_active()); // Transaction is committed (inactive)
    }

    #[test]
    fn test_rollback_transaction_no_truncation() {
        let mut header = PersistentHeaderV2::new_v2();
        header.free_space_offset = 5000;
        let mut tx_state = TransactionState::new();
        let mut coordinator = GraphFileCoordinator::new(&mut header, &mut tx_state);

        let current_size = 4000;
        let node_data_offset = 1024;
        let node_count = 2;
        let node_slot_size = 4096;

        let mut truncate_calls = 0;
        let mut last_truncate_size = 0;

        coordinator
            .rollback_transaction(
                current_size,
                node_data_offset,
                node_count,
                |size| {
                    truncate_calls += 1;
                    last_truncate_size = size;
                    Ok(())
                },
                node_slot_size,
            )
            .unwrap();

        // Should not truncate since current_size < final_rollback_size
        assert_eq!(truncate_calls, 0);
        assert_eq!(last_truncate_size, 0);
        assert_eq!(coordinator.persistent_header.outgoing_cluster_offset, 0);
        assert_eq!(coordinator.persistent_header.incoming_cluster_offset, 0);
    }

    #[test]
    fn test_rollback_transaction_with_truncation() {
        let mut header = PersistentHeaderV2::new_v2();
        header.free_space_offset = 3000;
        let mut tx_state = TransactionState::new();
        let mut coordinator = GraphFileCoordinator::new(&mut header, &mut tx_state);

        let current_size = 6000;
        let node_data_offset = 1024;
        let node_count = 1;
        let node_slot_size = 4096;

        let mut truncate_calls = 0;
        let mut last_truncate_size = 0;

        coordinator
            .rollback_transaction(
                current_size,
                node_data_offset,
                node_count,
                |size| {
                    truncate_calls += 1;
                    last_truncate_size = size;
                    Ok(())
                },
                node_slot_size,
            )
            .unwrap();

        // Should truncate since current_size > final_rollback_size
        assert_eq!(truncate_calls, 1);
        assert_eq!(last_truncate_size, 5120); // Truncate to rollback_floor (node region protection)
        assert_eq!(coordinator.persistent_header.free_space_offset, 5120);
        assert_eq!(coordinator.persistent_header.outgoing_cluster_offset, 0);
        assert_eq!(coordinator.persistent_header.incoming_cluster_offset, 0);
    }

    #[test]
    fn test_transaction_statistics() {
        let mut header = PersistentHeaderV2::new_v2();
        header.node_count = 100;
        header.edge_count = 200;
        header.free_space_offset = 5000;
        let mut tx_state = TransactionState::new();
        let mut coordinator = GraphFileCoordinator::new(&mut header, &mut tx_state);

        let stats = coordinator.get_transaction_statistics();
        assert_eq!(stats.tx_id, 0);
        assert_eq!(stats.node_count, 100);
        assert_eq!(stats.edge_count, 200);
        assert_eq!(stats.free_space_offset, 5000);
    }

    #[test]
    fn test_validate_transaction_state() {
        let mut header = PersistentHeaderV2::new_v2();
        let mut tx_state = TransactionState::new();
        let mut coordinator = GraphFileCoordinator::new(&mut header, &mut tx_state);

        // Empty state should be valid
        assert!(coordinator.validate_transaction_state().is_ok());

        // Active transaction should be valid
        coordinator.begin_transaction(789).unwrap();
        assert!(coordinator.validate_transaction_state().is_ok());
    }

    #[test]
    fn test_reset_cluster_offsets() {
        let mut header = PersistentHeaderV2::new_v2();
        header.outgoing_cluster_offset = 12345;
        header.incoming_cluster_offset = 67890;
        let mut tx_state = TransactionState::new();
        let mut coordinator = GraphFileCoordinator::new(&mut header, &mut tx_state);

        coordinator.reset_cluster_offsets();

        assert_eq!(coordinator.persistent_header.outgoing_cluster_offset, 0);
        assert_eq!(coordinator.persistent_header.incoming_cluster_offset, 0);
    }
}
