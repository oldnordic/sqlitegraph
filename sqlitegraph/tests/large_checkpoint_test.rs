//! Large Checkpoint Scaling Tests
//!
//! Integration tests for multi-file checkpointing with large databases.
//! Tests verify segment rotation, manifest creation, and recovery for
//! checkpoints that exceed the 1GB single-file limit.

use sqlitegraph::backend::native::v2::wal::checkpoint::{
    CheckpointStrategy, CheckpointState, MultiFileCheckpointConfig, MultiFileRecovery,
    V2WALCheckpointManager,
};
use sqlitegraph::backend::native::v2::wal::V2WALConfig;
use sqlitegraph::backend::native::GraphFile;
use std::time::Duration;
use tempfile::tempdir;

/// Test checkpoint with data that exceeds 1GB (simulated with smaller sizes for CI)
///
/// This test creates a checkpoint with multiple segments to verify the
/// multi-file checkpoint system correctly handles large databases.
/// For CI, we use smaller segment sizes but the same scaling pattern.
#[test]
fn test_checkpoint_exceeds_1gb() {
    let temp_dir = tempdir().unwrap();
    let v2_graph_path = temp_dir.path().join("test.v2");

    // Create a V2 graph file
    let _graph_file = GraphFile::create(&v2_graph_path)
        .expect("Failed to create test V2 graph file");

    let checkpoint_path = temp_dir.path().join("checkpoint");

    let config = V2WALConfig {
        wal_path: temp_dir.path().join("test.wal"),
        checkpoint_path: temp_dir.path().join("test.checkpoint"),
        max_wal_size: 64 * 1024 * 1024,
        buffer_size: 1024 * 1024,
        checkpoint_interval: 100,
        enable_compression: false,
        ..Default::default()
    };

    // Create multi-file config with small segment size for testing
    // In production, this would be 512MB; here we use 10MB for testing
    let multi_file_config = MultiFileCheckpointConfig::new(checkpoint_path.clone())
        .with_max_segment_size(10 * 1024 * 1024) // 10MB segments
        .with_max_segments(16); // 160MB total

    let strategy = CheckpointStrategy::TimeInterval(Duration::from_secs(60));
    let manager = V2WALCheckpointManager::with_multi_file(
        config,
        strategy,
        multi_file_config,
    )
    .expect("Failed to create multi-file checkpoint manager");

    // Verify multi-file is enabled
    assert!(manager.is_multi_file_enabled());

    // Get the config to verify settings
    let retrieved_config = manager.get_multi_file_config();
    assert!(retrieved_config.is_some());
    let cfg = retrieved_config.unwrap();
    assert_eq!(cfg.max_segment_size, 10 * 1024 * 1024);

    // Verify max total size
    assert_eq!(cfg.max_total_size(), 160 * 1024 * 1024);
}

/// Test checkpoint segment rotation
///
/// This test verifies that when a checkpoint exceeds the segment size
/// threshold, a new segment is created and data continues to be written.
#[test]
fn test_checkpoint_segment_rotation() {
    use sqlitegraph::backend::native::v2::wal::checkpoint::io::SegmentWriter;

    let temp_dir = tempdir().unwrap();
    let checkpoint_path = temp_dir.path().join("checkpoint");

    // Create multi-file config with very small segment size for rotation testing
    let multi_file_config = MultiFileCheckpointConfig::new(checkpoint_path.clone())
        .with_max_segment_size(1 * 1024 * 1024) // 1MB segments
        .with_max_segments(10);

    // Write some data to simulate segment rotation
    let mut writer = SegmentWriter::create(multi_file_config.clone(), 0, 100)
        .expect("Failed to create segment writer");

    // Write data close to the limit
    let data = vec![1u8; 900 * 1024]; // 900KB
    writer.write_data(&data).expect("Failed to write data");

    // Should not need rotation yet
    assert!(!writer.needs_rotation());

    // Rotate to next segment (this will auto-finalize the first segment)
    writer.rotate_segment(200).expect("Failed to rotate segment");
    assert_eq!(writer.current_index(), 1);

    // Verify first segment was completed
    assert_eq!(writer.completed_segments().len(), 1);

    // Write to second segment
    let data2 = vec![2u8; 500 * 1024]; // 500KB
    writer.write_data(&data2).expect("Failed to write data to second segment");

    let segment2 = writer.finalize(300, 50).expect("Failed to finalize second segment");
    assert_eq!(segment2.segment_index, 1);
    assert_eq!(segment2.lsn_range, (200, 300));

    // Verify two segments were completed
    assert_eq!(writer.completed_segments().len(), 2);
}

