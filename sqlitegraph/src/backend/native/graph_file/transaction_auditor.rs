//! Transaction auditor and node modification tracking module
//!
//! This module provides functionality for tracking node modifications during transactions
//! and providing audit trails for debugging and analysis.

use crate::backend::native::types::NativeNodeId;
use std::collections::HashSet;

/// Transaction auditor for tracking node modifications and providing audit trails
pub struct TransactionAuditor {
    /// Set of nodes whose V2 cluster metadata has been modified during current transaction
    tx_modified_nodes: HashSet<NativeNodeId>,
    /// Flag indicating if transaction begin audit is enabled
    tx_begin_audit_enabled: bool,
    /// Flag indicating if phase 75 instrumentation is enabled
    phase75_instrumentation_enabled: bool,
    /// Flag indicating if edge cluster debug is enabled
    edge_cluster_debug_enabled: bool,
}

impl TransactionAuditor {
    /// Create a new transaction auditor
    pub fn new() -> Self {
        Self {
            tx_modified_nodes: HashSet::new(),
            tx_begin_audit_enabled: std::env::var("TX_BEGIN_AUDIT").is_ok(),
            phase75_instrumentation_enabled: std::env::var("PHASE75_INSTRUMENTATION").is_ok(),
            edge_cluster_debug_enabled: std::env::var("EDGE_CLUSTER_DEBUG").is_ok(),
        }
    }

    /// Record that a node's V2 cluster metadata has been modified during transaction
    ///
    /// # Arguments
    /// * `node_id` - The ID of the modified node
    pub fn record_node_v2_cluster_modified(&mut self, node_id: NativeNodeId) {
        self.tx_modified_nodes.insert(node_id);

        #[cfg(feature = "trace_v2_io")]
        if self.phase75_instrumentation_enabled {
            println!(
                "[phase75] WRITESET_RECORD: node_id={} marked for rollback cleanup",
                node_id
            );
        }
    }

    /// Check if a node has been modified during the current transaction
    ///
    /// # Arguments
    /// * `node_id` - The ID of the node to check
    ///
    /// # Returns
    /// `true` if the node has been modified, `false` otherwise
    pub fn is_node_modified(&self, node_id: NativeNodeId) -> bool {
        self.tx_modified_nodes.contains(&node_id)
    }

    /// Get all nodes that have been modified during the current transaction
    ///
    /// # Returns
    /// A vector containing all modified node IDs
    pub fn get_modified_nodes(&self) -> Vec<NativeNodeId> {
        self.tx_modified_nodes.iter().copied().collect()
    }

    /// Get the count of modified nodes during the current transaction
    ///
    /// # Returns
    /// The number of nodes that have been modified
    pub fn modified_node_count(&self) -> usize {
        self.tx_modified_nodes.len()
    }

    /// Clear all transaction modification tracking
    ///
    /// This should be called after transaction commit or rollback
    pub fn clear_modified_nodes(&mut self) {
        #[cfg(feature = "trace_v2_io")]
        if self.phase75_instrumentation_enabled {
            println!("[phase75] ROLLBACK_CLEANUP: Clearing transaction modification tracking");
        }

        self.tx_modified_nodes.clear();
    }

