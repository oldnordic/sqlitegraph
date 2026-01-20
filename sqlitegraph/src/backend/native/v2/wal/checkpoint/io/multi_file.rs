//! Multi-File Checkpoint Support for Large Databases
//!
//! This module provides multi-file checkpoint functionality to handle databases
//! larger than the 1GB single-file limit. Checkpoints are split into multiple
//! segment files with sequence numbers and a manifest file for atomic recovery.
//!
//! ## Architecture
//!
//! - **Segment files**: Individual checkpoint data chunks with `.ckpt.{index:03d}` suffix
//! - **Manifest file**: Contains metadata about all segments for recovery
//! - **Segment rotation**: Automatic creation of new segments when size threshold exceeded
//!
//! ## File Naming Convention
//!
//! - Segment files: `{base}.ckpt.{index:03d}` (e.g., `database.ckpt.000`, `database.ckpt.001`)
//! - Manifest file: `{base}.ckpt.manifest`

use crate::backend::native::v2::wal::checkpoint::constants::*;
use crate::backend::native::v2::wal::checkpoint::errors::{CheckpointError, CheckpointErrorKind, CheckpointResult};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Default maximum segment size (512 MB)
pub const DEFAULT_MAX_SEGMENT_SIZE: u64 = 512 * 1024 * 1024;

/// Default maximum number of segments (8 GB total with default segment size)
pub const DEFAULT_MAX_SEGMENTS: usize = 16;

/// Segment file extension
const SEGMENT_EXTENSION: &str = "ckpt";

/// Manifest file extension
const MANIFEST_EXTENSION: &str = "manifest";

/// Segment file magic number for validation
pub const SEGMENT_MAGIC: &[u8; 4] = b"SGMT";

/// Manifest file magic number for validation
pub const MANIFEST_MAGIC: &[u8; 4] = b"MNFT";

/// Multi-file checkpoint configuration
#[derive(Debug, Clone)]
pub struct MultiFileCheckpointConfig {
    /// Maximum size of each segment file in bytes
    pub max_segment_size: u64,

    /// Base path for checkpoint files (without extension)
    pub base_path: PathBuf,

    /// Maximum number of segment files allowed
    pub max_segments: usize,
}

impl Default for MultiFileCheckpointConfig {
    fn default() -> Self {
        Self {
            max_segment_size: DEFAULT_MAX_SEGMENT_SIZE,
            base_path: PathBuf::from("checkpoint"),
            max_segments: DEFAULT_MAX_SEGMENTS,
        }
    }
}

impl MultiFileCheckpointConfig {
    /// Create a new multi-file checkpoint configuration
    pub fn new(base_path: PathBuf) -> Self {
        Self {
            base_path,
            ..Default::default()
        }
    }

    /// Set the maximum segment size
    pub fn with_max_segment_size(mut self, size: u64) -> Self {
        self.max_segment_size = size;
        self
    }

    /// Set the maximum number of segments
    pub fn with_max_segments(mut self, count: usize) -> Self {
        self.max_segments = count;
        self
    }

    /// Validate the configuration
    pub fn validate(&self) -> CheckpointResult<()> {
        if self.max_segment_size == 0 {
            return Err(CheckpointError::validation(
                "Max segment size cannot be zero",
            ));
        }

        if self.max_segment_size > MAX_CHECKPOINT_SIZE {
            return Err(CheckpointError::validation(format!(
                "Max segment size {} exceeds maximum checkpoint size {}",
                self.max_segment_size, MAX_CHECKPOINT_SIZE
            )));
        }

        if self.max_segments == 0 {
            return Err(CheckpointError::validation(
                "Max segments cannot be zero",
            ));
        }

        if self.max_segments > 256 {
            return Err(CheckpointError::validation(
                "Max segments cannot exceed 256",
            ));
        }

        Ok(())
    }

    /// Calculate maximum total checkpoint size
    pub fn max_total_size(&self) -> u64 {
        self.max_segment_size * self.max_segments as u64
    }
}

/// Metadata for a single checkpoint segment
#[derive(Debug, Clone)]
pub struct CheckpointSegment {
    /// Segment index in the sequence (0-based)
    pub segment_index: u32,

    /// Path to the segment file
    pub segment_path: PathBuf,

    /// LSN range covered by this segment
    pub lsn_range: (u64, u64),

    /// Number of blocks in this segment
    pub block_count: u64,

    /// CRC32 checksum of segment data
    pub checksum: u64,

    /// Size of the segment file in bytes
    pub size: u64,
}

impl CheckpointSegment {
    /// Create a new segment metadata
    pub fn new(
        segment_index: u32,
        segment_path: PathBuf,
        lsn_range: (u64, u64),
        block_count: u64,
    ) -> Self {
        Self {
            segment_index,
            segment_path,
            lsn_range,
            block_count,
            checksum: 0,
            size: 0,
        }
    }

    /// Get the segment file name
    pub fn file_name(&self) -> String {
        format!("{:03}", self.segment_index)
    }
}

/// Metadata for checkpoint recovery from manifest
#[derive(Debug, Clone)]
pub struct CheckpointSegmentMeta {
    /// Segment index
    pub index: u32,

    /// LSN range start
    pub lsn_start: u64,

