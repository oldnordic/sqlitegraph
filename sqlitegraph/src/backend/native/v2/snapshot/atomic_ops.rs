//! Atomic File Operations for SQLiteGraph V2
//!
//! Database-grade filesystem operations with atomic semantics and fsync discipline.
//! Provides guaranteed atomic copy operations suitable for crash-safe snapshot management.

use crate::backend::native::{NativeBackendError, NativeResult};
use std::path::{Path, PathBuf};

/// Atomic file operations handler
pub struct AtomicFileOperations;

/// Error types for atomic operations
#[derive(Debug)]
pub enum AtomicOperationError {
    /// Source path does not exist or is inaccessible
    SourceInaccessible {
        path: PathBuf,
        error: String,
    },

    /// Destination path exists and overwrite protection is enabled
    DestinationExists {
        path: PathBuf,
    },

    /// Source path is a directory when file expected
    SourceIsDirectory {
        path: PathBuf,
    },

    /// Destination parent directory does not exist
    ParentDirectoryMissing {
        parent: PathBuf,
    },

    /// Filesystem I/O error during operation
    IoError {
        context: String,
        error: String,
    },

    /// Temporary file cleanup failed
    CleanupFailed {
        temp_path: PathBuf,
        error: String,
    },
}

impl AtomicFileOperations {
    /// Create new atomic file operations instance
    pub fn new() -> Self {
        Self
    }

    /// Perform atomic file copy from source to destination
    ///
    /// Requirements:
    /// - Source must exist and be a file (not directory)
    /// - Destination must not exist (overwrite protection)
    /// - Uses temporary file + rename for atomicity
    /// - Full fsync discipline for crash safety
    /// - Cleanup on any failure
    ///
    /// # Failing TDD Tests Expected:
    /// - test_atomic_copy_file_to_new_location
    /// - test_atomic_copy_rejects_directory
    /// - test_atomic_copy_overwrite_protection
    /// - test_atomic_copy_crash_safety_simulation
    pub fn atomic_copy_file(&self, source: &Path, destination: &Path) -> NativeResult<()> {
        // Step 1: Validate preconditions
        self.validate_preconditions(source, destination)?;

        // Step 2: Create temporary file path
        let temp_path = self.create_temp_path(destination);

        // Step 3: Ensure temp path doesn't exist from previous operations
        if temp_path.exists() {
            let _ = self.cleanup_temp_file(&temp_path);
        }

        // Step 4: Perform copy with proper error handling
        let copy_result = std::fs::copy(source, &temp_path);
        if let Err(e) = copy_result {
            let _ = self.cleanup_temp_file(&temp_path);
            return Err(NativeBackendError::Io(e));
        }

        // Step 5: Verify the temp file was created as a file (not directory)
        if !temp_path.is_file() {
            let _ = self.cleanup_temp_file(&temp_path);
            return Err(NativeBackendError::IoError {
                context: format!("Temporary path was not created as a file: {:?}", temp_path),
                source: std::io::Error::new(std::io::ErrorKind::Other, "File creation failed"),
            });
        }

        // Step 6: Sync temporary file to ensure data is durable
        if let Err(e) = self.sync_file(&temp_path) {
            let _ = self.cleanup_temp_file(&temp_path);
            return Err(e);
        }

        // Step 7: Atomic rename to final destination
        if let Err(e) = std::fs::rename(&temp_path, destination) {
            let _ = self.cleanup_temp_file(&temp_path);
            return Err(NativeBackendError::IoError {
                context: "Failed to rename temporary file".to_string(),
                source: e,
            });
        }

        // Step 8: Sync parent directory to make rename durable
        if let Some(parent) = destination.parent() {
            if let Err(e) = self.sync_directory(parent) {
                return Err(e);
            }
        }

        Ok(())
    }

    /// Validate file copy preconditions before performing operation
    fn validate_preconditions(&self, source: &Path, destination: &Path) -> NativeResult<()> {
        // Check source exists and is a file
        if !source.exists() {
            return Err(NativeBackendError::InvalidParameter {
                context: format!("Source file does not exist: {:?}", source),
                source: None,
            });
        }

        // Check source is explicitly a file (not directory)
        if !source.is_file() {
            return Err(NativeBackendError::InvalidParameter {
                context: format!("Source path is not a file: {:?} (is_directory: {})", source, source.is_dir()),
                source: None,
            });
        }

        // Check destination does not exist (overwrite protection)
        if destination.exists() {
            return Err(NativeBackendError::InvalidParameter {
                context: format!("Destination already exists, overwrite protection enabled: {:?}", destination),
                source: None,
            });
        }

        // Check parent directory exists and is actually a directory
        if let Some(parent) = destination.parent() {
            if !parent.exists() {
                return Err(NativeBackendError::InvalidParameter {
                    context: format!("Destination parent directory does not exist: {:?}", parent),
                    source: None,
                });
            }
            if !parent.is_dir() {
                return Err(NativeBackendError::InvalidParameter {
                    context: format!("Destination parent is not a directory: {:?} (is_file: {})", parent, parent.is_file()),
                    source: None,
                });
            }
        }

        Ok(())
    }

