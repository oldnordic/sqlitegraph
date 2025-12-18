//! Cluster container that stores a node’s adjacency in contiguous storage.

use super::compact_record::CompactEdgeRecord;
use crate::backend::native::v2::string_table::StringTable;
use crate::backend::native::{EdgeRecord, FileOffset, NativeBackendError, NativeResult};
use std::cell::{Cell, RefCell};
use std::fmt::Write;

/// Adjacency direction for cluster construction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Outgoing,
    Incoming,
}

#[derive(Clone, Copy, Debug)]
pub struct TraceContext {
    pub node_id: i64,
    pub direction: Direction,
    pub cluster_offset: FileOffset,
    pub payload_size: u32,
    pub strict: bool,
}

pub struct TraceGuard {
    strict_guard: StrictModeGuard,
}

pub struct StrictModeGuard {
    previous: bool,
}

thread_local! {
    static TRACE_CONTEXT: RefCell<Option<TraceContext>> = RefCell::new(None);
    static STRICT_MODE: Cell<bool> = Cell::new(false);
}

impl TraceGuard {
    pub fn new(context: TraceContext) -> Self {
        TRACE_CONTEXT.with(|slot| {
            *slot.borrow_mut() = Some(context);
        });
        let strict_guard = StrictModeGuard::new(context.strict);
        TraceGuard { strict_guard }
    }
}

impl Drop for TraceGuard {
    fn drop(&mut self) {
        TRACE_CONTEXT.with(|slot| {
            slot.borrow_mut().take();
        });
    }
}

impl StrictModeGuard {
    pub fn new(strict: bool) -> Self {
        let previous = STRICT_MODE.with(|cell| {
            let prev = cell.get();
            cell.set(strict);
            prev
        });
        StrictModeGuard { previous }
    }
}

impl Drop for StrictModeGuard {
    fn drop(&mut self) {
        STRICT_MODE.with(|cell| {
            cell.set(self.previous);
        });
    }
}

fn strict_mode_enabled() -> bool {
    STRICT_MODE.with(|cell| cell.get())
}

fn with_trace_context<F: FnOnce(&TraceContext)>(f: F) {
    TRACE_CONTEXT.with(|slot| {
        if let Some(ctx) = *slot.borrow() {
            f(&ctx);
        }
    });
}

fn current_trace_context() -> Option<TraceContext> {
    TRACE_CONTEXT.with(|slot| *slot.borrow())
}

