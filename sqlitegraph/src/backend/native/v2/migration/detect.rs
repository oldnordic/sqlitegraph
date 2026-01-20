//! Version detection for file format migration
//!
//! Provides functions to detect the format version of a graph database file
//! and determine whether migration is required.

use crate::backend::native::{
    constants::MAGIC_BYTES,
    types::{NativeBackendError, NativeResult},
};
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// File format version enum
///
/// Represents the different file format versions that SQLiteGraph has used.
/// V1 is unsupported, V2 can be migrated to V3, V3 is current.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatVersion {
    /// V1 format - unsupported, cannot be migrated
    V1,
    /// V2 format - has 8-byte schema_version field at offset 32-39
    /// Can be migrated to V3 (4-byte schema_version + 4-byte reserved)
    V2,
    /// V3 format - current version with 4-byte schema_version + 4-byte reserved
    V3,
    /// Unknown format version
    Unknown(u32),
}

impl FormatVersion {
    /// Check if this format version is supported
    ///
    /// V2 and V3 are supported. V1 and unknown versions are not.
    pub fn is_supported(&self) -> bool {
        matches!(self, FormatVersion::V2 | FormatVersion::V3)
    }

    /// Check if this format needs migration to current version
    ///
    /// V2 needs migration to V3. V3 is current.
    pub fn needs_migration_to_current(&self) -> bool {
        matches!(self, FormatVersion::V2)
    }

    /// Get the numeric version value
    pub fn as_u32(&self) -> u32 {
        match self {
            FormatVersion::V1 => 1,
            FormatVersion::V2 => 2,
            FormatVersion::V3 => 3,
            FormatVersion::Unknown(v) => *v,
        }
    }
}

/// Detect the format version of a graph database file
///
/// Reads the file header and extracts the format version.
/// Returns an error if:
/// - File doesn't exist or can't be opened
/// - File is too small to contain a valid header
/// - Magic bytes don't match
///
/// # Arguments
///
/// * `path` - Path to the graph database file
///
/// # Returns
///
/// * `Ok(FormatVersion)` - The detected format version
/// * `Err(NativeBackendError)` - Error reading file or invalid format
pub fn detect_format_version(path: &Path) -> NativeResult<FormatVersion> {
    // Open file
    let mut file = File::open(path).map_err(|e| NativeBackendError::Io(e))?;

    // Read header (first 80 bytes)
    let mut header = [0u8; 80];
    file.read_exact(&mut header)
        .map_err(|e| NativeBackendError::Io(e))?;

    // Verify magic bytes
    let magic = &header[0..8];
    if magic != MAGIC_BYTES {
        return Err(NativeBackendError::InvalidMagic {
            expected: u64::from_be_bytes(MAGIC_BYTES),
            found: u64::from_be_bytes(
                magic.try_into().unwrap_or([0u8; 8]),
            ),
        });
    }

    // Read version field (offset 8-11) as u32 big-endian
    let version_bytes = [header[8], header[9], header[10], header[11]];
    let version = u32::from_be_bytes(version_bytes);

    Ok(match version {
        1 => FormatVersion::V1,
        2 => FormatVersion::V2,
        3 => FormatVersion::V3,
        v => FormatVersion::Unknown(v),
    })
}

