//! Atomic migration execution for file format conversion
//!
//! Provides atomic file migration operations with automatic rollback on failure.
//! Uses atomic file operations to ensure data safety during migration.

use crate::backend::native::{
    graph_file::encoding::{decode_persistent_header, encode_persistent_header},
    types::{NativeBackendError, NativeResult},
    v2::snapshot::AtomicFileOperations,
};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Instant;

/// Migration error types
///
/// Specific errors that can occur during migration operations.
#[derive(Debug)]
pub enum MigrationError {
    /// Unsupported version for migration
    UnsupportedVersion { version: u32 },

    /// Backup creation failed
    BackupFailed { source: String },

    /// Migration operation failed
    MigrationFailed { source: String },

    /// Post-migration verification failed
    VerificationFailed { reason: String },
}

impl std::fmt::Display for MigrationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MigrationError::UnsupportedVersion { version } => {
                write!(f, "Unsupported version for migration: {}", version)
            }
            MigrationError::BackupFailed { source } => {
                write!(f, "Backup creation failed: {}", source)
            }
            MigrationError::MigrationFailed { source } => {
                write!(f, "Migration operation failed: {}", source)
            }
            MigrationError::VerificationFailed { reason } => {
                write!(f, "Migration verification failed: {}", reason)
            }
        }
    }
}

impl std::error::Error for MigrationError {}

/// Result of a successful migration operation
///
/// Contains information about the completed migration.
#[derive(Debug, Clone)]
pub struct MigrationResult {
    /// Source format version
    pub from_version: u32,
    /// Target format version
    pub to_version: u32,
    /// Path to backup file (retained until explicitly deleted)
    pub backup_path: std::path::PathBuf,
    /// Time taken to complete migration
    pub duration: std::time::Duration,
}

/// Migrate a graph database file to the current format version
///
/// Performs atomic migration from V2 to V3 format:
/// 1. Creates backup of original file
/// 2. Reads and converts header
/// 3. Writes migrated content to temporary file
/// 4. Atomically replaces original with temporary file
/// 5. Verifies migration success
/// 6. Retains backup for safety (caller can delete)
///
/// On any failure, automatically rolls back by restoring the backup.
///
/// # Arguments
///
/// * `path` - Path to the graph database file to migrate
///
/// # Returns
///
/// * `Ok(MigrationResult)` - Migration completed successfully
/// * `Err(NativeBackendError)` - Migration failed and was rolled back
///
/// # Errors
///
/// Returns error if:
/// - File version is not V2 (only V2->V3 migration is supported)
/// - Backup creation fails
/// - Header read/write fails
/// - Atomic rename fails
/// - Verification fails
pub fn migrate_file(path: &Path) -> NativeResult<MigrationResult> {
    let start = Instant::now();
    let backup_path = path.with_extension("bak");

    // Step 1: Read header to detect version
    let (version, file_content) = read_file_with_version(path)?;

    // Step 2: Check if migration is needed and supported
    if version == 3 {
        // Already at current version
        return Ok(MigrationResult {
            from_version: 3,
            to_version: 3,
            backup_path: std::path::PathBuf::new(), // No backup needed
            duration: start.elapsed(),
        });
    }

    if version != 2 {
        return Err(NativeBackendError::MigrationFailed(
            format!(
                "Unsupported migration from version {} to version 3. Only V2->V3 migration is supported.",
                version
            ),
        ));
    }

    // Step 3: Create backup using atomic file operations
    AtomicFileOperations::new()
        .atomic_copy_file(path, &backup_path)
        .map_err(|e| NativeBackendError::MigrationFailed(
            format!("Backup creation failed: {:?}", e),
        ))?;

    // Step 4: Perform migration
    let migrate_result = migrate_v2_to_v3_internal(path, &file_content);

    // Step 5: Rollback on failure, cleanup on success
    if migrate_result.is_err() {
        // Attempt rollback
        let _ = rollback_migration(&backup_path, path);
        // Remove backup after rollback attempt
        let _ = std::fs::remove_file(&backup_path);
        return Err(migrate_result.unwrap_err());
    }

    // Step 6: Verify migration
    match verify_migration(path, 3) {
        Ok(()) => {
            // Success - backup retained for safety
            Ok(MigrationResult {
                from_version: 2,
                to_version: 3,
                backup_path,
                duration: start.elapsed(),
            })
        }
        Err(e) => {
            // Verification failed - rollback
            let _ = rollback_migration(&backup_path, path);
            let _ = std::fs::remove_file(&backup_path);
            Err(NativeBackendError::MigrationFailed(
                format!("Verification failed: {:?}", e),
            ))
        }
    }
}

