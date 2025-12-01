//! MVCC-lite snapshot system for SQLiteGraph
//!
//! Provides deterministic, read-only snapshots using the S3 Hybrid approach:
//! - SnapshotState stores immutable cloned HashMaps (not Arc-shared)
//! - ArcSwap provides lock-free atomic updates
//! - Read-only SQLite connections ensure database consistency
//! - Deterministic behavior with repeatable results

use arc_swap::ArcSwap;
use rusqlite::{Connection, OpenFlags, Result as SqliteResult};
use std::collections::HashMap;
use std::sync::Arc;

pub type NodeId = i64;

/// Immutable snapshot state containing cloned adjacency data
///
/// This structure stores complete copies of adjacency maps to ensure
/// true isolation - snapshots are unaffected by subsequent writes.
#[derive(Debug, Clone)]
pub struct SnapshotState {
    /// Immutable copy of outgoing adjacency map
    pub outgoing: HashMap<NodeId, Vec<NodeId>>,
    /// Immutable copy of incoming adjacency map  
    pub incoming: HashMap<NodeId, Vec<NodeId>>,
    /// Snapshot creation timestamp for debugging
    pub created_at: std::time::SystemTime,
}

impl SnapshotState {
    /// Create a new snapshot state by cloning adjacency maps
    ///
    /// # Arguments
    /// * `outgoing` - Current outgoing adjacency map to clone
    /// * `incoming` - Current incoming adjacency map to clone
    ///
    /// # Returns
    /// New SnapshotState with immutable cloned data
    pub fn new(
        outgoing: &HashMap<NodeId, Vec<NodeId>>,
        incoming: &HashMap<NodeId, Vec<NodeId>>,
    ) -> Self {
        Self {
            // Use .clone() to create deep copies, not Arc::clone()
            outgoing: outgoing.clone(),
            incoming: incoming.clone(),
            created_at: std::time::SystemTime::now(),
        }
    }

    /// Get the number of nodes in this snapshot
    pub fn node_count(&self) -> usize {
        self.outgoing.len()
    }

    /// Get the number of edges in this snapshot
    pub fn edge_count(&self) -> usize {
        self.outgoing.values().map(|adj| adj.len()).sum()
    }

    /// Check if a node exists in this snapshot
    pub fn contains_node(&self, node_id: NodeId) -> bool {
        self.outgoing.contains_key(&node_id)
    }

    /// Get outgoing neighbors for a node in this snapshot
    pub fn get_outgoing(&self, node_id: NodeId) -> Option<&Vec<NodeId>> {
        self.outgoing.get(&node_id)
    }

    /// Get incoming neighbors for a node in this snapshot
    pub fn get_incoming(&self, node_id: NodeId) -> Option<&Vec<NodeId>> {
        self.incoming.get(&node_id)
    }
}

/// MVCC snapshot manager using ArcSwap for atomic updates
///
/// Provides lock-free snapshot acquisition and deterministic behavior.
/// Snapshots are completely isolated from write operations.
#[derive(Debug)]
pub struct SnapshotManager {
    /// Atomic reference to current snapshot state
    current: ArcSwap<SnapshotState>,
}

impl SnapshotManager {
    /// Create a new snapshot manager with empty initial state
    pub fn new() -> Self {
        let initial_state = SnapshotState::new(&HashMap::new(), &HashMap::new());
        Self {
            current: ArcSwap::new(Arc::new(initial_state)),
        }
    }

    /// Create a new snapshot manager with initial state
    pub fn with_state(
        outgoing: &HashMap<NodeId, Vec<NodeId>>,
        incoming: &HashMap<NodeId, Vec<NodeId>>,
    ) -> Self {
        let initial_state = SnapshotState::new(outgoing, incoming);
        Self {
            current: ArcSwap::new(Arc::new(initial_state)),
        }
    }

