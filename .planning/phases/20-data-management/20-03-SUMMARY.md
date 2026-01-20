---
phase: 20-data-management
plan: 03
type: execute
completed: 2026-01-20
duration: 517 seconds (8.6 minutes)
subsystem: Data Management
tags: [backup, api, snapshot, checkpoint, v2]
---

# Phase 20 Plan 03: Backup API Summary

**Public backup API that wraps existing SnapshotExporter, triggers checkpoint to ensure consistent state, and provides user-friendly interface for creating database snapshots.**

## One-Liner

Implemented public backup API with `BackupConfig`, `BackupResult`, and `create_backup()` functions for Native V2 backend, plus `GraphBackend::backup()` method for unified backend access.

## Deliverables

### Artifacts Created

| Path | Purpose | Exports |
|------|---------|---------|
| `sqlitegraph/src/backend/native/v2/backup/mod.rs` | Public backup API wrapper | `BackupConfig`, `BackupResult`, `create_backup`, `backup` |

### Files Modified

| File | Changes |
|------|---------|
| `sqlitegraph/src/backend/native/v2/mod.rs` | Added backup module declaration and re-exports |
| `sqlitegraph/src/backend.rs` | Added `backup()` method to `GraphBackend` trait and `BackupResult` struct |
| `sqlitegraph/src/backend/native/graph_backend.rs` | Implemented `backup()` for `NativeGraphBackend` |
| `sqlitegraph/src/backend/sqlite/impl_.rs` | Implemented `backup()` for `SqliteGraphBackend` |
| `sqlitegraph/src/lib.rs` | Re-exported backup API at crate root with convenience function |

## Dependency Graph

### Requires
- **Phase 20-01**: V3 file format established (schema_version u32 + reserved)
- **Phase 20-02**: File format migration API (auto-detection and atomic V2-to-V3 conversion)
- **Phase 11-19**: WAL checkpoint infrastructure (`V2WALManager::force_checkpoint`)
- **Phase 14**: SnapshotExporter implementation

### Provides
- Public backup API for Native V2 backend
- Unified `GraphBackend::backup()` method for all backends
- Backup configuration with checkpoint-before-backup option

### Affects
- **Phase 20-04**: Restore API (will use backup files as source)
- External applications using SQLiteGraph for data protection

## Tech Stack

### Added
- `BackupConfig` - Builder pattern configuration for backup operations
- `BackupResult` - Metadata returned from backup operations
- `create_backup()` - Main backup function with checkpoint integration
- `backup()` - Convenience function with default configuration

### Patterns Established
- **Checkpoint-before-backup**: Optional WAL checkpoint via `V2WALManager::force_checkpoint()`
- **Builder pattern**: `BackupConfig::new().with_backup_id().with_checkpoint()`
- **Multi-level API**:
  - Low-level: `sqlitegraph::backend::native::v2::backup::create_backup()`
  - Mid-level: `sqlitegraph::database_backup()` (re-export)
  - High-level: `sqlitegraph::create_backup()` (convenience function)
  - Backend trait: `GraphBackend::backup()`

## Decisions Made

1. **Checkpoint-before-backup defaults to enabled**: Ensures WAL is applied before backup for consistent snapshots
2. **Checkpoint failures are non-fatal**: Logs warning but continues with backup if checkpoint fails
3. **Separate BackupResult from SnapshotExportResult**: User-facing struct with cleaner field names
4. **Multi-level API design**: Convenience function at crate root, full API via backend modules
5. **SQLite backend uses VACUUM INTO**: Leverages SQLite's native backup for consistent snapshots

## Deviations from Plan

### Auto-fixed Issues

**None - plan executed exactly as written.**

All tasks completed as specified with no unexpected bugs or modifications required.

## Success Criteria

| Criterion | Status | Evidence |
|-----------|--------|----------|
| Backup API creates consistent snapshots | ✅ | Uses `SnapshotExporter` with optional checkpoint |
| Checkpoint-before-backup option available | ✅ | `BackupConfig::with_checkpoint()` |
| Backup includes manifest with metadata | ✅ | `BackupResult` includes all metadata |
| Public API accessible at crate root | ✅ | `sqlitegraph::create_backup()` and `sqlitegraph::BackupResult` |
| All tests pass | ✅ | 5 backup tests pass |

## Commits

| Commit | Hash | Description |
|--------|------|-------------|
| 1 | 7c28d01 | feat(20-03): create backup module with public API wrapper |
| 2 | d6162d6 | feat(20-03): export backup API from v2 module |
| 3 | 68317de | feat(20-03): add backup method to GraphBackend trait |
| 4 | b7d9aad | feat(20-03): export backup API at crate root |

## Verification Commands

```bash
# Check compilation
cargo check --package sqlitegraph

# Run backup tests
cargo test --package sqlitegraph --lib 'backup'

# Verify API accessibility
# All of these should work:
# - sqlitegraph::create_backup(path, backup_dir)
# - sqlitegraph::backend::native::v2::create_backup()
# - GraphBackend::backup(backup_dir)
```

## Next Phase Readiness

**Ready for Phase 20-04 (Restore API):**

- Backup file format established (V2 snapshot + manifest)
- Backup API provides files for restore to consume
- No blockers identified

## Notes

- Backup creates both snapshot file (`.v2`) and manifest file (`.json`)
- Checkpoint is optional and gracefully handles missing WAL
- SQLite backend uses `VACUUM INTO` for clean backup files
- Native V2 backup includes checksum validation
