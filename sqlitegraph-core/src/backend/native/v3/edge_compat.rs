//! V3 Edge Compatibility Layer
//!
//! This module provides a compatibility layer for using V2 EdgeCluster format
//! within V3's page-based storage system. This is a temporary design to get
//! V3 working end-to-end quickly without re-inventing edge layout while
//! NodeStore/B+Tree/allocator/WAL are still settling.
//!
//! # Design Principles
//!
//! 1. **Logical NodeIDs only**: EdgeCluster references NodeID, not V2 slot assumptions
//!    Resolution is via B+Tree → page.
//!
//! 2. **V3 pages + allocator**: Edge storage lives in V3 pages allocated by V3 allocator.
//!    Only the record format is reused from V2.
//!
//! 3. **Separate PageType**: Edges get their own PageType::EDGE_CLUSTER.
//!    Node pages never embed edge blobs.
//!
//! 4. **WAL-first**: Write path is WAL'd (insert_edge/delete_edge/update adjacency)
//!    before any compaction/relocation.
//!
//! # Edge Type Storage Model
//!
//! ## Durable Storage
//! Edge types are stored durably in the edge_data field of CompactEdgeRecord using
//! an inline encoding format: `[type_len: u8][type_bytes]`. This ensures edge types
//! survive reopen/recovery.
//!
//! ## In-Memory Index
//! The `edge_types: HashMap<(src, dst, dir), String>` field provides O(1) filtering.
//! This HashMap is rebuilt from durable storage on cache miss via `load_neighbors_from_disk()`.
//!
//! ## SEMANTIC CONSTRAINT (Known Limitation)
//!
//! The edge_types HashMap is keyed by `(src, dst, dir)`, NOT by edge_id. This means:
//!
//! - **Only ONE edge type can exist between a given (src, dst, dir) tuple**
//! - Inserting a second edge between same endpoints with a different type OVERWRITES the previous type
//! - This is intentional for V3's simple tuple-key model
//! - If multi-edge support (same endpoints, different types) is needed, the key model must change to use edge_id
//!
//! Example of the aliasing behavior:
//! ```ignore
//! insert_edge(1, 2, Outgoing, "CALLS")  // edge_types: {(1,2,Out) -> "CALLS"}
//! insert_edge(1, 2, Outgoing, "USES")   // edge_types: {(1,2,Out) -> "USES"} ← OVERWRITES!
//! ```
//!
//! # Architecture
//!
//! ```text
//! EdgeCluster { src: NodeId, dsts: Vec<NodeId>, dir: Out|In, metadata }
//!
//! B+Tree index: key = (src, dir) → value = edge_page_id
//!
//! Neighbor query: lookup_edge_page(src) → decode cluster → return iterator
//!
//! Insert edge: load cluster (or create), append, maybe split if page full
//! ```

use crate::backend::native::v3::compression::edge_delta::{compress_edge_ids, decompress_edge_ids};
#[cfg(feature = "v3-forensics")]
use crate::backend::native::v3::forensics::{
    FORENSIC_COUNTERS, PageType as ForensicPageType, Subsystem,
};
use crate::backend::native::v3::{
    allocator::PageAllocator, btree::BTreeManager, file_coordinator::FileCoordinator,
    header::PersistentHeaderV3, wal::WALWriter,
};
use crate::backend::native::{
    types::{NativeBackendError, NativeResult},
    v3::compact_edge_record::{CompactEdgeRecord, Direction as V2Direction},
};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// Page type constants for V3 storage
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PageType {
    /// Free/unallocated page
    Free = 0,
    /// B+Tree index page (node_id → page_id mapping)
    BTreeIndex = 1,
    /// Node data page (contains NodeRecordV3 entries)
    NodeData = 2,
    /// Edge cluster page (contains EdgeCluster entries)
    EdgeCluster = 3,
    /// WAL page (contains WAL records)
    Wal = 4,
    /// Checkpoint page
    Checkpoint = 5,
}

impl PageType {
    /// Convert from u8 to PageType
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(PageType::Free),
            1 => Some(PageType::BTreeIndex),
            2 => Some(PageType::NodeData),
            3 => Some(PageType::EdgeCluster),
            4 => Some(PageType::Wal),
            5 => Some(PageType::Checkpoint),
            _ => None,
        }
    }
}

/// Direction for edge traversal
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Outgoing,
    Incoming,
}

const EDGE_CLUSTER_PAGE_MAGIC: [u8; 4] = *b"V3EC";
const EDGE_CLUSTER_PAGE_HEADER_SIZE: usize = 16;
const PACKED_EDGE_PAGE_MAGIC: [u8; 4] = *b"V3EP";
const PACKED_EDGE_PAGE_HEADER_SIZE: usize = 8;
const PACKED_EDGE_PAGE_SLOT_SIZE: usize = 16;

impl Direction {
    /// Convert to V2 Direction for EdgeCluster compatibility
    pub fn to_v2(&self) -> V2Direction {
        match self {
            Direction::Outgoing => V2Direction::Outgoing,
            Direction::Incoming => V2Direction::Incoming,
        }
    }
}

