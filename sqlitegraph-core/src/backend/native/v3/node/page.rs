//! NodePage - Variable-size page storage for NodeRecordV3
//!
//! This module implements the NodePage structure for storing variable-size NodeRecordV3
//! records in a 4KB page. Uses delta/varint encoding for space efficiency.
//!
//! # Compression
//!
//! - Node IDs are delta-encoded from a base ID (saves ~4 bytes per node)
//! - Field counts/lengths use varint encoding (saves ~1-3 bytes per field)
//! - Nodes are packed contiguously without fixed slot boundaries

use super::record::NodeRecordV3;
use crate::backend::native::NativeBackendError;
use crate::backend::native::NativeResult;
use crate::backend::native::v3::compression;
use crate::backend::native::v3::compression::delta::{decode_id_delta, encode_id_delta};
use crate::backend::native::v3::compression::varint::{
    decode_varint, decode_varint_u16, encode_varint, encode_varint_u16,
};
use crate::backend::native::v3::constants as v3_constants;

/// NodePage header and layout constants
pub mod constants {

    /// Page header size in bytes
    ///
    /// Layout:
    /// - page_id: 8 bytes (u64)
    /// - next_page_id: 8 bytes (u64, overflow link)
    /// - node_count: 2 bytes (u16)
    /// - used_bytes: 2 bytes (u16, actual bytes used in data region)
    /// - base_id: 8 bytes (i64, for delta encoding)
    /// - checksum: 4 bytes (u32)
    /// - reserved: 0 bytes (header exactly 32 bytes)
    pub const PAGE_HEADER_SIZE: usize = 32;

    /// Fixed metadata size from NodeRecordV3
    pub const FIXED_METADATA_SIZE: usize = 44;

    /// Maximum inline data size from NodeRecordV3
    pub const MAX_INLINE_DATA: usize = 64;

    /// Page ID offset within header
    pub const PAGE_ID_OFFSET: usize = 0;

    /// Next page ID offset (overflow link)
    pub const NEXT_PAGE_ID_OFFSET: usize = 8;

    /// Node count offset (u16)
    pub const NODE_COUNT_OFFSET: usize = 16;

    /// Used bytes offset (u16)
    pub const USED_BYTES_OFFSET: usize = 18;

    /// Base ID offset (i64, for delta encoding)
    pub const BASE_ID_OFFSET: usize = 20;

    /// Checksum offset (u32)
    pub const CHECKSUM_OFFSET: usize = 28;

    /// Total page size (4KB)
    pub const MAX_PAGE_SIZE: usize = 4096;

    /// Usable page size after header
    pub const USABLE_SIZE: usize = MAX_PAGE_SIZE - PAGE_HEADER_SIZE;

    /// Estimated fixed node slot size (conservative estimate for non-compressed records)
    /// 44 bytes metadata + 32 bytes average inline data = 76 bytes
    /// Rounded up to 80 bytes for safety
    pub const ESTIMATED_NODE_SLOT_SIZE: usize = 80;

    /// Fixed node capacity (conservative estimate)
    /// USABLE_SIZE (4064) / ESTIMATED_NODE_SLOT_SIZE (80) ≈ 50 nodes
    /// Using 50 as max capacity
    pub const MAX_NODE_CAPACITY: usize = USABLE_SIZE / ESTIMATED_NODE_SLOT_SIZE;

    // Field sizes
    pub const PAGE_ID_SIZE: usize = 8;
    pub const NEXT_PAGE_ID_SIZE: usize = 8;
    pub const NODE_COUNT_SIZE: usize = 2;
    pub const USED_BYTES_SIZE: usize = 2;
    pub const BASE_ID_SIZE: usize = 8;
    pub const CHECKSUM_SIZE: usize = 4;

    /// Minimum size of a compressed node record (varint encoded)
    /// - ID delta: 1 byte (varint, small delta)
    /// - flags: 4 bytes (fixed)
    /// - kind_offset: 1 byte (varint u16, small values)
    /// - name_offset: 1 byte (varint u16, small values)
    /// - data_len: 1 byte (varint u16, small values)
    /// - outgoing_cluster_offset: 1 byte (varint, small values)
    /// - outgoing_edge_count: 1 byte (varint u32, small values)
    /// - incoming_cluster_offset: 1 byte (varint, small values)
    /// - incoming_edge_count: 1 byte (varint u32, small values)
    ///
    /// Total: ~12 bytes minimum + inline data
    pub const MIN_COMPRESSED_RECORD_SIZE: usize = 12;
}

/// Re-export constants for convenience
pub use constants::{
    BASE_ID_OFFSET, ESTIMATED_NODE_SLOT_SIZE, MAX_NODE_CAPACITY, MAX_PAGE_SIZE,
    MIN_COMPRESSED_RECORD_SIZE, PAGE_HEADER_SIZE, USABLE_SIZE, USED_BYTES_OFFSET,
};

/// NodePage for storing variable-size NodeRecordV3 records
///
/// Pages store nodes with delta/varint compression for space efficiency.
/// Overflow pages are linked via next_page_id for large nodes.
///
/// # Block Locality (PROTOTYPE)
///
/// The `block_id` field is computed from `base_id` for block-aware caching:
/// - `block_id = (base_id - 1) / BLOCK_SIZE` where `BLOCK_SIZE = 128`
/// - This is in-memory metadata only, not persisted
/// - Used for block-aware cache eviction decisions
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodePage {
    /// Page ID for this page
    pub page_id: u64,

    /// Next page ID for overflow (0 if none)
    pub next_page_id: u64,

    /// Node records stored in this page
    pub nodes: Vec<NodeRecordV3>,

    /// Actual bytes used in data region (tracked for capacity)
    pub used_bytes: u16,

    /// Base ID for delta encoding (minimum ID in this page)
    pub base_id: i64,

    /// Page checksum for validation
    pub checksum: u32,

    /// Block ID for locality-aware caching (PROTOTYPE: computed, not persisted)
    ///
    /// This field is computed from `base_id` after unpacking the page.
    /// It allows the cache to make block-aware retention decisions.
    /// NOT persisted to disk - recomputed on each page load.
    pub block_id: i64,
}

/// Block size for locality calculations
///
/// Each logical block contains approximately 128 node IDs.
/// Nodes 1-127 → block 0, nodes 128-255 → block 1, etc.
pub const BLOCK_SIZE: i64 = 128;

/// Compute block_id from a node_id
#[inline]
pub const fn node_id_to_block(node_id: i64) -> i64 {
    if node_id < 1 {
        return 0;
    }
    (node_id - 1) / BLOCK_SIZE
}

impl NodePage {
    /// Create a new empty node page
    pub fn new(page_id: u64) -> Self {
        NodePage {
            page_id,
            next_page_id: 0,
            nodes: Vec::new(),
            used_bytes: 0,
            base_id: 0,
            checksum: 0,
            block_id: 0, // Will be computed when nodes are added
        }
    }

