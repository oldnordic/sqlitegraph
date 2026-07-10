use super::{
    Direction, EDGE_CLUSTER_PAGE_HEADER_SIZE, EDGE_CLUSTER_PAGE_MAGIC,
    PACKED_EDGE_PAGE_HEADER_SIZE, PACKED_EDGE_PAGE_MAGIC, PACKED_EDGE_PAGE_SLOT_SIZE,
};
use crate::backend::native::types::{NativeBackendError, NativeResult};
use crate::backend::native::v3::compact_edge_record::CompactEdgeRecord;
use crate::backend::native::v3::compression::edge_delta::{compress_edge_ids, decompress_edge_ids};
use std::vec::Vec;

#[path = "edge_cluster_support/page_codec_support.rs"]
mod page_codec_support;
#[cfg(test)]
#[path = "edge_cluster_support/tests.rs"]
mod tests;

/// Edge cluster entry for V3 storage.
/// Uses V2 `CompactEdgeRecord` format for compatibility.
#[derive(Debug, Clone)]
pub struct V3EdgeCluster {
    /// Source node ID (logical, not slot)
    pub src: i64,
    /// Destination node IDs with edge data
    pub edges: Vec<CompactEdgeRecord>,
    /// Edge direction
    pub direction: Direction,
    /// Format version for future migration
    pub format_version: u8,
    /// Page ID where this cluster is stored
    pub page_id: u64,
}

impl V3EdgeCluster {
    pub(crate) fn sort_for_weighted_queries(&mut self) {
        self.edges.sort_by(|a, b| {
            let weight_cmp = Self::extract_edge_weight(&b.edge_data)
                .partial_cmp(&Self::extract_edge_weight(&a.edge_data))
                .unwrap_or(std::cmp::Ordering::Equal);
            weight_cmp.then_with(|| a.neighbor_id.cmp(&b.neighbor_id))
        });
    }

    /// Create new empty edge cluster
    pub fn new(src: i64, direction: Direction, page_id: u64) -> Self {
        Self {
            src,
            edges: Vec::new(),
            direction,
            format_version: 3, // v3 includes delta compression for edge IDs
            page_id,
        }
    }

    /// Add edge to cluster
    /// Edge type is encoded in edge_data using inline format: [type_len: u8][type_bytes]
    pub fn add_edge(&mut self, dst: i64, edge_type: Option<String>) {
        // Encode edge_type into edge_data using inline format: [type_len: u8][type_bytes]
        let edge_data = if let Some(et) = edge_type {
            let et_bytes = et.as_bytes();
            let mut data = Vec::with_capacity(1 + et_bytes.len());
            data.push(et_bytes.len() as u8);
            data.extend_from_slice(et_bytes);
            data
        } else {
            // Empty edge_data means no edge_type
            Vec::new()
        };

        let edge = CompactEdgeRecord::new(dst, 0, edge_data);
        self.edges.push(edge);
    }

    /// Add a weighted edge to the cluster
    pub fn add_edge_weighted(&mut self, dst: i64, edge_type: Option<String>, weight: f32) {
        let edge_data = {
            let et_bytes = edge_type.as_ref().map(|s| s.as_bytes()).unwrap_or(&[]);
            let mut data = Vec::with_capacity(6 + et_bytes.len());
            data.push(0x80); // Format flag for weighted edge
            data.extend_from_slice(&weight.to_be_bytes());
            data.push(et_bytes.len() as u8);
            data.extend_from_slice(et_bytes);
            data
        };

        let edge = CompactEdgeRecord::new(dst, 0, edge_data);
        self.edges.push(edge);
    }

    /// Extract edge type from edge data
    /// Returns None if edge_data is empty (no edge_type stored)
    pub(crate) fn extract_edge_type(edge_data: &[u8]) -> Option<String> {
        if edge_data.is_empty() {
            return None;
        }
        if edge_data[0] == 0x80 {
            // Weighted format: [0x80: 1 byte] [weight: 4 bytes] [type_len: 1 byte] [type_bytes...]
            if edge_data.len() < 6 {
                return None;
            }
            let type_len = edge_data[5] as usize;
            if edge_data.len() < 6 + type_len {
                return None;
            }
            Some(String::from_utf8_lossy(&edge_data[6..6 + type_len]).to_string())
        } else {
            // Legacy format: [type_len: 1 byte] [type_bytes...]
            let type_len = edge_data[0] as usize;
            if edge_data.len() < 1 + type_len {
                return None;
            }
            Some(String::from_utf8_lossy(&edge_data[1..1 + type_len]).to_string())
        }
    }

    /// Extract edge weight from edge data
    /// Returns 1.0 if not specified (legacy or not provided)
    pub(crate) fn extract_edge_weight(edge_data: &[u8]) -> f32 {
        if edge_data.len() >= 5 && edge_data[0] == 0x80 {
            f32::from_be_bytes([edge_data[1], edge_data[2], edge_data[3], edge_data[4]])
        } else {
            1.0
        }
    }