    /// Perform transaction begin audit for node 257 slot
    ///
    /// # Arguments
    /// * `node_data_offset` - Offset to node data region
    /// * `read_bytes_fn` - Function to read bytes from file
    pub fn audit_transaction_begin<F>(&self, node_data_offset: u64, read_bytes_fn: F) -> NativeResult<()>
    where
        F: FnOnce(u64, &mut [u8]) -> NativeResult<()>,
    {
        if !self.tx_begin_audit_enabled {
            return Ok(());
        }

        const AUDIT_NODE_ID: NativeNodeId = 257;
        let slot_offset = node_data_offset + ((AUDIT_NODE_ID - 1) as u64 * 4096);
        let mut buffer = vec![0u8; 32];

        match read_bytes_fn(slot_offset, &mut buffer) {
            Ok(_) => {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} first_32={:02x?} version={}",
                    "BEFORE_TX_BEGIN", AUDIT_NODE_ID, slot_offset, &buffer, buffer[0]
                );
            }
            Err(_) => {
                println!(
                    "[TX_BEGIN_AUDIT] {} node_id={} slot_offset=0x{:x} READ_FAILED",
                    "BEFORE_TX_BEGIN", AUDIT_NODE_ID, slot_offset
                );
            }
        }

        Ok(())
    }

    /// Perform edge cluster debug audit before transaction operations
    ///
    /// # Arguments
    /// * `file_path` - Path to the graph file
    /// * `file_size_fn` - Function to get current file size
    pub fn debug_edge_cluster_before_transaction<F>(&self, file_path: &std::path::Path, file_size_fn: F) -> NativeResult<()>
    where
        F: FnOnce() -> NativeResult<u64>,
    {
        if !self.edge_cluster_debug_enabled {
            return Ok(());
        }

        const NODE1_SLOT_OFFSET: u64 = 0x400;
        let mut disk_file = std::fs::File::open(file_path)?;
        let mut node1_bytes = vec![0u8; 32];

        use std::io::{Seek, Read};
        disk_file.seek(std::io::SeekFrom::Start(NODE1_SLOT_OFFSET))?;
        disk_file.read_exact(&mut node1_bytes)?;

        let version_before_tx_ops = node1_bytes[0];
        let file_size_before_tx_ops = file_size_fn().unwrap_or(0);

        println!(
            "[EDGE_CLUSTER_DEBUG] BEFORE_TX_OPS: node1_version={}, file_size={}, node1_bytes={:02x?}",
            version_before_tx_ops, file_size_before_tx_ops, &node1_bytes
        );

        Ok(())
    }

    /// Clear V2 cluster metadata on rollback with corruption prevention
    ///
    /// Phase 75: CRITICAL FIX - Skip V2 node slot rewriting during rollback to prevent corruption
    pub fn clear_v2_cluster_metadata_on_rollback(&mut self) -> NativeResult<()> {
        #[cfg(feature = "trace_v2_io")]
        if self.phase75_instrumentation_enabled {
            println!("[phase75] ROLLBACK_CLEANUP: SKIPPING V2 node slot rewrite to prevent corruption");
        }

        // CRITICAL FIX: Do NOT rewrite V2 node slots during rollback
        // This prevents corruption of V2 format (version=2 -> version=1)

        // Just clear the transaction tracking
        self.clear_modified_nodes();

        #[cfg(feature = "trace_v2_io")]
        if self.phase75_instrumentation_enabled {
            println!("[phase75] ROLLBACK_CLEANUP: Completed without V2 slot corruption");
        }

        Ok(())
    }

    /// Generate audit report for current transaction state
    ///
    /// # Returns
    /// A formatted audit report string
    pub fn generate_audit_report(&self) -> String {
        let mut report = String::new();
        report.push_str("=== Transaction Audit Report ===\n");
        report.push_str(&format!("Modified nodes: {}\n", self.modified_node_count()));

        if !self.tx_modified_nodes.is_empty() {
            report.push_str("Modified node IDs: ");
            let mut node_ids: Vec<_> = self.tx_modified_nodes.iter().copied().collect();
            node_ids.sort();
            for (i, node_id) in node_ids.iter().enumerate() {
                if i > 0 {
                    report.push_str(", ");
                }
                report.push_str(&node_id.to_string());
            }
            report.push('\n');
        }

        report.push_str(&format!("TX_BEGIN_AUDIT enabled: {}\n", self.tx_begin_audit_enabled));
        report.push_str(&format!("PHASE75_INSTRUMENTATION enabled: {}\n", self.phase75_instrumentation_enabled));
        report.push_str(&format!("EDGE_CLUSTER_DEBUG enabled: {}\n", self.edge_cluster_debug_enabled));

        report
    }

    /// Check if any debugging features are enabled
    ///
    /// # Returns
    /// `true` if any debugging features are enabled
    pub fn has_debugging_enabled(&self) -> bool {
        self.tx_begin_audit_enabled || self.phase75_instrumentation_enabled || self.edge_cluster_debug_enabled
    }

    /// Get transaction auditor statistics
    ///
    /// # Returns
    /// Transaction auditor statistics
    pub fn get_statistics(&self) -> TransactionAuditorStatistics {
        TransactionAuditorStatistics {
            modified_node_count: self.modified_node_count(),
            tx_begin_audit_enabled: self.tx_begin_audit_enabled,
            phase75_instrumentation_enabled: self.phase75_instrumentation_enabled,
            edge_cluster_debug_enabled: self.edge_cluster_debug_enabled,
            has_debugging_enabled: self.has_debugging_enabled(),
        }
    }
}

