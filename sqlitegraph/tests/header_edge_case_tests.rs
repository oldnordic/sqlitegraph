//! Manual tests for edge cases around header bounds checking

use sqlitegraph::backend::native::{decode_persistent_header, get_slice_safe};

#[test]
fn test_edge_cases_around_80_bytes() {
    println!("Testing edge cases for header bounds checking...");

    // Test 1: Exactly 80 bytes (the boundary case that was causing panics)
    println!("\n=== Test 1: Exactly 80 bytes ===");
    let mut header_80 = vec![0u8; 80];
    header_80[0..8].copy_from_slice(b"SQLTGRPH");
    header_80[8..12].copy_from_slice(&2u32.to_be_bytes());

    let result = decode_persistent_header(&header_80);
    match result {
        Ok(header) => println!("✅ SUCCESS: 80 bytes decoded successfully - version: {}", header.version),
        Err(e) => println!("✅ EXPECTED: 80 bytes returned error (not panic): {:?}", e),
    }

    // Test 2: 79 bytes (one byte short)
    println!("\n=== Test 2: 79 bytes ===");
    let header_79 = vec![0u8; 79];
    let result = decode_persistent_header(&header_79);
    assert!(result.is_err(), "79 bytes should return an error");
    println!("✅ EXPECTED: 79 bytes returned error");

    // Test 3: 81 bytes (one byte extra)
    println!("\n=== Test 3: 81 bytes ===");
    let header_81 = vec![0u8; 81];
    let result = decode_persistent_header(&header_81);
    match result {
        Ok(header) => println!("✅ SUCCESS: 81 bytes decoded successfully - version: {}", header.version),
        Err(e) => println!("✅ EXPECTED: 81 bytes returned error: {:?}", e),
    }

    // Test 4: Much larger header (200 bytes)
    println!("\n=== Test 4: 200 bytes ===");
    let header_200 = vec![0u8; 200];
    let result = decode_persistent_header(&header_200);
    match result {
        Ok(header) => println!("✅ SUCCESS: 200 bytes decoded successfully - version: {}", header.version),
        Err(e) => println!("✅ EXPECTED: 200 bytes returned error: {:?}", e),
    }

    println!("\n🎉 All edge case tests completed successfully without panics!");
}

#[test]
fn test_get_slice_safe_edge_cases() {
    println!("Testing get_slice_safe edge cases...");

    let data = vec![1, 2, 3, 4, 5];

    // Valid case
    match get_slice_safe(&data, 1, 3) {
        Ok(slice) => {
            println!("✅ Valid slice: {:?}", slice);
            assert_eq!(slice, &[2, 3, 4]);
        }
        Err(e) => panic!("Valid slice failed: {:?}", e),
    }

    // Boundary case - exactly at the end
    match get_slice_safe(&data, 2, 3) {
        Ok(slice) => {
            println!("✅ Boundary slice: {:?}", slice);
            assert_eq!(slice, &[3, 4, 5]);
        }
        Err(e) => panic!("Boundary slice failed: {:?}", e),
    }

    // Out of bounds case
    match get_slice_safe(&data, 3, 3) {
        Ok(_) => panic!("Out of bounds should not succeed"),
        Err(e) => {
            println!("✅ Out of bounds correctly rejected: {:?}", e);
            assert!(format!("{:?}", e).contains("slice access out of bounds"));
        }
    }

    // Start at end
    match get_slice_safe(&data, 5, 1) {
        Ok(_) => panic!("Start at end should not succeed"),
        Err(_) => println!("✅ Start at end correctly rejected"),
    }

    // Zero length
    match get_slice_safe(&data, 2, 0) {
        Ok(slice) => {
            println!("✅ Zero length slice: {:?}", slice);
            assert_eq!(slice.len(), 0);
        }
        Err(e) => panic!("Zero length should succeed: {:?}", e),
    }
}

#[test]
fn test_no_panic_on_corrupted_headers() {
    // Test that we never panic even with completely corrupted data

    let corrupted_cases = vec![
        vec![], // Empty
        vec![0u8; 10], // Too small
        vec![0xFF; 79], // One byte too small, corrupted
        vec![0xFF; 80], // Exactly right size, corrupted
        vec![0xFF; 81], // One byte too large, corrupted
        vec![0xAA; 100], // Larger, corrupted
    ];

    for (i, test_data) in corrupted_cases.iter().enumerate() {
        println!("Testing case {} with {} bytes", i, test_data.len());

        let result = std::panic::catch_unwind(|| {
            decode_persistent_header(test_data)
        });

        match result {
            Ok(Ok(_header)) => {
                println!("  ✅ Success: header decoded without panic");
            }
            Ok(Err(e)) => {
                println!("  ✅ Expected error (not panic): {:?}", e);
            }
            Err(panic_info) => {
                let panic_msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                    s.to_string()
                } else {
                    "Unknown panic".to_string()
                };

                panic!("❌ PANIC detected in case {} with {} bytes: {}",
                       i, test_data.len(), panic_msg);
            }
        }
    }
}