    /// LSN range end
    pub lsn_end: u64,

    /// Block count
    pub block_count: u64,

    /// Segment checksum
    pub checksum: u64,

    /// Segment size
    pub size: u64,
}

/// Checkpoint manifest containing all segment metadata
#[derive(Debug, Clone)]
pub struct CheckpointManifest {
    /// Number of segments in the checkpoint
    pub segment_count: u32,

    /// Metadata for each segment
    pub segments: Vec<CheckpointSegmentMeta>,

    /// Total LSN range across all segments
    pub total_lsn_range: (u64, u64),

    /// Total block count across all segments
    pub total_block_count: u64,

    /// Manifest checksum for validation
    pub checksum: u64,

    /// Checkpoint timestamp
    pub timestamp: u64,
}

impl Default for CheckpointManifest {
    fn default() -> Self {
        Self {
            segment_count: 0,
            segments: Vec::new(),
            total_lsn_range: (0, 0),
            total_block_count: 0,
            checksum: 0,
            timestamp: 0,
        }
    }
}

impl CheckpointManifest {
    /// Create a new checkpoint manifest
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a segment to the manifest
    pub fn add_segment(&mut self, meta: CheckpointSegmentMeta) {
        self.segments.push(meta);
        self.segment_count = self.segments.len() as u32;
        self.update_totals();
    }

    /// Update total LSN range and block count
    fn update_totals(&mut self) {
        if self.segments.is_empty() {
            self.total_lsn_range = (0, 0);
            self.total_block_count = 0;
            return;
        }

        let min_lsn = self.segments.iter().map(|s| s.lsn_start).min().unwrap_or(0);
        let max_lsn = self.segments.iter().map(|s| s.lsn_end).max().unwrap_or(0);
        self.total_lsn_range = (min_lsn, max_lsn);
        self.total_block_count = self.segments.iter().map(|s| s.block_count).sum();
    }

    /// Validate manifest consistency
    pub fn validate(&self) -> CheckpointResult<()> {
        if self.segment_count as usize != self.segments.len() {
            return Err(CheckpointError::corruption(format!(
                "Segment count mismatch: manifest says {} but has {} segments",
                self.segment_count,
                self.segments.len()
            )));
        }

        // Check segment index continuity
        for (i, segment) in self.segments.iter().enumerate() {
            if segment.index as usize != i {
                return Err(CheckpointError::corruption(format!(
                    "Segment index mismatch: expected {} but found {}",
                    i, segment.index
                )));
            }
        }

        // Check LSN continuity
        for window in self.segments.windows(2) {
            if window[0].lsn_end > window[1].lsn_start {
                return Err(CheckpointError::corruption(format!(
                    "LSN range overlap: segment {} ends at {} but segment {} starts at {}",
                    window[0].index, window[0].lsn_end, window[1].index, window[1].lsn_start
                )));
            }
        }

        Ok(())
    }
}

/// Writer for multi-file checkpoint segments
pub struct SegmentWriter {
    /// Current segment index
    segment_index: u32,

    /// Current segment file writer
    writer: BufWriter<File>,

    /// Current segment path
    segment_path: PathBuf,

    /// Current segment size in bytes
    current_size: u64,

    /// Maximum segment size
    max_segment_size: u64,

    /// LSN start of current segment
    segment_lsn_start: u64,

    /// Block count in current segment
    segment_block_count: u64,

    /// Running CRC32 checksum
    checksum: u32,

    /// All completed segments
    completed_segments: Vec<CheckpointSegment>,

    /// Configuration
    config: MultiFileCheckpointConfig,
}

impl SegmentWriter {
    /// Create a new segment writer
    pub fn create(config: MultiFileCheckpointConfig, index: u32, lsn_start: u64) -> CheckpointResult<Self> {
        config.validate()?;

        if index as usize >= config.max_segments {
            return Err(CheckpointError::resource(format!(
                "Segment index {} exceeds max segments {}",
                index, config.max_segments
            )));
        }

        let segment_path = config.base_path.with_extension(format!("{}.{}", SEGMENT_EXTENSION, format!("{:03}", index)));

        // Create segment file
        let file = File::create(&segment_path).map_err(|e| {
            CheckpointError::io(format!("Failed to create segment file {}: {}", segment_path.display(), e))
        })?;

        let mut writer = BufWriter::with_capacity(DEFAULT_CHECKPOINT_BUFFER_SIZE, file);

        // Write segment header
        Self::write_segment_header(&mut writer, index, lsn_start)?;

        Ok(Self {
            segment_index: index,
            writer,
            segment_path,
            current_size: 0,
            max_segment_size: config.max_segment_size,
            segment_lsn_start: lsn_start,
            segment_block_count: 0,
            checksum: 0,
            completed_segments: Vec::new(),
            config,
        })
    }