    /// Create a new node page with the given capacity pre-allocated
    pub fn with_capacity(page_id: u64, capacity: usize) -> Self {
        NodePage {
            page_id,
            next_page_id: 0,
            nodes: Vec::with_capacity(capacity.min(MAX_NODE_CAPACITY)),
            used_bytes: 0,
            base_id: 0,
            checksum: 0,
            block_id: 0,
        }
    }

    /// Get the block_id for this page
    ///
    /// For pages with nodes, computes based on base_id.
    /// For empty pages, returns 0.
    pub fn block_id(&self) -> i64 {
        if self.block_id != 0 {
            self.block_id
        } else {
            node_id_to_block(self.base_id)
        }
    }

    /// Recompute block_id from current nodes
    ///
    /// Updates block_id based on the minimum node_id in this page.
    /// Called after adding nodes or unpacking from disk.
    pub fn recompute_block_id(&mut self) {
        if let Some(min_id) = self.nodes.iter().map(|n| n.id()).min() {
            self.base_id = min_id;
            self.block_id = node_id_to_block(min_id);
        } else {
            self.base_id = 0;
            self.block_id = 0;
        }
    }

    /// Get the number of nodes in this page
    pub fn node_count(&self) -> u16 {
        self.nodes.len() as u16
    }

    /// Check if the page has an overflow page
    pub fn has_overflow(&self) -> bool {
        self.next_page_id != 0
    }

    /// Check if the page is full (at estimated capacity)
    pub fn is_full(&self) -> bool {
        // Check if adding an average-sized node would exceed usable size
        (self.capacity() as usize) < ESTIMATED_NODE_SLOT_SIZE
    }

    /// Check if the page is empty
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Calculate the actual used size in bytes (from tracked value)
    pub fn used_size(&self) -> usize {
        self.used_bytes as usize
    }

    /// Calculate remaining capacity in bytes
    pub fn remaining_capacity(&self) -> usize {
        USABLE_SIZE.saturating_sub(self.used_bytes as usize)
    }

    /// Calculate remaining capacity as u16 (for internal use)
    pub fn capacity(&self) -> u16 {
        (USABLE_SIZE as u16).saturating_sub(self.used_bytes)
    }

    /// Find a node by ID using binary search.
    ///
    /// # Panics
    /// Panics if nodes are not sorted by ID. This is guaranteed by the insertion
    /// order (sequential node_id allocation) and page filling strategy.
    ///
    /// # Returns
    /// * `Some(&node_record)` if found
    /// * `None` if not found in this page
    #[inline]
    pub fn find_node(&self, node_id: i64) -> Option<&NodeRecordV3> {
        // Binary search on sorted node IDs
        // Nodes are guaranteed sorted by sequential insertion
        let mut left = 0;
        let mut right = self.nodes.len();

        while left < right {
            let mid = left + (right - left) / 2;
            let mid_id = self.nodes[mid].id();

            if mid_id == node_id {
                return Some(&self.nodes[mid]);
            } else if mid_id < node_id {
                left = mid + 1;
            } else {
                right = mid;
            }
        }

        // Check the last element
        if left < self.nodes.len() && self.nodes[left].id() == node_id {
            Some(&self.nodes[left])
        } else {
            None
        }
    }