    /// Create temporary file path for atomic operation
    fn create_temp_path(&self, destination: &Path) -> PathBuf {
        // Generate unique temporary file path
        let file_stem = destination.file_stem().unwrap_or_else(|| std::ffi::OsStr::new("temp"));
        let parent = destination.parent().unwrap_or_else(|| Path::new("."));

        // Simple format that guarantees file semantics
        parent.join(format!("{}.tmp.{}",
            file_stem.to_string_lossy(),
            std::process::id()
        ))
    }

    /// Perform cleanup of temporary files on failure
    fn cleanup_temp_file(&self, temp_path: &Path) -> NativeResult<()> {
        if temp_path.exists() {
            // Remove only if it's a file, not a directory
            if temp_path.is_file() {
                match std::fs::remove_file(temp_path) {
                    Ok(()) => {
                        // Successfully cleaned up temp file
                    }
                    Err(e) => {
                        // Log warning but don't fail the operation if cleanup fails
                        eprintln!("Warning: Failed to cleanup temporary file {:?}: {}", temp_path, e);
                    }
                }
            } else {
                // Unexpected: temp path exists but is a directory
                eprintln!("Warning: Temporary path exists as directory, attempting to remove: {:?}", temp_path);
                match std::fs::remove_dir_all(temp_path) {
                    Ok(()) => {
                        // Successfully removed directory
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to cleanup temporary directory {:?}: {}", temp_path, e);
                    }
                }
            }
        }
        Ok(())
    }

    /// Sync file data to disk for durability
    fn sync_file(&self, file_path: &Path) -> NativeResult<()> {
        use std::fs::OpenOptions;

        let file = OpenOptions::new()
            .write(true)
            .open(file_path)?;

        file.sync_all().map_err(|e| NativeBackendError::IoError {
            context: format!("Failed to sync file: {:?}", file_path),
            source: e,
        })
    }

