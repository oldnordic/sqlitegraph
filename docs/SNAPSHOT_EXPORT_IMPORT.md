# SQLiteGraph V2 Snapshot Export/Import

## Overview

Snapshot Export/Import provides instant database state snapshots that bypass WAL complexity and deliver atomic database restoration through direct file operations. This feature is designed for deterministic, high-performance database state management.

## Core Architecture

### Snapshot vs WAL Export

| Aspect | Snapshot Export | WAL Export |
|--------|----------------|-----------|
| **Primary Use Case** | Instant state capture, backups, testing | Continuous export with WAL replay |
| **Complexity** | Minimal (file copy) | High (WAL orchestration) |
| **Recovery Required** | No | Yes (always) |
| **Performance** | Fast (O(file size)) | Slower (WAL processing) |
| **Atomicity** | File-level atomicity | Transaction-level atomicity |
| **State Requirements** | Stable graph required | Handles all states |

### Key Invariants

**Before Snapshot Export:**
1. Graph file must be in stable state (no active transactions)
2. WAL directory must be empty or clean
3. All file buffers must be flushed and synced
4. Commit markers must be valid
5. Header consistency must pass validation

**During Snapshot Export:**
1. Atomic file copy with fsync discipline
2. No WAL reads or writes
3. No checkpoint generation
4. Direct file operations only
5. Immediate metadata generation

**After Snapshot Import:**
1. Graph file restored byte-identically
2. WAL directory empty
3. No recovery orchestration needed
4. Database immediately usable
5. All invariants preserved

## State Diagrams

### Snapshot Export Decision Flow

```
┌─────────────────────┐
│   Analyze State       │
└─────────┬───────────┘
          │
          ▼
    ┌─────┴──────┐
    │ Graph Stable? │
    └─────┬──────┘
          │ No        │ Yes
          ▼           ▼
    ┌─────┴────┐  ┌───┴──────┐
    │  Return  │  │ WAL Dir  │
    │   WAL   │  │ Clean?  │
    └──────────┘  └───┬──────┘
                     │ No   │ Yes
                     ▼      ▼
                ┌────┴─────┐┌─┴───────┐
                │ Return  ││Snapshot │
                │   WAL   ││ Export  │
                └─────────┘└─────────┘
```

### Planner Decision Rules

```
Stable Graph + No WAL → Snapshot (Optimal)
Stable Graph + Empty WAL → Snapshot (Optimal)
Active Transactions → WAL (Required)
Dirty WAL State → WAL (Required)
WAL Corruption → WAL (Recovery)
Graph Corruption → WAL (Recovery)
Clean WAL + Stable Graph → Snapshot (Required)
Default → WAL (Fallback)
```

## API Usage Examples

### Basic Snapshot Export

```rust
use sqlitegraph::backend::native::v2::{
    export::{SnapshotExporter, SnapshotExportConfig},
    graph_file::GraphFile,
};

// Create snapshot exporter
let config = SnapshotExportConfig {
    export_path: PathBuf::from("backup"),
    snapshot_id: "daily_backup_2024".to_string(),
    include_statistics: true,
    min_stable_duration: Duration::from_secs(5),
    checksum_validation: true,
};

let mut exporter = SnapshotExporter::new(&graph_path, config)?;

// Export snapshot
let result = exporter.export_snapshot()?;

println!("Snapshot exported to: {:?}", result.snapshot_path);
println!("Export size: {} bytes", result.snapshot_size_bytes);
```

### Using Export Factory

```rust
use sqlitegraph::backend::native::v2::export::ExportFactory;

// Create snapshot exporter via factory
let exporter = ExportFactory::create_snapshot_exporter(
    &graph_path,
    &export_dir,
    Some("backup_001".to_string())
)?;

let result = exporter.export_snapshot();
```

### Snapshot Import

```rust
use sqlitegraph::backend::native::v2::{
    import::{SnapshotImporter, SnapshotImportConfig},
    import::ImportMode,
};

// Configure import
let config = SnapshotImportConfig {
    target_graph_path: PathBuf::from("restored.v2"),
    export_dir_path: PathBuf::from("backup"),
    import_mode: ImportMode::Fresh,
    validate_manifest: true,
    verify_checksum: true,
    overwrite_existing: false,
};

// Create importer from export directory
let importer = SnapshotImporter::from_export_dir(
    &export_dir,
    &target_path,
    config
)?;

// Perform import
let result = importer.import()?;

println!("Imported {} records", result.records_imported);
println!("Import duration: {:?}", result.import_duration);
```

