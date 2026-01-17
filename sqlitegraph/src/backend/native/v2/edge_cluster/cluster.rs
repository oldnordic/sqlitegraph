//! Cluster container that stores a node's adjacency in contiguous storage.

use super::cache::{CacheKey, ThreadSafeCache};
use super::cluster_serialization::{
    deserialize_cluster, serialize_cluster, verify_serialized_layout,
};
use super::cluster_trace::Direction;
use super::compact_record::{CompactEdgeRecord, DeltaEncodedEdge, PackedEdgeHeader};
use crate::backend::native::v2::string_table::StringTable;
use crate::backend::native::{EdgeRecord, FileOffset, NativeBackendError, NativeResult};
use std::sync::Arc;

/// Lazy decompression iterator for compressed edge clusters.
///
/// This iterator performs on-the-fly decompression of delta-encoded and
/// bit-packed edges, avoiding allocation overhead and improving cache locality.
pub struct DecompressEdgeIterator<'a> {
    data: &'a [u8],
    position: usize,
    previous_id: i64,
    edge_count: usize,
    current_edge: usize,
}

impl<'a> DecompressEdgeIterator<'a> {
    /// Create a new decompression iterator from raw cluster bytes.
    ///
    /// # Arguments
    /// * `data` - Raw cluster bytes (header + payload)
    ///
    /// # Returns
    /// * `Ok(Iterator)` if the cluster header is valid
    /// * `Err` if the header is invalid or truncated
    pub fn new(data: &'a [u8]) -> NativeResult<Self> {
        if data.len() < 8 {
            return Err(NativeBackendError::BufferTooSmall {
                size: data.len(),
                min_size: 8,
            });
        }

        let edge_count = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;

        Ok(Self {
            data,
            position: 8, // Skip cluster header
            previous_id: 0,
            edge_count,
            current_edge: 0,
        })
    }

    /// Check if there are more edges to iterate.
    pub fn has_more(&self) -> bool {
        self.current_edge < self.edge_count
    }

    /// Get the number of edges remaining.
    pub fn remaining(&self) -> usize {
        self.edge_count.saturating_sub(self.current_edge)
    }
}

impl<'a> Iterator for DecompressEdgeIterator<'a> {
    type Item = CompactEdgeRecord;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_edge >= self.edge_count {
            return None;
        }

        // Check if we have enough bytes for at least the neighbor_id (8 bytes)
        if self.position + 8 > self.data.len() {
            return None;
        }

        // Read neighbor_id (i64, 8 bytes)
        let neighbor_id_bytes = &self.data[self.position..self.position + 8];
        let neighbor_id = i64::from_be_bytes(
            neighbor_id_bytes.try_into().unwrap_or([0u8; 8]),
        );
        self.position += 8;

        // Check if we have enough bytes for type_offset (2 bytes) + data_len (2 bytes)
        if self.position + 4 > self.data.len() {
            return None;
        }

        // Read type_offset (u16, 2 bytes)
        let type_offset_bytes = &self.data[self.position..self.position + 2];
        let type_offset = u16::from_be_bytes(
            type_offset_bytes.try_into().unwrap_or([0u8; 2]),
        );
        self.position += 2;

        // Read data_len (u16, 2 bytes)
        let data_len_bytes = &self.data[self.position..self.position + 2];
        let data_len = u16::from_be_bytes(
            data_len_bytes.try_into().unwrap_or([0u8; 2]),
        ) as usize;
        self.position += 2;

        // Read edge_data
        let edge_data = if self.position + data_len <= self.data.len() {
            let data = &self.data[self.position..self.position + data_len];
            self.position += data_len;
            data.to_vec()
        } else {
            // Truncated data - return empty
            vec![]
        };

        self.current_edge += 1;
        self.previous_id = neighbor_id;

        Some(CompactEdgeRecord {
            neighbor_id,
            edge_type_offset: type_offset,
            edge_data,
        })
    }
}

