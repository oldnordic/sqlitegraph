//! I/O backend routing and management for GraphFile
//!
//! This module provides I/O backend selection and routing logic,
//! handling different I/O modes (standard vs mmap) and exclusive access patterns.

use crate::backend::native::{
    graph_file::buffers::WriteBuffer, graph_file::file_ops::IOMode,
    types::NativeResult, NativeBackendError,
};

#[cfg(feature = "v2_experimental")]
use memmap2::MmapMut;

/// I/O backend management utilities for GraphFile
pub struct IOBackendManager;

impl IOBackendManager {
    /// Route read operations based on current I/O mode and configuration
    ///
    /// This method routes read requests to the appropriate backend:
    /// - Exclusive MMAP mode: Direct memory-mapped reads
    /// - Exclusive STD mode: Standard file I/O with buffer management
    /// - Default mode: Standard file I/O
    #[allow(unused_variables)]  // Allow warnings for feature-conditional parameters
    pub fn route_read_bytes(
        file: &mut std::fs::File,
        buffer: &mut [u8],
        offset: u64,
        write_buffer: &mut WriteBuffer,
        #[cfg(feature = "v2_experimental")] mmap: Option<&MmapMut>,
        io_mode: IOMode,
    ) -> NativeResult<()> {
        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
        {
            if io_mode.is_exclusive_mmap() {
                return Self::read_bytes_mmap_exclusive(mmap, buffer, offset);
            }
        }

        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
        {
            if io_mode.is_exclusive_std() {
                return Self::read_bytes_std_exclusive(file, buffer, offset, write_buffer);
            }
        }

        // Default mode: use standard file I/O
        Self::read_bytes_std(file, buffer, offset)
    }

    /// Route write operations based on current I/O mode and configuration
    ///
    /// This method routes write requests to the appropriate backend:
    /// - Exclusive MMAP mode: Direct memory-mapped writes
    /// - Exclusive STD mode: Standard file I/O with buffer management
    /// - Default mode: Standard file I/O
    #[allow(unused_variables)]  // Allow warnings for feature-conditional parameters
    pub fn route_write_bytes(
        file: &mut std::fs::File,
        data: &[u8],
        offset: u64,
        write_buffer: &mut WriteBuffer,
        #[cfg(feature = "v2_experimental")] mmap: Option<&mut MmapMut>,
        io_mode: IOMode,
    ) -> NativeResult<()> {
        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
        {
            if io_mode.is_exclusive_mmap() {
                return Self::write_bytes_mmap_exclusive(mmap, data, offset);
            }
        }

        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
        {
            if io_mode.is_exclusive_std() {
                return Self::write_bytes_std_exclusive(file, data, offset, write_buffer);
            }
        }

        // Default mode: use standard file I/O
        Self::write_bytes_std(file, data, offset)
    }

    /// Route buffered write operations based on current I/O mode
    ///
    /// Handles buffered write operations with proper backend routing
    /// for different I/O modes and exclusive access patterns.
    #[allow(unused_variables)]  // Allow warnings for feature-conditional parameters
    pub fn route_buffered_write_bytes(
        file: &mut std::fs::File,
        data: &[u8],
        offset: u64,
        write_buffer: &mut WriteBuffer,
        #[cfg(feature = "v2_experimental")] mmap: Option<&mut MmapMut>,
        io_mode: IOMode,
    ) -> NativeResult<()> {
        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
        {
            if io_mode.is_exclusive_mmap() {
                let end_offset = offset + data.len() as u64;
                return Self::write_buffered_bytes_mmap_exclusive(mmap, data, offset, end_offset);
            }
        }

        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
        {
            if io_mode.is_exclusive_std() {
                return Self::write_buffered_bytes_std_exclusive(file, data, offset, write_buffer);
            }
        }

        // Default mode: use standard file I/O
        Self::write_buffered_bytes_std(file, data, offset, write_buffer)
    }

    /// Check if mmap mode is available and configured
    #[cfg(feature = "v2_experimental")]
    pub fn is_mmap_mode_available(io_mode: IOMode) -> bool {
        io_mode.is_exclusive_mmap()
    }