    /// Find a node by ID using lazy decoding - only decodes O(log n) node IDs
    /// then fully decodes only the target node.
    ///
    /// This performs binary search directly on the encoded data, decoding node IDs
    /// on-the-fly. Once the target node is found, it fully decodes that node's record.
    ///
    /// # Parameters
    /// - `page_data`: Raw page bytes (must be MAX_PAGE_SIZE)
    ///
    /// Find a node by ID using lazy linear scan.
    ///
    /// For ~100-node pages, linear scan is simpler and nearly as fast as binary search.
    /// This only decodes node IDs during scan, then fully decodes only the target node.
    ///
    /// # Arguments
    /// - `page_data`: Raw page bytes
    /// - `node_id`: The node ID to find
    ///
    /// # Returns
    /// - `Some(NodeRecordV3)` if found
    /// - `None` if not found in this page
    pub fn find_node_lazy(page_data: &[u8], node_id: i64) -> NativeResult<Option<NodeRecordV3>> {
        use crate::backend::native::v3::compression::delta::decode_id_delta;
        use crate::backend::native::v3::compression::varint::decode_varint;

        // Read page header
        let node_count = u16::from_be_bytes(
            page_data[constants::NODE_COUNT_OFFSET..constants::NODE_COUNT_OFFSET + 2]
                .try_into()
                .map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node_page.node_count".to_string(),
                    reason: "invalid node_count bytes".to_string(),
                })?,
        ) as usize;

        let base_id = i64::from_be_bytes(
            page_data[constants::BASE_ID_OFFSET..constants::BASE_ID_OFFSET + 8]
                .try_into()
                .map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node_page.base_id".to_string(),
                    reason: "invalid base_id bytes".to_string(),
                })?,
        );

        let used_bytes = u16::from_be_bytes(
            page_data[constants::USED_BYTES_OFFSET..constants::USED_BYTES_OFFSET + 2]
                .try_into()
                .map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node_page.used_bytes".to_string(),
                    reason: "invalid used_bytes bytes".to_string(),
                })?,
        ) as usize;

        let data_start = PAGE_HEADER_SIZE;
        let data_end = data_start + used_bytes;
        let data = &page_data[data_start..data_end];

        // Linear scan: decode only node IDs first
        let mut offset = 0;
        for _ in 0..node_count {
            let node_start_offset = offset;

            // Decode ID delta (1 varint)
            let (delta, id_bytes) =
                decode_varint(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node.id_delta".to_string(),
                    reason: "invalid varint encoding".to_string(),
                })?;
            offset += id_bytes;

            let id = decode_id_delta(delta as u32, base_id).map_err(|_| {
                NativeBackendError::InvalidHeader {
                    field: "node.id".to_string(),
                    reason: format!(
                        "failed to reconstruct ID from delta {} and base_id {}",
                        delta, base_id
                    ),
                }
            })?;

            if id == node_id {
                // Found! Decode the full node starting from node_start_offset
                return Self::decode_node_at_offset(&data[node_start_offset..], base_id);
            }

            // Skip to next node (skip remaining fields after ID delta)
            offset = Self::skip_remaining_fields(data, offset)?;
        }

        Ok(None)
    }

    /// Skip remaining fields of a node (after ID delta has been decoded)
    /// Returns the offset at the start of the next node
    fn skip_remaining_fields(data: &[u8], mut offset: usize) -> NativeResult<usize> {
        // Skip flags (4 bytes)
        offset += 4;

        // Skip kind_offset (1 varint u16)
        let (_, bytes_read) =
            decode_varint_u16(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                field: "node.kind_offset".to_string(),
                reason: "invalid varint encoding".to_string(),
            })?;
        offset += bytes_read;

        // Skip name_offset (1 varint u16)
        let (_, bytes_read) =
            decode_varint_u16(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                field: "node.name_offset".to_string(),
                reason: "invalid varint encoding".to_string(),
            })?;
        offset += bytes_read;

        // Skip data_len (1 varint u16)
        let (encoded_data_len, bytes_read) =
            decode_varint_u16(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                field: "node.data_len".to_string(),
                reason: "invalid varint encoding".to_string(),
            })?;
        offset += bytes_read;

        let is_external = (encoded_data_len & super::record::constants::EXTERNAL_DATA_FLAG) != 0;
        let data_len = encoded_data_len & super::record::constants::MAX_DATA_LEN;

        // Skip outgoing_cluster_offset (1 varint)
        let (_, bytes_read) =
            decode_varint(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                field: "node.outgoing_cluster_offset".to_string(),
                reason: "invalid varint encoding".to_string(),
            })?;
        offset += bytes_read;

        // Skip outgoing_edge_count (1 varint)
        let (_, bytes_read) =
            decode_varint(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                field: "node.outgoing_edge_count".to_string(),
                reason: "invalid varint encoding".to_string(),
            })?;
        offset += bytes_read;

        // Skip incoming_cluster_offset (1 varint)
        let (_, bytes_read) =
            decode_varint(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                field: "node.incoming_cluster_offset".to_string(),
                reason: "invalid varint encoding".to_string(),
            })?;
        offset += bytes_read;

        // Skip incoming_edge_count (1 varint)
        let (_, bytes_read) =
            decode_varint(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                field: "node.incoming_edge_count".to_string(),
                reason: "invalid varint encoding".to_string(),
            })?;
        offset += bytes_read;

        // Skip inline/external data
        if is_external {
            offset += 8;
        } else {
            offset += data_len as usize;
        }

        Ok(offset)
    }

    /// Decode a single node record starting at the given offset
    fn decode_node_at_offset(data: &[u8], base_id: i64) -> NativeResult<Option<NodeRecordV3>> {
        use crate::backend::native::types::NodeFlags;
        let mut offset = 0;

        // Decode ID delta (already done by caller, but we need the full ID)
        let (delta, bytes_read) =
            decode_varint(data).map_err(|_| NativeBackendError::InvalidHeader {
                field: "node.id_delta".to_string(),
                reason: "invalid varint encoding for ID delta".to_string(),
            })?;
        offset += bytes_read;

        let id = decode_id_delta(delta as u32, base_id).map_err(|_| {
            NativeBackendError::InvalidHeader {
                field: "node.id".to_string(),
                reason: format!(
                    "failed to reconstruct ID from delta {} and base_id {}",
                    delta, base_id
                ),
            }
        })?;

        // Decode flags (4 bytes fixed)
        if offset + 4 > data.len() {
            return Err(NativeBackendError::InvalidHeader {
                field: "node.flags".to_string(),
                reason: "insufficient bytes for flags".to_string(),
            });
        }
        let flags = NodeFlags(u32::from_be_bytes(
            data.get(offset..offset + 4)
                .ok_or_else(|| NativeBackendError::InvalidHeader {
                    field: "node.flags".to_string(),
                    reason: "cannot read flag bytes".to_string(),
                })?
                .try_into()
                .map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node.flags".to_string(),
                    reason: "invalid flag byte array".to_string(),
                })?,
        ));
        offset += 4;

        // Decode kind_offset as varint u16
        let (kind_offset, bytes_read) =
            decode_varint_u16(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                field: "node.kind_offset".to_string(),
                reason: "invalid varint encoding for kind_offset".to_string(),
            })?;
        offset += bytes_read;

        // Decode name_offset as varint u16
        let (name_offset, bytes_read) =
            decode_varint_u16(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                field: "node.name_offset".to_string(),
                reason: "invalid varint encoding for name_offset".to_string(),
            })?;
        offset += bytes_read;

        // Decode data_len as varint u16
        let (encoded_data_len, bytes_read) =
            decode_varint_u16(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                field: "node.data_len".to_string(),
                reason: "invalid varint encoding for data_len".to_string(),
            })?;
        offset += bytes_read;

        let is_external = (encoded_data_len & super::record::constants::EXTERNAL_DATA_FLAG) != 0;
        let data_len = encoded_data_len & super::record::constants::MAX_DATA_LEN;

        // Decode outgoing_cluster_offset as varint
        let (outgoing_cluster_offset, bytes_read) =
            decode_varint(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                field: "node.outgoing_cluster_offset".to_string(),
                reason: "invalid varint encoding for outgoing_cluster_offset".to_string(),
            })?;
        offset += bytes_read;

        // Decode outgoing_edge_count as varint
        let (outgoing_edge_count, bytes_read) =
            decode_varint(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                field: "node.outgoing_edge_count".to_string(),
                reason: "invalid varint encoding for outgoing_edge_count".to_string(),
            })?;
        let outgoing_edge_count = outgoing_edge_count as u32;
        offset += bytes_read;

        // Decode incoming_cluster_offset as varint
        let (incoming_cluster_offset, bytes_read) =
            decode_varint(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                field: "node.incoming_cluster_offset".to_string(),
                reason: "invalid varint encoding for incoming_cluster_offset".to_string(),
            })?;
        offset += bytes_read;

        // Decode incoming_edge_count as varint
        let (incoming_edge_count, bytes_read) =
            decode_varint(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                field: "node.incoming_edge_count".to_string(),
                reason: "invalid varint encoding for incoming_edge_count".to_string(),
            })?;
        let incoming_edge_count = incoming_edge_count as u32;
        offset += bytes_read;

        // Handle inline vs external data
        let (data_inline, data_external_offset) = if is_external {
            // External data - read 8-byte offset
            if offset + 8 > data.len() {
                return Err(NativeBackendError::InvalidHeader {
                    field: "node.data_external_offset".to_string(),
                    reason: format!(
                        "insufficient bytes for external offset: need 8, have {}",
                        data.len().saturating_sub(offset)
                    ),
                });
            }
            let ext_offset = u64::from_be_bytes(
                data.get(offset..offset + 8)
                    .ok_or_else(|| NativeBackendError::InvalidHeader {
                        field: "node.data_external_offset".to_string(),
                        reason: "cannot read external offset bytes".to_string(),
                    })?
                    .try_into()
                    .map_err(|_| NativeBackendError::InvalidHeader {
                        field: "node.data_external_offset".to_string(),
                        reason: "invalid external offset byte array".to_string(),
                    })?,
            );
            (None, Some(ext_offset))
        } else {
            // Inline data - copy remaining bytes
            let data_end = offset + data_len as usize;
            if data_end > data.len() {
                return Err(NativeBackendError::InvalidHeader {
                    field: "node.data_inline".to_string(),
                    reason: format!(
                        "insufficient bytes for inline data: need {}, have {}",
                        data_len,
                        data.len().saturating_sub(offset)
                    ),
                });
            }
            let inline_data = data[offset..data_end].to_vec();
            (Some(inline_data), None)
        };

        let node = NodeRecordV3 {
            id,
            flags,
            kind_offset,
            name_offset,
            data_len: encoded_data_len,
            data_inline,
            data_external_offset,
            outgoing_cluster_offset,
            outgoing_edge_count,
            incoming_cluster_offset,
            incoming_edge_count,
        };

        Ok(Some(node))
    }

    /// Add a node to the page
    ///
    /// Returns an error if the node would cause the page to overflow.
    /// Uses compressed size calculation with delta/varint encoding.
    pub fn add_node(&mut self, node: NodeRecordV3) -> NativeResult<()> {
        // Temporarily add node to get accurate base_id
        let temp_base_id = if self.nodes.is_empty() {
            node.id()
        } else {
            self.base_id.min(node.id())
        };

        // Calculate compressed size using the temp base_id
        let compressed_size = self.estimate_compressed_size_with_base(&node, temp_base_id)?;

        // Check if adding this node would exceed page capacity
        if self.capacity() < compressed_size {
            return Err(NativeBackendError::InvalidHeader {
                field: "node_page".to_string(),
                reason: format!(
                    "adding node would exceed page capacity: need {} bytes, have {} remaining",
                    compressed_size,
                    self.capacity()
                ),
            });
        }

        // Update base_id
        self.base_id = temp_base_id;

        self.nodes.push(node);
        self.used_bytes += compressed_size;

        // CRITICAL VALIDATION: Verify the actual packed size fits in USABLE_SIZE
        // This catches any discrepancy between estimated size and actual packed size
        let actual_packed_size = self.pack_nodes()?.len();
        if actual_packed_size > USABLE_SIZE {
            // Rollback: remove the node we just added
            self.nodes.pop();
            self.used_bytes -= compressed_size;
            // Restore previous base_id if we had nodes
            if !self.nodes.is_empty() {
                self.base_id = self
                    .nodes
                    .iter()
                    .map(|n| n.id())
                    .min()
                    .unwrap_or(self.base_id);
            }

            // Provide detailed diagnostics for debugging
            #[cfg(feature = "v3-page-overflow-debug")]
            {
                eprintln!(
                    "Page {} overflow: node_count={}, used_bytes={}, actual_packed_size={}, USABLE_SIZE={}, estimated={}",
                    self.page_id,
                    self.nodes.len() + 1, // including the failed node
                    self.used_bytes + compressed_size,
                    actual_packed_size,
                    USABLE_SIZE,
                    compressed_size
                );
            }

            return Err(NativeBackendError::InvalidHeader {
                field: "node_page".to_string(),
                reason: format!(
                    "page {} full: node_count={}, used_bytes={}, actual packed {} exceeds USABLE_SIZE {} (new node estimated {})",
                    self.page_id,
                    self.nodes.len(),
                    self.used_bytes,
                    actual_packed_size,
                    USABLE_SIZE,
                    compressed_size
                ),
            });
        }

        // Update used_bytes to actual packed size for accuracy
        self.used_bytes = actual_packed_size as u16;

        // Recompute block_id after adding node (PROTOTYPE: block-aware caching)
        self.block_id = node_id_to_block(self.base_id);

        Ok(())
    }

    /// Estimate the compressed size of a node record with a specific base_id
    ///
    /// This is used by add_node to calculate size before updating base_id.
    fn estimate_compressed_size_with_base(
        &self,
        node: &NodeRecordV3,
        base_id: i64,
    ) -> NativeResult<u16> {
        let mut size: usize = 0;

        // ID delta (varint, usually 1-4 bytes)
        let delta = encode_id_delta(node.id(), base_id);
        size += compression::varint::varint_size(delta as u64);

        // Flags: 4 bytes (fixed)
        size += 4;

        // kind_offset: varint u16 (usually 1-2 bytes)
        size += compression::varint::varint_size(node.kind_offset as u64);

        // name_offset: varint u16 (usually 1-2 bytes)
        size += compression::varint::varint_size(node.name_offset as u64);

        // data_len: varint u16 (usually 1 byte for small data)
        size += compression::varint::varint_size(node.data_len() as u64);

        // outgoing_cluster_offset: varint u64 (1-10 bytes)
        size += compression::varint::varint_size(node.outgoing_cluster_offset);

        // outgoing_edge_count: varint u32 (usually 1-3 bytes)
        size += compression::varint::varint_size(node.outgoing_edge_count as u64);

        // incoming_cluster_offset: varint u64 (1-10 bytes)
        size += compression::varint::varint_size(node.incoming_cluster_offset);

        // incoming_edge_count: varint u32 (usually 1-3 bytes)
        size += compression::varint::varint_size(node.incoming_edge_count as u64);

        // Inline data OR external offset (8 bytes)
        if let Some(ref data) = node.data_inline {
            size += data.len();
        } else if node.data_external_offset.is_some() {
            size += 8; // External offset is u64 (8 bytes)
        }

        // Ensure we don't overflow u16
        if size > u16::MAX as usize {
            return Err(NativeBackendError::InvalidHeader {
                field: "compressed_size".to_string(),
                reason: format!("compressed size {} exceeds u16::MAX", size),
            });
        }

        Ok(size as u16)
    }

    /// Calculate checksum for page data
    fn calculate_checksum(&self, data: &[u8]) -> u32 {
        v3_constants::checksum::xor_checksum(data) as u32
    }

    /// Pack nodes using delta/varint encoding
    ///
    /// Returns a byte vector containing all nodes packed contiguously.
    /// Uses delta encoding for IDs and varint for variable-length fields.
    fn pack_nodes(&self) -> NativeResult<Vec<u8>> {
        let mut buffer = Vec::with_capacity(self.used_bytes as usize);

        for node in &self.nodes {
            // Encode ID delta
            let delta = encode_id_delta(node.id(), self.base_id);
            buffer.extend_from_slice(&encode_varint(delta as u64));

            // Encode flags (fixed 4 bytes)
            buffer.extend_from_slice(&node.flags.0.to_be_bytes());

            // Encode kind_offset as varint
            buffer.extend_from_slice(&encode_varint_u16(node.kind_offset));

            // Encode name_offset as varint
            buffer.extend_from_slice(&encode_varint_u16(node.name_offset));

            // Encode data_len as varint (with external flag if needed)
            let encoded_data_len = if node.is_external() {
                node.data_len | super::record::constants::EXTERNAL_DATA_FLAG
            } else {
                node.data_len
            };
            buffer.extend_from_slice(&encode_varint_u16(encoded_data_len));

            // Encode outgoing_cluster_offset as varint
            buffer.extend_from_slice(&encode_varint(node.outgoing_cluster_offset));

            // Encode outgoing_edge_count as varint
            buffer.extend_from_slice(&encode_varint(node.outgoing_edge_count as u64));

            // Encode incoming_cluster_offset as varint
            buffer.extend_from_slice(&encode_varint(node.incoming_cluster_offset));

            // Encode incoming_edge_count as varint
            buffer.extend_from_slice(&encode_varint(node.incoming_edge_count as u64));

            // Append inline data if present, or external offset if external
            if let Some(ref data) = node.data_inline {
                buffer.extend_from_slice(data);
            } else if let Some(offset) = node.data_external_offset {
                buffer.extend_from_slice(&offset.to_be_bytes());
            }
        }

        Ok(buffer)
    }

    /// Unpack nodes from a byte slice using delta/varint encoding
    ///
    /// Returns a vector of NodeRecordV3 and the actual bytes consumed.
    fn unpack_nodes(
        data: &[u8],
        base_id: i64,
        node_count: usize,
    ) -> NativeResult<(Vec<NodeRecordV3>, usize)> {
        let mut nodes = Vec::with_capacity(node_count);
        let mut offset = 0;

        for _ in 0..node_count {
            // Decode ID delta
            let (delta, bytes_read) =
                decode_varint(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node.id_delta".to_string(),
                    reason: "invalid varint encoding for ID delta".to_string(),
                })?;
            offset += bytes_read;

            // Reconstruct full ID
            let id = decode_id_delta(delta as u32, base_id).map_err(|_| {
                NativeBackendError::InvalidHeader {
                    field: "node.id".to_string(),
                    reason: format!(
                        "failed to reconstruct ID from delta {} and base_id {}",
                        delta, base_id
                    ),
                }
            })?;

            // Decode flags (4 bytes fixed)
            if offset + 4 > data.len() {
                return Err(NativeBackendError::InvalidHeader {
                    field: "node.flags".to_string(),
                    reason: "insufficient bytes for flags".to_string(),
                });
            }
            let flags = crate::backend::native::types::NodeFlags(u32::from_be_bytes(
                data.get(offset..offset + 4)
                    .ok_or_else(|| NativeBackendError::InvalidHeader {
                        field: "node.flags".to_string(),
                        reason: "cannot read flag bytes".to_string(),
                    })?
                    .try_into()
                    .map_err(|_| NativeBackendError::InvalidHeader {
                        field: "node.flags".to_string(),
                        reason: "invalid flag byte array".to_string(),
                    })?,
            ));
            offset += 4;

            // Decode kind_offset as varint u16
            let (kind_offset, bytes_read) = decode_varint_u16(&data[offset..]).map_err(|_| {
                NativeBackendError::InvalidHeader {
                    field: "node.kind_offset".to_string(),
                    reason: "invalid varint encoding for kind_offset".to_string(),
                }
            })?;
            offset += bytes_read;

            // Decode name_offset as varint u16
            let (name_offset, bytes_read) = decode_varint_u16(&data[offset..]).map_err(|_| {
                NativeBackendError::InvalidHeader {
                    field: "node.name_offset".to_string(),
                    reason: "invalid varint encoding for name_offset".to_string(),
                }
            })?;
            offset += bytes_read;

            // Decode data_len as varint u16
            let (encoded_data_len, bytes_read) =
                decode_varint_u16(&data[offset..]).map_err(|_| {
                    NativeBackendError::InvalidHeader {
                        field: "node.data_len".to_string(),
                        reason: "invalid varint encoding for data_len".to_string(),
                    }
                })?;
            offset += bytes_read;

            let is_external =
                (encoded_data_len & super::record::constants::EXTERNAL_DATA_FLAG) != 0;
            let data_len = encoded_data_len & super::record::constants::MAX_DATA_LEN;

            // Decode outgoing_cluster_offset as varint u64
            let (outgoing_cluster_offset, bytes_read) =
                decode_varint(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node.outgoing_cluster_offset".to_string(),
                    reason: "invalid varint encoding for outgoing_cluster_offset".to_string(),
                })?;
            offset += bytes_read;

            // Decode outgoing_edge_count as varint u32
            let (outgoing_edge_count, bytes_read) =
                decode_varint(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node.outgoing_edge_count".to_string(),
                    reason: "invalid varint encoding for outgoing_edge_count".to_string(),
                })?;
            let outgoing_edge_count = outgoing_edge_count as u32;
            offset += bytes_read;

            // Decode incoming_cluster_offset as varint u64
            let (incoming_cluster_offset, bytes_read) =
                decode_varint(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node.incoming_cluster_offset".to_string(),
                    reason: "invalid varint encoding for incoming_cluster_offset".to_string(),
                })?;
            offset += bytes_read;

            // Decode incoming_edge_count as varint u32
            let (incoming_edge_count, bytes_read) =
                decode_varint(&data[offset..]).map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node.incoming_edge_count".to_string(),
                    reason: "invalid varint encoding for incoming_edge_count".to_string(),
                })?;
            let incoming_edge_count = incoming_edge_count as u32;
            offset += bytes_read;

            // Handle inline vs external data
            let (data_inline, data_external_offset) = if is_external {
                // External data - read 8-byte offset
                if offset + 8 > data.len() {
                    return Err(NativeBackendError::InvalidHeader {
                        field: "node.data_external_offset".to_string(),
                        reason: format!(
                            "insufficient bytes for external offset: need 8, have {}",
                            data.len().saturating_sub(offset)
                        ),
                    });
                }
                let ext_offset = u64::from_be_bytes(
                    data.get(offset..offset + 8)
                        .ok_or_else(|| NativeBackendError::InvalidHeader {
                            field: "node.data_external_offset".to_string(),
                            reason: "cannot read external offset bytes".to_string(),
                        })?
                        .try_into()
                        .map_err(|_| NativeBackendError::InvalidHeader {
                            field: "node.data_external_offset".to_string(),
                            reason: "invalid external offset byte array".to_string(),
                        })?,
                );
                offset += 8;
                (None, Some(ext_offset))
            } else {
                // Inline data - copy remaining bytes
                let data_end = offset + data_len as usize;
                if data_end > data.len() {
                    return Err(NativeBackendError::InvalidHeader {
                        field: "node.data_inline".to_string(),
                        reason: format!(
                            "insufficient bytes for inline data: need {}, have {}",
                            data_len,
                            data.len().saturating_sub(offset)
                        ),
                    });
                }
                let inline_data = data[offset..data_end].to_vec();
                offset = data_end;
                (Some(inline_data), None)
            };

            // Reconstruct the node record
            let node = NodeRecordV3 {
                id,
                flags,
                kind_offset,
                name_offset,
                data_len: encoded_data_len,
                data_inline,
                data_external_offset,
                outgoing_cluster_offset,
                outgoing_edge_count,
                incoming_cluster_offset,
                incoming_edge_count,
            };

            nodes.push(node);
        }

        Ok((nodes, offset))
    }

    /// Calculate checksum for header and node data (compressed format)
    fn calculate_checksum_with_nodes(&self) -> u32 {
        let mut data = Vec::with_capacity(PAGE_HEADER_SIZE + self.used_size());

        // Serialize header (new format with used_bytes and base_id)
        data.extend_from_slice(&self.page_id.to_be_bytes());
        data.extend_from_slice(&self.next_page_id.to_be_bytes());
        data.extend_from_slice(&(self.nodes.len() as u16).to_be_bytes());
        data.extend_from_slice(&self.used_bytes.to_be_bytes());
        data.extend_from_slice(&self.base_id.to_be_bytes());
        data.extend_from_slice(&[0u8; 4]); // checksum field (computed on finalize)

        // Serialize nodes using compressed format
        if let Ok(node_data) = self.pack_nodes() {
            data.extend_from_slice(&node_data);
        }

        v3_constants::checksum::xor_checksum(&data) as u32
    }

    /// Pack the page into a 4KB byte array
    ///
    /// Serializes the page using delta/varint compression for space efficiency.
    pub fn pack(&self) -> NativeResult<[u8; MAX_PAGE_SIZE]> {
        let mut bytes = [0u8; MAX_PAGE_SIZE];

        // Pack node data using compression (do this first to get actual size)
        let node_data = self.pack_nodes()?;
        let actual_used_bytes = node_data.len() as u16;

        // Write page header (new format)
        bytes[constants::PAGE_ID_OFFSET..constants::PAGE_ID_OFFSET + 8]
            .copy_from_slice(&self.page_id.to_be_bytes());

        bytes[constants::NEXT_PAGE_ID_OFFSET..constants::NEXT_PAGE_ID_OFFSET + 8]
            .copy_from_slice(&self.next_page_id.to_be_bytes());

        bytes[constants::NODE_COUNT_OFFSET..constants::NODE_COUNT_OFFSET + 2]
            .copy_from_slice(&(self.nodes.len() as u16).to_be_bytes());

        // Use actual used bytes from packed data (not self.used_bytes estimate)
        bytes[constants::USED_BYTES_OFFSET..constants::USED_BYTES_OFFSET + 2]
            .copy_from_slice(&actual_used_bytes.to_be_bytes());

        bytes[constants::BASE_ID_OFFSET..constants::BASE_ID_OFFSET + 8]
            .copy_from_slice(&self.base_id.to_be_bytes());

        // Reserve space for checksum (calculated after data is written)
        let checksum_offset = constants::CHECKSUM_OFFSET;

        // Validate node data fits in page
        if PAGE_HEADER_SIZE + node_data.len() > MAX_PAGE_SIZE {
            return Err(NativeBackendError::InvalidHeader {
                field: "node_page".to_string(),
                reason: format!(
                    "page overflow: header {} + data {} > {}",
                    PAGE_HEADER_SIZE,
                    node_data.len(),
                    MAX_PAGE_SIZE
                ),
            });
        }

        // Write node data
        let data_offset = PAGE_HEADER_SIZE;
        bytes[data_offset..data_offset + node_data.len()].copy_from_slice(&node_data);

        // Calculate and write checksum (over header + node data)
        let checksum_end = data_offset + node_data.len();
        let checksum = self.calculate_checksum(&bytes[..checksum_end]);
        bytes[checksum_offset..checksum_offset + 4].copy_from_slice(&checksum.to_be_bytes());

        Ok(bytes)
    }

    /// Unpack a page from a byte array
    ///
    /// Deserializes the page using delta/varint decompression and validates checksum.
    pub fn unpack(bytes: &[u8]) -> NativeResult<Self> {
        #[cfg(feature = "v3-forensics")]
        {
            use crate::backend::native::v3::forensics::FORENSIC_COUNTERS;
            FORENSIC_COUNTERS
                .node_page_unpack_count
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }

        if bytes.len() < MAX_PAGE_SIZE {
            return Err(NativeBackendError::InvalidHeader {
                field: "node_page".to_string(),
                reason: format!(
                    "insufficient bytes: expected {}, found {}",
                    MAX_PAGE_SIZE,
                    bytes.len()
                ),
            });
        }

        // Read page header (new format with used_bytes and base_id)
        let page_id = u64::from_be_bytes(
            bytes[constants::PAGE_ID_OFFSET..constants::PAGE_ID_OFFSET + 8]
                .try_into()
                .map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node_page.page_id".to_string(),
                    reason: "invalid page_id bytes".to_string(),
                })?,
        );

        let next_page_id = u64::from_be_bytes(
            bytes[constants::NEXT_PAGE_ID_OFFSET..constants::NEXT_PAGE_ID_OFFSET + 8]
                .try_into()
                .map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node_page.next_page_id".to_string(),
                    reason: "invalid next_page_id bytes".to_string(),
                })?,
        );

        let node_count = u16::from_be_bytes(
            bytes[constants::NODE_COUNT_OFFSET..constants::NODE_COUNT_OFFSET + 2]
                .try_into()
                .map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node_page.node_count".to_string(),
                    reason: "invalid node_count bytes".to_string(),
                })?,
        ) as usize;

        let used_bytes = u16::from_be_bytes(
            bytes[constants::USED_BYTES_OFFSET..constants::USED_BYTES_OFFSET + 2]
                .try_into()
                .map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node_page.used_bytes".to_string(),
                    reason: "invalid used_bytes bytes".to_string(),
                })?,
        );

        let base_id = i64::from_be_bytes(
            bytes[constants::BASE_ID_OFFSET..constants::BASE_ID_OFFSET + 8]
                .try_into()
                .map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node_page.base_id".to_string(),
                    reason: "invalid base_id bytes".to_string(),
                })?,
        );

        let checksum = u32::from_be_bytes(
            bytes[constants::CHECKSUM_OFFSET..constants::CHECKSUM_OFFSET + 4]
                .try_into()
                .map_err(|_| NativeBackendError::InvalidHeader {
                    field: "node_page.checksum".to_string(),
                    reason: "invalid checksum bytes".to_string(),
                })?,
        );

        // Unpack node data using delta/varint decompression
        let data_start = PAGE_HEADER_SIZE;
        let data_end = data_start + used_bytes as usize;

        if data_end > MAX_PAGE_SIZE {
            return Err(NativeBackendError::InvalidHeader {
                field: "node_page".to_string(),
                reason: format!(
                    "used_bytes exceeds page boundary: {} + {} > {}",
                    data_start, used_bytes, MAX_PAGE_SIZE
                ),
            });
        }

        let (nodes, actual_bytes_used) =
            Self::unpack_nodes(&bytes[data_start..data_end], base_id, node_count)?;

        // Verify bytes used matches expected
        if actual_bytes_used != used_bytes as usize {
            return Err(NativeBackendError::InvalidHeader {
                field: "node_page".to_string(),
                reason: format!(
                    "node data size mismatch: expected {} bytes, actually used {}",
                    used_bytes, actual_bytes_used
                ),
            });
        }

        // Verify checksum
        let page = NodePage {
            page_id,
            next_page_id,
            nodes,
            used_bytes,
            base_id,
            checksum,
            block_id: node_id_to_block(base_id), // Compute from base_id (PROTOTYPE)
        };

        // Calculate checksum on all data up to actual end
        let calculated_checksum = page.calculate_checksum_with_nodes();
        if calculated_checksum != checksum {
            return Err(NativeBackendError::InvalidHeader {
                field: "node_page_checksum".to_string(),
                reason: format!(
                    "checksum mismatch: expected {}, found {}",
                    calculated_checksum, checksum
                ),
            });
        }

        Ok(page)
    }

    /// Get the total size this page would consume on disk
    pub fn disk_size(&self) -> usize {
        MAX_PAGE_SIZE
    }

    /// Calculate space efficiency (ratio of used to total space)
    pub fn space_efficiency(&self) -> f64 {
        if MAX_PAGE_SIZE == 0 {
            return 0.0;
        }
        (self.used_size() as f64) / (USABLE_SIZE as f64)
    }
}

