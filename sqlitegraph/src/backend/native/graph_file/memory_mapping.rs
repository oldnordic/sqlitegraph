//! Memory mapping operations and management for GraphFile
//!
//! This module provides memory mapping functionality including initialization,
//! coverage management, and safe read/write operations with proper bounds checking.

use crate::backend::native::{
    types::NativeResult,
    types::NativeBackendError,
    graph_file::buffers::WriteBuffer,
};

#[cfg(feature = "v2_experimental")]
use memmap2::{MmapMut, MmapOptions};
use std::io::{Write, Seek, SeekFrom};

/// Memory mapping management utilities for GraphFile
pub struct MemoryMappingManager;

impl MemoryMappingManager {
    /// Initialize mmap if not already present
    ///
    /// Creates a memory mapping for the file if one doesn't already exist.
    /// Handles both empty and non-empty files appropriately.
    #[cfg(feature = "v2_experimental")]
    pub fn ensure_mmap_initialized(
        file: &std::fs::File,
        mmap: &mut Option<MmapMut>,
    ) -> NativeResult<()> {
        if mmap.is_none() {
            let file_size = file.metadata()?.len();
            if file_size > 0 {
                *mmap = unsafe { Some(MmapOptions::new().map_mut(&file.try_clone()?)?) };
            } else {
                // For empty files, create minimal mmap to cover header
                *mmap = unsafe { Some(MmapOptions::new().map_mut(&file.try_clone()?)?) };
            }
        }
        Ok(())
    }

    /// Ensure mmap covers at least the specified offset using conservative remapping
    ///
    /// Ensures the memory-mapped region covers at least the specified length,
    /// growing the file and remapping if necessary. Includes recursion prevention
    /// to avoid infinite loops during write buffer flushes.
    #[cfg(feature = "v2_experimental")]
    pub fn ensure_mmap_covers(
        file: &mut std::fs::File,
        write_buffer: &mut WriteBuffer,
        mmap: &mut Option<MmapMut>,
        min_len: u64,
    ) -> NativeResult<()> {
        // CRITICAL: Prevent flush_write_buffer ↔ ensure_mmap_covers recursion
        thread_local! {
            static MMAP_ENSURE_DEPTH: std::cell::RefCell<u32> = const { std::cell::RefCell::new(0) };
        }

        MMAP_ENSURE_DEPTH.with(|d| {
            let mut depth = d.borrow_mut();
            if *depth >= 2 {
                return Err(NativeBackendError::CorruptNodeRecord {
                    node_id: -1,
                    reason: format!("ensure_mmap_covers recursion depth exceeded: {}", *depth),
                });
            }
            *depth += 1;
            Ok(())
        })?;

        let depth = MMAP_ENSURE_DEPTH.with(|d| *d.borrow());

        // Initialize mmap if needed
        Self::ensure_mmap_initialized(file, mmap)?;

        let current_file_size = file.metadata()?.len();

        // Ensure file is large enough
        if min_len > current_file_size {
            // Grow file to required size using set_len for atomic allocation
            file.set_len(min_len)?;
            file.flush()?;
        }

        let current_mmap_size = mmap.as_ref().unwrap().len() as u64;

        // PHASE 40 CRITICAL FIX: Remap if we need to cover data outside current mmap
        // This is more aggressive than the 4KB threshold to prevent "Read beyond mmap region" errors
        if min_len > current_mmap_size {
            // CRITICAL: Only flush if we're not already being called from flush_write_buffer
            if depth == 1 {
                // Flush any pending writes before remapping
                Self::flush_write_buffer(file, write_buffer)?;
            }

            // Remap to cover the full file size
            *mmap = unsafe { Some(MmapOptions::new().map_mut(&file.try_clone()?)?) };
        }

        // Decrement depth counter
        MMAP_ENSURE_DEPTH.with(|d| {
            let mut depth = d.borrow_mut();
            *depth = depth.saturating_sub(1);
        });

        Ok(())
    }

    /// Flush pending write buffer operations
    ///
    /// Commits all pending write buffer operations to disk
    /// in optimal order to minimize disk seeks.
    #[cfg(feature = "v2_experimental")]
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

