//! GraphFile I/O Invariant Regression Tests
//!
//! This test ensures the file-size invariant prevents "failed to fill whole buffer" errors
//! by validating file size during GraphFile::open() operations.
//!
//! Updated to align with current GraphFile API behavior where validation occurs at open time.

use sqlitegraph::backend::native::{GraphFile, NativeBackendError};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_read_bytes_direct_file_size_invariant() -> Result<(), Box<dyn std::error::Error>> {
    // Test 1: GraphFile::open() fails on tiny file with FileTooSmall error
    let mut temp_file = NamedTempFile::new()?;
    let test_data = b"Hello, World!"; // 13 bytes
    temp_file.write_all(test_data)?;
    temp_file.flush()?;

    // Opening tiny file should fail with FileTooSmall error during validation
    let result = GraphFile::open(temp_file.path());
    assert!(result.is_err(), "Opening tiny file should fail");

    if let Err(error) = result {
        match error {
            NativeBackendError::FileTooSmall { size, min_size } => {
                assert_eq!(size, 13, "Should report actual file size");
                assert_eq!(min_size, 80, "Should report minimum required size (HEADER_SIZE)");
            }
            other => panic!(
                "Expected FileTooSmall error, got: {:?}",
                other
            ),
        }
    }

    // Test 2: Create properly sized file with GraphFile::create()
    let temp_file = NamedTempFile::new()?;
    let graph_file = GraphFile::create(temp_file.path())?;
    assert!(graph_file.file_size()? >= 80, "Created file should meet minimum size requirements");

    // Test 3: Valid reads should succeed on properly created file
    let mut graph_file = GraphFile::open(temp_file.path())?;
    let mut buffer = vec![0u8; 10];
    let result = graph_file.read_bytes(0, &mut buffer);
    assert!(result.is_ok(), "Valid read on proper file should succeed: {:?}", result);

    // Test 4: Reading at valid offset should succeed
    let result = graph_file.read_bytes(40, &mut buffer);
    assert!(result.is_ok(), "Reading at valid offset should succeed");

    Ok(())
}

#[test]
fn test_read_header_file_size_invariant() -> Result<(), Box<dyn std::error::Error>> {
    // Create a file smaller than header size (0 bytes)
    let temp_file = NamedTempFile::new()?;

    // Opening empty file should fail with FileTooSmall error during header validation
    let result = GraphFile::open(temp_file.path());
    assert!(result.is_err(), "Opening empty file should fail");

    if let Err(error) = result {
        match error {
            NativeBackendError::FileTooSmall { size, min_size } => {
                assert_eq!(size, 0, "Should report actual file size as 0");
                assert_eq!(min_size, 80, "Should report minimum required size (HEADER_SIZE)");
            }
            other => panic!(
                "Expected FileTooSmall error for empty file, got: {:?}",
                other
            ),
        }
    }

    Ok(())
}

#[test]
fn test_read_edge_at_offset_file_size_invariant() -> Result<(), Box<dyn std::error::Error>> {
    // Create a properly sized file with GraphFile::create()
    let temp_file = NamedTempFile::new()?;

    // Open with GraphFile
    let mut graph_file = GraphFile::create(temp_file.path())?;

    // Test reading edge beyond file size - should return error gracefully
    let result = graph_file.read_edge_at_offset(1000000); // 1MB offset
    assert!(
        result.is_err(),
        "Reading edge beyond file should return error"
    );

    // Verify the error type is appropriate for reading beyond file bounds
    if let Err(error) = result {
        match error {
            NativeBackendError::Io(_) |
            NativeBackendError::CorruptNodeRecord { .. } |
            NativeBackendError::InvalidHeader { .. } => {
                // Expected error types for reading beyond file bounds
            }
            other => panic!(
                "Expected I/O, CorruptNodeRecord, or InvalidHeader error for reading beyond file, got: {:?}",
                other
            ),
        }
    }

    Ok(())
}

#[test]
fn test_detailed_error_message_format() -> Result<(), Box<dyn std::error::Error>> {
    // Test that FileTooSmall error messages contain all necessary details
    let mut temp_file = NamedTempFile::new()?;

    // Write exactly 50 bytes
    temp_file.as_file().write_all(&vec![0u8; 50])?;
    temp_file.flush()?;

    // Opening file should fail with FileTooSmall error
    let result = GraphFile::open(temp_file.path());
    assert!(result.is_err());

    if let Err(error) = result {
        let error_msg = format!("{}", error);

        // FileTooSmall errors should contain size information
        // Note: The exact format may vary, but should contain size details
        assert!(error_msg.contains("50") || error_msg.contains("size"),
               "Error should mention file size: {}", error_msg);
    }

    Ok(())
}

#[test]
fn test_invariant_prevents_failed_to_fill_whole_buffer() -> Result<(), Box<dyn std::error::Error>> {
    // This test specifically ensures we never get "failed to fill whole buffer"
    // because validation happens at open time, not during read operations

    let temp_file = NamedTempFile::new()?;

    // Opening empty file should fail with FileTooSmall, preventing any read operations
    let result = GraphFile::open(temp_file.path());
    assert!(result.is_err());

    if let Err(error) = result {
        let error_str = error.to_string();

        // Verify we get FileTooSmall error, not "failed to fill whole buffer"
        assert!(error_str.contains("File too small"));
        assert!(error_str.contains("0 bytes"));
        assert!(error_str.contains("80 bytes"));
        assert!(!error_str.contains("failed to fill whole buffer"));
    }

    Ok(())
}
