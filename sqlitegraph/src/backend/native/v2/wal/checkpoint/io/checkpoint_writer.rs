//! Checkpoint Writer for Checkpoint File Operations
//!
//! This module provides checkpoint file writing operations including headers,
//! progress tracking, and completion markers.

use crate::backend::native::v2::wal::checkpoint::core::CheckpointProgress;
use crate::backend::native::v2::wal::checkpoint::errors::{CheckpointError, CheckpointResult};
use crate::backend::native::v2::wal::checkpoint::constants::*;
use std::io::{Seek, SeekFrom, Write};

/// Checkpoint file writer for structured checkpoint file operations
pub struct CheckpointWriter;

impl CheckpointWriter {
    /// Write checkpoint header to checkpoint file
    pub fn write_header<W: Write + Seek>(
        writer: &mut W,
        lsn_range: (u64, u64),
        timestamp: u64,
        block_count: u64,
    ) -> CheckpointResult<()> {
        // Write checkpoint magic number
        writer
            .write_all(CHECKPOINT_MAGIC)
            .map_err(|e| CheckpointError::io(format!("Failed to write checkpoint magic: {}", e)))?;

        // Write checkpoint version
        writer
            .write_all(&CHECKPOINT_VERSION.to_le_bytes())
            .map_err(|e| {
                CheckpointError::io(format!("Failed to write checkpoint version: {}", e))
            })?;

        // Write LSN range
        writer
            .write_all(&lsn_range.0.to_le_bytes())
            .map_err(|e| {
                CheckpointError::io(format!("Failed to write checkpoint start LSN: {}", e))
            })?;

        writer
            .write_all(&lsn_range.1.to_le_bytes())
            .map_err(|e| {
                CheckpointError::io(format!("Failed to write checkpoint end LSN: {}", e))
            })?;

        // Write timestamp
        writer
            .write_all(&timestamp.to_le_bytes())
            .map_err(|e| {
                CheckpointError::io(format!("Failed to write checkpoint timestamp: {}", e))
            })?;

        // Write block count
        writer
            .write_all(&block_count.to_le_bytes())
            .map_err(|e| {
                CheckpointError::io(format!("Failed to write checkpoint block count: {}", e))
            })?;

        // Write V2-specific metadata
        Self::write_v2_metadata(writer)?;

        Ok(())
    }

    /// Write V2-specific checkpoint metadata
    fn write_v2_metadata<W: Write + Seek>(writer: &mut W) -> CheckpointResult<()> {
        let metadata_start = writer
            .stream_position()
            .map_err(|e| CheckpointError::io(format!("Failed to get metadata position: {}", e)))?;

        // Write V2 checkpoint metadata header
        let v2_version = 2u32; // V2 format version
        writer
            .write_all(&v2_version.to_le_bytes())
            .map_err(|e| CheckpointError::io(format!("Failed to write V2 version: {}", e)))?;

        // Write V2-specific configuration
        writer
            .write_all(&v2::V2_GRAPH_BLOCK_SIZE.to_le_bytes())
            .map_err(|e| CheckpointError::io(format!("Failed to write V2 block size: {}", e)))?;

        writer
            .write_all(&v2::V2_CLUSTER_ALIGNMENT.to_le_bytes())
            .map_err(|e| {
                CheckpointError::io(format!("Failed to write V2 cluster alignment: {}", e))
            })?;

        // Write metadata length placeholder
        let metadata_length_pos = writer.stream_position().map_err(|e| {
            CheckpointError::io(format!("Failed to get metadata length position: {}", e))
        })?;
        writer.write_all(&0u32.to_le_bytes()).map_err(|e| {
            CheckpointError::io(format!(
                "Failed to write metadata length placeholder: {}",
                e
            ))
        })?;

        // Write additional V2 metadata here in future implementations
        // For now, we write an empty metadata section

        let metadata_end = writer.stream_position().map_err(|e| {
            CheckpointError::io(format!("Failed to get metadata end position: {}", e))
        })?;
        let metadata_length = (metadata_end - metadata_start - 4) as u32;

        // Seek back and write actual metadata length
        writer
            .seek(SeekFrom::Start(metadata_length_pos))
            .map_err(|e| {
                CheckpointError::io(format!("Failed to seek to metadata length: {}", e))
            })?;
        writer
            .write_all(&metadata_length.to_le_bytes())
            .map_err(|e| CheckpointError::io(format!("Failed to write metadata length: {}", e)))?;
        writer.seek(SeekFrom::Start(metadata_end)).map_err(|e| {
            CheckpointError::io(format!("Failed to seek back to metadata end: {}", e))
        })?;

        Ok(())
    }

