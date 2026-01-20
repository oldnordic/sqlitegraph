//! Header Rollback Operations
//!
//! This module provides rollback operations for header-related WAL records:
//! - HeaderUpdate: Restore old data to header region

use crate::backend::native::constants::HEADER_SIZE;
use super::super::RollbackSystem;
use crate::backend::native::v2::wal::recovery::errors::RecoveryError;
use crate::debug::debug_log;

/// Rollback header update by restoring old data
pub fn rollback_header_update(
    system: &RollbackSystem,
    header_offset: u64,
    old_data: &[u8],
) -> Result<(), RecoveryError> {
    debug_log!("Rolling back header update: offset={}, data_size={}", header_offset, old_data.len());

    // Step 1: Validate offset within header region
    if header_offset >= HEADER_SIZE as u64 {
        return Err(RecoveryError::validation(
            format!("Header offset {} exceeds header region size {}", header_offset, HEADER_SIZE)
        ));
    }

    let end_offset = header_offset + old_data.len() as u64;
    if end_offset > HEADER_SIZE as u64 {
        return Err(RecoveryError::validation(
            format!("Header rollback exceeds header region: offset={} + size={} > {}",
                   header_offset, old_data.len(), HEADER_SIZE)
        ));
    }

    // Step 2: Restore old data to GraphFile
    {
        let mut graph_file = system.graph_file().write()
            .map_err(|e| RecoveryError::replay_failure(
                format!("Failed to lock graph file: {}", e)
            ))?;

        graph_file.write_bytes(header_offset, old_data)
            .map_err(|e| RecoveryError::io_error(
                format!("Failed to restore header at offset {}: {:?}", header_offset, e)
            ))?;

        debug_log!("Successfully restored header at offset {} ({} bytes)", header_offset, old_data.len());
    }

    debug_log!("Header update rollback completed: offset={}, size={}", header_offset, old_data.len());
    Ok(())
}