### Planner Integration

```rust
use sqlitegraph::backend::native::v2::planner::ExportPlanner;

// Analyze optimal export strategy
let decision = ExportPlanner::analyze_export_strategy(&graph_path)?;

match decision.export_mode {
    ExportMode::Snapshot => {
        println!("Snapshot export recommended: {:?}", decision.reasoning);
        // Use snapshot export
    }
    ExportMode::CheckpointAligned => {
        println!("Checkpoint-aligned export recommended: {:?}", decision.reasoning);
        // Use WAL export
    }
    _ => {
        println!("WAL export recommended: {:?}", decision.reasoning);
    }
}

// Quick check for snapshot advisability
let snapshot_advisable = ExportPlanner::is_snapshot_advisable(&graph_path)?;
println!("Snapshot advisable: {}", snapshot_advisable);
```

### Validation Before Export

```rust
use sqlitegraph::backend::native::v2::export::SnapshotExporter;

let mut exporter = SnapshotExporter::new(&graph_path, config)?;

// Validate preconditions
let validation = exporter.validate_snapshot_conditions()?;

if !validation.is_stable {
    eprintln!("Cannot export snapshot - not stable:");
    for error in &validation.errors {
        eprintln!("  Error: {}", error);
    }
    return Err(ExportError::UnstableState);
}

// Proceed with export
let result = exporter.export_snapshot()?;
```

## Export Manifest Format

Snapshot exports include a JSON manifest with metadata:

```json
{
  "magic": [86, 50, 88, 80, 77, 70, 0, 0],
  "version": 1,
  "recovery_state": "CleanShutdown",
  "authority": "GraphFile",
  "export_mode": "Snapshot",
  "graph_checkpoint_lsn": 0,
  "wal_start_lsn": null,
  "wal_end_lsn": null,
  "graph_format_version": 2,
  "wal_format_version": 1,
  "v2_clustered_edges": true,
  "export_timestamp": 1704067200,
  "export_duration_ms": 150,
  "graph_checksum": 1234567890,
  "wal_checksum": null,
  "total_records": 42,
  "total_bytes": 1048576,
  "reserved": [0, 0, 0, 0, 0, 0, 0, 0]
}
```

### Key Manifest Fields

- `export_mode`: Always "Snapshot" for snapshot exports
- `authority`: Always "GraphFile" (no WAL involvement)
- `wal_start_lsn`, `wal_end_lsn`: Always `null` for snapshots
- `recovery_state`: "CleanShutdown" (snapshots only export clean states)

## File Structure

### Export Directory Layout

```
snapshot_export/
├── export.manifest          # JSON manifest with metadata
├── snapshot_id.v2           # Graph file snapshot
└── (no WAL files)           # Snapshots don't include WAL
```

### Import Requirements

- `export.manifest`: Must exist and be valid JSON
- `*.v2` file: Must exist and be readable GraphFile
- Manifest `export_mode`: Must be "Snapshot"
- Manifest `v2_clustered_edges`: Must be `true`

## Performance Characteristics

### Export Performance

- **Time Complexity**: O(file size) - linear copy time
- **Space Complexity**: O(1) additional space for manifest
- **I/O Pattern**: Sequential read + sequential write
- **Memory Usage**: Minimal (streaming copy)
- **Atomicity**: File-level atomicity via rename

### Import Performance

- **Time Complexity**: O(file size) - linear copy time
- **Space Complexity**: O(1) additional space
- **I/O Pattern**: Sequential read + sequential write
- **Memory Usage**: Minimal (streaming copy)
- **Recovery**: No recovery required (immediate availability)

### Performance Benchmarks

Typical performance on SSD storage:
- 1GB database: ~2-3 seconds export, ~2-3 seconds import
- 100MB database: ~200-300ms export, ~200-300ms import
- Manifest generation: <1ms
- Validation: <5ms

## Error Handling

### Common Export Errors

