# Formal Export System Design for SQLiteGraph V2

## Overview

This document defines the formal export system for SQLiteGraph V2 that reuses existing WAL, checkpointing, and crash recovery subsystems. This is NOT a utility script but a FIRST-CLASS DATABASE FEATURE that produces deterministic, crash-safe snapshots of V2 graph databases.

## Design Constraints

- **≤300 LOC per file**: All implementation files must respect the 300-line limit
- **No async**: Pure synchronous Rust implementation using existing APIs
- **No external dependencies**: Uses only existing SQLiteGraph components
- **Deterministic behavior**: Same inputs always produce same outputs
- **TDD methodology**: All tests initially FAIL before implementation

## Existing Architecture Analysis

### LSN Range Representation (from `backend/native/v2/wal/mod.rs`)

**V2WALHeader Structure** (lines 155-184):
```rust
pub struct V2WALHeader {
    pub magic: [u8; 8],
    pub version: u32,
    pub created_at: u64,
    pub current_lsn: u64,         // Current LSN position in WAL
    pub committed_lsn: u64,       // Highest committed LSN
    pub checkpointed_lsn: u64,    // Highest checkpointed LSN
    pub active_transactions: u32,
    pub flags: u32,
    pub reserved: [u64; 4],
}
```

**LSN Invariant**: `checkpointed_lsn <= committed_lsn <= current_lsn`

**LSN Utilities** (lines 279-304):
- `LSN_BEGIN: u64 = 1` - Start of WAL
- `LSN_INVALID: u64 = 0` - Invalid/empty position
- `lsn::distance(from, to)` - Calculate range size
- `lsn::next(lsn)` - Get next LSN

### Checkpoint Consistency Boundaries (from `backend/native/v2/wal/checkpoint/`)

**Explicit Recovery States** (from `backend/native/v2/wal/recovery/states.rs`):
- **CleanShutdown**: `committed_lsn == current_lsn AND active_transactions == 0`
- **PartialCheckpoint**: `checkpoint_exists AND checkpointed_lsn < committed_lsn`
- **DirtyShutdown**: `active_transactions > 0 OR committed_lsn < current_lsn`

**Checkpoint Module Architecture** (`checkpoint/mod.rs:1-30`):
- **Core**: `V2WALCheckpointManager`, `DirtyBlockTracker`, `CheckpointState`
- **Strategies**: Size-based, time-based, transaction-based, adaptive
- **Operations**: `BlockFlusher`, `CheckpointExecutor`, `V2GraphIntegrator`
- **Validation**: Metrics collection, consistency checking

### WAL Record Types (from `backend/native/v2/wal/record.rs`)

**Supported Records** (lines 14-97):
- Node operations: `NodeInsert`, `NodeUpdate`, `NodeDelete`
- Edge operations: `ClusterCreate`, `EdgeInsert`, `EdgeUpdate`, `EdgeDelete`
- Transaction control: `TransactionBegin`, `TransactionCommit`, `TransactionRollback`
- System operations: `Checkpoint`, `HeaderUpdate`, `SegmentEnd`

**Record Serialization**:
- Type byte + 4-byte size + data payload
- Cluster affinity for I/O optimization
- `V2WALSerializer::serialize()`/`deserialize()` methods

### WAL Reading Capabilities (from `backend/native/v2/wal/reader.rs`)

**V2WALReader API**:
```rust
pub struct V2WALReader {
    file: Arc<BufReader<File>>,
    header: V2WALHeader,
    current_position: u64,
    wal_end: u64,
}

pub struct WALReadFilter {
    pub record_types: Option<Vec<V2WALRecordType>>,
    pub lsn_range: Option<(u64, u64)>,      // Key for export ranges
    pub cluster_keys: Option<Vec<i64>>,
    pub data_modifying_only: bool,
    pub transaction_control_only: bool,
}
```

### Graph File Access (from `backend/native/graph_file/`)

**GraphFile Core API** (`graph_file_core.rs:26-38`):
```rust
impl GraphFile {
    pub fn create<P: AsRef<Path>>(path: P) -> NativeResult<Self>;
    pub fn open<P: AsRef<Path>>(path: P) -> NativeResult<Self>;
    pub fn read_header(&mut self) -> NativeResult<()>;
    pub fn header(&self) -> &PersistentHeaderV2;
    pub fn write_header(&mut self) -> NativeResult<()>;
    pub fn sync(&mut self) -> NativeResult<()>;
    pub fn file_size(&self) -> NativeResult<u64>;
}
```

