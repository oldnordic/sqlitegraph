//! File management and lifecycle operations for GraphFile
//!
//! This module provides file-level management operations including file growth,
//! size validation, memory mapping management, and cleanup operations.

use crate::backend::native::{
    graph_file::buffers::ReadBuffer, graph_file::buffers::WriteBuffer,
    types::{NativeBackendError, NativeResult},
};

#[cfg(feature = "v2_experimental")]
use memmap2::{MmapMut, MmapOptions};
use std::io::{Seek, SeekFrom, Write};

/// File management operations utilities for GraphFile
pub struct FileManager;

impl FileManager {
    /// Validate file size against header information
    ///
    /// Performs validation to ensure the file size is consistent with
    /// the expected size based on header metadata and cluster information.
    pub fn validate_file_size(
        file_size: u64,
        persistent_header: &crate::backend::native::persistent_header::PersistentHeaderV2,
    ) -> NativeResult<()> {
        crate::backend::native::graph_file::validation::GraphFileValidator::validate_file_size(
            file_size,
            persistent_header,
        )
    }

    /// Grow file by specified number of bytes
    ///
    /// Increases the file size by the specified number of bytes using
    /// sparse file allocation when supported by the filesystem.
    pub fn grow_file(file: &mut std::fs::File, additional_bytes: u64) -> NativeResult<()> {
        if additional_bytes == 0 {
            return Ok(());
        }

        let current_size = file.metadata()?.len();
        let new_size = current_size + additional_bytes;
        file.set_len(new_size)?;
        file.flush()?;

        Ok(())
    }

    /// Flush pending writes and sync file to disk
    ///
    /// Performs a complete flush operation by flushing any pending
    /// write buffer operations and then syncing the file to disk.
    pub fn flush_complete(
        file: &mut std::fs::File,
        write_buffer: &mut WriteBuffer,
    ) -> NativeResult<()> {
        // Flush pending write buffer operations
        Self::flush_write_buffer(file, write_buffer)?;

        // Sync file to disk
        file.flush()?;
        Ok(())
    }

    /// Flush pending write buffer operations
    ///
    /// Commits all pending write buffer operations to disk
    /// in optimal order to minimize disk seeks.
    fn flush_write_buffer(
        file: &mut std::fs::File,
        write_buffer: &mut WriteBuffer,
    ) -> NativeResult<()> {
        let operations = write_buffer.flush();

        // Sort operations by offset for sequential disk access
        let mut sorted_ops: Vec<_> = operations.into_iter().collect();
        sorted_ops.sort_by_key(|(offset, _)| *offset);

        for (offset, data) in sorted_ops {
            file.seek(SeekFrom::Start(offset))?;
            file.write_all(&data)?;
        }

        file.flush()?;
        Ok(())
    }

    /// Invalidate read buffer to force fresh reads from disk
    ///
    /// Clears any cached read data to ensure subsequent reads
    /// will fetch fresh data from disk rather than from cache.
    pub fn invalidate_read_buffer(read_buffer: &mut ReadBuffer) {
        read_buffer.offset = 0;
        read_buffer.size = 0;
    }

    /// Ensure mmap region is at least the specified size
    ///
    /// Ensures the memory-mapped region covers at least the specified
    /// length, growing the file and remapping if necessary.
    #[cfg(feature = "v2_experimental")]
    pub fn mmap_ensure_size(
        file: &mut std::fs::File,
        file_path: &std::path::Path,
        len: u64,
        mmap: &mut Option<MmapMut>,
    ) -> NativeResult<()> {
        // CRITICAL: Prevent mmap recursion cycle
        thread_local! {
            static MMAP_DEPTH: std::cell::RefCell<u32> = const { std::cell::RefCell::new(0) };
        }
        MMAP_DEPTH.with(|d| {
            let mut depth = d.borrow_mut();
            *depth += 1;
            if *depth > 10 {
                return Err(NativeBackendError::CorruptNodeRecord {
                    node_id: -1,
                    reason: format!("mmap recursion depth exceeded: {}", *depth),
                });
            }
            Ok(())
        })?;

        let result = (|| {
            let current_size = file.metadata()?.len();
            if len > current_size {
                Self::grow_file(file, len - current_size)?;
            }

            // Use conservative mmap management
            Self::ensure_mmap_covers(file, file_path, len, mmap)?;

            Ok(())
        })();

        MMAP_DEPTH.with(|d| {
            *d.borrow_mut() -= 1;
        });

        result
    }