/// Read file and detect version
///
/// Reads the entire file content and extracts the format version from the header.
fn read_file_with_version(path: &Path) -> NativeResult<(u32, Vec<u8>)> {
    let mut file = File::open(path).map_err(|e| NativeBackendError::Io(e))?;

    // Read header first to get version
    let mut header = [0u8; 80];
    file.read_exact(&mut header)
        .map_err(|e| NativeBackendError::Io(e))?;

    // Extract version (offset 8-11)
    let version_bytes = [header[8], header[9], header[10], header[11]];
    let version = u32::from_be_bytes(version_bytes);

    // Read entire file
    file.seek(SeekFrom::Start(0))
        .map_err(|e| NativeBackendError::Io(e))?;
    let mut content = Vec::new();
    file.read_to_end(&mut content)
        .map_err(|e| NativeBackendError::Io(e))?;

    Ok((version, content))
}

/// Internal migration from V2 to V3 format
///
/// Converts the header from V2 format (8-byte schema_version) to V3 format
/// (4-byte schema_version + 4-byte reserved). The rest of the file remains unchanged.
fn migrate_v2_to_v3_internal(path: &Path, content: &[u8]) -> NativeResult<()> {
    // Decode header
    let header = decode_persistent_header(content)
        .map_err(|e| NativeBackendError::MigrationFailed(
            format!("Failed to decode header: {:?}", e),
        ))?;

    if header.version != 2 {
        return Err(NativeBackendError::MigrationFailed(
            format!("Expected V2 file, found V{}", header.version),
        ));
    }

    // Create new header with version 3
    let mut new_header = header.clone();
    new_header.version = 3;
    // schema_version and reserved are already correct from decode

    // Encode new header
    let encoded_header = encode_persistent_header(&new_header)
        .map_err(|e| NativeBackendError::MigrationFailed(
            format!("Failed to encode header: {:?}", e),
        ))?;

    // Create new content with updated header
    if encoded_header.len() != 80 {
        return Err(NativeBackendError::MigrationFailed(
            format!("Encoded header size is {} bytes, expected 80", encoded_header.len()),
        ));
    }

    let mut new_content = content.to_vec();

    // Replace first 80 bytes with new header
    new_content[0..80].copy_from_slice(&encoded_header);

    // Write to temporary file
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, &new_content)
        .map_err(|e| NativeBackendError::MigrationFailed(
            format!("Failed to write temp file: {:?}", e),
        ))?;

    // Sync temp file
    sync_file(&temp_path).map_err(|e| NativeBackendError::MigrationFailed(
        format!("Failed to sync temp file: {:?}", e),
    ))?;

    // Atomic rename
    std::fs::rename(&temp_path, path).map_err(|e| NativeBackendError::MigrationFailed(
        format!("Failed to rename temp file: {:?}", e),
    ))?;

    // Sync parent directory
    if let Some(parent) = path.parent() {
        sync_directory(parent).map_err(|e| NativeBackendError::MigrationFailed(
            format!("Failed to sync parent directory: {:?}", e),
        ))?;
    }

    Ok(())
}

/// Verify that migration was successful
///
/// Reopens the file and verifies the header is in the expected format.
fn verify_migration(path: &Path, expected_version: u32) -> NativeResult<()> {
    let content = std::fs::read(path).map_err(|e| NativeBackendError::MigrationFailed(
        format!("Failed to read migrated file: {:?}", e),
    ))?;

    let header = decode_persistent_header(&content).map_err(|e| {
        NativeBackendError::MigrationFailed(
            format!("Failed to decode migrated header: {:?}", e),
        )
    })?;

    if header.version != expected_version {
        return Err(NativeBackendError::MigrationFailed(
            format!(
                "Version mismatch after migration: expected {}, found {}",
                expected_version, header.version
            ),
        ));
    }

    Ok(())
}

/// Rollback a failed migration by restoring backup
///
/// Attempts to restore the backup file to the original location.
/// If rollback fails, logs a warning but doesn't panic.
fn rollback_migration(backup_path: &Path, original_path: &Path) -> NativeResult<()> {
    // Try to restore the backup
    if let Err(e) = AtomicFileOperations::new().atomic_copy_file(backup_path, original_path) {
        eprintln!(
            "Warning: Failed to restore backup during rollback: {:?}. \
             Original file may be in inconsistent state.",
            e
        );
        return Err(NativeBackendError::MigrationFailed(
            format!("Rollback failed: {:?}", e),
        ));
    }
    Ok(())
}

/// Sync file to disk
///
/// Opens the file and calls sync_all() to ensure data is persisted.
fn sync_file(path: &Path) -> NativeResult<()> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| NativeBackendError::Io(e))?;

    file.sync_all()
        .map_err(|e| NativeBackendError::IoError {
            context: format!("Failed to sync file: {:?}", path),
            source: e,
        })
}