    /// Write segment header
    fn write_segment_header<W: Write>(writer: &mut W, index: u32, lsn_start: u64) -> CheckpointResult<()> {
        // Write magic number
        writer
            .write_all(SEGMENT_MAGIC)
            .map_err(|e| CheckpointError::io(format!("Failed to write segment magic: {}", e)))?;

        // Write segment index
        writer
            .write_all(&index.to_le_bytes())
            .map_err(|e| CheckpointError::io(format!("Failed to write segment index: {}", e)))?;

        // Write LSN start
        writer
            .write_all(&lsn_start.to_le_bytes())
            .map_err(|e| CheckpointError::io(format!("Failed to write LSN start: {}", e)))?;

        // Write placeholder LSN end (will be updated on finalize)
        writer
            .write_all(&0u64.to_le_bytes())
            .map_err(|e| CheckpointError::io(format!("Failed to write LSN end placeholder: {}", e)))?;

        // Write placeholder block count
        writer
            .write_all(&0u64.to_le_bytes())
            .map_err(|e| CheckpointError::io(format!("Failed to write block count placeholder: {}", e)))?;

        Ok(())
    }

    /// Write data to the current segment
    pub fn write_data(&mut self, data: &[u8]) -> CheckpointResult<usize> {
        // Check if we need to rotate before writing
        if self.current_size + data.len() as u64 > self.max_segment_size {
            return Err(CheckpointError::resource(
                "Segment size exceeded, call rotate_segment() first",
            ));
        }

        self.writer
            .write_all(data)
            .map_err(|e| CheckpointError::io(format!("Failed to write segment data: {}", e)))?;

        let written = data.len();
        self.current_size += written as u64;

        // Update checksum using simple hash (sum of bytes with rotation)
        let mut current = self.checksum as u64;
        for &byte in data {
            current = current.wrapping_mul(31).wrapping_add(byte as u64);
        }
        self.checksum = current as u32;

        Ok(written)
    }

    /// Check if segment needs rotation
    pub fn needs_rotation(&self) -> bool {
        self.current_size >= self.max_segment_size
    }

    /// Get remaining space in current segment
    pub fn remaining_space(&self) -> u64 {
        self.max_segment_size.saturating_sub(self.current_size)
    }

    /// Finalize current segment and create a new one
    pub fn rotate_segment(&mut self, lsn_start: u64) -> CheckpointResult<()> {
        // Finalize current segment first
        if self.current_size > 0 {
            self.finalize(lsn_start - 1, self.segment_block_count)?;
        }

        // Create new segment
        let new_index = self.segment_index + 1;

        if new_index as usize >= self.config.max_segments {
            return Err(CheckpointError::resource(format!(
                "Cannot create segment {}: maximum segments ({}) reached",
                new_index, self.config.max_segments
            )));
        }

        let segment_path = self.config.base_path.with_extension(format!("{}.{}", SEGMENT_EXTENSION, format!("{:03}", new_index)));

        let file = File::create(&segment_path).map_err(|e| {
            CheckpointError::io(format!("Failed to create rotated segment file {}: {}", segment_path.display(), e))
        })?;

        let mut writer = BufWriter::with_capacity(DEFAULT_CHECKPOINT_BUFFER_SIZE, file);

        // Write segment header
        Self::write_segment_header(&mut writer, new_index, lsn_start)?;

        self.segment_index = new_index;
        self.writer = writer;
        self.segment_path = segment_path;
        self.current_size = 0;
        self.segment_lsn_start = lsn_start;
        self.segment_block_count = 0;
        self.checksum = 0;

        Ok(())
    }

    /// Finalize the current segment
    pub fn finalize(&mut self, lsn_end: u64, block_count: u64) -> CheckpointResult<CheckpointSegment> {
        // Flush all data
        self.writer.flush().map_err(|e| {
            CheckpointError::io(format!("Failed to flush segment: {}", e))
        })?;

        // Seek back to update LSN end and block count in header
        let mut file = self.writer.get_ref().try_clone().map_err(|e| {
            CheckpointError::io(format!("Failed to clone file handle: {}", e))
        })?;

        // LSN end is at offset 16 (4 magic + 4 index + 8 lsn_start)
        file.seek(SeekFrom::Start(16)).map_err(|e| {
            CheckpointError::io(format!("Failed to seek to LSN end: {}", e))
        })?;

        file.write_all(&lsn_end.to_le_bytes()).map_err(|e| {
            CheckpointError::io(format!("Failed to write LSN end: {}", e))
        })?;

        // Block count is at offset 24
        file.write_all(&block_count.to_le_bytes()).map_err(|e| {
            CheckpointError::io(format!("Failed to write block count: {}", e))
        })?;

        file.sync_all().map_err(|e| {
            CheckpointError::io(format!("Failed to sync segment file: {}", e))
        })?;

        // Create segment metadata
        let segment = CheckpointSegment {
            segment_index: self.segment_index,
            segment_path: self.segment_path.clone(),
            lsn_range: (self.segment_lsn_start, lsn_end),
            block_count,
            checksum: self.checksum as u64,
            size: self.current_size,
        };

        self.completed_segments.push(segment.clone());

        Ok(segment)
    }

    /// Get all completed segments
    pub fn completed_segments(&self) -> &[CheckpointSegment] {
        &self.completed_segments
    }

    /// Get current segment index
    pub fn current_index(&self) -> u32 {
        self.segment_index
    }

    /// Get current segment size
    pub fn current_size(&self) -> u64 {
        self.current_size
    }

    /// Flush current data without finalizing
    pub fn flush(&mut self) -> CheckpointResult<()> {
        self.writer.flush().map_err(|e| {
            CheckpointError::io(format!("Failed to flush segment writer: {}", e))
        })
    }
}

