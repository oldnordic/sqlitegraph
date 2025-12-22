//! Core file operations for GraphFile
//!
//! This module provides fundamental file I/O operations including reading,
//! writing, file management, and I/O mode handling for GraphFile operations.

use crate::backend::native::{
    persistent_header::PersistentHeaderV2, types::NativeBackendError, types::NativeResult,
};

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

/// Core file operations utilities for GraphFile
pub struct FileOperations;

impl FileOperations {
    /// Create a new file with write permissions
    pub fn create_file<P: AsRef<Path>>(path: P) -> NativeResult<File> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(path)?;
        Ok(file)
    }

    /// Open an existing file with read-write permissions
    pub fn open_file<P: AsRef<Path>>(path: P) -> NativeResult<File> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        Ok(file)
    }

    /// Get file size from metadata
    pub fn file_size(file: &File) -> NativeResult<u64> {
        let metadata = file.metadata()?;
        Ok(metadata.len())
    }

    /// Read bytes directly from file without any buffering
    ///
    /// This is the fundamental read operation that bypasses all buffering
    /// and reads directly from the underlying file descriptor.
    pub fn read_bytes_direct(file: &mut File, offset: u64, buffer: &mut [u8]) -> NativeResult<()> {
        // CRITICAL: Validate file size before read_exact to prevent "failed to fill whole buffer"
        Self::ensure_file_len_at_least(file, offset, buffer.len())?;

        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(buffer)?;
        Ok(())
    }

    /// Write bytes directly to file without any buffering
    ///
    /// This is the fundamental write operation that bypasses all buffering
    /// and writes directly to the underlying file descriptor.
    pub fn write_bytes_direct(file: &mut File, offset: u64, data: &[u8]) -> NativeResult<()> {
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(data)?;
        Ok(())
    }

    /// Sync file contents to disk
    pub fn sync(file: &File) -> NativeResult<()> {
        file.sync_all()?;
        Ok(())
    }

    /// Ensure file is large enough for the requested read operation
    ///
    /// This prevents "failed to fill whole buffer" errors when reading
    /// beyond the end of the file.
    fn ensure_file_len_at_least(file: &File, offset: u64, len: usize) -> NativeResult<()> {
        let metadata = file.metadata()?;
        let file_len = metadata.len();

        if file_len < offset + len as u64 {
            return Err(NativeBackendError::FileTooSmall {
                size: file_len,
                min_size: offset + len as u64,
            });
        }
        Ok(())
    }

    /// Validate file size against header information
    pub fn validate_file_size(
        file_size: u64,
        persistent_header: &PersistentHeaderV2,
    ) -> NativeResult<()> {
        // Basic size validation
        if file_size == 0 {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: "Empty file detected".to_string(),
            });
        }

        // Use the validation module for comprehensive checks
        crate::backend::native::graph_file::validation::GraphFileValidator::validate_file_size(
            file_size,
            persistent_header,
        )
    }

    /// Read and validate file header
    pub fn read_and_validate_header(file: &mut File) -> NativeResult<PersistentHeaderV2> {
        // Read header bytes
        let mut header_bytes =
            vec![0u8; crate::backend::native::persistent_header::PERSISTENT_HEADER_SIZE];
        Self::read_bytes_direct(file, 0, &mut header_bytes)?;

        // Decode header
        crate::backend::native::graph_file::encoding::decode_persistent_header(&header_bytes)
    }

    /// Write header to file with validation
    pub fn write_header(file: &mut File, header: &PersistentHeaderV2) -> NativeResult<()> {
        // Encode header
        let header_bytes =
            crate::backend::native::graph_file::encoding::encode_persistent_header(header)?;

        // Write header bytes
        Self::write_bytes_direct(file, 0, &header_bytes)?;

        // Sync to disk
        Self::sync(file)?;

        Ok(())
    }

    /// Open disk file for debugging purposes
    pub fn open_disk_file_for_debug(file_path: &Path) -> NativeResult<File> {
        std::fs::File::open(file_path).map_err(|e| NativeBackendError::CorruptNodeRecord {
            node_id: -1,
            reason: format!("Failed to open disk file for debug: {}", e),
        })
    }

    /// Read node slot for debugging purposes
    pub fn read_node_slot_for_debug(
        disk_file: &mut File,
        node_slot_offset: u64,
    ) -> NativeResult<[u8; 32]> {
        let mut node_bytes = [0u8; 32];
        disk_file.seek(SeekFrom::Start(node_slot_offset))?;
        disk_file.read_exact(&mut node_bytes)?;
        Ok(node_bytes)
    }
}

