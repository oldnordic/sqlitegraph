//! Header encoding and decoding utilities for persistent header operations
//!
//! This module provides safe encoding and decoding functions for the
//! PersistentHeaderV2 structure, ensuring big-endian serialization
//! and bounds-checked slice access.

use crate::backend::native::{
    types::NativeResult,
    persistent_header::{PersistentHeaderV2, PERSISTENT_HEADER_SIZE},
    constants::HEADER_SIZE,
};

/// Encode a PersistentHeaderV2 into binary format
///
/// Serializes the header to a byte vector using big-endian encoding.
/// Includes assertions to ensure the encoded header size matches expectations.
pub fn encode_persistent_header(header: &PersistentHeaderV2) -> NativeResult<Vec<u8>> {
    let mut buffer = Vec::with_capacity(PERSISTENT_HEADER_SIZE);

    // Write magic bytes
    buffer.extend_from_slice(&header.magic);

    // Write version (big-endian)
    buffer.extend_from_slice(&header.version.to_be_bytes());

    // Write flags (big-endian)
    buffer.extend_from_slice(&header.flags.to_be_bytes());

    // Write node count (big-endian)
    buffer.extend_from_slice(&header.node_count.to_be_bytes());

    // Write edge count (big-endian)
    buffer.extend_from_slice(&header.edge_count.to_be_bytes());

    // Write schema version (big-endian)
    buffer.extend_from_slice(&header.schema_version.to_be_bytes());

    // Write node data offset (big-endian)
    buffer.extend_from_slice(&header.node_data_offset.to_be_bytes());

    // Write edge data offset (big-endian)
    buffer.extend_from_slice(&header.edge_data_offset.to_be_bytes());

    // Write V2 cluster offsets (big-endian)
    buffer.extend_from_slice(&header.outgoing_cluster_offset.to_be_bytes());
    buffer.extend_from_slice(&header.incoming_cluster_offset.to_be_bytes());
    buffer.extend_from_slice(&header.free_space_offset.to_be_bytes());

    assert_eq!(
        buffer.len(),
        PERSISTENT_HEADER_SIZE,
        "Persistent header encoding size mismatch"
    );
    assert_eq!(
        buffer.len(),
        HEADER_SIZE as usize,
        "Header must match constants::HEADER_SIZE"
    );

    Ok(buffer)
}