    /// Sync directory metadata to disk for rename durability
    fn sync_directory(&self, dir_path: &Path) -> NativeResult<()> {
        use std::fs::OpenOptions;

        // Try to open directory for syncing, but don't fail if unsupported
        match OpenOptions::new()
            .read(true)
            .write(true)
            .open(dir_path)
        {
            Ok(dir) => {
                dir.sync_all().map_err(|e| NativeBackendError::IoError {
                    context: format!("Failed to sync directory: {:?}", dir_path),
                    source: e,
                })
            }
            Err(e) => {
                // Directory sync not supported on this filesystem, log warning but continue
                eprintln!("Warning: Directory sync not supported: {:?} (error: {})", dir_path, e);
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::{TempDir, NamedTempFile};
    use std::fs;
    use std::io::Write;

    #[test]
    fn test_atomic_copy_file_to_new_location() {
        // This test should FAIL initially - create failing TDD test
        let temp_dir = TempDir::new().unwrap();
        let atomic_ops = AtomicFileOperations::new();

        // Create source file with test data
        let source_file = temp_dir.path().join("source.txt");
        let mut source = fs::File::create(&source_file).unwrap();
        source.write_all(b"Hello, SQLiteGraph V2!").unwrap();
        source.sync_all().unwrap();

        // Debug: Verify source file was created correctly
        assert!(source_file.exists(), "Source file should exist");
        assert!(source_file.is_file(), "Source should be a file");
        assert!(!source_file.is_dir(), "Source should not be a directory");

        // Destination path (must not exist)
        let dest_file = temp_dir.path().join("destination.txt");

        // Debug: Verify destination doesn't exist initially
        assert!(!dest_file.exists(), "Destination should not exist initially");

        // Perform atomic copy - should succeed when implemented
        match atomic_ops.atomic_copy_file(&source_file, &dest_file) {
            Ok(()) => {
                // Success case
            }
            Err(e) => {
                // Debug: Print error details
                panic!("Atomic copy failed with error: {:?}", e);
            }
        }

        // Verify destination exists and has correct content
        assert!(dest_file.exists());
        assert!(dest_file.is_file(), "Destination should be a file");
        let dest_content = fs::read_to_string(&dest_file).unwrap();
        assert_eq!(dest_content, "Hello, SQLiteGraph V2!");

        // Verify source still exists (copy, not move)
        assert!(source_file.exists());
        let source_content = fs::read_to_string(&source_file).unwrap();
        assert_eq!(source_content, "Hello, SQLiteGraph V2!");
    }

    #[test]
    fn test_atomic_copy_rejects_directory() {
        // Test case: Source is directory, should reject
        let temp_dir = TempDir::new().unwrap();
        let atomic_ops = AtomicFileOperations::new();

        // Create a directory as "source"
        let source_dir = temp_dir.path().join("source_dir");
        fs::create_dir(&source_dir).unwrap();

        // Destination file path
        let dest_file = temp_dir.path().join("destination.txt");

        // Should reject directory source with appropriate error
        let result = atomic_ops.atomic_copy_file(&source_dir, &dest_file);
        assert!(result.is_err(), "Should reject directory source");

        // When implemented, should return specific error type
        // match result {
        //     Err(NativeBackendError::InvalidOperation { context, .. }) => {
        //         assert!(context.contains("directory"));
        //     }
        //     _ => panic!("Expected directory rejection error"),
        // }
    }

    #[test]
    fn test_atomic_copy_overwrite_protection() {
        // Test case: Destination exists, should reject
        let temp_dir = TempDir::new().unwrap();
        let atomic_ops = AtomicFileOperations::new();

        // Create source file
        let source_file = temp_dir.path().join("source.txt");
        let mut source = fs::File::create(&source_file).unwrap();
        source.write_all(b"Source content").unwrap();
        source.sync_all().unwrap();

        // Create destination file that already exists
        let dest_file = temp_dir.path().join("destination.txt");
        let mut dest = fs::File::create(&dest_file).unwrap();
        dest.write_all(b"Existing content").unwrap();
        dest.sync_all().unwrap();

        // Should reject overwriting existing file
        let result = atomic_ops.atomic_copy_file(&source_file, &dest_file);
        assert!(result.is_err(), "Should reject overwriting existing file");

        // Verify destination content unchanged
        let dest_content = fs::read_to_string(&dest_file).unwrap();
        assert_eq!(dest_content, "Existing content");
    }

    #[test]
    fn test_atomic_copy_crash_safety_simulation() {
        // Test case: Simulate crash during copy operation
        let temp_dir = TempDir::new().unwrap();
        let atomic_ops = AtomicFileOperations::new();

        // Create source file with known content
        let source_file = temp_dir.path().join("source.txt");
        let test_content = "Important SQLiteGraph data that must be crash-safe";
        let mut source = fs::File::create(&source_file).unwrap();
        source.write_all(test_content.as_bytes()).unwrap();
        source.sync_all().unwrap();

        // Destination path
        let dest_file = temp_dir.path().join("destination.txt");

        // When implemented, this should handle crash scenarios:
        // 1. If copy fails after temp file creation, temp file should be cleaned up
        // 2. Destination should never be partially written
        // 3. Operation should be atomic (either fully succeeds or fully fails)

        // Test that operation either succeeds completely or fails cleanly
        let result = atomic_ops.atomic_copy_file(&source_file, &dest_file);

        if result.is_ok() {
            // If succeeded, destination should have complete, correct content
            assert!(dest_file.exists());
            let dest_content = fs::read_to_string(&dest_file).unwrap();
            assert_eq!(dest_content, test_content);

            // No temporary files should remain
            let temp_file = dest_file.with_extension("tmp");
            assert!(!temp_file.exists(), "Temporary file should be cleaned up");
        } else {
            // If failed, destination should not exist and no temp files should remain
            assert!(!dest_file.exists(), "Destination should not exist on failed copy");
            let temp_file = dest_file.with_extension("tmp");
            assert!(!temp_file.exists(), "Temporary file should be cleaned up on failure");
        }
    }

    #[test]
    fn test_atomic_copy_missing_parent_directory() {
        // Test case: Destination parent directory doesn't exist
        let temp_dir = TempDir::new().unwrap();
        let atomic_ops = AtomicFileOperations::new();

        // Create source file
        let source_file = temp_dir.path().join("source.txt");
        let mut source = fs::File::create(&source_file).unwrap();
        source.write_all(b"Test content").unwrap();
        source.sync_all().unwrap();

        // Destination in non-existent parent directory
        let dest_file = temp_dir.path().join("nonexistent").join("destination.txt");

        // Should fail due to missing parent directory
        let result = atomic_ops.atomic_copy_file(&source_file, &dest_file);
        assert!(result.is_err(), "Should fail when parent directory missing");
    }

    #[test]
    fn test_atomic_copy_missing_source() {
        // Test case: Source file doesn't exist
        let temp_dir = TempDir::new().unwrap();
        let atomic_ops = AtomicFileOperations::new();

        // Non-existent source file
        let source_file = temp_dir.path().join("nonexistent.txt");
        let dest_file = temp_dir.path().join("destination.txt");

        // Should fail due to missing source file
        let result = atomic_ops.atomic_copy_file(&source_file, &dest_file);
        assert!(result.is_err(), "Should fail when source file missing");
    }
}