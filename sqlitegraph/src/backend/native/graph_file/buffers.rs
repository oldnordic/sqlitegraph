//! Adaptive buffer management for I/O optimization
//!
//! This module provides read-ahead and write-behind buffering to minimize
//! I/O amplification while maintaining performance. The buffers are designed
//! to be safe and prevent accidental writes into the header region.

use crate::backend::native::constants::HEADER_SIZE;

/// Simple read-ahead buffer for performance optimization
/// Adaptive sizing to minimize I/O amplification while maintaining performance
pub struct ReadBuffer {
    pub(crate) data: Vec<u8>,
    pub(crate) offset: u64,
    pub(crate) size: usize,
    pub(crate) capacity: usize,
}

impl ReadBuffer {
    /// Calculate adaptive buffer capacity based on request size
    /// Goal: minimize I/O amplification while maintaining performance
    pub fn adaptive_capacity(request_size: usize) -> usize {
        if request_size < 128 {
            256 // ~8x amplification for tiny reads
        } else if request_size < 1024 {
            512 // ~2x amplification for small reads
        } else if request_size < 4096 {
            4096 // Page-aligned for medium reads
        } else {
            std::cmp::min(request_size * 2, 16384) // Bounded for large reads
        }
    }

    pub fn new() -> Self {
        Self::with_capacity(256) // Default 256B for typical node records
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: vec![0u8; capacity],
            offset: 0,
            size: 0,
            capacity,
        }
    }

    pub fn contains(&self, offset: u64, len: usize) -> bool {
        offset >= self.offset && (offset + len as u64) <= (self.offset + self.size as u64)
    }

    pub fn read(&self, offset: u64, buffer: &mut [u8]) -> bool {
        if self.contains(offset, buffer.len()) {
            let start = (offset - self.offset) as usize;
            buffer.copy_from_slice(&self.data[start..start + buffer.len()]);
            true
        } else {
            false
        }
    }

    /// Get the current buffer size (capacity)
    pub fn len(&self) -> usize {
        self.capacity
    }
}

/// Simple write-behind buffer for batched writes
pub struct WriteBuffer {
    pub(crate) operations: Vec<(u64, Vec<u8>)>,
    pub(crate) capacity: usize,
}

impl WriteBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            operations: Vec::new(),
            capacity,
        }
    }

    pub fn add(&mut self, offset: u64, data: Vec<u8>) -> bool {
        // PHASE 43: CRITICAL HEADER REGION LOCKDOWN
        // Prevent any buffered writes into header region [0, HEADER_SIZE)
        if offset < HEADER_SIZE {
            return false; // Reject header region writes
        }

        if self.operations.len() < self.capacity {
            self.operations.push((offset, data));
            true
        } else {
            false
        }
    }

    pub fn flush(&mut self) -> Vec<(u64, Vec<u8>)> {
        std::mem::take(&mut self.operations)
    }

    /// Get the current number of pending operations
    pub fn len(&self) -> usize {
        self.operations.len()
    }
}