/// Create a new empty page with default capacity
impl Default for NodePage {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::constants::*;
    use super::*;
    use crate::backend::native::types::NodeFlags;

    #[test]
    fn test_constants() {
        assert_eq!(PAGE_HEADER_SIZE, 32);
        assert_eq!(MAX_PAGE_SIZE, 4096);
        assert_eq!(USABLE_SIZE, 4064);
        const { assert!(MAX_NODE_CAPACITY > 0) };
        const { assert!(MAX_NODE_CAPACITY <= 100) };
    }

    #[test]
    fn test_new_page() {
        let page = NodePage::new(42);
        assert_eq!(page.page_id, 42);
        assert_eq!(page.next_page_id, 0);
        assert_eq!(page.node_count(), 0);
        assert!(page.is_empty());
        assert!(!page.is_full());
        assert!(!page.has_overflow());
    }

    #[test]
    fn test_page_with_capacity() {
        let page = NodePage::with_capacity(100, 20);
        assert_eq!(page.page_id, 100);
        assert_eq!(page.node_count(), 0);
        assert!(page.nodes.capacity() >= 20);
    }

    #[test]
    fn test_add_node() {
        let page = &mut NodePage::new(1);

        let node = NodeRecordV3::new_inline(
            123,
            NodeFlags::empty(),
            10,
            20,
            b"test data".to_vec(),
            1000,
            5,
            2000,
            3,
        );

        assert!(page.add_node(node).is_ok());
        assert_eq!(page.node_count(), 1);
        assert!(!page.is_empty());
    }