### Bulk Ingest Integration (from `backend/native/v2/wal/bulk_ingest.rs`)

**RAII Pattern**:
```rust
pub struct BulkIngestGuard<'a> {
    manager: &'a V2WALManager,
    config: BulkIngestConfig,
    records_written: u64,
    start_metrics: WALManagerMetrics,
}

impl<'a> Drop for BulkIngestGuard<'a> {
    fn drop(&mut self) {
        let _ = self.finish_bulk_session();  // Ensures consistency
    }
}
```

## CORRECTED EXPORT SYSTEM DESIGN (Based on ACTUAL APIs)

### Actual WAL Manager API (CORRECTED)

**V2WALManager Interface** (from manager.rs:163-221):
```rust
impl V2WALManager {
    pub fn create(config: V2WALConfig) -> NativeResult<Self>;
    pub fn begin_transaction(&self, isolation_level: TransactionIsolation) -> NativeResult<u64>;
    pub fn write_transaction_record(&self, tx_id: u64, record: V2WALRecord) -> NativeResult<u64>;
    pub fn commit_transaction(&self, tx_id: u64) -> NativeResult<()>;
    pub fn rollback_transaction(&self, tx_id: u64) -> NativeResult<()>;
    pub fn force_checkpoint(&self) -> NativeResult<()>;
    pub fn get_header(&self) -> V2WALHeader;
    pub fn get_metrics(&self) -> WALManagerMetrics;
    pub fn write_record(&self, record: V2WALRecord) -> NativeResult<u64>;
    pub fn write_records_batch(&self, records: Vec<V2WALRecord>) -> NativeResult<Vec<u64>>;
    pub fn flush(&self) -> NativeResult<()>;
}
```

### Actual V2WALRecord Types (CORRECTED)

**V2WALRecord Variants** (from record.rs:178-259):
```rust
pub enum V2WALRecord {
    NodeInsert { node_id: i64, slot_offset: u64, node_data: Vec<u8> },
    NodeUpdate { node_id: i64, slot_offset: u64, old_data: Vec<u8>, new_data: Vec<u8> },
    NodeDelete { node_id: i64, slot_offset: u64, old_data: Vec<u8> },
    ClusterCreate { node_id: i64, direction: Direction, cluster_offset: u64, cluster_size: u32, edge_data: Vec<u8> },
    EdgeInsert { cluster_key: (i64, Direction), edge_record: CompactEdgeRecord, insertion_point: u32 },
    EdgeUpdate { cluster_key: (i64, Direction), old_edge: CompactEdgeRecord, new_edge: CompactEdgeRecord, position: u32 },
    EdgeDelete { cluster_key: (i64, Direction), old_edge: CompactEdgeRecord, position: u32 },
    StringInsert { string_id: u32, string_value: String },
    FreeSpaceAllocate { block_offset: u64, block_size: u32, block_type: u8 },
    FreeSpaceDeallocate { block_offset: u64, block_size: u32, block_type: u8 },
    TransactionBegin { tx_id: u64, timestamp: u64 },
    TransactionCommit { tx_id: u64, timestamp: u64 },
    TransactionRollback { tx_id: u64, timestamp: u64 },
    Checkpoint { checkpointed_lsn: u64, timestamp: u64 },
    HeaderUpdate { header_offset: u64, old_data: Vec<u8>, new_data: Vec<u8> },
    SegmentEnd { segment_lsn: u64, checksum: u32 },
    // ... other variants
}
```

### Actual GraphFile API (CORRECTED)

**GraphFile Interface** (from graph_file_core.rs:27-38):
```rust
impl GraphFile {
    pub fn create<P: AsRef<Path>>(path: P) -> NativeResult<Self>;
    pub fn open<P: AsRef<Path>>(path: P) -> NativeResult<Self>;
    pub fn read_header(&mut self) -> NativeResult<()>;
    pub fn current_transaction_id(&self) -> u64;
    pub fn is_transaction_active(&self) -> bool;
    pub fn begin_transaction(&mut self) -> NativeResult<u64>;
    pub fn commit_transaction(&mut self) -> NativeResult<()>;
}
```