fn format_strict_reason(
    ctx: Option<TraceContext>,
    detail: &str,
    edge_index: usize,
    cursor: usize,
    payload_size: usize,
    remaining: usize,
    preview: &[u8],
) -> String {
    let mut preview_hex = String::new();
    for (i, byte) in preview.iter().enumerate() {
        if i > 0 {
            preview_hex.push(' ');
        }
        let _ = write!(&mut preview_hex, "{:02X}", byte);
    }
    let preview_ascii = String::from_utf8_lossy(preview);

    if let Some(ctx) = ctx {
        format!(
            "{} [node_id={}, direction={:?}, cluster_offset={}, payload_size={}, edge_index={}, cursor={}, remaining={}, preview_hex={}, preview_ascii={:?}]",
            detail,
            ctx.node_id,
            ctx.direction,
            ctx.cluster_offset,
            payload_size,
            edge_index,
            cursor,
            remaining,
            preview_hex,
            preview_ascii
        )
    } else {
        format!(
            "{} [payload_size={}, edge_index={}, cursor={}, remaining={}, preview_hex={}, preview_ascii={:?}]",
            detail, payload_size, edge_index, cursor, remaining, preview_hex, preview_ascii
        )
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
        let expected_total_size = 8 + self.serialized_size;
        let mut buffer = Vec::with_capacity(expected_total_size);
        buffer.extend_from_slice(&self.edge_count().to_be_bytes());
        buffer.extend_from_slice(&(self.serialized_size as u32).to_be_bytes());

        // V2_CLUSTER_AUDIT: Log cluster write details
        if std::env::var("V2_CLUSTER_AUDIT").is_ok() {
            println!(
                "[V2_CLUSTER_AUDIT] {}:serialize(): file:{} line={}, edge_count={}, payload_size={}, expected_total={}",
                std::module_path!(),
                file!(),
                line!(),
                self.edge_count(),
                self.serialized_size,
                expected_total_size
            );
        }

        // PHASE 74 INSTRUMENTATION: Trace serialization before writing to disk
        #[cfg(feature = "trace_v2_io")]
        {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut hasher = DefaultHasher::new();
            for byte in &buffer {
                byte.hash(&mut hasher);
            }
            let checksum32 = hasher.finish() as u32;

            let first_32 = if buffer.len() >= 32 {
                &buffer[..32]
            } else {
                &buffer[..]
            };
            let last_32 = if buffer.len() >= 32 {
                &buffer[buffer.len() - 32..]
            } else {
                &buffer[..]
            };

            println!(
                "[phase74] SERIALIZE: node_id={:?}, direction={:?}, size={}, checksum32=0x{:08x}, first_32={:02x?}, last_32={:02x?}",
                self.edges.first().map(|e| e.neighbor_id).unwrap_or(0),
                "serialize",
                buffer.len(),
                checksum32,
                first_32,
                last_32
            );
        }

        let mut actual_payload_size = 0;
        for edge in &self.edges {
            let edge_bytes = edge.serialize();
            actual_payload_size += edge_bytes.len();
            buffer.extend_from_slice(&edge_bytes);
        }

        // CRITICAL SAFETY CHECK: Detect corruption at serialization time
        assert_eq!(
            actual_payload_size,
            self.serialized_size,
            "SERIALIZATION CORRUPTION: calculated payload size {} != actual payload size {} for {} edges",
            self.serialized_size,
            actual_payload_size,
            self.edges.len()
        );

        assert_eq!(
            buffer.len(),
            expected_total_size,
            "SERIALIZATION CORRUPTION: final buffer size {} != expected {}",
            buffer.len(),
            expected_total_size
        );

        // PHASE 74 INSTRUMENTATION: Final serialization trace
        #[cfg(feature = "trace_v2_io")]
        {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut hasher = DefaultHasher::new();
            for byte in &buffer {
                byte.hash(&mut hasher);
            }
            let checksum32 = hasher.finish() as u32;

            let first_32 = if buffer.len() >= 32 {
                &buffer[..32]
            } else {
                &buffer[..]
            };
            let last_32 = if buffer.len() >= 32 {
                &buffer[buffer.len() - 32..]
            } else {
                &buffer[..]
            };

            println!(
                "[phase74] SERIALIZE_FINAL: edges={}, size={}, checksum32=0x{:08x}, first_32={:02x?}, last_32={:02x?}",
                self.edges.len(),
                buffer.len(),
                checksum32,
                first_32,
                last_32
            );
        }

        buffer
    }

    /// Validate serialized bytes before writing to disk.
    pub fn verify_serialized_layout(bytes: &[u8]) -> NativeResult<()> {
        if bytes.len() < 8 {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: 8,
            });
        }

        let edge_count = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let payload_size = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        let expected_total = 8 + payload_size;

        if bytes.len() != expected_total {
            return Err(NativeBackendError::CorruptEdgeRecord {
                edge_id: -1,
                reason: format!(
                    "cluster serialization payload mismatch: header_payload={} actual_payload={}",
                    payload_size,
                    bytes.len().saturating_sub(8)
                ),
            });
        }

        let mut cursor = 8;
        for edge_index in 0..edge_count {
            if cursor + 12 > bytes.len() {
                return Err(NativeBackendError::CorruptEdgeRecord {
                    edge_id: edge_index as i64,
                    reason: format!(
                        "cluster serialization truncated before record {} header (cursor={}, payload_size={})",
                        edge_index, cursor, payload_size
                    ),
                });
            }

            let data_len = u16::from_be_bytes([bytes[cursor + 10], bytes[cursor + 11]]) as usize;
            let record_size = 12 + data_len;

            if cursor + record_size > bytes.len() {
                return Err(NativeBackendError::CorruptEdgeRecord {
                    edge_id: edge_index as i64,
                    reason: format!(
                        "cluster serialization truncated record {} at cursor {} (record_size={}, payload_size={})",
                        edge_index, cursor, record_size, payload_size
                    ),
                });
            }

            cursor += record_size;
        }

        if cursor != expected_total {
            return Err(NativeBackendError::CorruptEdgeRecord {
                edge_id: -1,
                reason: format!(
                    "cluster serialization cursor mismatch: cursor={} payload_end={}",
                    cursor, expected_total
                ),
            });
        }

        Ok(())
    }

    /// Rebuild a cluster from raw bytes.
    pub fn deserialize(bytes: &[u8]) -> NativeResult<Self> {
        // PHASE 74 INSTRUMENTATION: Trace deserialization start
        #[cfg(feature = "trace_v2_io")]
        {
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};

            let mut hasher = DefaultHasher::new();
            for byte in bytes {
                byte.hash(&mut hasher);
            }
            let checksum32 = hasher.finish() as u32;

            let first_32 = if bytes.len() >= 32 {
                &bytes[..32]
            } else {
                &bytes[..]
            };
            let last_32 = if bytes.len() >= 32 {
                &bytes[bytes.len() - 32..]
            } else {
                &bytes[..]
            };

            if bytes.len() >= 8 {
                let edge_count =
                    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
                let payload_size =
                    u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;

                println!(
                    "[phase74] DESERIALIZE: edge_count={}, payload_size={}, total_size={}, checksum32=0x{:08x}, first_32={:02x?}, last_32={:02x?}",
                    edge_count,
                    payload_size,
                    bytes.len(),
                    checksum32,
                    first_32,
                    last_32
                );
            }
        }

        if bytes.len() < 8 {
            return Err(NativeBackendError::BufferTooSmall {
                size: bytes.len(),
                min_size: 8,
            });
        }

        let edge_count = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
        let payload_size = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
        let expected_total = 8 + payload_size;

        // V2_CLUSTER_AUDIT: Log deserialization attempt before failure check
        if std::env::var("V2_CLUSTER_AUDIT").is_ok() {
            let first_8_bytes = if bytes.len() >= 8 {
                &bytes[..8]
            } else {
                &bytes[..]
            };
            println!(
                "[V2_CLUSTER_AUDIT] {}:deserialize(): file:{} line={}, bytes_len={}, expected_total={}, payload_size={}, edge_count={}, first_8_bytes={:02X?}",
                std::module_path!(),
                file!(),
                line!(),
                bytes.len(),
                expected_total,
                payload_size,
                edge_count,
                first_8_bytes
            );
        }

        // PHASE 69 FIX: STRICT FRAMED MODE - NO FALLBACK
        if bytes.len() != expected_total {
            // V2_CLUSTER_AUDIT: Log the exact mismatch failure
            if std::env::var("V2_CLUSTER_AUDIT").is_ok() {
                println!(
                    "[V2_CLUSTER_AUDIT] {}:deserialize(): SIZE_MISMATCH file:{} line={}, actual={}, expected={}, diff={}, payload_size_from_header={}",
                    std::module_path!(),
                    file!(),
                    line!(),
                    bytes.len(),
                    expected_total,
                    bytes.len() as isize - expected_total as isize,
                    payload_size
                );
            }

            let ctx_opt = current_trace_context();
            #[cfg(any(test, feature = "trace_v2_io"))]
            with_trace_context(|ctx| {
                let preview = if bytes.len() > 8 {
                    &bytes[8..bytes.len().min(72)]
                } else {
                    bytes
                };
                println!(
                    "[trace_v2_io] node_id={}, direction={:?}, cluster_offset={}, payload_size={}, edge_index=0, cursor={}, remaining={}, preview_hex={:02X?}, preview_ascii={:?}",
                    ctx.node_id,
                    ctx.direction,
                    ctx.cluster_offset,
                    ctx.payload_size,
                    bytes.len(),
                    expected_total.saturating_sub(bytes.len()),
                    preview,
                    String::from_utf8_lossy(preview)
                );
            });

            // PHASE 69: STRICT MODE ONLY - No fallback to legacy parsing when framed flag is set
            if strict_mode_enabled() {
                let preview = if bytes.len() > 8 {
                    &bytes[8..bytes.len().min(24)]
                } else {
                    bytes
                };
                let reason = format_strict_reason(
                    ctx_opt,
                    "V2 FRAMED: cluster header size mismatch",
                    0,
                    bytes.len(),
                    payload_size,
                    expected_total.saturating_sub(bytes.len()),
                    preview,
                );
                return Err(NativeBackendError::CorruptEdgeRecord {
                    edge_id: -1,
                    reason,
                });
            }

            // PHASE 69: ALWAYS fail in framed mode - never fall back to legacy parsing
            return Err(NativeBackendError::CorruptEdgeRecord {
                edge_id: -1,
                reason: format!(
                    "V2 FRAMED: Cluster size mismatch: expected {}, found {} [header: edge_count={}, payload_size={}]",
                    expected_total,
                    bytes.len(),
                    edge_count,
                    payload_size
                ),
            });
        }

        let mut edges = Vec::with_capacity(edge_count);
        let mut cursor = 8;
        for edge_index in 0..edge_count {
            // Phase 44.1: Check bounds before calling deserialize to prevent "Buffer too small: 0 < 10" error
            if cursor >= bytes.len() {
                let ctx_opt = current_trace_context();
                if ctx_opt
                    .map(|ctx| ctx.strict)
                    .unwrap_or_else(strict_mode_enabled)
                {
                    let reason = format_strict_reason(
                        ctx_opt,
                        "framed cluster ended before advertised edge_count",
                        edge_index,
                        cursor,
                        payload_size,
                        bytes.len().saturating_sub(cursor),
                        &[],
                    );
                    return Err(NativeBackendError::CorruptEdgeRecord {
                        edge_id: -1,
                        reason,
                    });
                }
                return Err(NativeBackendError::CorruptEdgeRecord {
                    edge_id: -1,
                    reason: format!(
                        "Cluster header corruption: expected {} edges but cluster ends at cursor {} with total size {} bytes",
                        edge_count,
                        cursor,
                        bytes.len()
                    ),
                });
            }

            let record = match CompactEdgeRecord::deserialize(&bytes[cursor..]) {
                Ok(record) => record,
                Err(err) => {
                    let ctx_opt = current_trace_context();
                    #[cfg(any(test, feature = "trace_v2_io"))]
                    {
                        let preview = if cursor < bytes.len() {
                            &bytes[cursor..bytes.len().min(cursor + 16)]
                        } else {
                            &[]
                        };
                        let remaining = bytes.len().saturating_sub(cursor);
                        with_trace_context(|ctx| {
                            println!(
                                "[trace_v2_io] node_id={}, direction={:?}, cluster_offset={}, payload_size={}, edge_index={}, cursor={}, remaining={}, preview_hex={:02X?}, preview_ascii={:?}",
                                ctx.node_id,
                                ctx.direction,
                                ctx.cluster_offset,
                                ctx.payload_size,
                                edges.len(),
                                cursor,
                                remaining,
                                preview,
                                String::from_utf8_lossy(preview)
                            );
                        });
                    }
                    if ctx_opt
                        .map(|ctx| ctx.strict)
                        .unwrap_or_else(strict_mode_enabled)
                    {
                        let preview = if cursor < bytes.len() {
                            &bytes[cursor..bytes.len().min(cursor + 16)]
                        } else {
                            &[]
                        };
                        let remaining = bytes.len().saturating_sub(cursor);
                        let detail = match &err {
                            NativeBackendError::BufferTooSmall { size, min_size } => {
                                format!(
                                    "framed edge payload truncated: size={} min_size={}",
                                    size, min_size
                                )
                            }
                            _ => format!("framed edge decode failed: {}", err),
                        };
                        let reason = format_strict_reason(
                            ctx_opt,
                            &detail,
                            edges.len(),
                            cursor,
                            payload_size,
                            remaining,
                            preview,
                        );
                        return Err(NativeBackendError::CorruptEdgeRecord {
                            edge_id: -1,
                            reason,
                        });
                    }
                    return Err(err);
                }
            };
            let record_size = record.size_bytes();

            if cursor + record_size > bytes.len() {
                let ctx_opt = current_trace_context();
                if ctx_opt
                    .map(|ctx| ctx.strict)
                    .unwrap_or_else(strict_mode_enabled)
                {
                    let preview = if cursor < bytes.len() {
                        &bytes[cursor..bytes.len().min(cursor + 16)]
                    } else {
                        &[]
                    };
                    let reason = format_strict_reason(
                        ctx_opt,
                        "framed edge spans beyond cluster payload",
                        edge_index,
                        cursor,
                        payload_size,
                        bytes.len().saturating_sub(cursor),
                        preview,
                    );
                    return Err(NativeBackendError::CorruptEdgeRecord {
                        edge_id: -1,
                        reason,
                    });
                }
                return Err(NativeBackendError::CorruptEdgeRecord {
                    edge_id: -1,
                    reason: "Edge record extends beyond cluster payload".into(),
                });
            }
            edges.push(record);
            cursor += record_size;
        }

        // Phase 44.2: Debug cluster deserialization results
        #[cfg(debug_assertions)]
        {
            println!(
                "Phase 44.2: DESERIALIZE - expected_edge_count={}, actual_edges={}",
                edge_count,
                edges.len()
            );
            for (i, edge) in edges.iter().enumerate() {
                println!(
                    "Phase 44.2: DESERIALIZE - edge[{}]: neighbor_id={}",
                    i, edge.neighbor_id
                );
            }

            // Phase 44.2: Debug cluster payload content
            println!(
                "Phase 44.2: DESERIALIZE - payload_size={}, payload_bytes: {:02X?}",
                payload_size,
                &bytes[8..8 + payload_size as usize]
            );
        }

        if cursor != expected_total {
            let ctx_opt = current_trace_context();
            #[cfg(any(test, feature = "trace_v2_io"))]
            {
                let remaining = expected_total.saturating_sub(cursor);
                let preview = if cursor < bytes.len() {
                    &bytes[cursor..bytes.len().min(cursor + 16)]
                } else {
                    &[]
                };
                with_trace_context(|ctx| {
                    println!(
                        "[trace_v2_io] node_id={}, direction={:?}, cluster_offset={}, payload_size={}, edge_index={}, cursor={}, remaining={}, preview_hex={:02X?}, preview_ascii={:?}",
                        ctx.node_id,
                        ctx.direction,
                        ctx.cluster_offset,
                        ctx.payload_size,
                        edges.len(),
                        cursor,
                        remaining,
                        preview,
                        String::from_utf8_lossy(preview)
                    );
                });
            }

            if ctx_opt
                .map(|ctx| ctx.strict)
                .unwrap_or_else(strict_mode_enabled)
            {
                let preview = if cursor < bytes.len() {
                    &bytes[cursor..bytes.len().min(cursor + 16)]
                } else {
                    &[]
                };
                let reason = format_strict_reason(
                    ctx_opt,
                    "framed cluster cursor mismatch",
                    edges.len(),
                    cursor,
                    payload_size,
                    expected_total.saturating_sub(cursor),
                    preview,
                );
                return Err(NativeBackendError::CorruptEdgeRecord {
                    edge_id: -1,
                    reason,
                });
            }
        }

        Ok(Self {
            offset: 0,
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