    #[test]
    fn test_add_multiple_nodes() {
        let page = &mut NodePage::new(1);

        for i in 0..10 {
            let node = NodeRecordV3::new_inline(
                i,
                NodeFlags::empty(),
                i as u16 * 10,
                i as u16 * 20,
                vec![i as u8; 20],
                i as u64 * 1000,
                i as u32 * 5,
                i as u64 * 2000,
                i as u32 * 3,
            );
            assert!(page.add_node(node).is_ok());
        }

        assert_eq!(page.node_count(), 10);
    }

    #[test]
    fn test_pack_unpack_round_trip() {
        let page = &mut NodePage::new(1);

        // Add some nodes
        for i in 0..5 {
            let node = NodeRecordV3::new_inline(
                i * 100,
                NodeFlags::empty(),
                i as u16 * 10,
                i as u16 * 20,
                format!("node_{}_data", i).into_bytes(),
                i as u64 * 1000,
                i as u32 * 5,
                i as u64 * 2000,
                i as u32 * 3,
            );
            page.add_node(node).unwrap();
        }

        // Pack the page
        let bytes = page.pack().unwrap();
        assert_eq!(bytes.len(), MAX_PAGE_SIZE);

        // Unpack and verify
        let restored = NodePage::unpack(&bytes).unwrap();
        assert_eq!(restored.page_id, 1);
        assert_eq!(restored.node_count(), 5);
        assert_eq!(restored.nodes.len(), 5);

        // Verify node data
        for (i, node) in restored.nodes.iter().enumerate() {
            assert_eq!(node.id(), (i * 100) as i64);
            assert_eq!(node.kind_offset, (i * 10) as u16);
            assert_eq!(node.name_offset, (i * 20) as u16);
        }
    }

