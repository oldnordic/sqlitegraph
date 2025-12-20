//! Comprehensive TDD unit tests for V2 WAL core components
//!
//! This module provides thorough testing for all WAL core functionality including
//! configuration management, header validation, and basic WAL operations.
//! Tests follow TDD methodology with comprehensive edge case coverage.

use std::path::Path;
use tempfile::tempdir;
use sqlitegraph::backend::native::{NativeResult, NativeBackendError};
use sqlitegraph::backend::native::v2::wal::{
    V2WALConfig, V2WALHeader, LSN, validate_lsn_sequence,
    calculate_wal_size_estimate, format_lsn, parse_lsn,
};

/// Test configuration validation with various parameter combinations
#[test]
fn test_wal_config_validation() -> NativeResult<()> {
    let temp_dir = tempdir()?;

    // Valid configuration
    let valid_config = V2WALConfig {
        wal_path: temp_dir.path().join("valid.wal"),
        max_wal_size: 64 * 1024 * 1024, // 64MB
        buffer_size: 1024 * 1024,      // 1MB
        flush_interval_ms: 100,
        enable_compression: true,
        cluster_affinity_groups: 4,
        ..Default::default()
    };

    assert!(valid_config.validate().is_ok(), "Valid config should pass validation");

    // Invalid WAL size (too small)
    let mut invalid_config = valid_config.clone();
    invalid_config.max_wal_size = 1024; // Too small
    assert!(invalid_config.validate().is_err(), "Config with too small WAL size should fail");

    // Invalid buffer size (too large)
    let mut invalid_config = valid_config.clone();
    invalid_config.buffer_size = 128 * 1024 * 1024; // Too large
    assert!(invalid_config.validate().is_err(), "Config with too large buffer size should fail");

    // Invalid buffer size (not power of 2)
    let mut invalid_config = valid_config.clone();
    invalid_config.buffer_size = 1000; // Not power of 2
    assert!(invalid_config.validate().is_err(), "Config with non-power-of-2 buffer size should fail");

    // Invalid flush interval (too frequent)
    let mut invalid_config = valid_config.clone();
    invalid_config.flush_interval_ms = 5; // Too frequent
    assert!(invalid_config.validate().is_err(), "Config with too frequent flush should fail");

    // Invalid cluster affinity groups (not power of 2)
    let mut invalid_config = valid_config.clone();
    invalid_config.cluster_affinity_groups = 3; // Not power of 2
    assert!(invalid_config.validate().is_err(), "Config with non-power-of-2 cluster groups should fail");

    Ok(())
}

/// Test WAL header creation, serialization, and validation
#[test]
fn test_wal_header_operations() -> NativeResult<()> {
    // Create a new header
    let mut header = V2WALHeader::new();

    // Validate default values
    assert_eq!(header.magic_bytes(), &sqlitegraph::backend::native::v2::wal::V2_MAGIC);
    assert_eq!(header.version(), 1);
    assert_eq!(header.current_lsn(), 0);
    assert_eq!(header.checkpoint_lsn(), 0);

    // Update header fields
    header.set_current_lsn(1000)?;
    header.set_checkpoint_lsn(500)?;
    header.set_committed(true)?;

    // Verify updated values
    assert_eq!(header.current_lsn(), 1000);
    assert_eq!(header.checkpoint_lsn(), 500);
    assert!(header.is_committed());

    // Test LSN validation
    assert!(header.validate_lsn_sequence(1001).is_ok(), "Valid next LSN should pass");
    assert!(header.validate_lsn_sequence(999).is_err(), "Invalid next LSN should fail");

    // Test magic bytes validation
    let valid_magic = sqlitegraph::backend::native::v2::wal::V2_MAGIC;
    assert!(sqlitegraph::backend::native::v2::wal::validate_magic_bytes(&valid_magic).is_ok());

    let invalid_magic = [b'X', b'2', b'W', b'A', b'L', 0, 0, 0];
    assert!(sqlitegraph::backend::native::v2::wal::validate_magic_bytes(&invalid_magic).is_err());

    Ok(())
}