/// Sync directory to disk
///
/// Opens the directory and syncs it to ensure file renames are persisted.
fn sync_directory(path: &Path) -> NativeResult<()> {
    match std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
    {
        Ok(dir) => {
            dir.sync_all().map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to sync directory: {:?}", path),
                source: e,
            })
        }
        Err(_) => {
            // Directory sync not supported on this filesystem
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::{
        constants::DEFAULT_FEATURE_FLAGS,
        persistent_header::PersistentHeaderV2,
        v2::{V2_MAGIC, V2_FORMAT_VERSION},
    };
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Helper to create a V2 format file
    fn create_v2_file() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();

        let header = PersistentHeaderV2 {
            magic: V2_MAGIC,
            version: 2,
            flags: DEFAULT_FEATURE_FLAGS,
            node_count: 10,
            edge_count: 50,
            schema_version: 1,
            reserved: 0,
            node_data_offset: 80,
            edge_data_offset: 80,
            outgoing_cluster_offset: 0,
            incoming_cluster_offset: 0,
            free_space_offset: 0,
        };

        let encoded = encode_persistent_header(&header).unwrap();
        file.as_file_mut().write_all(&encoded).unwrap();

        // Write some dummy data after header
        file.as_file_mut().write_all(b"DUMMY_DATA_AFTER_HEADER").unwrap();
        file.as_file_mut().flush().unwrap();
        file.as_file_mut().sync_all().unwrap();

        file
    }

    /// Helper to create a V3 format file
    fn create_v3_file() -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();

        let header = PersistentHeaderV2 {
            magic: V2_MAGIC,
            version: 3,
            flags: DEFAULT_FEATURE_FLAGS,
            node_count: 10,
            edge_count: 50,
            schema_version: 1,
            reserved: 0,
            node_data_offset: 80,
            edge_data_offset: 80,
            outgoing_cluster_offset: 0,
            incoming_cluster_offset: 0,
            free_space_offset: 0,
        };

        let encoded = encode_persistent_header(&header).unwrap();
        file.as_file_mut().write_all(&encoded).unwrap();

        // Write some dummy data after header
        file.as_file_mut().write_all(b"DUMMY_DATA_AFTER_HEADER").unwrap();
        file.as_file_mut().flush().unwrap();
        file.as_file_mut().sync_all().unwrap();

        file
    }

    #[test]
    fn test_migrate_v2_to_v3_success() {
        let v2_file = create_v2_file();
        let path = v2_file.path();

        // Verify initial version
        let (version, _) = read_file_with_version(path).unwrap();
        assert_eq!(version, 2);

        // Migrate
        let result = migrate_file(path).unwrap();

        assert_eq!(result.from_version, 2);
        assert_eq!(result.to_version, 3);
        assert!(result.backup_path.exists());

        // Verify migrated version
        let (new_version, content) = read_file_with_version(path).unwrap();
        assert_eq!(new_version, 3);

        // Verify data after header is preserved
        let data_after_header = &content[80..];
        assert_eq!(data_after_header, b"DUMMY_DATA_AFTER_HEADER");

        // Cleanup backup
        std::fs::remove_file(&result.backup_path).ok();
    }

    #[test]
    fn test_migrate_v3_no_op() {
        let v3_file = create_v3_file();
        let path = v3_file.path();

        // Migrate (should be no-op)
        let result = migrate_file(path).unwrap();

        assert_eq!(result.from_version, 3);
        assert_eq!(result.to_version, 3);
        assert!(!result.backup_path.exists()); // No backup for no-op
    }

    #[test]
    fn test_verify_migration_success() {
        let v3_file = create_v3_file();
        assert!(verify_migration(v3_file.path(), 3).is_ok());
    }

    #[test]
    fn test_verify_migration_version_mismatch() {
        let v2_file = create_v2_file();
        let result = verify_migration(v2_file.path(), 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_file_with_version_v2() {
        let v2_file = create_v2_file();
        let (version, content) = read_file_with_version(v2_file.path()).unwrap();
        assert_eq!(version, 2);
        assert!(content.len() > 80);
    }

    #[test]
    fn test_read_file_with_version_v3() {
        let v3_file = create_v3_file();
        let (version, content) = read_file_with_version(v3_file.path()).unwrap();
        assert_eq!(version, 3);
        assert!(content.len() > 80);
    }

    #[test]
    fn test_migrate_creates_backup() {
        let v2_file = create_v2_file();
        let original_content = std::fs::read(v2_file.path()).unwrap();

        let result = migrate_file(v2_file.path()).unwrap();

        // Backup should exist
        assert!(result.backup_path.exists());

        // Backup content should match original
        let backup_content = std::fs::read(&result.backup_path).unwrap();
        assert_eq!(backup_content, original_content);

        // Cleanup
        std::fs::remove_file(&result.backup_path).ok();
    }

    #[test]
    fn test_migration_preserves_data() {
        let v2_file = create_v2_file();

        // Get the current file content which is just the header + DUMMY_DATA_AFTER_HEADER
        let original_content = std::fs::read(v2_file.path()).unwrap();

        let result = migrate_file(v2_file.path()).unwrap();

        // Verify data after header is preserved
        let migrated_content = std::fs::read(v2_file.path()).unwrap();
        assert_eq!(&migrated_content[80..], &original_content[80..]);

        // Cleanup
        std::fs::remove_file(&result.backup_path).ok();
    }

    #[test]
    fn test_migration_error_display() {
        let err = MigrationError::UnsupportedVersion { version: 1 };
        assert!(format!("{}", err).contains("Unsupported version"));

        let err = MigrationError::BackupFailed {
            source: "test error".to_string(),
        };
        assert!(format!("{}", err).contains("Backup creation failed"));
    }
}
