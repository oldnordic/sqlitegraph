//! Node storage implementation with V2 as default
//!
//! This module handles node record storage in the native graph file format.
//! Phase 31: V2 is now the default and unconditional format.

use super::constants;
use super::graph_file::GraphFile;
use super::types::*;
use crate::backend::native::v2::node_record_v2::NodeRecordV2;
use std::collections::HashMap;

/// Node storage manager for native graph database files
pub struct NodeStore<'a> {
    graph_file: &'a mut GraphFile,
    node_index: HashMap<NativeNodeId, FileOffset>,
}

impl<'a> NodeStore<'a> {
    /// Create a new node store from a graph file
    pub fn new(graph_file: &'a mut GraphFile) -> Self {
        Self {
            graph_file,
            node_index: HashMap::new(),
        }
    }

    /// Allocate the next available node ID
    pub fn allocate_node_id(&mut self) -> NativeResult<NativeNodeId> {
        let current_count = self.graph_file.persistent_header().node_count;
        let next_id = (current_count + 1) as NativeNodeId;

        // PHASE 2A FIX: Prevent node region overflow corruption
        // Check if allocating this node would exceed reserved node region
        let header = self.graph_file.persistent_header();
        let node_slot_offset = header.node_data_offset
            + ((next_id - 1) as u64 * super::constants::node::NODE_SLOT_SIZE);
        let max_node_offset =
            header.node_data_offset + super::graph_file::RESERVED_NODE_REGION_BYTES;

        if node_slot_offset >= max_node_offset {
            return Err(NativeBackendError::CorruptFreeSpace {
                reason: format!(
                    "Node region overflow: node_id={} would exceed reserved region (offset={} >= max_offset={}). \
                    Increase RESERVED_NODE_REGION_BYTES or implement node relocation.",
                    next_id, node_slot_offset, max_node_offset
                ),
            });
        }

        Ok(next_id)
    }

    /// Write a node record to the file (V2-ONLY: direct write)
    pub fn write_node(&mut self, node: &NodeRecord) -> NativeResult<()> {
        // NodeRecord is now a type alias to NodeRecordV2, write directly
        self.write_node_v2(node)
    }