/// File I/O mode configuration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum IOMode {
    /// Default mixed I/O mode with buffering
    Default,
    /// Exclusive standard I/O mode
    ExclusiveStd,
    /// Exclusive memory-mapped I/O mode
    ExclusiveMmap,
}

impl IOMode {
    /// Determine the current I/O mode based on feature flags
    pub fn current() -> Self {
        #[cfg(all(feature = "native-v2", feature = "v2_io_exclusive_mmap"))]
        return IOMode::ExclusiveMmap;

        #[cfg(all(feature = "native-v2", feature = "v2_io_exclusive_std"))]
        return IOMode::ExclusiveStd;

        #[cfg(not(any(
            feature = "native-v2",
            feature = "v2_io_exclusive_mmap",
            feature = "v2_io_exclusive_std"
        )))]
        return IOMode::Default;
    }

    /// Check if exclusive mmap mode is enabled
    pub fn is_exclusive_mmap(&self) -> bool {
        matches!(self, IOMode::ExclusiveMmap)
    }

    /// Check if exclusive std mode is enabled
    pub fn is_exclusive_std(&self) -> bool {
        matches!(self, IOMode::ExclusiveStd)
    }

    /// Check if default mode is enabled
    pub fn is_default(&self) -> bool {
        matches!(self, IOMode::Default)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempfile;

    #[test]
    fn test_file_size() {
        use std::io::Write;

        let mut temp_file = tempfile().unwrap();
        // Write some data
        temp_file.set_len(1024).unwrap();
        temp_file.flush().unwrap();

        let size = FileOperations::file_size(&temp_file).unwrap();
        assert_eq!(size, 1024);
    }

    #[test]
    fn test_read_write_bytes_direct() {
        let mut temp_file = tempfile().unwrap();

        // Write test data
        let test_data = b"Hello, World!";
        FileOperations::write_bytes_direct(&mut temp_file, 0, test_data).unwrap();

        // Read back test data
        let mut buffer = vec![0u8; test_data.len()];
        FileOperations::read_bytes_direct(&mut temp_file, 0, &mut buffer).unwrap();

        assert_eq!(buffer, test_data);
    }

    #[test]
    fn test_ensure_file_len_at_least_success() {
        let mut temp_file = tempfile().unwrap();

        // Set file size
        temp_file.set_len(100).unwrap();

        // Should succeed
        assert!(FileOperations::ensure_file_len_at_least(&temp_file, 50, 10).is_ok());
        assert!(FileOperations::ensure_file_len_at_least(&temp_file, 100, 0).is_ok());
    }

    #[test]
    fn test_ensure_file_len_at_least_failure() {
        let mut temp_file = tempfile().unwrap();

        // Set file size to 50 bytes
        temp_file.set_len(50).unwrap();

        // Should fail when requesting more data than available
        let result = FileOperations::ensure_file_len_at_least(&temp_file, 40, 20);
        assert!(result.is_err());
    }

    #[test]
    fn test_io_mode_detection() {
        let mode = IOMode::current();
        // The mode should be one of the defined variants
        assert!(matches!(
            mode,
            IOMode::Default | IOMode::ExclusiveStd | IOMode::ExclusiveMmap
        ));
    }

    #[test]
    fn test_validate_file_size_empty() {
        // Empty file should fail validation
        let header = PersistentHeaderV2::new_v2();
        let result = FileOperations::validate_file_size(0, &header);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_file_size_valid() {
        // Create a valid file with some data
        let mut header = PersistentHeaderV2::new_v2();
        header.node_data_offset = 1024;

        // Valid file should pass validation
        let result = FileOperations::validate_file_size(2048, &header);
        assert!(result.is_ok());
    }

    #[test]
    fn test_read_node_slot_for_debug() {
        use std::io::{Seek, SeekFrom, Write};

        let mut temp_file = tempfile().unwrap();

        // Write test pattern at node slot position
        let test_pattern = [0x42u8; 32];
        let slot_offset = 0x1000; // Typical node slot offset
        temp_file.seek(SeekFrom::Start(slot_offset)).unwrap();
        temp_file.write_all(&test_pattern).unwrap();
        temp_file.flush().unwrap();

        // Read the node slot back
        let read_pattern =
            FileOperations::read_node_slot_for_debug(&mut temp_file, slot_offset).unwrap();
        assert_eq!(read_pattern, test_pattern);
    }
}
