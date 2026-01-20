//! Database backup API for SQLiteGraph
//!
//! Provides user-friendly backup functionality that creates consistent
//! snapshots of the database including all data pages and WAL state.

use crate::backend::native::types::{NativeResult, NativeBackendError};
use crate::backend::native::v2::{
    export::snapshot::{SnapshotExporter, SnapshotExportConfig, SnapshotExportResult},
    wal::V2WALManager,
};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Configuration for backup operations
#[derive(Debug, Clone)]
pub struct BackupConfig {
    /// Destination directory for backup
    pub backup_dir: PathBuf,

    /// Optional backup identifier (defaults to timestamp)
    pub backup_id: Option<String>,

    /// Whether to force checkpoint before backup (default: true)
    pub checkpoint_before_backup: bool,

    /// Whether to include statistics in manifest (default: true)
    pub include_statistics: bool,

    /// Whether to validate checksum (default: true)
    pub validate_checksum: bool,
}

impl Default for BackupConfig {
    fn default() -> Self {
        Self {
            backup_dir: PathBuf::from("backup"),
            backup_id: None,
            checkpoint_before_backup: true,
            include_statistics: true,
            validate_checksum: true,
        }
    }
}

impl BackupConfig {
    /// Create a new backup configuration with the specified backup directory
    pub fn new(backup_dir: impl AsRef<Path>) -> Self {
        Self {
            backup_dir: backup_dir.as_ref().to_path_buf(),
            ..Default::default()
        }
    }

    /// Set a custom backup identifier
    pub fn with_backup_id(mut self, id: impl Into<String>) -> Self {
        self.backup_id = Some(id.into());
        self
    }

    /// Enable or disable checkpoint before backup
    pub fn with_checkpoint(mut self, enabled: bool) -> Self {
        self.checkpoint_before_backup = enabled;
        self
    }

    /// Enable or disable statistics inclusion in manifest
    pub fn with_statistics(mut self, enabled: bool) -> Self {
        self.include_statistics = enabled;
        self
    }

    /// Enable or disable checksum validation
    pub fn with_checksum_validation(mut self, enabled: bool) -> Self {
        self.validate_checksum = enabled;
        self
    }
}

/// Result of a backup operation
#[derive(Debug, Clone)]
pub struct BackupResult {
    /// Path to backup snapshot file
    pub snapshot_path: PathBuf,

    /// Path to backup manifest file
    pub manifest_path: PathBuf,

    /// Backup size in bytes
    pub size_bytes: u64,

    /// Backup checksum
    pub checksum: u64,

    /// Number of records in backup
    pub record_count: u64,

    /// Backup duration in seconds
    pub duration_secs: f64,

    /// Backup timestamp (Unix epoch)
    pub timestamp: u64,

    /// Whether checkpoint was performed before backup
    pub checkpoint_performed: bool,
}

/// Create a backup of the database
///
/// This function creates a consistent snapshot of the database including
/// all data pages and metadata. Optionally performs a checkpoint before
/// backup to ensure WAL is applied.
///
/// # Arguments
/// * `graph_path` - Path to the graph database file
/// * `config` - Backup configuration options
///
/// # Returns
/// Backup result containing file paths, checksum, and metadata
///
/// # Errors
/// Returns an error if:
/// - The graph file does not exist
/// - The graph file is corrupt
/// - Backup directory cannot be created
/// - File I/O fails during backup
pub fn create_backup(
    graph_path: &Path,
    config: BackupConfig,
) -> NativeResult<BackupResult> {
    let start = SystemTime::now();
    let backup_id = config.backup_id.clone().unwrap_or_else(|| {
        format!(
            "backup_{}",
            start
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()
        )
    });

    // Ensure backup directory exists
    std::fs::create_dir_all(&config.backup_dir).map_err(|e| NativeBackendError::Io(e))?;

    // Step 1: Checkpoint before backup (if enabled)
    let checkpoint_performed = if config.checkpoint_before_backup {
        // Attempt to perform checkpoint before backup
        match perform_checkpoint(graph_path) {
            Ok(()) => true,
            Err(e) => {
                // Log warning but continue with backup
                // WAL may not exist or may be clean
                eprintln!(
                    "Warning: Checkpoint before backup failed: {:?}. Continuing with backup.",
                    e
                );
                false
            }
        }
    } else {
        false
    };

    // Step 2: Create snapshot export config
    let export_config = SnapshotExportConfig {
        export_path: config.backup_dir.clone(),
        snapshot_id: backup_id.clone(),
        include_statistics: config.include_statistics,
        min_stable_duration: Duration::from_secs(0),
        checksum_validation: config.validate_checksum,
    };

    // Step 3: Create exporter and export
    let mut exporter = SnapshotExporter::new(graph_path, export_config)?;
    let export_result = exporter.export_snapshot()?;

    // Step 4: Convert to BackupResult
    let duration = start
        .elapsed()
        .unwrap_or(Duration::from_secs(0))
        .as_secs_f64();

    Ok(BackupResult {
        snapshot_path: export_result.snapshot_path,
        manifest_path: export_result.manifest_path,
        size_bytes: export_result.snapshot_size_bytes,
        checksum: export_result.checksum,
        record_count: export_result.record_count,
        duration_secs: duration,
        timestamp: export_result.export_timestamp,
        checkpoint_performed,
    })
}

