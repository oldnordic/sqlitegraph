# Phase 20: Data Management - Research

**Researched:** 2026-01-20
**Domain:** File format migration, backup/restore for embedded graph databases
**Confidence:** HIGH

## Summary

This phase implements file format migration and backup/restore APIs for SQLiteGraph's Native V2 backend. The research reveals that:

1. **Current schema version is 8 bytes (u64)** but needs to be 4 bytes (u32) per SCHEMA-01
2. **Existing snapshot infrastructure exists** in `v2/export/snapshot.rs` and `v2/snapshot/` - can be extended for public backup/restore API
3. **No migration API exists** - files with old format versions are rejected with `UnsupportedVersion` error
4. **Atomic file operations are already implemented** in `v2/snapshot/atomic_ops.rs` - can be reused for migration
5. **The current header format is 80 bytes** with schema_version at offset 32-39 (8 bytes)

**Primary recommendation:** Extend existing snapshot infrastructure for backup/restore, create new migration module using atomic file operations, and change schema_version from u64 to u32 with a file format version bump.

## Current Schema Version Handling

### Location in File Header

**Source:** `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/persistent_header.rs:26`

```rust
/// Schema version
pub schema_version: u64,
```

### Header Layout (80 bytes total)

| Offset | Field | Size | Type |
|--------|-------|------|------|
| 0-7 | magic | 8 | [u8; 8] |
| 8-11 | version | 4 | u32 |
| 12-15 | flags | 4 | u32 |
| 16-23 | node_count | 8 | u64 |
| 24-31 | edge_count | 8 | u64 |
| 32-39 | **schema_version** | **8** | **u64** (needs to be u32) |
| 40-47 | node_data_offset | 8 | u64 |
| 48-55 | edge_data_offset | 8 | u64 |
| 56-63 | outgoing_cluster_offset | 8 | u64 |
| 64-71 | incoming_cluster_offset | 8 | u64 |
| 72-79 | free_space_offset | 8 | u64 |

**Constants source:** `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/constants.rs`

### Current Default Schema Version

```rust
/// Default schema version
pub const DEFAULT_SCHEMA_VERSION: u64 = 1;
```

### Version Checking at File Open

**Source:** `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/graph_file/file_lifecycle.rs:109-124`

```rust
// V2-ONLY REFACTOR: Hard format gate - refuse non-V2 files
let required_flags = FLAG_V2_FRAMED_RECORDS | FLAG_V2_ATOMIC_COMMIT;
if (graph_file.persistent_header.flags & required_flags) != required_flags {
    return Err(NativeBackendError::UnsupportedVersion {
        version: 1, // Any file without both V2 flags is unsupported
        supported_version: 2,
    });
}

// V2-specific validation
if graph_file.persistent_header.version != 2 {
    return Err(NativeBackendError::UnsupportedVersion {
        version: graph_file.persistent_header.version,
        supported_version: 2,
    });
}
```

**Key finding:** The code currently only supports version 2. Older files are rejected with `UnsupportedVersion` error. No migration path exists.

## Existing Infrastructure to Reuse

### 1. Atomic File Operations (HIGH confidence)

**Source:** `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/snapshot/atomic_ops.rs`

```rust
pub struct AtomicFileOperations;

impl AtomicFileOperations {
    /// Perform atomic file copy from source to destination
    pub fn atomic_copy_file(&self, source: &Path, destination: &Path) -> NativeResult<()> {
        // Uses temp file + fsync + rename pattern for atomicity
        // Already tested and functional
    }
}
```

**Reusable for:** Migration atomic writes

### 2. Snapshot Export Infrastructure (HIGH confidence)

**Source:** `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/export/snapshot.rs`

```rust
pub struct SnapshotExporter {
    graph_file: GraphFile,
    config: SnapshotExportConfig,
    source_path: PathBuf,
}

pub struct SnapshotExportResult {
    pub snapshot_path: PathBuf,
    pub manifest_path: PathBuf,
    pub export_duration: Duration,
    pub snapshot_size_bytes: u64,
    pub checksum: u64,
    pub record_count: u64,
    pub export_timestamp: u64,
}
```

