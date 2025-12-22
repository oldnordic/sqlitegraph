//! Advanced GraphFile features
//!
//! This module contains advanced features, debug operations,
//! experimental features, and specialized utilities.

use crate::backend::native::{
    types::{NativeBackendError, NativeResult},
};

use super::GraphFile;

/// Simple file health status structure
#[derive(Debug, Clone)]
pub struct FileHealthStatus {
    pub is_healthy: bool,
    pub issues: Vec<String>,
    pub file_size: u64,
    pub node_count: u32,
    pub edge_count: u32,
    pub free_space: u64,
}

/// Simple optimization report structure
#[derive(Debug, Clone)]
pub struct OptimizationReport {
    pub original_size: u64,
    pub optimized_size: u64,
    pub space_saved: u64,
    pub optimizations_applied: Vec<String>,
}

/// Simple debug info structure
#[derive(Debug, Clone)]
pub struct DebugInfo {
    pub file_path: std::path::PathBuf,
    pub file_size: u64,
    pub node_count: u32,
    pub edge_count: u32,
    pub free_space: u64,
    pub transaction_id: u64,
    pub is_transaction_active: bool,
}

impl GraphFile {
    /// Verify header was written immediately
    pub fn verify_header_written_immediately(&mut self, expected_bytes: &[u8]) -> NativeResult<()> {
        let mut actual_bytes = vec![0u8; expected_bytes.len()];
        self.read_bytes(0, &mut actual_bytes)?;

        if actual_bytes != expected_bytes {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1, // Using -1 since this is a header corruption, not a specific node
                reason: format!(
                    "Header verification failed: expected {:?}, got {:?}",
                    expected_bytes, actual_bytes
                ),
            });
        }

        Ok(())
    }

    /// Check file consistency
    pub fn check_consistency(&self) -> NativeResult<Vec<String>> {
        let mut issues = Vec::new();

        // Basic consistency checks
        if self.persistent_header().node_count == 0 && self.persistent_header().edge_count > 0 {
            issues.push("File has edges but no nodes".to_string());
        }

        if self.persistent_header().free_space_offset < self.persistent_header().edge_data_offset {
            issues.push("Free space offset is before edge data offset".to_string());
        }

        let file_size = self.file_size()?;
        if self.persistent_header().free_space_offset > file_size {
            issues.push("Free space offset exceeds file size".to_string());
        }

        Ok(issues)
    }

    /// Repair file corruption issues
    pub fn repair_corruption(&mut self) -> NativeResult<Vec<String>> {
        let mut repairs = Vec::new();

        // Basic repair operations
        let file_size = self.file_size()?;
        if self.persistent_header().free_space_offset > file_size {
            let old_offset = self.persistent_header().free_space_offset;
            self.persistent_header_mut().free_space_offset = file_size;
            repairs.push(format!(
                "Fixed free space offset: {} -> {}",
                old_offset, file_size
            ));
        }

        // Validate and repair node/edge counts
        let issues = self.check_consistency()?;
        for issue in issues {
            repairs.push(format!("Consistency issue: {}", issue));
        }

        Ok(repairs)
    }

    /// Get file health status
    pub fn get_health_status(&mut self) -> NativeResult<FileHealthStatus> {
        let issues = self.check_consistency()?;
        let is_healthy = issues.is_empty();

        Ok(FileHealthStatus {
            is_healthy,
            issues,
            file_size: self.file_size()?,
            node_count: self.persistent_header().node_count as u32,
            edge_count: self.persistent_header().edge_count as u32,
            free_space: self.persistent_header().free_space_offset,
        })
    }

    /// Optimize file layout
    pub fn optimize_layout(&mut self) -> NativeResult<OptimizationReport> {
        // This would implement file layout optimization
        // For now, return a basic report
        Ok(OptimizationReport {
            original_size: self.file_size()?,
            optimized_size: self.file_size()?, // No optimization yet
            space_saved: 0,
            optimizations_applied: Vec::new(),
        })
    }

    /// Compact file to reclaim free space
    pub fn compact_file(&mut self) -> NativeResult<u64> {
        let original_size = self.file_size()?;

        // Basic compaction: truncate to free_space_offset if there's unused space
        let free_space_offset = self.persistent_header().free_space_offset;
        if free_space_offset < original_size {
            self.file.set_len(free_space_offset)?;
            Ok(original_size - free_space_offset)
        } else {
            Ok(0)
        }
    }

    /// Create file snapshot for backup
    pub fn create_snapshot<P: AsRef<std::path::Path>>(&self, snapshot_path: P) -> NativeResult<()> {
        use std::fs::File;
        use std::io::{Read, Write};

        let mut source = File::open(&self.file_path)?;
        let mut dest = File::create(snapshot_path)?;

        let mut buffer = [0u8; 8192];
        loop {
            let bytes_read = source.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            dest.write_all(&buffer[..bytes_read])?;
        }

        Ok(())
    }

    /// Get debug information
    pub fn get_debug_info(&self) -> DebugInfo {
        DebugInfo {
            file_path: self.file_path.clone(),
            file_size: self.file_size().unwrap_or(0),
            node_count: self.persistent_header().node_count as u32,
            edge_count: self.persistent_header().edge_count as u32,
            free_space: self.persistent_header().free_space_offset,
            transaction_id: self.transaction_state.current_transaction_id(),
            is_transaction_active: self.transaction_state.is_active(),
        }
    }
}