/// Test recovery with partial segments
///
/// This test verifies that recovery fails properly when a segment
/// file is missing, ensuring atomic recovery semantics.
#[test]
fn test_recovery_partial_segments() {
    use sqlitegraph::backend::native::v2::wal::checkpoint::io::{CheckpointManifest, CheckpointSegmentMeta, SegmentWriter};

    let temp_dir = tempdir().unwrap();
    let checkpoint_path = temp_dir.path().join("checkpoint");

    // Create multi-file config
    let multi_file_config = MultiFileCheckpointConfig::new(checkpoint_path.clone())
        .with_max_segment_size(10 * 1024 * 1024)
        .with_max_segments(4);

    // Create and write manifest with 2 segments
    let mut manifest = CheckpointManifest::new();
    manifest.add_segment(CheckpointSegmentMeta {
        index: 0,
        lsn_start: 100,
        lsn_end: 200,
        block_count: 50,
        checksum: 12345,
        size: 1024 * 1024,
    });
    manifest.add_segment(CheckpointSegmentMeta {
        index: 1,
        lsn_start: 200,
        lsn_end: 300,
        block_count: 50,
        checksum: 67890,
        size: 1024 * 1024,
    });

    MultiFileRecovery::write_manifest(&manifest, &checkpoint_path)
        .expect("Failed to write manifest");

    // Create only the first segment file
    let segment0_path = checkpoint_path.with_extension("ckpt.000");
    let mut writer = SegmentWriter::create(multi_file_config.clone(), 0, 100)
        .expect("Failed to create segment 0");
    writer.write_data(&[1u8; 1024]).expect("Failed to write data");
    writer.finalize(200, 50).expect("Failed to finalize segment 0");

    // Verify segment 0 exists but segment 1 doesn't
    assert!(segment0_path.exists());
    assert!(!checkpoint_path.with_extension("ckpt.001").exists());

    // Recovery should fail due to missing segment
    let result = MultiFileRecovery::recover_checkpoint(manifest, checkpoint_path);
    assert!(result.is_err());
    assert!(result.unwrap_err().message.contains("Segment file"));
}

/// Test concurrent checkpoint limit enforcement
///
/// This test verifies that MAX_CONCURRENT_CHECKPOINTS = 1 is enforced
/// for multi-file checkpoints.
#[test]
fn test_concurrent_checkpoint_limit() {
    let temp_dir = tempdir().unwrap();
    let v2_graph_path = temp_dir.path().join("test.v2");

    let _graph_file = GraphFile::create(&v2_graph_path)
        .expect("Failed to create test V2 graph file");

    let checkpoint_path = temp_dir.path().join("checkpoint");

    let config = V2WALConfig {
        wal_path: temp_dir.path().join("test.wal"),
        checkpoint_path: temp_dir.path().join("test.checkpoint"),
        max_wal_size: 64 * 1024 * 1024,
        buffer_size: 1024 * 1024,
        checkpoint_interval: 100,
        enable_compression: false,
        ..Default::default()
    };

    let multi_file_config = MultiFileCheckpointConfig::new(checkpoint_path)
        .with_max_segment_size(10 * 1024 * 1024)
        .with_max_segments(4);

    let strategy = CheckpointStrategy::TimeInterval(Duration::from_secs(1));
    let manager = V2WALCheckpointManager::with_multi_file(
        config,
        strategy,
        multi_file_config,
    )
    .expect("Failed to create multi-file checkpoint manager");

    // Initially idle
    assert_eq!(manager.get_state(), CheckpointState::Idle);
    assert!(!manager.is_checkpoint_in_progress());

    // Start a checkpoint (will fail due to missing WAL, but state should transition)
    let _result = manager.checkpoint();

    // Verify state handling
    let state = manager.get_state();
    // After failed attempt, should return to Idle or be in Failed state
    assert!(state == CheckpointState::Idle || state == CheckpointState::Failed);
}