### Actual V2WALReader API (CORRECTED)

**V2WALReader Interface** (from reader.rs:165):
```rust
impl V2WALReader {
    pub fn open(wal_path: &Path) -> NativeResult<Self>;
    // Reading and iteration methods for WAL records
}
```

### Actual Recovery State (CORRECTED)

**Correct Import Path** (from recovery/states.rs:24):
```rust
pub enum RecoveryState {
    CleanShutdown,
    DirtyShutdown,
    PartialCheckpoint,
    CorruptWAL,
    CorruptGraphFile,
    Unrecoverable,
}

pub enum Authority {
    WAL,
    GraphFile,
    Unrecoverable,
}
```

## Export Architecture

The export system operates on **CONSISTENT VIEWS** using existing components:

1. **Consistency Detection**: Use `RecoveryContext::analyze_files()` from recovery module
2. **Range Definition**: Leverage existing `WALReadFilter::by_lsn_range(start_lsn, end_lsn)`
3. **Data Extraction**: Use `V2WALReader` with existing filters
4. **Graph State**: Read directly from `GraphFile` for checkpointed data
5. **Manifest Generation**: Create metadata describing export boundaries

### Export Modes

**Mode 1: Checkpoint-Aligned Export**
- Preconditions: `checkpointed_lsn == committed_lsn AND active_transactions == 0`
- Export: Graph file snapshot only (no WAL tail needed)
- Use case: Clean database exports

**Mode 2: LSN-Bounded Export**
- Preconditions: `checkpointed_lsn < committed_lsn`
- Export: Graph file + WAL records from `checkpointed_lsn..committed_lsn`
- Use case: Export with recent uncommitted transactions

**Mode 3: Full WAL Export**
- Preconditions: Any valid WAL file
- Export: Graph file + all WAL records from `checkpointed_lsn..current_lsn`
- Use case: Complete database state including in-flight changes

### Export Components

**File Structure**:
```
src/backend/native/v2/export/
├── mod.rs          (≤300 LOC) - Module exports and factory
├── exporter.rs     (≤300 LOC) - Main export orchestration
└── manifest.rs     (≤300 LOC) - Manifest generation and validation
```

**Output Files**:
- `export.graph` - Graph file snapshot
- `export.wal` - WAL tail records (if needed)
- `export.manifest` - Metadata and consistency information

### Export API Design

**Main Export Interface** (`exporter.rs`):
```rust
pub struct V2ExportConfig {
    pub export_path: PathBuf,
    pub include_wal_tail: bool,
    pub compression_enabled: bool,
    pub checksum_validation: bool,
}

pub struct V2Exporter {
    config: V2WALConfig,
    graph_file: GraphFile,
    wal_reader: Option<V2WALReader>,
}

impl V2Exporter {
    /// Create exporter from existing graph file
    pub fn from_graph_file(
        graph_path: &Path,
        export_config: V2ExportConfig,
    ) -> NativeResult<Self>;

    /// Perform consistency analysis before export
    pub fn analyze_consistency(&self) -> NativeResult<ExportConsistencyReport>;

    /// Export with checkpoint-aligned consistency
    pub fn export_checkpoint_aligned(&self) -> NativeResult<ExportResult>;

    /// Export with LSN-bounded consistency
    pub fn export_lsn_bounded(&self, from_lsn: u64, to_lsn: u64) -> NativeResult<ExportResult>;

    /// Export full database state (graph + WAL)
    pub fn export_full(&self) -> NativeResult<ExportResult>;
}
```

**Export Result Types**:
```rust
pub struct ExportResult {
    pub manifest_path: PathBuf,
    pub graph_file_path: PathBuf,
    pub wal_file_path: Option<PathBuf>,
    pub records_exported: u64,
    pub bytes_exported: u64,
    pub export_duration: Duration,
    pub checksum: u64,
}

pub struct ExportConsistencyReport {
    pub recovery_state: ExplicitRecoveryState,
    pub authority: Authority,
    pub checkpoint_lsn: u64,
    pub committed_lsn: u64,
    pub current_lsn: u64,
    pub active_transactions: u32,
    pub recommended_export_mode: ExportMode,
}
```

