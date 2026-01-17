//! Centralized debug logging with feature flag control
//!
//! This module provides conditional logging macros that are only compiled
//! when the `debug` feature is enabled. This allows release builds to
//! completely exclude debug/info logging for zero runtime overhead.
//!
//! # Usage
//!
//! ```rust,ignore
//! use crate::debug::{debug_log, info_log};
//!
//! debug_log!("This is a debug message: {}", value);
//! info_log!("This is an info message: {}", value);
//! ```
//!
//! The macros expand to nothing when the `debug` feature is not enabled,
//! ensuring zero runtime overhead in release builds.

/// Debug logging macro - only compiled when `debug` feature is enabled
#[cfg(feature = "debug")]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        log::debug!($($arg)*);
    };
}

/// Debug logging macro - compiles to nothing when `debug` feature is disabled
#[cfg(not(feature = "debug"))]
macro_rules! debug_log {
    ($($arg:tt)*) => {
        // Debug logging disabled - compile to nothing
    };
}

/// Info logging macro - only compiled when `debug` feature is enabled
#[cfg(feature = "debug")]
macro_rules! info_log {
    ($($arg:tt)*) => {
        log::info!($($arg)*);
    };
}

/// Info logging macro - compiles to nothing when `debug` feature is disabled
#[cfg(not(feature = "debug"))]
macro_rules! info_log {
    ($($arg:tt)*) => {
        // Info logging disabled - compile to nothing
    };
}

/// Warn logging macro - always enabled (even in release builds)
/// for critical diagnostics
macro_rules! warn_log {
    ($($arg:tt)*) => {
        log::warn!($($arg)*);
    };
}

/// Error logging macro - always enabled (even in release builds)
/// for critical diagnostics
macro_rules! error_log {
    ($($arg:tt)*) => {
        log::error!($($arg)*);
    };
}

// Export macros for use in other modules
pub(crate) use debug_log;
pub(crate) use info_log;
pub(crate) use warn_log;
pub(crate) use error_log;
