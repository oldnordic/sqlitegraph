//! Edge storage capacity coordination module
//!
//! This module provides capacity coordination for edge storage, ensuring that
//! edge ID allocation is properly coordinated with file growth to prevent
//! "Attempted read beyond end of file" errors.

pub use coordinator::EdgeCapacityCoordinator;

mod coordinator;

/// Re-export the main capacity coordinator for external use
pub use coordinator::EdgeCapacityCoordinator as CapacityCoordinator;
