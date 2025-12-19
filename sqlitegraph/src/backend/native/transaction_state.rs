//! Runtime transaction state for V2 native backend
//!
//! This module defines the transaction metadata that is kept in memory only
//! and never persisted to disk. This eliminates the header corruption issue
//! caused by struct size mismatches.

/// Runtime-only transaction state for V2 atomic commit protocol
///
/// This struct contains all transaction-related metadata that should never be
/// written to the persistent file header. It's maintained entirely in memory
/// by the GraphFile / NativeBackend runtime.
#[derive(Debug, Clone)]
pub struct TransactionState {
    /// Previous outgoing cluster offset for rollback
    pub tx_prev_outgoing_cluster_offset: u64,
    /// Previous incoming cluster offset for rollback
    pub tx_prev_incoming_cluster_offset: u64,
    /// Previous free space offset for rollback
    pub tx_prev_free_space_offset: u64,
    /// Transaction identifier for crash detection
    pub tx_id: u64,
}

impl TransactionState {
    /// Create a new transaction state with default values
    pub fn new() -> Self {
        Self {
            tx_prev_outgoing_cluster_offset: 0,
            tx_prev_incoming_cluster_offset: 0,
            tx_prev_free_space_offset: 0,
            tx_id: 0,
        }
    }

    /// Begin a new transaction and save current state
    pub fn begin_tx(&mut self, current_tx_id: u64) {
        self.tx_id = current_tx_id;
    }

    /// Save current cluster offsets for rollback
    pub fn save_checkpoint(
        &mut self,
        outgoing_offset: u64,
        incoming_offset: u64,
        free_space_offset: u64,
    ) {
        self.tx_prev_outgoing_cluster_offset = outgoing_offset;
        self.tx_prev_incoming_cluster_offset = incoming_offset;
        self.tx_prev_free_space_offset = free_space_offset;
    }

    /// Check if transaction is in progress
    pub fn is_in_progress(&self) -> bool {
        self.tx_id > 0
    }

    /// Rollback to saved checkpoint
    pub fn rollback(&mut self) -> (u64, u64, u64) {
        (
            self.tx_prev_outgoing_cluster_offset,
            self.tx_prev_incoming_cluster_offset,
            self.tx_prev_free_space_offset,
        )
    }

    /// Commit transaction and reset state
    pub fn commit(&mut self) {
        self.tx_id = 0;
        self.tx_prev_outgoing_cluster_offset = 0;
        self.tx_prev_incoming_cluster_offset = 0;
        self.tx_prev_free_space_offset = 0;
    }

    /// Get current transaction ID (alias for compatibility)
    pub fn current_transaction_id(&self) -> u64 {
        self.tx_id
    }

    /// Check if transaction is active (alias for is_in_progress)
    pub fn is_active(&self) -> bool {
        self.is_in_progress()
    }
}

impl Default for TransactionState {
    fn default() -> Self {
        Self::new()
    }
}
