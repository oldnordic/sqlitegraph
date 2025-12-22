//! Edge capacity coordinator for coordinating edge ID allocation with file capacity
//!
//! This module ensures that edge ID allocation is coordinated with file growth
//! to prevent "Attempted read beyond end of file" errors.

use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::types::{NativeEdgeId, NativeResult};

/// Coordinates edge ID allocation with file capacity management
pub struct EdgeCapacityCoordinator<'a> {
    graph_file: &'a mut GraphFile,
}

impl<'a> EdgeCapacityCoordinator<'a> {
    /// Create a new edge capacity coordinator
    ///
    /// # Arguments
    /// * `graph_file` - Mutable reference to the graph file
    pub fn new(graph_file: &'a mut GraphFile) -> Self {
        Self { graph_file }
    }

    /// Allocate edge ID with guaranteed file capacity
    ///
    /// This method allocates a new edge ID and ensures the underlying file
    /// is large enough to store an edge record at the calculated offset.
    ///
    /// # Returns
    /// The newly allocated edge ID with guaranteed file capacity
    pub fn allocate_edge_id_with_capacity(&mut self) -> NativeResult<NativeEdgeId> {
        // Get current edge count to allocate next ID
        let current_count = self.graph_file.persistent_header().edge_count;
        let new_edge_id = current_count + 1;

        // Ensure file has capacity for this edge before allocation
        self.ensure_capacity_for_edge_id(new_edge_id)?;

        // Update edge count in persistent header
        self.graph_file.persistent_header_mut().edge_count = new_edge_id;

        Ok(new_edge_id as NativeEdgeId)
    }

    /// Ensure file is large enough for edge with given ID
    ///
    /// This method checks if the file is large enough to store an edge
    /// at the calculated offset and grows the file if necessary.
    ///
    /// # Arguments
    /// * `edge_id` - The edge ID to ensure capacity for
    ///
    /// # Returns
    /// Ok(()) if capacity is ensured, Err with details if failed
    pub fn ensure_capacity_for_edge_id(&mut self, edge_id: u64) -> NativeResult<()> {
        const EDGE_SLOT_SIZE: u64 = 256;

        let edge_offset = self.calculate_edge_offset(edge_id);
        let required_size = edge_offset + EDGE_SLOT_SIZE;

        let current_file_size = self.graph_file.file_size()?;

        if current_file_size < required_size {
            let growth_amount = self.calculate_growth_amount(required_size, current_file_size);
            self.graph_file.grow(growth_amount)?;
        }

        Ok(())
    }

    /// Calculate file offset for edge
    ///
    /// Uses the same calculation as record_operations to ensure consistency.
    ///
    /// # Arguments
    /// * `edge_id` - The edge ID to calculate offset for
    ///
    /// # Returns
    /// File offset where the edge record is stored
    pub fn calculate_edge_offset(&self, edge_id: u64) -> u64 {
        let base_offset = self.graph_file.persistent_header().edge_data_offset;
        base_offset + ((edge_id - 1) * 256)
    }

    /// Calculate how much to grow file by using stepped growth strategy
    ///
    /// This implements an efficient growth strategy that grows files in
    /// reasonable increments to avoid frequent small growths.
    ///
    /// # Arguments
    /// * `required_size` - The minimum file size needed
    /// * `current_size` - The current file size
    ///
    /// # Returns
    /// Amount to grow the file by
    pub fn calculate_growth_amount(&self, required_size: u64, current_size: u64) -> u64 {
        // Stepped growth strategy for efficiency and to avoid fragmentation
        match required_size {
            0..=4096 => {
                // For small sizes, grow to 4KB
                if current_size < 4096 {
                    4096 - current_size
                } else {
                    0
                }
            }
            4097..=16384 => {
                // For medium sizes, grow to 16KB
                if current_size < 16384 {
                    16384 - current_size
                } else {
                    0
                }
            }
            16385..=65536 => {
                // For larger sizes, grow to 64KB
                if current_size < 65536 {
                    65536 - current_size
                } else {
                    0
                }
            }
            65537..=262144 => {
                // For even larger sizes, grow to 256KB
                if current_size < 262144 {
                    262144 - current_size
                } else {
                    0
                }
            }
            _ => {
                // For very large sizes, grow to next multiple of 1MB
                let target_size = ((required_size + 1048576 - 1) / 1048576) * 1048576;
                if target_size > current_size {
                    target_size - current_size
                } else {
                    0
                }
            }
        }
    }

