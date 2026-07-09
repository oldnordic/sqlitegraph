//! V3 Edge Compatibility Layer
//!
//! This module provides a compatibility layer for using V2 EdgeCluster format
//! within V3's page-based storage system. This compat layer bridges
//! edge layout until NodeStore/B+Tree/allocator/WAL are fully settled.
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

#![allow(
    clippy::type_complexity,
    clippy::collapsible_if,
    reason = "V3 cache types are complex by design; collapsed let-chains reduce readability"
)]
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
use std::path::{Path, PathBuf};
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
pub(crate) const EDGE_CLUSTER_PAGE_HEADER_SIZE: usize = 16;
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
    fn sort_for_weighted_queries(&mut self) {
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

pub(crate) fn decode_edge_cluster_page_header(page: &[u8]) -> Option<(usize, u64)> {
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

pub(crate) fn decode_packed_edge_page(
    page: &[u8],
    src: i64,
    dir: Direction,
) -> NativeResult<Option<Vec<u8>>> {
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
    pub btree: Arc<parking_lot::RwLock<BTreeManager>>,
    #[cfg(not(test))]
    btree: Arc<parking_lot::RwLock<BTreeManager>>,
    /// Optional WAL writer for durability (shared with V3Backend via Arc)
    #[cfg(test)]
    pub wal: Option<Arc<RwLock<WALWriter>>>,
    #[cfg(not(test))]
    wal: Option<Arc<RwLock<WALWriter>>>,
    /// In-memory cache of neighbor lists - using Arc<[i64]> for zero-copy reads
    /// This matches SQLite's AdjacencyCache pattern but with Arc for zero-copy
    cache: Arc<RwLock<HashMap<(i64, Direction), Arc<[i64]>>>>,
    /// In-memory cache of weighted neighbors
    cache_weighted: Arc<RwLock<HashMap<(i64, Direction), Arc<[(i64, f32)]>>>>,
    /// Edge type storage: (src, dst, dir) -> edge_type string
    ///
    /// SEMANTIC CONSTRAINT: Key is (src, dst, dir), NOT edge_id.
    /// This means only ONE edge type can exist between a given (src, dst, dir) tuple.
    /// Inserting multiple edges between same endpoints with different types will
    /// cause aliasing - the last type wins. See module-level docs for details.
    ///
    /// This HashMap is rebuilt from durable edge_data on cache miss via
    /// load_neighbors_from_disk(), ensuring edge types survive reopen/recovery.
    edge_types: Arc<RwLock<HashMap<(i64, i64, Direction), String>>>,
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
pub(crate) fn edge_key(src: i64, dir: Direction) -> i64 {
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
    fn read_page_from_disk(
        &self,
        file: &mut std::fs::File,
        db_path: &Path,
        page_id: u64,
    ) -> NativeResult<Vec<u8>> {
        use crate::backend::native::v3::constants::V3_HEADER_SIZE;
        use std::io::{Read, Seek, SeekFrom};

        let offset = V3_HEADER_SIZE + (page_id - 1) * (self.page_size as u64);
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to seek to edge page {} offset {}", page_id, offset),
                source: e,
            })?;

        let mut buffer = vec![0u8; self.page_size as usize];
        file.read_exact(&mut buffer)
            .map_err(|e| NativeBackendError::IoError {
                context: format!(
                    "Failed to read edge page {} from {}",
                    page_id,
                    db_path.display()
                ),
                source: e,
            })?;
        Ok(buffer)
    }

    fn weighted_neighbors_from_cluster(
        &self,
        src: i64,
        dir: Direction,
        cluster: &V3EdgeCluster,
    ) -> Arc<[(i64, f32)]> {
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
        neighbors.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        Arc::from(neighbors.into_boxed_slice())
    }

    fn unweighted_neighbors_from_weighted(neighbors: &[(i64, f32)]) -> Arc<[i64]> {
        let dsts: Vec<i64> = neighbors.iter().map(|(dst, _)| *dst).collect();
        Arc::from(dsts.into_boxed_slice())
    }

    pub fn btree_lock(&self) -> &Arc<parking_lot::RwLock<BTreeManager>> {
        &self.btree
    }

    pub fn cache_lock(&self) -> &Arc<RwLock<HashMap<(i64, Direction), Arc<[i64]>>>> {
        &self.cache
    }

    pub fn cache_weighted_lock(
        &self,
    ) -> &Arc<RwLock<HashMap<(i64, Direction), Arc<[(i64, f32)]>>>> {
        &self.cache_weighted
    }

    pub fn edge_types_lock(&self) -> &Arc<RwLock<HashMap<(i64, i64, Direction), String>>> {
        &self.edge_types
    }

    pub fn page_size(&self) -> u64 {
        self.page_size as u64
    }

    /// Create new edge store (in-memory only)
    /// NOTE: Prefer with_path_and_allocator() for database-backed edge stores
    pub fn new(
        btree: BTreeManager,
        wal: Option<WALWriter>,
        allocator: Arc<RwLock<PageAllocator>>,
        page_size: u32,
    ) -> Self {
        Self {
            btree: Arc::new(parking_lot::RwLock::new(btree)),
            wal: wal.map(|w| Arc::new(RwLock::new(w))),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_weighted: Arc::new(RwLock::new(HashMap::new())),
            edge_types: Arc::new(RwLock::new(HashMap::new())),
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
            btree: Arc::new(parking_lot::RwLock::new(btree)),
            wal: wal.map(|w| Arc::new(RwLock::new(w))),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_weighted: Arc::new(RwLock::new(HashMap::new())),
            edge_types: Arc::new(RwLock::new(HashMap::new())),
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
    /// NOTE: This creates a standalone allocator for backward compatibility.
    /// For proper page allocation, use with_path_and_allocator() instead.
    pub fn with_path(btree: BTreeManager, wal: Option<WALWriter>, db_path: PathBuf) -> Self {
        // Create a standalone allocator for backward compatibility
        // WARNING: This allocator is not shared with NodeStore, so page IDs
        // may collide. Always use with_path_and_allocator() in production.
        let header = PersistentHeaderV3::new_v3();
        Self {
            btree: Arc::new(parking_lot::RwLock::new(btree)),
            wal: wal.map(|w| Arc::new(RwLock::new(w))),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_weighted: Arc::new(RwLock::new(HashMap::new())),
            edge_types: Arc::new(RwLock::new(HashMap::new())),
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
    /// When set, all file reads/writes go through this coordinator to prevent
    /// race conditions and avoid per-cache-miss file open/close overhead.
    /// The coordinator is also propagated to the internal BTreeManager so
    /// edge-cluster lookups share the same persistent handle.
    pub fn set_file_coordinator(&mut self, coordinator: Arc<FileCoordinator>) {
        self.btree.write().set_file_coordinator(coordinator.clone());
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
        db_path: &Path,
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
            return dirty
                .get_mut(&cache_key)
                .ok_or_else(|| NativeBackendError::InvalidOperation {
                    context: format!(
                        "dirty edge cluster missing despite existing key for src={} dir={:?}",
                        src, dir
                    ),
                });
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
        dirty
            .get_mut(&cache_key)
            .ok_or_else(|| NativeBackendError::InvalidOperation {
                context: format!(
                    "dirty edge cluster missing after insertion for src={} dir={:?}",
                    src, dir
                ),
            })
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
                }
                vec.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.0.cmp(&b.0))
                });
                *neighbors = Arc::from(vec);
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
                vec.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.0.cmp(&b.0))
                });
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
                self.cache_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(neighbors.clone());
            }
        }

        self.cache_misses.fetch_add(1, Ordering::Relaxed);

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
        db_path: &Path,
    ) -> NativeResult<Arc<[(i64, f32)]>> {
        let key = edge_key(src, dir);
        let btree = self.btree.read();

        let page_id = match btree.lookup(key) {
            Ok(Some(pid)) => pid,
            Ok(None) | Err(_) => return Ok(Arc::from([])),
        };
        drop(btree);

        match self.load_cluster_from_disk(page_id, src, dir, db_path) {
            Ok(cluster) => Ok(self.weighted_neighbors_from_cluster(src, dir, &cluster)),
            Err(_) => Ok(Arc::from([])),
        }
    }

    pub fn warm_weighted_neighbors(&self, sources: &[i64], dir: Direction) -> NativeResult<usize> {
        use std::collections::BTreeMap;

        let db_path = match &self.db_path {
            Some(path) => path,
            None => return Ok(0),
        };
        if sources.is_empty() {
            return Ok(0);
        }

        let misses: Vec<i64> = {
            let cache = self.cache_weighted.read();
            sources
                .iter()
                .copied()
                .filter(|src| !cache.contains_key(&(*src, dir)))
                .collect()
        };
        if misses.is_empty() {
            return Ok(0);
        }

        let lookups: Vec<(i64, u64)> = {
            let btree = self.btree.read();
            misses
                .into_iter()
                .filter_map(|src| match btree.lookup(edge_key(src, dir)) {
                    Ok(Some(page_id)) => Some((src, page_id)),
                    Ok(None) | Err(_) => None,
                })
                .collect()
        };
        if lookups.is_empty() {
            return Ok(0);
        }

        let mut page_groups: BTreeMap<u64, Vec<i64>> = BTreeMap::new();
        for (src, page_id) in lookups {
            page_groups.entry(page_id).or_default().push(src);
        }

        let mut file = std::fs::File::open(db_path).map_err(|e| NativeBackendError::IoError {
            context: format!(
                "Failed to open db file for edge warm read: {}",
                db_path.display()
            ),
            source: e,
        })?;

        let mut warmed = Vec::new();
        for (page_id, srcs) in page_groups {
            let buffer = self.read_page_from_disk(&mut file, db_path, page_id)?;
            if buffer.len() >= PACKED_EDGE_PAGE_HEADER_SIZE
                && buffer[0..4] == PACKED_EDGE_PAGE_MAGIC
            {
                for src in srcs {
                    if let Some(cluster_bytes) = decode_packed_edge_page(&buffer, src, dir)? {
                        let mut cluster = V3EdgeCluster::deserialize(&cluster_bytes, page_id)?;
                        cluster.page_id = 0;
                        cluster.sort_for_weighted_queries();
                        let weighted = self.weighted_neighbors_from_cluster(src, dir, &cluster);
                        let unweighted = Self::unweighted_neighbors_from_weighted(&weighted);
                        warmed.push((src, weighted, unweighted));
                    }
                }
                continue;
            }

            for src in srcs {
                let mut cluster = self.load_cluster_from_open_file(
                    &mut file,
                    page_id,
                    src,
                    dir,
                    db_path,
                    Some(buffer.clone()),
                )?;
                cluster.sort_for_weighted_queries();
                let weighted = self.weighted_neighbors_from_cluster(src, dir, &cluster);
                let unweighted = Self::unweighted_neighbors_from_weighted(&weighted);
                warmed.push((src, weighted, unweighted));
            }
        }

        if warmed.is_empty() {
            return Ok(0);
        }

        let warmed_count = warmed.len();
        let mut weighted_cache = self.cache_weighted.write();
        let mut unweighted_cache = self.cache.write();
        for (src, weighted, unweighted) in warmed {
            weighted_cache.insert((src, dir), weighted);
            unweighted_cache.insert((src, dir), unweighted);
        }
        Ok(warmed_count)
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
                vec.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.0.cmp(&b.0))
                });
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
                vec.sort_by(|a, b| {
                    b.1.partial_cmp(&a.1)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| a.0.cmp(&b.0))
                });
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
            let mut cluster = cluster;
            cluster.sort_for_weighted_queries();
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
            for (page_id, page_bytes) in page_ids.into_iter().zip(encoded_pages) {
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
        db_path: &Path,
    ) -> NativeResult<V3EdgeCluster> {
        use std::fs::File;

        let mut file = File::open(db_path).map_err(|e| NativeBackendError::IoError {
            context: format!(
                "Failed to open db file for edge cluster read: {}",
                db_path.display()
            ),
            source: e,
        })?;

        self.load_cluster_from_open_file(&mut file, page_id, src, dir, db_path, None)
    }

    fn load_cluster_from_open_file(
        &self,
        file: &mut std::fs::File,
        page_id: u64,
        src: i64,
        dir: Direction,
        db_path: &Path,
        first_page: Option<Vec<u8>>,
    ) -> NativeResult<V3EdgeCluster> {
        let mut current_page_id = page_id;
        let mut cluster_bytes = Vec::new();
        let mut buffer = match first_page {
            Some(buffer) => buffer,
            None => self.read_page_from_disk(file, db_path, current_page_id)?,
        };

        loop {
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
                buffer = self.read_page_from_disk(file, db_path, current_page_id)?;
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
    fn write_page_to_disk(&self, db_path: &Path, page_id: u64, data: &[u8]) -> NativeResult<()> {
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
#[path = "edge_compat_tests.rs"]
mod tests;
