//! Test bounds checking in header decode
//!
//! This test specifically verifies that our bounds checking works correctly
//! and prevents the out-of-bounds panic.

use sqlitegraph::backend::native::{decode_persistent_header, get_slice_safe};
use sqlitegraph::backend::native::types::NativeBackendError;

#[test]
fn test_get_slice_safe_bounds_checking() {
    // Test the get_slice_safe function directly

    // Valid case
    let data = vec![1, 2, 3, 4, 5];
    let result = get_slice_safe(&data, 1, 3);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), &[2, 3, 4]);

    // Out of bounds - start + len > data.len()
    let result = get_slice_safe(&data, 3, 3); // 3 + 3 = 6 > 5
    assert!(result.is_err());
    match result.unwrap_err() {
        NativeBackendError::InvalidHeader { field, reason } => {
            assert_eq!(field, "header_data");
            assert!(reason.contains("slice access out of bounds"));
            assert!(reason.contains("start=3"));
            assert!(reason.contains("len=3"));
            assert!(reason.contains("data_len=5"));
        }
        other => panic!("Expected InvalidHeader error, got: {:?}", other),
    }

    // Edge case - exactly at boundary
    let result = get_slice_safe(&data, 2, 3); // 2 + 3 = 5 == data.len()
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), &[3, 4, 5]);

    // Edge case - start beyond data
    let result = get_slice_safe(&data, 5, 1); // start = 5 >= data.len()
    assert!(result.is_err());
}

#[test]
fn test_decode_header_with_80_bytes() {
    // Test that exactly 80 bytes works without panicking
    let mut header_data = vec![0u8; 80];

    // Set minimal valid data
    header_data[0..8].copy_from_slice(b"SQLTGRPH");
    header_data[8..12].copy_from_slice(&2u32.to_be_bytes());

    let result = decode_persistent_header(&header_data);

    // Should succeed without panic (even if it fails with validation error)
    match result {
        Ok(_) => println!("Header decoded successfully"),
        Err(e) => {
            println!("Header decode failed with error (acceptable): {:?}", e);
            // Accept certain error types that don't indicate panic
            match e {
                NativeBackendError::InvalidHeader { .. } => {
                    // This is acceptable - bounds checking worked
                }
                other => {
                    println!("Error type: {:?}", other);
                    // Other errors might also be acceptable depending on validation
                }
            }
        }
    }
}

#[test]
fn test_decode_header_with_too_few_bytes() {
    // Test that less than 80 bytes returns FileTooSmall error without panicking
    let header_data = vec![0u8; 79];

    let result = decode_persistent_header(&header_data);
    assert!(result.is_err());

    match result.unwrap_err() {
        NativeBackendError::FileTooSmall { size, min_size } => {
            assert_eq!(size, 79);
            assert_eq!(min_size, 80);
        }
        other => panic!("Expected FileTooSmall error, got: {:?}", other),
    }
}

#[test]
fn test_no_panic_on_any_input() {
    // Test that decode_persistent_header never panics, regardless of input

    let test_cases = vec![
        vec![],                           // Empty
        vec![0u8; 10],                   // Too small
        vec![0u8; 79],                   // One byte too small
        vec![0u8; 80],                   // Exactly right size
        vec![0u8; 81],                   // One byte too large
        vec![0xFF; 100],                 // Large with non-zero data
    ];

    for (i, test_data) in test_cases.iter().enumerate() {
        println!("Test case {}: {} bytes", i, test_data.len());

        let result = std::panic::catch_unwind(|| {
            decode_persistent_header(test_data)
        });

        match result {
            Ok(Ok(_)) => {
                println!("  → Success");
            }
            Ok(Err(e)) => {
                println!("  → Error (acceptable): {:?}", e);
            }
            Err(panic_info) => {
                panic!("PANIC in test case {} with {} bytes: {:?}",
                      i, test_data.len(), panic_info);
            }
        }
    }
}