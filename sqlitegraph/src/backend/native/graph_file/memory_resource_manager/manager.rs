//! Memory resource manager for coordinating buffers and memory mapping
//!
//! This module provides the main coordination logic for memory resources
//! including buffer management and I/O mode detection.

use crate::backend::native::{
    types::{NativeResult, NativeBackendError},
    graph_file::buffers::{ReadBuffer, WriteBuffer},
};
use super::types::{MemoryIOMode, MemoryManagementStatistics};

#[cfg(feature = "native-v2")]
use memmap2::MmapMut;

/// Memory resource manager for coordinating buffers and memory mapping
pub struct MemoryResourceManager<'a> {
    /// Read buffer for caching data with read-ahead optimization
    pub(crate) read_buffer: &'a mut ReadBuffer,
    /// Write buffer for batching small writes
    pub(crate) write_buffer: &'a mut WriteBuffer,
    /// Optional memory mapping for direct memory access
    #[cfg(feature = "native-v2")]
    pub(crate) mmap: &'a mut Option<MmapMut>,
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
        #[cfg(feature = "native-v2")] mmap: &'a mut Option<MmapMut>,
    ) -> Self {
        Self {
            read_buffer,
            write_buffer,
            #[cfg(feature = "native-v2")]
            mmap,
        }
    }

    /// Get the current I/O mode based on feature flags
    pub fn current_io_mode(&self) -> MemoryIOMode {
        #[cfg(all(feature = "native-v2", feature = "v2_io_exclusive_mmap"))]
        {
            if self.mmap.is_some() {
                return MemoryIOMode::MemoryMapped;
            }
        }

        #[cfg(all(feature = "native-v2", feature = "v2_io_exclusive_std"))]
        {
            return MemoryIOMode::ExclusiveStd;
        }

        MemoryIOMode::Standard
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

    // Private helper methods

    /// Check if memory mapping is enabled
    pub fn mmap_enabled(&self) -> bool {
        #[cfg(feature = "native-v2")]
        {
            self.mmap.is_some()
        }
        #[cfg(not(feature = "native-v2"))]
        {
            false
        }
    }

    /// Clear write buffer safely with debugging
    pub fn clear_write_buffer_safely(&mut self) {
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
    pub fn validate_header_region_protection(&self, offset: u64) -> NativeResult<()> {
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

    /// Flush write buffer to disk
    pub(crate) fn flush_write_buffer(&mut self, file: &mut std::fs::File) -> NativeResult<()> {
        use std::io::{Seek, SeekFrom, Write};

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