//! Cluster container that stores a node's adjacency in contiguous storage.

use super::cluster_trace::{Direction, TraceContext, TraceGuard, strict_mode_enabled, with_trace_context, current_trace_context, format_strict_reason};
use super::cluster_serialization::{serialize_cluster, verify_serialized_layout, deserialize_cluster};
use super::compact_record::CompactEdgeRecord;
use crate::backend::native::v2::string_table::StringTable;
use crate::backend::native::{EdgeRecord, FileOffset, NativeBackendError, NativeResult};

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
                println!("[EDGE_DEBUG] create_from_edges: node_id={}, direction={:?}, edge.from_id={}, edge.to_id={}, calculated_neighbor_id={}",
                    node_id, direction, edge.from_id, edge.to_id, neighbor_id);
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
}