```rust
match snapshot_export_result {
    Ok(result) => println!("Export successful"),
    Err(NativeBackendError::InvalidState { context, .. }) => {
        eprintln!("Graph not stable: {}", context);
    }
    Err(NativeBackendError::Io(e)) => {
        eprintln!("File system error: {}", e);
    }
    Err(NativeBackendError::InvalidMagicBytes { .. }) => {
        eprintln!("Corrupt graph file detected");
    }
    Err(NativeBackendError::UnsupportedVersion { version, .. }) => {
        eprintln!("Unsupported version: {}", version);
    }
    Err(other) => {
        eprintln!("Unexpected error: {:?}", other);
    }
}
```

### Common Import Errors

```rust
match snapshot_import_result {
    Ok(result) => println!("Import successful"),
    Err(NativeBackendError::InvalidParameter { context, .. }) => {
        eprintln!("Invalid configuration: {}", context);
    }
    Err(NativeBackendError::Io(e)) => {
        eprintln!("File system error: {}", e);
    }
    Err(NativeBackendError::CorruptStringTable { reason }) => {
        eprintln!("Manifest corruption: {}", reason);
    }
    Err(other) => {
        eprintln!("Unexpected error: {:?}", other);
    }
}
```

## Failure Modes and Recovery

### Export Failure Scenarios

1. **Active Transactions**
   - **Cause**: Graph has in-flight transactions
   - **Detection**: `graph_file.is_transaction_active()`
   - **Recovery**: Commit or rollback transactions

2. **Dirty WAL Files**
   - **Cause**: WAL contains uncommitted data
   - **Detection**: WAL file size > 0
   - **Recovery**: Use WAL export instead

3. **Graph File Corruption**
   - **Cause**: Invalid headers, checksum failures
   - **Detection**: `validate_file_size()`, `verify_commit_marker()`
   - **Recovery**: File repair or WAL export

4. **File System Errors**
   - **Cause**: Permission issues, disk full
   - **Detection**: I/O operation failures
   - **Recovery**: Check permissions, disk space

### Import Failure Scenarios

1. **Missing Manifest**
   - **Cause**: Export directory incomplete
   - **Detection**: Manifest file not found
   - **Recovery**: Verify export completeness

2. **Invalid Manifest**
   - **Cause**: Corrupted JSON, wrong format
   - **Detection**: JSON parsing errors, validation failures
   - **Recovery**: Re-export with correct format

3. **Snapshot File Corruption**
   - **Cause**: File transfer errors, disk corruption
   - **Detection**: GraphFile open failures
   - **Recovery**: Re-export from source

4. **Version Mismatch**
   - **Cause**: Export from incompatible version
   - **Detection**: Version validation in manifest
   - **Recovery**: Use migration or compatible source

## Monitoring and Diagnostics

### Export Metrics

```rust
let result = exporter.export_snapshot()?;

println!("Export metrics:");
println!("  File size: {} bytes", result.snapshot_size_bytes);
println!("  Duration: {:?}", result.export_duration);
println!("  Records: {}", result.record_count);
println!("  Checksum: {:x}", result.checksum);
println!("  Timestamp: {}", result.export_timestamp);
```

### Validation Reports

```rust
let validation = exporter.validate_snapshot_conditions()?;

println!("Validation report:");
println!("  Stable: {}", validation.is_stable);
println!("  WAL Clean: {}", validation.wal_clean);
println!("  File Consistent: {}", validation.file_consistent);
println!("  Commit Marker Valid: {}", validation.commit_marker_valid);

if !validation.errors.is_empty() {
    eprintln!("Validation errors:");
    for error in &validation.errors {
        eprintln!("  {}", error);
    }
}

if !validation.warnings.is_empty() {
    println!("Validation warnings:");
    for warning in &validation.warnings {
        println!("  {}", warning);
    }
}
```

## Best Practices

### Export Best Practices

1. **Ensure Stable State**
   ```rust
   // Ensure no active transactions
   if graph_file.is_transaction_active() {
       graph_file.commit_transaction()?;
   }

   // Flush all buffers
   graph_file.flush()?;
   ```

2. **Use Descriptive Snapshot IDs**
   ```rust
   let config = SnapshotExportConfig {
       snapshot_id: format!("backup_{}",
           chrono::Utc::now().format("%Y%m%d_%H%M%S")),
       // ...
   };
   ```

3. **Validate Before Export**
   ```rust
   let validation = exporter.validate_snapshot_conditions()?;
   if validation.is_stable {
       let result = exporter.export_snapshot()?;
   }
   ```

