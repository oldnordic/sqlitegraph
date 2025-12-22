//! SQLiteGraph V2 Snapshot Management Module
//!
//! This module provides comprehensive snapshot lifecycle management for V2 clustered edge
//! format databases. Snapshots provide instant database state exports without WAL complexity.
//!
//! ## Module Organization
//!
//! - **lifecycle**: Snapshot lifecycle state management and inspection
//! - **atomic_ops**: Database-grade atomic file operations (Phase 2)
//! - **validation**: Snapshot integrity and validation operations
//!
//! ## Architecture
//!
//! The snapshot system follows strict lifecycle principles:
//! 1. **Deterministic State**: Explicit lifecycle states with clear transitions
//! 2. **READ-ONLY Inspection**: Lifecycle inspection has no side effects
//! 3. **Atomic Operations**: All file operations use atomic semantics
//! 4. **Recovery Integration**: Snapshots bypass WAL recovery for instant restore

pub mod atomic_ops;
pub mod lifecycle;

// Re-export lifecycle components
pub use lifecycle::{
    SnapshotLifecycleInspector, SnapshotLifecycleState, SnapshotMetadata,
};

// Re-export atomic operations components
pub use atomic_ops::{
    AtomicFileOperations, AtomicOperationError,
};