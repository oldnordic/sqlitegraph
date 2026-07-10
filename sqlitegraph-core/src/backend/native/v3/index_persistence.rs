//! Index Persistence for V3 Native Backend
//!
//! This module provides functionality to persist and restore the kind and name indexes
//! to avoid O(N) page scan during database open.
//!
//! ## Format
//!
//! The `.v3index` file format:
//! - magic[4]: "V3XI" (V3 eXlude Index)
//! - version[4]: Version number (currently 1)
//! - db_node_count[8]: Node count from DB header when index was persisted (staleness guard)
//! - kind_count[4]: Number of unique kinds
//! - kind_entries: Variable length
//!   - For each kind:
//!     - kind_len[4]: Length of kind string
//!     - kind_bytes: Kind string data
//!     - node_count[4]: Number of nodes with this kind
//!     - node_ids: Node IDs (8 bytes each)
//! - name_count[4]: Number of unique names
//! - name_entries: Variable length
//!   - For each name:
//!     - name_len[4]: Length of name string
//!     - name_bytes: Name string data
//!     - node_count[4]: Number of nodes with this name
//!     - node_ids: Node IDs (8 bytes each)

use crate::backend::native::v3::kind_index::KindIndex;
use crate::backend::native::v3::name_index::NameIndex;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;

mod support;

/// Magic number for the index file
pub const INDEX_MAGIC: &[u8; 4] = b"V3XI";
/// Current version of the index file format
pub const INDEX_VERSION: u32 = 1;

/// Error type for index persistence operations
#[derive(Debug, Clone)]
pub enum IndexPersistenceError {
    Io(String),
    InvalidMagic(Vec<u8>),
    UnsupportedVersion(u32),
    Corrupted(String),
}

impl std::fmt::Display for IndexPersistenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "IO error: {}", msg),
            Self::InvalidMagic(bytes) => write!(f, "Invalid magic: {:?}", bytes),
            Self::UnsupportedVersion(v) => write!(f, "Unsupported version: {}", v),
            Self::Corrupted(msg) => write!(f, "Corrupted data: {}", msg),
        }
    }
}

impl std::error::Error for IndexPersistenceError {}

/// Persist the kind and name indexes to a sidecar file
///
/// # Arguments
/// * `db_path` - Path to the main database file
/// * `kind_index` - The kind index to persist
/// * `name_index` - The name index to persist
/// * `db_node_count` - Node count from DB header (for staleness detection on restore)
///
/// # Returns
/// Ok(()) if persistence succeeded, Err otherwise
pub fn persist_indexes(
    db_path: &Path,
    kind_index: &KindIndex,
    name_index: &NameIndex,
    db_node_count: u64,
) -> Result<(), IndexPersistenceError> {
    let index_path = index_path_for_db(db_path);
    let temp_path = temp_path_for_db(db_path);

    // Create temporary file
    let mut file = std::fs::File::create(&temp_path)
        .map_err(|e| IndexPersistenceError::Io(format!("Failed to create temp file: {}", e)))?;

    // Write magic and version
    support::write_all(&mut file, INDEX_MAGIC, "magic")?;
    support::write_all(&mut file, &INDEX_VERSION.to_be_bytes(), "version")?;

    // Write DB node count (staleness guard)
    support::write_all(&mut file, &db_node_count.to_be_bytes(), "db node count")?;

    // Write kind index
    let kind_data = kind_index.export();
    let kind_entries: Vec<(&String, &Vec<i64>)> = kind_data.iter().collect();
    support::write_all(
        &mut file,
        &(kind_entries.len() as u32).to_be_bytes(),
        "kind count",
    )?;

    for (kind, node_ids) in kind_entries {
        support::write_string_entry(&mut file, kind, node_ids, "kind")?;
    }

    // Write name index
    let name_data = name_index.export();
    let name_entries: Vec<(&String, &Vec<i64>)> = name_data.iter().collect();
    support::write_all(
        &mut file,
        &(name_entries.len() as u32).to_be_bytes(),
        "name count",
    )?;

    for (name, node_ids) in name_entries {
        support::write_string_entry(&mut file, name, node_ids, "name")?;
    }

    // Sync to ensure data is written
    file.sync_all()
        .map_err(|e| IndexPersistenceError::Io(format!("Failed to sync file: {}", e)))?;
    drop(file);

    // Atomic rename
    std::fs::rename(&temp_path, &index_path)
        .map_err(|e| IndexPersistenceError::Io(format!("Failed to rename index file: {}", e)))?;

    Ok(())
}

