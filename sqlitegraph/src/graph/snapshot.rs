//! Snapshot management functionality for SqliteGraph.

use std::sync::Arc;

use crate::errors::SqliteGraphError;

use super::SqliteGraph;

impl SqliteGraph {
    /// Update snapshot with current cache state
    /// This is called automatically after write operations
    pub(crate) fn update_snapshot(&self) {
        self.snapshot_manager.update_snapshot(
            &self.outgoing_cache_ref().inner(),
            &self.incoming_cache_ref().inner(),
        );
    }

    /// Acquire a deterministic snapshot of the current graph state
    ///
    /// Returns a read-only snapshot that provides isolated access to graph data.
    /// The snapshot contains cloned adjacency maps and uses a read-only SQLite connection.
    ///
    /// # Returns
    /// Result containing GraphSnapshot or error
    pub fn acquire_snapshot(&self) -> Result<crate::mvcc::GraphSnapshot, SqliteGraphError> {
        // Update snapshot with current cache state
        self.update_snapshot();

        // Acquire snapshot state
        let snapshot_state = self.snapshot_manager.acquire_snapshot();

        // Use in-memory database for snapshot operations
        let db_path = ":memory:";

        crate::mvcc::GraphSnapshot::new(snapshot_state, db_path)
            .map_err(|e| SqliteGraphError::connection(e.to_string()))
    }

    /// Get the current snapshot state without creating a new connection
    /// This is useful for internal operations and testing
    pub(crate) fn current_snapshot_state(&self) -> Arc<crate::mvcc::SnapshotState> {
        self.update_snapshot();
        self.snapshot_manager.current_snapshot()
    }

    /// Get the number of nodes in the current snapshot
    pub fn snapshot_node_count(&self) -> usize {
        self.current_snapshot_state().node_count()
    }

    /// Get the number of edges in the current snapshot
    pub fn snapshot_edge_count(&self) -> usize {
        self.current_snapshot_state().edge_count()
    }

    /// Check if a node exists in the current snapshot
    pub fn snapshot_contains_node(&self, node_id: i64) -> bool {
        self.current_snapshot_state().contains_node(node_id)
    }
}