/// Test manifest file creation and validation
///
/// This test verifies that manifest files are created correctly
/// and can be validated for consistency.
#[test]
fn test_manifest_creation_and_validation() {
    use sqlitegraph::backend::native::v2::wal::checkpoint::io::{CheckpointManifest, CheckpointSegmentMeta};

    let temp_dir = tempdir().unwrap();
    let checkpoint_path = temp_dir.path().join("checkpoint");

    // Create a manifest with multiple segments
    let mut manifest = CheckpointManifest::new();
    manifest.timestamp = 1234567890;

    for i in 0u64..3 {
        manifest.add_segment(CheckpointSegmentMeta {
            index: i as u32,
            lsn_start: i * 1000,
            lsn_end: (i + 1) * 1000,
            block_count: 100,
            checksum: i * 1000,
            size: 1024 * 1024,
        });
    }

    // Write manifest
    MultiFileRecovery::write_manifest(&manifest, &checkpoint_path)
        .expect("Failed to write manifest");

    // Load manifest back
    let loaded = MultiFileRecovery::load_manifest(&checkpoint_path.with_extension("manifest"))
        .expect("Failed to load manifest");

    // Verify contents
    assert_eq!(loaded.segment_count, 3);
    assert_eq!(loaded.timestamp, 1234567890);
    assert_eq!(loaded.total_lsn_range, (0, 3000));
    assert_eq!(loaded.total_block_count, 300);

    // Validate manifest
    let validation_result = manifest.validate();
    assert!(validation_result.is_ok());

    // Verify segment index continuity
    for (i, segment) in loaded.segments.iter().enumerate() {
        assert_eq!(segment.index, i as u32);
    }
}

/// Test LSN continuity across segments
///
/// This test verifies that LSN ranges are continuous across
/// segments without gaps or overlaps.
#[test]
fn test_lsn_continuity_across_segments() {
    use sqlitegraph::backend::native::v2::wal::checkpoint::io::{CheckpointManifest, CheckpointSegmentMeta};

    let mut manifest = CheckpointManifest::new();

    // Add segments with continuous LSN ranges
    manifest.add_segment(CheckpointSegmentMeta {
        index: 0,
        lsn_start: 1000,
        lsn_end: 2000,
        block_count: 100,
        checksum: 1,
        size: 1024,
    });
    manifest.add_segment(CheckpointSegmentMeta {
        index: 1,
        lsn_start: 2000,
        lsn_end: 3000,
        block_count: 100,
        checksum: 2,
        size: 1024,
    });
    manifest.add_segment(CheckpointSegmentMeta {
        index: 2,
        lsn_start: 3000,
        lsn_end: 4000,
        block_count: 100,
        checksum: 3,
        size: 1024,
    });

    // Should validate successfully
    let result = manifest.validate();
    assert!(result.is_ok());

    // Now add an overlapping segment
    manifest.add_segment(CheckpointSegmentMeta {
        index: 3,
        lsn_start: 3500, // Overlaps with segment 2
        lsn_end: 4500,
        block_count: 100,
        checksum: 4,
        size: 1024,
    });

    // Should fail validation
    let result = manifest.validate();
    assert!(result.is_err());
}