/// Check if a file needs migration to the current format version
///
/// Returns `true` if the file is in V2 format and needs migration to V3.
/// Returns `false` if the file is already in V3 format.
/// Returns an error for:
/// - V1 format (unsupported, cannot be migrated)
/// - Unknown format versions
/// - Read/detection errors
///
/// # Arguments
///
/// * `path` - Path to the graph database file
///
/// # Returns
///
/// * `Ok(true)` - File needs migration (V2 -> V3)
/// * `Ok(false)` - File is current version (V3)
/// * `Err(NativeBackendError)` - Unsupported version or detection error
pub fn needs_migration(path: &Path) -> NativeResult<bool> {
    let version = detect_format_version(path)?;

    match version {
        FormatVersion::V1 => Err(NativeBackendError::UnsupportedVersion {
            version: 1,
            supported_version: 3,
        }),
        FormatVersion::V2 => Ok(true), // V2 needs migration to V3
        FormatVersion::V3 => Ok(false), // V3 is current
        FormatVersion::Unknown(v) => Err(NativeBackendError::UnsupportedVersion {
            version: v,
            supported_version: 3,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::{
        constants::DEFAULT_FEATURE_FLAGS,
        v2::{V2_FORMAT_VERSION, V2_MAGIC},
    };
    use std::io::Write;
    use tempfile::NamedTempFile;

    /// Helper to create a graph file with specific version
    fn create_test_file(version: u32) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();

        // Write header (80 bytes)
        let mut header = [0u8; 80];

        // Magic bytes (0-7)
        header[0..8].copy_from_slice(&V2_MAGIC);

        // Version (8-11)
        header[8..12].copy_from_slice(&version.to_be_bytes());

        // Flags (12-15)
        header[12..16].copy_from_slice(&DEFAULT_FEATURE_FLAGS.to_be_bytes());

        // Rest of header is zero-filled for this test
        file.as_file_mut().write_all(&header).unwrap();
        file.as_file_mut().flush().unwrap();
        file.as_file_mut().sync_all().unwrap();

        file
    }

    #[test]
    fn test_detect_format_version_v2() {
        let file = create_test_file(2);
        let version = detect_format_version(file.path()).unwrap();
        assert_eq!(version, FormatVersion::V2);
        assert!(version.is_supported());
        assert!(version.needs_migration_to_current());
    }

    #[test]
    fn test_detect_format_version_v3() {
        let file = create_test_file(3);
        let version = detect_format_version(file.path()).unwrap();
        assert_eq!(version, FormatVersion::V3);
        assert!(version.is_supported());
        assert!(!version.needs_migration_to_current());
    }

    #[test]
    fn test_detect_format_version_v1_unsupported() {
        let file = create_test_file(1);
        let version = detect_format_version(file.path()).unwrap();
        assert_eq!(version, FormatVersion::V1);
        assert!(!version.is_supported());
        // V1 can't be migrated to current (would return error in needs_migration)
    }

    #[test]
    fn test_detect_format_version_unknown() {
        let file = create_test_file(99);
        let version = detect_format_version(file.path()).unwrap();
        assert_eq!(version, FormatVersion::Unknown(99));
        assert!(!version.is_supported());
    }

    #[test]
    fn test_needs_migration_v2_returns_true() {
        let file = create_test_file(2);
        let needs = needs_migration(file.path()).unwrap();
        assert!(needs);
    }

    #[test]
    fn test_needs_migration_v3_returns_false() {
        let file = create_test_file(3);
        let needs = needs_migration(file.path()).unwrap();
        assert!(!needs);
    }

    #[test]
    fn test_needs_migration_v1_returns_error() {
        let file = create_test_file(1);
        let result = needs_migration(file.path());
        assert!(result.is_err());
        match result.unwrap_err() {
            NativeBackendError::UnsupportedVersion { version, .. } => {
                assert_eq!(version, 1);
            }
            _ => panic!("Expected UnsupportedVersion error"),
        }
    }

    #[test]
    fn test_detect_format_version_invalid_magic() {
        let mut file = NamedTempFile::new().unwrap();

        // Write invalid magic (need 8 bytes)
        let mut header = [0u8; 80];
        header[0..7].copy_from_slice(b"INVALID");
        file.as_file_mut().write_all(&header).unwrap();

        let result = detect_format_version(file.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_format_version_missing_file() {
        let result = detect_format_version(Path::new("/nonexistent/file.db"));
        assert!(result.is_err());
    }

    #[test]
    fn test_format_version_as_u32() {
        assert_eq!(FormatVersion::V1.as_u32(), 1);
        assert_eq!(FormatVersion::V2.as_u32(), 2);
        assert_eq!(FormatVersion::V3.as_u32(), 3);
        assert_eq!(FormatVersion::Unknown(99).as_u32(), 99);
    }
}
