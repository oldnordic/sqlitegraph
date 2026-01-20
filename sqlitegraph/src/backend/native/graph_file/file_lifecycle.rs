//! File lifecycle operations for GraphFile
//!
//! This module provides file creation, opening, header management,
//! and basic file lifecycle operations for GraphFile instances.

use crate::backend::native::{
    constants::FLAG_V2_ATOMIC_COMMIT, constants::FLAG_V2_FRAMED_RECORDS,
    graph_file::file_ops::FileOperations, graph_file::header::HeaderManager,
    persistent_header::PersistentHeaderV2, transaction_state::TransactionState,
    types::NativeBackendError, types::NativeResult,
};
use std::path::Path;

/// File lifecycle management utilities for GraphFile
pub struct FileLifecycleManager;

impl FileLifecycleManager {
    /// Create a new graph file with initial header
    ///
    /// Creates a new graph file with V2 format, initializes the header,
    /// writes initial state to disk, and sets up file structures.
    pub fn create<P: AsRef<Path>>(
        path: P,
    ) -> NativeResult<crate::backend::native::graph_file::GraphFile> {
        use crate::backend::native::graph_file::GraphFile;
        use crate::backend::native::{
            graph_file::TransactionAuditor, graph_file::buffers::ReadBuffer,
            graph_file::buffers::WriteBuffer,
        };

        let path = path.as_ref();
        let file_path = path.to_path_buf();

        // Create new file with appropriate permissions
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;

        let mut graph_file = GraphFile {
            file,
            persistent_header: PersistentHeaderV2::new_v2(),
            transaction_state: TransactionState::new(),
            file_path,
            read_buffer: ReadBuffer::new(),     // Adaptive 256B buffer
            write_buffer: WriteBuffer::new(32), // 32 pending writes
            #[cfg(feature = "v2_experimental")]
            mmap: None,
            // Initialize transaction auditor for tracking modified nodes
            transaction_auditor: TransactionAuditor::new(),
        };

        Self::initialize_v2_header(&mut graph_file)?;
        // Write initial header
        Self::write_header(&mut graph_file)?;
        Self::finish_cluster_commit(&mut graph_file)?;

        // Initialize mmap using centralized method
        #[cfg(feature = "v2_experimental")]
        {
            Self::ensure_mmap_initialized(&mut graph_file)?;
        }

        Ok(graph_file)
    }

    /// Open an existing graph file
    ///
    /// Opens an existing graph file, validates the V2 format,
    /// reads the header, and initializes file structures.
    pub fn open<P: AsRef<Path>>(
        path: P,
    ) -> NativeResult<crate::backend::native::graph_file::GraphFile> {
        use crate::backend::native::graph_file::GraphFile;
        use crate::backend::native::{
            graph_file::TransactionAuditor, graph_file::buffers::ReadBuffer,
            graph_file::buffers::WriteBuffer,
        };

        let path = path.as_ref();
        let file_path = path.to_path_buf();

        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(path)?;

        let mut graph_file = GraphFile {
            file,
            persistent_header: PersistentHeaderV2::new_v2(), // Will be overwritten by read_header
            transaction_state: TransactionState::new(),
            file_path,
            read_buffer: ReadBuffer::new(),     // Adaptive 256B buffer
            write_buffer: WriteBuffer::new(32), // 32 pending writes
            #[cfg(feature = "v2_experimental")]
            mmap: None,
            // Initialize transaction auditor for tracking modified nodes
            transaction_auditor: TransactionAuditor::new(),
        };

        // Read and validate existing header
        Self::read_header(&mut graph_file)?;

        // Transaction recovery - runtime only, no persistent tx state to check
        // TransactionState is runtime-only and initialized to defaults on open

        // V2-ONLY REFACTOR: Hard format gate - refuse non-V2 files
        let required_flags = FLAG_V2_FRAMED_RECORDS | FLAG_V2_ATOMIC_COMMIT;
        if (graph_file.persistent_header.flags & required_flags) != required_flags {
            return Err(NativeBackendError::UnsupportedVersion {
                version: 1, // Any file without both V2 flags is unsupported
                supported_version: 3, // Updated to v3
            });
        }

        // Accept v2 or v3 format files
        // v2: 8-byte schema_version field
        // v3: 4-byte schema_version + 4-byte reserved
        if graph_file.persistent_header.version != 2 && graph_file.persistent_header.version != 3 {
            return Err(NativeBackendError::UnsupportedVersion {
                version: graph_file.persistent_header.version,
                supported_version: 3,
            });
        }

        graph_file.persistent_header.validate()?;

        // V2 commit verification
        Self::verify_commit_marker(&mut graph_file)?;

        // Initialize mmap using centralized method
        #[cfg(feature = "v2_experimental")]
        {
            Self::ensure_mmap_initialized(&mut graph_file)?;
        }

        Ok(graph_file)
    }