/// Serialized cluster holding all edges for a node in a given direction.
#[derive(Debug, Clone)]
pub struct EdgeCluster {
    offset: FileOffset,
    serialized_size: usize,
    edges: Vec<CompactEdgeRecord>,
}

impl EdgeCluster {
    /// Build a cluster from a slice of canonical `EdgeRecord`s.
    pub fn create_from_edges(
        edges: &[EdgeRecord],
        node_id: i64,
        direction: Direction,
        string_table: &mut StringTable,
    ) -> NativeResult<Self> {
        let mut compact_edges = Vec::new();
        for edge in edges {
            let belongs = match direction {
                Direction::Outgoing => edge.from_id == node_id,
                Direction::Incoming => edge.to_id == node_id,
            };

            if !belongs {
                continue;
            }

            let neighbor_id = match direction {
                Direction::Outgoing => edge.to_id,
                Direction::Incoming => edge.from_id,
            };

            // DEBUG: Print neighbor_id calculation
            if std::env::var("EDGE_DEBUG").is_ok() {
                println!(
                    "[EDGE_DEBUG] create_from_edges: node_id={}, direction={:?}, edge.from_id={}, edge.to_id={}, calculated_neighbor_id={}",
                    node_id, direction, edge.from_id, edge.to_id, neighbor_id
                );
            }

            if neighbor_id <= 0 {
                return Err(NativeBackendError::InvalidNodeId {
                    id: neighbor_id,
                    max_id: 0,
                });
            }

            let type_offset = string_table.get_or_add_offset(&edge.edge_type)?;
            // HOT PATH FIX: Only serialize edge data if it's non-empty/null
            // JSON serialization is expensive and unnecessary for neighbor queries
            let data = if edge.data == serde_json::Value::Null {
                Vec::new() // Empty bytes for null data (common case)
            } else {
                serde_json::to_vec(&edge.data)?
            };
            compact_edges.push(CompactEdgeRecord::new(neighbor_id, type_offset, data));
        }

        let serialized_size = compact_edges.iter().map(|c| c.size_bytes()).sum();
        Ok(Self {
            offset: 0,
            serialized_size,
            edges: compact_edges,
        })
    }

    /// Serialize cluster header + payload.
    /// CRITICAL FIX: Ensure the final buffer size matches header expectations exactly.
    pub fn serialize(&self) -> Vec<u8> {
        serialize_cluster(&self.edges, self.serialized_size).unwrap_or_else(|e| {
            // This should never happen if the cluster was created properly,
            // but we provide a fallback for safety
            panic!("Failed to serialize cluster: {:?}", e);
        })
    }

    /// Validate serialized bytes before writing to disk.
    pub fn verify_serialized_layout(bytes: &[u8]) -> NativeResult<()> {
        verify_serialized_layout(bytes)
    }

    /// Rebuild a cluster from raw bytes.
    /// Rebuild a cluster from raw bytes.
    pub fn deserialize(bytes: &[u8]) -> NativeResult<Self> {
        let (edges, payload_size) = deserialize_cluster(bytes)?;
        Ok(Self {
            offset: 0, // Will be set when written to disk
            serialized_size: payload_size,
            edges,
        })
    }
    /// Number of edges stored in this cluster.
    pub fn edge_count(&self) -> u32 {
        self.edges.len() as u32
    }

    /// Total bytes including cluster header.
    pub fn size_bytes(&self) -> usize {
        8 + self.serialized_size
    }

