//! V2 WAL Recovery Replay Operations - Modular Organization
//!
//! This module provides production-grade replay operations for V2 WAL recovery.
//! The functionality is split across focused modules for maintainability.

use crate::backend::native::{
    GraphFile, NodeStore, EdgeStore,
};
use crate::backend::native::v2::{
    StringTable, FreeSpaceManager,
};
use std::sync::{Arc, Mutex, RwLock};

// Re-export operation handler implementations
mod node_ops;
mod edge_ops;
mod transaction_ops;

// Logging macros
macro_rules! debug { ($($arg:tt)*) => { log::debug!($($arg)*); }; }
macro_rules! warn { ($($arg:tt)*) => { log::warn!($($arg)*); }; }
macro_rules! error { ($($arg:tt)*) => { log::error!($($arg)*); }; }

/// Production-grade replay operations handler
///
/// This struct provides concrete implementations for all V2 WAL replay operations
/// with proper error handling, rollback support, and statistics tracking.
///
/// The operations are organized across multiple modules:
/// - `node_ops`: Node insertion, update, deletion
/// - `edge_ops`: Edge insertion, update, deletion
/// - `transaction_ops`: String insertion, cluster creation, free space management, header updates
pub struct DefaultReplayOperations {
    /// Graph file reference
    pub(in crate::backend::native::v2::wal::recovery::replayer) graph_file: Arc<RwLock<GraphFile>>,
    /// Node store (initialized on demand)
    pub(in crate::backend::native::v2::wal::recovery::replayer) node_store: Arc<Mutex<Option<NodeStore<'static>>>>,
    /// Edge store (initialized on demand)
    pub(in crate::backend::native::v2::wal::recovery::replayer) edge_store: Arc<Mutex<Option<EdgeStore<'static>>>>,
    /// String table for V2 string management
    pub(in crate::backend::native::v2::wal::recovery::replayer) string_table: Arc<Mutex<StringTable>>,
    /// Free space manager for slot deallocation
    pub(in crate::backend::native::v2::wal::recovery::replayer) free_space_manager: Arc<Mutex<Option<FreeSpaceManager>>>,
    /// Statistics tracking
    pub(in crate::backend::native::v2::wal::recovery::replayer) statistics: Arc<Mutex<crate::backend::native::v2::wal::recovery::replayer::types::ReplayStatistics>>,
}

impl DefaultReplayOperations {
    /// Create a new operations handler
    pub fn new(
        graph_file: Arc<RwLock<GraphFile>>,
        node_store: Arc<Mutex<Option<NodeStore<'static>>>>,
        edge_store: Arc<Mutex<Option<EdgeStore<'static>>>>,
        string_table: Arc<Mutex<StringTable>>,
        free_space_manager: Arc<Mutex<Option<FreeSpaceManager>>>,
        statistics: Arc<Mutex<crate::backend::native::v2::wal::recovery::replayer::types::ReplayStatistics>>,
    ) -> Self {
        Self {
            graph_file,
            node_store,
            edge_store,
            string_table,
            free_space_manager,
            statistics,
        }
    }

    // Test helper functions

    #[cfg(test)]
    /// Create test operations instance
    pub fn create_test_operations() -> Self {
        use tempfile::NamedTempFile;

        // Create temporary file for GraphFile
        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let graph_file = GraphFile::create(temp_file.path()).expect("Failed to create GraphFile");
        let graph_file = Arc::new(RwLock::new(graph_file));

        // Initialize components
        let node_store: Arc<Mutex<Option<NodeStore<'static>>>> = Arc::new(Mutex::new(None));
        let edge_store: Arc<Mutex<Option<EdgeStore<'static>>>> = Arc::new(Mutex::new(None));
        let string_table = Arc::new(Mutex::new(StringTable::new()));
        let mut free_space_mgr = crate::backend::native::v2::free_space::FreeSpaceManager::new(
            crate::backend::native::v2::free_space::AllocationStrategy::FirstFit
        );

        // Add initial free space for testing (like a fresh file with available space)
        // Add a large free block starting at offset 2048 (after headers and initial data)
        free_space_mgr.add_free_block(2048, 1024 * 1024); // 1MB of free space starting at offset 2048

        let free_space_manager = Arc::new(Mutex::new(Some(free_space_mgr)));
        let statistics = Arc::new(Mutex::new(crate::backend::native::v2::wal::recovery::replayer::types::ReplayStatistics::new()));

        DefaultReplayOperations {
            graph_file,
            node_store,
            edge_store,
            string_table,
            free_space_manager,
            statistics,
        }
    }
}

// Include operation handler implementations from each module
// These are defined as impl blocks in their respective modules
use node_ops::*;
use edge_ops::*;
use transaction_ops::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operations_creation() {
        use tempfile::NamedTempFile;

        let temp_file = NamedTempFile::new().expect("Failed to create temp file");
        let graph_file = GraphFile::create(temp_file.path()).expect("Failed to create GraphFile");
        let graph_file = Arc::new(RwLock::new(graph_file));

        let node_store: Arc<Mutex<Option<NodeStore<'static>>>> = Arc::new(Mutex::new(None));
        let edge_store: Arc<Mutex<Option<EdgeStore<'static>>>> = Arc::new(Mutex::new(None));
        let string_table = Arc::new(Mutex::new(StringTable::new()));
        let free_space_manager = Arc::new(Mutex::new(None));
        let statistics = Arc::new(Mutex::new(crate::backend::native::v2::wal::recovery::replayer::types::ReplayStatistics::new()));

        let ops = DefaultReplayOperations::new(
            graph_file,
            node_store,
            edge_store,
            string_table,
            free_space_manager,
            statistics,
        );

        // Verify the operations handler was created successfully
        assert_eq!(ops.statistics.lock().unwrap().total_operations(), 0);
    }
}
