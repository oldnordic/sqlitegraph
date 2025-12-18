//! Header decode out-of-bounds regression tests
//!
//! Tests for the critical bug: panic: index out of bounds (len=80, index=80)
//! Location: sqlitegraph/src/backend/native/graph_file.rs:1679:13
//! Symptom: graph reopen failures / header decode path

use sqlitegraph::backend::native::{GraphFile, decode_persistent_header};
use sqlitegraph::backend::native::constants::HEADER_SIZE;
use sqlitegraph::backend::native::persistent_header::PERSISTENT_HEADER_SIZE;
use sqlitegraph::backend::native::types::NativeBackendError;
use tempfile::tempdir;
use std::fs::OpenOptions;
use std::io::Write;

#[test]
fn test_header_decode_exact_80_bytes_boundary() {
    // Test the exact boundary case that causes len=80, index=80 panic
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_boundary_80.db");

    // Create a file with exactly 80 bytes (PERSISTENT_HEADER_SIZE)
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(&file_path)
        .expect("Failed to create test file");

    // Write exactly 80 bytes of valid header data
    let mut header_data = vec![0u8; PERSISTENT_HEADER_SIZE];

    // Set valid magic bytes
    header_data[0..8].copy_from_slice(b"SQLTGRPH");

    // Set version to 2 (supported version)
    header_data[8] = 0;
    header_data[9] = 0;
    header_data[10] = 0;
    header_data[11] = 2;

    // Write the data
    file.write_all(&header_data).expect("Failed to write header data");
    file.sync_all().expect("Failed to sync file");
    drop(file);

    // Now try to reopen with GraphFile - this should NOT panic
    let result = std::panic::catch_unwind(|| {
        GraphFile::open(&file_path)
    });

    // Assert that no panic occurred
    assert!(result.is_ok(), "GraphFile::open should not panic with exactly 80 bytes");

    // Assert that the open succeeded (or at least didn't panic with out-of-bounds)
    match result.unwrap() {
        Ok(graph_file) => {
            // If open succeeds, verify basic properties
            assert_eq!(&graph_file.persistent_header().magic, b"SQLTGRPH");
        }
        Err(e) => {
            // If open fails, it should be a normal error, not a panic
            match e {
                NativeBackendError::FileTooSmall { .. } => {
                    // This would be acceptable - the error handling path
                }
                NativeBackendError::InvalidHeader { .. } => {
                    // Also acceptable
                }
                other => {
                    panic!("Unexpected error type: {:?}", other);
                }
            }
        }
    }
}

#[test]
fn test_decode_persistent_header_direct_boundary() {
    // Test the decode_persistent_header function directly with exact boundary
    let mut header_data = vec![0u8; PERSISTENT_HEADER_SIZE];

    // Set valid magic bytes
    header_data[0..8].copy_from_slice(b"SQLTGRPH");

    // Set version to 2 (supported version)
    header_data[8..12].copy_from_slice(&2u32.to_be_bytes());

    // Set some other fields to valid values
    header_data[12..16].copy_from_slice(&0u32.to_be_bytes()); // flags
    header_data[16..24].copy_from_slice(&0u64.to_be_bytes()); // node_count
    header_data[24..32].copy_from_slice(&0u64.to_be_bytes()); // edge_count
    header_data[32..36].copy_from_slice(&2u32.to_be_bytes()); // schema_version
    header_data[36..44].copy_from_slice(&80u64.to_be_bytes()); // node_data_offset (after header)
    header_data[44..52].copy_from_slice(&80u64.to_be_bytes()); // edge_data_offset (same as node_data)
    header_data[52..56].copy_from_slice(&0u32.to_be_bytes()); // cluster_size
    header_data[56..64].copy_from_slice(&80u64.to_be_bytes()); // outgoing_cluster_offset
    header_data[64..72].copy_from_slice(&80u64.to_be_bytes()); // incoming_cluster_offset
    header_data[72..80].copy_from_slice(&80u64.to_be_bytes()); // free_space_offset

    // This should NOT panic - it's the exact case that was causing len=80, index=80
    let result = std::panic::catch_unwind(|| {
        decode_persistent_header(&header_data)
    });

    assert!(result.is_ok(), "decode_persistent_header should not panic with exactly 80 bytes");

    match result.unwrap() {
        Ok(header) => {
            assert_eq!(&header.magic, b"SQLTGRPH");
            assert_eq!(header.version, 2);
            assert_eq!(header.schema_version, 2);
            assert_eq!(header.node_data_offset, 80);
            assert_eq!(header.edge_data_offset, 80);
            assert_eq!(header.outgoing_cluster_offset, 80);
            assert_eq!(header.incoming_cluster_offset, 80);
            assert_eq!(header.free_space_offset, 80);
        }
        Err(e) => {
            // Should not be a bounds-related error
            match e {
                NativeBackendError::FileTooSmall { .. } => {
                    panic!("Should not get FileTooSmall error with exactly 80 bytes");
                }
                other => {
                    // Other errors might be acceptable depending on validation logic
                    println!("Got error (acceptable): {:?}", other);
                }
            }
        }
    }
}

