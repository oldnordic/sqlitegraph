//! Graph reindexing functionality for syncore and sync graph operations.
//!
//! Provides comprehensive reindexing capabilities including:
//! - Syncore reindexing: Core database indexes (entities, edges, labels, properties)
//! - Sync graph reindexing: Graph-specific indexes and adjacency caches
//! - Progress tracking and validation
//! - Transactional safety with rollback capability

pub use cache::CacheRebuilder;
pub use core::{GraphReindexer, ReindexConfig};
pub use entity_edge::EntityEdgeRebuilder;
pub use label_property::LabelPropertyRebuilder;
pub use progress::{ReindexProgress, ReindexStage};
pub use validation::ReindexResult;

mod cache;
mod core;
mod entity_edge;
mod label_property;
mod progress;
mod validation;
