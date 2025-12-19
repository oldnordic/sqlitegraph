# GraphFile Refactor Plan

## Mission
Safe modularization of `sqlitegraph/src/backend/native/graph_file.rs` (2014 lines) into focused modules (<300 LOC each) under `sqlitegraph/src/backend/native/graph_file/` with zero behavioral changes.

## Current State Analysis

### File: `sqlitegraph/src/backend/native/graph_file.rs`
- **Size**: 2014 lines (significantly over 300 LOC limit)
- **Largest module** in native backend
- **Complex responsibilities**: file I/O, header management, transactions, memory mapping, validation

### Key Concerns Identified
1. **IO Operations**: Multiple backends (std, mmap, exclusive), buffer management, coherence
2. **Header Layout**: 80-byte persistent header, big-endian encoding, version validation
3. **Encoding/Decoding**: Header serialization, safe slice access, validation logic
4. **Validation**: Bounds checking, commit markers, corruption prevention, transaction safety
5. **Memory Mapping**: Optional mmap support, conservative remapping, recursive call prevention

## Proposed Module Structure

### Target Directory: `sqlitegraph/src/backend/native/graph_file/`

```
graph_file/
├── mod.rs                    # Module exports and re-exports
├── file_ops.rs              # GraphFile struct, file creation/opening
├── header.rs                # Header encoding/decoding, persistent header ops
├── buffers.rs               # ReadBuffer, WriteBuffer implementations
├── transaction.rs           # Transaction lifecycle management
├── io_backend.rs            # I/O backend routing (std vs mmap vs exclusive)
├── mmap_ops.rs              # Memory mapping operations
├── validation.rs            # File size validation, corruption checks
├── encoding.rs              # Header encoding utilities and safe access
└── debug.rs                 # Debug instrumentation and logging
```

### Module Breakdown

#### `file_ops.rs` (~200 lines)
**Purpose**: Core GraphFile struct and basic file operations
**Public APIs**:
- `GraphFile::new()` - internal constructor
- `GraphFile::create()` - create new graph file
- `GraphFile::open()` - open existing graph file
- `file_path()`, `file_size()` - file metadata access
- `sync()`, `flush()` - system operations
**Lines**: 118-292 from original file

#### `header.rs` (~300 lines)
**Purpose**: Persistent header management and validation
**Public APIs**:
- `read_header()` - read file header with validation
- `write_header()` - write file header with persistence
- `persistent_header()`, `persistent_header_mut()` - header access
- `validate_header()` - header validation logic
**Lines**: 1343-1456 + 1709-1968 (header encoding/decoding)

#### `buffers.rs` (~150 lines)
**Purpose**: Adaptive buffer management for I/O optimization
**Public APIs**:
- `ReadBuffer` struct and methods
- `WriteBuffer` struct and methods
- Buffer sizing and management utilities
**Lines**: 34-116 (buffer management)

#### `transaction.rs` (~300 lines)
**Purpose**: Transaction lifecycle and commit management
**Public APIs**:
- `begin_transaction()` - start new transaction
- `commit_transaction()` - commit changes
- `rollback_transaction()` - rollback changes
- `begin_cluster_commit()`, `finish_cluster_commit()` - cluster operations
**Lines**: 1385-1500 + 645-750 (transaction logic)

#### `io_backend.rs` (~400 lines)
**Purpose**: I/O routing and backend selection
**Public APIs**:
- `read_bytes()`, `write_bytes()` - buffered I/O routing
- `read_bytes_direct()`, `write_bytes_direct()` - direct I/O routing
- Backend selection logic (std vs mmap vs exclusive)
**Lines**: 608-900 + 1278-1384 (I/O operations)

#### `mmap_ops.rs` (~250 lines)
**Purpose**: Memory mapping operations and management
**Public APIs**:
- `ensure_mmap_initialized()` - mmap initialization
- `mmap_read_bytes()`, `mmap_write_bytes()` - mmap I/O
- Conservative remapping logic
**Lines**: 1135-1277 (mmap operations)