4. **Monitor Export Performance**
   ```rust
   let start = Instant::now();
   let result = exporter.export_snapshot()?;
   let duration = start.elapsed();

   if duration > Duration::from_secs(30) {
       eprintln!("Slow export detected: {:?}", duration);
   }
   ```

### Import Best Practices

1. **Validate Import Configuration**
   ```rust
   let validation = importer.validate_import()?;
   if validation.manifest_valid && validation.snapshot_accessible {
       let result = importer.import()?;
   }
   ```

2. **Use Fresh Import for Clean Restores**
   ```rust
   let config = SnapshotImportConfig {
       import_mode: ImportMode::Fresh,  // Always for snapshots
       overwrite_existing: false,   // Prevent accidental overwrites
       validate_manifest: true,
       verify_checksum: true,
       // ...
   };
   ```

3. **Verify Import Success**
   ```rust
   let result = importer.import()?;

   if result.validation_passed {
       let restored_graph = GraphFile::open(&target_path)?;
       // Verify database is usable
       assert!(!restored_graph.is_transaction_active());
   }
   ```

4. **Monitor WAL Absence**
   ```rust
   let wal_path = target_path.with_extension("wal");
   assert!(!wal_path.exists(), "WAL should not exist after snapshot import");
   ```

## Non-Goals

### Explicitly NOT Supported

1. **Real-time Snapshots**: Snapshots are point-in-time, not continuous
2. **Differential Snapshots**: No delta or incremental snapshots
3. **Cross-version Export**: Snapshots require exact version compatibility
4. **Compressed Snapshots**: No built-in compression (external compression possible)
5. **Network-based Export**: Direct file system operations only
6. **Streaming Export**: Atomic file copy only
7. **Live Database Export**: Database must be stable during export

### Alternative Approaches

For requirements not covered by snapshots:

1. **Continuous Backup**: Use WAL export with continuous WAL archiving
2. **Cross-version Migration**: Use dedicated migration tools
3. **Real-time Replication**: Use database-native replication
4. **Network Transfer**: Combine snapshot export with external transfer tools

## Integration with Existing Systems

### Backup Strategies

```rust
// 1. Daily snapshot backup
let daily_snapshot = SnapshotExportConfig {
    snapshot_id: format!("daily_{}",
        chrono::Utc::now().format("%Y%m%d")),
    ..Default::default()
};

// 2. Hourly WAL export for point-in-time recovery
let hourly_wal = V2ExportConfig {
    export_path: backup_dir.join("hourly"),
    include_wal_tail: true,
    ..Default::default()
};
```

### Testing Workflows

```rust
// 1. Create test fixture
let test_graph = create_test_database()?;
let fixture_path = create_snapshot_fixture(&test_graph)?;

// 2. Use snapshot for isolated tests
let test_db = import_snapshot_fixture(&fixture_path)?;
run_test_suite(&test_db);

// 3. Clean up
fs::remove_file(&test_db.path())?;
```

### Disaster Recovery

```rust
// 1. Assess damage
let planner_decision = ExportPlanner::analyze_export_strategy(&corrupted_path)?;

// 2. Choose recovery strategy
match planner_decision.export_mode {
    ExportMode::Snapshot => {
        // Use latest available snapshot
        recover_from_snapshot(&latest_snapshot_dir, &target_path)?;
    }
    ExportMode::Full => {
        // Use WAL export for comprehensive recovery
        recover_from_wal(&wal_backup_dir, &target_path)?;
    }
    _ => {
        return Err(RecoveryError::NoRecoveryPath);
    }
}
```

## Troubleshooting

### Common Issues

**Export Fails with "Graph not stable"**
```bash
# Check for active transactions
sqlitegraph --command status --db /path/to/graph.v2

# Check WAL state
ls -la /path/to/graph.v2.wal
```

**Import Fails with "Invalid manifest"**
```bash
# Check manifest exists
ls -la /path/to/export/export.manifest

# Validate JSON format
cat /path/to/export/export.manifest | jq .
```

**Performance Issues**
```bash
# Check file sizes
du -h /path/to/graph.v2

# Check I/O performance
iostat -x 1  # Monitor during export/import
```

### Debug Commands

