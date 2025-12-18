//! Debug instrumentation and logging utilities for GraphFile
//!
//! This module provides debug instrumentation functions for forensic analysis,
//! transaction auditing, and system state inspection during development and
//! troubleshooting.

use crate::backend::native::{
    types::NativeResult,
    persistent_header::PersistentHeaderV2,
    v2::{V2_MAGIC, V2_FORMAT_VERSION},
};

/// Debug instrumentation utilities for GraphFile operations
pub struct DebugInstrumentation;

impl DebugInstrumentation {
    /// Verify header was written correctly (temporary instrumentation)
    ///
    /// Reads back the header immediately after writing to ensure persistence integrity.
    /// Used for detecting write coherence issues during development.
    pub fn verify_header_written_immediately(
        file: &mut std::fs::File,
        expected_bytes: &[u8],
    ) -> NativeResult<()> {
        use std::io::{Seek, SeekFrom, Read};

        let mut read_back = vec![0u8; 16]; // Read first 16 bytes for verification
        file.seek(SeekFrom::Start(0))?;
        file.read_exact(&mut read_back)?;

        if expected_bytes.len() >= 16 {
            if read_back != &expected_bytes[..16] {
                return Err(crate::backend::native::types::NativeBackendError::CorruptNodeRecord {
                    node_id: -1,
                    reason: format!(
                        "Header write verification failed\nExpected first 16 bytes: {:02X?}\nActually read: {:02X?}",
                        &expected_bytes[..16],
                        read_back
                    ),
                });
            }
        }

        Ok(())
    }

    /// Print cluster layout debugging information
    ///
    /// Outputs detailed layout invariants for cluster offset calculations.
    /// Essential for debugging node/cluster collision issues.
    pub fn print_cluster_layout_debug(
        header: &PersistentHeaderV2,
        node_region_end: u64,
        base_cluster_start: u64,
        cluster_floor: u64,
    ) {
        println!("[CLUSTER_DEBUG] Layout invariants:");
        println!("  node_data_offset = {}", header.node_data_offset);
        println!("  node_count = {}", header.node_count);
        println!("  node_region_end = {}", node_region_end);
        println!("  base_cluster_start = {}", base_cluster_start);
        println!("  cluster_floor = {}", cluster_floor);
        println!(
            "  current outgoing_cluster_offset = {}",
            header.outgoing_cluster_offset
        );
        println!(
            "  current incoming_cluster_offset = {}",
            header.incoming_cluster_offset
        );
    }

    /// Print final cluster layout after corrections
    ///
    /// Shows the final state after all cluster offset corrections have been applied.
    pub fn print_final_cluster_layout(header: &PersistentHeaderV2) {
        println!(
            "  final outgoing_cluster_offset = {}",
            header.outgoing_cluster_offset
        );
        println!(
            "  final incoming_cluster_offset = {}",
            header.incoming_cluster_offset
        );
    }

    /// Log critical cluster offset fixes
    ///
    /// Records when cluster offsets are moved to prevent node slot corruption.
    pub fn log_cluster_offset_fix(
        cluster_type: &str,
        old_offset: u64,
        new_offset: u64,
    ) {
        println!(
            "CRITICAL FIX: Moving {}_cluster_offset from {} to {} to prevent node slot corruption",
            cluster_type, old_offset, new_offset
        );
    }

    /// Log transaction phase transitions
    ///
    /// Records the beginning and end of transaction phases for debugging.
    pub fn log_transaction_phase(phase: &str, tx_id: u64) {
        println!("PHASE 70: Transaction {} {}", tx_id, phase);
    }

    /// Log rollback information
    ///
    /// Outputs detailed rollback metrics for debugging transaction failures.
    pub fn log_rollback_info(
        rollback_floor: u64,
        enhanced_rollback_floor: u64,
        final_rollback_size: u64,
    ) {
        println!(
            "PHASE 72: rollback_floor = {}, enhanced_rollback_floor = {}, final_rollback_size = {}",
            rollback_floor, enhanced_rollback_floor, final_rollback_size
        );
    }

    /// Log file truncation operations (TRUNC_AUDIT)
    pub fn log_truncation_operation(
        current_size: u64,
        intended_rollback_size: u64,
        rollback_floor: u64,
        enhanced_rollback_floor: u64,
        final_rollback_size: u64,
        will_truncate: bool,
    ) {
        println!(
            "[TRUNC_AUDIT] ROLLBACK: current_size={}, intended_rollback_size={}, rollback_floor={}, enhanced_rollback_floor={}, final_rollback_size={}, will_truncate={}",
            current_size,
            intended_rollback_size,
            rollback_floor,
            enhanced_rollback_floor,
            final_rollback_size,
            will_truncate
        );
    }

    /// Log slot corruption checks
    pub fn log_slot_corruption_check(
        operation: &str,
        current_size: u64,
        final_rollback_size: u64,
        difference: u64,
    ) {
        println!(
            "[SLOT_CORRUPTION] FILE_TRUNCATE: current_size={}, final_rollback_size={}, difference={} bytes",
            current_size,
            final_rollback_size,
            difference
        );
        println!("[SLOT_CORRUPTION] {}: truncating {} bytes", operation, difference);
    }