/// Perform checkpoint before backup
///
/// This helper function attempts to flush the WAL to the main database
/// file before creating a backup. This ensures the backup contains
/// all committed transactions.
fn perform_checkpoint(graph_path: &Path) -> NativeResult<()> {
    // Check if WAL file exists
    let wal_path = graph_path.with_extension("wal");
    if !wal_path.exists() {
        // No WAL file, nothing to checkpoint
        return Ok(());
    }

    // Create WAL manager and force checkpoint
    let mut wal_manager = V2WALManager::for_graph_file(graph_path)?;
    wal_manager.force_checkpoint()?;

    Ok(())
}

/// Quick backup with default configuration
///
/// Convenience function that creates a backup with sensible defaults:
/// - Checkpoint enabled
/// - Statistics included
/// - Checksum validation enabled
///
/// # Arguments
/// * `graph_path` - Path to the graph database file
/// * `backup_dir` - Directory where backup files will be stored
///
/// # Returns
/// Backup result containing file paths, checksum, and metadata
pub fn backup(graph_path: &Path, backup_dir: &Path) -> NativeResult<BackupResult> {
    create_backup(graph_path, BackupConfig::new(backup_dir))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_backup_config_default() {
        let config = BackupConfig::default();
        assert_eq!(config.backup_dir, PathBuf::from("backup"));
        assert!(config.backup_id.is_none());
        assert!(config.checkpoint_before_backup);
        assert!(config.include_statistics);
        assert!(config.validate_checksum);
    }

    #[test]
    fn test_backup_config_builder() {
        let config = BackupConfig::new("/tmp/backup")
            .with_backup_id("test_backup")
            .with_checkpoint(false)
            .with_statistics(false)
            .with_checksum_validation(false);

        assert_eq!(config.backup_dir, PathBuf::from("/tmp/backup"));
        assert_eq!(config.backup_id, Some("test_backup".to_string()));
        assert!(!config.checkpoint_before_backup);
        assert!(!config.include_statistics);
        assert!(!config.validate_checksum);
    }

    #[test]
    fn test_backup_config_with_backup_id() {
        let config = BackupConfig::default().with_backup_id("my_backup");
        assert_eq!(config.backup_id, Some("my_backup".to_string()));
    }

    #[test]
    fn test_backup_result_fields() {
        let result = BackupResult {
            snapshot_path: PathBuf::from("/backup/snapshot.v2"),
            manifest_path: PathBuf::from("/backup/manifest.json"),
            size_bytes: 1024,
            checksum: 12345,
            record_count: 100,
            duration_secs: 1.5,
            timestamp: 1234567890,
            checkpoint_performed: true,
        };

        assert_eq!(result.snapshot_path, PathBuf::from("/backup/snapshot.v2"));
        assert_eq!(result.size_bytes, 1024);
        assert!(result.checkpoint_performed);
    }
}
