//! File operations and header management for native backend.
//!
//! This module handles file creation, opening, header encoding/decoding,
//! and basic file I/O operations for the native graph database format.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use crate::backend::native::constants::*;
use crate::backend::native::types::*;

/// Graph file wrapper that manages file handle and header operations
pub struct GraphFile {
    file: File,
    header: FileHeader,
    file_path: std::path::PathBuf,
}

impl GraphFile {
    /// Create a new graph file with initial header
    pub fn create<P: AsRef<Path>>(path: P) -> NativeResult<Self> {
        let path = path.as_ref();
        let file_path = path.to_path_buf();

        // Create new file with appropriate permissions
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .mode(FILE_PERMISSIONS)
            .open(path)?;

        let mut graph_file = Self {
            file,
            header: FileHeader::new(),
            file_path,
        };

        // Write initial header
        graph_file.write_header()?;

        Ok(graph_file)
    }

    /// Open an existing graph file
    pub fn open<P: AsRef<Path>>(path: P) -> NativeResult<Self> {
        let path = path.as_ref();
        let file_path = path.to_path_buf();

        let file = OpenOptions::new().read(true).write(true).open(path)?;

        let mut graph_file = Self {
            file,
            header: FileHeader::new(), // Will be overwritten by read_header
            file_path,
        };

        // Read and validate existing header
        graph_file.read_header()?;
        graph_file.header.validate()?;

        Ok(graph_file)
    }

    /// Read header from file
    pub fn read_header(&mut self) -> NativeResult<()> {
        self.file.seek(SeekFrom::Start(0))?;

        let mut header_bytes = vec![0u8; HEADER_SIZE as usize];
        self.file.read_exact(&mut header_bytes)?;

        self.header = decode_header(&header_bytes)?;
        Ok(())
    }

    /// Write header to file
    fn write_header(&mut self) -> NativeResult<()> {
        self.header.update_checksum();
        let header_bytes = encode_header(&self.header)?;

        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&header_bytes)?;
        self.file.flush()?;

        Ok(())
    }

    /// Get the current header
    pub fn header(&self) -> &FileHeader {
        &self.header
    }

    /// Get mutable reference to header (must call write_header() to persist changes)
    pub fn header_mut(&mut self) -> &mut FileHeader {
        &mut self.header
    }

    /// Get file path
    pub fn path(&self) -> &std::path::Path {
        &self.file_path
    }

    /// Get file size
    pub fn file_size(&self) -> NativeResult<u64> {
        let metadata = self.file.metadata()?;
        Ok(metadata.len())
    }

    /// Validate file size against header information
    pub fn validate_file_size(&self) -> NativeResult<()> {
        let file_size = self.file_size()?;

        if file_size < HEADER_SIZE {
            return Err(NativeBackendError::FileTooSmall {
                size: file_size,
                min_size: HEADER_SIZE,
            });
        }

        // Basic sanity check: file should be at least large enough for declared records
        // For native backend, we only require file to be large enough for actual data written
        // edge_data_offset is a reservation for future edge data, not a current requirement
        let min_expected_size = if self.header.edge_count > 0 {
            // If edges exist, file must be large enough to contain them
            std::cmp::max(self.header.edge_data_offset, self.header.node_data_offset)
        } else {
            // If no edges exist, file only needs to be large enough for header and node data
            self.header.node_data_offset
        };

        if file_size < min_expected_size {
            return Err(NativeBackendError::FileTooSmall {
                size: file_size,
                min_size: min_expected_size,
            });
        }

        Ok(())
    }

    /// Grow file by specified number of bytes
    pub fn grow(&mut self, additional_bytes: u64) -> NativeResult<()> {
        if additional_bytes == 0 {
            return Ok(());
        }

        let current_size = self.file_size()?;
        self.file
            .seek(SeekFrom::Start(current_size + additional_bytes - 1))?;
        self.file.write_all(&[0])?;
        self.file.flush()?;

        Ok(())
    }

    /// Sync file to disk
    pub fn sync(&self) -> NativeResult<()> {
        self.file.sync_all()?;
        Ok(())
    }

    /// Read bytes from file at specific offset
    pub fn read_bytes(&mut self, offset: u64, buffer: &mut [u8]) -> NativeResult<()> {
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(buffer)?;
        Ok(())
    }

    /// Write bytes to file at specific offset
    pub fn write_bytes(&mut self, offset: u64, data: &[u8]) -> NativeResult<()> {
        self.file.seek(SeekFrom::Start(offset))?;
        self.file.write_all(data)?;
        Ok(())
    }

    /// Flush pending writes
    pub fn flush(&mut self) -> NativeResult<()> {
        self.file.flush()?;
        Ok(())
    }
}