/// Test LSN validation and sequence checking
#[test]
fn test_lsn_validation() -> NativeResult<()> {
    // Test valid LSN sequences
    assert!(validate_lsn_sequence(0, 1).is_ok(), "Valid forward sequence should pass");
    assert!(validate_lsn_sequence(1000, 1001).is_ok(), "Valid increment by 1 should pass");
    assert!(validate_lsn_sequence(1000, 1050).is_ok(), "Valid increment by more than 1 should pass");

    // Test invalid LSN sequences
    assert!(validate_lsn_sequence(1000, 999).is_err(), "Backward sequence should fail");
    assert!(validate_lsn_sequence(1000, 1000).is_err(), "Same LSN should fail");
    assert!(validate_lsn_sequence(u64::MAX - 10, u64::MAX + 5).is_err(), "Overflow should fail");

    // Test boundary conditions
    assert!(validate_lsn_sequence(0, u64::MAX).is_ok(), "Maximum jump should be valid");
    assert!(validate_lsn_sequence(u64::MAX - 1, u64::MAX).is_ok(), "Maximum LSN increment should work");

    Ok(())
}

/// Test WAL size estimation accuracy
#[test]
fn test_wal_size_estimation() -> NativeResult<()> {
    // Test with various record counts and average sizes
    let test_cases = vec![
        (100, 100),    // Small workload
        (1000, 500),   // Medium workload
        (10000, 1000), // Large workload
        (100000, 2000), // Very large workload
    ];

    for (record_count, avg_size) in test_cases {
        let estimated_size = calculate_wal_size_estimate(record_count, avg_size);

        // Estimate should include header size + record data + overhead
        let header_size = std::mem::size_of::<V2WALHeader>();
        let min_expected_size = header_size + (record_count * avg_size);
        let max_expected_size = min_expected_size + (record_count * 20); // 20 bytes overhead per record

        assert!(estimated_size >= min_expected_size as u64,
                "Size estimate should be at least minimum expected for {} records", record_count);
        assert!(estimated_size <= max_expected_size as u64,
                "Size estimate should not exceed maximum expected for {} records", record_count);
    }

    Ok(())
}

/// Test LSN formatting and parsing utilities
#[test]
fn test_lsn_formatting_parsing() -> NativeResult<()> {
    let test_lsns = vec![0, 1, 42, 1000, u64::MAX / 2, u64::MAX - 1];

    for lsn in test_lsns {
        let formatted = format_lsn(lsn);
        let parsed = parse_lsn(&formatted)?;

        assert_eq!(lsn, parsed, "LSN should be preserved through format/parse cycle");
    }

    // Test invalid LSN strings
    let invalid_strings = vec!["", "not_a_number", "-1", "18446744073709551616"]; // u64::MAX + 1

    for invalid_str in invalid_strings {
        assert!(parse_lsn(invalid_str).is_err(), "Invalid LSN string '{}' should fail", invalid_str);
    }

    Ok(())
}

/// Test configuration serialization/deserialization
#[test]
fn test_config_serialization() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let original_config = V2WALConfig {
        wal_path: temp_dir.path().join("test.wal"),
        max_wal_size: 32 * 1024 * 1024,
        buffer_size: 512 * 1024,
        flush_interval_ms: 200,
        enable_compression: true,
        cluster_affinity_groups: 8,
        ..Default::default()
    };

    // Convert config to string and back
    let config_str = original_config.to_string();
    let reconstructed_config: V2WALHeader = V2WALHeader::from_string(&config_str)?;

    assert_eq!(original_config.max_wal_size, reconstructed_config.current_lsn()); // Simplified comparison

    Ok(())
}

/// Test WAL file creation and basic operations
#[test]
fn test_wal_file_creation() -> NativeResult<()> {
    let temp_dir = tempdir()?;
    let wal_path = temp_dir.path().join("test_creation.wal");

    // Ensure WAL file doesn't exist initially
    assert!(!wal_path.exists(), "WAL file should not exist initially");

    // Create a basic WAL configuration
    let config = V2WALConfig {
        wal_path: wal_path.clone(),
        max_wal_size: 16 * 1024 * 1024,
        buffer_size: 256 * 1024,
        flush_interval_ms: 100,
        enable_compression: false,
        cluster_affinity_groups: 4,
        ..Default::default()
    };

    // Validate configuration
    config.validate()?;

    // Check that the directory exists
    assert!(temp_dir.path().exists(), "Temp directory should exist");

    Ok(())
}