### Manifest Format (`manifest.rs`)

**Manifest Structure** (based on existing header patterns):
```rust
pub struct ExportManifest {
    // Format identification
    pub magic: [u8; 8],                    // "V2EXPMF"
    pub version: u32,                      // Manifest format version

    // Consistency information
    pub recovery_state: ExplicitRecoveryState,
    pub authority: Authority,
    pub export_mode: ExportMode,

    // LSN boundaries
    pub graph_checkpoint_lsn: u64,
    pub wal_start_lsn: Option<u64>,
    pub wal_end_lsn: Option<u64>,

    // Format compatibility
    pub graph_format_version: u32,
    pub wal_format_version: u32,
    pub v2_clustered_edges: bool,

    // Integrity
    pub export_timestamp: u64,
    pub export_duration_ms: u64,
    pub graph_checksum: u64,
    pub wal_checksum: Option<u64>,
    pub total_records: u64,
    pub total_bytes: u64,

    // Reserved for future
    pub reserved: [u64; 8],
}
```

**Manifest Serialization**:
- Reuse existing `V2WALSerializer` patterns
- Binary format with magic bytes and version
- Little-endian encoding (consistent with WAL)
- Checksum validation using existing patterns

### Export Workflow

**Step 1: Pre-Export Analysis**
1. Open `GraphFile` using `GraphFile::open()`
2. Analyze files using `RecoveryContext::analyze_files()`
3. Determine consistency state and recommended export mode
4. Validate export preconditions

**Step 2: Graph File Export**
1. Read current graph state from `GraphFile`
2. Copy graph file to export location
3. Validate graph file integrity using existing validation
4. Calculate graph file checksum

**Step 3: WAL Tail Export (if needed)**
1. Create `V2WALReader` from WAL file
2. Use `WALReadFilter::by_lsn_range()` for range extraction
3. Serialize filtered records using `V2WALSerializer`
4. Calculate WAL checksum

**Step 4: Manifest Generation**
1. Create `ExportManifest` with all metadata
2. Serialize manifest using existing patterns
3. Validate manifest integrity
4. Write manifest file

**Step 5: Final Validation**
1. Verify all files written successfully
2. Validate checksums match
3. Ensure export consistency guarantees
4. Return `ExportResult` with metrics

### Consistency Guarantees

**Export Atomicity**:
- Either all export files are created successfully or none
- Manifest is written last to indicate completion
- Partial exports are detectable and rejected

**Crash Safety**:
- Export never mutates source files
- Uses read-only operations on source graph/WAL
- Existing crash recovery can detect interrupted exports

**Deterministic Results**:
- Same graph state + same configuration = identical export
- Export boundaries are explicitly defined by LSN ranges
- No timing-dependent or non-deterministic operations

### Error Handling

**Export Failure Modes**:
- **Source Corruption**: Detected by `RecoveryContext` analysis
- **Insufficient Permissions**: File access errors
- **Disk Space**: Export file creation failures
- **Checksum Mismatch**: Data integrity validation failures
- **Version Incompatibility**: Format version mismatches

**Recovery from Failures**:
- All intermediate files are cleaned up on failure
- No partial exports left behind
- Source database remains unchanged

### Integration Points

**Recovery System Integration**:
- Reuse `RecoveryContext::analyze_files()` for consistency detection
- Leverage existing `ExplicitRecoveryState` and `Authority` types
- Use existing LSN validation logic

**WAL System Integration**:
- Use `V2WALReader` for WAL file access
- Leverage `WALReadFilter` for LSN range extraction
- Reuse `V2WALRecord` serialization patterns

**Graph File Integration**:
- Use `GraphFile::open()` for graph file access
- Leverage existing validation and checksum patterns
- Reuse file I/O operations from `graph_file/` modules

**Bulk Ingest Integration**:
- Export format compatible with `BulkIngestGuard` expectations
- WAL tail format matches bulk ingest input requirements
- Consistency boundaries align with bulk ingest checkpoints

## Non-Goals

### What Export Never Does

- **Mutate source files**: Export is read-only on source database
- **Perform schema migration**: Export preserves existing format exactly
- **Compress data beyond basic**: No complex compression algorithms
- **Handle version upgrades**: Export only works with compatible versions
- **Provide incremental exports**: Only full snapshot exports supported