/// Decode a PersistentHeaderV2 from binary data
///
/// Safely deserializes a header from bytes with bounds checking.
/// Returns error if the data is too small or contains invalid offsets.
pub fn decode_persistent_header(bytes: &[u8]) -> NativeResult<PersistentHeaderV2> {
    if bytes.len() < PERSISTENT_HEADER_SIZE {
        return Err(crate::backend::native::types::NativeBackendError::FileTooSmall {
            size: bytes.len() as u64,
            min_size: PERSISTENT_HEADER_SIZE as u64,
        });
    }

    let mut offset = 0;

    // Read magic bytes
    let magic_slice = get_slice_safe(bytes, offset, 8)?;
    let mut magic = [0u8; 8];
    magic.copy_from_slice(magic_slice);
    offset += 8;

    // Read version
    let version_slice = get_slice_safe(bytes, offset, 4)?;
    let version = u32::from_be_bytes([
        version_slice[0],
        version_slice[1],
        version_slice[2],
        version_slice[3],
    ]);
    offset += 4;

    // Read flags
    let flags_slice = get_slice_safe(bytes, offset, 4)?;
    let flags = u32::from_be_bytes([
        flags_slice[0],
        flags_slice[1],
        flags_slice[2],
        flags_slice[3],
    ]);
    offset += 4;

    // Read node count
    let node_count_slice = get_slice_safe(bytes, offset, 8)?;
    let node_count = u64::from_be_bytes([
        node_count_slice[0],
        node_count_slice[1],
        node_count_slice[2],
        node_count_slice[3],
        node_count_slice[4],
        node_count_slice[5],
        node_count_slice[6],
        node_count_slice[7],
    ]);
    offset += 8;

    // Read edge count
    let edge_count_slice = get_slice_safe(bytes, offset, 8)?;
    let edge_count = u64::from_be_bytes([
        edge_count_slice[0],
        edge_count_slice[1],
        edge_count_slice[2],
        edge_count_slice[3],
        edge_count_slice[4],
        edge_count_slice[5],
        edge_count_slice[6],
        edge_count_slice[7],
    ]);
    offset += 8;

    // Read schema version
    let schema_version_slice = get_slice_safe(bytes, offset, 8)?;  // TODO: This should probably be 4 bytes, not 8
    let schema_version = u64::from_be_bytes([
        schema_version_slice[0],
        schema_version_slice[1],
        schema_version_slice[2],
        schema_version_slice[3],
        schema_version_slice[4],
        schema_version_slice[5],
        schema_version_slice[6],
        schema_version_slice[7],
    ]);
    offset += 8;

    // Read node data offset
    let node_data_offset_slice = get_slice_safe(bytes, offset, 8)?;
    let node_data_offset = u64::from_be_bytes([
        node_data_offset_slice[0],
        node_data_offset_slice[1],
        node_data_offset_slice[2],
        node_data_offset_slice[3],
        node_data_offset_slice[4],
        node_data_offset_slice[5],
        node_data_offset_slice[6],
        node_data_offset_slice[7],
    ]);
    offset += 8;

    // Read edge data offset
    let edge_data_offset_slice = get_slice_safe(bytes, offset, 8)?;
    let edge_data_offset = u64::from_be_bytes([
        edge_data_offset_slice[0],
        edge_data_offset_slice[1],
        edge_data_offset_slice[2],
        edge_data_offset_slice[3],
        edge_data_offset_slice[4],
        edge_data_offset_slice[5],
        edge_data_offset_slice[6],
        edge_data_offset_slice[7],
    ]);
    offset += 8;

    let mut outgoing_cluster_offset = 0u64;
    let mut incoming_cluster_offset = 0u64;
    let mut free_space_offset = 0u64;

    if bytes.len() >= HEADER_SIZE as usize {
        // HEADER_VALIDATE_DEBUG: Track byte positions
        if std::env::var("HEADER_VALIDATE_DEBUG").is_ok() {
            println!(
                "[HEADER_READ_DEBUG] Reading outgoing_cluster_offset at offset {} (should be 56)",
                offset
            );
            let outgoing_bytes = get_slice_safe(bytes, offset, 8)?;
            println!(
                "[HEADER_READ_DEBUG] Raw outgoing bytes: {:02x?}",
                outgoing_bytes
            );
        }

        let outgoing_slice = get_slice_safe(bytes, offset, 8)?;
        outgoing_cluster_offset = u64::from_be_bytes([
            outgoing_slice[0],
            outgoing_slice[1],
            outgoing_slice[2],
            outgoing_slice[3],
            outgoing_slice[4],
            outgoing_slice[5],
            outgoing_slice[6],
            outgoing_slice[7],
        ]);
        offset += 8;

        // HEADER_VALIDATE_DEBUG: Track byte positions
        if std::env::var("HEADER_VALIDATE_DEBUG").is_ok() {
            println!(
                "[HEADER_READ_DEBUG] Reading incoming_cluster_offset at offset {} (should be 64)",
                offset
            );
            let incoming_bytes = get_slice_safe(bytes, offset, 8)?;
            println!(
                "[HEADER_READ_DEBUG] Raw incoming bytes: {:02x?}",
                incoming_bytes
            );
        }

        let incoming_slice = get_slice_safe(bytes, offset, 8)?;
        incoming_cluster_offset = u64::from_be_bytes([
            incoming_slice[0],
            incoming_slice[1],
            incoming_slice[2],
            incoming_slice[3],
            incoming_slice[4],
            incoming_slice[5],
            incoming_slice[6],
            incoming_slice[7],
        ]);
        offset += 8;

        let free_space_slice = get_slice_safe(bytes, offset, 8)?;
        free_space_offset = u64::from_be_bytes([
            free_space_slice[0],
            free_space_slice[1],
            free_space_slice[2],
            free_space_slice[3],
            free_space_slice[4],
            free_space_slice[5],
            free_space_slice[6],
            free_space_slice[7],
        ]);
        offset += 8;
    }

    Ok(PersistentHeaderV2 {
        magic,
        version,
        flags,
        node_count,
        edge_count,
        schema_version,
        node_data_offset,
        edge_data_offset,
        outgoing_cluster_offset,
        incoming_cluster_offset,
        free_space_offset,
    })
}

