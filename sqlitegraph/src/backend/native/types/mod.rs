//! Core type definitions for native backend
//!
//! This module provides all the core structs, enums, and error types needed
//! for the native graph database file format, organized into logical submodules.

// Public exports - maintain the same public API as before
pub use aliases::*;
pub use flags::*;
pub use file_header::*;
pub use records::*;
pub use errors::*;
pub use cpu_profile::*;
pub use utils::*;

// Module declarations
mod aliases;
mod flags;
mod file_header;
mod records;
mod errors;
mod cpu_profile;
mod utils;