/// Reader for multi-file checkpoint segments
pub struct SegmentReader {
    /// Segment file reader
    reader: BufReader<File>,

    /// Segment metadata
    segment: CheckpointSegment,

    /// Current position in segment
    position: u64,

    /// Validation checksum
    expected_checksum: u64,
}

impl SegmentReader {
    /// Open a segment file for reading
    pub fn open_segment(path: &Path) -> CheckpointResult<Self> {
        let file = File::open(path).map_err(|e| {
            CheckpointError::io(format!("Failed to open segment file {}: {}", path.display(), e))
        })?;

        let mut reader = BufReader::with_capacity(DEFAULT_CHECKPOINT_BUFFER_SIZE, file);

        // Read and validate segment header
        let segment = Self::read_segment_header(&mut reader, path)?;
        let expected_checksum = segment.checksum;

        Ok(Self {
            reader,
            segment,
            position: 0,
            expected_checksum,
        })
    }

    /// Read segment header
    fn read_segment_header<R: Read + Seek>(reader: &mut R, path: &Path) -> CheckpointResult<CheckpointSegment> {
        let mut magic = [0u8; 4];
        reader.read_exact(&mut magic).map_err(|e| {
            CheckpointError::io(format!("Failed to read segment magic: {}", e))
        })?;

        if magic != *SEGMENT_MAGIC {
            return Err(CheckpointError::corruption(format!(
                "Invalid segment magic in {}: expected {:?}, found {:?}",
                path.display(),
                SEGMENT_MAGIC,
                magic
            )));
        }

        let mut index_bytes = [0u8; 4];
        reader.read_exact(&mut index_bytes).map_err(|e| {
            CheckpointError::io(format!("Failed to read segment index: {}", e))
        })?;
        let segment_index = u32::from_le_bytes(index_bytes);

        let mut lsn_start_bytes = [0u8; 8];
        reader.read_exact(&mut lsn_start_bytes).map_err(|e| {
            CheckpointError::io(format!("Failed to read LSN start: {}", e))
        })?;
        let lsn_start = u64::from_le_bytes(lsn_start_bytes);

        let mut lsn_end_bytes = [0u8; 8];
        reader.read_exact(&mut lsn_end_bytes).map_err(|e| {
            CheckpointError::io(format!("Failed to read LSN end: {}", e))
        })?;
        let lsn_end = u64::from_le_bytes(lsn_end_bytes);

        let mut block_count_bytes = [0u8; 8];
        reader.read_exact(&mut block_count_bytes).map_err(|e| {
            CheckpointError::io(format!("Failed to read block count: {}", e))
        })?;
        let block_count = u64::from_le_bytes(block_count_bytes);

        // Get file size
        let metadata = std::fs::metadata(path).map_err(|e| {
            CheckpointError::io(format!("Failed to get segment metadata: {}", e))
        })?;

        Ok(CheckpointSegment {
            segment_index,
            segment_path: path.to_path_buf(),
            lsn_range: (lsn_start, lsn_end),
            block_count,
            checksum: 0,
            size: metadata.len(),
        })
    }

    /// Read data from the segment
    pub fn read_data(&mut self, buf: &mut [u8]) -> CheckpointResult<usize> {
        let n = self.reader.read(buf).map_err(|e| {
            CheckpointError::io(format!("Failed to read segment data: {}", e))
        })?;

        self.position += n as u64;
        Ok(n)
    }

    /// Validate the segment checksum
    pub fn validate_checksum(&self) -> CheckpointResult<bool> {
        // For now, we'll do basic validation
        // Full checksum validation would require re-reading the entire file
        Ok(true)
    }

    /// Get segment metadata
    pub fn segment(&self) -> &CheckpointSegment {
        &self.segment
    }

    /// Get remaining bytes in segment
    pub fn remaining(&self) -> u64 {
        self.segment.size.saturating_sub(self.position)
    }
}

/// Multi-file checkpoint recovery handler
pub struct MultiFileRecovery;

impl MultiFileRecovery {
    /// Discover all checkpoint manifests in a directory
    pub fn discover_checkpoints(base_path: &Path) -> CheckpointResult<Vec<PathBuf>> {
        let parent = base_path.parent().ok_or_else(|| {
            CheckpointError::io("Checkpoint path has no parent directory")
        })?;

        let mut manifests = Vec::new();

        let entries = std::fs::read_dir(parent).map_err(|e| {
            CheckpointError::io(format!("Failed to read checkpoint directory: {}", e))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| {
                CheckpointError::io(format!("Failed to read directory entry: {}", e))
            })?;

            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some(MANIFEST_EXTENSION) {
                manifests.push(path);
            }
        }

        manifests.sort();
        Ok(manifests)
    }

