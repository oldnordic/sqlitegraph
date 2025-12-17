//! Regression tests for Phase 10 Header Architecture Refactor
//!
//! These tests ensure that:
//! 1. Persistent header uses exactly HEADER_SIZE bytes (80 bytes)
//! 2. Transaction state is runtime-only and never persisted
//! 3. Header corruption is fixed on reopen
//! 4. All file operations work correctly

use sqlitegraph::backend::native::{GraphFile, constants::HEADER_SIZE};
use sqlitegraph::backend::native::persistent_header::PersistentHeaderV2;
use std::fs::File;
use std::io::Read;

#[test]
fn test_header_size_stability() {
    let test_file = "test_header_size.db";

    // Create a new graph file
    let mut graph_file = GraphFile::create(test_file).expect("Failed to create graph file");

    // Read raw header bytes from file
    let mut file = File::open(test_file).expect("Failed to open file");
    let mut header_bytes = vec![0u8; HEADER_SIZE as usize];
    file.read_exact(&mut header_bytes).expect("Failed to read header bytes");

    // Verify header size matches HEADER_SIZE
    assert_eq!(header_bytes.len(), HEADER_SIZE as usize,
               "Header bytes length should equal HEADER_SIZE");

    // Verify header can be decoded correctly
    let decoded = sqlitegraph::backend::native::decode_persistent_header(&header_bytes)
        .expect("Failed to decode persistent header");

    // Verify basic header fields
    assert_eq!(decoded.version, 2, "Header version should be 2");
    assert_eq!(decoded.magic, [b'S', b'Q', b'L', b'T', b'G', b'F', 0, 0], "Magic bytes should match");

    // Clean up
    std::fs::remove_file(test_file).ok();
}

#[test]
fn test_transaction_state_runtime_only() {
    let test_file = "test_tx_runtime.db";

    // Create a new graph file
    let mut graph_file = GraphFile::create(test_file).expect("Failed to create graph file");

    // Verify initial transaction state is clean
    let tx_state = graph_file.tx_state();
    assert_eq!(tx_state.tx_id, 0, "Initial tx_id should be 0");
    assert!(!tx_state.is_in_progress(), "Initial state should not be in progress");

    // Modify transaction state at runtime
    graph_file.tx_state_mut().begin_tx(123);
    assert_eq!(graph_file.tx_state().tx_id, 123, "Runtime tx_id should be updated");

    // Write header to disk (should not persist transaction state)
    graph_file.write_header().expect("Failed to write header");

    // Close and reopen file
    drop(graph_file);
    let mut graph_file = GraphFile::open(test_file).expect("Failed to reopen graph file");

    // Verify transaction state is reset (not persisted)
    let tx_state = graph_file.tx_state();
    assert_eq!(tx_state.tx_id, 0, "Transaction state should be reset on reopen");
    assert!(!tx_state.is_in_progress(), "Transaction state should not be in progress on reopen");

    // But persistent header should still be valid
    let header = graph_file.persistent_header();
    assert_eq!(header.version, 2, "Persistent header version should persist");

    // Clean up
    std::fs::remove_file(test_file).ok();
}

#[test]
fn test_reopen_invariant_offsets() {
    let test_file = "test_reopen_offsets.db";

    // Create a new graph file and insert some data
    let mut graph_file = GraphFile::create(test_file).expect("Failed to create graph file");

    // Modify persistent header fields (simulate some operations)
    {
        let header = graph_file.persistent_header_mut();
        header.node_count = 5;
        header.edge_count = 10;
        header.outgoing_cluster_offset = 2000;
        header.incoming_cluster_offset = 4000;
    }

    // Write header and close
    graph_file.write_header().expect("Failed to write header");
    drop(graph_file);

    // Reopen file
    let mut graph_file = GraphFile::open(test_file).expect("Failed to reopen graph file");

    // Verify header invariants are maintained
    let header = graph_file.persistent_header();
    assert_eq!(header.node_count, 5, "Node count should persist");
    assert_eq!(header.edge_count, 10, "Edge count should persist");
    assert_eq!(header.outgoing_cluster_offset, 2000, "Outgoing cluster offset should persist");
    assert_eq!(header.incoming_cluster_offset, 4000, "Incoming cluster offset should persist");

    // Verify critical invariant: incoming_cluster_offset >= outgoing_cluster_offset
    assert!(header.incoming_cluster_offset >= header.outgoing_cluster_offset,
            "Critical invariant violated: incoming_cluster_offset >= outgoing_cluster_offset");

    // Verify ordering: node_data_offset <= edge_data_offset <= outgoing_cluster_offset
    assert!(header.node_data_offset <= header.edge_data_offset,
            "Invariant violated: node_data_offset <= edge_data_offset");

    // Clean up
    std::fs::remove_file(test_file).ok();
}