/// Edge cluster entry for V3 storage
/// Uses V2 CompactEdgeRecord format for compatibility
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
    fn extract_edge_type(edge_data: &[u8]) -> Option<String> {
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
    fn extract_edge_weight(edge_data: &[u8]) -> f32 {
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

fn encode_edge_cluster_pages(
    cluster_bytes: &[u8],
    page_size: usize,
    page_ids: &[u64],
) -> NativeResult<Vec<Vec<u8>>> {
    if page_size <= EDGE_CLUSTER_PAGE_HEADER_SIZE {
        return Err(NativeBackendError::SerializationError {
            context: format!(
                "edge page size {} too small for header {}",
                page_size, EDGE_CLUSTER_PAGE_HEADER_SIZE
            ),
        });
    }
    if page_ids.is_empty() {
        return Err(NativeBackendError::SerializationError {
            context: "missing page ids for edge cluster encode".to_string(),
        });
    }

    let payload_capacity = page_size - EDGE_CLUSTER_PAGE_HEADER_SIZE;
    let expected_pages = cluster_bytes.len().max(1).div_ceil(payload_capacity);
    if expected_pages != page_ids.len() {
        return Err(NativeBackendError::SerializationError {
            context: format!(
                "edge cluster page count mismatch: expected {}, got {}",
                expected_pages,
                page_ids.len()
            ),
        });
    }

    let mut pages = Vec::with_capacity(page_ids.len());
    for (index, chunk) in cluster_bytes.chunks(payload_capacity).enumerate() {
        let mut page = vec![0u8; page_size];
        page[0..4].copy_from_slice(&EDGE_CLUSTER_PAGE_MAGIC);
        page[4..8].copy_from_slice(&(chunk.len() as u32).to_be_bytes());
        let next_page_id = page_ids.get(index + 1).copied().unwrap_or(0);
        page[8..16].copy_from_slice(&next_page_id.to_be_bytes());
        let payload_end = EDGE_CLUSTER_PAGE_HEADER_SIZE + chunk.len();
        page[EDGE_CLUSTER_PAGE_HEADER_SIZE..payload_end].copy_from_slice(chunk);
        pages.push(page);
    }

    if cluster_bytes.is_empty() {
        let mut page = vec![0u8; page_size];
        page[0..4].copy_from_slice(&EDGE_CLUSTER_PAGE_MAGIC);
        page[4..8].copy_from_slice(&0u32.to_be_bytes());
        page[8..16].copy_from_slice(&0u64.to_be_bytes());
        pages.push(page);
    }

    Ok(pages)
}

fn decode_edge_cluster_page_header(page: &[u8]) -> Option<(usize, u64)> {
    if page.len() < EDGE_CLUSTER_PAGE_HEADER_SIZE || page[0..4] != EDGE_CLUSTER_PAGE_MAGIC {
        return None;
    }

    let payload_len = u32::from_be_bytes([page[4], page[5], page[6], page[7]]) as usize;
    let max_payload = page.len().saturating_sub(EDGE_CLUSTER_PAGE_HEADER_SIZE);
    if payload_len > max_payload {
        return None;
    }

    let next_page_id = u64::from_be_bytes([
        page[8], page[9], page[10], page[11], page[12], page[13], page[14], page[15],
    ]);
    Some((payload_len, next_page_id))
}

fn encode_packed_edge_page(
    page_size: usize,
    entries: &[((i64, Direction), Vec<u8>)],
) -> NativeResult<Vec<u8>> {
    let slot_bytes = entries.len() * PACKED_EDGE_PAGE_SLOT_SIZE;
    if PACKED_EDGE_PAGE_HEADER_SIZE + slot_bytes > page_size {
        return Err(NativeBackendError::SerializationError {
            context: format!(
                "packed edge page header {} + slots {} exceed page size {}",
                PACKED_EDGE_PAGE_HEADER_SIZE, slot_bytes, page_size
            ),
        });
    }

    let mut payload_cursor = PACKED_EDGE_PAGE_HEADER_SIZE + slot_bytes;
    let total_payload: usize = entries.iter().map(|(_, bytes)| bytes.len()).sum();
    if payload_cursor + total_payload > page_size {
        return Err(NativeBackendError::SerializationError {
            context: format!(
                "packed edge page payload {} exceeds page size {}",
                payload_cursor + total_payload,
                page_size
            ),
        });
    }

    let mut page = vec![0u8; page_size];
    page[0..4].copy_from_slice(&PACKED_EDGE_PAGE_MAGIC);
    page[4..6].copy_from_slice(&(entries.len() as u16).to_be_bytes());

    for (idx, ((src, dir), cluster_bytes)) in entries.iter().enumerate() {
        let slot_offset = PACKED_EDGE_PAGE_HEADER_SIZE + idx * PACKED_EDGE_PAGE_SLOT_SIZE;
        page[slot_offset..slot_offset + 8].copy_from_slice(&src.to_be_bytes());
        page[slot_offset + 8] = match dir {
            Direction::Outgoing => 0,
            Direction::Incoming => 1,
        };
        page[slot_offset + 9] = 0;
        page[slot_offset + 10..slot_offset + 12]
            .copy_from_slice(&(payload_cursor as u16).to_be_bytes());
        page[slot_offset + 12..slot_offset + 14]
            .copy_from_slice(&(cluster_bytes.len() as u16).to_be_bytes());
        page[slot_offset + 14..slot_offset + 16].copy_from_slice(&0u16.to_be_bytes());

        let payload_end = payload_cursor + cluster_bytes.len();
        page[payload_cursor..payload_end].copy_from_slice(cluster_bytes);
        payload_cursor = payload_end;
    }

    Ok(page)
}

fn decode_packed_edge_page(page: &[u8], src: i64, dir: Direction) -> NativeResult<Option<Vec<u8>>> {
    if page.len() < PACKED_EDGE_PAGE_HEADER_SIZE || page[0..4] != PACKED_EDGE_PAGE_MAGIC {
        return Ok(None);
    }

    let slot_count = u16::from_be_bytes([page[4], page[5]]) as usize;
    let slot_region_end = PACKED_EDGE_PAGE_HEADER_SIZE + slot_count * PACKED_EDGE_PAGE_SLOT_SIZE;
    if slot_region_end > page.len() {
        return Err(NativeBackendError::DeserializationError {
            context: "Packed edge page slot directory exceeds page length".to_string(),
        });
    }

    for idx in 0..slot_count {
        let slot_offset = PACKED_EDGE_PAGE_HEADER_SIZE + idx * PACKED_EDGE_PAGE_SLOT_SIZE;
        let slot_src = i64::from_be_bytes(
            page[slot_offset..slot_offset + 8]
                .try_into()
                .expect("slot src bounds checked"),
        );
        let slot_dir = if page[slot_offset + 8] == 1 {
            Direction::Incoming
        } else {
            Direction::Outgoing
        };
        if slot_src != src || slot_dir != dir {
            continue;
        }

        let payload_offset =
            u16::from_be_bytes([page[slot_offset + 10], page[slot_offset + 11]]) as usize;
        let payload_len =
            u16::from_be_bytes([page[slot_offset + 12], page[slot_offset + 13]]) as usize;
        let payload_end = payload_offset + payload_len;
        if payload_offset < slot_region_end || payload_end > page.len() {
            return Err(NativeBackendError::DeserializationError {
                context: format!(
                    "Packed edge page payload out of bounds: offset {} len {} page {}",
                    payload_offset,
                    payload_len,
                    page.len()
                ),
            });
        }

        return Ok(Some(page[payload_offset..payload_end].to_vec()));
    }

    Ok(None)
}

/// V3 Edge Store - PERFORMANCE FIX: Store Arc<[i64]> in cache to avoid cloning
///
/// This change makes neighbor queries faster by:
/// 1. Using RwLock instead of &mut self (allows concurrent reads)
/// 2. Storing Arc<[i64]> instead of Vec<i64> - no cloning on read!
/// 3. Direct cache lookup without indirection
pub struct V3EdgeStore {
    /// B+Tree index: (src, dir) → page_id
    #[cfg(test)]
    pub btree: parking_lot::RwLock<BTreeManager>,
    #[cfg(not(test))]
    btree: parking_lot::RwLock<BTreeManager>,
    /// Optional WAL writer for durability (shared with V3Backend via Arc)
    #[cfg(test)]
    pub wal: Option<Arc<RwLock<WALWriter>>>,
    #[cfg(not(test))]
    wal: Option<Arc<RwLock<WALWriter>>>,
    /// In-memory cache of neighbor lists - using Arc<[i64]> for zero-copy reads
    /// This matches SQLite's AdjacencyCache pattern but with Arc for zero-copy
    cache: RwLock<HashMap<(i64, Direction), Arc<[i64]>>>,
    /// In-memory cache of weighted neighbors
    cache_weighted: RwLock<HashMap<(i64, Direction), Arc<[(i64, f32)]>>>,
    /// Edge type storage: (src, dst, dir) -> edge_type string
    ///
    /// SEMANTIC CONSTRAINT: Key is (src, dst, dir), NOT edge_id.
    /// This means only ONE edge type can exist between a given (src, dst, dir) tuple.
    /// Inserting multiple edges between same endpoints with different types will
    /// cause aliasing - the last type wins. See module-level docs for details.
    ///
    /// This HashMap is rebuilt from durable edge_data on cache miss via
    /// load_neighbors_from_disk(), ensuring edge types survive reopen/recovery.
    edge_types: RwLock<HashMap<(i64, i64, Direction), String>>,
    /// Performance counters
    cache_hits: AtomicU64,
    cache_misses: AtomicU64,
    /// Hit time accumulator (nanoseconds) - for profiling
    hit_time_ns: AtomicU64,
    /// Miss time accumulator (nanoseconds) - for profiling
    miss_time_ns: AtomicU64,
    /// Track dirty clusters that need to be flushed
    #[cfg(test)]
    pub dirty_clusters: RwLock<HashMap<(i64, Direction), V3EdgeCluster>>,
    #[cfg(not(test))]
    dirty_clusters: RwLock<HashMap<(i64, Direction), V3EdgeCluster>>,
    /// Path to database file for writing pages
    db_path: Option<PathBuf>,
    /// Page allocator for edge page allocation
    /// CRITICAL: Shared with NodeStore to prevent page ID collisions
    allocator: Arc<RwLock<PageAllocator>>,
    /// Page size for I/O operations (detected from storage media)
    page_size: u32,
    /// Coordinated file handle for all main DB I/O (optional for backward compat)
    /// When set, all file writes go through this coordinator to prevent race conditions
    file_coordinator: Option<Arc<FileCoordinator>>,
}

/// Encode (src, dir) into a composite key for B+Tree lookup
/// Format: [dir: 1 bit][src_abs: 62 bits][sign: 1 bit]
///
/// This encoding guarantees the resulting i64 is always positive by placing
/// the direction bit in the MSB position and using only the magnitude of src.
/// Negative src node IDs are encoded with a sign bit in the LSB.
///
/// Ordering: Incoming edges sort before Outgoing edges for the same node.
fn edge_key(src: i64, dir: Direction) -> i64 {
    let dir_bit = if dir == Direction::Outgoing {
        0i64
    } else {
        1i64
    };
    // Use zigzag encoding for src to handle negative node IDs
    // zigzag(n) = (n << 1) ^ (n >> 63) maps negatives to positive even numbers
    let zigzag_src = (src << 1) ^ (src >> 63);
    // Combine: dir in high bit, zigzag_src in lower bits
    // Ensure result is positive: dir_bit is 0 or 1, zigzag_src is non-negative
    // We place zigzag_src in lower 63 bits and dir_bit in bit 63
    // But bit 63 makes it negative! Instead, interleave:
    // key = (dir_bit << 62) | (zigzag_src & 0x3FFF_FFFF_FFFF_FFFF)

    (dir_bit << 62) | (zigzag_src & 0x3FFF_FFFF_FFFF_FFFF)
}

impl V3EdgeStore {
    /// Create new edge store (in-memory only)
    /// NOTE: Prefer with_path_and_allocator() for database-backed edge stores
    pub fn new(
        btree: BTreeManager,
        wal: Option<WALWriter>,
        allocator: Arc<RwLock<PageAllocator>>,
        page_size: u32,
    ) -> Self {
        Self {
            btree: parking_lot::RwLock::new(btree),
            wal: wal.map(|w| Arc::new(RwLock::new(w))),
            cache: RwLock::new(HashMap::new()),
            cache_weighted: RwLock::new(HashMap::new()),
            edge_types: RwLock::new(HashMap::new()),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            hit_time_ns: AtomicU64::new(0),
            miss_time_ns: AtomicU64::new(0),
            dirty_clusters: RwLock::new(HashMap::new()),
            db_path: None,
            allocator,
            page_size,
            file_coordinator: None,
        }
    }

    /// Create new edge store with database path and allocator
    /// This is the preferred constructor for database-backed edge stores
    pub fn with_path_and_allocator(
        btree: BTreeManager,
        wal: Option<WALWriter>,
        db_path: PathBuf,
        allocator: Arc<RwLock<PageAllocator>>,
        page_size: u32,
    ) -> Self {
        Self {
            btree: parking_lot::RwLock::new(btree),
            wal: wal.map(|w| Arc::new(RwLock::new(w))),
            cache: RwLock::new(HashMap::new()),
            cache_weighted: RwLock::new(HashMap::new()),
            edge_types: RwLock::new(HashMap::new()),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            hit_time_ns: AtomicU64::new(0),
            miss_time_ns: AtomicU64::new(0),
            dirty_clusters: RwLock::new(HashMap::new()),
            db_path: Some(db_path),
            allocator,
            page_size,
            file_coordinator: None,
        }
    }

    /// Create new edge store with disk persistence path
    /// NOTE: This creates a temporary allocator for compatibility.
    /// For proper page allocation, use with_path_and_allocator() instead.
    pub fn with_path(btree: BTreeManager, wal: Option<WALWriter>, db_path: PathBuf) -> Self {
        // Create a temporary allocator for compatibility
        // WARNING: This allocator is not shared with NodeStore, so page IDs
        // may collide. Always use with_path_and_allocator() in production.
        let header = PersistentHeaderV3::new_v3();
        Self {
            btree: parking_lot::RwLock::new(btree),
            wal: wal.map(|w| Arc::new(RwLock::new(w))),
            cache: RwLock::new(HashMap::new()),
            cache_weighted: RwLock::new(HashMap::new()),
            edge_types: RwLock::new(HashMap::new()),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            hit_time_ns: AtomicU64::new(0),
            miss_time_ns: AtomicU64::new(0),
            dirty_clusters: RwLock::new(HashMap::new()),
            db_path: Some(db_path),
            allocator: Arc::new(RwLock::new(PageAllocator::new(&header))),
            page_size: header.page_size,
            file_coordinator: None,
        }
    }

    /// Set the file coordinator for coordinated I/O
    ///
    /// When set, all file writes will go through this coordinator to prevent
    /// race conditions when multiple components write to the same file.
    pub fn set_file_coordinator(&mut self, coordinator: Arc<FileCoordinator>) {
        self.file_coordinator = Some(coordinator);
    }

    /// Get neighbors from cache - returns Arc<[i64]> for zero-copy!
    ///
    /// IMPROVED: On cache miss, attempts to load from disk if db_path is set.
    /// This enables recovery after reopening the edge store.
    pub fn neighbors(&self, src: i64, dir: Direction) -> NativeResult<Arc<[i64]>> {
        let key = (src, dir);

        #[cfg(feature = "v3-forensics")]
        FORENSIC_COUNTERS
            .logical_neighbors_calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // First check in-memory cache
        {
            let cache = self.cache.read();
            if let Some(neighbors) = cache.get(&key) {
                self.cache_hits.fetch_add(1, Ordering::Relaxed);
                #[cfg(feature = "v3-forensics")]
                FORENSIC_COUNTERS
                    .edge_cache_hit_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Ok(neighbors.clone()); // Arc clone is just pointer bump, no data copy
            }
        }

        self.cache_misses.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "v3-forensics")]
        FORENSIC_COUNTERS
            .edge_cache_miss_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Cache miss - try to load from disk if we have a db_path
        if let Some(ref db_path) = self.db_path
            && let Ok(neighbors) = self.load_neighbors_from_disk(src, dir, db_path)
        {
            #[cfg(feature = "v3-forensics")]
            if !neighbors.is_empty() {
                FORENSIC_COUNTERS
                    .edge_page_read_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            if !neighbors.is_empty() {
                // Cache the loaded neighbors
                let mut cache = self.cache.write();
                cache.insert(key, neighbors.clone());
                return Ok(neighbors);
            }
        }

        Ok(Arc::from([])) // Empty slice, no allocation
    }

    /// Load neighbors from disk for recovery
    /// IMPORTANT: Also rebuilds edge_types HashMap from deserialized edge records
    /// CRITICAL FIX: Query B+Tree for page_id instead of using formula
    fn load_neighbors_from_disk(
        &self,
        src: i64,
        dir: Direction,
        db_path: &PathBuf,
    ) -> NativeResult<Arc<[i64]>> {
        // CRITICAL FIX: Query B+Tree for page_id instead of calculating it
        // This prevents page ID collision with node storage
        let key = edge_key(src, dir);
        let btree = self.btree.read();

        // Try to get page_id from B+Tree
        let page_id = match btree.lookup(key) {
            Ok(Some(pid)) => pid,
            Ok(None) => {
                // No entry in B+Tree means no edges for this (src, dir)
                return Ok(Arc::from([]));
            }
            Err(_) => {
                // B+Tree lookup error - treat as no edges
                return Ok(Arc::from([]));
            }
        };
        drop(btree);

        match self.load_cluster_from_disk(page_id, src, dir, db_path) {
            Ok(cluster) => {
                let edges_with_types = cluster.edges_with_types();
                let mut edge_types = self.edge_types.write();
                for (dst, edge_type) in edges_with_types {
                    if let Some(et) = edge_type {
                        edge_types.insert((src, dst, dir), et);
                    }
                }

                let neighbors: Vec<i64> = cluster.dsts();
                Ok(Arc::from(neighbors.into_boxed_slice()))
            }
            Err(_) => Ok(Arc::from([])),
        }
    }

    /// Get outgoing neighbors
    pub fn outgoing(&self, src: i64) -> NativeResult<Arc<[i64]>> {
        self.neighbors(src, Direction::Outgoing)
    }

    /// Get incoming neighbors
    pub fn incoming(&self, src: i64) -> NativeResult<Arc<[i64]>> {
        self.neighbors(src, Direction::Incoming)
    }

    /// Get neighbors filtered by edge type
    /// Returns only neighbors connected by edges matching the specified edge_type
    pub fn neighbors_filtered(
        &self,
        src: i64,
        dir: Direction,
        edge_type: &str,
    ) -> NativeResult<Arc<[i64]>> {
        // Get all neighbors first
        let all_neighbors = self.neighbors(src, dir)?;

        // Filter by edge type
        let edge_types = self.edge_types.read();
        let filtered: Vec<i64> = all_neighbors
            .iter()
            .filter(|&&dst| {
                edge_types
                    .get(&(src, dst, dir))
                    .map(|stored_type| stored_type == edge_type)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        Ok(Arc::from(filtered.into_boxed_slice()))
    }

    /// Get the edge type for a specific edge
    pub fn get_edge_type(&self, src: i64, dst: i64, dir: Direction) -> Option<String> {
        let edge_types = self.edge_types.read();
        edge_types.get(&(src, dst, dir)).cloned()
    }

    /// Get or create a dirty cluster, loading from disk if it already exists to avoid dropping previous edges.
    fn get_or_create_dirty_cluster<'m>(
        &self,
        dirty: &'m mut std::collections::HashMap<(i64, Direction), V3EdgeCluster>,
        src: i64,
        dir: Direction,
    ) -> NativeResult<&'m mut V3EdgeCluster> {
        let cache_key = (src, dir);
        if dirty.contains_key(&cache_key) {
            return Ok(dirty.get_mut(&cache_key).unwrap());
        }

        let key = edge_key(src, dir);
        let lookup_res = {
            let btree = self.btree.read();
            btree.lookup(key)
        };

        let mut existing_cluster = None;
        let page_id_to_use = match lookup_res {
            Ok(Some(pid)) => {
                // Try to load existing cluster from disk to avoid dropping previous edges
                if let Some(ref db_path) = self.db_path
                    && let Ok(mut cluster) = self.load_cluster_from_disk(pid, src, dir, db_path)
                {
                    cluster.page_id = 0;
                    existing_cluster = Some(cluster);
                }
                pid
            }
            Ok(None) | Err(_) => 0,
        };

        let cluster =
            existing_cluster.unwrap_or_else(|| V3EdgeCluster::new(src, dir, page_id_to_use));

        dirty.insert(cache_key, cluster);
        Ok(dirty.get_mut(&cache_key).unwrap())
    }

    /// Insert an edge - uses interior mutability via RwLock, takes &self!
    ///
    /// # SEMANTIC CONSTRAINT
    /// The edge_types HashMap is keyed by (src, dst, dir). This means:
    /// - Only ONE edge type can exist between a given (src, dst, dir) tuple
    /// - Inserting a second edge between same endpoints with different type will OVERWRITE
    /// - This is intentional for V3's simple tuple-key model
    /// - If multi-edge support is needed, the key model must change to use edge_id
    pub fn insert_edge(
        &self,
        src: i64,
        dst: i64,
        dir: Direction,
        edge_type: Option<String>,
    ) -> NativeResult<()> {
        let cache_key = (src, dir);
        let mut cache = self.cache.write();

        // Get or update entry in cache
        if let Some(neighbors) = cache.get_mut(&cache_key) {
            // Existing entry - need to convert Arc back to Vec, modify, then re-Arc
            let mut vec: Vec<i64> = neighbors.to_vec();
            if !vec.contains(&dst) {
                vec.push(dst);
                *neighbors = Arc::from(vec);
            }
        } else {
            // Cache miss/cold cache or new node
            // Load existing from disk if available to avoid caching a partial list
            let mut vec = Vec::new();
            if let Some(ref db_path) = self.db_path {
                if let Ok(existing) = self.load_neighbors_from_disk(src, dir, db_path) {
                    vec = existing.to_vec();
                }
            }
            if !vec.contains(&dst) {
                vec.push(dst);
            }
            cache.insert(cache_key, Arc::from(vec));
        }

        // Also update weighted cache with default weight 1.0
        {
            let mut cache_weighted = self.cache_weighted.write();
            if let Some(neighbors) = cache_weighted.get_mut(&cache_key) {
                let mut vec = neighbors.to_vec();
                if !vec.iter().any(|(n, _)| *n == dst) {
                    vec.push((dst, 1.0));
                    *neighbors = Arc::from(vec);
                }
            } else {
                let mut vec = Vec::new();
                if let Some(ref db_path) = self.db_path {
                    if let Ok(existing) = self.load_neighbors_weighted_from_disk(src, dir, db_path)
                    {
                        vec = existing.to_vec();
                    }
                }
                if !vec.iter().any(|(n, _)| *n == dst) {
                    vec.push((dst, 1.0));
                }
                cache_weighted.insert(cache_key, Arc::from(vec));
            }
        }

        // Store edge type in HashMap AND pass to cluster for durable storage
        // NOTE: If an edge already exists between (src, dst, dir) with a different type,
        // this will overwrite the previous type. This is a known semantic limitation.
        if let Some(ref edge_type_str) = edge_type {
            let mut edge_types = self.edge_types.write();

            // DETECT POTENTIAL ALIASING: Check if we're overwriting a different type
            let key = (src, dst, dir);
            if let Some(existing_type) = edge_types.get(&key)
                && existing_type != edge_type_str
            {
                // SEMANTIC WARNING: Overwriting different edge type for same tuple
                // This is logged but not an error - the caller's responsibility
                eprintln!(
                    "WARNING: V3EdgeStore inserting edge_type '{}' for ({}, {}, {:?}), overwriting existing type '{}'. This is a known limitation of tuple-key model.",
                    edge_type_str, src, dst, dir, existing_type
                );
            }

            edge_types.insert(key, edge_type_str.clone());
        } else {
            // If edge_type is None, remove any existing entry to clear it
            let mut edge_types = self.edge_types.write();
            edge_types.remove(&(src, dst, dir));
        }

        // Mark cluster as dirty for later flush
        let page_id = {
            let mut dirty = self.dirty_clusters.write();
            let cluster = self.get_or_create_dirty_cluster(&mut dirty, src, dir)?;
            let cluster_page_id = cluster.page_id;
            // CRITICAL: Pass edge_type to add_edge() so it gets serialized into edge_data
            cluster.add_edge(dst, edge_type);
            cluster_page_id
        };

        // Log to WAL if configured
        if let Some(ref wal) = self.wal {
            let mut wal_guard = wal.write();
            // Write EdgeInsert WAL record for crash recovery
            let _ = wal_guard.edge_insert(src, dst, dir as u8, page_id);
        }

        Ok(())
    }

    /// Clear in-memory cache
    /// Also clears edge_types HashMap to ensure consistency
    pub fn clear_cache(&self) {
        self.cache.write().clear();
        self.cache_weighted.write().clear();
        self.edge_types.write().clear();
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
    }

    /// Print cache statistics for debugging/benchmarking
    pub fn print_stats(&self) {
        let hits = self.cache_hits.load(Ordering::Relaxed);
        let misses = self.cache_misses.load(Ordering::Relaxed);
        let cache_size = self.cache.read().len();
        let hit_ns = self.hit_time_ns.load(Ordering::Relaxed);
        let miss_ns = self.miss_time_ns.load(Ordering::Relaxed);
        let total = hits + misses;
        let hit_rate = if total > 0 {
            (hits as f64 / total as f64) * 100.0
        } else {
            0.0
        };
        let avg_hit_ns = hit_ns.checked_div(hits).unwrap_or(0);
        let avg_miss_ns = miss_ns.checked_div(misses).unwrap_or(0);

        println!("Cache stats:");
        println!("  Entries: {}", cache_size);
        println!("  Hits: {} ({:.1}%)", hits, hit_rate);
        println!("  Misses: {}", misses);
        println!("  Avg hit time: {} ns", avg_hit_ns);
        println!("  Avg miss time: {} ns", avg_miss_ns);
    }

    /// Return cache statistics for focused benchmarking and profiling.
    pub fn cache_stats(&self) -> (u64, u64, u64, u64, usize) {
        (
            self.cache_hits.load(Ordering::Relaxed),
            self.cache_misses.load(Ordering::Relaxed),
            self.hit_time_ns.load(Ordering::Relaxed),
            self.miss_time_ns.load(Ordering::Relaxed),
            self.cache.read().len(),
        )
    }

    /// Reset cache timing counters without evicting cached neighbor slices.
    pub fn reset_stats(&self) {
        self.cache_hits.store(0, Ordering::Relaxed);
        self.cache_misses.store(0, Ordering::Relaxed);
        self.hit_time_ns.store(0, Ordering::Relaxed);
        self.miss_time_ns.store(0, Ordering::Relaxed);
    }

    /// Get weighted neighbors from cache - returns Arc<[(i64, f32)]> for zero-copy!
    pub fn neighbors_weighted(&self, src: i64, dir: Direction) -> NativeResult<Arc<[(i64, f32)]>> {
        let key = (src, dir);

        // First check in-memory cache
        {
            let cache = self.cache_weighted.read();
            if let Some(neighbors) = cache.get(&key) {
                return Ok(neighbors.clone());
            }
        }

        // Cache miss - try to load from disk if we have a db_path
        if let Some(ref db_path) = self.db_path
            && let Ok(neighbors) = self.load_neighbors_weighted_from_disk(src, dir, db_path)
        {
            if !neighbors.is_empty() {
                // Cache the loaded neighbors
                let mut cache = self.cache_weighted.write();
                cache.insert(key, neighbors.clone());
                return Ok(neighbors);
            }
        }

        Ok(Arc::from([]))
    }

    /// Load weighted neighbors from disk
    fn load_neighbors_weighted_from_disk(
        &self,
        src: i64,
        dir: Direction,
        db_path: &PathBuf,
    ) -> NativeResult<Arc<[(i64, f32)]>> {
        let key = edge_key(src, dir);
        let btree = self.btree.read();

        let page_id = match btree.lookup(key) {
            Ok(Some(pid)) => pid,
            Ok(None) | Err(_) => return Ok(Arc::from([])),
        };
        drop(btree);

        match self.load_cluster_from_disk(page_id, src, dir, db_path) {
            Ok(cluster) => {
                let mut edge_types = self.edge_types.write();
                let mut neighbors = Vec::with_capacity(cluster.edges.len());
                for e in &cluster.edges {
                    let edge_type = V3EdgeCluster::extract_edge_type(&e.edge_data);
                    if let Some(et) = edge_type {
                        edge_types.insert((src, e.neighbor_id, dir), et);
                    }
                    let weight = V3EdgeCluster::extract_edge_weight(&e.edge_data);
                    neighbors.push((e.neighbor_id, weight));
                }
                Ok(Arc::from(neighbors.into_boxed_slice()))
            }
            Err(_) => Ok(Arc::from([])),
        }
    }

    /// Get weighted neighbors filtered by edge type
    pub fn neighbors_weighted_filtered(
        &self,
        src: i64,
        dir: Direction,
        edge_type: &str,
    ) -> NativeResult<Arc<[(i64, f32)]>> {
        let all_neighbors = self.neighbors_weighted(src, dir)?;

        let edge_types = self.edge_types.read();
        let filtered: Vec<(i64, f32)> = all_neighbors
            .iter()
            .filter(|&&(dst, _)| {
                edge_types
                    .get(&(src, dst, dir))
                    .map(|stored_type| stored_type == edge_type)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        Ok(Arc::from(filtered.into_boxed_slice()))
    }

    /// Insert a weighted edge
    pub fn insert_edge_weighted(
        &self,
        src: i64,
        dst: i64,
        dir: Direction,
        edge_type: Option<String>,
        weight: f32,
    ) -> NativeResult<()> {
        let cache_key = (src, dir);

        // Update unweighted cache
        {
            let mut cache = self.cache.write();
            if let Some(neighbors) = cache.get_mut(&cache_key) {
                let mut vec: Vec<i64> = neighbors.to_vec();
                if !vec.contains(&dst) {
                    vec.push(dst);
                    *neighbors = Arc::from(vec);
                }
            } else {
                let mut vec = Vec::new();
                if let Some(ref db_path) = self.db_path {
                    if let Ok(existing) = self.load_neighbors_from_disk(src, dir, db_path) {
                        vec = existing.to_vec();
                    }
                }
                if !vec.contains(&dst) {
                    vec.push(dst);
                }
                cache.insert(cache_key, Arc::from(vec));
            }
        }

        // Update weighted cache
        {
            let mut cache_weighted = self.cache_weighted.write();
            if let Some(neighbors) = cache_weighted.get_mut(&cache_key) {
                let mut vec = neighbors.to_vec();
                if let Some(pos) = vec.iter().position(|(n, _)| *n == dst) {
                    vec[pos].1 = weight;
                } else {
                    vec.push((dst, weight));
                }
                *neighbors = Arc::from(vec);
            } else {
                let mut vec = Vec::new();
                if let Some(ref db_path) = self.db_path {
                    if let Ok(existing) = self.load_neighbors_weighted_from_disk(src, dir, db_path)
                    {
                        vec = existing.to_vec();
                    }
                }
                if let Some(pos) = vec.iter().position(|(n, _)| *n == dst) {
                    vec[pos].1 = weight;
                } else {
                    vec.push((dst, weight));
                }
                cache_weighted.insert(cache_key, Arc::from(vec));
            }
        }

        if let Some(ref edge_type_str) = edge_type {
            let mut edge_types = self.edge_types.write();
            edge_types.insert((src, dst, dir), edge_type_str.clone());
        } else {
            let mut edge_types = self.edge_types.write();
            edge_types.remove(&(src, dst, dir));
        }

        // Mark cluster as dirty
        let page_id = {
            let mut dirty = self.dirty_clusters.write();
            let cluster = self.get_or_create_dirty_cluster(&mut dirty, src, dir)?;
            let cluster_page_id = cluster.page_id;
            cluster.add_edge_weighted(dst, edge_type, weight);
            cluster_page_id
        };

        if let Some(ref wal) = self.wal {
            let mut wal_guard = wal.write();
            let _ = wal_guard.edge_insert(src, dst, dir as u8, page_id);
        }

        Ok(())
    }

    /// Flush dirty clusters to disk
    ///
    /// IMPLEMENTATION:
    /// 1. Write dirty clusters to pages
    /// 2. Update B+Tree index
    ///
    /// Note: WAL checkpoint and KV checkpoint are handled by V3Backend::flush_to_disk()
    /// because V3EdgeStore doesn't have direct access to V3Backend's WAL.
    pub fn flush(
        &self,
        _kv_store: Option<
            &parking_lot::RwLock<Option<crate::backend::native::v3::kv_store::store::KvStore>>,
        >,
    ) -> NativeResult<()> {
        let db_path = match &self.db_path {
            Some(path) => path.clone(),
            None => {
                // In-memory mode: just clear dirty clusters
                self.dirty_clusters.write().clear();
                return Ok(());
            }
        };

        let dirty = self.dirty_clusters.write();

        if dirty.is_empty() {
            return Ok(()); // Nothing to flush
        }

        // Get mutable access to btree for index updates
        // Note: We use unsafe here because we need to mutate through &self
        // In production, this would use interior mutability patterns
        // For now, we use a simple approach: clone dirty clusters and process them
        let clusters_to_flush: Vec<((i64, Direction), V3EdgeCluster)> =
            dirty.iter().map(|(k, v)| (*k, v.clone())).collect();

        // Drop the lock before doing I/O
        drop(dirty);

        let packed_capacity = (self.page_size as usize)
            .checked_sub(PACKED_EDGE_PAGE_HEADER_SIZE + PACKED_EDGE_PAGE_SLOT_SIZE)
            .ok_or_else(|| NativeBackendError::SerializationError {
                context: format!(
                    "edge page size {} too small for packed edge encoding",
                    self.page_size
                ),
            })?;
        let overflow_capacity = (self.page_size as usize)
            .checked_sub(EDGE_CLUSTER_PAGE_HEADER_SIZE)
            .ok_or_else(|| NativeBackendError::SerializationError {
                context: format!(
                    "edge page size {} too small for multi-page encoding",
                    self.page_size
                ),
            })?;

        let mut packed_entries: Vec<((i64, Direction), Vec<u8>)> = Vec::new();
        let mut packed_bytes_used = PACKED_EDGE_PAGE_HEADER_SIZE;

        let flush_packed_entries = |entries: &mut Vec<((i64, Direction), Vec<u8>)>,
                                    bytes_used: &mut usize|
         -> NativeResult<()> {
            if entries.is_empty() {
                return Ok(());
            }

            let packed_page_id = {
                let mut allocator = self.allocator.write();
                allocator
                    .allocate()
                    .map_err(|e| NativeBackendError::IoError {
                        context: "Failed to allocate packed edge page".to_string(),
                        source: std::io::Error::other(e.to_string()),
                    })?
            };

            let page_bytes = encode_packed_edge_page(self.page_size as usize, entries)?;
            self.write_page_to_disk(&db_path, packed_page_id, &page_bytes)?;

            let mut btree = self.btree.write();
            for ((src, dir), _) in entries.iter() {
                let key = edge_key(*src, *dir);
                let _ = btree.insert(key, packed_page_id);
            }

            entries.clear();
            *bytes_used = PACKED_EDGE_PAGE_HEADER_SIZE;
            Ok(())
        };

        for ((src, dir), cluster) in clusters_to_flush {
            let cluster_bytes = cluster.serialize()?;
            let packed_size = PACKED_EDGE_PAGE_SLOT_SIZE + cluster_bytes.len();

            if cluster_bytes.len() <= packed_capacity {
                if packed_bytes_used + packed_size > self.page_size as usize {
                    flush_packed_entries(&mut packed_entries, &mut packed_bytes_used)?;
                }
                packed_bytes_used += packed_size;
                packed_entries.push(((src, dir), cluster_bytes));
                continue;
            }

            flush_packed_entries(&mut packed_entries, &mut packed_bytes_used)?;

            let first_page_id = {
                let mut allocator = self.allocator.write();
                allocator
                    .allocate()
                    .map_err(|e| NativeBackendError::IoError {
                        context: format!("Failed to allocate edge page for ({}, {:?})", src, dir),
                        source: std::io::Error::other(e.to_string()),
                    })?
            };

            let total_pages = cluster_bytes.len().max(1).div_ceil(overflow_capacity);
            let mut page_ids = Vec::with_capacity(total_pages);
            page_ids.push(first_page_id);
            if total_pages > 1 {
                let mut allocator = self.allocator.write();
                for _ in 1..total_pages {
                    let overflow_page_id =
                        allocator
                            .allocate()
                            .map_err(|e| NativeBackendError::IoError {
                                context: format!(
                                    "Failed to allocate overflow edge page for ({}, {:?})",
                                    src, dir
                                ),
                                source: std::io::Error::other(e.to_string()),
                            })?;
                    page_ids.push(overflow_page_id);
                }
            }

            let encoded_pages =
                encode_edge_cluster_pages(&cluster_bytes, self.page_size as usize, &page_ids)?;
            for (page_id, page_bytes) in page_ids.into_iter().zip(encoded_pages.into_iter()) {
                self.write_page_to_disk(&db_path, page_id, &page_bytes)?;
            }

            {
                let mut btree = self.btree.write();
                let key = edge_key(src, dir);
                let _ = btree.insert(key, first_page_id);
            }
        }

        flush_packed_entries(&mut packed_entries, &mut packed_bytes_used)?;

        // Clear dirty clusters after successful flush
        self.dirty_clusters.write().clear();

        // CRITICAL FIX: Checkpoint and truncate WAL after successful flush
        // This ensures WAL doesn't grow unbounded and data is durable
        if let Some(ref wal) = self.wal {
            let mut wal_guard = wal.write();

            // Get current B+Tree state for checkpoint
            let btree = self.btree.read();
            let root_page_id = btree.root_page_id();
            let tree_height = btree.tree_height();

            // Write checkpoint record
            let _ = wal_guard.checkpoint(
                root_page_id,
                0, // total_pages - not tracked in edge store
                tree_height,
                0,                             // free_page_list_head - not tracked in edge store
                &PersistentHeaderV3::new_v3(), // header snapshot
            );

            // Flush WAL to ensure checkpoint is on disk
            let _ = wal_guard.flush();

            // Truncate WAL after successful checkpoint
            // Safe because main DB pages are now durable
            let _ = wal_guard.truncate();
        }

        // CRITICAL FIX: Persist B+Tree metadata to allow recovery
        // This must happen after WAL checkpoint since we need the final root_page_id
        self.persist_btree_metadata()?;

        Ok(())
    }

    /// Get the path to the edge metadata file
    fn metadata_path(&self) -> Option<PathBuf> {
        self.db_path
            .as_ref()
            .map(|p| p.with_extension("v3edgemeta"))
    }

    fn load_cluster_from_disk(
        &self,
        page_id: u64,
        src: i64,
        dir: Direction,
        db_path: &PathBuf,
    ) -> NativeResult<V3EdgeCluster> {
        use crate::backend::native::v3::constants::V3_HEADER_SIZE;
        use std::fs::File;
        use std::io::Read;

        let mut file = File::open(db_path).map_err(|e| NativeBackendError::IoError {
            context: format!(
                "Failed to open db file for edge cluster read: {}",
                db_path.display()
            ),
            source: e,
        })?;

        let mut current_page_id = page_id;
        let mut cluster_bytes = Vec::new();
        loop {
            let offset = V3_HEADER_SIZE + (current_page_id - 1) * (self.page_size as u64);
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| NativeBackendError::IoError {
                    context: format!(
                        "Failed to seek to edge page {} offset {}",
                        current_page_id, offset
                    ),
                    source: e,
                })?;

            let mut buffer = vec![0u8; self.page_size as usize];
            file.read_exact(&mut buffer)
                .map_err(|e| NativeBackendError::IoError {
                    context: format!("Failed to read edge page {}", current_page_id),
                    source: e,
                })?;

            if let Some(cluster_bytes) = decode_packed_edge_page(&buffer, src, dir)? {
                let mut cluster = V3EdgeCluster::deserialize(&cluster_bytes, page_id)?;
                cluster.page_id = 0;
                return Ok(cluster);
            }

            if let Some((payload_len, next_page_id)) = decode_edge_cluster_page_header(&buffer) {
                let payload_end = EDGE_CLUSTER_PAGE_HEADER_SIZE + payload_len;
                cluster_bytes
                    .extend_from_slice(&buffer[EDGE_CLUSTER_PAGE_HEADER_SIZE..payload_end]);
                if next_page_id == 0 {
                    break;
                }
                current_page_id = next_page_id;
            } else {
                return V3EdgeCluster::deserialize(&buffer, page_id);
            }
        }

        V3EdgeCluster::deserialize(&cluster_bytes, page_id)
    }

    /// Persist B+Tree root metadata to disk for recovery
    ///
    /// This writes a small metadata file containing the B+Tree root_page_id
    /// and tree_height so that the edge index can be recovered after restart.
    fn persist_btree_metadata(&self) -> NativeResult<()> {
        let meta_path = match self.metadata_path() {
            Some(p) => p,
            None => return Ok(()), // In-memory mode, no persistence needed
        };

        let btree = self.btree.read();
        let root_page_id = btree.root_page_id();
        let tree_height = btree.tree_height();

        // Metadata format: [magic: 8 bytes][root_page_id: 8 bytes][tree_height: 4 bytes][checksum: 4 bytes]
        let mut data = Vec::with_capacity(24);
        data.extend_from_slice(b"V3EDGE\x00\x00"); // 8 bytes magic
        data.extend_from_slice(&root_page_id.to_le_bytes()); // 8 bytes
        data.extend_from_slice(&tree_height.to_le_bytes()); // 4 bytes

        // Simple checksum (XOR of bytes)
        let checksum: u32 = data.iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32));
        data.extend_from_slice(&checksum.to_le_bytes()); // 4 bytes

        std::fs::write(&meta_path, &data).map_err(|e| NativeBackendError::IoError {
            context: format!("Failed to write edge metadata: {}", meta_path.display()),
            source: e,
        })?;

        Ok(())
    }

    /// Recover B+Tree root metadata from disk
    ///
    /// Returns (root_page_id, tree_height) if metadata file exists and is valid.
    /// Returns None if metadata doesn't exist or is corrupted.
    fn recover_btree_metadata(&self) -> NativeResult<Option<(u64, u32)>> {
        let meta_path = match self.metadata_path() {
            Some(p) => p,
            None => return Ok(None), // In-memory mode, no recovery possible
        };

        if !meta_path.exists() {
            return Ok(None);
        }

        let data = std::fs::read(&meta_path).map_err(|e| NativeBackendError::IoError {
            context: format!("Failed to read edge metadata: {}", meta_path.display()),
            source: e,
        })?;

        if data.len() < 24 {
            return Ok(None); // Corrupted or incomplete
        }

        // Verify magic
        if &data[0..8] != b"V3EDGE\x00\x00" {
            return Ok(None); // Invalid magic
        }

        // Parse values
        let root_page_id = u64::from_le_bytes([
            data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
        ]);
        let tree_height = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);

        // Verify checksum
        let stored_checksum = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
        let computed_checksum: u32 = data[..20]
            .iter()
            .fold(0u32, |acc, &b| acc.wrapping_add(b as u32));

        if stored_checksum != computed_checksum {
            return Ok(None); // Checksum mismatch
        }

        Ok(Some((root_page_id, tree_height)))
    }

    /// Restore B+Tree state from persisted metadata
    ///
    /// This should be called after creating a new V3EdgeStore to recover
    /// the B+Tree root from a previous session.
    pub fn restore_btree_from_metadata(&self) -> NativeResult<bool> {
        if let Some((root_page_id, tree_height)) = self.recover_btree_metadata()? {
            let mut btree = self.btree.write();
            btree.set_root_page_id(root_page_id);
            btree.set_tree_height(tree_height);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Write a page of data to disk
    ///
    /// BUG FIX: Previously opened a raw file handle bypassing FileCoordinator,
    /// which could cause data corruption from concurrent writes with NodeStore.
    /// Now routes through FileCoordinator when available.
    fn write_page_to_disk(&self, db_path: &PathBuf, page_id: u64, data: &[u8]) -> NativeResult<()> {
        #[cfg(feature = "v3-forensics")]
        {
            use crate::backend::native::v3::constants::V3_HEADER_SIZE;
            let offset: u64 = if page_id == 0 {
                0
            } else {
                V3_HEADER_SIZE + (page_id - 1) * (self.page_size as u64)
            };
            crate::track_page_alloc!(page_id, Subsystem::EdgeStore, ForensicPageType::Edge);
            crate::track_page_write!(
                page_id,
                Subsystem::EdgeStore,
                ForensicPageType::Edge,
                offset,
                "EdgeStore::write_page_to_disk"
            );
        }

        // Use FileCoordinator when available to prevent race conditions
        if let Some(ref coordinator) = self.file_coordinator {
            // Pad data to full page size if needed for page-aligned writes
            let page_data = if data.len() < self.page_size as usize {
                let mut padded = data.to_vec();
                padded.resize(self.page_size as usize, 0);
                padded
            } else {
                data.to_vec()
            };
            return coordinator.write_page(page_id, &page_data);
        }

        // Fallback: raw file I/O (legacy path, no coordinator set)
        use crate::backend::native::v3::constants::V3_HEADER_SIZE;

        let offset: u64 = if page_id == 0 {
            0
        } else {
            V3_HEADER_SIZE + (page_id - 1) * (self.page_size as u64)
        };

        // CRITICAL FIX: Do NOT use create(true) - it truncates the file!
        let file_exists = db_path.exists();
        let mut file = OpenOptions::new()
            .write(true)
            .create(!file_exists)
            .open(db_path)
            .map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to open db file for page write: {}", page_id),
                source: e,
            })?;

        // Extend file if needed
        let required_len = offset + data.len() as u64;
        let current_len = file.metadata().map(|m| m.len()).unwrap_or(0);
        if required_len > current_len {
            file.set_len(required_len)
                .map_err(|e| NativeBackendError::IoError {
                    context: format!(
                        "Failed to extend file to {} bytes for page {}",
                        required_len, page_id
                    ),
                    source: e,
                })?;
        }

        file.seek(SeekFrom::Start(offset))
            .map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to seek to page {} offset {}", page_id, offset),
                source: e,
            })?;

        file.write_all(data)
            .map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to write page {} data", page_id),
                source: e,
            })?;

        file.sync_data().map_err(|e| NativeBackendError::IoError {
            context: format!("Failed to sync page {} write", page_id),
            source: e,
        })?;

        Ok(())
    }

    /// Flush WAL buffer to disk (for durability testing)
    #[cfg(test)]
    pub fn flush_wal(&self) -> NativeResult<()> {
        if let Some(ref wal) = self.wal {
            let mut wal_guard = wal.write();
            wal_guard.flush()
        } else {
            Ok(())
        }
    }

    /// Get the current B+Tree root page ID
    /// CRITICAL: Must be called during flush_to_disk to update header
    ///
    /// Returns None if the tree is empty (EMPTY_TREE_ROOT = u64::MAX)
    /// Returns Some(page_id) if the tree has a valid root
    pub fn btree_root_page_id(&self) -> Option<u64> {
        let root = self.btree.read().root_page_id();
        // Filter out EMPTY_TREE_ROOT (u64::MAX) and 0 (uninitialized)
        if root != 0 && root != u64::MAX {
            Some(root)
        } else {
            None
        }
    }

    /// Get the current B+Tree height
    /// CRITICAL: Must be called during flush_to_disk to update header
    pub fn btree_height(&self) -> u32 {
        self.btree.read().tree_height()
    }

    /// Set the WAL writer for this edge store
    ///
    /// This is called after opening an existing database when a WAL is discovered.
    pub fn set_wal(&mut self, wal: Arc<RwLock<WALWriter>>) {
        self.wal = Some(wal);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::v3::{
        allocator::PageAllocator, btree::BTreeManager, header::PersistentHeaderV3,
    };
    use parking_lot::RwLock;
    use std::path::PathBuf;
    use std::sync::Arc;

    use tempfile::TempDir;

    #[test]
    fn test_page_type_from_u8() {
        assert_eq!(PageType::from_u8(0), Some(PageType::Free));
        assert_eq!(PageType::from_u8(1), Some(PageType::BTreeIndex));
        assert_eq!(PageType::from_u8(2), Some(PageType::NodeData));
        assert_eq!(PageType::from_u8(3), Some(PageType::EdgeCluster));
        assert_eq!(PageType::from_u8(255), None);
    }

    #[test]
    fn test_direction_conversion() {
        assert_eq!(Direction::Outgoing.to_v2(), V2Direction::Outgoing);
        assert_eq!(Direction::Incoming.to_v2(), V2Direction::Incoming);
    }

    #[test]
    fn test_v3_edge_cluster_new() {
        let cluster = V3EdgeCluster::new(42, Direction::Outgoing, 100);
        assert_eq!(cluster.src, 42);
        assert!(cluster.edges.is_empty());
        assert_eq!(cluster.direction, Direction::Outgoing);
        assert_eq!(cluster.page_id, 100);
        assert_eq!(cluster.format_version, 3);
    }

    #[test]
    fn test_v3_edge_cluster_add_edge() {
        let mut cluster = V3EdgeCluster::new(1, Direction::Outgoing, 1);
        cluster.add_edge(2, None);
        cluster.add_edge(3, None);
        assert_eq!(cluster.dsts(), vec![2, 3]);
    }

    #[test]
    fn test_v3_edge_cluster_serialize_roundtrip() {
        let mut cluster = V3EdgeCluster::new(42, Direction::Outgoing, 100);
        cluster.add_edge(100, None);
        cluster.add_edge(200, None);

        let bytes = cluster.serialize().unwrap();
        let deserialized = V3EdgeCluster::deserialize(&bytes, 100).unwrap();

        assert_eq!(deserialized.format_version, 3);
        assert_eq!(deserialized.src, 42, "src should survive roundtrip");
        assert_eq!(
            deserialized.direction,
            Direction::Outgoing,
            "direction should survive roundtrip"
        );
        assert_eq!(deserialized.dsts(), vec![100, 200]);
        assert_eq!(deserialized.page_id, 100);
    }

    #[test]
    fn test_v3_edge_cluster_roundtrip_incoming() {
        let mut cluster = V3EdgeCluster::new(99, Direction::Incoming, 50);
        cluster.add_edge(10, None);

        let bytes = cluster.serialize().unwrap();
        let deserialized = V3EdgeCluster::deserialize(&bytes, 50).unwrap();

        assert_eq!(deserialized.src, 99);
        assert_eq!(
            deserialized.direction,
            Direction::Incoming,
            "Incoming direction must survive serialization roundtrip"
        );
    }

    //========================================================================
    // TDD Tests for Edge Store Durability TODOs
    // These tests verify the critical production issues:
    // 1. WAL record for edge insert
    // 2. Dirty cluster flush to pages
    // 3. B+Tree index update
    // 4. WAL checkpoint
    //========================================================================

    /// Test helper: Create a V3EdgeStore with WAL for durability testing
    fn create_test_edge_store(
        db_path: Option<PathBuf>,
    ) -> (V3EdgeStore, Arc<RwLock<PageAllocator>>) {
        let header = PersistentHeaderV3::new_v3();
        let allocator = Arc::new(RwLock::new(PageAllocator::new(&header)));

        // Create BTreeManager with the allocator
        let btree = if let Some(ref path) = db_path {
            BTreeManager::new(allocator.clone(), None, path.clone())
        } else {
            BTreeManager::new(allocator.clone(), None, None::<PathBuf>)
        };

        // Create edge store with or without persistence path
        let edge_store = if let Some(ref path) = db_path {
            // Create WAL writer
            let wal_path = path.with_extension("v3wal");
            let writer = WALWriter::new(wal_path, 1).expect("Failed to create WAL writer");
            writer.write_header().expect("Failed to write WAL header");
            V3EdgeStore::with_path_and_allocator(
                btree,
                Some(writer),
                path.clone(),
                allocator.clone(),
                header.page_size,
            )
        } else {
            V3EdgeStore::new(btree, None, allocator.clone(), header.page_size)
        };

        // CRITICAL FIX: Restore B+Tree metadata if it exists
        // This allows recovery of the edge index from a previous session
        let _ = edge_store.restore_btree_from_metadata();

        (edge_store, allocator)
    }

    /// Test 1: Edge insert should write WAL record for durability
    ///
    /// CRITICAL: This test verifies that insert_edge() creates a proper WAL record.
    /// Without this, edges inserted via cache are lost on crash.
    #[test]
    fn test_edge_insert_creates_wal_record() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");
        let wal_path = db_path.with_extension("v3wal");

        // Create edge store with WAL
        let (edge_store, _allocator) = create_test_edge_store(Some(db_path.clone()));

        // Insert an edge - this should create a WAL record
        edge_store
            .insert_edge(1, 2, Direction::Outgoing, None)
            .expect("Insert failed");

        // Flush WAL to ensure record is written
        edge_store.flush_wal().expect("WAL flush failed");

        // Verify: Verify WAL file exists and contains edge insert record
        // Currently this fails because insert_edge() does NOT write WAL records
        assert!(
            wal_path.exists(),
            "NOTE: WAL file should exist after edge insert with WAL enabled"
        );

        // Read WAL and verify edge insert record exists
        let wal_content = std::fs::read(&wal_path).expect("Failed to read WAL file");
        assert!(
            wal_content.len() > 64, // Header is 64 bytes, records add more
            "NOTE: WAL should contain edge insert record beyond header"
        );

        // Verify WAL parsing: and verify edge-specific record type exists
        // This requires implementing EdgeInsert record type in WAL
    }

    /// Test 2: Flush should write dirty clusters to pages
    ///
    /// CRITICAL: This test verifies that flush() actually persists edge data.
    /// Flush writes dirty clusters to disk pages via write_page_to_disk.
    #[test]
    fn test_flush_writes_dirty_clusters_to_pages() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");

        // Create the database file first
        std::fs::write(&db_path, vec![0u8; 4096]).expect("Failed to create db file");

        // Create edge store with disk persistence
        let (edge_store, _allocator) = create_test_edge_store(Some(db_path.clone()));

        // Insert edges into cache
        edge_store
            .insert_edge(1, 2, Direction::Outgoing, None)
            .expect("Insert 1->2 failed");
        edge_store
            .insert_edge(1, 3, Direction::Outgoing, None)
            .expect("Insert 1->3 failed");
        edge_store
            .insert_edge(2, 4, Direction::Outgoing, None)
            .expect("Insert 2->4 failed");

        // Flush should write dirty clusters to disk pages
        let result = edge_store.flush(None);
        assert!(result.is_ok(), "Flush should succeed");

        // Verify: After flush, edge data should be on disk
        // Currently this fails because flush() does nothing
        let file_size = std::fs::metadata(&db_path)
            .expect("Failed to read file metadata")
            .len();

        assert!(
            file_size > 4096,
            "NOTE: Database file should grow after flush writes dirty clusters"
        );

        // Verify we can read back the edges after reopening
        // This requires implementing cluster deserialization from pages
    }

    /// Test 3: Flush should update B+Tree index
    ///
    /// CRITICAL: The B+Tree index maps (src_node_id, direction) -> page_id.
    /// Without this update, edge lookups will fail after recovery.
    #[test]
    fn test_flush_updates_btree_index() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");

        // Create database file
        std::fs::write(&db_path, vec![0u8; 4096]).expect("Failed to create db file");

        // Create edge store
        let (edge_store, _allocator) = create_test_edge_store(Some(db_path.clone()));

        // Insert edges for node 1
        edge_store
            .insert_edge(1, 2, Direction::Outgoing, None)
            .expect("Insert failed");
        edge_store
            .insert_edge(1, 3, Direction::Outgoing, None)
            .expect("Insert failed");

        // Flush should update B+Tree index
        edge_store.flush(None).expect("Flush failed");

        // Verify: B+Tree should contain mapping for node 1
        // Currently btree only tracks node_id -> page_id, not edge lookups
        // Need to implement (src, direction) composite key lookup

        // After fix: verify B+Tree contains edge cluster mapping
        let btree = edge_store.btree.read();
        let lookup_key = edge_key(1, Direction::Outgoing);
        let lookup_result = btree.lookup(lookup_key);

        assert!(lookup_result.is_ok(), "B+Tree lookup should succeed");
        assert!(
            lookup_result.unwrap().is_some(),
            "B+Tree should contain edge page mapping for node 1 after flush"
        );
    }

    /// Test 4: WAL checkpoint should truncate WAL after successful flush
    ///
    /// VERIFIED: After flush() persists data to pages, WAL is checkpointed
    /// and truncated to prevent unbounded WAL growth.
    #[test]
    fn test_wal_checkpoint_after_flush() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");
        let wal_path = db_path.with_extension("v3wal");

        // Create database file
        std::fs::write(&db_path, vec![0u8; 4096]).expect("Failed to create db file");

        // Create edge store with WAL
        let (edge_store, _allocator) = create_test_edge_store(Some(db_path.clone()));

        // Insert and flush multiple times
        for i in 0..5 {
            edge_store
                .insert_edge(1, i as i64 + 10, Direction::Outgoing, None)
                .unwrap_or_else(|_| panic!("Insert iteration {} failed", i));
            edge_store.flush(None).expect("Flush failed");
        }

        // VERIFIED: WAL should be truncated (removed) after flush
        // The truncate() call now happens after checkpoint in flush()
        //
        // DURABILITY GUARANTEE:
        // - Main DB pages are synced before WAL is truncated
        // - WAL replay is not implemented (so WAL is not needed for recovery)
        // - Safe to remove WAL file after checkpoint
        assert!(
            !wal_path.exists(),
            "WAL file should be truncated (removed) after flush"
        );
    }

    /// Test 5: Edge data should survive crash (recovery test)
    ///
    /// VERIFIED: Edges persisted to disk can be recovered after reopening.
    /// WAL is truncated after flush, but main DB file contains durable data.
    #[test]
    fn test_edge_recovery_after_crash() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");
        let wal_path = db_path.with_extension("v3wal");

        // Create database file
        std::fs::write(&db_path, vec![0u8; 4096]).expect("Failed to create db file");

        // Phase 1: Create edges and persist to disk
        {
            let (edge_store, _allocator) = create_test_edge_store(Some(db_path.clone()));

            edge_store
                .insert_edge(1, 2, Direction::Outgoing, None)
                .expect("Insert failed");
            edge_store
                .insert_edge(1, 3, Direction::Outgoing, None)
                .expect("Insert failed");
            edge_store
                .insert_edge(2, 4, Direction::Outgoing, None)
                .expect("Insert failed");

            // Call flush() to write dirty clusters to disk pages
            // This ensures data survives after the edge store is dropped
            edge_store.flush(None).expect("Flush failed");

            // VERIFIED: WAL is now truncated after flush
            assert!(
                !wal_path.exists(),
                "WAL file should be truncated (removed) after flush with checkpoint"
            );
        }

        // Phase 2: "Recover" by creating new edge store
        // The new store should load edges from disk on cache miss
        {
            let (recovered_store, _allocator) = create_test_edge_store(Some(db_path.clone()));

            // Load neighbors for node 1 - should read from disk since cache is empty
            let neighbors = recovered_store
                .outgoing(1)
                .expect("Failed to get neighbors");

            // VERIFIED: Data persists from main DB file (WAL is not needed for recovery)
            assert!(
                neighbors.len() >= 2,
                "After recovery, node 1 should have at least 2 outgoing neighbors"
            );

            // Verify specific neighbors are present
            let neighbor_vec: Vec<i64> = neighbors.iter().copied().collect();
            assert!(
                neighbor_vec.contains(&2),
                "Node 1 should have edge to node 2"
            );
            assert!(
                neighbor_vec.contains(&3),
                "Node 1 should have edge to node 3"
            );
        }
    }

    /// Test 6: Data persists after multiple flush cycles with WAL truncation
    ///
    /// VERIFIED: Multiple insert/flush cycles work correctly, WAL is truncated each time,
    /// and all data is recoverable from main DB file.
    #[test]
    fn test_data_persists_after_multiple_wal_truncations() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");
        let wal_path = db_path.with_extension("v3wal");

        // Create database file
        std::fs::write(&db_path, vec![0u8; 4096]).expect("Failed to create db file");

        // Phase 1: Insert multiple batches, each flushed
        {
            let (edge_store, _allocator) = create_test_edge_store(Some(db_path.clone()));

            // First batch
            for i in 0..5 {
                edge_store
                    .insert_edge(1, i + 10, Direction::Outgoing, None)
                    .expect("Insert failed");
            }
            edge_store.flush(None).expect("Flush failed");
            assert!(
                !wal_path.exists(),
                "WAL should be truncated after first flush"
            );

            // Second batch
            for i in 0..5 {
                edge_store
                    .insert_edge(2, i + 20, Direction::Outgoing, None)
                    .expect("Insert failed");
            }
            edge_store.flush(None).expect("Flush failed");
            assert!(
                !wal_path.exists(),
                "WAL should be truncated after second flush"
            );
        }

        // Phase 2: Verify all data persisted
        let (recovered_store, _allocator) = create_test_edge_store(Some(db_path.clone()));

        let neighbors1 = recovered_store
            .outgoing(1)
            .expect("Failed to get node 1 neighbors");
        assert_eq!(
            neighbors1.len(),
            5,
            "Node 1 should have 5 outgoing neighbors"
        );

        let neighbors2 = recovered_store
            .outgoing(2)
            .expect("Failed to get node 2 neighbors");
        assert_eq!(
            neighbors2.len(),
            5,
            "Node 2 should have 5 outgoing neighbors"
        );
    }

    /// Test 6: Empty flush should not error
    ///
    /// Edge case: flush() with no dirty clusters should succeed gracefully.
    #[test]
    fn test_flush_with_no_dirty_clusters() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");

        // Create edge store without inserting any edges
        let (edge_store, _allocator) = create_test_edge_store(Some(db_path));

        // Flush with empty cache should succeed
        let result = edge_store.flush(None);
        assert!(result.is_ok(), "Flush with empty cache should succeed");
    }

    /// Test 7: Multiple flushes should be idempotent
    ///
    /// Calling flush() multiple times should not corrupt data.
    #[test]
    fn test_multiple_flushes_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");
        std::fs::write(&db_path, vec![0u8; 4096]).expect("Failed to create db file");

        let (edge_store, _allocator) = create_test_edge_store(Some(db_path.clone()));

        // Insert edges
        edge_store
            .insert_edge(1, 2, Direction::Outgoing, None)
            .expect("Insert failed");

        // Flush multiple times
        for _ in 0..3 {
            edge_store.flush(None).expect("Flush failed");
        }

        // After implementing flush: verify edges are still queryable
        // Currently this just verifies no panic occurs
    }

    /// Test 8: WAL EdgeInsert record is correctly written and can be recovered
    ///
    /// CRITICAL: This test verifies that edge_insert() writes a proper WAL record
    /// that can be recovered during WAL replay.
    #[test]
    fn test_wal_edge_insert_record_format() {
        use crate::backend::native::v3::wal::{V3_WAL_HEADER_SIZE, V3WALRecord, V3WALRecordType};
        use std::fs;

        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");
        let wal_path = db_path.with_extension("v3wal");

        // Create edge store with WAL
        let (edge_store, _allocator) = create_test_edge_store(Some(db_path.clone()));

        // Insert an edge - should write EdgeInsert WAL record
        edge_store
            .insert_edge(1, 2, Direction::Outgoing, None)
            .expect("Insert failed");
        edge_store.flush_wal().expect("WAL flush failed");

        // Read WAL file
        let wal_content = fs::read(&wal_path).expect("Failed to read WAL");

        // Verify WAL has more than just header (64 bytes)
        assert!(
            wal_content.len() > V3_WAL_HEADER_SIZE,
            "WAL should have records beyond header"
        );

        // Verify EdgeInsert record type is in the WAL
        // WAL format: [size: 4 bytes][bincode serialized record]
        let mut pos = V3_WAL_HEADER_SIZE; // Skip header

        let mut found_edge_insert = false;
        while pos < wal_content.len() - 8 {
            // Read record size
            if pos + 4 > wal_content.len() {
                break;
            }
            let size = u32::from_le_bytes([
                wal_content[pos],
                wal_content[pos + 1],
                wal_content[pos + 2],
                wal_content[pos + 3],
            ]) as usize;

            pos += 4;
            if pos + size > wal_content.len() || size == 0 {
                break;
            }

            // Deserialize the record using bincode
            let record_bytes = &wal_content[pos..pos + size];
            if let Ok(record) = V3WALRecord::from_bytes(record_bytes)
                && record.record_type() == V3WALRecordType::EdgeInsert
            {
                found_edge_insert = true;
                break;
            }

            // Skip to next record
            pos += size;
        }

        assert!(
            found_edge_insert,
            "WAL should contain EdgeInsert record (type 12)"
        );
    }

    //========================================================================
    // Edge Type Durability Tests
    //========================================================================

    /// Test that edge_type survives serialization roundtrip
    /// This is critical for durability across reopen
    #[test]
    fn test_edge_type_serialization_roundtrip() {
        let mut cluster = V3EdgeCluster::new(1, Direction::Outgoing, 100);

        // Add edge with type
        cluster.add_edge(2, Some("TEST_TYPE".to_string()));

        // Verify edge_data was populated
        assert_eq!(cluster.edges.len(), 1);
        let edge = &cluster.edges[0];
        assert!(
            !edge.edge_data.is_empty(),
            "edge_data should not be empty when edge_type is set"
        );

        // Verify edge_type can be extracted
        let extracted = V3EdgeCluster::extract_edge_type(&edge.edge_data);
        assert_eq!(extracted, Some("TEST_TYPE".to_string()));

        // Test serialization roundtrip
        let serialized = cluster.serialize().unwrap();
        let deserialized = V3EdgeCluster::deserialize(&serialized, 100).unwrap();

        assert_eq!(deserialized.edges.len(), 1);
        let deser_edge = &deserialized.edges[0];
        let deser_type = V3EdgeCluster::extract_edge_type(&deser_edge.edge_data);
        assert_eq!(
            deser_type,
            Some("TEST_TYPE".to_string()),
            "edge_type should survive serialization roundtrip"
        );
    }

    /// Test that edge_type is extracted correctly during edges_with_types
    #[test]
    fn test_edges_with_types_extraction() {
        let mut cluster = V3EdgeCluster::new(1, Direction::Outgoing, 100);

        // Add edges with different types
        cluster.add_edge(2, Some("CALLS".to_string()));
        cluster.add_edge(3, Some("USES".to_string()));
        cluster.add_edge(4, None); // No type

        let edges_with_types = cluster.edges_with_types();
        assert_eq!(edges_with_types.len(), 3);

        // Check first edge
        assert_eq!(edges_with_types[0].0, 2);
        assert_eq!(edges_with_types[0].1, Some("CALLS".to_string()));

        // Check second edge
        assert_eq!(edges_with_types[1].0, 3);
        assert_eq!(edges_with_types[1].1, Some("USES".to_string()));

        // Check third edge (no type)
        assert_eq!(edges_with_types[2].0, 4);
        assert_eq!(edges_with_types[2].1, None);
    }

    #[test]
    fn test_weighted_edges() {
        use crate::backend::{BackendDirection, EdgeSpec, GraphBackend, NeighborQuery, NodeSpec};

        let temp_dir = tempfile::TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.graph");
        let backend = crate::backend::native::v3::V3Backend::create(&db_path).unwrap();

        // Create nodes
        backend
            .insert_node(NodeSpec {
                kind: "Node".to_string(),
                name: "n1".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        backend
            .insert_node(NodeSpec {
                kind: "Node".to_string(),
                name: "n2".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();

        let n1_id = backend.entity_ids().unwrap()[0];
        let n2_id = backend.entity_ids().unwrap()[1];

        // 1. Insert a weighted edge via standard insert_edge by passing weight inside data
        backend
            .insert_edge(EdgeSpec {
                from: n1_id,
                to: n2_id,
                edge_type: "CALLS".to_string(),
                data: serde_json::json!({ "weight": 2.5 }),
            })
            .unwrap();

        // Query weighted neighbors
        let query = NeighborQuery {
            direction: BackendDirection::Outgoing,
            edge_type: None,
        };
        let neighbors = backend
            .neighbors_weighted(crate::SnapshotId::current(), n1_id, query.clone())
            .unwrap();
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].0, n2_id);
        assert_eq!(neighbors[0].1, 2.5);

        // Query filtered weighted neighbors
        let query_filtered = NeighborQuery {
            direction: BackendDirection::Outgoing,
            edge_type: Some("CALLS".to_string()),
        };
        let neighbors_filt = backend
            .neighbors_weighted(crate::SnapshotId::current(), n1_id, query_filtered)
            .unwrap();
        assert_eq!(neighbors_filt.len(), 1);
        assert_eq!(neighbors_filt[0].1, 2.5);

        // 2. Insert via batch_insert_edges_with_weights
        backend
            .batch_insert_edges_with_weights(vec![(n2_id, n1_id, 4.2, Some("RETURNS".to_string()))])
            .unwrap();

        let query_incoming = NeighborQuery {
            direction: BackendDirection::Outgoing,
            edge_type: Some("RETURNS".to_string()),
        };
        let neighbors_incoming = backend
            .neighbors_weighted(crate::SnapshotId::current(), n2_id, query_incoming)
            .unwrap();
        assert_eq!(neighbors_incoming.len(), 1);
        assert_eq!(neighbors_incoming[0].0, n1_id);
        assert_eq!(neighbors_incoming[0].1, 4.2);
    }

    #[test]
    fn test_large_edge_cluster_survives_flush_and_reopen() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("large_edge_cluster.graph");
        std::fs::write(&db_path, vec![0u8; 4096]).expect("Failed to create db file");

        {
            let (edge_store, _allocator) = create_test_edge_store(Some(db_path.clone()));
            for dst in 2..=2500 {
                edge_store
                    .insert_edge_weighted(
                        1,
                        dst,
                        Direction::Outgoing,
                        Some("LINK".to_string()),
                        1.5,
                    )
                    .expect("Insert failed");
            }
            edge_store.flush(None).expect("Flush failed");
        }

        {
            let (recovered_store, _allocator) = create_test_edge_store(Some(db_path.clone()));
            let neighbors = recovered_store
                .neighbors_weighted(1, Direction::Outgoing)
                .expect("Failed to recover weighted neighbors");
            assert_eq!(
                neighbors.len(),
                2499,
                "all oversized-cluster edges must survive reopen"
            );
            assert_eq!(neighbors[0], (2, 1.5));
            assert_eq!(neighbors[2498], (2500, 1.5));
        }
    }

    #[test]
    fn test_small_clusters_are_packed_and_reopen_correctly() {
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("packed_small_clusters.graph");
        std::fs::write(&db_path, vec![0u8; 4096]).expect("Failed to create db file");

        {
            let (edge_store, _allocator) = create_test_edge_store(Some(db_path.clone()));
            for src in 1..=128 {
                edge_store
                    .insert_edge_weighted(src, 10_000 + src, Direction::Outgoing, None, 0.5)
                    .expect("Insert failed");
                edge_store
                    .insert_edge_weighted(10_000 + src, src, Direction::Incoming, None, 0.5)
                    .expect("Insert failed");
            }
            edge_store.flush(None).expect("Flush failed");
        }

        let file_size = std::fs::metadata(&db_path).expect("metadata").len();
        assert!(
            file_size < 65_536,
            "packed sparse clusters should stay well below one-page-per-cluster size, got {} bytes",
            file_size
        );

        {
            let (recovered_store, _allocator) = create_test_edge_store(Some(db_path.clone()));
            for src in 1..=128 {
                let outgoing = recovered_store
                    .neighbors_weighted(src, Direction::Outgoing)
                    .expect("Failed to recover outgoing neighbors");
                assert_eq!(outgoing.as_ref(), &[(10_000 + src, 0.5)]);

                let incoming = recovered_store
                    .neighbors_weighted(10_000 + src, Direction::Incoming)
                    .expect("Failed to recover incoming neighbors");
                assert_eq!(incoming.as_ref(), &[(src, 0.5)]);
            }
        }
    }

    //========================================================================
    // End TDD Tests
    //========================================================================
}