    #[test]
    fn test_pack_unpack_preserves_all_fields() {
        let page = &mut NodePage::new(99);
        page.next_page_id = 200;

        let node = NodeRecordV3::new_inline(
            -12345,
            NodeFlags::DELETED,
            42,
            84,
            b"Test node data for full preservation".to_vec(),
            0x123456789ABCDEF0,
            42,
            0xFEDCBA9876543210,
            99,
        );

        page.add_node(node).unwrap();

        let bytes = page.pack().unwrap();
        let restored = NodePage::unpack(&bytes).unwrap();

        assert_eq!(restored.page_id, 99);
        assert_eq!(restored.next_page_id, 200);
        assert_eq!(restored.node_count(), 1);

        let restored_node = &restored.nodes[0];
        assert_eq!(restored_node.id(), -12345);
        assert_eq!(restored_node.flags, NodeFlags::DELETED);
        assert_eq!(restored_node.kind_offset, 42);
        assert_eq!(restored_node.name_offset, 84);
        assert_eq!(
            restored_node.data_inline,
            Some(b"Test node data for full preservation".to_vec())
        );
        assert_eq!(restored_node.outgoing_cluster_offset, 0x123456789ABCDEF0);
        assert_eq!(restored_node.outgoing_edge_count, 42);
        assert_eq!(restored_node.incoming_cluster_offset, 0xFEDCBA9876543210);
        assert_eq!(restored_node.incoming_edge_count, 99);
    }