    /// Get destination node IDs
    pub fn dsts(&self) -> Vec<i64> {
        self.edges.iter().map(|e| e.neighbor_id).collect()
    }

    /// Get edges with their types (for recovery/rebuilding HashMap)
    pub fn edges_with_types(&self) -> Vec<(i64, Option<String>)> {
        self.edges
            .iter()
            .map(|e| {
                let edge_type = Self::extract_edge_type(&e.edge_data);
                (e.neighbor_id, edge_type)
            })
            .collect()
    }

    /// Serialize to bytes for page storage
    /// Format v3: [version: 1 byte] [src: 8 bytes] [dir: 1 byte] [compressed: 1 byte] [edge_count: 4 bytes] [compressed_ids...][edge_metadata...]
    /// Format v2: [version: 1 byte] [src: 8 bytes] [dir: 1 byte] [edge_count: 4 bytes] [edges...]
    /// Format v1: [version: 1 byte] [edge_count: 4 bytes] [edges...]  (legacy, no src/dir)
    pub fn serialize(&self) -> NativeResult<Vec<u8>> {
        let mut result = Vec::new();

        // Header: format_version (1 byte)
        result.push(self.format_version);

        // v2+ format: embed src and direction for recovery
        if self.format_version >= 2 {
            // Source node ID (8 bytes, big-endian)
            result.extend_from_slice(&self.src.to_be_bytes());
            // Direction (1 byte): 0 = Outgoing, 1 = Incoming
            result.push(if self.direction == Direction::Outgoing {
                0
            } else {
                1
            });
        }

        // Edge count (4 bytes, big-endian)
        let count = self.edges.len() as u32;
        result.extend_from_slice(&count.to_be_bytes());

        // v3 format: use delta compression for edge IDs
        if self.format_version >= 3 {
            // Compression flag (1 byte): 1 = compressed, 0 = uncompressed
            result.push(1); // Always compress in v3

            // Extract and compress neighbor IDs
            let neighbor_ids: Vec<i64> = self.edges.iter().map(|e| e.neighbor_id).collect();
            let compressed_ids = compress_edge_ids(&neighbor_ids);

            // Store compressed ID count (4 bytes) and data
            result.extend_from_slice(&(compressed_ids.len() as u32).to_be_bytes());
            result.extend_from_slice(&compressed_ids);

            // Store edge metadata (type_offset and edge_data) separately
            for edge in &self.edges {
                // type_offset (2 bytes)
                result.extend_from_slice(&edge.edge_type_offset.to_be_bytes());
                // edge_data_len (2 bytes) + edge_data
                let data_len = edge.edge_data.len() as u16;
                result.extend_from_slice(&data_len.to_be_bytes());
                result.extend_from_slice(&edge.edge_data);
            }
        } else {
            // v2 format: serialize each edge using V2 CompactEdgeRecord format
            for edge in &self.edges {
                let edge_bytes = edge.serialize();
                result.extend_from_slice(&edge_bytes);
            }
        }

        Ok(result)
    }

    /// Deserialize from bytes
    /// Format v2: [version: 1 byte] [src: 8 bytes] [dir: 1 byte] [edge_count: 4 bytes] [edges...]
    /// Format v1: [version: 1 byte] [edge_count: 4 bytes] [edges...]  (src=0, dir=Outgoing)
    pub fn deserialize(bytes: &[u8], page_id: u64) -> NativeResult<Self> {
        if bytes.len() < 5 {
            return Err(NativeBackendError::DeserializationError {
                context: "Edge cluster bytes too short".to_string(),
            });
        }

        let format_version = bytes[0];

        if format_version > 3 {
            return Err(NativeBackendError::DeserializationError {
                context: format!("Unknown edge cluster format version: {}", format_version),
            });
        }

        let mut pos = 1;

        // v2: read src and direction from serialized data
        let (src, direction) = if format_version >= 2 {
            if bytes.len() < 1 + 8 + 1 {
                return Err(NativeBackendError::DeserializationError {
                    context: "Edge cluster v2 header too short".to_string(),
                });
            }
            let src = i64::from_be_bytes(
                bytes[pos..pos + 8]
                    .try_into()
                    .expect("bounds checked above"),
            );
            pos += 8;
            let dir_byte = bytes[pos];
            pos += 1;
            let direction = if dir_byte == 1 {
                Direction::Incoming
            } else {
                Direction::Outgoing
            };
            (src, direction)
        } else {
            // v1: no src/direction in serialized data (legacy)
            (0, Direction::Outgoing)
        };

        // Read edge count
        if pos + 4 > bytes.len() {
            return Err(NativeBackendError::DeserializationError {
                context: "Edge cluster truncated at edge count".to_string(),
            });
        }
        let count = u32::from_be_bytes([bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]])
            as usize;
        pos += 4;

        let mut edges = Vec::with_capacity(count);