/// Statistics for the transaction auditor
#[derive(Debug, Clone)]
pub struct TransactionAuditorStatistics {
    /// Number of nodes modified during current transaction
    pub modified_node_count: usize,
    /// Whether TX_BEGIN_AUDIT is enabled
    pub tx_begin_audit_enabled: bool,
    /// Whether PHASE75_INSTRUMENTATION is enabled
    pub phase75_instrumentation_enabled: bool,
    /// Whether EDGE_CLUSTER_DEBUG is enabled
    pub edge_cluster_debug_enabled: bool,
    /// Whether any debugging features are enabled
    pub has_debugging_enabled: bool,
}

impl Default for TransactionAuditor {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export for backward compatibility
pub use crate::backend::native::types::NativeResult;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn test_transaction_auditor_creation() {
        let auditor = TransactionAuditor::new();
        assert_eq!(auditor.modified_node_count(), 0);
        assert!(!auditor.has_debugging_enabled()); // Usually false in test environment
    }

    #[test]
    fn test_node_modification_tracking() {
        let mut auditor = TransactionAuditor::new();

        // Initially no modifications
        assert_eq!(auditor.modified_node_count(), 0);
        assert!(!auditor.is_node_modified(100));

        // Record modifications
        auditor.record_node_v2_cluster_modified(100);
        auditor.record_node_v2_cluster_modified(200);

        // Check modifications
        assert_eq!(auditor.modified_node_count(), 2);
        assert!(auditor.is_node_modified(100));
        assert!(auditor.is_node_modified(200));
        assert!(!auditor.is_node_modified(300));

        let modified_nodes = auditor.get_modified_nodes();
        assert_eq!(modified_nodes.len(), 2);
        assert!(modified_nodes.contains(&100));
        assert!(modified_nodes.contains(&200));
    }

    #[test]
    fn test_clear_modified_nodes() {
        let mut auditor = TransactionAuditor::new();

        auditor.record_node_v2_cluster_modified(100);
        auditor.record_node_v2_cluster_modified(200);

        assert_eq!(auditor.modified_node_count(), 2);

        auditor.clear_modified_nodes();

        assert_eq!(auditor.modified_node_count(), 0);
        assert!(!auditor.is_node_modified(100));
        assert!(!auditor.is_node_modified(200));
    }

    #[test]
    fn test_audit_report_generation() {
        let mut auditor = TransactionAuditor::new();

        // Empty report
        let empty_report = auditor.generate_audit_report();
        assert!(empty_report.contains("Modified nodes: 0"));

        // Report with modifications
        auditor.record_node_v2_cluster_modified(100);
        auditor.record_node_v2_cluster_modified(50);
        auditor.record_node_v2_cluster_modified(200);

        let report = auditor.generate_audit_report();
        assert!(report.contains("Modified nodes: 3"));
        assert!(report.contains("50, 100, 200")); // Should be sorted
    }

    #[test]
    fn test_statistics() {
        let mut auditor = TransactionAuditor::new();

        auditor.record_node_v2_cluster_modified(100);
        auditor.record_node_v2_cluster_modified(200);

        let stats = auditor.get_statistics();
        assert_eq!(stats.modified_node_count, 2);
        assert_eq!(stats.tx_begin_audit_enabled, auditor.tx_begin_audit_enabled);
        assert_eq!(stats.phase75_instrumentation_enabled, auditor.phase75_instrumentation_enabled);
        assert_eq!(stats.edge_cluster_debug_enabled, auditor.edge_cluster_debug_enabled);
    }

    #[test]
    fn test_audit_transaction_begin_disabled() {
        let auditor = TransactionAuditor::new();

        // Should not panic when audit is disabled
        let result = auditor.audit_transaction_begin(1024, |_, _| Ok(()));
        assert!(result.is_ok());
    }

    #[test]
    fn test_debug_edge_cluster_disabled() {
        let auditor = TransactionAuditor::new();
        let temp_file = NamedTempFile::new().unwrap();

        // Should not panic when debug is disabled
        let result = auditor.debug_edge_cluster_before_transaction(temp_file.path(), || Ok(1024));
        assert!(result.is_ok());
    }

    #[test]
    fn test_clear_v2_cluster_metadata_on_rollback() {
        let mut auditor = TransactionAuditor::new();

        auditor.record_node_v2_cluster_modified(100);
        auditor.record_node_v2_cluster_modified(200);

        assert_eq!(auditor.modified_node_count(), 2);

        let result = auditor.clear_v2_cluster_metadata_on_rollback();
        assert!(result.is_ok());
        assert_eq!(auditor.modified_node_count(), 0);
    }
}