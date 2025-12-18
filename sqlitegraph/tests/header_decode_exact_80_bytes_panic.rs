//! Direct reproduction test for the index out of bounds panic
//! Target: len=80, index=80 in header decode

use sqlitegraph::backend::native::decode_persistent_header;
use sqlitegraph::backend::native::persistent_header::PERSISTENT_HEADER_SIZE;

#[test]
fn test_exact_80_bytes_causes_oob_panic() {
    // Create a buffer with exactly 80 bytes (PERSISTENT_HEADER_SIZE)
    let mut header_data = vec![0u8; PERSISTENT_HEADER_SIZE];

    // This test is designed to trigger the panic we're trying to fix
    // We'll create a buffer of exactly 80 bytes and see if decode_persistent_header
    // tries to access beyond index 79 (which would be index 80 == len)

    // Set some basic valid data
    header_data[0..8].copy_from_slice(b"SQLTGRPH");  // magic
    header_data[8..12].copy_from_slice(&2u32.to_be_bytes());  // version

    println!("Created header buffer of {} bytes", header_data.len());

    // This should panic if the bug exists: "index out of bounds: (len=80, index=80)"
    let result = std::panic::catch_unwind(|| {
        decode_persistent_header(&header_data)
    });

    match result {
        Ok(Ok(_header)) => {
            println!("SUCCESS: No panic occurred, header decoded successfully");
        }
        Ok(Err(e)) => {
            println!("ERROR: Header decode failed with error (not panic): {:?}", e);
            // This is acceptable - the function should return an error, not panic
        }
        Err(panic_info) => {
            let panic_msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                s.clone()
            } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                s.to_string()
            } else {
                "Unknown panic".to_string()
            };

            if panic_msg.contains("index out of bounds") && panic_msg.contains("len=80") {
                panic!("CONFIRMED BUG: Found the exact panic we need to fix: {}", panic_msg);
            } else {
                panic!("Different panic occurred: {}", panic_msg);
            }
        }
    }
}

#[test]
fn test_79_bytes_should_not_panic() {
    // Test with 79 bytes - should return FileTooSmall error, not panic
    let header_data = vec![0u8; PERSISTENT_HEADER_SIZE - 1];

    let result = std::panic::catch_unwind(|| {
        decode_persistent_header(&header_data)
    });

    assert!(result.is_ok(), "Should not panic with 79 bytes");

    match result.unwrap() {
        Ok(_) => panic!("Should not succeed with only 79 bytes"),
        Err(_) => {
            // Expected - should return FileTooSmall error
        }
    }
}