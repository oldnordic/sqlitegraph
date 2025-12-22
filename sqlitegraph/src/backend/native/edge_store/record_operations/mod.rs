//! Edge record operations module
//!
//! This module provides modularized edge record CRUD operations, serialization,
//! validation, and comprehensive testing functionality with proper separation
//! of concerns and focused responsibilities.

// Re-export core types
pub use operations::EdgeRecordOperations;

// Import modules
mod operations;
mod serialization;
#[cfg(test)]
mod tests;
mod validation;