#### `validation.rs` (~100 lines)
**Purpose**: File validation and corruption detection
**Public APIs**:
- `validate_file_size()` - file size validation
- `cluster_floor()` - cluster boundary calculation
- `verify_commit_marker()` - commit validation
**Lines**: 1500-1550 (validation logic)

#### `encoding.rs` (~100 lines)
**Purpose**: Safe header encoding/decoding utilities
**Public APIs**:
- Safe slice access helpers
- Encoding/decoding utilities
- Validation helper functions
**Lines**: 1770-1850 (safe access helpers)

#### `debug.rs` (~150 lines)
**Purpose**: Debug instrumentation and logging
**Public APIs**:
- Debug logging functions
- Corruption detection helpers
- Development-time validation
**Lines**: Scattered throughout original file

## Implementation Strategy

### Phase 1: TDD First - Regression Tests ✅ COMPLETED
1. ✅ Created comprehensive test suite `tests/graph_file_modularization_regression_tests.rs`
2. ✅ Tests designed to fail before refactor proving identical behavior
3. ✅ Covered critical paths:
   - ✅ File creation/opening/reopening
   - ✅ Header persistence across reopens
   - ✅ Transaction commit/rollback behavior
   - ✅ I/O consistency across operations
   - ✅ V1 format rejection and corruption handling
   - ✅ File growth and edge case operations
   - ✅ Error handling consistency
   - ✅ Backend selection compatibility

### Phase 2: Incremental Splitting
1. Create `graph_file/` directory structure
2. Implement `mod.rs` with careful re-exports
3. Split modules one by one, running tests after each split
4. Maintain all existing public APIs without changes

### Phase 3: Documentation
1. Create `docs/GRAPH_FILE_MODULE_MAP.md`
2. Document each module: purpose, public APIs, invariants, tests
3. Include integration points and dependency relationships

## Invariants to Preserve

### Functional Invariants
- Header roundtrip correctness: `encode(decode(header)) == header`
- Transaction atomicity: commit/rollback behavior unchanged
- File size consistency across all operations
- I/O coherence: buffered/unbuffered I/O equivalence
- Memory mapping safety: no invalid memory access

### Performance Invariants
- Read/write buffer adaptive sizing algorithms unchanged
- Memory mapping performance characteristics preserved
- Transaction commit latency maintained
- I/O amplification control logic intact

### Integration Invariants
- All public API signatures maintained exactly
- Error handling and propagation unchanged
- Debug instrumentation behavior preserved
- Corruption detection and prevention active

## Risk Mitigation

### Modularization Risks
- **Breaking changes**: Prevented by TDD approach and exact API preservation
- **Performance regression**: Mitigated by running benchmarks after each split
- **Complexity explosion**: Controlled by single-responsibility principle per module

### Testing Strategy
- Comprehensive regression tests covering all public APIs
- Integration tests verifying module interactions
- Performance benchmarks to ensure no regression
- Fuzzing tests for corruption detection edge cases

## Success Criteria

### Code Quality
- All modules < 300 LOC
- Each module has single, clear responsibility
- No circular dependencies between modules
- All existing tests pass without modification

### Functional Verification
- All public APIs behave identically to original
- Header roundtrip tests pass 100%
- Transaction integrity preserved
- File operations work across all backends

### Documentation
- `docs/GRAPH_FILE_MODULE_MAP.md` created and complete
- Each module documented with purpose and APIs
- Integration points clearly identified
- Invariants and test coverage documented

## File Path References
- **Source**: `sqlitegraph/src/backend/native/graph_file.rs` (lines 1-2014)
- **Constants**: `sqlitegraph/src/backend/native/constants.rs` (magic numbers, offsets)
- **Types**: `sqlitegraph/src/backend/native/types.rs` (error types)
- **Header**: `sqlitegraph/src/backend/native/persistent_header.rs` (header struct)
- **Tests**: `sqlitegraph/tests/` (existing graph file integration tests)

## Next Steps
1. Create `docs/GRAPH_FILE_MODULE_MAP.md` with detailed module mapping
2. Implement `tests/graph_file_modularization_tests.rs` with comprehensive regression tests
3. Begin module splitting starting with file_ops.rs
4. Validate each split with existing and new test suites
5. Update integration points as needed
6. Final verification with benchmarks and performance testing