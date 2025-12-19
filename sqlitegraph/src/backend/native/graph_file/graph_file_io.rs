//! GraphFile I/O operations
//!
//! This module contains file I/O operations including reading, writing,
//! file growth, synchronization, and basic file operations.

use crate::backend::native::graph_file::{
    FileManager,
    IOOperationsManager,
};

use super::GraphFile;

impl GraphFile {
    /// Get file path (alias for file_path)
    pub fn path(&self) -> &std::path::Path {
        &self.file_path
    }

    /// Validate file size against header information
    pub fn validate_file_size(&self) -> crate::backend::native::types::NativeResult<()> {
        let file_size = self.file_size()?;
        FileManager::validate_file_size(file_size, &self.persistent_header)
    }

    /// Grow file by specified number of bytes
    pub fn grow(&mut self, additional_bytes: u64) -> crate::backend::native::types::NativeResult<()> {
        FileManager::grow_file(&mut self.file, additional_bytes)
    }

    
    /// Read bytes from file at specific offset with managed memory resources (enhanced version)
    pub fn read_bytes_managed(&mut self, offset: u64, buffer: &mut [u8]) -> crate::backend::native::types::NativeResult<()> {
        IOOperationsManager::read_bytes(
            self,
            offset,
            buffer,
        )
    }

    /// Write bytes to file at specific offset with managed memory resources (enhanced version)
    pub fn write_bytes_managed(&mut self, offset: u64, data: &[u8]) -> crate::backend::native::types::NativeResult<()> {
        IOOperationsManager::write_bytes(
            self,
            offset,
            data,
        )
    }

    /// Write bytes directly to file, bypassing write buffer
    pub fn write_bytes_direct(&mut self, offset: u64, data: &[u8]) -> crate::backend::native::types::NativeResult<()> {
        IOOperationsManager::write_bytes_direct(
            self,
            offset,
            data,
        )
    }

    /// Flush write buffer to disk
    pub fn flush(&mut self) -> crate::backend::native::types::NativeResult<()> {
        IOOperationsManager::flush(self)
    }

    /// Invalidate read buffer
    pub fn invalidate_read_buffer(&mut self) {
        IOOperationsManager::invalidate_read_buffer(&mut self.read_buffer)
    }

    /// Get current memory usage statistics
    pub fn get_memory_usage(&self) -> crate::backend::native::graph_file::memory_resource_manager::MemoryManagementStatistics {
        // This would typically be delegated to a memory manager
        // For now, return basic statistics using correct field names
        crate::backend::native::graph_file::memory_resource_manager::MemoryManagementStatistics {
            read_buffer_capacity: self.read_buffer.len(),
            write_buffer_pending_ops: self.write_buffer.len(),
            mmap_enabled: false, // Would be true if mmap is used
            io_mode: crate::backend::native::graph_file::memory_resource_manager::MemoryIOMode::Standard,
        }
    }

    /// Prefetch data for optimal read performance
    pub fn prefetch(&mut self, offset: u64, length: u64) -> crate::backend::native::types::NativeResult<()> {
        IOOperationsManager::prefetch(self, offset, length)
    }

    /// Read with ahead buffering for sequential access patterns
    pub fn read_with_ahead(&mut self, offset: u64, buffer: &mut [u8]) -> crate::backend::native::types::NativeResult<()> {
        IOOperationsManager::read_with_ahead(&mut self.file, offset, buffer)
    }

    // Memory mapping operations (experimental feature)
    #[cfg(feature = "v2_experimental")]
    /// Ensure mmap region is at least the specified size
    pub fn mmap_ensure_size(&mut self, len: u64) -> crate::backend::native::types::NativeResult<()> {
        FileManager::mmap_ensure_size(&mut self.file, &self.file_path, len, &mut self.mmap)
    }

    #[cfg(feature = "v2_experimental")]
    /// Read bytes using mmap (V2 path only)
    pub fn mmap_read_bytes(&self, offset: u64, buffer: &mut [u8]) -> crate::backend::native::types::NativeResult<()> {
        crate::backend::native::graph_file::memory_mapping::MemoryMappingManager::mmap_read_bytes(&self.mmap, offset, buffer)
    }

    #[cfg(feature = "v2_experimental")]
    /// Write bytes using mmap (V2 path only)
    pub fn mmap_write_bytes(&mut self, offset: u64, data: &[u8]) -> crate::backend::native::types::NativeResult<()> {
        crate::backend::native::graph_file::memory_mapping::MemoryMappingManager::mmap_write_bytes(
            &mut self.file,
            &self.file_path,
            &mut self.write_buffer,
            &mut self.mmap,
            offset,
            data,
        )
    }

    #[cfg(feature = "v2_experimental")]
    /// Ensure mmap covers at least the specified offset using conservative remapping
    fn ensure_mmap_covers(&mut self, min_len: u64) -> crate::backend::native::types::NativeResult<()> {
        crate::backend::native::graph_file::memory_mapping::MemoryMappingManager::ensure_mmap_covers(
            &mut self.file,
            &mut self.write_buffer,
            &mut self.mmap,
            min_len,
        )
    }
}