//! Memory-aware I/O operations
//!
//! This module provides the core read/write operations with intelligent
//! routing based on I/O mode and buffer state.

use super::manager::MemoryResourceManager;

#[cfg(any(feature = "v2_io_exclusive_mmap", feature = "v2_io_exclusive_std"))]
use super::types::MemoryIOMode;
use crate::backend::native::{
    graph_file::buffers::ReadBuffer,
    graph_file::file_ops::FileOperations,
    types::{NativeResult, NativeBackendError},
};
use std::io::{Read, Seek, SeekFrom};

#[cfg(feature = "native-v2")]
use memmap2::MmapMut;

impl<'a> MemoryResourceManager<'a> {
    /// Perform memory-aware read operation with appropriate routing
    ///
    /// # Arguments
    /// * `file` - Mutable reference to the underlying file
    /// * `offset` - File offset to read from
    /// * `buffer` - Buffer to store the read data
    /// * `file_size_fn` - Function to get current file size
    ///
    /// Routes read operations based on I/O mode and buffer state
    pub fn memory_aware_read<F>(
        &mut self,
        file: &mut std::fs::File,
        offset: u64,
        buffer: &mut [u8],
        file_size_fn: F,
    ) -> NativeResult<()>
    where
        F: FnOnce() -> NativeResult<u64>,
    {
        match self.current_io_mode() {
            #[cfg(all(feature = "native-v2", feature = "v2_io_exclusive_mmap"))]
            MemoryIOMode::MemoryMapped => {
                self.read_from_mmap(offset, buffer)?;
            }
            #[cfg(all(feature = "native-v2", feature = "v2_io_exclusive_std"))]
            MemoryIOMode::ExclusiveStd => {
                self.clear_write_buffer_safely();
                self.direct_read_with_sync(file, offset, buffer)?;
            }
            _ => {
                self.buffered_read(file, offset, buffer, file_size_fn)?;
            }
        }

        Ok(())
    }

    /// Perform memory-aware write operation with appropriate routing
    ///
    /// # Arguments
    /// * `file` - Mutable reference to the underlying file
    /// * `offset` - File offset to write to
    /// * `data` - Data to write
    /// * `file_size_fn` - Function to get current file size
    ///
    /// Routes write operations based on I/O mode and buffer considerations
    #[allow(unused_variables)]  // Allow warnings for feature-conditional parameters
    pub fn memory_aware_write<F>(
        &mut self,
        file: &mut std::fs::File,
        offset: u64,
        data: &[u8],
        file_size_fn: F,
    ) -> NativeResult<()>
    where
        F: FnOnce() -> NativeResult<u64>,
    {
        // Validate header region protection
        self.validate_header_region_protection(offset)?;

        match self.current_io_mode() {
            #[cfg(all(feature = "native-v2", feature = "v2_io_exclusive_mmap"))]
            MemoryIOMode::MemoryMapped => {
                self.write_to_mmap(offset, data, file_size_fn)?;
            }
            #[cfg(all(feature = "native-v2", feature = "v2_io_exclusive_std"))]
            MemoryIOMode::ExclusiveStd => {
                self.clear_write_buffer_safely();
                self.direct_write_with_sync(file, offset, data)?;
            }
            _ => {
                self.buffered_write(file, offset, data)?;
            }
        }

        Ok(())
    }

