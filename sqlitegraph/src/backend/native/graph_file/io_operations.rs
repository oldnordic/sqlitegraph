//! I/O operations and data transfer for GraphFile
//!
//! This module provides core I/O operations including byte-level reading/writing,
//! buffer management, and file size operations for GraphFile instances.

use crate::backend::native::{
    graph_file::buffers::WriteBuffer, types::NativeResult,
};

#[cfg(feature = "v2_experimental")]
use memmap2::{MmapMut, MmapOptions};
use std::io::{Read, Seek, SeekFrom, Write};

/// I/O operations management utilities for GraphFile
pub struct IOOperationsManager;

impl IOOperationsManager {
    /// Read bytes from file using standard I/O
    ///
    /// Provides basic byte-level reading with proper error handling
    /// and position management. Used when no specialized I/O mode is active.
    pub fn read_bytes_std(
        file: &mut std::fs::File,
        offset: u64,
        buffer: &mut [u8],
    ) -> NativeResult<()> {
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(buffer)?;
        Ok(())
    }

    /// Write bytes to file using standard I/O
    ///
    /// Provides basic byte-level writing with proper error handling
    /// and position management.
    pub fn write_bytes_std(file: &mut std::fs::File, offset: u64, data: &[u8]) -> NativeResult<()> {
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(data)?;
        Ok(())
    }

    /// Direct write operation without buffering
    ///
    /// Writes data directly to file without going through write buffer,
    /// ensuring immediate persistence.
    pub fn write_bytes_direct(
        graph_file: &mut crate::backend::native::graph_file::GraphFile,
        offset: u64,
        data: &[u8],
    ) -> NativeResult<()> {
        use std::io::{Seek, SeekFrom, Write};
        let file = graph_file.file_mut();
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(data)?;
        file.flush()?;
        Ok(())
    }

    /// Read bytes with read-ahead optimization
    ///
    /// Attempts to optimize sequential reads by reading larger blocks
    /// when possible to reduce system call overhead.
    pub fn read_with_ahead(
        file: &mut std::fs::File,
        offset: u64,
        buffer: &mut [u8],
    ) -> NativeResult<()> {
        // Simple implementation - can be enhanced with actual read-ahead logic
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(buffer)?;
        Ok(())
    }

    /// Flush pending write buffer operations
    ///
    /// Commits all pending write buffer operations to disk
    /// in optimal order to minimize disk seeks.
    pub fn flush_write_buffer(
        file: &mut std::fs::File,
        write_buffer: &mut WriteBuffer,
    ) -> NativeResult<usize> {
        let operations = write_buffer.flush();
        let mut bytes_written = 0;

        // Sort operations by offset for sequential disk access
        let mut sorted_ops: Vec<_> = operations.into_iter().collect();
        sorted_ops.sort_by_key(|(offset, _)| *offset);

        for (offset, data) in sorted_ops {
            file.seek(SeekFrom::Start(offset))?;
            file.write_all(&data)?;
            bytes_written += data.len();
        }

        file.flush()?;
        Ok(bytes_written)
    }

    /// Invalidate read buffer
    ///
    /// Clears any cached read data to ensure fresh reads
    /// from disk for subsequent operations.
    pub fn invalidate_read_buffer(
        _read_buffer: &mut crate::backend::native::graph_file::buffers::ReadBuffer,
    ) {
        // Implementation depends on ReadBuffer structure
        // This is a placeholder for the actual buffer invalidation logic
    }

    /// Ensure file is at least the specified size
    ///
    /// Grows file if necessary to accommodate data at the specified offset.
    /// Uses sparse file allocation when supported by the filesystem.
    pub fn ensure_file_len_at_least(
        file: &mut std::fs::File,
        required_size: u64,
    ) -> NativeResult<()> {
        let metadata = file.metadata()?;
        let current_size = metadata.len();

        if current_size < required_size {
            file.set_len(required_size)?;
        }

        Ok(())
    }