**Reusable for:** BACKUP-01 (creates consistent snapshot)

### 3. Snapshot Import Infrastructure (HIGH confidence)

**Source:** `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/import/snapshot.rs`

```rust
pub struct SnapshotImporter {
    config: SnapshotImportConfig,
    manifest: ExportManifest,
    snapshot_path: PathBuf,
}

pub struct SnapshotImportResult {
    pub records_imported: u64,
    pub import_duration: Duration,
    pub snapshot_size_bytes: u64,
    pub imported_checksum: u64,
    pub validation_passed: bool,
    pub final_recovery_state: ExplicitRecoveryState,
}
```

**Reusable for:** BACKUP-02 (loads snapshot and verifies integrity)

### 4. Export Manifest (HIGH confidence)

**Source:** `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/export/manifest.rs`

```rust
pub struct ExportManifest {
    pub magic: [u8; 8],
    pub version: u32,
    pub recovery_state: ExplicitRecoveryState,
    pub authority: Authority,
    pub export_mode: ExportMode,
    pub graph_format_version: u32,
    pub total_bytes: u64,
    // ... other fields
}
```

**Reusable for:** Backup metadata and migration manifest

## Standard Stack

### Core
| Library/Module | Location | Purpose | Why Standard |
|----------------|----------|---------|--------------|
| AtomicFileOperations | `v2/snapshot/atomic_ops.rs` | Atomic file copy with temp+rename | POSIX atomic rename pattern (verified by [StackOverflow](https://stackoverflow.com/questions/167414/is-an-atomic-file-rename-with-overwrite-possible-on-windows)) |
| SnapshotExporter | `v2/export/snapshot.rs` | Graph file snapshot creation | Already implements consistent snapshot |
| SnapshotImporter | `v2/import/snapshot.rs` | Graph file restoration | Already implements validation |
| ExportManifest | `v2/export/manifest.rs` | Metadata for snapshots/backups | JSON-serializable manifest format |

### External (if needed)
| Library | Purpose | When to Use |
|---------|---------|-------------|
| tempfile | Temporary directory creation for migration tests | Testing only (already in dev dependencies) |
| serde | JSON serialization for manifests | Already used |

## Architecture Patterns

### Pattern 1: Atomic Migration with Shadow File

**What:** Write migrated data to temporary file, then atomic rename to replace original

**When to use:** File format migration (MIGRATE-03: atomic migration requirement)

**Example:**
```rust
// Based on AtomicFileOperations::atomic_copy_file pattern
pub fn migrate_file(source: &Path, destination: &Path) -> NativeResult<()> {
    let temp_path = destination.with_extension("tmp");

    // 1. Write migrated data to temp file
    write_migrated_data(source, &temp_path)?;

    // 2. Sync temp file
    sync_file(&temp_path)?;

    // 3. Atomic rename
    fs::rename(&temp_path, destination)?;

    // 4. Sync parent directory
    sync_directory(destination.parent().unwrap())?;

    Ok(())
}
```

**Source:** Derived from existing `AtomicFileOperations` implementation

### Pattern 2: Version Detection at Open

**What:** Detect old format version and trigger migration before returning GraphFile

**When to use:** MIGRATE-01 (detect old format versions)

**Example:**
```rust
pub fn open_or_migrate<P: AsRef<Path>>(path: P) -> NativeResult<GraphFile> {
    let path = path.as_ref();

    // Try to open normally
    match GraphFile::open(path) {
        Ok(graph) => Ok(graph),
        Err(NativeBackendError::UnsupportedVersion { version, .. }) => {
            // Trigger migration for supported old versions
            if version == 1 || (version == 2 && needs_schema_migration(path)?) {
                migrate_file_format(path)?;
                GraphFile::open(path) // Retry after migration
            } else {
                Err(...) // Unsupported version
            }
        }
        Err(e) => Err(e),
    }
}
```

### Pattern 3: Rollback via Backup Preservation

**What:** Keep original file until migration verified successfully

**When to use:** MIGRATE-04 (migration can be rolled back)

**Example:**
```rust
pub fn migrate_with_rollback(path: &Path) -> NativeResult<()> {
    let backup_path = path.with_extension("bak");

    // 1. Create backup
    AtomicFileOperations::new().atomic_copy_file(path, &backup_path)?;

    // 2. Attempt migration
    let result = migrate_file_internal(path);

    // 3. Rollback on failure
    if result.is_err() {
        let _ = AtomicFileOperations::new().atomic_copy_file(&backup_path, path);
        let _ = fs::remove_file(&backup_path);
        return result;
    }

    // 4. Verify and cleanup backup on success
    verify_migration(path)?;
    fs::remove_file(&backup_path)?;

    Ok(())
}
```

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Atomic file copy | Manual `fs::copy` + rename logic | `AtomicFileOperations::atomic_copy_file` | Already implements temp file + fsync + atomic rename pattern correctly |
| Snapshot metadata format | Custom binary format | `ExportManifest` (JSON) | Already exists, JSON is human-readable and debuggable |
| Version detection logic | Scanning file bytes manually | Read header via existing `FileOperations::read_and_validate_header` | Safe parsing with bounds checking already implemented |
| Checksum calculation | Rolling hash implementation | `std::collections::hash_map::DefaultHasher` (used in snapshot import) | Standard library implementation, verified in existing code |

## Common Pitfalls

### Pitfall 1: Breaking Header Size When Changing schema_version Type

**What goes wrong:** Changing `schema_version: u64` to `u32` without adjusting header layout creates a 4-byte gap or shifts subsequent fields

**Why it happens:** Header is a byte-level structure with fixed offsets

**How to avoid:**
1. File format version bump (2 -> 3) when changing header layout
2. Reclaim the 4 bytes for reserved/future use
3. Update `PERSISTENT_HEADER_SIZE` calculation
4. Add backward-compatible read path for version 2 files

**Warning signs:** Compiler error in `encoding.rs` about size mismatch

### Pitfall 2: Non-Atomic Migration on Crash

**What goes wrong:** If process crashes after writing new file but before deleting old, database is in inconsistent state

**Why it happens:** Not using temp file + atomic rename pattern

**How to avoid:**
1. Always write to temporary file with different name
2. Sync temp file to disk
3. Use `fs::rename()` which is atomic on POSIX
4. Sync parent directory to persist rename

**Warning signs:** Direct file writes without temp file pattern

### Pitfall 3: WAL Files Not Included in Snapshot

**What goes wrong:** Snapshot of graph file only, missing WAL directory with uncommitted transactions

**Why it happens:** Graph file and WAL are separate directories/files

**How to avoid:**
1. For BACKUP-03 (all data pages and WAL position), include WAL directory
2. Or use checkpoint-aligned snapshot (WAL already applied to graph file)
3. Document snapshot type in manifest (already exists via `ExportMode`)

**Warning signs:** Snapshot size much smaller than expected

### Pitfall 4: Migration Loses Data Due to Offset Assumptions

**What goes wrong:** Migration code assumes field positions that changed in new format

**Why it happens:** Hard-coded offsets instead of using struct serialization

**How to avoid:**
1. Use existing `encode_persistent_header`/`decode_persistent_header` functions
2. These functions already handle all fields correctly
3. Test round-trip: old format -> decode -> migrate -> encode -> new format

**Warning signs:** Manual byte array construction

## Code Examples

### Reading Header and Detecting Version

**Source:** Derived from `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/graph_file/file_lifecycle.rs`

```rust
use crate::backend::native::{
    graph_file::GraphFile,
    persistent_header::PersistentHeaderV2,
    types::{NativeBackendError, NativeResult},
};

/// Detect file format version from header
pub fn detect_format_version(path: &std::path::Path) -> NativeResult<u32> {
    let file = std::fs::File::open(path)
        .map_err(|e| NativeBackendError::Io(e))?;

    // Use existing header read logic
    let header = crate::backend::native::graph_file::file_ops::FileOperations
        ::read_and_validate_header(&mut file)?;

    Ok(header.version)
}

/// Check if file needs schema migration
pub fn needs_schema_migration(path: &std::path::Path) -> NativeResult<bool> {
    let file = std::fs::File::open(path)
        .map_err(|e| NativeBackendError::Io(e))?;

    let header = crate::backend::native::graph_file::file_ops::FileOperations
        ::read_and_validate_header(&mut file)?;

    // Version 2 with old schema_version size needs migration
    // Version 3+ already has new format
    Ok(header.version == 2) // Will be 3 after schema_version change
}
```

### Creating a Backup Snapshot

**Source:** `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/export/snapshot.rs:120-200`

```rust
use crate::backend::native::v2::export::{SnapshotExporter, SnapshotExportConfig};
use std::path::Path;

pub fn create_backup(
    graph_path: &Path,
    backup_dir: &Path,
) -> NativeResult<BackupResult> {
    let config = SnapshotExportConfig {
        export_path: backup_dir.to_path_buf(),
        snapshot_id: format!("backup_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs()),
        include_statistics: true,
        min_stable_duration: std::time::Duration::from_secs(0),
        checksum_validation: true,
    };

    let mut exporter = SnapshotExporter::new(graph_path, config)?;
    exporter.export_snapshot()
}
```

### Restoring from Backup

**Source:** `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/import/snapshot.rs:187-239`

```rust
use crate::backend::native::v2::import::{SnapshotImporter, SnapshotImportConfig};
use std::path::Path;

pub fn restore_backup(
    backup_dir: &Path,
    target_path: &Path,
) -> NativeResult<RestoreResult> {
    let config = SnapshotImportConfig {
        target_graph_path: target_path.to_path_buf(),
        export_dir_path: backup_dir.to_path_buf(),
        import_mode: crate::backend::native::v2::import::ImportMode::Fresh,
        validate_manifest: true,
        verify_checksum: true,
        overwrite_existing: false,
    };

    let importer = SnapshotImporter::from_export_dir(
        backup_dir,
        target_path,
        config,
    )?;
    importer.import()
}
```

## Files That Need Modification

### For Schema Version Change (SCHEMA-01, SCHEMA-02, SCHEMA-03)

| File | Change |
|------|--------|
| `src/backend/native/persistent_header.rs` | Change `schema_version: u64` to `u32`, add reserved bytes |
| `src/backend/native/constants.rs` | Update `SCHEMA_VERSION` size from 8 to 4 |
| `src/backend/native/graph_file/encoding.rs` | Update encode/decode for schema_version (4 bytes not 8) |
| `src/backend/native/v2/mod.rs` | Bump `V2_FORMAT_VERSION` from 2 to 3 |

### For Migration API (MIGRATE-01 through MIGRATE-04)

| File | New/Modified |
|------|--------------|
| `src/backend/native/v2/migration/mod.rs` | **NEW** - Migration module |
| `src/backend/native/v2/migration/detect.rs` | **NEW** - Version detection |
| `src/backend/native/v2/migration/plan.rs` | **NEW** - Migration planning |
| `src/backend/native/v2/migration/execute.rs` | **NEW** - Atomic migration execution |
| `src/backend/native/v2/mod.rs` | Re-export migration API |

### For Backup/Restore API (BACKUP-01 through BACKUP-03)

| File | New/Modified |
|------|--------------|
| `src/backend/native/v2/backup/mod.rs` | **NEW** - Public backup API wrapper |
| `src/backend/native/v2/restore/mod.rs` | **NEW** - Public restore API wrapper |
| `src/lib.rs` | Add public `backup()` and `restore()` functions |
| `src/backend.rs` | Add backend-level backup/restore methods |

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| No migration support | Planned: Auto-detect and migrate v2 -> v3 | Phase 20 | Enables schema evolution |
| No public backup/restore | Internal snapshot API exists | Phase 14 | Need public API wrapper |
| schema_version: u64 (8 bytes) | schema_version: u32 (4 bytes) + 4 reserved | Phase 20 | Saves 4 bytes in header, more typical for schema version |

**Deprecated/outdated:** None - this is new functionality

## Schema Version Migration Strategy

### Current Layout (Version 2)
```
Offset 32-39: schema_version (u64, 8 bytes)
```

### Proposed Layout (Version 3)
```
Offset 32-35: schema_version (u32, 4 bytes)
Offset 36-39: reserved (4 bytes) - for future use
```

### Migration Steps
1. **Detect**: Read header at offset 32-39, interpret as u64 (v2) or u32 + reserved (v3)
2. **Convert**: For v2 files, take lower 32 bits of schema_version, preserve upper 32 bits in reserved
3. **Write**: Write v3 header with 4-byte schema_version and 4-byte reserved
4. **Verify**: Re-open and validate header reads correctly as v3

### Backward Compatibility
- **Read v2 files**: Interpret 8 bytes at offset 32-39 as u64
- **Write v3 files**: Write 4 bytes schema_version + 4 bytes reserved
- **Migration detection**: Check file_format_version field (offset 8-11)

## Open Questions

### Low Priority (can be resolved during planning)

1. **Should migration be automatic or opt-in?**
   - Automatic: GraphFile::open() detects and migrates transparently
   - Opt-in: Separate `GraphFile::open_or_migrate()` method
   - **Recommendation:** Automatic for version 2->3, explicit for later versions

2. **Should backup include WAL files?**
   - Current snapshot export is graph-file only
   - BACKUP-03 says "all data pages and WAL position"
   - **Recommendation:** Checkpoint before backup (WAL already applied), graph file only

3. **How to handle very large files during migration?**
   - Migration needs 2x disk space (original + temp file)
   - **Recommendation:** Document requirement, add pre-check for available space

## Sources

### Primary (HIGH confidence)
- `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/persistent_header.rs` - Header structure definition
- `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/constants.rs` - Header size constants
- `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/graph_file/file_lifecycle.rs` - Version checking logic
- `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/snapshot/atomic_ops.rs` - Atomic file operations
- `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/export/snapshot.rs` - Snapshot export
- `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/import/snapshot.rs` - Snapshot import
- `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/v2/export/manifest.rs` - Manifest format
- `/home/feanor/Projects/sqlitegraph/sqlitegraph/src/backend/native/graph_file/encoding.rs` - Header encoding

### Secondary (MEDIUM confidence)
- [StackOverflow: Atomic file rename with overwrite](https://stackoverflow.com/questions/167414/is-an-atomic-file-rename-with-overwrite-possible-on-windows) - POSIX atomic rename behavior
- [Datadog: Rust timeseries engine (2025)](https://www.datadoghq.com/blog/engineering/rust-timeseries-engine/) - Production snapshot patterns
- [SurrealKV: Rust embedded database](https://github.com/surrealdb/surrealkv) - Reference for embedded DB patterns

### Tertiary (LOW confidence - general best practices)
- [Database Migration Best Practices](https://www.bacancytechnology.com/blog/15-best-practices-for-a-seamless-database-migration/) - General migration patterns

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - existing snapshot infrastructure verified in codebase
- Architecture: HIGH - atomic file operations already implemented and tested
- Pitfalls: HIGH - schema_version change is well-understood, migration pattern is standard

**Research date:** 2026-01-20
**Valid until:** 60 days (domain is stable - embedded database patterns don't change rapidly)