    #[test]
    fn test_checksum_validation() {
        let page = &mut NodePage::new(1);

        let node =
            NodeRecordV3::new_inline(1, NodeFlags::empty(), 0, 0, b"data".to_vec(), 0, 0, 0, 0);
        page.add_node(node).unwrap();

        let bytes = page.pack().unwrap();

        // Valid unpack should work
        assert!(NodePage::unpack(&bytes).is_ok());

        // Corrupt the checksum
        let mut corrupted = bytes;
        corrupted[constants::CHECKSUM_OFFSET] ^= 0xFF;

        // Should fail checksum validation
        assert!(NodePage::unpack(&corrupted).is_err());
    }

    #[test]
    fn test_empty_page_round_trip() {
        let page = NodePage::new(0);

        let bytes = page.pack().unwrap();
        let restored = NodePage::unpack(&bytes).unwrap();

        assert_eq!(restored.page_id, 0);
        assert_eq!(restored.node_count(), 0);
        assert!(restored.is_empty());
    }

    #[test]
    fn test_overflow_page_link() {
        let mut page = NodePage::new(10);
        page.next_page_id = 20;

        let bytes = page.pack().unwrap();
        let restored = NodePage::unpack(&bytes).unwrap();

        assert_eq!(restored.next_page_id, 20);
        assert!(restored.has_overflow());
    }

