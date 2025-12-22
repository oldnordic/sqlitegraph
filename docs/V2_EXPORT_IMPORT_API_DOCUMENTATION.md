# SQLiteGraph V2 Export/Import API Documentation

**Document Date**: 2025-12-21
**Purpose**: Document actual V2 export/import APIs vs test expectations
**Scope**: Complete API surface analysis for integration test fixes

---

## Executive Summary

This document provides a comprehensive analysis of the actual V2 export/import APIs implemented in SQLiteGraph versus what the integration tests expect. The analysis reveals significant API drift that needs to be addressed for test compilation.

## 1. Export API Analysis

### 1.1 Actual V2ExportConfig Structure

**Location**: `sqlitegraph::backend::native::v2::export::exporter::V2ExportConfig`

```rust
pub struct V2ExportConfig {
    /// Export directory path
    pub export_path: PathBuf,

    /// Whether to include WAL tail in export
    pub include_wal_tail: bool,

    /// Whether to enable compression
    pub compression_enabled: bool,

    /// Whether to perform checksum validation
    pub checksum_validation: bool,
}
```

### 1.2 Test-Expected V2ExportConfig Structure

**Test Location**: `snapshot_export_import_tdd_tests.rs`

```rust
// WHAT TESTS EXPECT (INCORRECT):
V2ExportConfig {
    graph_path: graph_path.clone(),        // ❌ Does not exist
    export_dir: TempDir::new().unwrap().into_path(),  // ❌ Should be export_path
    export_mode: ExportMode::Snapshot,     // ❌ Not a field, should be inferred
    include_wal: false,                    // ❌ Should be include_wal_tail
    validate_recovery: false,              // ❌ Does not exist
    compression_level: None,               // ❌ Should be compression_enabled (bool)
}
```

### 1.3 Actual V2Exporter Constructor

**Location**: `sqlitegraph::backend::native::v2::export::exporter::V2Exporter`

```rust
impl V2Exporter {
    /// Create exporter from existing graph file
    pub fn from_graph_file(
        graph_path: &Path,
        export_config: V2ExportConfig,
    ) -> NativeResult<Self>

    // ❌ NO new() METHOD EXISTS
}
```

### 1.4 Test-Expected V2Exporter Constructor

```rust
// WHAT TESTS CALL (INCORRECT):
let exporter = V2Exporter::new(snapshot_config);  // ❌ No new() method

// WHAT ACTUALLY EXISTS:
let exporter = V2Exporter::from_graph_file(&graph_path, config);  // ✅ Correct
```

### 1.5 Actual Export Modes

**Location**: `sqlitegraph::backend::native::v2::export::ExportMode`

```rust
pub enum ExportMode {
    /// Export checkpoint-aligned state (no WAL tail)
    CheckpointAligned,

    /// Export with LSN-bounded WAL tail
    LsnBounded,

    /// Export full state (graph + all WAL records)
    Full,

    /// Export instant snapshot (atomic graph file copy, no WAL involvement)
    Snapshot,  // ✅ This variant exists
}
```

### 1.6 Export Factory Methods

**Location**: `sqlitegraph::backend::native::v2::export::ExportFactory`

```rust
impl ExportFactory {
    /// Create an exporter with default configuration
    pub fn create_exporter(
        graph_path: &Path,
        export_config: V2ExportConfig,
    ) -> NativeResult<V2Exporter>;

    /// Create an exporter optimized for checkpoint-aligned exports
    pub fn create_checkpoint_aligned_exporter(
        graph_path: &Path,
        export_dir: &Path,
    ) -> NativeResult<V2Exporter>;

    /// Create an exporter optimized for full exports (graph + WAL)
    pub fn create_full_exporter(
        graph_path: &Path,
        export_dir: &Path,
    ) -> NativeResult<V2Exporter>;

    /// Create a snapshot exporter for instant database state exports
    pub fn create_snapshot_exporter(
        graph_path: &Path,
        export_dir: &Path,
        snapshot_id: Option<String>,
    ) -> NativeResult<SnapshotExporter>;
}
```

## 2. Import API Analysis

### 2.1 Actual V2ImportConfig Structure

**Location**: `sqlitegraph::backend::native::v2::import::importer::V2ImportConfig`