    /// Write a V2 node record to the file
    pub fn write_node_v2(&mut self, record: &NodeRecordV2) -> NativeResult<()> {
        if record.id <= 0 {
            return Err(NativeBackendError::InvalidNodeId {
                id: record.id,
                max_id: 0,
            });
        }
        record.validate()?;

        // Use V2 serialization layer
        let serialized = record.serialize();

        let node_data_offset = self.graph_file.persistent_header().node_data_offset;
        let slot_offset = node_data_offset + ((record.id - 1) as u64 * 4096);

        // Ensure V2 record is padded to fill entire 4096-byte slot
        let mut slot_buffer = vec![0u8; 4096];
        slot_buffer[..serialized.len()].copy_from_slice(&serialized);

        let required_size = slot_offset + 4096;
        let current_size = self.graph_file.file_size()?;
        if required_size > current_size {
            self.graph_file.grow(required_size - current_size)?;
        }

        // Phase 2C.2 FORENSIC: I/O path markers for write operation
        #[cfg(feature = "v2_experimental")]
        {
            println!(
                "[V2_SLOT_DEBUG] WRITE: node_id={}, slot_offset=0x{:x}, version={}, io_path=MMAP_WRITE, callsite={}:{}",
                record.id,
                slot_offset,
                slot_buffer[0],
                file!(),
                line!()
            );
            self.graph_file
                .mmap_write_bytes(slot_offset, &slot_buffer)?;
        }

        #[cfg(not(feature = "v2_experimental"))]
        {
            println!(
                "[V2_SLOT_DEBUG] WRITE: node_id={}, slot_offset=0x{:x}, version={}, io_path=FILE_WRITE_BYTES, callsite={}:{}",
                record.id,
                slot_offset,
                slot_buffer[0],
                file!(),
                line!()
            );

            self.graph_file.write_bytes(slot_offset, &slot_buffer)?;

            // SLOT CORRUPTION DEBUG: Verify write was successful
            if std::env::var("SLOT_CORRUPTION_DEBUG").is_ok() {
                let mut verify_buffer = [0u8; 1];
                if self
                    .graph_file
                    .read_bytes(slot_offset, &mut verify_buffer)
                    .is_ok()
                {
                    println!(
                        "[SLOT_CORRUPTION] POST_WRITE_VERIFY: node_id={}, slot_offset=0x{:x}, written_version={}, read_version={}",
                        record.id, slot_offset, slot_buffer[0], verify_buffer[0]
                    );
                }
            }
        }

        // Phase 76: Read-back verification after write
        #[cfg(all(feature = "v2_experimental", feature = "trace_v2_io"))]
        {
            let mut verify_buffer = vec![0u8; 32];
            if let Ok(_) = self
                .graph_file
                .mmap_read_bytes(slot_offset, &mut verify_buffer)
            {
                println!(
                    "[phase76] NODE_READBACK: node_id={}, slot_offset={}, verify_32={:02x?}",
                    record.id, slot_offset, verify_buffer
                );
            }
        }

        // PHASE 2C.1 FORENSIC: Dual-API instrumentation to detect cache/coherence issues
        if std::env::var("V2_SLOT_DEBUG").is_ok() {
            let mut before_buffer_file = vec![0u8; 32];
            let mut _before_buffer_mmap = vec![0u8; 32];
            let mut after_buffer_file = vec![0u8; 32];
            let mut _after_buffer_mmap = vec![0u8; 32];

            let file_size_before = self.graph_file.file_size().unwrap_or(0);

            // Read bytes BEFORE write using BOTH APIs
            if slot_offset + 32 <= file_size_before {
                let _ = self
                    .graph_file
                    .read_bytes(slot_offset, &mut before_buffer_file);
                #[cfg(feature = "v2_experimental")]
                {
                    let _ = self
                        .graph_file
                        .mmap_read_bytes(slot_offset, &mut before_buffer_mmap);
                }
            }

            // Read bytes AFTER write using BOTH APIs
            let _ = self
                .graph_file
                .read_bytes(slot_offset, &mut after_buffer_file);
            #[cfg(feature = "v2_experimental")]
            {
                let _ = self
                    .graph_file
                    .mmap_read_bytes(slot_offset, &mut after_buffer_mmap);
            }

            println!(
                "[V2_SLOT_DEBUG] WRITE_AFTER: node_id={}, slot_offset=0x{:x}, file_size={}, callsite={}:{}",
                record.id,
                slot_offset,
                file_size_before,
                file!(),
                line!()
            );
            println!(
                "[V2_SLOT_DEBUG] WRITE_BEFORE_FILE:  version={}, bytes={:02x?}",
                before_buffer_file.get(0).unwrap_or(&0),
                &before_buffer_file[..before_buffer_file.len().min(32)]
            );
            #[cfg(feature = "v2_experimental")]
            println!(
                "[V2_SLOT_DEBUG] WRITE_BEFORE_MMAP:  version={}, bytes={:02x?}",
                before_buffer_mmap.get(0).unwrap_or(&0),
                &before_buffer_mmap[..before_buffer_mmap.len().min(32)]
            );
            println!(
                "[V2_SLOT_DEBUG] WRITE_AFTER_FILE:   version={}, bytes={:02x?}",
                after_buffer_file.get(0).unwrap_or(&0),
                &after_buffer_file[..after_buffer_file.len().min(32)]
            );
            #[cfg(feature = "v2_experimental")]
            println!(
                "[V2_SLOT_DEBUG] WRITE_AFTER_MMAP:   version={}, bytes={:02x?}",
                after_buffer_mmap.get(0).unwrap_or(&0),
                &after_buffer_mmap[..after_buffer_mmap.len().min(32)]
            );
        }

        self.node_index.insert(record.id, slot_offset);

        if record.id as u64 > self.graph_file.persistent_header().node_count {
            self.graph_file.persistent_header_mut().node_count = record.id as u64;
            self.graph_file.write_header()?;
        }

        // Ensure all node data is flushed to disk before returning
        self.graph_file.flush()?;

        Ok(())
    }