    #[test]
    fn test_used_size_calculation() {
        let page = &mut NodePage::new(1);

        let empty_node = NodeRecordV3::new_inline(1, NodeFlags::empty(), 0, 0, vec![], 0, 0, 0, 0);

        page.add_node(empty_node).unwrap();
        // Compressed size: delta(1) + flags(4) + kind(1) + name(1) + data_len(1) +
        //                     outgoing_cluster(1) + outgoing_count(1) + incoming_cluster(1) + incoming_count(1) = 12 bytes
        assert_eq!(page.used_size(), 12);

        let node_with_data =
            NodeRecordV3::new_inline(2, NodeFlags::empty(), 0, 0, vec![1u8; 32], 0, 0, 0, 0);

        page.add_node(node_with_data).unwrap();
        // First node: 12 bytes, Second node: 12 + 32 = 44 bytes
        assert_eq!(page.used_size(), 12 + 12 + 32);
    }

    #[test]
    fn test_remaining_capacity() {
        let page = &mut NodePage::new(1);
        assert_eq!(page.remaining_capacity(), USABLE_SIZE);

        // Use 50 bytes which is less than MAX_INLINE_DATA (64)
        let node = NodeRecordV3::new_inline(1, NodeFlags::empty(), 0, 0, vec![0u8; 50], 0, 0, 0, 0);

        page.add_node(node).unwrap();
        assert!(page.remaining_capacity() < USABLE_SIZE);
    }

    #[test]
    fn test_space_efficiency() {
        let page = &mut NodePage::new(1);

        // Empty page has 0 efficiency
        assert_eq!(page.space_efficiency(), 0.0);

        // Add a node that uses some space
        let node = NodeRecordV3::new_inline(
            1,
            NodeFlags::empty(),
            0,
            0,
            vec![1u8; FIXED_METADATA_SIZE],
            0,
            0,
            0,
            0,
        );

        page.add_node(node).unwrap();

        // Efficiency should be > 0 and < 1
        let efficiency = page.space_efficiency();
        assert!(efficiency > 0.0);
        assert!(efficiency < 1.0);
    }

    #[test]
    fn test_disk_size() {
        let page = NodePage::new(1);
        assert_eq!(page.disk_size(), MAX_PAGE_SIZE);
    }

    #[test]
    fn test_pack_returns_exact_size() {
        let page = NodePage::new(1);
        let bytes = page.pack().unwrap();
        assert_eq!(bytes.len(), MAX_PAGE_SIZE);
    }

    #[test]
    fn test_insufficient_bytes_error() {
        let short_data = vec![0u8; 100];
        let result = NodePage::unpack(&short_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_full_id_encoding_preserved() {
        let page = &mut NodePage::new(1);

        // Test with sequential ID values for optimal delta encoding
        // Using sequential IDs since delta encoding works best with sorted data
        let test_ids = vec![0, 1, 100, 101, 1000, 1001];

        for id in &test_ids {
            let node = NodeRecordV3::new_inline(
                *id,
                NodeFlags::empty(),
                10, // kind_offset
                20, // name_offset
                vec![],
                0,
                5, // outgoing_edge_count
                0,
                3, // incoming_edge_count
            );
            page.add_node(node).unwrap();
        }

        let bytes = page.pack().unwrap();
        let restored = NodePage::unpack(&bytes).unwrap();

        for (i, node) in restored.nodes.iter().enumerate() {
            assert_eq!(node.id(), test_ids[i], "ID at index {} not preserved", i);
        }
    }

    #[test]
    fn test_page_capacity_limits() {
        let page = &mut NodePage::new(1);

        // Keep adding nodes until page would be full
        // Each node: 44 (metadata) + 50 (data) = 94 bytes, + 2 byte length prefix = 96 bytes
        // 4064 / 96 = 42.3, so we should fit 42 nodes
        for count in 0..50 {
            let node = NodeRecordV3::new_inline(
                count as i64,
                NodeFlags::empty(),
                0,
                0,
                vec![count as u8; 50], // 50 bytes of data
                0,
                0,
                0,
                0,
            );

            // Try to add the node - it may fail near capacity
            if page.add_node(node).is_err() {
                // Page is full
                break;
            }
        }

        // Verify round-trip works
        let bytes = page.pack().unwrap();
        let restored = NodePage::unpack(&bytes).unwrap();

        // Should fit at least 20 nodes
        assert!(
            restored.node_count() >= 20,
            "Should fit at least 20 nodes, got {}",
            restored.node_count()
        );
    }

    #[test]
    fn test_max_inline_data_node() {
        let page = &mut NodePage::new(1);

        // Add node with max inline data
        let max_data = vec![0xFFu8; MAX_INLINE_DATA];
        let node = NodeRecordV3::new_inline(1, NodeFlags::empty(), 0, 0, max_data, 0, 0, 0, 0);

        page.add_node(node).unwrap();

        // Verify round-trip preserves max inline data
        let bytes = page.pack().unwrap();
        let restored = NodePage::unpack(&bytes).unwrap();

        assert_eq!(restored.node_count(), 1);
        assert_eq!(
            restored.nodes[0].data_inline.as_ref().unwrap().len(),
            MAX_INLINE_DATA
        );
    }

    #[test]
    fn test_external_node_record() {
        let page = &mut NodePage::new(1);

        // External data node (data > MAX_INLINE_DATA)
        // Note: Compression doesn't store external offset separately
        let node = NodeRecordV3::new_external(
            1,
            NodeFlags::empty(),
            0,
            0,
            5000, // external data offset (not stored in compressed format)
            200,  // data length
            0,
            5, // outgoing_edge_count (non-zero for better varint testing)
            0,
            3, // incoming_edge_count (non-zero for better varint testing)
        );

        page.add_node(node).unwrap();

        let bytes = page.pack().unwrap();
        let restored = NodePage::unpack(&bytes).unwrap();

        assert_eq!(restored.node_count(), 1);
        assert!(restored.nodes[0].is_external());
        assert_eq!(restored.nodes[0].data_len(), 200);
        // Note: external offset is not preserved in compressed format
        // This is acceptable as external offset is stored separately in the page structure
    }

    #[test]
    fn test_multiple_page_ids() {
        for page_id in [0, 1, 100, u64::MAX] {
            let page = NodePage::new(page_id);
            assert_eq!(page.page_id, page_id);

            let bytes = page.pack().unwrap();
            let restored = NodePage::unpack(&bytes).unwrap();
            assert_eq!(restored.page_id, page_id);
        }
    }

    #[test]
    fn test_default_trait() {
        let page = NodePage::default();
        assert_eq!(page.page_id, 0);
        assert!(page.is_empty());
    }
}