    /// Log post-truncate slot verification
    pub fn log_post_truncate_slot_check(
        node_id: u64,
        slot_offset: u64,
        version: u8,
    ) {
        println!(
            "[SLOT_CORRUPTION] POST_TRUNCATE_CHECK: node_id={}, slot_offset=0x{:x}, version={}",
            node_id, slot_offset, version
        );
    }

    /// Log rollback completion
    pub fn log_rollback_completion(final_rollback_size: u64) {
        println!(
            "PHASE 72: Transaction rolled back to offset {}",
            final_rollback_size
        );
    }

    /// Log Phase 75 writeset recording
    #[cfg(feature = "trace_v2_io")]
    pub fn log_writeset_record(node_id: u64) {
        println!(
            "[phase75] WRITESET_RECORD: node_id={} marked for rollback cleanup",
            node_id
        );
    }

    /// Log Phase 75 rollback cleanup operations
    #[cfg(feature = "trace_v2_io")]
    pub fn log_rollback_cleanup(message: &str) {
        println!("[phase75] ROLLBACK_CLEANUP: {}", message);
    }

    /// Log V2 header initialization
    pub fn log_v2_header_initialization() {
        println!(
            "[CLUSTER_DEBUG] initialize_v2_header() called - fixing cluster offsets to prevent node slot corruption"
        );
    }

    /// Get V2 magic bytes for header initialization
    pub fn get_v2_magic() -> [u8; 8] {
        V2_MAGIC
    }

    /// Get V2 format version for header initialization
    pub fn get_v2_format_version() -> u32 {
        V2_FORMAT_VERSION
    }
}

/// Convenience functions for common debug operations
pub mod convenience {
    use super::*;

    /// TX_BEGIN_AUDIT wrapper for common case
    pub fn audit_transaction_begin(
        enabled: bool,
        file_path: &std::path::Path,
        node_data_offset: u64,
        node_id: u64,
        label: &str,
        read_bytes_fn: &mut dyn FnMut(u64, &mut [u8]) -> NativeResult<()>,
    ) -> NativeResult<()> {
        if enabled {
            let slot_offset = node_data_offset + ((node_id - 1) as u64 * 4096);
            let mut buffer = vec![0u8; 32];

            if read_bytes_fn(slot_offset, &mut buffer).is_ok() {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} first_32={:02x?} version={}",
                    label, node_id, slot_offset, &buffer, buffer[0]
                );
            } else {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} READ_FAILED",
                    label, node_id, slot_offset
                );
            }
        }
        Ok(())
    }

    /// EDGE_CLUSTER_DEBUG wrapper for node state inspection
    pub fn debug_edge_cluster_state(
        enabled: bool,
        file_path: &std::path::Path,
        operation: &str,
        read_bytes_fn: &mut dyn FnMut(u64, &mut [u8]) -> NativeResult<()>,
        file_size_fn: &dyn Fn() -> NativeResult<u64>,
    ) -> NativeResult<()> {
        if enabled {
            let mut node1_bytes = vec![0u8; 32];
            if read_bytes_fn(0x400, &mut node1_bytes).is_ok() {
                let version = node1_bytes[0];
                let file_size = file_size_fn()?;
                println!(
                    "[EDGE_CLUSTER_DEBUG] {}: node1_version={}, file_size={}, node1_bytes={:02x?}",
                    operation, version, file_size, &node1_bytes
                );
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_transaction_phase() {
        // This test just ensures the function doesn't panic
        DebugInstrumentation::log_transaction_phase("begun", 12345);
        DebugInstrumentation::log_transaction_phase("committed", 12345);
    }

    #[test]
    fn test_log_cluster_offset_fix() {
        // This test just ensures the function doesn't panic
        DebugInstrumentation::log_cluster_offset_fix("outgoing", 1000, 2000);
        DebugInstrumentation::log_cluster_offset_fix("incoming", 3000, 4000);
    }

    #[test]
    fn test_convenience_audit_transaction_begin_disabled() {
        let file_path = std::path::Path::new("/tmp/test");
        let node_data_offset = 1024;
        let node_id = 257;
        let label = "TEST";

        let mut call_count = 0;
        let mut mock_read_fn = |_: u64, _: &mut [u8]| {
            call_count += 1;
            Ok(())
        };

        let result = convenience::audit_transaction_begin(
            false, // disabled
            file_path,
            node_data_offset,
            node_id,
            label,
            &mut mock_read_fn,
        );

        assert!(result.is_ok());
        assert_eq!(call_count, 0); // Should not be called when disabled
    }

    #[test]
    fn test_convenience_debug_edge_cluster_state_disabled() {
        let file_path = std::path::Path::new("/tmp/test");
        let operation = "TEST_OPERATION";

        let mut read_call_count = 0;
        let mut mock_read_fn = |_: u64, _: &mut [u8]| {
            read_call_count += 1;
            Ok(())
        };

        let result = convenience::debug_edge_cluster_state(
            false, // disabled
            file_path,
            operation,
            &mut mock_read_fn,
            &|| Ok(4096), // Simple closure that implements Fn
        );

        assert!(result.is_ok());
        assert_eq!(read_call_count, 0); // Should not be called when disabled
    }
}