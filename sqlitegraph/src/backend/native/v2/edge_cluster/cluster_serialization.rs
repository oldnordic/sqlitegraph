//! Serialization and deserialization operations for edge clusters.
//!
//! This module handles the binary format operations for edge clusters,
//! including serialization, deserialization, and layout validation.

use super::compact_record::CompactEdgeRecord;
use crate::backend::native::{NativeBackendError, NativeResult};

/// Serialize cluster header + payload.
/// CRITICAL FIX: Ensure the final buffer size matches header expectations exactly.
pub fn serialize_cluster(
    edges: &[CompactEdgeRecord],
    serialized_size: usize,
) -> NativeResult<Vec<u8>> {
    let expected_total_size = 8 + serialized_size;
    let mut buffer = Vec::with_capacity(expected_total_size);
    buffer.extend_from_slice(&(edges.len() as u32).to_be_bytes());
    buffer.extend_from_slice(&(serialized_size as u32).to_be_bytes());

    // V2_CLUSTER_AUDIT: Log cluster write details
    if std::env::var("V2_CLUSTER_AUDIT").is_ok() {
        println!(
            "[V2_CLUSTER_AUDIT] {}:serialize(): file:{} line={}, edge_count={}, payload_size={}, expected_total={}",
            std::module_path!(),
            file!(),
            line!(),
            edges.len(),
            serialized_size,
            expected_total_size
        );
    }

    // HOT PATH FIX: Only serialize edge data if it's non-empty/null
    if !edges.is_empty() {
        let mut cursor = 8;
        for (edge_index, edge) in edges.iter().enumerate() {
            if std::env::var("V2_CLUSTER_AUDIT").is_ok() {
                println!(
                    "[V2_CLUSTER_AUDIT] {}:serialize(): file:{} line={}, edge_index={}, edge_size={}, cursor={}",
                    std::module_path!(),
                    file!(),
                    line!(),
                    edge_index,
                    edge.size_bytes(),
                    cursor
                );
            }

            let edge_bytes = edge.serialize();
            cursor += edge_bytes.len();
            buffer.extend_from_slice(&edge_bytes);
        }

        if cursor != 8 + serialized_size {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "serialize(): cursor mismatch: {}, expected {}",
                    cursor,
                    8 + serialized_size
                ),
            });
        }
    }

    // CRITICAL FIX: Ensure the final buffer size matches header expectations exactly
    if buffer.len() != expected_total_size {
        return Err(NativeBackendError::CorruptNodeRecord {
            node_id: -1,
            reason: format!(
                "serialize(): final buffer size mismatch: actual {}, expected {}",
                buffer.len(),
                expected_total_size
            ),
        });
    }

    Ok(buffer)
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
        return Err(NativeBackendError::CorruptNodeRecord {
            node_id: -1,
            reason: format!(
                "verify_serialized_layout(): size mismatch: expected {}, actual {}",
                expected_total,
                bytes.len()
            ),
        });
    }

    // Phase 74.1: Verify edge payload integrity
    let mut cursor = 8;
    for edge_index in 0..edge_count {
        // Check that we have enough bytes for the edge header (8 bytes)
        if cursor + 8 > bytes.len() {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "verify_serialized_layout(): truncated edge header at edge {}, cursor {}",
                    edge_index, cursor
                ),
            });
        }

        // Read edge header (neighbor_id + type_offset + data_len)
        let neighbor_id_bytes = &bytes[cursor..cursor + 8];
        let _neighbor_id = i64::from_be_bytes(neighbor_id_bytes.try_into().unwrap());
        cursor += 8;

        // Skip type_offset (2 bytes)
        cursor += 2;
        // Read data_len (2 bytes)
        let data_len_bytes = &bytes[cursor..cursor + 2];
        let data_len = u16::from_be_bytes(data_len_bytes.try_into().unwrap()) as usize;
        cursor += 2;

        // Validate data_len and skip edge data
        if data_len > 10000 {
            // Sanity check: edge data shouldn't be extremely large
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "verify_serialized_layout(): edge data too large: {} bytes at edge {}",
                    data_len, edge_index
                ),
            });
        }

        cursor += data_len;

        // Check that we haven't overrun the buffer
        if cursor > bytes.len() {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "verify_serialized_layout(): buffer overrun at edge {}, cursor {}",
                    edge_index, cursor
                ),
            });
        }
    }

    // Final check: cursor should exactly match the end of payload
    if cursor != expected_total {
        return Err(NativeBackendError::CorruptNodeRecord {
            node_id: -1,
            reason: format!(
                "verify_serialized_layout(): cursor mismatch: expected {}, actual {}",
                expected_total, cursor
            ),
        });
    }

    Ok(())
}