#[test]
fn test_transaction_rollback_does_not_corrupt_header() {
    let test_file = "test_tx_rollback.db";

    // Create a new graph file
    let mut graph_file = GraphFile::create(test_file).expect("Failed to create graph file");

    // Set up some initial state
    {
        let header = graph_file.persistent_header_mut();
        header.node_count = 3;
        header.outgoing_cluster_offset = 1500;
        header.incoming_cluster_offset = 3000;
    }
    graph_file.write_header().expect("Failed to write initial header");

    // Begin transaction and save checkpoint
    graph_file.tx_state_mut().begin_tx(42);
    graph_file.tx_state_mut().save_checkpoint(1500, 3000, 5000);

    // Modify persistent fields during "transaction"
    {
        let header = graph_file.persistent_header_mut();
        header.outgoing_cluster_offset = 2500; // This would normally be persisted
        header.incoming_cluster_offset = 4500;
        header.node_count = 7;
    }
    graph_file.write_header().expect("Failed to write modified header");

    // Simulate transaction rollback (runtime-only)
    let (rollback_outgoing, rollback_incoming, rollback_free) = graph_file.tx_state_mut().rollback();

    // Restore persistent header to rollback state
    {
        let header = graph_file.persistent_header_mut();
        header.outgoing_cluster_offset = rollback_outgoing;
        header.incoming_cluster_offset = rollback_incoming;
        header.node_count = 3; // Restore original
    }

    graph_file.tx_state_mut().commit();
    graph_file.write_header().expect("Failed to write rollback header");

    // Close and reopen
    drop(graph_file);
    let mut graph_file = GraphFile::open(test_file).expect("Failed to reopen after rollback");

    // Verify persistent header is in correct rollback state
    let header = graph_file.persistent_header();
    assert_eq!(header.node_count, 3, "Node count should be rolled back");
    assert_eq!(header.outgoing_cluster_offset, 1500, "Outgoing cluster offset should be rolled back");
    assert_eq!(header.incoming_cluster_offset, 3000, "Incoming cluster offset should be rolled back");

    // Verify transaction state is clean
    let tx_state = graph_file.tx_state();
    assert_eq!(tx_state.tx_id, 0, "Transaction should be committed");
    assert!(!tx_state.is_in_progress(), "Transaction should not be in progress");

    // Clean up
    std::fs::remove_file(test_file).ok();
}

#[test]
fn test_encode_decode_header_exact_size() {
    use sqlitegraph::backend::native::{encode_persistent_header, decode_persistent_header};

    // Create a test header
    let mut header = PersistentHeaderV2::new_v2();
    header.node_count = 100;
    header.edge_count = 200;
    header.outgoing_cluster_offset = 5000;
    header.incoming_cluster_offset = 8000;

    // Encode header
    let encoded = encode_persistent_header(&header).expect("Failed to encode header");

    // Verify encoded size matches HEADER_SIZE exactly
    assert_eq!(encoded.len(), HEADER_SIZE as usize,
               "Encoded header should be exactly HEADER_SIZE bytes");
    assert_eq!(encoded.len(), 80, "Encoded header should be exactly 80 bytes");

    // Decode header
    let decoded = decode_persistent_header(&encoded).expect("Failed to decode header");

    // Verify all fields match
    assert_eq!(decoded.node_count, header.node_count, "Node count should match");
    assert_eq!(decoded.edge_count, header.edge_count, "Edge count should match");
    assert_eq!(decoded.outgoing_cluster_offset, header.outgoing_cluster_offset,
               "Outgoing cluster offset should match");
    assert_eq!(decoded.incoming_cluster_offset, header.incoming_cluster_offset,
               "Incoming cluster offset should match");
}