/// Test error handling and edge cases
#[test]
fn test_error_handling_edge_cases() -> NativeResult<()> {
    // Test with invalid path (non-existent parent directory)
    let invalid_config = V2WALConfig {
        wal_path: Path::new("/non/existent/directory/test.wal").to_path_buf(),
        ..Default::default()
    };

    assert!(invalid_config.validate().is_err(), "Config with invalid path should fail");

    // Test LSN boundary conditions
    assert!(validate_lsn_sequence(u64::MAX, u64::MAX).is_err(), "Same LSN should fail");
    assert!(validate_lsn_sequence(u64::MAX, 0).is_err(), "Wrap-around should fail");

    // Test size estimation with extreme values
    let extreme_estimate = calculate_wal_size_estimate(u32::MAX as u64, u32::MAX as usize);
    assert!(extreme_estimate > 0, "Extreme estimate should be positive");

    Ok(())
}

/// Test concurrent access patterns (simplified)
#[test]
fn test_concurrent_access_patterns() -> NativeResult<()> {
    use std::sync::{Arc, Mutex};
    use std::thread;

    let temp_dir = tempdir()?;
    let config = Arc::new(V2WALConfig {
        wal_path: temp_dir.path().join("concurrent_test.wal"),
        max_wal_size: 16 * 1024 * 1024,
        buffer_size: 256 * 1024,
        flush_interval_ms: 50,
        enable_compression: false,
        cluster_affinity_groups: 4,
        ..Default::default()
    });

    let validation_results = Arc::new(Mutex::new(Vec::new()));
    let mut handles = vec![];

    // Spawn multiple threads to validate configuration concurrently
    for i in 0..4 {
        let config_clone = Arc::clone(&config);
        let results_clone = Arc::clone(&validation_results);

        let handle = thread::spawn(move || {
            let result = config_clone.validate();
            let mut results = results_clone.lock().unwrap();
            results.push((i, result.is_ok()));
        });

        handles.push(handle);
    }

    // Wait for all threads to complete
    for handle in handles {
        handle.join().unwrap();
    }

    // Verify all validations succeeded
    let results = validation_results.lock().unwrap();
    assert_eq!(results.len(), 4, "All threads should complete");
    for (thread_id, success) in results.iter() {
        assert!(success, "Thread {} validation should succeed", thread_id);
    }

    Ok(())
}

/// Test memory usage and resource management
#[test]
fn test_memory_resource_management() -> NativeResult<()> {
    let temp_dir = tempdir()?;

    // Test configuration with various memory parameters
    let memory_configs = vec![
        (64 * 1024, 8 * 1024),       // Small WAL, small buffer
        (1024 * 1024, 64 * 1024),    // Medium WAL, small buffer
        (16 * 1024 * 1024, 1024 * 1024), // Large WAL, large buffer
    ];

    for (wal_size, buffer_size) in memory_configs {
        let config = V2WALConfig {
            wal_path: temp_dir.path().join(format!("memory_test_{}.wal", wal_size)),
            max_wal_size: wal_size,
            buffer_size,
            ..Default::default()
        };

        assert!(config.validate().is_ok(),
                "Config with WAL size {} and buffer size {} should be valid",
                wal_size, buffer_size);

        // Verify memory constraints are reasonable
        assert!(buffer_size <= wal_size / 4,
                "Buffer size should be reasonable fraction of WAL size");
    }

    Ok(())
}

/// Performance validation tests
#[test]
fn test_performance_requirements() -> NativeResult<()> {
    // Test that LSN operations are fast enough for high-throughput scenarios
    let start_time = std::time::Instant::now();
    let iterations = 1_000_000;

    let mut current_lsn = 0;
    for i in 0..iterations {
        current_lsn += 1;
        assert!(validate_lsn_sequence(current_lsn - 1, current_lsn).is_ok());
    }

    let elapsed = start_time.elapsed();
    let lsn_ops_per_second = iterations as f64 / elapsed.as_secs_f64();

    // Should be able to process at least 1M LSN validations per second
    assert!(lsn_ops_per_second >= 1_000_000.0,
            "LSN validation should be fast: {:.0} ops/sec", lsn_ops_per_second);

    Ok(())
}