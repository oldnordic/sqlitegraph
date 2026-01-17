//! Edge clustering primitives for the V2 storage kernel.
//!
//! This module is intentionally split into small components to keep each file
//! below the 300‑line architectural limit. Public consumers should import
//! items from this `mod.rs` re-export rather than submodules directly.

mod cache;
mod cluster;
mod cluster_serialization;
mod cluster_trace;
mod compact_record;
mod record_ext;

pub use cache::{CacheEntry, CacheKey, ThreadSafeCache, TraversalAwareCache};
pub use cluster::EdgeCluster;
pub use cluster_trace::{Direction, StrictModeGuard, TraceContext, TraceGuard};
pub use compact_record::CompactEdgeRecord;
pub use record_ext::EdgeRecordCompactExt;