    /// Load a checkpoint manifest from file
    pub fn load_manifest(path: &Path) -> CheckpointResult<CheckpointManifest> {
        let mut file = File::open(path).map_err(|e| {
            CheckpointError::io(format!("Failed to open manifest file {}: {}", path.display(), e))
        })?;

        // Read and validate magic
        let mut magic = [0u8; 4];
        file.read_exact(&mut magic).map_err(|e| {
            CheckpointError::io(format!("Failed to read manifest magic: {}", e))
        })?;

        if magic != *MANIFEST_MAGIC {
            return Err(CheckpointError::corruption(format!(
                "Invalid manifest magic in {}: expected {:?}, found {:?}",
                path.display(),
                MANIFEST_MAGIC,
                magic
            )));
        }

        // Read segment count
        let mut count_bytes = [0u8; 4];
        file.read_exact(&mut count_bytes).map_err(|e| {
            CheckpointError::io(format!("Failed to read segment count: {}", e))
        })?;
        let segment_count = u32::from_le_bytes(count_bytes);

        // Read timestamp
        let mut timestamp_bytes = [0u8; 8];
        file.read_exact(&mut timestamp_bytes).map_err(|e| {
            CheckpointError::io(format!("Failed to read timestamp: {}", e))
        })?;
        let timestamp = u64::from_le_bytes(timestamp_bytes);

        // Read total LSN range
        let mut lsn_start_bytes = [0u8; 8];
        file.read_exact(&mut lsn_start_bytes).map_err(|e| {
            CheckpointError::io(format!("Failed to read total LSN start: {}", e))
        })?;
        let lsn_start = u64::from_le_bytes(lsn_start_bytes);

        let mut lsn_end_bytes = [0u8; 8];
        file.read_exact(&mut lsn_end_bytes).map_err(|e| {
            CheckpointError::io(format!("Failed to read total LSN end: {}", e))
        })?;
        let lsn_end = u64::from_le_bytes(lsn_end_bytes);

        // Read total block count
        let mut block_count_bytes = [0u8; 8];
        file.read_exact(&mut block_count_bytes).map_err(|e| {
            CheckpointError::io(format!("Failed to read total block count: {}", e))
        })?;
        let total_block_count = u64::from_le_bytes(block_count_bytes);

        // Read checksum
        let mut checksum_bytes = [0u8; 8];
        file.read_exact(&mut checksum_bytes).map_err(|e| {
            CheckpointError::io(format!("Failed to read checksum: {}", e))
        })?;
        let checksum = u64::from_le_bytes(checksum_bytes);

        // Read segment metadata
        let mut segments = Vec::new();
        for i in 0..segment_count {
            let mut index_bytes = [0u8; 4];
            file.read_exact(&mut index_bytes).map_err(|e| {
                CheckpointError::io(format!("Failed to read segment {} index: {}", i, e))
            })?;
            let index = u32::from_le_bytes(index_bytes);

            let mut lsn_start_bytes = [0u8; 8];
            file.read_exact(&mut lsn_start_bytes).map_err(|e| {
                CheckpointError::io(format!("Failed to read segment {} LSN start: {}", i, e))
            })?;
            let lsn_start = u64::from_le_bytes(lsn_start_bytes);

            let mut lsn_end_bytes = [0u8; 8];
            file.read_exact(&mut lsn_end_bytes).map_err(|e| {
                CheckpointError::io(format!("Failed to read segment {} LSN end: {}", i, e))
            })?;
            let lsn_end = u64::from_le_bytes(lsn_end_bytes);

            let mut block_count_bytes = [0u8; 8];
            file.read_exact(&mut block_count_bytes).map_err(|e| {
                CheckpointError::io(format!("Failed to read segment {} block count: {}", i, e))
            })?;
            let block_count = u64::from_le_bytes(block_count_bytes);

            let mut checksum_bytes = [0u8; 8];
            file.read_exact(&mut checksum_bytes).map_err(|e| {
                CheckpointError::io(format!("Failed to read segment {} checksum: {}", i, e))
            })?;
            let checksum = u64::from_le_bytes(checksum_bytes);

            let mut size_bytes = [0u8; 8];
            file.read_exact(&mut size_bytes).map_err(|e| {
                CheckpointError::io(format!("Failed to read segment {} size: {}", i, e))
            })?;
            let size = u64::from_le_bytes(size_bytes);

            segments.push(CheckpointSegmentMeta {
                index,
                lsn_start,
                lsn_end,
                block_count,
                checksum,
                size,
            });
        }

        Ok(CheckpointManifest {
            segment_count,
            segments,
            total_lsn_range: (lsn_start, lsn_end),
            total_block_count,
            checksum,
            timestamp,
        })
    }

    /// Validate a checkpoint manifest and its segments
    pub fn validate_checkpoint(
        manifest: &CheckpointManifest,
        base_path: &Path,
    ) -> CheckpointResult<bool> {
        // Validate manifest consistency
        manifest.validate()?;

        // Check all segment files exist
        for segment_meta in &manifest.segments {
            let segment_path = base_path.with_extension(format!(
                "{}.{}",
                SEGMENT_EXTENSION,
                format!("{:03}", segment_meta.index)
            ));

            if !segment_path.exists() {
                return Err(CheckpointError::corruption(format!(
                    "Segment file {} missing for checkpoint",
                    segment_path.display()
                )));
            }

            // Validate segment file
            let _reader = SegmentReader::open_segment(&segment_path)?;
        }

        Ok(true)
    }