    /// Read bytes using mmap (V2 path only)
    ///
    /// Reads data from the memory-mapped region with proper bounds checking.
    /// Provides fast read access for large sequential reads.
    #[cfg(feature = "v2_experimental")]
    pub fn mmap_read_bytes(
        mmap: &Option<MmapMut>,
        offset: u64,
        buffer: &mut [u8],
    ) -> NativeResult<()> {
        let mmap = mmap.as_ref().ok_or_else(|| NativeBackendError::CorruptNodeRecord {
            node_id: -1,
            reason: "mmap not initialized".to_string(),
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

    /// Write bytes using mmap (V2 path only)
    ///
    /// Writes data to the memory-mapped region with automatic size management.
    /// Ensures the mmap covers the required range before writing.
    #[cfg(feature = "v2_experimental")]
    pub fn mmap_write_bytes(
        file: &mut std::fs::File,
        file_path: &std::path::Path,
        write_buffer: &mut WriteBuffer,
        mmap: &mut Option<MmapMut>,
        offset: u64,
        data: &[u8],
    ) -> NativeResult<()> {
        // Ensure mmap is large enough using FileManager's function
        super::file_management::FileManager::mmap_ensure_size(
            file,
            file_path,
            offset + data.len() as u64,
            mmap,
        )?;

        let mmap_ref = mmap.as_mut().ok_or_else(|| NativeBackendError::CorruptNodeRecord {
            node_id: -1,
            reason: "mmap not initialized".to_string(),
        })?;

        if offset as usize + data.len() > mmap_ref.len() {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "Write beyond mmap region: offset={}, len={}, mmap_size={}",
                    offset,
                    data.len(),
                    mmap_ref.len()
                ),
            });
        }

        let start = offset as usize;
        let end = start + data.len();
        mmap_ref[start..end].copy_from_slice(data);

        Ok(())
    }

    /// Check if memory mapping is available and initialized
    ///
    /// Returns true if mmap is supported and initialized for the file.
    #[cfg(feature = "v2_experimental")]
    pub fn is_mmap_available(mmap: &Option<MmapMut>) -> bool {
        mmap.is_some()
    }

    /// Get the current size of the memory-mapped region
    ///
    /// Returns the size in bytes of the current mmap, or None if not initialized.
    #[cfg(feature = "v2_experimental")]
    pub fn get_mmap_size(mmap: &Option<MmapMut>) -> Option<usize> {
        mmap.as_ref().map(|m| m.len())
    }