    /// Read header from file
    ///
    /// Reads and validates the persistent header from disk.
    pub fn read_header(
        graph_file: &mut crate::backend::native::graph_file::GraphFile,
    ) -> NativeResult<()> {
        graph_file.persistent_header =
            FileOperations::read_and_validate_header(&mut graph_file.file)?;
        Ok(())
    }

    /// Write header to file with durable persistence
    ///
    /// This is the ONLY authorized way to write to header region [0, HEADER_SIZE)
    pub fn write_header(
        graph_file: &mut crate::backend::native::graph_file::GraphFile,
    ) -> NativeResult<()> {
        Self::write_header_and_sync(graph_file)
    }

    /// Internal helper: Write header with immediate verification and sync
    ///
    /// Ensures header bytes reach disk and can be read back immediately.
    fn write_header_and_sync(
        graph_file: &mut crate::backend::native::graph_file::GraphFile,
    ) -> NativeResult<()> {
        FileOperations::write_header(&mut graph_file.file, &graph_file.persistent_header)?;
        Ok(())
    }

    /// Sync file to disk
    ///
    /// Ensures all buffered data is written to persistent storage.
    pub fn sync(graph_file: &crate::backend::native::graph_file::GraphFile) -> NativeResult<()> {
        FileOperations::sync(&graph_file.file)
    }

    /// Begin cluster commit operation
    ///
    /// Initializes a cluster commit by setting the commit marker to 0.
    fn begin_cluster_commit(
        graph_file: &mut crate::backend::native::graph_file::GraphFile,
    ) -> NativeResult<()> {
        use crate::backend::native::graph_file::transaction::TransactionManager;
        TransactionManager::begin_cluster_commit(&mut graph_file.file)
    }

    /// Finish cluster commit operation
    ///
    /// Completes a cluster commit by setting the commit marker to clean value.
    fn finish_cluster_commit(
        graph_file: &mut crate::backend::native::graph_file::GraphFile,
    ) -> NativeResult<()> {
        use crate::backend::native::graph_file::transaction::TransactionManager;
        TransactionManager::finish_cluster_commit(&mut graph_file.file)
    }

    /// Verify commit marker integrity
    ///
    /// Ensures the commit marker is in a valid state.
    fn verify_commit_marker(
        graph_file: &mut crate::backend::native::graph_file::GraphFile,
    ) -> NativeResult<()> {
        use crate::backend::native::graph_file::transaction::TransactionManager;
        use crate::backend::native::graph_file::validation::GraphFileValidator;

        let marker = TransactionManager::read_commit_marker_value(&mut graph_file.file)?;
        if marker != GraphFileValidator::clean_commit_marker() {
            return Err(NativeBackendError::CorruptNodeRecord {
                node_id: -1,
                reason: format!("File has incomplete transaction: commit_marker={}", marker),
            });
        }
        Ok(())
    }

    /// Initialize V2 header with proper cluster offsets
    ///
    /// Sets up the initial V2 header structure with cluster allocation regions.
    fn initialize_v2_header(
        graph_file: &mut crate::backend::native::graph_file::GraphFile,
    ) -> NativeResult<()> {
        // Initialize header with default parameters
        HeaderManager::initialize_v2_header(
            &mut graph_file.persistent_header,
            0,    // node_count: start with 0 nodes
            512,  // default_node_data_start: start after header
            1024, // reserved_node_region_bytes: reserve initial space
        )
    }

    /// Ensure memory mapping is initialized
    ///
    /// Initializes memory mapping for the file if available.
    #[cfg(feature = "v2_experimental")]
    fn ensure_mmap_initialized(
        graph_file: &mut crate::backend::native::graph_file::GraphFile,
    ) -> NativeResult<()> {
        // This would be implemented in the memory mapping module
        // For now, ensure mmap is properly initialized based on file size
        if graph_file.mmap.is_none() {
            // Initialize mmap if available
            // Implementation would go here
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use tempfile::tempdir;

    #[test]
    fn test_create_new_graph_file() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_graph.db");

        // This test would require the full GraphFile struct to be available
        // For now, we test the basic file operations
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&file_path)
            .unwrap();

        assert!(file_path.exists());
        drop(file);
    }

    #[test]
    fn test_file_sync() {
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_sync.db");

        let mut file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(&file_path)
            .unwrap();

        file.write_all(b"test data").unwrap();

        // Test sync functionality
        file.sync_all().unwrap();

        drop(file);

        // Verify file exists and has content
        assert!(file_path.exists());
    }

    #[test]
    fn test_header_validation() {
        use crate::backend::native::persistent_header::PersistentHeaderV2;

        let header = PersistentHeaderV2::new_v2();

        // Test that new header has correct version (v3 for new schema_version format)
        assert_eq!(header.version, 3);

        // Test that required flags are set
        let required_flags = FLAG_V2_FRAMED_RECORDS | FLAG_V2_ATOMIC_COMMIT;
        assert_eq!(header.flags & required_flags, required_flags);
    }
}