/// Helper function for safe slice access with bounds checking
///
/// Provides safe access to byte slices with comprehensive bounds checking
/// to prevent buffer overflows and invalid memory access.
pub fn get_slice_safe(data: &[u8], start: usize, len: usize) -> NativeResult<&[u8]> {
    if start.checked_add(len).map_or(true, |end| end > data.len()) {
        return Err(crate::backend::native::types::NativeBackendError::InvalidHeader {
            field: "header_data".to_string(),
            reason: format!("slice access out of bounds: start={}, len={}, data_len={}",
                          start, len, data.len()),
        });
    }
    // This is safe now because we checked the bounds above
    Ok(&data[start..start + len])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_persistent_header_encode_decode_roundtrip() {
        use crate::backend::native::v2::V2_MAGIC;

        let header = PersistentHeaderV2 {
            magic: V2_MAGIC,
            version: 2,
            flags: crate::backend::native::constants::DEFAULT_FEATURE_FLAGS,
            node_count: 100,
            edge_count: 500,
            schema_version: 1,
            node_data_offset: 1024,
            edge_data_offset: 8192,
            outgoing_cluster_offset: 16384,
            incoming_cluster_offset: 24576,
            free_space_offset: 32768,
        };

        let encoded = encode_persistent_header(&header).unwrap();
        let decoded = decode_persistent_header(&encoded).unwrap();

        assert_eq!(header.node_count, decoded.node_count);
        assert_eq!(header.edge_count, decoded.edge_count);
        assert_eq!(header.schema_version, decoded.schema_version);
        assert_eq!(header.node_data_offset, decoded.node_data_offset);
        assert_eq!(header.edge_data_offset, decoded.edge_data_offset);
        assert_eq!(header.magic, decoded.magic);
        assert_eq!(header.version, decoded.version);
        assert_eq!(header.flags, decoded.flags);
    }

    #[test]
    fn test_persistent_header_encode_size() {
        let header = PersistentHeaderV2::new_v2();
        let encoded = encode_persistent_header(&header).unwrap();

        assert_eq!(encoded.len(), PERSISTENT_HEADER_SIZE);
        assert_eq!(encoded.len(), HEADER_SIZE as usize);
    }

    #[test]
    fn test_decode_header_too_small() {
        let small_data = vec![0u8; 10];
        let result = decode_persistent_header(&small_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_slice_safe_valid() {
        let data = vec![1, 2, 3, 4, 5];
        let slice = get_slice_safe(&data, 1, 3).unwrap();
        assert_eq!(slice, &[2, 3, 4]);
    }

    #[test]
    fn test_get_slice_safe_out_of_bounds() {
        let data = vec![1, 2, 3, 4, 5];
        let result = get_slice_safe(&data, 3, 5);
        assert!(result.is_err());
    }

    #[test]
    fn test_get_slice_safe_overflow() {
        let data = vec![1, 2, 3, 4, 5];
        let result = get_slice_safe(&data, usize::MAX, 1);
        assert!(result.is_err());
    }

    #[test]
    fn test_header_constants_consistency() {
        // Ensure that the constants we're using are consistent
        assert_eq!(PERSISTENT_HEADER_SIZE, HEADER_SIZE as usize);
    }
}