    /// Check if exclusive std mode is available and configured
    #[cfg(feature = "v2_experimental")]
    pub fn is_exclusive_std_mode_available(io_mode: IOMode) -> bool {
        io_mode.is_exclusive_std()
    }

    /// Get backend description for debugging
    pub fn get_backend_description(io_mode: IOMode) -> &'static str {
        match io_mode {
            #[cfg(feature = "v2_experimental")]
            IOMode::ExclusiveMmap => "Exclusive Memory-Mapped I/O",
            #[cfg(feature = "v2_experimental")]
            IOMode::ExclusiveStd => "Exclusive Standard I/O",
            _ => "Default Mixed I/O",
        }
    }

    // Private implementation methods for specific backends

    /// Read bytes using exclusive mmap mode
    #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
    fn read_bytes_mmap_exclusive(
        mmap: Option<&MmapMut>,
        buffer: &mut [u8],
        offset: u64,
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

    /// Read bytes using exclusive std mode
    #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
    fn read_bytes_std_exclusive(
        file: &mut std::fs::File,
        buffer: &mut [u8],
        offset: u64,
        write_buffer: &mut WriteBuffer,
    ) -> NativeResult<()> {
        use std::io::{Read, Seek, SeekFrom};

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

    /// Read bytes using standard file I/O
    fn read_bytes_std(
        file: &mut std::fs::File,
        buffer: &mut [u8],
        offset: u64,
    ) -> NativeResult<()> {
        use std::io::{Read, Seek, SeekFrom};

        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(buffer)?;
        Ok(())
    }

    /// Write bytes using exclusive mmap mode
    #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
    fn write_bytes_mmap_exclusive(
        mmap: Option<&mut MmapMut>,
        data: &[u8],
        offset: u64,
    ) -> NativeResult<()> {
        let end_offset = offset + data.len() as u64;

        // Ensure mmap covers the write region
        // Note: In a real implementation, you'd need to handle mmap resizing here
        let mmap = mmap.ok_or(NativeBackendError::CorruptNodeRecord {
            node_id: -1,
            reason: "mmap not initialized in exclusive mmap mode".to_string(),
        })?;

        if offset as usize + data.len() > mmap.len() {
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

        mmap[offset as usize..offset as usize + data.len()].copy_from_slice(data);
        mmap.flush()?;
        Ok(())
    }

    /// Write bytes using exclusive std mode
    #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
    fn write_bytes_std_exclusive(
        file: &mut std::fs::File,
        data: &[u8],
        offset: u64,
        write_buffer: &mut WriteBuffer,
    ) -> NativeResult<()> {
        use std::io::{Seek, SeekFrom, Write};

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

    /// Write bytes using standard file I/O
    fn write_bytes_std(file: &mut std::fs::File, data: &[u8], offset: u64) -> NativeResult<()> {
        use std::io::{Seek, SeekFrom, Write};

        file.seek(SeekFrom::Start(offset))?;
        file.write_all(data)?;
        Ok(())
    }

    /// Write buffered bytes using exclusive mmap mode
    #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
    fn write_buffered_bytes_mmap_exclusive(
        mmap: Option<&mut MmapMut>,
        data: &[u8],
        offset: u64,
        end_offset: u64,
    ) -> NativeResult<()> {
        // Ensure mmap covers the write region
        // Note: In a real implementation, you'd need to handle mmap resizing here
        let mmap = mmap.ok_or(NativeBackendError::CorruptNodeRecord {
            node_id: -1,
            reason: "mmap not initialized in exclusive mmap mode".to_string(),
        })?;

        if end_offset as usize > mmap.len() {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "Write beyond mmap region: offset={}, len={}, mmap_size={}",
                    offset,
                    data.len(),
                    mmap.len()
                ),
            });
        }

        let start = offset as usize;
        let end = start + data.len();
        mmap[start..end].copy_from_slice(data);

        // Flush mmap changes to disk
        mmap.flush()?;
        Ok(())
    }

    /// Write buffered bytes using exclusive std mode
    #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
    fn write_buffered_bytes_std_exclusive(
        file: &mut std::fs::File,
        data: &[u8],
        offset: u64,
        write_buffer: &mut WriteBuffer,
    ) -> NativeResult<()> {
        use std::io::{Seek, SeekFrom, Write};

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

    /// Write buffered bytes using standard file I/O
    fn write_buffered_bytes_std(
        file: &mut std::fs::File,
        data: &[u8],
        offset: u64,
        write_buffer: &mut WriteBuffer,
    ) -> NativeResult<()> {
        use std::io::{Seek, SeekFrom, Write};

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
}

/// I/O backend statistics for debugging and monitoring
#[derive(Debug, Clone)]
pub struct IOBackendStatistics {
    pub backend_type: String,
    pub is_exclusive_mmap: bool,
    pub is_exclusive_std: bool,
    pub is_default_mode: bool,
}

impl IOBackendStatistics {
    /// Create new I/O backend statistics
    pub fn new(io_mode: IOMode) -> Self {
        Self {
            backend_type: IOBackendManager::get_backend_description(io_mode).to_string(),
            is_exclusive_mmap: io_mode.is_exclusive_mmap(),
            is_exclusive_std: io_mode.is_exclusive_std(),
            is_default_mode: io_mode.is_default(),
        }
    }

    /// Get backend description
    pub fn get_backend_type(&self) -> &str {
        &self.backend_type
    }

    /// Check if using high-performance mmap backend
    pub fn is_high_performance(&self) -> bool {
        self.is_exclusive_mmap
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempfile;

    #[test]
    fn test_backend_description() {
        let mode = IOMode::current();
        let description = IOBackendManager::get_backend_description(mode);
        assert!(!description.is_empty());
    }

    #[test]
    fn test_io_backend_statistics() {
        let mode = IOMode::current();
        let stats = IOBackendStatistics::new(mode);
        assert!(!stats.backend_type.is_empty());
        assert_eq!(stats.is_default_mode, mode.is_default());
    }

    #[test]
    fn test_standard_read_write() {
        let mut temp_file = tempfile().unwrap();

        // Write test data
        let test_data = b"Hello, I/O Backend!";
        IOBackendManager::route_write_bytes(
            &mut temp_file,
            test_data,
            0,
            &mut WriteBuffer::new(10),
            #[cfg(feature = "v2_experimental")]
            None,
            IOMode::Default,
        )
        .unwrap();

        // Read back test data
        let mut buffer = vec![0u8; test_data.len()];
        IOBackendManager::route_read_bytes(
            &mut temp_file,
            &mut buffer,
            0,
            &mut WriteBuffer::new(10),
            #[cfg(feature = "v2_experimental")]
            None,
            IOMode::Default,
        )
        .unwrap();

        assert_eq!(buffer, test_data);
    }

    #[test]
    fn test_buffered_write() {
        let mut temp_file = tempfile().unwrap();
        let mut write_buffer = WriteBuffer::new(10);

        // Write test data using buffered write
        let test_data = b"Buffered write test";
        IOBackendManager::route_buffered_write_bytes(
            &mut temp_file,
            test_data,
            0,
            &mut write_buffer,
            #[cfg(feature = "v2_experimental")]
            None,
            IOMode::Default,
        )
        .unwrap();

        // Force flush to ensure data is written
        let operations = write_buffer.flush();
        for (offset, data) in operations {
            use std::io::{Seek, SeekFrom, Write};
            temp_file.seek(SeekFrom::Start(offset)).unwrap();
            temp_file.write_all(&data).unwrap();
        }

        // Verify data was written
        let mut buffer = vec![0u8; test_data.len()];
        use std::io::{Read, Seek, SeekFrom};
        temp_file.seek(SeekFrom::Start(0)).unwrap();
        temp_file.read_exact(&mut buffer).unwrap();

        assert_eq!(buffer, test_data);
    }

    #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
    #[test]
    fn test_exclusive_std_mode() {
        let mode = IOMode::ExclusiveStd;
        assert!(IOBackendManager::is_exclusive_std_mode_available(mode));
        assert!(!IOBackendManager::is_mmap_mode_available(mode));
        assert!(mode.is_exclusive_std());
    }

    #[test]
    fn test_io_mode_properties() {
        let default_mode = IOMode::Default;
        assert!(default_mode.is_default());
        assert!(!default_mode.is_exclusive_mmap());
        assert!(!default_mode.is_exclusive_std());
    }
}