    /// Read a node record from the file by ID (V2-only)
    pub fn read_node(&mut self, node_id: NativeNodeId) -> NativeResult<NodeRecord> {
        // V2-only: Always use V2 node reading (V1 format detection removed)
        self.read_node_v2(node_id)
    }

    /// Read a V2 node record from the file by ID
    pub fn read_node_v2(&mut self, node_id: NativeNodeId) -> NativeResult<NodeRecordV2> {
        let header = self.graph_file.header();
        if node_id <= 0 || node_id > header.node_count as NativeNodeId {
            return Err(NativeBackendError::InvalidNodeId {
                id: node_id,
                max_id: header.node_count as NativeNodeId,
            });
        }

        let node_data_offset = self.graph_file.persistent_header().node_data_offset;
        let slot_offset = node_data_offset + ((node_id - 1) as u64 * 4096);
        let file_size = self.graph_file.file_size()?;
        let remaining = file_size.checked_sub(slot_offset).ok_or_else(|| {
            NativeBackendError::CorruptNodeRecord {
                node_id,
                reason: format!("Slot offset {} beyond file size {}", slot_offset, file_size),
            }
        })?;

        // Read minimum required for V2 header (21 bytes for header parsing)
        if remaining < 21 {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id,
                reason: format!(
                    "Insufficient bytes ({}) for V2 header at offset {}",
                    remaining, slot_offset
                ),
            });
        }

        // PHASE 2C.1 FORENSIC: Dual-API instrumentation for reader
        if std::env::var("V2_SLOT_DEBUG").is_ok() {
            let mut debug_buffer_file = vec![0u8; 32];
            let mut _debug_buffer_mmap = vec![0u8; 32];
            let file_size = self.graph_file.file_size().unwrap_or(0);

            if slot_offset + 32 <= file_size {
                // Read using BOTH APIs
                let _ = self
                    .graph_file
                    .read_bytes(slot_offset, &mut debug_buffer_file);
                #[cfg(feature = "v2_experimental")]
                {
                    let _ = self
                        .graph_file
                        .mmap_read_bytes(slot_offset, &mut debug_buffer_mmap);
                }

                println!(
                    "[V2_SLOT_DEBUG] READ_ENTRY: node_id={}, slot_offset=0x{:x}, file_size={}, callsite={}:{}",
                    node_id,
                    slot_offset,
                    file_size,
                    file!(),
                    line!()
                );
                println!(
                    "[V2_SLOT_DEBUG] READ_PRE_PARSE_FILE: version={}, bytes={:02x?}",
                    debug_buffer_file.get(0).unwrap_or(&0),
                    &debug_buffer_file[..debug_buffer_file.len().min(32)]
                );
                #[cfg(feature = "v2_experimental")]
                println!(
                    "[V2_SLOT_DEBUG] READ_PRE_PARSE_MMAP: version={}, bytes={:02x?}",
                    debug_buffer_mmap.get(0).unwrap_or(&0),
                    &debug_buffer_mmap[..debug_buffer_mmap.len().min(32)]
                );
            } else {
                println!(
                    "[V2_SLOT_DEBUG] READ_ENTRY: node_id={}, slot_offset=0x{:x}, file_size={} - SLOT BEYOND FILE",
                    node_id, slot_offset, file_size
                );
            }
        }

        // Phase 2C.2 FORENSIC: I/O path markers for header read operation
        let mut header_buffer = vec![0u8; 21];
        #[cfg(feature = "v2_experimental")]
        {
            self.graph_file
                .mmap_read_bytes(slot_offset, &mut header_buffer)?;
            println!(
                "[V2_SLOT_DEBUG] READ_PRE_PARSE: node_id={}, slot_offset=0x{:x}, version={}, io_path=MMAP_READ_BYTES, callsite={}:{}",
                node_id,
                slot_offset,
                header_buffer[0],
                file!(),
                line!()
            );
        }

        #[cfg(not(feature = "v2_experimental"))]
        {
            self.graph_file
                .read_bytes(slot_offset, &mut header_buffer)?;
            println!(
                "[V2_SLOT_DEBUG] READ_PRE_PARSE: node_id={}, slot_offset=0x{:x}, version={}, io_path=FILE_READ_BYTES, callsite={}:{}",
                node_id,
                slot_offset,
                header_buffer[0],
                file!(),
                line!()
            );
        }

        // Parse V2 header to get exact record size
        let (kind_len, name_len, data_len) =
            crate::backend::native::v2::node_record_v2::parse_v2_header_lengths(&header_buffer)?;
        let actual_record_size =
            21 + kind_len as usize + name_len as usize + data_len as usize + 32; // 32 for cluster metadata

        // Verify we have enough bytes for the actual record
        if remaining < actual_record_size as u64 {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id,
                reason: format!(
                    "V2 record truncated: need {} bytes, have {} at offset {}",
                    actual_record_size, remaining, slot_offset
                ),
            });
        }

        // Read the exact V2 record size (not the entire slot)
        let mut buffer = vec![0u8; actual_record_size];

        // Phase 76: Byte-proof instrumentation before read
        #[cfg(all(feature = "v2_experimental", feature = "trace_v2_io"))]
        {
            println!(
                "[phase76] NODE_READ_START: node_id={}, slot_offset={}, len={}",
                node_id, slot_offset, actual_record_size
            );
        }

        // Phase 41: Route node reads based on exclusive I/O mode
        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_mmap"))]
        {
            self.graph_file.mmap_read_bytes(slot_offset, &mut buffer)?;
        }
        #[cfg(all(feature = "v2_experimental", feature = "v2_io_exclusive_std"))]
        {
            // EXCLUSIVE STD MODE: Use standard I/O for V2 node reads
            self.graph_file.read_bytes(slot_offset, &mut buffer)?;
        }
        #[cfg(not(any(
            feature = "v2_experimental",
            feature = "v2_io_exclusive_mmap",
            feature = "v2_io_exclusive_std"
        )))]
        {
            // DEFAULT MODE: Use canonical read_bytes API for V2
            self.graph_file.read_bytes(slot_offset, &mut buffer)?;
        }

        // Phase 76: Read result verification
        #[cfg(all(feature = "v2_experimental", feature = "trace_v2_io"))]
        {
            let first_32 = if buffer.len() >= 32 {
                &buffer[..32]
            } else {
                &buffer
            };
            println!(
                "[phase76] NODE_READ_RESULT: node_id={}, slot_offset={}, read_32={:02x?}",
                node_id, slot_offset, first_32
            );
        }

        let record = NodeRecordV2::deserialize(&buffer)?;
        Ok(record)
    }

    /// Delete a node record by ID (simple stub - doesn't handle edge cleanup)
    pub fn delete_node(&mut self, node_id: NativeNodeId) -> NativeResult<()> {
        // For now, just remove from index
        self.node_index.remove(&node_id);

        // TODO: Implement proper deletion with edge cleanup and space reclamation
        Ok(())
    }

    /// Get the maximum node ID in the file
    pub fn max_node_id(&self) -> NativeNodeId {
        self.graph_file.persistent_header().node_count as NativeNodeId
    }

    /// Rebuild V2 index (experimental feature)
    #[cfg(feature = "v2_experimental")]
    pub fn rebuild_v2_index(&mut self) -> NativeResult<()> {
        // Implementation stub for V2 index rebuilding
        Ok(())
    }

    /// Validate node record fields before writing (excluding ID range)
    fn validate_node_fields(&self, node: &NodeRecord) -> NativeResult<()> {
        // Validate kind string length
        if node.kind.len() > constants::node::MAX_STRING_LENGTH as usize {
            return Err(NativeBackendError::RecordTooLarge {
                size: node.kind.len() as u32,
                max_size: constants::node::MAX_STRING_LENGTH as u32,
            });
        }

        // Validate name string length
        if node.name.len() > constants::node::MAX_STRING_LENGTH as usize {
            return Err(NativeBackendError::RecordTooLarge {
                size: node.name.len() as u32,
                max_size: constants::node::MAX_STRING_LENGTH as u32,
            });
        }

        Ok(())
    }
}

/// Clear the node cache (no-op since we removed caching)
pub fn clear_node_cache() {
    // No cache to clear
}
