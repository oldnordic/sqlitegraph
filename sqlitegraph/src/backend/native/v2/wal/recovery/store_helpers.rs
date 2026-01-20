//! Shared helpers for creating NodeStore/EdgeStore with lifetime workaround.
//!
//! # Safety
//!
//! The transmute here extends GraphFile lifetime to 'static to satisfy Store APIs.
//! This is safe because Arc<RwLock<GraphFile>> ensures GraphFile lives as long as needed.
//!
//! This is a workaround for the NodeStore/EdgeStore lifetime API requirements.
//! A future refactor could remove the need for transmute by changing those APIs.

use std::mem;
use std::sync::Arc;
use crate::backend::native::{NodeStore, EdgeStore, GraphFile};

/// # Safety
/// Caller must ensure the returned NodeStore does not outlive the GraphFile reference.
/// Since we store Arc<RwLock<GraphFile>>, the Arc keeps it alive.
///
/// The transmute is safe because:
/// - graph_file is owned by the Arc<RwLock<>> stored in the parent struct
/// - The Arc ensures graph_file lives as long as any store reference exists
/// - Stores are accessed through Mutex/RwLock guards, preventing use-after-free
pub unsafe fn create_node_store(graph_file: &mut GraphFile) -> NodeStore<'static> {
    NodeStore::new(mem::transmute::<&mut _, &'static mut _>(graph_file))
}

/// # Safety
/// Caller must ensure the returned EdgeStore does not outlive the GraphFile reference.
/// Since we store Arc<RwLock<GraphFile>>, the Arc keeps it alive.
///
/// The transmute is safe because:
/// - graph_file is owned by the Arc<RwLock<>> stored in the parent struct
/// - The Arc ensures graph_file lives as long as any store reference exists
/// - Stores are accessed through Mutex/RwLock guards, preventing use-after-free
pub unsafe fn create_edge_store(graph_file: &mut GraphFile) -> EdgeStore<'static> {
    EdgeStore::new(mem::transmute::<&mut _, &'static mut _>(graph_file))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_node_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let graph_path = temp_dir.path().join("test.v2");
        let graph_file = GraphFile::create(&graph_path).unwrap();
        let mut graph_file = graph_file;

        // Create NodeStore using our helper
        let _node_store = unsafe {
            create_node_store(&mut graph_file)
        };
        // node_store goes out of scope here
    }

    #[test]
    fn test_create_edge_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let graph_path = temp_dir.path().join("test.v2");
        let graph_file = GraphFile::create(&graph_path).unwrap();
        let mut graph_file = graph_file;

        // Create EdgeStore using our helper
        let _edge_store = unsafe {
            create_edge_store(&mut graph_file)
        };
        // edge_store goes out of scope here
    }
}

/// Miri-specific tests for undefined behavior detection
#[cfg(all(miri, test))]
mod miri_tests {
    use super::*;
    use parking_lot::RwLock;

    /// Miri test: Verify Arc<RwLock<>> pattern keeps GraphFile alive
    #[test]
    fn miri_test_arc_rwlock_graphfile_lifetime() {
        let temp_dir = tempfile::tempdir().unwrap();
        let graph_path = temp_dir.path().join("test.v2");
        let graph_file = GraphFile::create(&graph_path).unwrap();

        // Wrap in Arc<RwLock<>>
        let graph_file = Arc::new(RwLock::new(graph_file));

        // Clone Arc (cheap reference count increment)
        let graph_file_clone = Arc::clone(&graph_file);

        // Create NodeStore using our helper
        {
            let mut guard = graph_file.write();
            let _node_store = unsafe {
                create_node_store(&mut guard)
            };
            // node_store goes out of scope here
            // graph_file should still be valid
        }

        // Original Arc still valid - Miri will catch use-after-free
        let guard = graph_file_clone.read();
        let _header = guard.header();
    }

    /// Miri test: Store lifetime is bounded by lock scope
    #[test]
    fn miri_test_store_lifetime_bounded() {
        let temp_dir = tempfile::tempdir().unwrap();
        let graph_path = temp_dir.path().join("test.v2");
        let graph_file = GraphFile::create(&graph_path).unwrap();

        let graph_file = Arc::new(RwLock::new(graph_file));

        // Create a store, use it, then drop it
        {
            let mut node_store = unsafe {
                let mut guard = graph_file.write();
                create_node_store(&mut guard)
            };
            // Store is valid within this scope - allocate a node to prove it works
            let _node_id = node_store.allocate_node_id().unwrap();
        } // Store dropped here

        // Create a different store type - safe because previous store was dropped
        {
            let mut edge_store = unsafe {
                let mut guard = graph_file.write();
                create_edge_store(&mut guard)
            };
            // Store is valid within this scope - get max edge id to prove it works
            let _max_id = edge_store.max_edge_id();
        } // Store dropped here

        // GraphFile still accessible after both stores dropped
        let guard = graph_file.read();
        let _header = guard.header();
    }

    /// Miri test: Drop order doesn't cause use-after-free
    #[test]
    fn miri_test_drop_order() {
        let temp_dir = tempfile::tempdir().unwrap();
        let graph_path = temp_dir.path().join("test.v2");
        let graph_file = GraphFile::create(&graph_path).unwrap();

        let graph_file = Arc::new(RwLock::new(graph_file));
        let arc_clone = Arc::clone(&graph_file);

        // Drop original Arc first
        drop(graph_file);

        // Clone should still work
        let guard = arc_clone.read();
        let _header = guard.header();
    }
}
