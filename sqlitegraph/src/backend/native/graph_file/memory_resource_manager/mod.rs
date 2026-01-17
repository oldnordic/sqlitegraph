//! Memory resource manager for GraphFile
//!
//! This module provides centralized coordination of memory resources including
//! read buffers, write buffers, memory mapping, and I/O mode selection.
//! It handles complex memory management workflows and optimization strategies.

use crate::backend::native::graph_file::buffers::{ReadBuffer, WriteBuffer};

// Public exports
pub use manager::MemoryResourceManager;
pub use types::*;

// Module declarations
mod manager;
mod operations;
mod optimization;
mod types;

/// Memory management utilities for standalone usage
pub struct MemoryUtils;

impl MemoryUtils {
    /// Create optimized buffers for specific workload types
    pub fn create_optimized_buffers(
        workload_type: &str,
        max_write_ops: usize,
    ) -> (ReadBuffer, WriteBuffer) {
        let read_capacity = match workload_type {
            "bulk_import" | "sequential_scan" => 8192,
            "random_access" | "oltp" => 256,
            "mixed_workload" | "default" => 512,
            _ => 512,
        };

        (
            ReadBuffer::with_capacity(read_capacity),
            WriteBuffer::new(max_write_ops),
        )
    }

    /// Estimate memory usage for given configuration
    pub fn estimate_memory_usage(
        read_capacity: usize,
        max_write_ops: usize,
        avg_write_size: usize,
    ) -> usize {
        let read_buffer_bytes = read_capacity;
        let write_buffer_bytes = max_write_ops * avg_write_size;
        read_buffer_bytes + write_buffer_bytes
    }

    /// Get recommended configuration for available memory
    pub fn recommended_config_for_memory(available_bytes: usize) -> (usize, usize) {
        // Reserve 25% for read buffer, 25% for write buffer ops
        let read_buffer_bytes = available_bytes / 4;
        let write_buffer_bytes = available_bytes / 4;

        // Estimate average write size of 128 bytes
        let max_ops = write_buffer_bytes.saturating_div(128);
        let read_capacity = read_buffer_bytes;

        (read_capacity, max_ops)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempfile;

    #[test]
    fn test_memory_manager_creation() {
        let mut read_buffer = ReadBuffer::new();
        let mut write_buffer = WriteBuffer::new(32);
        #[cfg(feature = "native-v2")]
        let mut mmap = None;

        let manager = MemoryResourceManager::new(
            &mut read_buffer,
            &mut write_buffer,
            #[cfg(feature = "native-v2")]
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
        #[cfg(feature = "native-v2")]
        let mut mmap = None;

        let manager = MemoryResourceManager::new(
            &mut read_buffer,
            &mut write_buffer,
            #[cfg(feature = "native-v2")]
            &mut mmap,
        );

        // Should detect standard mode by default
        assert_eq!(manager.current_io_mode(), MemoryIOMode::Standard);
    }

    #[test]
    fn test_buffer_optimization() {
        let mut read_buffer = ReadBuffer::new();
        let mut write_buffer = WriteBuffer::new(32);
        #[cfg(feature = "native-v2")]
        let mut mmap = None;

        let mut manager = MemoryResourceManager::new(
            &mut read_buffer,
            &mut write_buffer,
            #[cfg(feature = "native-v2")]
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
        #[cfg(feature = "native-v2")]
        let mut mmap = None;

        let mut manager = MemoryResourceManager::new(
            &mut read_buffer,
            &mut write_buffer,
            #[cfg(feature = "native-v2")]
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
        #[cfg(feature = "native-v2")]
        let mut mmap = None;

        let mut manager = MemoryResourceManager::new(
            &mut read_buffer,
            &mut write_buffer,
            #[cfg(feature = "native-v2")]
            &mut mmap,
        );

        // Test adding operations to write buffer
        let mut temp_file = tempfile().unwrap();
        assert!(
            manager
                .memory_aware_write(&mut temp_file, 2048, b"test data", || Ok(4096))
                .is_ok()
        );

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
        #[cfg(feature = "native-v2")]
        let mut mmap = None;

        let mut manager = MemoryResourceManager::new(
            &mut read_buffer,
            &mut write_buffer,
            #[cfg(feature = "native-v2")]
            &mut mmap,
        );

        let mut temp_file = tempfile().unwrap();

        // Node slot write (4096 bytes at 0x400 + n*4096) should not be buffered
        let node_slot_data = vec![0u8; 4096];
        assert!(
            manager
                .memory_aware_write(
                    &mut temp_file,
                    0x400, // First node slot
                    &node_slot_data,
                    || Ok(8192)
                )
                .is_ok()
        );

        // Should have 0 pending operations since node slots are written directly
        let stats = manager.get_statistics();
        assert_eq!(stats.write_buffer_pending_ops, 0);
    }

    #[test]
    fn test_memory_utils() {
        // Test buffer creation
        let (read_buf, write_buf) = MemoryUtils::create_optimized_buffers("sequential_scan", 64);
        assert!(read_buf.capacity >= 8192);
        assert_eq!(write_buf.capacity, 64);

        // Test memory usage estimation
        let usage = MemoryUtils::estimate_memory_usage(4096, 32, 128);
        assert_eq!(usage, 4096 + (32 * 128));

        // Test memory-based configuration
        let (read_cap, max_ops) = MemoryUtils::recommended_config_for_memory(8192);
        assert_eq!(read_cap, 2048);
        assert_eq!(max_ops, 16);
    }
}
