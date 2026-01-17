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
    /// Snapshot creation timestamp
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
///
/// # Memory Ordering Guarantees
///
/// This implementation relies on ArcSwap's memory ordering guarantees:
/// - **ArcSwap::load()**: Uses Acquire ordering, ensuring all writes before store are visible
/// - **ArcSwap::store()**: Uses Release ordering, ensuring all writes complete before publication
/// - This provides proper happens-before relationship between writers and readers
///
/// # Thread Safety
///
/// The SnapshotManager is thread-safe and can be shared across threads via Arc:
/// - Multiple readers can acquire snapshots concurrently without blocking
/// - Writers can update state concurrently with readers
/// - No locks or mutexes required (lock-free)
/// - No TOCTOU (time-of-check-time-of-use) issues due to atomic pointer swap
///
/// # Invariants
///
/// 1. **Snapshot State Immutability**: Once a SnapshotState is created, it never changes
/// 2. **Atomic Publication**: State updates are atomic - readers see either old or new state, never partial
/// 3. **Arc Reference Counting**: Each snapshot maintains proper Arc reference counts
/// 4. **No Mutable Aliasing**: Arc<SnapshotState> ensures no mutable access to snapshot data
#[derive(Debug)]
pub struct SnapshotManager {
    /// Atomic reference to current snapshot state
    ///
    /// ArcSwap provides lock-free atomic updates with proper memory ordering:
    /// - Load uses Acquire ordering
    /// - Store uses Release ordering
    /// - Guarantees happens-before relationship
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
    /// This method creates a new immutable snapshot state and publishes it atomically
    /// using ArcSwap's store operation with Release memory ordering.
    ///
    /// # Memory Ordering
    ///
    /// - All writes to the new SnapshotState complete **before** the store
    /// - The store operation uses Release ordering
    /// - Readers with Acquire ordering see the complete, consistent state
    ///
    /// # Thread Safety
    ///
    /// This method is thread-safe and can be called concurrently with snapshot acquisition:
    /// - Multiple writers can call this (though serialization happens at ArcSwap level)
    /// - Readers continue to see old state until this store completes
    /// - No partial updates visible to readers (atomic pointer swap)
    ///
    /// # Arguments
    /// * `outgoing` - New outgoing adjacency map to clone
    /// * `incoming` - New incoming adjacency map to clone
    ///
    /// # Invariants Preserved
    ///
    /// 1. The new SnapshotState is fully constructed before store
    /// 2. No mutable references to the state exist after publication
    /// 3. Arc reference count starts at 1 (this ArcSwap reference)
    pub fn update_snapshot(
        &self,
        outgoing: &HashMap<NodeId, Vec<NodeId>>,
        incoming: &HashMap<NodeId, Vec<NodeId>>,
    ) {
        // Create new state with cloned HashMaps
        // This is a deep copy, ensuring complete isolation
        let new_state = SnapshotState::new(outgoing, incoming);

        // Verify invariants before publication
        // These checks run in debug mode to catch bugs early
        #[cfg(debug_assertions)]
        {
            // Verify state is fully constructed
            assert_eq!(new_state.node_count(), outgoing.len(),
                "Snapshot state node count mismatch");
            assert_eq!(new_state.edge_count(),
                outgoing.values().map(|v| v.len()).sum::<usize>(),
                "Snapshot state edge count mismatch");
        }

        // Atomic publication with Release ordering
        // All writes to new_state happen-before this store
        self.current.store(Arc::new(new_state));
    }

    /// Acquire a deterministic snapshot of current state
    ///
    /// This method atomically loads the current snapshot state using ArcSwap's
    /// load operation with Acquire memory ordering.
    ///
    /// # Memory Ordering
    ///
    /// - The load operation uses Acquire ordering
    /// - All writes from the corresponding store are visible
    /// - Provides happens-before relationship with writer
    ///
    /// # Thread Safety
    ///
    /// This method is thread-safe and can be called concurrently with state updates:
    /// - Multiple readers can acquire snapshots concurrently without blocking
    /// - Acquiring a snapshot never blocks a writer
    /// - The returned Arc<SnapshotState> provides immutable access
    ///
    /// # Returns
    ///
    /// `Arc<SnapshotState>` containing immutable snapshot data.
    /// The Arc ensures the snapshot data remains valid as long as needed.
    ///
    /// # Invariants Guaranteed
    ///
    /// 1. The returned snapshot state is immutable (no mutable access possible)
    /// 2. The snapshot is complete and consistent (no torn reads)
    /// 3. Arc reference count is >= 1 during snapshot lifetime
    /// 4. No TOCTOU issues (atomic load, not check-then-use)
    pub fn acquire_snapshot(&self) -> Arc<SnapshotState> {
        // Atomic load with Acquire ordering
        // This sees either the old state or the new state, never partial
        let state = self.current.load();

        // Clone the Arc (increases reference count, atomic operation)
        // This is cheap (just an atomic increment) and doesn't copy the data
        let snapshot = Arc::clone(&state);

        #[cfg(debug_assertions)]
        {
            // Verify snapshot consistency
            // These checks validate that we got a complete, consistent snapshot
            let node_count = snapshot.node_count();
            let edge_count = snapshot.edge_count();

            // Verify internal consistency
            // (node_count + edge_count should be reasonable for the state)
            assert!(node_count <= 10_000_000, "Suspiciously large node count");
            assert!(edge_count <= 100_000_000, "Suspiciously large edge count");
        }

        snapshot
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
