---
phase: 20-data-management
plan: 01
subsystem: data-management
tags: [file-format, schema-version, backward-compatibility, v3-format]

# Dependency graph
requires:
  - phase: 19
    provides: Connection pooling and concurrent features
provides:
  - v3 file format with 4-byte schema_version + 4-byte reserved field
  - Backward-compatible read path for v2 files with 8-byte schema_version
  - Bumped FILE_FORMAT_VERSION from 2 to 3
affects: [20-02, migration, backup-restore]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - Version-aware header decoding for format migration
    - Reserved field pattern for future format evolution

key-files:
  created: []
  modified:
    - sqlitegraph/src/backend/native/persistent_header.rs
    - sqlitegraph/src/backend/native/constants.rs
    - sqlitegraph/src/backend/native/graph_file/encoding.rs
    - sqlitegraph/src/backend/native/v2/mod.rs
    - sqlitegraph/src/backend/native/graph_file/file_lifecycle.rs
    - sqlitegraph/src/backend/native/types/file_header.rs

key-decisions:
  - "Schema version changed from u64 (8 bytes) to u32 (4 bytes) to meet SCHEMA-01 requirement"
  - "File format version bumped from 2 to 3 to indicate breaking header format change"
  - "Backward-compatible decode allows reading v2 files (8-byte schema_version) as v3 structure"
  - "4-byte reserved field added for future use while maintaining 80-byte header size"

patterns-established:
  - "Version-aware decode: Check file_format_version first, then decode schema_version accordingly"
  - "Format evolution: Bump version number when changing on-disk header structure"

# Metrics
duration: 14min
completed: 2026-01-20
---

# Phase 20 Plan 1: Schema Version u32 Migration Summary

**v3 file format with 4-byte schema_version field and backward-compatible v2 file reading**

## Performance

- **Duration:** 14 min
- **Started:** 2026-01-20T18:37:24Z
- **Completed:** 2026-01-20T18:51:06Z
- **Tasks:** 4
- **Files modified:** 6

## Accomplishments

- Changed `schema_version` from u64 (8 bytes) to u32 (4 bytes) per SCHEMA-01 requirement
- Added `reserved: u32` field to maintain 80-byte header size
- Bumped FILE_FORMAT_VERSION from 2 to 3 to indicate breaking format change
- Implemented backward-compatible decode path for v2 files
- Updated encoding/decoding to handle both v2 (8-byte) and v3 (4+4-byte) schema_version formats

## Task Commits

Each task was committed atomically:

1. **Task 1: Update PersistentHeaderV2 struct with u32 schema_version** - `461521a` (feat)
2. **Task 2: Update encoding.rs for 4-byte schema_version with backward compatibility** - `8f162df` (feat)
3. **Task 3: Bump file format version to 3 and update constants** - `cf86648` (feat)
4. **Task 4: Update file_lifecycle.rs to accept v2 and v3 formats** - `0a3e08a` (feat)

## Files Created/Modified

- `sqlitegraph/src/backend/native/persistent_header.rs` - Changed schema_version to u32, added reserved field, updated offsets and sizes
- `sqlitegraph/src/backend/native/constants.rs` - Updated FILE_FORMAT_VERSION to 3, SCHEMA_VERSION size to 4, DEFAULT_SCHEMA_VERSION to u32
- `sqlitegraph/src/backend/native/graph_file/encoding.rs` - Version-aware encode/decode for v2/v3 formats
- `sqlitegraph/src/backend/native/v2/mod.rs` - Bumped V2_FORMAT_VERSION to 3
- `sqlitegraph/src/backend/native/graph_file/file_lifecycle.rs` - Accept both v2 and v3 format versions
- `sqlitegraph/src/backend/native/types/file_header.rs` - Updated schema_version to u32, added reserved field

## Decisions Made

- Schema version changed from u64 to u32 (4 bytes) to meet SCHEMA-01 requirement
- 4-byte reserved field added for future use while maintaining 80-byte header size
- File format version bumped from 2 to 3 to indicate breaking header format change
- Backward-compatible decode reads v2 files (8-byte schema_version) by taking lower 32 bits as schema_version, upper 32 bits as reserved
- Version-aware encoding: new files always written in v3 format (4+4 bytes), old v2 files can be read transparently

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None - all changes compiled successfully and tests related to the changes pass.

Note: 4 pre-existing test failures (test_native_bfs_simple, test_native_shortest_path, test_snapshot_exporter_creation, test_snapshot_importer_creation) are unrelated to these changes and were failing before the plan execution.

## Next Phase Readiness

- v3 file format with 4-byte schema_version is complete
- Backward compatibility with v2 files is implemented
- All constants updated consistently across the codebase
- Ready for subsequent data management plans (backup/restore, migration APIs)

---
*Phase: 20-data-management*
*Completed: 2026-01-20*
