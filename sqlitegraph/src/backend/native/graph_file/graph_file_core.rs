//! Core GraphFile API operations
//!
//! This module contains the core GraphFile methods for file creation,
//! opening, and basic lifecycle operations.

use crate::backend::native::{
    constants::node::NODE_SLOT_SIZE, graph_file::FileLifecycleManager,
    graph_file::TransactionManager, graph_file::validation::GraphFileValidator,
};

use super::{DEFAULT_NODE_DATA_START, GraphFile, RESERVED_NODE_REGION_BYTES};

impl GraphFile {
    /// Calculate the minimum safe offset for cluster allocation
    /// Ensures clusters are always outside the node region even when node_count = 0
    pub fn cluster_floor(&self) -> u64 {
        let header = &self.persistent_header;
        let node_region_end = header.node_data_offset + (header.node_count as u64 * NODE_SLOT_SIZE);

        // Ensure minimum separation: clusters must start at least 1MB beyond node data
        let min_cluster_start = header.node_data_offset + (1024 * 1024);

        std::cmp::max(node_region_end, min_cluster_start)
    }

    /// Create a new graph file with initial header
    pub fn create<P: AsRef<std::path::Path>>(
        path: P,
    ) -> crate::backend::native::types::NativeResult<Self> {
        FileLifecycleManager::create(path)
    }

    /// Open an existing graph file
    pub fn open<P: AsRef<std::path::Path>>(
        path: P,
    ) -> crate::backend::native::types::NativeResult<Self> {
        FileLifecycleManager::open(path)
    }

    /// Read header from file
    pub fn read_header(&mut self) -> crate::backend::native::types::NativeResult<()> {
        FileLifecycleManager::read_header(self)
    }

    /// Get current transaction ID
    pub fn current_transaction_id(&self) -> u64 {
        self.transaction_state.current_transaction_id()
    }

    /// Check if transaction is currently active
    pub fn is_transaction_active(&self) -> bool {
        self.transaction_state.is_active()
    }

    /// Get transaction statistics
    pub fn get_transaction_statistics(
        &self,
    ) -> crate::backend::native::graph_file::transaction::TransactionStatistics {
        crate::backend::native::graph_file::transaction::TransactionStatistics {
            tx_id: self.current_transaction_id(),
            node_count: self.persistent_header().node_count,
            edge_count: self.persistent_header().edge_count,
            free_space_offset: self.persistent_header().free_space_offset,
            is_active: self.is_transaction_active(),
            state: if self.is_transaction_active() {
                "InProgress".to_string()
            } else {
                "Inactive".to_string()
            },
        }
    }

    /// Begin a new transaction
    pub fn begin_transaction(&mut self) -> crate::backend::native::types::NativeResult<u64> {
        use crate::backend::native::graph_file::graph_file_coordinator::GraphFileCoordinator;

        let tx_id = self.transaction_state.current_transaction_id() + 1;

        let mut coordinator =
            GraphFileCoordinator::new(&mut self.persistent_header, &mut self.transaction_state);

        coordinator.begin_transaction(tx_id);
        Ok(tx_id)
    }

    /// Commit the current transaction
    pub fn commit_transaction(&mut self) -> crate::backend::native::types::NativeResult<()> {
        use crate::backend::native::graph_file::graph_file_coordinator::GraphFileCoordinator;

        let mut coordinator =
            GraphFileCoordinator::new(&mut self.persistent_header, &mut self.transaction_state);

        // Use simple closures that just return Ok(()) - the actual operations
        // will be handled by the coordinator internally
        coordinator.commit_transaction(|| Ok(()), || Ok(()))?;

        // Perform the actual file operations after coordinator is done
        self.write_header()?;
        self.sync()?;
        Ok(())
    }

    /// Rollback the current transaction
    pub fn rollback_transaction(&mut self) -> crate::backend::native::types::NativeResult<()> {
        use crate::backend::native::graph_file::graph_file_coordinator::GraphFileCoordinator;

        let file_size = self.file_size()?;
        let node_data_offset = self.persistent_header.node_data_offset;
        let node_count = self.persistent_header.node_count as u32;

        let mut coordinator =
            GraphFileCoordinator::new(&mut self.persistent_header, &mut self.transaction_state);

        coordinator.rollback_transaction(
            file_size,
            node_data_offset,
            node_count,
            |new_size| {
                self.file.set_len(new_size)?;
                Ok(())
            },
            crate::backend::native::constants::node::NODE_SLOT_SIZE,
        )
    }

    /// Write commit marker value
    pub fn write_commit_marker_value(
        &mut self,
        value: u64,
    ) -> crate::backend::native::types::NativeResult<()> {
        TransactionManager::write_commit_marker_value(&mut self.file, value)
    }

    /// Read commit marker value
    pub fn read_commit_marker_value(&mut self) -> crate::backend::native::types::NativeResult<u64> {
        TransactionManager::read_commit_marker_value(&mut self.file)
    }

    /// Verify commit marker
    pub fn verify_commit_marker(&mut self) -> crate::backend::native::types::NativeResult<()> {
        let marker = self.read_commit_marker_value()?;
        GraphFileValidator::verify_commit_marker(marker)
    }

    /// Begin cluster commit
    pub fn begin_cluster_commit(&mut self) -> crate::backend::native::types::NativeResult<()> {
        TransactionManager::begin_cluster_commit(&mut self.file)
    }

    /// Initialize V2 header
    fn initialize_v2_header(&mut self) {
        let node_count = self.persistent_header().node_count;
        crate::backend::native::graph_file::header::HeaderManager::initialize_v2_header(
            self.persistent_header_mut(),
            node_count,
            DEFAULT_NODE_DATA_START,
            RESERVED_NODE_REGION_BYTES,
        )
        .expect("Failed to initialize V2 header");

        // CRITICAL SAFETY: Validate header after initialization
        crate::backend::native::graph_file::header::HeaderManager::validate_header_invariants(
            &self.persistent_header,
        )
        .expect("Header invariants violated after initialization");
    }
}
