//! Memory resource manager for GraphFile
//!
//! This module provides centralized coordination of memory resources including
//! read buffers, write buffers, memory mapping, and I/O mode selection.
//! It handles complex memory management workflows and optimization strategies.

use crate::backend::native::{
    types::{NativeResult, NativeBackendError},
    graph_file::buffers::{ReadBuffer, WriteBuffer},
};

#[cfg(feature = "v2_experimental")]
use memmap2::MmapMut;

use std::io::{Read, Seek, SeekFrom};

/// Memory resource manager for coordinating buffers and memory mapping
pub struct MemoryResourceManager<'a> {
    /// Read buffer for caching data with read-ahead optimization
    read_buffer: &'a mut ReadBuffer,
    /// Write buffer for batching small writes
    write_buffer: &'a mut WriteBuffer,
    /// Optional memory mapping for direct memory access
    #[cfg(feature = "v2_experimental")]
    mmap: &'a mut Option<MmapMut>,
}

/// Memory management statistics and configuration
#[derive(Debug, Clone)]
pub struct MemoryManagementStatistics {
    /// Current read buffer capacity
    pub read_buffer_capacity: usize,
    /// Current write buffer pending operations
    pub write_buffer_pending_ops: usize,
    /// Whether memory mapping is enabled
    pub mmap_enabled: bool,
    /// Current I/O mode
    pub io_mode: MemoryIOMode,
}

/// Available memory I/O modes for memory management
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryIOMode {
    /// Standard I/O with adaptive buffering
    Standard,
    /// Memory-mapped I/O (experimental)
    #[cfg(feature = "v2_experimental")]
    MemoryMapped,
    /// Standard I/O without buffering (exclusive mode)
    #[cfg(feature = "v2_experimental")]
    ExclusiveStd,
}

impl<'a> MemoryResourceManager<'a> {
    /// Create a new memory resource manager
    ///
    /// # Arguments
    /// * `read_buffer` - Mutable reference to read buffer
    /// * `write_buffer` - Mutable reference to write buffer
    /// * `mmap` - Mutable reference to optional memory mapping
    pub fn new(
        read_buffer: &'a mut ReadBuffer,
        write_buffer: &'a mut WriteBuffer,
        #[cfg(feature = "v2_experimental")] mmap: &'a mut Option<MmapMut>,
    ) -> Self {
        Self {
            read_buffer,
            write_buffer,
            #[cfg(feature = "v2_experimental")]
            mmap,
        }
    }

