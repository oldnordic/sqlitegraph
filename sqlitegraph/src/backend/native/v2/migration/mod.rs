//! File format migration for SQLiteGraph
//!
//! Automatic detection and migration of old file formats to current version.
//! Migrations are atomic with automatic rollback on failure.

pub mod detect;
pub mod execute;

pub use detect::{detect_format_version, needs_migration, FormatVersion};
pub use execute::{migrate_file, MigrationError, MigrationResult};