#[test]
fn test_header_decode_79_bytes_should_fail_gracefully() {
    // Test that 79 bytes fails gracefully without panic
    let header_data = vec![0u8; PERSISTENT_HEADER_SIZE - 1]; // 79 bytes

    let result = std::panic::catch_unwind(|| {
        decode_persistent_header(&header_data)
    });

    assert!(result.is_ok(), "Should not panic, just return error");

    match result.unwrap() {
        Ok(_) => {
            panic!("Should not successfully decode header with only 79 bytes");
        }
        Err(e) => {
            match e {
                NativeBackendError::FileTooSmall { size, min_size } => {
                    assert_eq!(size, 79);
                    assert_eq!(min_size, 80);
                }
                other => {
                    panic!("Expected FileTooSmall error, got: {:?}", other);
                }
            }
        }
    }
}

#[test]
fn test_header_decode_81_bytes_should_work() {
    // Test that 81 bytes works fine (extra byte after header)
    let mut header_data = vec![0u8; PERSISTENT_HEADER_SIZE + 1]; // 81 bytes

    // Set valid magic bytes
    header_data[0..8].copy_from_slice(b"SQLTGRPH");
    header_data[8..12].copy_from_slice(&1u32.to_be_bytes());

    let result = std::panic::catch_unwind(|| {
        decode_persistent_header(&header_data)
    });

    assert!(result.is_ok(), "Should not panic with 81 bytes");

    match result.unwrap() {
        Ok(header) => {
            assert_eq!(&header.magic, b"SQLTGRPH");
            assert_eq!(header.version, 2);
        }
        Err(e) => {
            panic!("Should successfully decode header with 81 bytes, got error: {:?}", e);
        }
    }
}

#[test]
fn test_graphfile_reopen_with_exact_header_size() {
    // Full integration test: create graph, close, reopen - should not panic
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_reopen.db");

    // Create a graph file first
    {
        let mut graph_file = GraphFile::create(&file_path).expect("Failed to create graph file");

        // Write some data to make it realistic
        graph_file.write_header().expect("Failed to write header");

        // The file should be properly sized now
        assert!(graph_file.file_size().unwrap() >= HEADER_SIZE);
    }

    // Now reopen - this is where the panic was occurring
    let result = std::panic::catch_unwind(|| {
        GraphFile::open(&file_path)
    });

    assert!(result.is_ok(), "GraphFile reopen should not panic");

    match result.unwrap() {
        Ok(mut graph_file) => {
            // Verify we can read the header back
            graph_file.read_header().expect("Should be able to read header");
            assert_eq!(&graph_file.persistent_header().magic, b"SQLTGRPH");
        }
        Err(e) => {
            panic!("Should be able to reopen graph file: {:?}", e);
        }
    }
}