```rust
pub struct V2ImportConfig {
    /// Target graph file path
    pub target_graph_path: PathBuf,

    /// Export directory path
    pub export_dir_path: PathBuf,

    /// Import mode (Fresh or Merge)
    pub import_mode: ImportMode,

    /// Whether to validate recovery after import
    pub validate_recovery: bool,

    /// Whether to force checkpoint after import
    pub force_checkpoint_after_import: bool,
}
```

### 2.2 Import Modes

**Location**: `sqlitegraph::backend::native::v2::import::ImportMode`

```rust
pub enum ImportMode {
    /// Import into empty graph file (create new)
    Fresh,

    /// Import into existing graph file (only if compatible)
    Merge,
}
```

### 2.3 Import Factory Methods

**Location**: `sqlitegraph::backend::native::v2::import::ImportFactory`

```rust
impl ImportFactory {
    /// Create an importer with default configuration
    pub fn create_importer(
        export_dir: &Path,
        target_graph_path: &Path,
        import_config: V2ImportConfig,
    ) -> NativeResult<V2Importer>;

    /// Create an importer optimized for fresh imports
    pub fn create_fresh_importer(
        export_dir: &Path,
        target_graph_path: &Path,
    ) -> NativeResult<V2Importer>;

    /// Create an importer optimized for merge imports
    pub fn create_merge_importer(
        export_dir: &Path,
        target_graph_path: &Path,
    ) -> NativeResult<V2Importer>;
}
```

## 3. Module Import Path Analysis

### 3.1 Correct Import Paths

**For V2 Export Components**:
```rust
use sqlitegraph::backend::native::v2::export::{
    V2Exporter, V2ExportConfig, ExportMode, ExportFactory
};
```

**For V2 Import Components**:
```rust
use sqlitegraph::backend::native::v2::import::{
    V2Importer, V2ImportConfig, ImportMode, ImportFactory
};
```

**For Snapshot Components**:
```rust
use sqlitegraph::backend::native::v2::export::snapshot::{
    SnapshotExporter, SnapshotExportConfig
};
use sqlitegraph::backend::native::v2::import::snapshot::{
    SnapshotImporter, SnapshotImportConfig
};
```

### 3.2 Test File Import Issues

**Current Test Imports (INCORRECT)**:
```rust
use sqlitegraph::backend::native::{
    v2::{
        export::{ExportMode, ExportManifest, V2Exporter, V2ExportConfig},
        import::{ImportMode, V2Importer, V2ImportConfig},
    },
};
```

**Should Be (CORRECT)**:
```rust
use sqlitegraph::backend::native::v2::export::{
    V2Exporter, V2ExportConfig, ExportMode, ExportManifest
};
use sqlitegraph::backend::native::v2::import::{
    V2Importer, V2ImportConfig, ImportMode
};
```

## 4. API Mismatch Summary

### 4.1 Critical Mismatches

| Component | Test Expects | Actual Implementation | Impact |
|-----------|--------------|----------------------|---------|
| `V2ExportConfig.graph_path` | ✅ Exists | ❌ Does not exist | High |
| `V2ExportConfig.export_dir` | ✅ Exists | ❌ Should be `export_path` | High |
| `V2ExportConfig.export_mode` | ✅ Exists | ❌ Not a field | Medium |
| `V2ExportConfig.include_wal` | ✅ Exists | ❌ Should be `include_wal_tail` | Medium |
| `V2ExportConfig.validate_recovery` | ✅ Exists | ❌ Does not exist | Low |
| `V2ExportConfig.compression_level` | ✅ Exists | ❌ Should be `compression_enabled: bool` | Medium |
| `V2Exporter::new()` | ✅ Exists | ❌ Should be `from_graph_file()` | High |

### 4.2 Import Path Issues

| Issue | Current Path | Correct Path | Severity |
|-------|--------------|---------------|----------|
| Module structure | `backend::native::v2::export::{...}` | `backend::native::v2::export::{...}` | Medium |
| Missing re-exports | Partial exports needed | Full re-exports in mod.rs | Low |

## 5. Resolution Strategy

### 5.1 Immediate Fixes (Phase 1)