### Export Limitations

- **Requires consistent state**: Cannot export from corrupted databases
- **Single-threaded**: No parallel export operations
- **Local filesystem only**: No remote export capabilities
- **Memory constraints**: Large graphs may require streaming exports

## Implementation Phases

### Phase 1: Core Export Engine
- `V2Exporter` with checkpoint-aligned exports
- `ExportManifest` generation and validation
- Basic error handling and cleanup

### Phase 2: LSN-Bounded Exports
- WAL tail extraction using existing filters
- Consistency boundary validation
- Integration with recovery state analysis

### Phase 3: Advanced Features
- Compression support
- Progress reporting
- Performance optimizations

### Phase 4: Validation and Testing
- Comprehensive TDD test suite
- Integration with existing test harness
- Performance benchmarking

## Quality Assurance

### Testing Strategy
- **Unit Tests**: Each export component tested in isolation
- **Integration Tests**: End-to-end export workflows
- **Crash Tests**: Export interruption and recovery
- **Consistency Tests**: Export/import round-trip validation
- **Performance Tests**: Export speed and resource usage

### Code Quality Standards
- **≤300 LOC per file**: Strict adherence to file size limits
- **No unsafe code**: Safe Rust only
- **Comprehensive error handling**: All error paths tested
- **Documentation**: All public APIs documented
- **Code review**: All changes peer-reviewed

### Performance Targets
- **Export Speed**: ≥50MB/s for large graphs
- **Memory Usage**: ≤100MB for typical export operations
- **Overhead**: ≤5% additional storage for manifest
- **Validation Time**: ≤10% of export duration

## Formal Import System Design

### Import Architecture

The import system is the inverse of export - a WAL-backed, recovery-verifiable operation that reconstructs database state from exported artifacts. Import maintains all crash recovery semantics and validates integrity through the existing recovery subsystem.

**Import Design Principles**:
1. **WAL-Backed**: All writes go through WAL using existing `V2WALManager`
2. **Bulk Ingest Integration**: Use `BulkIngestGuard` for optimal performance
3. **Recovery Validation**: Run crash recovery even without a crash
4. **Manifest-Driven**: Strict validation using export manifest metadata
5. **Atomic Operations**: Either complete import or rollback entirely

### Import Components

**File Structure**:
```
src/backend/native/v2/import/
├── mod.rs          (≤300 LOC) - Module exports and factory
├── importer.rs     (≤300 LOC) - Main import orchestration
└── validation.rs   (≤300 LOC) - Import validation and recovery
```

**Required Input Files**:
- `export.graph` - Graph file snapshot (required)
- `export.wal` - WAL tail records (optional, based on manifest)
- `export.manifest` - Metadata and consistency information (required)

### Import API Design

**Main Import Interface** (`importer.rs`):
```rust
pub struct V2ImportConfig {
    pub target_graph_path: PathBuf,
    pub export_dir_path: PathBuf,
    pub import_mode: ImportMode,
    pub validate_recovery: bool,
    pub force_checkpoint_after_import: bool,
}

#[derive(Debug, Clone)]
pub enum ImportMode {
    /// Import into empty graph file (create new)
    Fresh,
    /// Merge into existing graph file (only if compatible)
    Merge,
}

pub struct V2Importer {
    config: V2ImportConfig,
    manifest: ExportManifest,
    wal_config: V2WALConfig,
}

impl V2Importer {
    /// Create importer from export directory
    pub fn from_export_dir(
        export_dir_path: &Path,
        target_graph_path: &Path,
        import_config: V2ImportConfig,
    ) -> NativeResult<Self>;

    /// Validate export before import
    pub fn validate_export(&self) -> NativeResult<ImportValidationReport>;

    /// Perform import into target graph
    pub fn import(&self) -> NativeResult<ImportResult>;
}
```

### Import Workflow

**Step 1: Export Validation**
1. Read `ExportManifest` from export directory
2. Validate manifest magic bytes and version compatibility
3. Verify all required files exist (`export.graph`, `export.manifest`)
4. Check optional WAL file matches manifest expectations
5. Validate format compatibility (graph version, WAL version)

