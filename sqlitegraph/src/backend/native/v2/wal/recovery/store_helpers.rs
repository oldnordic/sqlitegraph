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