    /// Read from memory-mapped region
    #[cfg(all(feature = "native-v2", feature = "v2_io_exclusive_mmap"))]
    fn read_from_mmap(&self, offset: u64, buffer: &mut [u8]) -> NativeResult<()> {
        let mmap = self
            .mmap
            .as_ref()
            .ok_or(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: "Memory mapping not initialized".to_string(),
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

    /// Write to memory-mapped region
    #[cfg(all(feature = "native-v2", feature = "v2_io_exclusive_mmap"))]
    fn write_to_mmap<F>(&mut self, offset: u64, data: &[u8], file_size_fn: F) -> NativeResult<()>
    where
        F: FnOnce() -> NativeResult<u64>,
    {
        let end_offset = offset + data.len() as u64;
        self.ensure_mmap_covers(end_offset)?;

        let mmap = self
            .mmap
            .as_mut()
            .ok_or(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: "Memory mapping not initialized".to_string(),
            })?;

        if offset as usize + data.len() > mmap.len() {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "Write beyond mmap region: offset={}, len={}, mmap_len={}",
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

        // Ensure file is extended if needed
        if end_offset > file_size_fn()? {
            // File extension would need to be handled at a higher level
        }

        Ok(())
    }

    /// Ensure memory mapping covers the required range
    #[cfg(feature = "native-v2")]
    fn ensure_mmap_covers(&self, required_offset: u64) -> NativeResult<()> {
        if let Some(mmap) = &self.mmap {
            if required_offset as usize > mmap.len() {
                return Err(NativeBackendError::CorruptNodeRecord {
                    node_id: -1,
                    reason: format!(
                        "Memory mapping too small: required_offset={}, mmap_size={}",
                        required_offset,
                        mmap.len()
                    ),
                });
            }
        }
        Ok(())
    }

    /// Perform direct read with synchronization
    fn direct_read_with_sync(
        &mut self,
        file: &mut std::fs::File,
        offset: u64,
        buffer: &mut [u8],
    ) -> NativeResult<()> {
        FileOperations::read_bytes_direct(file, offset, buffer)?;
        file.sync_all()?;
        Ok(())
    }

    /// Perform direct write with synchronization
    fn direct_write_with_sync(
        &mut self,
        file: &mut std::fs::File,
        offset: u64,
        data: &[u8],
    ) -> NativeResult<()> {
        use std::io::{Seek, SeekFrom, Write};
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(data)?;
        file.sync_all()?;
        Ok(())
    }

    /// Perform buffered read with read-ahead optimization
    fn buffered_read<F>(
        &mut self,
        file: &mut std::fs::File,
        offset: u64,
        buffer: &mut [u8],
        file_size_fn: F,
    ) -> NativeResult<()>
    where
        F: FnOnce() -> NativeResult<u64>,
    {
        // Ensure write buffer coherence
        if !self.write_buffer.operations.is_empty() {
            self.flush_write_buffer(file)?;
            // Invalidate read buffer to force fresh data from disk
            self.read_buffer.offset = 0;
            self.read_buffer.size = 0;
        }

        // Try to satisfy from read buffer first
        if !self.read_buffer.read(offset, buffer) {
            // Buffer miss - read from file with read-ahead
            self.read_with_ahead(file, offset, buffer, file_size_fn)?;
        }

        Ok(())
    }

    /// Perform buffered write with write-behind optimization
    fn buffered_write(
        &mut self,
        file: &mut std::fs::File,
        offset: u64,
        data: &[u8],
    ) -> NativeResult<()> {
        // Special handling for node slots (must not be buffered)
        let is_node_slot =
            (offset >= 0x400) && ((offset - 0x400) % 4096 == 0) && (data.len() == 4096);

        // Try to buffer small writes (except node slots)
        if !is_node_slot && data.len() <= 256 && self.write_buffer.add(offset, data.to_vec()) {
            return Ok(());
        }

        // Buffer full or large write - flush and write directly
        self.flush_write_buffer(file)?;
        self.direct_write_with_sync(file, offset, data)?;

        Ok(())
    }

    /// Read with adaptive read-ahead optimization
    fn read_with_ahead<F>(
        &mut self,
        file: &mut std::fs::File,
        offset: u64,
        buffer: &mut [u8],
        file_size_fn: F,
    ) -> NativeResult<()>
    where
        F: FnOnce() -> NativeResult<u64>,
    {
        use crate::backend::native::types::NativeBackendError;
        // IO traits removed - Read, Seek, SeekFrom not needed for this function
        // std::io::{Read, Seek, SeekFrom} removed - unused

        // Use adaptive sizing to minimize I/O amplification
        let optimal_capacity = ReadBuffer::adaptive_capacity(buffer.len());

        // Resize buffer if needed
        if optimal_capacity != self.read_buffer.capacity {
            *self.read_buffer = ReadBuffer::with_capacity(optimal_capacity);
        }

        // Calculate read-ahead size
        let read_ahead_size = std::cmp::max(buffer.len(), optimal_capacity);

        // Validate file bounds
        let file_size = file_size_fn()?;
        let remaining_bytes = file_size.saturating_sub(offset);

        if remaining_bytes == 0 {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: "Attempted read beyond end of file".to_string(),
            });
        }

        // Adjust read size to not exceed file bounds
        let adjusted_read_size = std::cmp::min(read_ahead_size, remaining_bytes as usize);

        // Perform read-ahead
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(&mut self.read_buffer.data[..adjusted_read_size])?;

        // Update buffer metadata
        self.read_buffer.offset = offset;
        self.read_buffer.size = adjusted_read_size;

        // Satisfy original request from read-ahead buffer
        if buffer.len() <= adjusted_read_size {
            buffer.copy_from_slice(&self.read_buffer.data[..buffer.len()]);
        } else {
            // Original request exceeds what we could read-ahead
            buffer[..adjusted_read_size]
                .copy_from_slice(&self.read_buffer.data[..adjusted_read_size]);
        }

        Ok(())
    }
}
