//! Checkpoint Record Processing Module
//!
//! This module contains record processing logic for applying WAL records
//! to the V2 clustered edge graph format during checkpoint operations.

pub mod integrator;

// Re-export the main integrator for easier access
pub use self::integrator::V2GraphIntegrator;