impl Drop for GraphFile {
    fn drop(&mut self) {
        // Ensure header is written before closing
        let _ = self.write_header();
        let _ = self.sync();
    }
}

/// Encode FileHeader to byte array
pub fn encode_header(header: &FileHeader) -> NativeResult<Vec<u8>> {
    let mut buffer = Vec::with_capacity(HEADER_SIZE as usize);

    // Write magic bytes
    buffer.extend_from_slice(&header.magic);

    // Write version (big-endian)
    buffer.extend_from_slice(&header.version.to_be_bytes());

    // Write flags (big-endian)
    buffer.extend_from_slice(&header.flags.to_be_bytes());

    // Write node count (big-endian)
    buffer.extend_from_slice(&header.node_count.to_be_bytes());

    // Write edge count (big-endian)
    buffer.extend_from_slice(&header.edge_count.to_be_bytes());

    // Write schema version (big-endian)
    buffer.extend_from_slice(&header.schema_version.to_be_bytes());

    // Write node data offset (big-endian)
    buffer.extend_from_slice(&header.node_data_offset.to_be_bytes());

    // Write edge data offset (big-endian)
    buffer.extend_from_slice(&header.edge_data_offset.to_be_bytes());

    // Write checksum (big-endian)
    buffer.extend_from_slice(&header.checksum.to_be_bytes());

    // Ensure we have exactly the right size
    assert_eq!(
        buffer.len(),
        HEADER_SIZE as usize,
        "Header encoding size mismatch"
    );

    Ok(buffer)
}

/// Decode FileHeader from byte array
pub fn decode_header(bytes: &[u8]) -> NativeResult<FileHeader> {
    if bytes.len() < HEADER_SIZE as usize {
        return Err(NativeBackendError::FileTooSmall {
            size: bytes.len() as u64,
            min_size: HEADER_SIZE,
        });
    }

    let mut offset = 0;

    // Read magic bytes
    let mut magic = [0u8; 8];
    magic.copy_from_slice(&bytes[offset..offset + 8]);
    offset += 8;

    // Read version
    let version = u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]);
    offset += 4;

    // Read flags
    let flags = u32::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ]);
    offset += 4;

    // Read node count
    let node_count = u64::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ]);
    offset += 8;

    // Read edge count
    let edge_count = u64::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ]);
    offset += 8;

    // Read schema version
    let schema_version = u64::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ]);
    offset += 8;

    // Read node data offset
    let node_data_offset = u64::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ]);
    offset += 8;

    // Read edge data offset
    let edge_data_offset = u64::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ]);
    offset += 8;

    // Read checksum
    let checksum = u64::from_be_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ]);

    Ok(FileHeader {
        magic,
        version,
        flags,
        node_count,
        edge_count,
        schema_version,
        node_data_offset,
        edge_data_offset,
        checksum,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_encode_decode_roundtrip() {
        let mut header = FileHeader::new();
        header.node_count = 42;
        header.edge_count = 100;
        header.schema_version = 2;
        header.update_checksum();

        let encoded = encode_header(&header).unwrap();
        let decoded = decode_header(&encoded).unwrap();

        assert_eq!(header, decoded);
    }

    #[test]
    fn test_header_magic_validation() {
        let mut header = FileHeader::new();
        header.magic[0] = 0xFF; // Corrupt magic
        let encoded = encode_header(&header).unwrap();
        let decoded = decode_header(&encoded).unwrap();

        assert!(decoded.validate().is_err());
    }

    #[test]
    fn test_header_checksum_validation() {
        let mut header = FileHeader::new();
        header.node_count = 42;
        header.update_checksum();

        // Corrupt checksum
        header.checksum = header.checksum.wrapping_add(1);

        assert!(header.verify_checksum().is_err());
    }
}