    /// Get the current I/O mode based on feature flags
    pub fn current_io_mode(&self) -> MemoryIOMode {
        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
        {
            if self.mmap.is_some() {
                return MemoryIOMode::MemoryMapped;
            }
        }

        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
        {
            return MemoryIOMode::ExclusiveStd;
        }

        MemoryIOMode::Standard
    }

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
            #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
            MemoryIOMode::MemoryMapped => {
                self.read_from_mmap(offset, buffer)?;
            }
            #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
            MemoryIOMode::ExclusiveStd => {
                self.clear_write_buffer_safely()?;
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
            #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
            MemoryIOMode::MemoryMapped => {
                self.write_to_mmap(offset, data, file_size_fn)?;
            }
            #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
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

    /// Flush all pending memory operations
    ///
    /// # Arguments
    /// * `file` - Mutable reference to the underlying file
    pub fn flush_all_operations(&mut self, file: &mut std::fs::File) -> NativeResult<()> {
        self.flush_write_buffer(file)?;
        Ok(())
    }

    /// Get memory management statistics
    pub fn get_statistics(&self) -> MemoryManagementStatistics {
        MemoryManagementStatistics {
            read_buffer_capacity: self.read_buffer.capacity,
            write_buffer_pending_ops: self.write_buffer.operations.len(),
            mmap_enabled: self.mmap_enabled(),
            io_mode: self.current_io_mode(),
        }
    }

    /// Check if memory mapping is enabled
    fn mmap_enabled(&self) -> bool {
        #[cfg(feature = "v2_experimental")]
        {
            self.mmap.is_some()
        }
        #[cfg(not(feature = "v2_experimental"))]
        {
            false
        }
    }

    /// Optimize buffer configurations for current workload
    ///
    /// # Arguments
    /// * `pattern_hint` - Hint about the access pattern for optimization
    pub fn optimize_buffers(&mut self, pattern_hint: AccessPatternHint) {
        match pattern_hint {
            AccessPatternHint::Sequential => {
                // Optimize for sequential access - larger read buffer
                let optimal_capacity = ReadBuffer::adaptive_capacity(8192);
                if optimal_capacity != self.read_buffer.capacity {
                    *self.read_buffer = ReadBuffer::with_capacity(optimal_capacity);
                }
            }
            AccessPatternHint::Random => {
                // Optimize for random access - smaller buffer for better cache locality
                let optimal_capacity = ReadBuffer::adaptive_capacity(256);
                if optimal_capacity != self.read_buffer.capacity {
                    *self.read_buffer = ReadBuffer::with_capacity(optimal_capacity);
                }
            }
            AccessPatternHint::Mixed => {
                // Use default adaptive sizing
                let optimal_capacity = ReadBuffer::adaptive_capacity(512);
                if optimal_capacity != self.read_buffer.capacity {
                    *self.read_buffer = ReadBuffer::with_capacity(optimal_capacity);
                }
            }
        }
    }

    // Private helper methods

    /// Clear write buffer safely with debugging
    fn clear_write_buffer_safely(&mut self) {
        if !self.write_buffer.operations.is_empty() {
            let ops_count = self.write_buffer.operations.len();
            if std::env::var("WRITEBUF_DEBUG").is_ok() {
                println!(
                    "[MEMORY_MANAGER] Clearing {} pending write operations",
                    ops_count
                );
            }
            self.write_buffer.operations.clear();
        }
    }

    /// Validate header region protection
    fn validate_header_region_protection(&self, offset: u64) -> NativeResult<()> {
        const HEADER_SIZE: u64 = 1024; // Simplified constant
        if offset < HEADER_SIZE {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "Write attempted into protected header region: offset={}",
                    offset
                ),
            });
        }
        Ok(())
    }

    /// Read from memory-mapped region
    #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
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
    #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
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
    #[cfg(feature = "v2_experimental")]
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
        use crate::backend::native::graph_file::file_ops::FileOperations;
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
        use std::io::{Seek, Write};
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
        let is_node_slot = (offset >= 0x400) && ((offset - 0x400) % 4096 == 0) && (data.len() == 4096);

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
            buffer[..adjusted_read_size].copy_from_slice(&self.read_buffer.data[..adjusted_read_size]);
        }

        Ok(())
    }

    /// Flush write buffer to disk
    fn flush_write_buffer(&mut self, file: &mut std::fs::File) -> NativeResult<()> {
        use std::io::{Seek, Write};

        // Sort operations by offset for optimal write patterns
        self.write_buffer.operations.sort_by_key(|(offset, _data)| *offset);

        for (offset, data) in &self.write_buffer.operations {
            file.seek(SeekFrom::Start(*offset))?;
            file.write_all(data)?;
        }

        file.sync_all()?;
        self.write_buffer.operations.clear();
        Ok(())
    }
}

/// Hint for memory access pattern optimization
#[derive(Debug, Clone)]
pub enum AccessPatternHint {
    /// Sequential access pattern (large reads, streaming)
    Sequential,
    /// Random access pattern (small reads, scattered)
    Random,
    /// Mixed access pattern (unpredictable)
    Mixed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempfile;

    #[test]
    fn test_memory_manager_creation() {
        let mut read_buffer = ReadBuffer::new();
        let mut write_buffer = WriteBuffer::new(32);
        #[cfg(feature = "v2_experimental")]
        let mut mmap = None;

        let manager = MemoryResourceManager::new(
            &mut read_buffer,
            &mut write_buffer,
            #[cfg(feature = "v2_experimental")]
            &mut mmap,
        );

        let stats = manager.get_statistics();
        assert_eq!(stats.write_buffer_pending_ops, 0);
        assert_eq!(stats.io_mode, MemoryIOMode::Standard);
    }