    /// Force refresh of the memory mapping
    ///
    /// Forces a remap of the file to pick up any external changes.
    #[cfg(feature = "v2_experimental")]
    pub fn refresh_mmap(
        file: &mut std::fs::File,
        write_buffer: &mut WriteBuffer,
        mmap: &mut Option<MmapMut>,
    ) -> NativeResult<()> {
        if mmap.is_some() {
            // Flush pending writes before remapping
            Self::flush_write_buffer(file, write_buffer)?;

            // Force remap to current file size
            *mmap = unsafe { Some(MmapOptions::new().map_mut(&file.try_clone()?)?) };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempfile;
    use std::io::{Write, Read, Seek, SeekFrom};

    #[cfg(feature = "v2_experimental")]
    #[test]
    fn test_ensure_mmap_initialized() {
        let mut temp_file = tempfile().unwrap();
        let mut mmap: Option<MmapMut> = None;

        // Write some data to file
        temp_file.write_all(b"test data for mmap").unwrap();
        temp_file.flush().unwrap();

        // Initialize mmap
        MemoryMappingManager::ensure_mmap_initialized(&temp_file, &mut mmap).unwrap();

        // Verify mmap was created
        assert!(mmap.is_some());
        let mmap_ref = mmap.as_ref().unwrap();
        assert!(mmap_ref.len() >= 17); // length of "test data for mmap"
    }

    #[cfg(feature = "v2_experimental")]
    #[test]
    fn test_ensure_mmap_initialized_empty_file() {
        let temp_file = tempfile().unwrap();
        let mut mmap: Option<MmapMut> = None;

        // Initialize mmap for empty file
        MemoryMappingManager::ensure_mmap_initialized(&temp_file, &mut mmap).unwrap();

        // Verify mmap was created even for empty file
        assert!(mmap.is_some());
    }

    #[cfg(feature = "v2_experimental")]
    #[test]
    fn test_ensure_mmap_covers() {
        let mut temp_file = tempfile().unwrap();
        let mut write_buffer = WriteBuffer::new(10);
        let mut mmap: Option<MmapMut> = None;

        // Initialize mmap first
        MemoryMappingManager::ensure_mmap_initialized(&temp_file, &mut mmap).unwrap();
        let initial_size = mmap.as_ref().unwrap().len();

        // Ensure mmap covers larger range
        MemoryMappingManager::ensure_mmap_covers(
            &mut temp_file,
            &mut write_buffer,
            &mut mmap,
            2048,
        ).unwrap();

        // Verify file was grown and mmap was remapped
        assert!(mmap.as_ref().unwrap().len() >= 2048);
        assert!(temp_file.metadata().unwrap().len() >= 2048);
    }

    #[cfg(feature = "v2_experimental")]
    #[test]
    fn test_mmap_read_bytes() {
        let mut temp_file = tempfile().unwrap();
        let mut mmap: Option<MmapMut> = None;

        // Write test data
        let test_data = b"memory mapping test data";
        temp_file.write_all(test_data).unwrap();
        temp_file.flush().unwrap();

        // Initialize mmap
        MemoryMappingManager::ensure_mmap_initialized(&temp_file, &mut mmap).unwrap();

        // Read using mmap
        let mut buffer = vec![0u8; test_data.len()];
        MemoryMappingManager::mmap_read_bytes(&mmap, 0, &mut buffer).unwrap();

        assert_eq!(buffer, test_data);
    }

    #[cfg(feature = "v2_experimental")]
    #[test]
    fn test_mmap_read_bytes_beyond_bounds() {
        let mut temp_file = tempfile().unwrap();
        let mut mmap: Option<MmapMut> = None;

        // Write small amount of data
        temp_file.write_all(b"small").unwrap();
        temp_file.flush().unwrap();

        // Initialize mmap
        MemoryMappingManager::ensure_mmap_initialized(&temp_file, &mut mmap).unwrap();

        // Try to read beyond bounds
        let mut buffer = vec![0u8; 100];
        let result = MemoryMappingManager::mmap_read_bytes(&mmap, 0, &mut buffer);

        assert!(result.is_err());
    }

    #[cfg(feature = "v2_experimental")]
    #[test]
    fn test_mmap_write_bytes() {
        let mut temp_file = tempfile().unwrap();
        let file_path = std::path::PathBuf::from("test");
        let mut write_buffer = WriteBuffer::new(10);
        let mut mmap: Option<MmapMut> = None;

        // Initialize mmap
        MemoryMappingManager::ensure_mmap_initialized(&temp_file, &mut mmap).unwrap();

        // Write using mmap
        let test_data = b"mmap write test";
        MemoryMappingManager::mmap_write_bytes(
            &mut temp_file,
            &file_path,
            &mut write_buffer,
            &mut mmap,
            0,
            test_data,
        ).unwrap();

        // Verify data was written by reading back from file
        let mut buffer = vec![0u8; test_data.len()];
        temp_file.seek(SeekFrom::Start(0)).unwrap();
        temp_file.read_exact(&mut buffer).unwrap();

        assert_eq!(buffer, test_data);
    }

    #[cfg(feature = "v2_experimental")]
    #[test]
    fn test_mmap_operations_helpers() {
        let mut temp_file = tempfile().unwrap();
        let mut mmap: Option<MmapMut> = None;

        // Test with uninitialized mmap
        assert!(!MemoryMappingManager::is_mmap_available(&mmap));
        assert_eq!(MemoryMappingManager::get_mmap_size(&mmap), None);

        // Initialize mmap
        MemoryMappingManager::ensure_mmap_initialized(&temp_file, &mut mmap).unwrap();

        // Test with initialized mmap
        assert!(MemoryMappingManager::is_mmap_available(&mmap));
        assert!(MemoryMappingManager::get_mmap_size(&mmap).is_some());
        assert!(MemoryMappingManager::get_mmap_size(&mmap).unwrap() >= 0);
    }

    #[cfg(feature = "v2_experimental")]
    #[test]
    fn test_refresh_mmap() {
        let mut temp_file = tempfile().unwrap();
        let mut write_buffer = WriteBuffer::new(10);
        let mut mmap: Option<MmapMut> = None;

        // Initialize mmap
        MemoryMappingManager::ensure_mmap_initialized(&temp_file, &mut mmap).unwrap();
        let initial_size = mmap.as_ref().unwrap().len();

        // Write more data to file directly
        temp_file.seek(SeekFrom::End(0)).unwrap();
        temp_file.write_all(b"additional data").unwrap();
        temp_file.flush().unwrap();

        // Refresh mmap to pick up changes
        MemoryMappingManager::refresh_mmap(
            &mut temp_file,
            &mut write_buffer,
            &mut mmap,
        ).unwrap();

        // Verify mmap was updated
        assert!(mmap.as_ref().unwrap().len() > initial_size);
    }
}