```rust
// Enable debug output
std::env::set_var("GRAPH_DEBUG", "1");
std::env::set_var("SNAPSHOT_DEBUG", "1");

// Validate graph state
let graph = GraphFile::open(&path)?;
graph.validate_file_size()?;
graph.verify_commit_marker()?;

// Analyze export decision
let decision = ExportPlanner::analyze_export_strategy(&path)?;
println!("Decision: {:?}", decision);
```

## Version Compatibility

### Supported Versions

- **SQLiteGraph**: 0.2.4+ with V2 clustered edges
- **Graph Format**: Version 2 (V2 clustered edge format)
- **Manifest Format**: Version 1
- **Minimum Rust**: 1.70+

### Upgrade Path

1. **V1 to V2**: Use built-in migration tools before snapshot export
2. **Incompatible Versions**: Export with older version, upgrade target, then import
3. **Manifest Versioning**: Automatic version validation during import

## Security Considerations

### File Permissions

```rust
// Secure export directory permissions
fs::set_permissions(&export_dir,
    fs::Permissions::from_mode(0o750))?;

// Restrict manifest access
fs::set_permissions(&manifest_path,
    fs::Permissions::from_mode(0o640))?;
```

### Checksum Verification

```rust
// Always verify checksums during import
let config = SnapshotImportConfig {
    verify_checksum: true,  // Always enabled
    // ...
};
```

### Path Validation

```rust
// Validate paths to prevent directory traversal
fn validate_path(path: &Path) -> NativeResult<()> {
    if path.is_absolute() && path.starts_with("/safe/backup/") {
        Ok(())
    } else {
        Err(NativeBackendError::InvalidParameter {
            context: "Path not allowed".to_string(),
            source: None,
        })
    }
}
```

## Root Cause Analysis: Directory vs File Confusion Bug

### Issue Summary
**Bug ID**: SNAPSHOT-DIR-FILE-BUG-001
**Date**: 2025-12-21
**Component**: SQLiteGraph V2 SnapshotExporter
**Test Failing**: `test_snapshot_importer_creation`

### Root Cause
The SnapshotExporter contained its own `atomic_file_copy` method that lacked the precondition validation present in the production AtomicFileOperations. This caused the "Is a directory" error (OS code 21) when:

1. **Missing Validation**: The SnapshotExporter's `atomic_file_copy` method didn't check if the destination path already existed as a directory
2. **No Precondition Checks**: It didn't validate that the destination was a file path, not a directory
3. **Bypassed Fixed Code**: It completely bypassed the AtomicFileOperations that had comprehensive validation

### The Bug
```rust
// BUGGY CODE (removed):
fn atomic_file_copy(&self, destination: &Path) -> NativeResult<()> {
    // No validation of destination path!
    fs::copy(&self.source_path, &temp_path) // Could fail if destination exists as directory
}
```

### The Fix
```rust
// FIXED CODE:
// Step 4: Perform atomic graph file copy using proper AtomicFileOperations
let atomic_ops = AtomicFileOperations::new();
atomic_ops.atomic_copy_file(&self.source_path, &snapshot_path)?;
```

### Corrected Contract
- **export_path**: Directory path (e.g., `/tmp/.tmpXXXXX`)
- **snapshot_path**: File path within export directory (e.g., `/tmp/.tmpXXXXX/snapshot_12345.v2`)
- **Only parent directories are created**: `fs::create_dir_all(&export_path)` creates the directory, AtomicFileOperations handles the file

### Why This Bug Survived Earlier Phases
The bug existed because:
1. **Duplicate Implementation**: SnapshotExporter had its own atomic copy method instead of using the centralized AtomicFileOperations
2. **Missing Code Review**: The duplicate implementation wasn't caught during code review
3. **Incomplete Testing**: The specific test case exposed the directory vs file confusion

### Lessons Learned
1. **Single Source of Truth**: Atomic file operations should use the centralized AtomicFileOperations
2. **Precondition Validation**: Always validate filesystem state before operations
3. **Comprehensive Testing**: Test edge cases with directories vs files

## Glossary

- **Atomic Operation**: Operation that either completes fully or not at all
- **Clean Shutdown**: Database state with all transactions committed
- **Dirty Shutdown**: Database state with uncommitted transactions
- **FSync**: System call to ensure data is written to disk
- **GraphFile**: SQLiteGraph's native storage format
- **WAL**: Write-Ahead Log for transaction durability
- **Snapshot**: Point-in-time database state capture