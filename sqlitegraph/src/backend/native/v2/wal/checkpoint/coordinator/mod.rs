//! Checkpoint Coordinator Module
//!
//! This module contains the main checkpoint orchestration logic that coordinates
//! the overall checkpoint process, managing record processing, progress tracking,
//! and coordination between different checkpoint subsystems.


// Re-export the main CheckpointExecutor as the primary public API
pub use self::executor::CheckpointExecutor;

mod executor;