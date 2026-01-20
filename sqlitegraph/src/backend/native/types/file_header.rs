//! File header structure and management for native graph database


use super::super::constants;
use super::super::v2::{V2_FORMAT_VERSION, V2_MAGIC};
use super::NativeBackendError;

/// File header structure for native graph database
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHeader {
    /// Magic number (should be "SQLTGF\0")
    pub magic: [u8; 8],
    /// File format version
    pub version: u32,
    /// Feature flags bitfield
    pub flags: u32,
    /// Total number of nodes in the file
    pub node_count: u64,
    /// Total number of edges in the file
    pub edge_count: u64,
    /// Schema version (u32 in v3 format)
    pub schema_version: u32,
    /// Reserved field (4 bytes for future use in v3 format)
    pub reserved: u32,
    /// Offset to node data section
    pub node_data_offset: u64,
    /// Offset to edge data section or outgoing clusters begin
    pub edge_data_offset: u64,
    /// V2: Offset where outgoing edge clusters begin
    pub outgoing_cluster_offset: u64,
    /// V2: Offset where incoming edge clusters begin
    pub incoming_cluster_offset: u64,
    /// V2: Offset where free space management begins
    pub free_space_offset: u64,
    /// V2 Atomic Commit: Previous outgoing cluster offset for rollback
    pub tx_prev_outgoing_cluster_offset: u64,
    /// V2 Atomic Commit: Previous incoming cluster offset for rollback
    pub tx_prev_incoming_cluster_offset: u64,
    /// V2 Atomic Commit: Previous free space offset for rollback
    pub tx_prev_free_space_offset: u64,
    /// V2 Atomic Commit: Transaction identifier for crash detection
    pub tx_id: u64,
    /// Header checksum
    pub checksum: u64,
}

impl FileHeader {
    /// Create a new header with default values
    pub fn new() -> Self {
        Self {
            magic: V2_MAGIC,            // V2 format by default
            version: V2_FORMAT_VERSION, // V3 format (updated from 2 to 3)
            flags: constants::DEFAULT_FEATURE_FLAGS,
            node_count: 0,
            edge_count: 0,
            schema_version: constants::DEFAULT_SCHEMA_VERSION,
            reserved: 0,
            node_data_offset: constants::HEADER_SIZE,
            edge_data_offset: constants::HEADER_SIZE,
            outgoing_cluster_offset: 0,
            incoming_cluster_offset: 0,
            free_space_offset: 0,
            tx_prev_outgoing_cluster_offset: 0,
            tx_prev_incoming_cluster_offset: 0,
            tx_prev_free_space_offset: 0,
            tx_id: 0,
            checksum: 0,
        }
    }