/// Restore the kind and name indexes from a sidecar file
///
/// # Arguments
/// * `db_path` - Path to the main database file
/// * `db_node_count` - Node count from DB header (for staleness validation)
///
/// # Returns
/// Ok((KindIndex, NameIndex)) if restoration succeeded, Err otherwise
pub fn restore_indexes(
    db_path: &Path,
    db_node_count: u64,
) -> Result<(KindIndex, NameIndex), IndexPersistenceError> {
    let index_path = index_path_for_db(db_path);

    #[cfg(feature = "v3-forensics")]
    let file_open_syscall_start = std::time::Instant::now();

    let mut file = std::fs::File::open(&index_path)
        .map_err(|_| IndexPersistenceError::Corrupted("Index file not found".to_string()))?;

    #[cfg(feature = "v3-forensics")]
    let file_open_syscall_elapsed = file_open_syscall_start.elapsed();

    #[cfg(feature = "v3-forensics")]
    let file_open_start = std::time::Instant::now();

    // Read and verify magic
    let magic = support::read_array::<4, _>(&mut file, "magic")?;
    if &magic != INDEX_MAGIC {
        return Err(IndexPersistenceError::InvalidMagic(magic.to_vec()));
    }

    // Read and verify version
    let version = u32::from_be_bytes(support::read_array::<4, _>(&mut file, "version")?);
    if version != INDEX_VERSION {
        return Err(IndexPersistenceError::UnsupportedVersion(version));
    }

    // Read and verify DB node count (staleness guard)
    let stored_node_count =
        u64::from_be_bytes(support::read_array::<8, _>(&mut file, "stored node count")?);

    // Staleness check: sidecar must match current DB node count
    if stored_node_count != db_node_count {
        return Err(IndexPersistenceError::Corrupted(format!(
            "Stale index: sidecar node_count {} != DB node_count {}",
            stored_node_count, db_node_count
        )));
    }

    #[cfg(feature = "v3-forensics")]
    let file_open_elapsed = file_open_start.elapsed();

    // OPTIMIZATION: Read entire remaining file content into memory
    // This eliminates thousands of small read_exact() syscalls
    #[cfg(feature = "v3-forensics")]
    let bulk_read_start = std::time::Instant::now();

    let mut remaining_data = Vec::new();
    file.read_to_end(&mut remaining_data).map_err(|e| {
        IndexPersistenceError::Corrupted(format!("Failed to read index data: {}", e))
    })?;

    #[cfg(feature = "v3-forensics")]
    let bulk_read_elapsed = bulk_read_start.elapsed();

    // Parse from in-memory buffer using a cursor
    let mut cursor = support::SliceCursor::new(&remaining_data);

    // Read kind index
    let kind_count = cursor.read_u32()? as usize;

    #[cfg(feature = "v3-forensics")]
    let kind_loop_start = std::time::Instant::now();

    let kind_data = support::read_string_index_entries(&mut cursor, kind_count, "kind")?;

    #[cfg(feature = "v3-forensics")]
    let kind_loop_elapsed = kind_loop_start.elapsed();

    // Read name index
    let name_count = cursor.read_u32()? as usize;

    #[cfg(feature = "v3-forensics")]
    let name_loop_start = std::time::Instant::now();

    let name_data = support::read_string_index_entries(&mut cursor, name_count, "name")?;

    #[cfg(feature = "v3-forensics")]
    let name_loop_elapsed = name_loop_start.elapsed();

    // Create indexes and import data in single operation
    #[cfg(feature = "v3-forensics")]
    let index_creation_start = std::time::Instant::now();

    let kind_index = KindIndex::new();
    let name_index = NameIndex::new();

    #[cfg(feature = "v3-forensics")]
    let index_creation_elapsed = index_creation_start.elapsed();

    #[cfg(feature = "v3-forensics")]
    let hashmap_import_start = std::time::Instant::now();

    kind_index.import(kind_data);
    name_index.import(name_data);

    #[cfg(feature = "v3-forensics")]
    let hashmap_import_elapsed = hashmap_import_start.elapsed();

    #[cfg(feature = "v3-forensics")]
    {
        use std::io::Write;
        let _ = std::io::stderr().flush();
        eprintln!("    [restore_indexes breakdown]");
        eprintln!(
            "      - File::open() syscall:    {:.2} µs",
            file_open_syscall_elapsed.as_secs_f64() * 1_000_000.0
        );
        eprintln!(
            "      - read/verify header:      {:.2} µs",
            file_open_elapsed.as_secs_f64() * 1_000_000.0
        );
        eprintln!(
            "      - bulk_read() all data:    {:.2} µs ({:.2} KB)",
            bulk_read_elapsed.as_secs_f64() * 1_000_000.0,
            remaining_data.len() as f64 / 1024.0
        );
        eprintln!(
            "      - kind_loop ({}, entries):   {:.2} µs",
            kind_count,
            kind_loop_elapsed.as_secs_f64() * 1_000_000.0
        );
        eprintln!(
            "      - name_loop ({}, entries):   {:.2} µs",
            name_count,
            name_loop_elapsed.as_secs_f64() * 1_000_000.0
        );
        eprintln!(
            "      - index creation (2 objs):  {:.2} µs",
            index_creation_elapsed.as_secs_f64() * 1_000_000.0
        );
        eprintln!(
            "      - hashmap import:            {:.2} µs",
            hashmap_import_elapsed.as_secs_f64() * 1_000_000.0
        );
        let _ = std::io::stderr().flush();
    }

    Ok((kind_index, name_index))
}

/// Get the path to the index file for a given database path
pub fn index_path_for_db(db_path: &Path) -> std::path::PathBuf {
    let mut path = db_path.to_path_buf();
    path.set_extension("v3index");
    path
}

/// Get the temporary path for index file creation
pub fn temp_path_for_db(db_path: &Path) -> std::path::PathBuf {
    let mut path = db_path.to_path_buf();
    path.set_extension("v3index.tmp");
    path
}

/// Remove the index file (used during tests or cleanup)
pub fn remove_index_file(db_path: &Path) -> Result<(), std::io::Error> {
    let index_path = index_path_for_db(db_path);
    std::fs::remove_file(index_path)
}

#[cfg(test)]
mod tests;