    /// Ensure memory mapping covers the specified range
    ///
    /// Ensures the memory-mapped region covers at least the specified
    /// length, remapping if necessary.
    #[cfg(feature = "v2_experimental")]
    fn ensure_mmap_covers(
        file: &mut std::fs::File,
        file_path: &std::path::Path,
        len: u64,
        mmap: &mut Option<MmapMut>,
    ) -> NativeResult<()> {
        let needs_remap = match mmap {
            None => true,
            Some(current_mmap) => len > current_mmap.len() as u64,
        };

        if needs_remap {
            // Remap with larger size
            let file_size = file.metadata()?.len();
            let required_size = len.max(file_size);

            *mmap = unsafe {
                Some(
                    MmapOptions::new()
                        .len(required_size as usize)
                        .map_mut(&file.try_clone()?)?,
                )
            };
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Seek, SeekFrom, Write};
    use tempfile::tempfile;

    #[test]
    fn test_grow_file() {
        let mut temp_file = tempfile().unwrap();

        // Initial file size should be 0
        let initial_size = temp_file.metadata().unwrap().len();
        assert_eq!(initial_size, 0);

        // Grow file by 1024 bytes
        FileManager::grow_file(&mut temp_file, 1024).unwrap();

        // File size should now be 1024
        let new_size = temp_file.metadata().unwrap().len();
        assert_eq!(new_size, 1024);
    }

    #[test]
    fn test_grow_file_zero_bytes() {
        let mut temp_file = tempfile().unwrap();

        // Write some initial data
        temp_file.write_all(b"test data").unwrap();
        let initial_size = temp_file.metadata().unwrap().len();

        // Growing by 0 bytes should not change file size
        FileManager::grow_file(&mut temp_file, 0).unwrap();

        let new_size = temp_file.metadata().unwrap().len();
        assert_eq!(new_size, initial_size);
    }

    #[test]
    fn test_flush_complete() {
        let mut temp_file = tempfile().unwrap();
        let mut write_buffer = WriteBuffer::new(10);

        // Add some operations to buffer (use offsets beyond HEADER_SIZE = 80)
        write_buffer.add(100, b"flush_test".to_vec());
        write_buffer.add(120, b"data".to_vec());

        // Flush completely
        FileManager::flush_complete(&mut temp_file, &mut write_buffer).unwrap();

        // Verify buffer is empty
        assert!(write_buffer.operations.is_empty());

        // Verify data was written by reading back
        let mut buffer1 = vec![0u8; 10];
        temp_file.seek(SeekFrom::Start(100)).unwrap();
        temp_file.read_exact(&mut buffer1).unwrap();
        assert_eq!(buffer1, b"flush_test");

        let mut buffer2 = vec![0u8; 4];
        temp_file.seek(SeekFrom::Start(120)).unwrap();
        temp_file.read_exact(&mut buffer2).unwrap();
        assert_eq!(buffer2, b"data");
    }

    #[test]
    fn test_invalidate_read_buffer() {
        let mut read_buffer = ReadBuffer::new();

        // Set some initial state
        read_buffer.offset = 1000;
        read_buffer.size = 512;

        // Invalidate buffer
        FileManager::invalidate_read_buffer(&mut read_buffer);

        // Buffer should be cleared
        assert_eq!(read_buffer.offset, 0);
        assert_eq!(read_buffer.size, 0);
    }

    #[test]
    fn test_validate_file_size() {
        use crate::backend::native::persistent_header::PersistentHeaderV2;

        let header = PersistentHeaderV2::new_v2();
        let file_size = 1024u64;

        // This should pass with a consistent file size
        let result = FileManager::validate_file_size(file_size, &header);
        assert!(result.is_ok());
    }

    #[cfg(feature = "v2_experimental")]
    #[test]
    fn test_mmap_ensure_size() {
        use std::path::PathBuf;

        let mut temp_file = tempfile().unwrap();
        let file_path = PathBuf::from("test_mmap");
        let mut mmap: Option<MmapMut> = None;

        // Ensure mmap covers 2048 bytes
        FileManager::mmap_ensure_size(&mut temp_file, &file_path, 2048, &mut mmap).unwrap();

        // Verify mmap was created and has the correct size
        assert!(mmap.is_some());
        let mmap_ref = mmap.as_ref().unwrap();
        assert!(mmap_ref.len() >= 2048);

        // Verify file was grown to at least 2048 bytes
        let file_size = temp_file.metadata().unwrap().len();
        assert!(file_size >= 2048);
    }
}