        // v3 format: delta-compressed edge IDs
        if format_version >= 3 {
            // Check compression flag (1 byte)
            if pos >= bytes.len() {
                return Err(NativeBackendError::DeserializationError {
                    context: "Missing compression flag".to_string(),
                });
            }
            let compressed_flag = bytes[pos];
            pos += 1;

            if compressed_flag == 1 {
                // Read compressed IDs
                if pos + 4 > bytes.len() {
                    return Err(NativeBackendError::DeserializationError {
                        context: "Missing compressed ID length".to_string(),
                    });
                }
                let compressed_len = u32::from_be_bytes([
                    bytes[pos],
                    bytes[pos + 1],
                    bytes[pos + 2],
                    bytes[pos + 3],
                ]) as usize;
                pos += 4;

                if pos + compressed_len > bytes.len() {
                    return Err(NativeBackendError::DeserializationError {
                        context: "Compressed IDs truncated".to_string(),
                    });
                }
                let compressed_data = &bytes[pos..pos + compressed_len];
                pos += compressed_len;

                // Decompress neighbor IDs
                let neighbor_ids = decompress_edge_ids(compressed_data, count).map_err(|e| {
                    NativeBackendError::DeserializationError {
                        context: format!("Failed to decompress edge IDs: {}", e),
                    }
                })?;

                // Read edge metadata for each ID
                for neighbor_id in neighbor_ids {
                    if pos + 4 > bytes.len() {
                        return Err(NativeBackendError::DeserializationError {
                            context: "Edge metadata truncated".to_string(),
                        });
                    }

                    let type_offset = u16::from_be_bytes(
                        bytes[pos..pos + 2]
                            .try_into()
                            .expect("bounds checked above"),
                    );
                    pos += 2;

                    let data_len = u16::from_be_bytes(
                        bytes[pos..pos + 2]
                            .try_into()
                            .expect("bounds checked above"),
                    ) as usize;
                    pos += 2;

                    let edge_data = if data_len > 0 {
                        if pos + data_len > bytes.len() {
                            return Err(NativeBackendError::DeserializationError {
                                context: "Edge data truncated".to_string(),
                            });
                        }
                        let data = bytes[pos..pos + data_len].to_vec();
                        pos += data_len;
                        data
                    } else {
                        Vec::new()
                    };

                    edges.push(CompactEdgeRecord::new(neighbor_id, type_offset, edge_data));
                }
            } else {
                // Uncompressed v3 - shouldn't happen but handle gracefully
                // Fall through to v2 format handling
            }
        }

        // v1/v2 format: deserialize each edge
        // CompactEdgeRecord format: [neighbor_id: 8 bytes] [type_offset: 2 bytes] [data_len: 2 bytes] [data: variable]
        if edges.is_empty() {
            for _ in 0..count {
                if pos + 12 > bytes.len() {
                    return Err(NativeBackendError::DeserializationError {
                        context: "Edge data truncated".to_string(),
                    });
                }

                let neighbor_id = i64::from_be_bytes(
                    bytes[pos..pos + 8]
                        .try_into()
                        .expect("bounds checked above"),
                );
                pos += 8;

                let type_offset = u16::from_be_bytes(
                    bytes[pos..pos + 2]
                        .try_into()
                        .expect("bounds checked above"),
                );
                pos += 2;

                let data_len = u16::from_be_bytes(
                    bytes[pos..pos + 2]
                        .try_into()
                        .expect("bounds checked above"),
                ) as usize;
                pos += 2;

                let edge_data = if data_len > 0 {
                    if pos + data_len > bytes.len() {
                        return Err(NativeBackendError::DeserializationError {
                            context: "Edge data truncated".to_string(),
                        });
                    }
                    bytes[pos..pos + data_len].to_vec()
                } else {
                    Vec::new()
                };
                pos += data_len;

                edges.push(CompactEdgeRecord::new(neighbor_id, type_offset, edge_data));
            }
        }

        Ok(Self {
            src,
            edges,
            direction,
            format_version,
            page_id,
        })
    }
}

pub(crate) fn encode_edge_cluster_pages(
    cluster_bytes: &[u8],
    page_size: usize,
    page_ids: &[u64],
) -> NativeResult<Vec<Vec<u8>>> {
    page_codec_support::encode_edge_cluster_pages(cluster_bytes, page_size, page_ids)
}

pub(crate) fn decode_edge_cluster_page_header(page: &[u8]) -> Option<(usize, u64)> {
    page_codec_support::decode_edge_cluster_page_header(page)
}

pub(crate) fn encode_packed_edge_page(
    page_size: usize,
    entries: &[((i64, Direction), Vec<u8>)],
) -> NativeResult<Vec<u8>> {
    page_codec_support::encode_packed_edge_page(page_size, entries)
}

pub(crate) fn decode_packed_edge_page(
    page: &[u8],
    src: i64,
    dir: Direction,
) -> NativeResult<Option<Vec<u8>>> {
    page_codec_support::decode_packed_edge_page(page, src, dir)
}