    /// Write checkpoint progress record
    pub fn write_progress<W: Write>(
        writer: &mut W,
        progress: &CheckpointProgress,
    ) -> CheckpointResult<()> {
        // Write progress magic number
        writer
            .write_all(PROGRESS_MAGIC)
            .map_err(|e| CheckpointError::io(format!("Failed to write progress magic: {}", e)))?;

        // Write progress timestamp
        let elapsed = progress.checkpoint_start.elapsed().as_secs();
        writer
            .write_all(&elapsed.to_le_bytes())
            .map_err(|e| CheckpointError::io(format!("Failed to write progress timestamp: {}", e)))?;

        // Write completion percentage
        writer
            .write_all(&(progress.completion_percentage as u32).to_le_bytes())
            .map_err(|e| CheckpointError::io(format!("Failed to write completion percentage: {}", e)))?;

        // Write processed records
        writer
            .write_all(&progress.processed_records.to_le_bytes())
            .map_err(|e| CheckpointError::io(format!("Failed to write processed records: {}", e)))?;

        // Write flushed blocks
        writer
            .write_all(&progress.flushed_blocks.to_le_bytes())
            .map_err(|e| CheckpointError::io(format!("Failed to write flushed blocks: {}", e)))?;

        Ok(())
    }

    /// Write checkpoint completion marker
    pub fn write_completion<W: Write>(
        writer: &mut W,
        progress: &CheckpointProgress,
    ) -> CheckpointResult<()> {
        // Write completion magic number
        writer
            .write_all(COMPLETION_MAGIC)
            .map_err(|e| CheckpointError::io(format!("Failed to write completion magic: {}", e)))?;

        // Write final statistics
        let elapsed = progress.checkpoint_start.elapsed().as_secs();
        writer
            .write_all(&elapsed.to_le_bytes())
            .map_err(|e| CheckpointError::io(format!("Failed to write completion timestamp: {}", e)))?;

        writer
            .write_all(&progress.processed_records.to_le_bytes())
            .map_err(|e| CheckpointError::io(format!("Failed to write final processed records: {}", e)))?;

        writer
            .write_all(&progress.flushed_blocks.to_le_bytes())
            .map_err(|e| CheckpointError::io(format!("Failed to write final flushed blocks: {}", e)))?;

        // Write completion status (100% = success)
        writer
            .write_all(&100u32.to_le_bytes())
            .map_err(|e| CheckpointError::io(format!("Failed to write completion status: {}", e)))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_checkpoint_header_writing() -> CheckpointResult<()> {
        let mut buffer = Vec::new();
        let mut writer = Cursor::new(&mut buffer);

        let lsn_range = (1000, 2000);
        let timestamp = 1234567890;
        let block_count = 42;

        CheckpointWriter::write_header(&mut writer, lsn_range, timestamp, block_count)?;

        // Verify the buffer has content
        assert!(!buffer.is_empty());
        assert!(buffer.len() > 50); // Basic size check

        Ok(())
    }

    #[test]
    fn test_progress_writing() -> CheckpointResult<()> {
        let mut buffer = Vec::new();
        let mut writer = Cursor::new(&mut buffer);

        let progress = CheckpointProgress {
            lsn_range: (1000, 2000),
            total_records: 100,
            processed_records: 50,
            flushed_blocks: 25,
            completion_percentage: 50.0,
            checkpoint_start: std::time::Instant::now(),
        };

        CheckpointWriter::write_progress(&mut writer, &progress)?;

        // Verify the buffer has content
        assert!(!buffer.is_empty());
        assert!(buffer.len() > 20); // Basic size check

        Ok(())
    }

    #[test]
    fn test_completion_writing() -> CheckpointResult<()> {
        let mut buffer = Vec::new();
        let mut writer = Cursor::new(&mut buffer);

        let progress = CheckpointProgress {
            lsn_range: (1000, 2000),
            total_records: 100,
            processed_records: 100,
            flushed_blocks: 50,
            completion_percentage: 100.0,
            checkpoint_start: std::time::Instant::now(),
        };

        CheckpointWriter::write_completion(&mut writer, &progress)?;

        // Verify the buffer has content
        assert!(!buffer.is_empty());
        assert!(buffer.len() > 20); // Basic size check

        Ok(())
    }
}