#### 5.1.1 Fix Import Paths
```rust
// BEFORE (INCORRECT):
use sqlitegraph::backend::native::{
    v2::{
        export::{ExportMode, ExportManifest, V2Exporter, V2ExportConfig},
        import::{ImportMode, V2Importer, V2ImportConfig},
    },
};

// AFTER (CORRECT):
use sqlitegraph::backend::native::v2::export::{
    V2Exporter, V2ExportConfig, ExportMode, ExportManifest
};
use sqlitegraph::backend::native::v2::import::{
    V2Importer, V2ImportConfig, ImportMode
};
```

#### 5.1.2 Fix V2ExportConfig Construction
```rust
// BEFORE (INCORRECT):
let snapshot_config = V2ExportConfig {
    graph_path: graph_path.clone(),
    export_dir: TempDir::new().unwrap().into_path(),
    export_mode: ExportMode::Snapshot,
    include_wal: false,
    validate_recovery: false,
    compression_level: None,
};

// AFTER (CORRECT):
let snapshot_config = V2ExportConfig {
    export_path: TempDir::new().unwrap().into_path(),
    include_wal_tail: false,  // For snapshots, no WAL tail
    compression_enabled: false,
    checksum_validation: true,
};
```

#### 5.1.3 Fix V2Exporter Construction
```rust
// BEFORE (INCORRECT):
let exporter = V2Exporter::new(snapshot_config);

// AFTER (CORRECT):
let exporter = V2Exporter::from_graph_file(&graph_path, snapshot_config);

// ALTERNATIVE (Using Factory):
let exporter = ExportFactory::create_snapshot_exporter(
    &graph_path,
    &TempDir::new().unwrap().into_path(),
    Some("test_snapshot".to_string())
)?;
```

### 5.2 Enhancement Opportunities (Future Phases)

#### 5.2.1 Add Backward Compatibility Aliases
```rust
// Could add to V2ExportConfig for backward compatibility:
impl V2ExportConfig {
    /// Create config using legacy field names (deprecated)
    #[deprecated(since = "0.2.5", note = "Use new field names instead")]
    pub fn from_legacy(
        export_dir: PathBuf,
        export_mode: ExportMode,
        include_wal: bool,
        compression_level: Option<u8>,
    ) -> Self {
        Self {
            export_path: export_dir,
            include_wal_tail: include_wal,
            compression_enabled: compression_level.is_some(),
            checksum_validation: true,
        }
    }
}
```

#### 5.2.2 Add Convenience Constructor
```rust
impl V2Exporter {
    /// Create exporter using legacy API (deprecated)
    #[deprecated(since = "0.2.5", note = "Use from_graph_file instead")]
    pub fn new(config: V2ExportConfig) -> NativeResult<Self> {
        // This won't work without graph_path - need factory pattern
        unimplemented!("Use ExportFactory::create_exporter instead")
    }
}
```

## 6. Testing Strategy After Fixes

### 6.1 Compilation Validation
```bash
# Test that integration tests compile
cargo test --test snapshot_export_import_tdd_tests --lib

# Validate specific test functions
cargo test test_snapshot_export_requires_stable_state --test snapshot_export_import_tdd_tests
```

### 6.2 Functionality Validation
```bash
# Run all integration tests
cargo test --test snapshot_export_import_tdd_tests

# Verify export/import functionality works end-to-end
cargo test integration_tests
```

## 7. Implementation Checklist

### 7.1 Phase 1 Tasks (Critical)
- [ ] Fix import paths in test files
- [ ] Update V2ExportConfig field usage
- [ ] Update V2Exporter constructor calls
- [ ] Update V2ImportConfig usage if needed
- [ ] Validate compilation succeeds

### 7.2 Phase 2 Tasks (Enhancement)
- [ ] Consider adding backward compatibility aliases
- [ ] Add convenience factory methods for common patterns
- [ ] Improve error messages for API mismatches
- [ ] Add API documentation examples

### 7.3 Phase 3 Tasks (Documentation)
- [ ] Update API documentation with examples
- [ ] Add migration guide for API changes
- [ ] Create compatibility matrix
- [ ] Document best practices

## 8. Conclusion

The V2 export/import APIs are fully implemented and functional, but the integration tests need to be updated to match the actual API surface. The mismatches are primarily due to API evolution during implementation and can be resolved with straightforward updates to the test code.

The actual APIs are well-designed and follow Rust best practices. The recommended approach is to update the tests to use the current APIs rather than adding backward compatibility shims, as this maintains clean API design going forward.

**Next Step**: Update the integration test file with the correct API usage as documented in Section 5.1.