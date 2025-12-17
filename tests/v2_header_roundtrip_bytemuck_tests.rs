#![cfg(feature = "v2_experimental")]

//! Tests for V2 node header roundtrip serialization using bytemuck

use sqlitegraph::backend::native::v2::node_record_v2::NodeRecordV2;
use sqlitegraph::backend::native::{NodeFlags, FileOffset};
use serde_json::json;

#[test]
fn test_v2_node_header_bytemuck_roundtrip() {
    // Test that NodeRecordV2 header can be roundtripped through bytemuck

    // Create test node
    let original = NodeRecordV2 {
        id: 42,
        flags: NodeFlags(0x12345678),
        kind: "TestNode".to_string(),
        name: "test_node_42".to_string(),
        data: json!({"key": "value", "number": 42}),
        outgoing_cluster_offset: 8192,
        outgoing_cluster_size: 1024,
        outgoing_edge_count: 5,
        incoming_cluster_offset: 16384,
        incoming_cluster_size: 512,
        incoming_edge_count: 3,
    };

    // Serialize using current implementation
    let serialized = original.serialize();

    // Deserialize using current implementation
    let deserialized = NodeRecordV2::deserialize(&serialized).expect("Should deserialize successfully");

    // Verify all fields match exactly
    assert_eq!(deserialized.id, original.id, "Node ID should match");
    assert_eq!(deserialized.flags, original.flags, "Flags should match");
    assert_eq!(deserialized.kind, original.kind, "Kind should match");
    assert_eq!(deserialized.name, original.name, "Name should match");
    assert_eq!(deserialized.data, original.data, "Data should match");
    assert_eq!(deserialized.outgoing_cluster_offset, original.outgoing_cluster_offset, "Outgoing cluster offset should match");
    assert_eq!(deserialized.outgoing_cluster_size, original.outgoing_cluster_size, "Outgoing cluster size should match");
    assert_eq!(deserialized.outgoing_edge_count, original.outgoing_edge_count, "Outgoing edge count should match");
    assert_eq!(deserialized.incoming_cluster_offset, original.incoming_cluster_offset, "Incoming cluster offset should match");
    assert_eq!(deserialized.incoming_cluster_size, original.incoming_cluster_size, "Incoming cluster size should match");
    assert_eq!(deserialized.incoming_edge_count, original.incoming_edge_count, "Incoming edge count should match");
}

#[test]
fn test_v2_node_header_fixed_layout() {
    // Test the exact byte layout of the fixed portion of V2 node header

    let node = NodeRecordV2 {
        id: 12345,
        flags: NodeFlags(0x89ABCDEF),
        kind: "Function".to_string(),
        name: "my_function".to_string(),
        data: json!({"complex": true}),
        outgoing_cluster_offset: 4096,
        outgoing_cluster_size: 2048,
        outgoing_edge_count: 10,
        incoming_cluster_offset: 6144,
        incoming_cluster_size: 1024,
        incoming_edge_count: 7,
    };

    let bytes = node.serialize();

    // Verify fixed header layout (first 21 bytes)
    assert_eq!(bytes[0], 2, "Version byte should be 2");

    // Flags (bytes 1-4) - 0x89ABCDEF in big-endian
    assert_eq!(bytes[1], 0x89);
    assert_eq!(bytes[2], 0xAB);
    assert_eq!(bytes[3], 0xCD);
    assert_eq!(bytes[4], 0xEF);

    // Node ID (bytes 5-12) - 12345 in big-endian
    assert_eq!(bytes[5], 0);
    assert_eq!(bytes[6], 0);
    assert_eq!(bytes[7], 0);
    assert_eq!(bytes[8], 0);
    assert_eq!(bytes[9], 0);
    assert_eq!(bytes[10], 0x30);
    assert_eq!(bytes[11], 0x39);

    // Kind length (bytes 13-14) - "Function" = 8 chars
    assert_eq!(u16::from_be_bytes([bytes[13], bytes[14]]), 8);

    // Name length (bytes 15-16) - "my_function" = 11 chars
    assert_eq!(u16::from_be_bytes([bytes[15], bytes[16]]), 11);

    // Data length (bytes 17-20) - JSON object size
    let data_len = u32::from_be_bytes([bytes[17], bytes[18], bytes[19], bytes[20]]);
    assert!(data_len > 0, "Data length should be positive");
}

#[test]
fn test_v2_cluster_footer_layout() {
    // Test the exact byte layout of the 32-byte cluster metadata at the end

    let node = NodeRecordV2 {
        id: 999,
        flags: NodeFlags::empty(),
        kind: "Test".to_string(),
        name: "cluster_test".to_string(),
        data: json!({}),
        outgoing_cluster_offset: 32768,
        outgoing_cluster_size: 4096,
        outgoing_edge_count: 15,
        incoming_cluster_offset: 49152,
        incoming_cluster_size: 2048,
        incoming_edge_count: 8,
    };

    let bytes = node.serialize();

    // Find cluster footer (last 32 bytes)
    let footer_start = bytes.len() - 32;
    let footer = &bytes[footer_start..];

    // Verify outgoing cluster metadata (first 16 bytes of footer)
    assert_eq!(u64::from_be_bytes([
        footer[0], footer[1], footer[2], footer[3],
        footer[4], footer[5], footer[6], footer[7]
    ]), 32768, "Outgoing cluster offset");

    assert_eq!(u32::from_be_bytes([
        footer[8], footer[9], footer[10], footer[11]
    ]), 4096, "Outgoing cluster size");

    assert_eq!(u32::from_be_bytes([
        footer[12], footer[13], footer[14], footer[15]
    ]), 15, "Outgoing edge count");

    // Verify incoming cluster metadata (last 16 bytes of footer)
    assert_eq!(u64::from_be_bytes([
        footer[16], footer[17], footer[18], footer[19],
        footer[20], footer[21], footer[22], footer[23]
    ]), 49152, "Incoming cluster offset");

    assert_eq!(u32::from_be_bytes([
        footer[24], footer[25], footer[26], footer[27]
    ]), 2048, "Incoming cluster size");

    assert_eq!(u32::from_be_bytes([
        footer[28], footer[29], footer[30], footer[31]
    ]), 8, "Incoming edge count");
}