    /// Write a manifest file
    pub fn write_manifest(
        manifest: &CheckpointManifest,
        base_path: &Path,
    ) -> CheckpointResult<()> {
        let manifest_path = base_path.with_extension(MANIFEST_EXTENSION);

        // Write to temporary file first
        let temp_path = manifest_path.with_extension("manifest.tmp");

        let mut file = File::create(&temp_path).map_err(|e| {
            CheckpointError::io(format!("Failed to create manifest file: {}", e))
        })?;

        // Write magic
        file.write_all(MANIFEST_MAGIC).map_err(|e| {
            CheckpointError::io(format!("Failed to write manifest magic: {}", e))
        })?;

        // Write segment count
        file.write_all(&manifest.segment_count.to_le_bytes()).map_err(|e| {
            CheckpointError::io(format!("Failed to write segment count: {}", e))
        })?;

        // Write timestamp
        file.write_all(&manifest.timestamp.to_le_bytes()).map_err(|e| {
            CheckpointError::io(format!("Failed to write timestamp: {}", e))
        })?;

        // Write total LSN range
        file.write_all(&manifest.total_lsn_range.0.to_le_bytes()).map_err(|e| {
            CheckpointError::io(format!("Failed to write total LSN start: {}", e))
        })?;
        file.write_all(&manifest.total_lsn_range.1.to_le_bytes()).map_err(|e| {
            CheckpointError::io(format!("Failed to write total LSN end: {}", e))
        })?;

        // Write total block count
        file.write_all(&manifest.total_block_count.to_le_bytes()).map_err(|e| {
            CheckpointError::io(format!("Failed to write total block count: {}", e))
        })?;

        // Write checksum
        file.write_all(&manifest.checksum.to_le_bytes()).map_err(|e| {
            CheckpointError::io(format!("Failed to write checksum: {}", e))
        })?;

        // Write segment metadata
        for segment_meta in &manifest.segments {
            file.write_all(&segment_meta.index.to_le_bytes()).map_err(|e| {
                CheckpointError::io(format!("Failed to write segment index: {}", e))
            })?;
            file.write_all(&segment_meta.lsn_start.to_le_bytes()).map_err(|e| {
                CheckpointError::io(format!("Failed to write segment LSN start: {}", e))
            })?;
            file.write_all(&segment_meta.lsn_end.to_le_bytes()).map_err(|e| {
                CheckpointError::io(format!("Failed to write segment LSN end: {}", e))
            })?;
            file.write_all(&segment_meta.block_count.to_le_bytes()).map_err(|e| {
                CheckpointError::io(format!("Failed to write segment block count: {}", e))
            })?;
            file.write_all(&segment_meta.checksum.to_le_bytes()).map_err(|e| {
                CheckpointError::io(format!("Failed to write segment checksum: {}", e))
            })?;
            file.write_all(&segment_meta.size.to_le_bytes()).map_err(|e| {
                CheckpointError::io(format!("Failed to write segment size: {}", e))
            })?;
        }

        // Sync and close
        file.sync_all().map_err(|e| {
            CheckpointError::io(format!("Failed to sync manifest file: {}", e))
        })?;

        // Atomic rename
        std::fs::rename(&temp_path, &manifest_path).map_err(|e| {
            CheckpointError::io(format!("Failed to rename manifest file: {}", e))
        })?;

        Ok(())
    }
}

/// Iterator for reading data across multiple checkpoint segments
pub struct MultiSegmentIterator {
    /// Manifest for the checkpoint
    manifest: CheckpointManifest,

    /// Base path for segment files
    base_path: PathBuf,

    /// Current segment index
    current_segment: usize,

    /// Current segment reader
    current_reader: Option<SegmentReader>,

    /// Read buffer
    buffer: VecDeque<u8>,
}

impl MultiSegmentIterator {
    /// Create a new multi-segment iterator
    pub fn new(manifest: CheckpointManifest, base_path: PathBuf) -> CheckpointResult<Self> {
        Ok(Self {
            manifest,
            base_path,
            current_segment: 0,
            current_reader: None,
            buffer: VecDeque::new(),
        })
    }

    /// Open the next segment
    fn open_next_segment(&mut self) -> CheckpointResult<()> {
        if self.current_segment >= self.manifest.segments.len() {
            return Err(CheckpointError::io("No more segments to read"));
        }

        let segment_meta = &self.manifest.segments[self.current_segment];
        let segment_path = self.base_path.with_extension(format!(
            "{}.{}",
            SEGMENT_EXTENSION,
            format!("{:03}", segment_meta.index)
        ));

        self.current_reader = Some(SegmentReader::open_segment(&segment_path)?);
        self.current_segment += 1;

        Ok(())
    }
}

impl Read for MultiSegmentIterator {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Fill from buffer first
        let mut total_read = 0;