    /// Validate the header for consistency
    pub fn validate(&self) -> Result<(), NativeBackendError> {
        // Check magic number
        if self.magic != constants::MAGIC_BYTES {
            return Err(NativeBackendError::InvalidMagic {
                expected: u64::from_be_bytes(constants::MAGIC_BYTES),
                found: u64::from_be_bytes(self.magic),
            });
        }

        // Check version
        if self.version != constants::FILE_FORMAT_VERSION && self.version != 2 && self.version != 3 {
            return Err(NativeBackendError::UnsupportedVersion {
                version: self.version,
                supported_version: constants::FILE_FORMAT_VERSION,
            });
        }

        // Check offset ordering
        if self.node_data_offset < constants::HEADER_SIZE {
            return Err(NativeBackendError::InvalidHeader {
                field: "node_data_offset".to_string(),
                reason: "must be >= header_size".to_string(),
            });
        }

        if self.edge_data_offset < self.node_data_offset {
            return Err(NativeBackendError::InvalidHeader {
                field: "edge_data_offset".to_string(),
                reason: "must be >= node_data_offset".to_string(),
            });
        }

        // For V2 format, validate additional fields
        if self.version == 2 {
            if self.outgoing_cluster_offset > 0
                && self.outgoing_cluster_offset < self.node_data_offset
            {
                return Err(NativeBackendError::InvalidHeader {
                    field: "outgoing_cluster_offset".to_string(),
                    reason: "must be >= node_data_offset".to_string(),
                });
            }

            if self.incoming_cluster_offset > 0
                && self.incoming_cluster_offset < self.outgoing_cluster_offset
            {
                // HEADER_VALIDATE_DEBUG instrumentation
                if std::env::var("HEADER_VALIDATE_DEBUG").is_ok() {
                    println!(
                        "[HEADER_VALIDATE_DEBUG] FAIL: incoming_cluster_offset ({}) < outgoing_cluster_offset ({})",
                        self.incoming_cluster_offset, self.outgoing_cluster_offset
                    );
                    println!(
                        "[HEADER_VALIDATE_DEBUG] Validation site: {}:{}",
                        file!(),
                        line!()
                    );

                    // Read first 1024 bytes of the file for raw evidence
                    if let Ok(file_path) = std::env::var("HEADER_VALIDATE_DEBUG_FILE") {
                        if let Ok(mut file) = std::fs::File::open(&file_path) {
                            use std::io::Read;
                            let mut buffer = vec![0u8; 1024];
                            if let Ok(_) = file.read_exact(&mut buffer) {
                                println!(
                                    "[HEADER_VALIDATE_DEBUG] First 88 bytes (full header): {:02x?}",
                                    &buffer[..88]
                                );

                                // Extract the offset fields from raw bytes using actual struct layout:
                                // magic: 8 bytes (0-7), version: 4 bytes (8-11), flags: 4 bytes (12-15)
                                // node_count: 8 bytes (16-23), edge_count: 8 bytes (24-31), schema_version: 8 bytes (32-39)
                                // node_data_offset: 8 bytes (40-47), edge_data_offset: 8 bytes (48-55)
                                // outgoing_cluster_offset: 8 bytes (56-63), incoming_cluster_offset: 8 bytes (64-71)
                                let incoming_offset_bytes = &buffer[64..72];
                                let outgoing_offset_bytes = &buffer[56..64];
                                println!(
                                    "[HEADER_VALIDATE_DEBUG] Raw incoming_offset bytes (64-71): {:02x?}",
                                    incoming_offset_bytes
                                );
                                println!(
                                    "[HEADER_VALIDATE_DEBUG] Raw outgoing_offset bytes (56-63): {:02x?}",
                                    outgoing_offset_bytes
                                );

                                // Parse as big-endian u64 for verification
                                let raw_incoming = u64::from_be_bytes([
                                    incoming_offset_bytes[0],
                                    incoming_offset_bytes[1],
                                    incoming_offset_bytes[2],
                                    incoming_offset_bytes[3],
                                    incoming_offset_bytes[4],
                                    incoming_offset_bytes[5],
                                    incoming_offset_bytes[6],
                                    incoming_offset_bytes[7],
                                ]);
                                let raw_outgoing = u64::from_be_bytes([
                                    outgoing_offset_bytes[0],
                                    outgoing_offset_bytes[1],
                                    outgoing_offset_bytes[2],
                                    outgoing_offset_bytes[3],
                                    outgoing_offset_bytes[4],
                                    outgoing_offset_bytes[5],
                                    outgoing_offset_bytes[6],
                                    outgoing_offset_bytes[7],
                                ]);
                                println!(
                                    "[HEADER_VALIDATE_DEBUG] Raw parsed incoming: {}, outgoing: {}",
                                    raw_incoming, raw_outgoing
                                );
                                println!(
                                    "[HEADER_VALIDATE_DEBUG] Comparison result: {} < {} = {}",
                                    raw_incoming,
                                    raw_outgoing,
                                    raw_incoming < raw_outgoing
                                );
                            }
                        }
                    }
                }

                return Err(NativeBackendError::InvalidHeader {
                    field: "incoming_cluster_offset".to_string(),
                    reason: "must be >= outgoing_cluster_offset".to_string(),
                });
            }

            if self.free_space_offset > 0 && self.free_space_offset < self.incoming_cluster_offset {
                return Err(NativeBackendError::InvalidHeader {
                    field: "free_space_offset".to_string(),
                    reason: "must be >= incoming_cluster_offset".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Compute checksum for the header
    pub fn compute_checksum(&self) -> u64 {
        let mut checksum = constants::checksum::XOR_SEED;

        // Simple XOR checksum over all fields except checksum itself
        checksum ^= u64::from_be_bytes(self.magic);
        checksum ^= self.version as u64;
        checksum ^= self.flags as u64;
        checksum ^= self.node_count;
        checksum ^= self.edge_count;
        checksum ^= self.schema_version as u64;
        checksum ^= self.reserved as u64;
        checksum ^= self.node_data_offset;
        checksum ^= self.edge_data_offset;
        checksum ^= self.outgoing_cluster_offset;
        checksum ^= self.incoming_cluster_offset;
        checksum ^= self.free_space_offset;
        checksum ^= self.tx_prev_outgoing_cluster_offset;
        checksum ^= self.tx_prev_incoming_cluster_offset;
        checksum ^= self.tx_prev_free_space_offset;
        checksum ^= self.tx_id;

        checksum
    }

    /// Update the checksum field
    pub fn update_checksum(&mut self) {
        self.checksum = self.compute_checksum();
    }

    /// Verify the checksum field
    pub fn verify_checksum(&self) -> Result<(), NativeBackendError> {
        let expected_checksum = self.compute_checksum();
        if self.checksum != expected_checksum {
            return Err(NativeBackendError::InvalidChecksum {
                expected: expected_checksum,
                found: self.checksum,
            });
        }
        Ok(())
    }

    /// Get transaction state from flags
    pub fn get_tx_state(&self) -> u32 {
        self.flags & constants::TX_STATE_MASK
    }

    /// Check if transaction is in progress
    pub fn is_tx_in_progress(&self) -> bool {
        self.get_tx_state() == constants::TX_STATE_IN_PROGRESS
    }

    /// Set transaction state
    pub fn set_tx_state(&mut self, state: u32) {
        self.flags = (self.flags & !constants::TX_STATE_MASK) | (state & constants::TX_STATE_MASK);
    }

    /// Begin transaction: save current state and set IN_PROGRESS
    pub fn begin_tx(&mut self, next_tx_id: u64) {
        self.tx_prev_outgoing_cluster_offset = self.outgoing_cluster_offset;
        self.tx_prev_incoming_cluster_offset = self.incoming_cluster_offset;
        self.tx_prev_free_space_offset = self.free_space_offset;
        self.tx_id = next_tx_id;
        self.set_tx_state(constants::TX_STATE_IN_PROGRESS);
    }

    /// Commit transaction: clear transaction state
    pub fn commit_tx(&mut self) {
        self.set_tx_state(constants::TX_STATE_CLEAN);
    }

    /// Rollback transaction: restore previous offsets
    pub fn rollback_tx(&mut self) {
        self.outgoing_cluster_offset = self.tx_prev_outgoing_cluster_offset;
        self.incoming_cluster_offset = self.tx_prev_incoming_cluster_offset;
        self.free_space_offset = self.tx_prev_free_space_offset;
        self.set_tx_state(constants::TX_STATE_CLEAN);
    }
}