/// Rebuild a cluster from raw bytes.
pub fn deserialize_cluster(bytes: &[u8]) -> NativeResult<(Vec<CompactEdgeRecord>, usize)> {
    // PHASE 74 INSTRUMENTATION: Trace deserialization start
    #[cfg(feature = "trace_v2_io")]
    {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        for byte in bytes {
            byte.hash(&mut hasher);
        }
        let hash_val = hasher.finish();
        println!(
            "[V2_CLUSTER_AUDIT] {}:deserialize(): file:{} line={}, bytes_len={}, hash={:x}",
            std::module_path!(),
            file!(),
            line!(),
            bytes.len(),
            hash_val
        );
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

    if bytes.len() != expected_total {
        return Err(NativeBackendError::CorruptNodeRecord {
            node_id: -1,
            reason: format!(
                "deserialize(): SIZE_MISMATCH file={} line={} actual={}, expected={}, diff={}, payload_size_from_header={}",
                file!(),
                line!(),
                bytes.len(),
                expected_total,
                bytes.len() as isize - expected_total as isize,
                payload_size
            ),
        });
    }

    let mut edges = Vec::with_capacity(edge_count);
    let mut cursor = 8;

    for edge_index in 0..edge_count {
        // Phase 44.1: Check bounds before calling deserialize to prevent "Buffer too small: 0 < 10" error
        if cursor > bytes.len() {
            let remaining = bytes.len() - cursor;
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "deserialize(): edge_index={}, cursor={}, remaining={}",
                    edge_index, cursor, remaining
                ),
            });
        }

        let record = match CompactEdgeRecord::deserialize(&bytes[cursor..]) {
            Ok(rec) => rec,
            Err(e) => {
                return Err(NativeBackendError::CorruptNodeRecord {
                    node_id: -1,
                    reason: format!(
                        "deserialize(): edge_index={}, cursor={}, error={:?}, bytes={:02X?}",
                        edge_index,
                        cursor,
                        e,
                        &bytes[cursor..cursor.saturating_add(20)]
                    ),
                });
            }
        };

        let next_cursor = cursor + record.size_bytes();
        if next_cursor > bytes.len() {
            let current_payload_size = payload_size;
            let new_payload_size = (next_cursor - 8) as u32;
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!(
                    "deserialize(): edge_index={}, cursor={}, edge_size={}, next_cursor={}, bytes_len={}, payload_size={} (current) -> {} (new)",
                    edge_index,
                    cursor,
                    record.size_bytes(),
                    next_cursor,
                    bytes.len(),
                    current_payload_size,
                    new_payload_size
                ),
            });
        }

        cursor = next_cursor;
        edges.push(record);
    }

    Ok((edges, payload_size))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::native::v2::edge_cluster::cluster_trace::Direction;
    use crate::backend::native::v2::string_table::StringTable;
    use crate::backend::native::{EdgeFlags, EdgeRecord};

    fn create_test_edge_record(from_id: i64, to_id: i64, edge_type: &str) -> EdgeRecord {
        EdgeRecord {
            id: 1,
            from_id,
            to_id,
            edge_type: edge_type.to_string(),
            flags: EdgeFlags::empty(),
            data: serde_json::json!([1, 2, 3]),
        }
    }

    #[test]
    fn test_serialize_empty_cluster() {
        let edges: Vec<CompactEdgeRecord> = vec![];
        let result = serialize_cluster(&edges, 0);
        assert!(result.is_ok());

        let serialized = result.unwrap();
        assert_eq!(serialized.len(), 8); // Only header (edge_count + payload_size)
        assert_eq!(serialized[0..4], [0, 0, 0, 0]); // edge_count = 0
        assert_eq!(serialized[4..8], [0, 0, 0, 0]); // payload_size = 0
    }

    #[test]
    fn test_serialize_single_edge() {
        // Create a test edge record first
        let mut string_table = StringTable::new();
        let edge_records = vec![create_test_edge_record(1, 2, "test")];
        let compact_edges = edge_records
            .iter()
            .map(|e| {
                CompactEdgeRecord::from_edge_record(e, Direction::Outgoing, &mut string_table)
                    .unwrap()
            })
            .collect::<Vec<_>>();

        let serialized_size = compact_edges.iter().map(|c| c.size_bytes()).sum();
        let result = serialize_cluster(&compact_edges, serialized_size);

        assert!(result.is_ok());
        let serialized = result.unwrap();

        // Should have header + edge data
        assert_eq!(serialized.len(), 8 + serialized_size);
        assert_eq!(serialized[0..4], [0, 0, 0, 1]); // edge_count = 1
    }

    #[test]
    fn test_verify_valid_layout() {
        // Create a valid minimal cluster (header only)
        let mut bytes = vec![0u8; 8];
        bytes[0..4].copy_from_slice(&1u32.to_be_bytes()); // edge_count = 1
        bytes[4..8].copy_from_slice(&10u32.to_be_bytes()); // payload_size = 10

        // This will fail because we have edge_count=1 but no edge data
        assert!(verify_serialized_layout(&bytes).is_err());
    }

    #[test]
    fn test_verify_empty_cluster() {
        // Create a valid empty cluster
        let bytes = vec![0u8; 8]; // edge_count=0, payload_size=0

        let result = verify_serialized_layout(&bytes);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_truncated_header() {
        let bytes = vec![1u8; 4]; // Too short for header

        let result = verify_serialized_layout(&bytes);
        assert!(result.is_err());
    }

    #[test]
    fn test_deserialize_empty_cluster() {
        let bytes = vec![0u8; 8]; // edge_count=0, payload_size=0

        let result = deserialize_cluster(&bytes);
        assert!(result.is_ok());

        let (edges, payload_size) = result.unwrap();
        assert_eq!(edges.len(), 0);
        assert_eq!(payload_size, 0);
    }

    #[test]
    fn test_round_trip_serialization() {
        // Create test data
        let mut string_table = StringTable::new();
        let edge_records = vec![
            create_test_edge_record(1, 2, "type1"),
            create_test_edge_record(1, 3, "type2"),
        ];
        let compact_edges = edge_records
            .iter()
            .map(|e| {
                CompactEdgeRecord::from_edge_record(e, Direction::Outgoing, &mut string_table)
                    .unwrap()
            })
            .collect::<Vec<_>>();

        let serialized_size = compact_edges.iter().map(|c| c.size_bytes()).sum();

        // Serialize
        let serialized = serialize_cluster(&compact_edges, serialized_size).unwrap();

        // Verify
        assert!(verify_serialized_layout(&serialized).is_ok());

        // Deserialize
        let (deserialized_edges, deserialized_size) = deserialize_cluster(&serialized).unwrap();

        assert_eq!(deserialized_edges.len(), compact_edges.len());
        assert_eq!(deserialized_size, serialized_size);
    }
}