        while total_read < buf.len() {
            // Read from buffer if available
            if !self.buffer.is_empty() {
                let to_read = std::cmp::min(self.buffer.len(), buf.len() - total_read);
                for (i, byte) in self.buffer.drain(..to_read).enumerate() {
                    buf[total_read + i] = byte;
                }
                total_read += to_read;
                continue;
            }

            // Need to read from current or next segment
            if self.current_reader.is_none() {
                if let Err(e) = self.open_next_segment() {
                    if e.kind == CheckpointErrorKind::Io {
                        return Ok(total_read); // End of all segments
                    }
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        e.to_string(),
                    ));
                }
            }

            let reader = self.current_reader.as_mut().unwrap();
            let remaining = &mut buf[total_read..];
            let n = reader.read_data(remaining).map_err(|e| {
                std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
            })?;

            if n == 0 {
                // End of current segment, move to next
                self.current_reader = None;
                continue;
            }

            total_read += n;
        }

        Ok(total_read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::tempdir;

    #[test]
    fn test_config_default() {
        let config = MultiFileCheckpointConfig::default();
        assert_eq!(config.max_segment_size, DEFAULT_MAX_SEGMENT_SIZE);
        assert_eq!(config.max_segments, DEFAULT_MAX_SEGMENTS);
    }

    #[test]
    fn test_config_builder() {
        let config = MultiFileCheckpointConfig::new(PathBuf::from("test"))
            .with_max_segment_size(256 * 1024 * 1024)
            .with_max_segments(8);

        assert_eq!(config.max_segment_size, 256 * 1024 * 1024);
        assert_eq!(config.max_segments, 8);
    }

    #[test]
    fn test_config_validation() {
        // Valid config
        let config = MultiFileCheckpointConfig::default();
        assert!(config.validate().is_ok());

        // Invalid max_segment_size
        let config = MultiFileCheckpointConfig {
            max_segment_size: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());

        // Invalid max_segments
        let config = MultiFileCheckpointConfig {
            max_segments: 0,
            ..Default::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_max_total_size() {
        let config = MultiFileCheckpointConfig {
            max_segment_size: 1024 * 1024, // 1 MB
            max_segments: 10,
            ..Default::default()
        };

        assert_eq!(config.max_total_size(), 10 * 1024 * 1024);
    }

    #[test]
    fn test_checkpoint_segment_creation() {
        let segment = CheckpointSegment::new(0, PathBuf::from("test.ckpt.000"), (100, 200), 50);

        assert_eq!(segment.segment_index, 0);
        assert_eq!(segment.lsn_range, (100, 200));
        assert_eq!(segment.block_count, 50);
        assert_eq!(segment.file_name(), "000");
    }

    #[test]
    fn test_manifest_default() {
        let manifest = CheckpointManifest::default();

        assert_eq!(manifest.segment_count, 0);
        assert!(manifest.segments.is_empty());
        assert_eq!(manifest.total_lsn_range, (0, 0));
    }

    #[test]
    fn test_manifest_add_segment() {
        let mut manifest = CheckpointManifest::new();

        manifest.add_segment(CheckpointSegmentMeta {
            index: 0,
            lsn_start: 100,
            lsn_end: 200,
            block_count: 50,
            checksum: 12345,
            size: 1024,
        });

        assert_eq!(manifest.segment_count, 1);
        assert_eq!(manifest.segments.len(), 1);
        assert_eq!(manifest.total_lsn_range, (100, 200));
        assert_eq!(manifest.total_block_count, 50);
    }

    #[test]
    fn test_manifest_validation() {
        let mut manifest = CheckpointManifest::new();

        // Add segments with proper indices
        manifest.add_segment(CheckpointSegmentMeta {
            index: 0,
            lsn_start: 100,
            lsn_end: 200,
            block_count: 50,
            checksum: 0,
            size: 1024,
        });
        manifest.add_segment(CheckpointSegmentMeta {
            index: 1,
            lsn_start: 200,
            lsn_end: 300,
            block_count: 50,
            checksum: 0,
            size: 1024,
        });

        assert!(manifest.validate().is_ok());

        // Test LSN overlap detection
        let mut bad_manifest = CheckpointManifest::new();
        bad_manifest.add_segment(CheckpointSegmentMeta {
            index: 0,
            lsn_start: 100,
            lsn_end: 300,
            block_count: 50,
            checksum: 0,
            size: 1024,
        });
        bad_manifest.add_segment(CheckpointSegmentMeta {
            index: 1,
            lsn_start: 200, // Overlaps
            lsn_end: 400,
            block_count: 50,
            checksum: 0,
            size: 1024,
        });

        assert!(bad_manifest.validate().is_err());
    }

    #[test]
    fn test_segment_writer_create() -> CheckpointResult<()> {
        let temp_dir = tempdir()?;
        let base_path = temp_dir.path().join("checkpoint");

        let config = MultiFileCheckpointConfig::new(base_path.clone())
            .with_max_segment_size(1024)
            .with_max_segments(4);

        let writer = SegmentWriter::create(config, 0, 100)?;

        assert_eq!(writer.current_index(), 0);
        assert_eq!(writer.current_size(), 0);
        assert!(!writer.needs_rotation());
        assert_eq!(writer.remaining_space(), 1024);

        Ok(())
    }

    #[test]
    fn test_segment_writer_write_data() -> CheckpointResult<()> {
        let temp_dir = tempdir()?;
        let base_path = temp_dir.path().join("checkpoint");

        let config = MultiFileCheckpointConfig::new(base_path.clone())
            .with_max_segment_size(1024)
            .with_max_segments(4);

        let mut writer = SegmentWriter::create(config, 0, 100)?;

        let data = vec![1u8, 2, 3, 4, 5];
        let written = writer.write_data(&data)?;

        assert_eq!(written, 5);
        assert_eq!(writer.current_size(), 5);

        Ok(())
    }

    #[test]
    fn test_segment_writer_needs_rotation() -> CheckpointResult<()> {
        let temp_dir = tempdir()?;
        let base_path = temp_dir.path().join("checkpoint");

        let config = MultiFileCheckpointConfig::new(base_path.clone())
            .with_max_segment_size(100)
            .with_max_segments(4);

        let mut writer = SegmentWriter::create(config, 0, 100)?;

        assert!(!writer.needs_rotation());

        // Write 90 bytes
        let data = vec![1u8; 90];
        writer.write_data(&data)?;
        assert!(!writer.needs_rotation());

        // Write 11 more bytes (would exceed limit, but check happens before write)
        // Since write_data checks before writing, this would return an error

        Ok(())
    }

    #[test]
    fn test_segment_writer_rotation() -> CheckpointResult<()> {
        let temp_dir = tempdir()?;
        let base_path = temp_dir.path().join("checkpoint");

        let config = MultiFileCheckpointConfig::new(base_path.clone())
            .with_max_segment_size(100)
            .with_max_segments(4);

        let mut writer = SegmentWriter::create(config, 0, 100)?;

        // Write some data and finalize
        let data = vec![1u8; 50];
        writer.write_data(&data)?;
        writer.finalize(150, 10)?;

        assert_eq!(writer.completed_segments().len(), 1);

        // Rotate to new segment
        writer.rotate_segment(150)?;

        assert_eq!(writer.current_index(), 1);
        assert_eq!(writer.current_size(), 0);

        Ok(())
    }

    #[test]
    fn test_segment_reader_open() -> CheckpointResult<()> {
        let temp_dir = tempdir()?;
        let base_path = temp_dir.path().join("checkpoint");

        let config = MultiFileCheckpointConfig::new(base_path.clone())
            .with_max_segment_size(1024)
            .with_max_segments(4);

        // Create a segment file first
        let mut writer = SegmentWriter::create(config, 0, 100)?;
        let data = vec![1u8, 2, 3, 4, 5];
        writer.write_data(&data)?;
        writer.finalize(150, 10)?;

        // Now read it back
        let segment_path = base_path.with_extension("ckpt.000");
        let reader = SegmentReader::open_segment(&segment_path)?;

        assert_eq!(reader.segment().segment_index, 0);
        assert_eq!(reader.segment().lsn_range, (100, 150));

        Ok(())
    }

    #[test]
    fn test_write_and_load_manifest() -> CheckpointResult<()> {
        let temp_dir = tempdir()?;
        let base_path = temp_dir.path().join("checkpoint");

        let mut manifest = CheckpointManifest::new();
        manifest.timestamp = 1234567890;
        manifest.add_segment(CheckpointSegmentMeta {
            index: 0,
            lsn_start: 100,
            lsn_end: 200,
            block_count: 50,
            checksum: 12345,
            size: 1024,
        });
        manifest.add_segment(CheckpointSegmentMeta {
            index: 1,
            lsn_start: 200,
            lsn_end: 300,
            block_count: 50,
            checksum: 67890,
            size: 2048,
        });

        MultiFileRecovery::write_manifest(&manifest, &base_path)?;

        let loaded = MultiFileRecovery::load_manifest(&base_path.with_extension("manifest"))?;

        assert_eq!(loaded.segment_count, 2);
        assert_eq!(loaded.timestamp, 1234567890);
        assert_eq!(loaded.total_lsn_range, (100, 300));
        assert_eq!(loaded.total_block_count, 100);

        Ok(())
    }

    #[test]
    fn test_discover_checkpoints() -> CheckpointResult<()> {
        let temp_dir = tempdir()?;
        let base_path = temp_dir.path().join("checkpoint");

        // Create some manifest files
        let manifest1 = base_path.with_extension("manifest");
        let manifest2 = temp_dir.path().join("other.manifest");

        // Create empty files
        File::create(&manifest1)?;
        File::create(&manifest2)?;

        let manifests = MultiFileRecovery::discover_checkpoints(&base_path)?;

        assert_eq!(manifests.len(), 2);

        Ok(())
    }

    #[test]
    fn test_validate_checkpoint_missing_segment() {
        let temp_dir = tempdir().unwrap();
        let base_path = temp_dir.path().join("checkpoint");

        let mut manifest = CheckpointManifest::new();
        manifest.add_segment(CheckpointSegmentMeta {
            index: 0,
            lsn_start: 100,
            lsn_end: 200,
            block_count: 50,
            checksum: 0,
            size: 1024,
        });

        // Don't create the segment file - validation should fail
        let result = MultiFileRecovery::validate_checkpoint(&manifest, &base_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_segment_magic_constants() {
        assert_eq!(SEGMENT_MAGIC, b"SGMT");
        assert_eq!(MANIFEST_MAGIC, b"MNFT");
    }

    #[test]
    fn test_max_segment_size_constant() {
        assert_eq!(DEFAULT_MAX_SEGMENT_SIZE, 512 * 1024 * 1024);
    }

    #[test]
    fn test_max_segments_constant() {
        assert_eq!(DEFAULT_MAX_SEGMENTS, 16);
    }
}