    /// Atomically update the snapshot state
    ///
    /// # Arguments
    /// * `outgoing` - New outgoing adjacency map to clone
    /// * `incoming` - New incoming adjacency map to clone
    pub fn update_snapshot(
        &self,
        outgoing: &HashMap<NodeId, Vec<NodeId>>,
        incoming: &HashMap<NodeId, Vec<NodeId>>,
    ) {
        let new_state = SnapshotState::new(outgoing, incoming);
        self.current.store(Arc::new(new_state));
    }

    /// Acquire a deterministic snapshot of current state
    ///
    /// # Returns
    /// `Arc<SnapshotState>` containing immutable snapshot data
    pub fn acquire_snapshot(&self) -> Arc<SnapshotState> {
        self.current.load().clone()
    }

    /// Get current snapshot state without cloning (for internal use)
    pub fn current_snapshot(&self) -> Arc<SnapshotState> {
        self.current.load().clone()
    }
}

impl Default for SnapshotManager {
    fn default() -> Self {
        Self::new()
    }
}

/// MVCC-lite read-only snapshot for graph data isolation.
///
/// Provides safe, read-only access to a point-in-time view of the graph
/// with its own SQLite connection to ensure database consistency.
pub struct GraphSnapshot {
    /// Immutable snapshot state
    state: Arc<SnapshotState>,
    /// Read-only SQLite connection for database queries
    conn: Connection,
}

impl GraphSnapshot {
    /// Create a new graph snapshot
    ///
    /// # Arguments
    /// * `state` - Immutable snapshot state
    /// * `db_path` - Path to SQLite database
    ///
    /// # Returns
    /// Result containing GraphSnapshot or error
    pub fn new(state: Arc<SnapshotState>, db_path: &str) -> SqliteResult<Self> {
        // Create read-only connection to ensure database consistency
        let conn = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;

        Ok(Self { state, conn })
    }

    /// Get the snapshot state
    pub fn state(&self) -> &Arc<SnapshotState> {
        &self.state
    }

    /// Get the read-only database connection
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Get the number of nodes in this snapshot
    pub fn node_count(&self) -> usize {
        self.state.node_count()
    }

    /// Get the number of edges in this snapshot
    pub fn edge_count(&self) -> usize {
        self.state.edge_count()
    }

    /// Check if a node exists in this snapshot
    pub fn contains_node(&self, node_id: NodeId) -> bool {
        self.state.contains_node(node_id)
    }

    /// Get outgoing neighbors for a node in this snapshot
    pub fn get_outgoing(&self, node_id: NodeId) -> Option<&Vec<NodeId>> {
        self.state.get_outgoing(node_id)
    }

    /// Get incoming neighbors for a node in this snapshot
    pub fn get_incoming(&self, node_id: NodeId) -> Option<&Vec<NodeId>> {
        self.state.get_incoming(node_id)
    }

    /// Get snapshot creation timestamp
    pub fn created_at(&self) -> std::time::SystemTime {
        self.state.created_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_state_creation() {
        let mut outgoing = HashMap::new();
        let mut incoming = HashMap::new();

        outgoing.insert(1, vec![2, 3]);
        incoming.insert(1, vec![]);

        let state = SnapshotState::new(&outgoing, &incoming);

        assert_eq!(state.node_count(), 1);
        assert_eq!(state.edge_count(), 2);
        assert!(state.contains_node(1));
        assert!(!state.contains_node(2));
    }

    #[test]
    fn test_snapshot_manager() {
        let mut outgoing = HashMap::new();
        let mut incoming = HashMap::new();

        outgoing.insert(1, vec![2]);
        incoming.insert(1, vec![]);

        let manager = SnapshotManager::with_state(&outgoing, &incoming);

        let snapshot = manager.acquire_snapshot();
        assert_eq!(snapshot.node_count(), 1);
        assert!(snapshot.contains_node(1));

        // Update state
        outgoing.insert(2, vec![]);
        incoming.insert(2, vec![1]);
        manager.update_snapshot(&outgoing, &incoming);

        let new_snapshot = manager.acquire_snapshot();
        assert_eq!(new_snapshot.node_count(), 2);

        // Original snapshot should be unchanged
        assert_eq!(snapshot.node_count(), 1);
    }
}