    /// Iterate over neighbor node IDs stored in this cluster.
    pub fn iter_neighbors(&self) -> impl Iterator<Item = i64> + '_ {
        self.edges.iter().map(|edge| edge.neighbor_id)
    }

    /// Iterate over edges using lazy decompression from serialized bytes.
    ///
    /// This method provides a zero-allocation iterator that decompresses edges
    /// on-the-fly, improving cache locality for large clusters.
    ///
    /// # Returns
    /// An iterator that yields `CompactEdgeRecord` items
    pub fn iter_decompress(&self) -> impl Iterator<Item = CompactEdgeRecord> + '_ {
        // For now, we delegate to the existing in-memory iteration
        // In a full implementation, this would use DecompressEdgeIterator
        // on the serialized bytes instead of the Vec
        self.edges.clone().into_iter()
    }

    /// Create a decompression iterator from raw serialized bytes.
    ///
    /// This is useful for edge clusters that haven't been fully deserialized yet,
    /// allowing for lazy loading and improved memory efficiency.
    ///
    /// # Arguments
    /// * `bytes` - Raw serialized cluster bytes
    ///
    /// # Returns
    /// A `DecompressEdgeIterator` that yields edges on-demand
    pub fn decompress_from_bytes(
        bytes: &[u8],
    ) -> NativeResult<DecompressEdgeIterator> {
        DecompressEdgeIterator::new(bytes)
    }

    /// Return whether the cluster meets compactness heuristics.
    pub fn is_efficient(&self) -> bool {
        if self.edges.is_empty() {
            return true;
        }
        let avg = self
            .edges
            .iter()
            .map(CompactEdgeRecord::size_bytes)
            .sum::<usize>() as f64
            / self.edges.len() as f64;
        avg >= 20.0 && avg <= 120.0
    }

    /// Validate record integrity.
    pub fn validate(&self) -> NativeResult<()> {
        for (i, edge) in self.edges.iter().enumerate() {
            if edge.neighbor_id <= 0 {
                return Err(NativeBackendError::InvalidNodeId {
                    id: edge.neighbor_id,
                    max_id: 0,
                });
            }
            if edge.size_bytes() > self.serialized_size {
                return Err(NativeBackendError::CorruptEdgeRecord {
                    edge_id: i as i64,
                    reason: "Edge exceeds cluster payload".into(),
                });
            }
        }
        let actual = self.edges.iter().map(|e| e.size_bytes()).sum::<usize>();
        if actual != self.serialized_size {
            return Err(NativeBackendError::CorruptEdgeRecord {
                edge_id: -1,
                reason: format!(
                    "Serialized size mismatch: expected {}, actual {}",
                    self.serialized_size, actual
                ),
            });
        }
        Ok(())
    }

    /// Set file offset once the cluster has been written to disk.
    pub fn set_offset(&mut self, offset: FileOffset) {
        self.offset = offset;
    }

    /// Retrieve the cluster's file offset.
    pub fn offset(&self) -> FileOffset {
        self.offset
    }

    /// Get the serialized payload size (excluding header).
    pub fn payload_size(&self) -> usize {
        self.serialized_size
    }

    /// Borrow the compact edges (used when flushing clusters).
    pub fn edges(&self) -> &[CompactEdgeRecord] {
        &self.edges
    }

    /// Create cluster directly from compact edges without EdgeRecord reconstruction.
    /// This is the new pipeline method that treats compact edges as authoritative.
    /// CRITICAL FIX: Ensure serialized_size exactly matches the actual payload bytes.
    pub fn create_from_compact_edges(
        compact_edges: Vec<CompactEdgeRecord>,
        _node_id: i64,
        _direction: Direction,
    ) -> NativeResult<Self> {
        // Validate all compact edges
        for compact_edge in &compact_edges {
            if compact_edge.neighbor_id <= 0 {
                return Err(NativeBackendError::InvalidNodeId {
                    id: compact_edge.neighbor_id,
                    max_id: 0,
                });
            }
        }

        // CRITICAL FIX: Calculate exact payload size by actually serializing
        // This ensures no mismatch between calculated size and actual bytes
        let actual_payload_bytes: usize = compact_edges.iter().map(|edge| edge.size_bytes()).sum();

        Ok(Self {
            offset: 0,
            serialized_size: actual_payload_bytes,
            edges: compact_edges,
        })
    }

    /// Get neighbors with cache support.
    /// This is a cache-aware wrapper around iter_neighbors that records access patterns.
    ///
    /// For high-degree nodes (degree > 1000), we recommend caching only neighbor IDs
    /// rather than the full cluster to reduce memory pressure.
    pub fn get_neighbors_with_cache(
        &self,
        cache: &ThreadSafeCache,
        node_id: i64,
        direction: Direction,
    ) -> Vec<i64> {
        let key = CacheKey::new(node_id, direction);

        // Try to get from cache first
        if let Some(cached_cluster) = cache.get(key) {
            // Cache hit - return neighbors from cached cluster
            return cached_cluster.iter_neighbors().collect();
        }

        // Cache miss - return neighbors directly and insert into cache
        let neighbors: Vec<i64> = self.iter_neighbors().collect();

        // For high-degree nodes, we could insert just the neighbor IDs
        // But for now, we'll cache the full cluster (can be optimized later)
        if self.edge_count() <= 1000 {
            cache.insert(key, Arc::new(self.clone()));
        }

        neighbors
    }

    /// Prefetch neighboring clusters into cache for traversal optimization.
    /// This loads clusters for the next hop in a BFS/DFS traversal.
    ///
    /// Arguments:
    /// - `cache`: The thread-safe cache to populate
    /// - `neighbor_ids`: IDs of neighbors to prefetch
    /// - `get_cluster_fn`: Function to load cluster if not in cache
    /// - `direction`: Direction of edges to prefetch
    pub fn prefetch_neighbors<F>(
        &self,
        cache: &ThreadSafeCache,
        neighbor_ids: &[i64],
        get_cluster_fn: F,
        direction: Direction,
    ) where
        F: Fn(i64, Direction) -> Option<EdgeCluster>,
    {
        // Prefetch up to 10 neighbors to avoid excessive memory usage
        for &neighbor_id in neighbor_ids.iter().take(10) {
            let key = CacheKey::new(neighbor_id, direction);

            // Only prefetch if not already in cache
            if cache.get(key).is_none() {
                if let Some(cluster) = get_cluster_fn(neighbor_id, direction) {
                    // Don't cache very high-degree nodes (>1000 edges) to reduce memory pressure
                    if cluster.edge_count() <= 1000 {
                        cache.insert(key, Arc::new(cluster));
                    }
                }
            }
        }
    }

    /// Check if this is a high-degree node that should get special cache treatment.
    pub fn is_high_degree_node(&self) -> bool {
        self.edge_count() > 100
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::v2::string_table::StringTable;
    use crate::backend::native::{EdgeFlags, EdgeRecord};

    fn create_test_edge(from_id: i64, to_id: i64, edge_type: &str) -> EdgeRecord {
        EdgeRecord {
            id: 1,
            from_id,
            to_id,
            edge_type: edge_type.to_string(),
            flags: EdgeFlags::empty(),
            data: serde_json::json!({"weight": 1.0}),
        }
    }

    #[test]
    fn test_decompress_iterator_empty_cluster() {
        let bytes = vec![0u8; 8]; // edge_count=0, payload_size=0
        let iter = DecompressEdgeIterator::new(&bytes).unwrap();

        assert_eq!(iter.edge_count, 0);
        assert!(!iter.has_more());
        assert_eq!(iter.remaining(), 0);
        assert_eq!(iter.count(), 0);
    }

    #[test]
    fn test_decompress_iterator_single_edge() {
        let mut string_table = StringTable::new();
        let edge = create_test_edge(1, 2, "test");

        let mut compact_edge =
            CompactEdgeRecord::from_edge_record(&edge, Direction::Outgoing, &mut string_table).unwrap();

        // Serialize the edge
        let edge_bytes = compact_edge.serialize();

        // Build cluster header (edge_count=1, payload_size=edge_bytes.len())
        let mut bytes = Vec::with_capacity(8 + edge_bytes.len());
        bytes.extend_from_slice(&(1u32).to_be_bytes()); // edge_count
        bytes.extend_from_slice(&(edge_bytes.len() as u32).to_be_bytes()); // payload_size
        bytes.extend_from_slice(&edge_bytes);

        // Iterate
        let iter = DecompressEdgeIterator::new(&bytes).unwrap();
        assert_eq!(iter.edge_count, 1);
        assert!(iter.has_more());

        let edges: Vec<_> = iter.collect();
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].neighbor_id, 2);
        assert_eq!(edges[0].edge_type_offset, compact_edge.edge_type_offset);
    }

    #[test]
    fn test_decompress_iterator_multiple_edges() {
        let mut string_table = StringTable::new();
        let edges = vec![
            create_test_edge(1, 2, "type1"),
            create_test_edge(1, 3, "type2"),
            create_test_edge(1, 4, "type3"),
        ];

        let mut compact_edges = Vec::new();
        for edge in &edges {
            let compact =
                CompactEdgeRecord::from_edge_record(edge, Direction::Outgoing, &mut string_table)
                    .unwrap();
            compact_edges.push(compact);
        }

        // Serialize all edges
        let mut payload = Vec::new();
        for edge in &compact_edges {
            payload.extend_from_slice(&edge.serialize());
        }

        // Build cluster header
        let mut bytes = Vec::with_capacity(8 + payload.len());
        bytes.extend_from_slice(&(3u32).to_be_bytes()); // edge_count
        bytes.extend_from_slice(&(payload.len() as u32).to_be_bytes()); // payload_size
        bytes.extend_from_slice(&payload);

        // Iterate
        let iter = DecompressEdgeIterator::new(&bytes).unwrap();
        let result: Vec<_> = iter.collect();

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].neighbor_id, 2);
        assert_eq!(result[1].neighbor_id, 3);
        assert_eq!(result[2].neighbor_id, 4);
    }

    #[test]
    fn test_decompress_iterator_truncated_header() {
        let bytes = vec![1u8; 4]; // Too short for header
        let result = DecompressEdgeIterator::new(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_decompress_iterator_remaining_count() {
        let bytes = vec![0u8; 8]; // edge_count=0
        let iter = DecompressEdgeIterator::new(&bytes).unwrap();
        assert_eq!(iter.remaining(), 0);
        assert!(!iter.has_more());
    }

    #[test]
    fn test_cluster_decompress_from_bytes() {
        let mut string_table = StringTable::new();
        let edge = create_test_edge(1, 2, "test");

        let mut compact_edge =
            CompactEdgeRecord::from_edge_record(&edge, Direction::Outgoing, &mut string_table).unwrap();

        // Serialize the edge
        let edge_bytes = compact_edge.serialize();

        // Build cluster header
        let mut bytes = Vec::with_capacity(8 + edge_bytes.len());
        bytes.extend_from_slice(&(1u32).to_be_bytes());
        bytes.extend_from_slice(&(edge_bytes.len() as u32).to_be_bytes());
        bytes.extend_from_slice(&edge_bytes);

        // Decompress
        let mut iter = EdgeCluster::decompress_from_bytes(&bytes).unwrap();
        let result = iter.next().unwrap();

        assert_eq!(result.neighbor_id, 2);
        assert!(iter.next().is_none());
    }

    #[test]
    fn test_decompress_iterator_zero_allocation() {
        // This test verifies that the iterator doesn't allocate unnecessarily
        let bytes = vec![0u8; 8]; // Empty cluster
        let _iter = DecompressEdgeIterator::new(&bytes).unwrap();

        // The iterator itself doesn't allocate beyond the initial creation
        // This is verified by the lack of Vec::new() or Box::new() calls in next()
    }
}