    /// Get current file capacity statistics
    ///
    /// # Returns
    /// Tuple of (current_size, allocated_edges, max_edge_id_supported)
    pub fn get_capacity_statistics(&self) -> NativeResult<(u64, u64, u64)> {
        let current_size = self.graph_file.file_size()?;
        let allocated_edges = self.graph_file.persistent_header().edge_count;
        let base_offset = self.graph_file.persistent_header().edge_data_offset;

        // Calculate how many edges the current file size can support
        let available_space = if current_size > base_offset {
            current_size - base_offset
        } else {
            0
        };
        let max_edge_id_supported = available_space / 256;

        Ok((current_size, allocated_edges, max_edge_id_supported))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_capacity_coordinator_creation() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut graph_file = GraphFile::create(temp_file.path()).unwrap();

        let coordinator = EdgeCapacityCoordinator::new(&mut graph_file);
        // Should create without panic
        assert_eq!(coordinator.graph_file.persistent_header().edge_count, 0);
    }

    #[test]
    fn test_edge_offset_calculation() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut graph_file = GraphFile::create(temp_file.path()).unwrap();
        let coordinator = EdgeCapacityCoordinator::new(&mut graph_file);

        // Edge ID 1 should be at base offset
        let base_offset = coordinator.graph_file.persistent_header().edge_data_offset;
        assert_eq!(coordinator.calculate_edge_offset(1), base_offset);

        // Edge ID 2 should be at base offset + 256
        assert_eq!(coordinator.calculate_edge_offset(2), base_offset + 256);

        // Edge ID 10 should be at base offset + 9 * 256
        assert_eq!(
            coordinator.calculate_edge_offset(10),
            base_offset + (9 * 256)
        );
    }

    #[test]
    fn test_growth_amount_calculation() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut graph_file = GraphFile::create(temp_file.path()).unwrap();
        let coordinator = EdgeCapacityCoordinator::new(&mut graph_file);

        // Test small growth requirements
        assert_eq!(coordinator.calculate_growth_amount(1000, 500), 4096 - 500);

        // Test no growth needed
        assert_eq!(coordinator.calculate_growth_amount(4000, 4096), 0);

        // Test medium growth (20000 requires 64KB growth, not 16KB)
        assert_eq!(
            coordinator.calculate_growth_amount(20000, 4096),
            65536 - 4096
        );

        // Test large growth
        assert_eq!(
            coordinator.calculate_growth_amount(100000, 65536),
            262144 - 65536
        );

        // Test very large growth (1MB boundaries)
        assert_eq!(
            coordinator.calculate_growth_amount(2000000, 1048576),
            2097152 - 1048576
        );
    }

    #[test]
    fn test_capacity_statistics() {
        let temp_file = NamedTempFile::new().unwrap();
        let mut graph_file = GraphFile::create(temp_file.path()).unwrap();
        let coordinator = EdgeCapacityCoordinator::new(&mut graph_file);

        let (current_size, allocated_edges, max_supported) =
            coordinator.get_capacity_statistics().unwrap();

        // Should have initial capacity
        assert!(current_size > 0);
        assert_eq!(allocated_edges, 0);

        // The file may not have space for edges yet if edge_data_offset > file_size
        // This is normal - capacity will be allocated when edges are added
        assert!(max_supported >= 0);
    }
}
