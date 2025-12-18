//! Integration tests for edge storage capacity coordination
//!
//! These tests use real file operations (no mocks) to verify that edge ID allocation
//! properly coordinates with file capacity management.

use crate::backend::native::edge_store::capacity_coordinator::EdgeCapacityCoordinator;
use crate::backend::native::types::{EdgeRecord, EdgeFlags, NativeEdgeId};
use crate::backend::native::graph_file::GraphFile;
use tempfile::NamedTempFile;

/// Helper function to calculate edge offset
fn calculate_edge_offset(edge_id: u64) -> u64 {
    let base_offset = 4096; // Default edge_data_offset after GraphFile::create()
    base_offset + ((edge_id - 1) * 256)
}

#[test]
fn test_allocate_edge_id_ensures_file_capacity() {
    // Create real temporary file - no mocks
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path())
        .expect("Failed to create graph file");

    // Use capacity coordinator to allocate edge ID
    let mut coordinator = EdgeCapacityCoordinator::new(&mut graph_file);
    let edge_id = coordinator.allocate_edge_id_with_capacity()
        .expect("Failed to allocate edge ID with capacity");

    // File should be large enough for this edge
    let edge_offset = calculate_edge_offset(edge_id as u64);
    let file_size = graph_file.file_size()
        .expect("Failed to get file size");

    assert!(
        file_size >= edge_offset + 256,
        "File size {} should be >= {} for edge {}",
        file_size,
        edge_offset + 256,
        edge_id
    );

    // Verify we can actually read from the edge slot
    let mut buffer = vec![0u8; 256];
    let result = graph_file.read_bytes(edge_offset, &mut buffer);
    assert!(result.is_ok(), "Should be able to read from edge slot");
}

#[test]
fn test_multiple_edge_allocation_grows_file_appropriately() {
    // Create real temporary file
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path())
        .expect("Failed to create graph file");

    // Allocate multiple edges
    let mut edge_ids = Vec::new();
    let mut coordinator = EdgeCapacityCoordinator::new(&mut graph_file);

    for i in 0..10 {
        let edge_id = coordinator.allocate_edge_id_with_capacity()
            .expect("Failed to allocate edge ID with capacity");
        edge_ids.push(edge_id);

        // Verify file is large enough for this edge immediately after allocation
        let edge_offset = calculate_edge_offset(edge_id as u64);
        let file_size = graph_file.file_size()
            .expect("Failed to get file size");
        assert!(
            file_size >= edge_offset + 256,
            "File should be large enough for edge {} after iteration {}",
            edge_id, i
        );
    }

    // Final verification - file should be large enough for all edges
    let file_size = graph_file.file_size()
        .expect("Failed to get final file size");
    let max_edge_id = *edge_ids.iter().max().unwrap();
    let max_offset = calculate_edge_offset(max_edge_id as u64);

    assert!(
        file_size >= max_offset + 256,
        "Final file size {} should be >= {} for max edge {}",
        file_size,
        max_offset + 256,
        max_edge_id
    );
}

#[test]
fn test_edge_write_succeeds_after_capacity_ensured() {
    // Create real temporary file
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path())
        .expect("Failed to create graph file");

    // Allocate edge with capacity coordination
    let mut coordinator = EdgeCapacityCoordinator::new(&mut graph_file);
    let edge_id = coordinator.allocate_edge_id_with_capacity()
        .expect("Failed to allocate edge ID with capacity");

    let edge = EdgeRecord {
        id: edge_id,
        from_id: 1,
        to_id: 2,
        edge_type: "TEST".to_string(),
        flags: EdgeFlags(0),
        data: serde_json::json!({"test": true}),
    };

    // This should not fail - file should have capacity
    let edge_offset = calculate_edge_offset(edge_id as u64);
    let serialized = serde_json::to_vec(&edge)
        .expect("Failed to serialize edge");

    // Ensure buffer fits in fixed slot
    assert!(serialized.len() <= 256, "Edge should fit in 256-byte slot");

    // Write to file
    graph_file.write_bytes(edge_offset, &serialized)
        .expect("Should be able to write edge after capacity coordination");

    // Verify we can read it back
    let mut read_buffer = vec![0u8; serialized.len()];
    graph_file.read_bytes(edge_offset, &mut read_buffer)
        .expect("Should be able to read edge back");

    assert_eq!(read_buffer, serialized, "Read data should match written data");
}

#[test]
fn test_edge_operations_never_fail_with_capacity_coordination() {
    // Create real temporary file
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path())
        .expect("Failed to create graph file");

    let mut coordinator = EdgeCapacityCoordinator::new(&mut graph_file);

    // Test many edge operations - none should fail with capacity errors
    for i in 1..50 {
        let edge_id = coordinator.allocate_edge_id_with_capacity()
            .expect("Failed to allocate edge ID with capacity");

        let edge = EdgeRecord {
            id: edge_id,
            from_id: i as i64,
            to_id: (i + 1) as i64,
            edge_type: format!("EDGE_{}", i),
            flags: EdgeFlags(0),
            data: serde_json::json!({"index": i}),
        };

        // These operations should never fail
        let edge_offset = calculate_edge_offset(edge_id as u64);
        let serialized = serde_json::to_vec(&edge)
            .expect("Failed to serialize edge");

        // Write edge
        graph_file.write_bytes(edge_offset, &serialized)
            .expect(&format!("Failed to write edge {} in iteration {}", edge_id, i));

        // Read edge back
        let mut read_buffer = vec![0u8; serialized.len()];
        graph_file.read_bytes(edge_offset, &mut read_buffer)
            .expect(&format!("Failed to read edge {} in iteration {}", edge_id, i));

        assert_eq!(
            read_buffer, serialized,
            "Read data should match written data for edge {} in iteration {}",
            edge_id, i
        );
    }
}

#[test]
fn test_capacity_coordinator_prevents_beyond_end_of_file_errors() {
    // This test specifically targets the "Attempted read beyond end of file" error
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path())
        .expect("Failed to create graph file");

    let mut coordinator = EdgeCapacityCoordinator::new(&mut graph_file);

    // Allocate an edge that would normally be beyond file size
    let edge_id = coordinator.allocate_edge_id_with_capacity()
        .expect("Failed to allocate edge ID with capacity");

    // The file should now be large enough to read from this edge's location
    let edge_offset = calculate_edge_offset(edge_id as u64);
    let mut buffer = vec![0u8; 256];

    // This should not fail with "Attempted read beyond end of file"
    let result = graph_file.read_bytes(edge_offset, &mut buffer);

    assert!(
        result.is_ok(),
        "Should be able to read from edge {} at offset {} without 'beyond end of file' error",
        edge_id, edge_offset
    );
}

#[test]
fn test_capacity_coordinator_with_very_large_edge_ids() {
    // Test capacity coordination with larger edge IDs
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let mut graph_file = GraphFile::create(temp_file.path())
        .expect("Failed to create graph file");

    let mut coordinator = EdgeCapacityCoordinator::new(&mut graph_file);

    // Skip ahead to a larger edge ID by allocating many edges
    let mut target_edge_id = 0;
    for _ in 0..100 {
        target_edge_id = coordinator.allocate_edge_id_with_capacity()
            .expect("Failed to allocate edge ID with capacity");
    }

    // File should be large enough for this large edge ID
    let edge_offset = calculate_edge_offset(target_edge_id as u64);
    let file_size = graph_file.file_size()
        .expect("Failed to get file size");

    assert!(
        file_size >= edge_offset + 256,
        "File should be large enough for large edge ID {} at offset {}",
        target_edge_id, edge_offset
    );
}