**Step 2: Target Graph Preparation**
1. **Fresh Mode**: Create new empty graph file using `GraphFile::create()`
2. **Merge Mode**: Open existing graph file, validate compatibility
3. Initialize WAL system using `V2WALManager::create()`
4. Set up bulk ingest mode for optimal performance

**Step 3: Graph File Import**
1. Copy `export.graph` to target location using existing file I/O
2. Validate copied graph file integrity using existing validation
3. Update target graph file header if needed for merge compatibility
4. Initialize WAL with checkpointed LSN from manifest

**Step 4: WAL Tail Replay (if present)**
1. Open WAL tail file using `V2WALReader`
2. Validate WAL tail checksum against manifest
3. Begin bulk ingest session using `BulkIngestGuard::new()`
4. Replay WAL records using existing transaction patterns:
   ```rust
   let tx_id = manager.begin_transaction(TransactionIsolation::ReadCommitted)?;
   for record in wal_records {
       manager.write_transaction_record(tx_id, record)?;
   }
   manager.commit_transaction(tx_id)?;
   ```
5. Complete bulk ingest session (automatic via RAII)

**Step 5: Post-Import Recovery Validation**
1. Force checkpoint using existing checkpoint manager
2. Run crash recovery validation using `RecoveryContext::analyze_files()`
3. Verify consistency state matches export expectations
4. Validate LSN boundaries and checksums
5. Ensure no active transactions remain

### Import Validation (`validation.rs`)

**Pre-Import Validation**:
```rust
pub struct ImportValidationReport {
    pub manifest_valid: bool,
    pub files_exist: bool,
    pub format_compatible: bool,
    pub target_compatible: bool,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

pub struct ImportValidator {
    manifest: ExportManifest,
    export_dir: PathBuf,
    target_path: PathBuf,
}

impl ImportValidator {
    /// Validate manifest integrity and format
    pub fn validate_manifest(&self) -> NativeResult<()>;

    /// Validate all required export files exist
    pub fn validate_files(&self) -> NativeResult<()>;

    /// Validate format compatibility
    pub fn validate_compatibility(&self) -> NativeResult<()>;

    /// Validate target graph for merge operations
    pub fn validate_target_compatibility(&self) -> NativeResult<()>;
}
```

**Post-Import Validation**:
```rust
pub struct ImportResult {
    pub records_imported: u64,
    pub wal_records_replayed: u64,
    pub import_duration: Duration,
    pub final_lsn: u64,
    pub recovery_state: ExplicitRecoveryState,
    pub validation_passed: bool,
}

pub struct PostImportValidator {
    wal_path: PathBuf,
    graph_path: PathBuf,
    expected_lsn: u64,
}

impl PostImportValidator {
    /// Run recovery validation after import
    pub fn validate_recovery(&self) -> NativeResult<ExplicitRecoveryState>;

    /// Verify final database state consistency
    pub fn validate_consistency(&self) -> NativeResult<()>;

    /// Validate LSN boundaries match expectations
    pub fn validate_lsn_boundaries(&self) -> NativeResult<()>;
}
```

### Import Modes

**Fresh Import Mode**:
- Creates new empty graph file at target path
- No compatibility validation needed beyond format support
- Best for database restoration and migration scenarios
- Guarantees clean import state

**Merge Import Mode**:
- Opens existing target graph file
- Validates compatibility before import:
  - Same V2 format version
  - Compatible cluster layouts
  - No conflicting node/edge IDs
- Merges export data with existing data
- Useful for incremental data updates

### Bulk Ingest Integration

**Performance Optimization**:
```rust
impl V2Importer {
    fn import_with_bulk_optimization(&self) -> NativeResult<()> {
        // Create WAL manager
        let wal_config = V2WALConfig::for_graph_file(&self.config.target_graph_path);
        let manager = V2WALManager::create(wal_config)?;

        // Begin bulk ingest for optimal performance
        let bulk_config = BulkIngestConfig {
            max_batch_size_bytes: 50 * 1024 * 1024, // 50MB batches
            flush_timeout_ms: 10_000,                // 10 second timeout
            force_checkpoint_on_exit: true,
            max_records_per_batch: 50_000,
        };

        let _bulk_guard = manager.begin_bulk_ingest(bulk_config)?;

        // Import graph file (already handled in Step 3)

        // Replay WAL tail if present
        if self.manifest.wal_start_lsn.is_some() {
            self.replay_wal_tail_through_bulk(&manager)?;
        }

        // Bulk guard automatically completes on drop
        Ok(())
    }
}
```