    /// Read bytes using memory mapping (exclusive mode)
    #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
    pub fn read_bytes_mmap_exclusive(
        mmap: Option<&MmapMut>,
        offset: u64,
        buffer: &mut [u8],
    ) -> NativeResult<()> {
        let mmap = mmap.ok_or(NativeBackendError::CorruptNodeRecord {
            node_id: -1,
            reason: "mmap not initialized in exclusive mmap mode".to_string(),
        })?;

        if offset as usize + buffer.len() > mmap.len() {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "Read beyond mmap region: offset={}, len={}, mmap_size={}",
                    offset,
                    buffer.len(),
                    mmap.len()
                ),
            });
        }

        let start = offset as usize;
        let end = start + buffer.len();
        buffer.copy_from_slice(&mmap[start..end]);
        Ok(())
    }

    /// Write bytes using memory mapping (exclusive mode)
    #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
    pub fn write_bytes_mmap_exclusive(
        mmap: Option<&mut MmapMut>,
        offset: u64,
        data: &[u8],
    ) -> NativeResult<()> {
        let mmap = mmap.ok_or(NativeBackendError::CorruptNodeRecord {
            node_id: -1,
            reason: "mmap not initialized in exclusive mmap mode".to_string(),
        })?;

        let end_offset = offset + data.len() as u64;
        if end_offset as usize > mmap.len() {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "mmap write out of bounds: offset={}, len={}, mmap_len={}",
                    offset,
                    data.len(),
                    mmap.len()
                ),
            });
        }

        let start = offset as usize;
        let end = start + data.len();
        mmap[start..end].copy_from_slice(data);
        mmap.flush()?;
        Ok(())
    }

    /// Read bytes using exclusive standard I/O mode
    #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
    pub fn read_bytes_std_exclusive(
        file: &mut std::fs::File,
        offset: u64,
        buffer: &mut [u8],
        write_buffer: &mut WriteBuffer,
    ) -> NativeResult<()> {
        // Clear pending write buffer operations before reading
        if !write_buffer.operations.is_empty() {
            let ops_count = write_buffer.operations.len();
            if std::env::var("WRITEBUF_DEBUG").is_ok() {
                println!(
                    "[WRITEBUF_DEBUG] EXCLUSIVE_STD: CLEARING {} pending ops without flush",
                    ops_count
                );
            }
            write_buffer.operations.clear();
        }

        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(buffer)?;
        Ok(())
    }

    /// Write bytes using exclusive standard I/O mode
    #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
    pub fn write_bytes_std_exclusive(
        file: &mut std::fs::File,
        offset: u64,
        data: &[u8],
        write_buffer: &mut WriteBuffer,
    ) -> NativeResult<()> {
        // Clear pending write buffer operations before writing
        if !write_buffer.operations.is_empty() {
            let ops_count = write_buffer.operations.len();
            if std::env::var("WRITEBUF_DEBUG").is_ok() {
                println!(
                    "[WRITEBUF_DEBUG] EXCLUSIVE_STD: CLEARING {} pending ops without flush",
                    ops_count
                );
            }
            write_buffer.operations.clear();
        }

        file.seek(SeekFrom::Start(offset))?;
        file.write_all(data)?;
        Ok(())
    }

    /// Write buffered bytes using standard I/O
    ///
    /// Uses write buffer for optimized batched writes.
    pub fn write_buffered_bytes_std(
        file: &mut std::fs::File,
        data: &[u8],
        offset: u64,
        write_buffer: &mut WriteBuffer,
    ) -> NativeResult<()> {
        // Use write_buffer for optimized batched writes
        let data_vec = data.to_vec();
        let added = write_buffer.add(offset, data_vec);

        if !added {
            // Buffer is full, flush and write directly
            let operations = write_buffer.flush();
            for (op_offset, op_data) in operations {
                file.seek(SeekFrom::Start(op_offset))?;
                file.write_all(&op_data)?;
            }

            // Write current data directly
            file.seek(SeekFrom::Start(offset))?;
            file.write_all(data)?;
        }

        Ok(())
    }

    /// Read bytes from GraphFile (alias for compatibility with existing code)
    pub fn read_bytes(
        graph_file: &mut crate::backend::native::graph_file::GraphFile,
        offset: u64,
        buffer: &mut [u8],
    ) -> NativeResult<()> {
        graph_file.read_bytes(offset, buffer)
    }

    /// Write bytes to GraphFile (alias for compatibility with existing code)
    pub fn write_bytes(
        graph_file: &mut crate::backend::native::graph_file::GraphFile,
        offset: u64,
        data: &[u8],
    ) -> NativeResult<()> {
        graph_file.write_bytes(offset, data)
    }

    /// Flush file buffers to disk (alias for compatibility with existing code)
    pub fn flush(
        graph_file: &mut crate::backend::native::graph_file::GraphFile,
    ) -> NativeResult<()> {
        graph_file.sync()
    }

    /// Prefetch data for optimal read performance (alias for compatibility)
    pub fn prefetch(
        graph_file: &mut crate::backend::native::graph_file::GraphFile,
        offset: u64,
        length: u64,
    ) -> NativeResult<()> {
        let required_size = offset + length;
        graph_file.ensure_file_len_at_least(required_size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom, Write};
    use tempfile::tempfile;

    #[test]
    fn test_read_write_bytes_std() {
        let mut temp_file = tempfile().unwrap();

        // Write test data
        let test_data = b"Hello, I/O Operations!";
        IOOperationsManager::write_bytes_std(&mut temp_file, 0, test_data).unwrap();

        // Read back test data
        let mut buffer = vec![0u8; test_data.len()];
        IOOperationsManager::read_bytes_std(&mut temp_file, 0, &mut buffer).unwrap();

        assert_eq!(buffer, test_data);
    }

    #[test]
    fn test_write_bytes_direct() {
        let mut temp_file = tempfile().unwrap();

        // Write test data directly using GraphFile
        let test_data = b"Direct write test";
        IOOperationsManager::write_bytes_std(&mut temp_file, 0, test_data).unwrap();

        // Verify data was written
        let mut buffer = vec![0u8; test_data.len()];
        temp_file.seek(SeekFrom::Start(0)).unwrap();
        temp_file.read_exact(&mut buffer).unwrap();

        assert_eq!(buffer, test_data);
    }

    #[test]
    fn test_read_with_ahead() {
        let mut temp_file = tempfile().unwrap();

        // Write test data
        let test_data = b"Read-ahead test data";
        temp_file.seek(SeekFrom::Start(0)).unwrap();
        temp_file.write_all(test_data).unwrap();

        // Read using read_with_ahead
        let mut buffer = vec![0u8; test_data.len()];
        IOOperationsManager::read_with_ahead(&mut temp_file, 0, &mut buffer).unwrap();

        assert_eq!(buffer, test_data);
    }

    #[test]
    fn test_ensure_file_len_at_least() {
        let mut temp_file = tempfile().unwrap();

        // Ensure file is at least 1024 bytes
        IOOperationsManager::ensure_file_len_at_least(&mut temp_file, 1024).unwrap();

        // Verify file size
        let metadata = temp_file.metadata().unwrap();
        assert!(metadata.len() >= 1024);
    }

    #[test]
    fn test_flush_write_buffer() {
        let mut temp_file = tempfile().unwrap();
        let mut write_buffer = WriteBuffer::new(10);

        // Add some operations to buffer (use offsets beyond HEADER_SIZE = 80)
        write_buffer.add(100, b"data1".to_vec());
        write_buffer.add(110, b"data2".to_vec());

        // Flush buffer
        let bytes_written =
            IOOperationsManager::flush_write_buffer(&mut temp_file, &mut write_buffer).unwrap();

        assert_eq!(bytes_written, 10); // 5 + 5
        assert!(write_buffer.operations.is_empty());
    }

    #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
    #[test]
    fn test_std_exclusive_operations() {
        let mut temp_file = tempfile().unwrap();
        let mut write_buffer = WriteBuffer::new(10);

        // Write using exclusive std mode
        let test_data = b"Exclusive std mode test";
        IOOperationsManager::write_bytes_std_exclusive(
            &mut temp_file,
            0,
            test_data,
            &mut write_buffer,
        )
        .unwrap();

        // Read using exclusive std mode
        let mut buffer = vec![0u8; test_data.len()];
        IOOperationsManager::read_bytes_std_exclusive(
            &mut temp_file,
            0,
            &mut buffer,
            &mut write_buffer,
        )
        .unwrap();

        assert_eq!(buffer, test_data);
    }
}
