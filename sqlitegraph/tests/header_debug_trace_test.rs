//! Debug test to trace exactly what happens during header decode with 80 bytes

use sqlitegraph::backend::native::constants::HEADER_SIZE;
use sqlitegraph::backend::native::decode_persistent_header;
use sqlitegraph::backend::native::persistent_header::PERSISTENT_HEADER_SIZE;

#[test]
fn test_debug_trace_decode_80_bytes() {
    println!("PERSISTENT_HEADER_SIZE = {}", PERSISTENT_HEADER_SIZE);
    println!("HEADER_SIZE = {}", HEADER_SIZE);

    // Create a buffer with exactly 80 bytes
    let mut header_data = vec![0u8; PERSISTENT_HEADER_SIZE];

    // Set some basic valid data to get past the initial checks
    header_data[0..8].copy_from_slice(b"SQLTGRPH"); // magic
    header_data[8..12].copy_from_slice(&2u32.to_be_bytes()); // version

    println!("Created header buffer of {} bytes", header_data.len());
    println!("First 16 bytes: {:02x?}", &header_data[..16]);

    // Let's manually check what offset will be used where the issue might occur
    let mut offset = 0;
    println!("Initial offset = {}", offset);

    offset += 8; // magic
    println!("After magic: offset = {}", offset);

    offset += 4; // version
    println!("After version: offset = {}", offset);

    offset += 4; // flags
    println!("After flags: offset = {}", offset);

    offset += 8; // node_count
    println!("After node_count: offset = {}", offset);

    offset += 8; // edge_count
    println!("After edge_count: offset = {}", offset);

    offset += 4; // schema_version
    println!("After schema_version: offset = {}", offset);

    offset += 8; // node_data_offset
    println!("After node_data_offset: offset = {}", offset);

    offset += 8; // edge_data_offset
    println!("After edge_data_offset: offset = {}", offset);

    offset += 4; // cluster_size
    println!("After cluster_size: offset = {}", offset);

    // At this point, offset should be 56, and we're about to read outgoing_cluster_offset
    println!("About to read outgoing_cluster_offset at offset {}", offset);
    println!("bytes[{}..{}] would be accessed", offset, offset + 8);
    println!(
        "bytes[{} + 7] = bytes[{}] would be accessed",
        offset,
        offset + 7
    );

    if offset + 8 >= header_data.len() {
        println!(
            "WARNING: offset + 8 ({}) >= header_data.len() ({})",
            offset + 8,
            header_data.len()
        );
    } else {
        println!(
            "OK: offset + 8 ({}) < header_data.len() ({})",
            offset + 8,
            header_data.len()
        );
    }

    // Continue tracing...
    offset += 8; // outgoing_cluster_offset
    println!("After outgoing_cluster_offset: offset = {}", offset);

    // Now offset should be 64, and we're about to read incoming_cluster_offset
    println!("About to read incoming_cluster_offset at offset {}", offset);
    println!("bytes[{}..{}] would be accessed", offset, offset + 8);
    println!(
        "bytes[{} + 7] = bytes[{}] would be accessed",
        offset,
        offset + 7
    );

    if offset + 8 >= header_data.len() {
        println!(
            "WARNING: offset + 8 ({}) >= header_data.len() ({})",
            offset + 8,
            header_data.len()
        );
    } else {
        println!(
            "OK: offset + 8 ({}) < header_data.len() ({})",
            offset + 8,
            header_data.len()
        );
    }

    // Continue tracing...
    offset += 8; // incoming_cluster_offset
    println!("After incoming_cluster_offset: offset = {}", offset);

    // Now offset should be 72, and we're about to read free_space_offset
    println!("About to read free_space_offset at offset {}", offset);
    println!("bytes[{}..{}] would be accessed", offset, offset + 8);
    println!(
        "bytes[{} + 7] = bytes[{}] would be accessed",
        offset,
        offset + 7
    );

    if offset + 8 >= header_data.len() {
        println!(
            "WARNING: offset + 8 ({}) >= header_data.len() ({})",
            offset + 8,
            header_data.len()
        );
    } else {
        println!(
            "OK: offset + 8 ({}) < header_data.len() ({})",
            offset + 8,
            header_data.len()
        );
    }

    // Continue tracing...
    offset += 8; // free_space_offset
    println!("After free_space_offset: offset = {}", offset);

    println!(
        "Final offset = {}, header_data.len() = {}",
        offset,
        header_data.len()
    );

    // Now try the actual decode
    println!("\n--- ACTUAL DECODE ATTEMPT ---");
    let result = decode_persistent_header(&header_data);

    match result {
        Ok(header) => {
            println!("SUCCESS: Header decoded successfully");
            println!(
                "Header: magic={:?}, version={}, schema_version={}",
                &header.magic, header.version, header.schema_version
            );
        }
        Err(e) => {
            println!("ERROR: Header decode failed: {:?}", e);
        }
    }
}