    #[test]
    fn test_io_mode_detection() {
        let mut read_buffer = ReadBuffer::new();
        let mut write_buffer = WriteBuffer::new(32);
        #[cfg(feature = "v2_experimental")]
        let mut mmap = None;

        let manager = MemoryResourceManager::new(
            &mut read_buffer,
            &mut write_buffer,
            #[cfg(feature = "v2_experimental")]
            &mut mmap,
        );

        // Should detect standard mode by default
        assert_eq!(manager.current_io_mode(), MemoryIOMode::Standard);
    }

    #[test]
    fn test_buffer_optimization() {
        let mut read_buffer = ReadBuffer::new();
        let mut write_buffer = WriteBuffer::new(32);
        #[cfg(feature = "v2_experimental")]
        let mut mmap = None;

        let mut manager = MemoryResourceManager::new(
            &mut read_buffer,
            &mut write_buffer,
            #[cfg(feature = "v2_experimental")]
            &mut mmap,
        );

        // Test sequential optimization
        manager.optimize_buffers(AccessPatternHint::Sequential);
        let stats = manager.get_statistics();
        assert!(stats.read_buffer_capacity >= 4096); // Should be larger for sequential

        // Test random optimization
        manager.optimize_buffers(AccessPatternHint::Random);
        let stats = manager.get_statistics();
        assert!(stats.read_buffer_capacity <= 1024); // Should be smaller for random
    }

    #[test]
    fn test_header_region_protection() {
        let mut read_buffer = ReadBuffer::new();
        let mut write_buffer = WriteBuffer::new(32);
        #[cfg(feature = "v2_experimental")]
        let mut mmap = None;

        let mut manager = MemoryResourceManager::new(
            &mut read_buffer,
            &mut write_buffer,
            #[cfg(feature = "v2_experimental")]
            &mut mmap,
        );

        // Should reject writes to header region
        assert!(manager.validate_header_region_protection(500).is_err());
        assert!(manager.validate_header_region_protection(2048).is_ok());
    }

    #[test]
    fn test_write_buffer_management() {
        let mut read_buffer = ReadBuffer::new();
        let mut write_buffer = WriteBuffer::new(32);
        #[cfg(feature = "v2_experimental")]
        let mut mmap = None;

        let mut manager = MemoryResourceManager::new(
            &mut read_buffer,
            &mut write_buffer,
            #[cfg(feature = "v2_experimental")]
            &mut mmap,
        );

        // Test adding operations to write buffer
        let mut temp_file = tempfile().unwrap();
        assert!(manager.memory_aware_write(
            &mut temp_file,
            2048,
            b"test data",
            || Ok(4096)
        ).is_ok());

        // Should have buffered the small write
        let stats = manager.get_statistics();
        assert!(stats.write_buffer_pending_ops > 0);

        // Test flushing
        assert!(manager.flush_all_operations(&mut temp_file).is_ok());
        let stats = manager.get_statistics();
        assert_eq!(stats.write_buffer_pending_ops, 0);
    }

    #[test]
    fn test_node_slot_detection() {
        let mut read_buffer = ReadBuffer::new();
        let mut write_buffer = WriteBuffer::new(32);
        #[cfg(feature = "v2_experimental")]
        let mut mmap = None;

        let mut manager = MemoryResourceManager::new(
            &mut read_buffer,
            &mut write_buffer,
            #[cfg(feature = "v2_experimental")]
            &mut mmap,
        );

        let mut temp_file = tempfile().unwrap();

        // Node slot write (4096 bytes at 0x400 + n*4096) should not be buffered
        let node_slot_data = vec![0u8; 4096];
        assert!(manager.memory_aware_write(
            &mut temp_file,
            0x400, // First node slot
            &node_slot_data,
            || Ok(8192)
        ).is_ok());

        // Should have 0 pending operations since node slots are written directly
        let stats = manager.get_statistics();
        assert_eq!(stats.write_buffer_pending_ops, 0);
    }
}