### Error Handling and Recovery

**Import Failure Modes**:
- **Manifest Corruption**: Invalid magic bytes, version mismatch
- **File Missing**: Required export files not found
- **Format Incompatibility**: Version mismatches between export and target
- **Checksum Validation**: Export file integrity failures
- **WAL Replay Failures**: Transaction errors during import
- **Recovery Validation**: Post-import consistency failures

**Recovery from Failures**:
```rust
impl V2Importer {
    fn rollback_on_failure(&self) -> NativeResult<()> {
        match self.config.import_mode {
            ImportMode::Fresh => {
                // Remove partially created target files
                if self.config.target_graph_path.exists() {
                    std::fs::remove_file(&self.config.target_graph_path)?;
                }
                let wal_path = self.config.target_graph_path.with_extension("wal");
                if wal_path.exists() {
                    std::fs::remove_file(&wal_path)?;
                }
            }
            ImportMode::Merge => {
                // Target was existing file - it should be unchanged
                // WAL replay would be transactional and atomic
            }
        }
        Ok(())
    }
}
```

### Integration with Existing Systems

**WAL System Integration**:
- Use `V2WALManager::create()` for WAL initialization
- Leverage `begin_transaction()`/`commit_transaction()` patterns
- Use existing `write_transaction_record()` API
- Integrate with `BulkIngestGuard` for performance

**Recovery System Integration**:
- Use `RecoveryContext::analyze_files()` for post-import validation
- Leverage existing `ExplicitRecoveryState` and `Authority` logic
- Run recovery validation even without crashes
- Use existing checkpoint management

**Graph File Integration**:
- Use `GraphFile::create()`/`open()` for target graph
- Leverage existing validation and checksum patterns
- Reuse file I/O operations from `graph_file/` modules
- Maintain V2 clustered edge format compatibility

### Import Guarantees

**Atomicity**:
- Either complete import or rollback to original state
- No partially imported database states
- Transaction boundaries preserved through WAL

**Consistency**:
- Post-import recovery validation ensures database integrity
- LSN boundaries match export expectations
- No active transactions remaining after import

**Performance**:
- Bulk ingest mode provides 5-10x performance improvement
- WAL replay uses existing optimized transaction patterns
- Checkpoint at import completion ensures clean state

**Determinism**:
- Same export + same configuration = identical import result
- Reproducible import behavior across runs
- No timing-dependent or random operations

### Import Validation Scenarios

**Scenario 1: Fresh Import with WAL Tail**
```rust
let config = V2ImportConfig {
    target_graph_path: PathBuf::from("restored.v2"),
    export_dir_path: PathBuf::from("export_20240101"),
    import_mode: ImportMode::Fresh,
    validate_recovery: true,
    force_checkpoint_after_import: true,
};

let importer = V2Importer::from_export_dir(
    &config.export_dir_path,
    &config.target_graph_path,
    config,
)?;

let validation = importer.validate_export()?;
assert!(validation.manifest_valid);
assert!(validation.format_compatible);

let result = importer.import()?;
assert!(result.validation_passed);
assert_eq!(result.final_lsn, expected_lsn);
```

**Scenario 2: Merge Import Compatibility Check**
```rust
// Validate merge compatibility before import
let validator = ImportValidator::new(export_dir, target_graph);
let compatibility = validator.validate_target_compatibility()?;

if !compatibility.is_mergeable {
    return Err(NativeBackendError::IncompatibleMerge {
        reason: compatibility.incompatibility_reason,
    });
}
```

## Conclusion

The formal export and import systems provide deterministic, crash-safe database operations by reusing existing SQLiteGraph V2 components. The export system produces consistent snapshots with clear LSN boundaries, while the import system reconstructs database state through WAL-backed operations with full recovery validation. Both systems maintain strict consistency guarantees while providing multiple modes optimized for different use cases. The design respects all existing architectural constraints and integrates seamlessly with the current WAL, checkpoint, bulk ingest, and recovery systems.