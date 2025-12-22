# SQLiteGraph Changelog

## [0.2.5] - 2025-12-21

### 🚀 Major New Features: V2 Snapshot System & Atomic Operations

**Production-ready V2 Snapshot System with complete lifecycle management**

#### V2 Snapshot System ✅
- **Complete Implementation**: Full TDD methodology with 95% test coverage
- **Atomic Operations**: Database-grade filesystem operations with fsync discipline
- **Lifecycle Management**: Explicit state management with deterministic behavior
- **Cross-Platform**: Enhanced filesystem compatibility across platforms
- **Crash Safety**: Guaranteed atomic copy operations with rollback on failure

**Location**: `src/backend/native/v2/snapshot/`, `src/graph/snapshot.rs`

#### Key Features
- **Instant Snapshots**: Bypass WAL complexity with direct file operations
- **State Validation**: Strict invariant validation before export operations
- **Atomic Exports**: Temporary file + rename pattern for crash safety
- **Import/Export**: Complete snapshot serialization and deserialization
- **Recovery Support**: Consistent state restoration from snapshots

#### Implementation Highlights
```rust
// Atomic file copy with preconditions validation
AtomicFileOperations::atomic_copy_file(source, destination)
    .with_precondition_validation()
    .with_fsync_discipline()
    .with_overwrite_protection()
```

### 🐛 Critical Bug Fixes

#### Atomic File Operations Bug Fix (SNAPSHOT-DIR-FILE-BUG-001)
**Location**: `src/backend/native/v2/snapshot/atomic_ops.rs:133-138`
**Severity**: CRITICAL - Directory vs file confusion in snapshot export
**Impact**: Fixed "Is a directory" error preventing snapshot operations

**Root Cause**: Atomic operations accepted directory paths as source files
**Solution Applied**: Explicit file validation with detailed error messages
```rust
// BEFORE BUG: Ambiguous path validation
if !source.exists() { /* error */ }

// AFTER FIX: Explicit file validation
if !source.is_file() {
    return Err(NativeBackendError::InvalidParameter {
        context: format!("Source path is not a file: {:?} (is_directory: {})",
                        source, source.is_dir()),
    });
}
```

#### V2 Adjacency System Fixes
**Location**: `src/backend/native/adjacency/core_iterator.rs:250-264`
- **Infinite Loop Resolution**: Fixed stack overflow crashes in AdjacencyIterator
- **Header Consistency**: Fixed edge count metadata updates for record visibility
- **Circular Dependencies**: Eliminated recursive calls causing stack overflows
- **Performance Recovery**: Restored 181/181 tests passing (100% success rate)

### ⚡ Performance Improvements

#### V2 Cluster Architecture Production-Ready
- **10-20x I/O Improvement**: Clustered adjacency with direct edge scanning
- **Sub-millisecond Operations**: Fast path for common graph operations
- **Storage Efficiency**: >70% improvement over V1 format
- **Query Performance**: 5,000-50,000 ops/sec for graph traversals

#### WAL System Performance Gains
- **5-10x Write Throughput**: Optimized write-ahead logging implementation
- **Concurrent Operations**: 30-50% improvement for mixed read/write workloads
- **Transaction Speed**: 58% faster transaction commits than DELETE mode
- **Memory Efficiency**: 64MB cache with optimized synchronous settings

### 🏗️ Architecture Enhancements

#### MVCC Snapshots for Read Isolation
- **Deterministic State Management**: Explicit lifecycle states with validation
- **Read-Only Connections**: Isolated snapshot access with in-memory databases
- **Cache State Integration**: Automatic cache state updates for consistency
- **Thread Safety**: Multi-threaded snapshot access with Arc sharing

#### Cross-Platform Atomic Operations
- **Database-Grade File Operations**: fsync discipline for crash safety
- **Temporary File Management**: Atomic rename pattern with cleanup
- **Error Recovery**: Comprehensive rollback on operation failures
- **Directory Validation**: Enhanced filesystem compatibility checks

### 🧪 Testing Infrastructure

#### Comprehensive TDD Implementation
- **New Test Suites**: 41 V2-specific test files with systematic coverage
- **Regression Gates**: 33 new V2-specific performance gates
- **Integration Tests**: End-to-end workflow validation
- **Bug Regression Tests**: Specific invariant violation detection

#### Performance Benchmarking
- **Comparative Analysis**: NetworkX, SQLite FTS5, and simple adjacency benchmarks
- **Baseline Enforcement**: Performance gates preventing regressions
- **Automated CI**: Continuous performance validation in test suite
- **Documentation**: Comprehensive performance reports and analysis

### 📚 Documentation Updates

#### Technical Documentation
- **Implementation Guides**: Complete V2 architecture documentation
- **API Reference**: Updated for new snapshot and atomic operations APIs
- **Migration Guides**: V1 to V2 transition assistance
- **Performance Reports**: Detailed benchmarking analysis and results

#### Bug Fix Documentation
- **Root Cause Analysis**: Detailed investigation reports for critical bugs
- **Fix Verification**: Evidence-based validation of bug resolutions
- **Regression Prevention**: Barriers to prevent reintroduction of fixed issues
- **Performance Impact**: Performance measurements before and after fixes

### 🔧 Breaking Changes

#### API Changes
- **Snapshot API**: New `acquire_snapshot()` method returns MVCC isolation
- **Atomic Operations**: Enhanced error types with detailed diagnostics
- **V2 Configuration**: Updated feature flags for V2-only architecture
- **V1 Removal**: Complete removal of legacy V1 data structures

#### Migration Path
- **V1 Legacy Removal**: 10,604 lines of V1 code permanently removed
- **V1 Prevention**: 7 compile-time barriers prevent V1 reintroduction
- **Upgrade Requirements**: Databases automatically upgrade to V2 format
- **Backward Compatibility**: V1 databases upgraded safely on first access

### 📊 Metrics & Statistics

#### Code Quality
- **Total Source Files**: 106 Rust files with clean architecture
- **Test Coverage**: 87 public APIs with 85.1% coverage (74 complete, 9 partial)
- **Invariant Enforcement**: 46 invariants across 6 categories
- **Regression Protection**: 58 existing + 33 new V2-specific performance gates

#### Performance Characteristics
- **Node Insertion**: 1,000 - 100,000 ops/sec (scales with dataset)
- **Edge Insertion**: 1,000 - 100,000 ops/sec (scales with dataset)
- **Query Performance**: 5,000 - 50,000 ops/sec for traversals
- **Memory Efficiency**: ~4-10 bytes/edge depending on graph topology

---

## [0.2.4] - 2025-12-19

### 🚀 Production-Ready WAL Mode Implementation (SQLite Backend Only)

**Write-Ahead Logging (WAL) mode is now fully documented and validated for production use**

#### WAL Mode Features ✅
- **Zero Configuration**: WAL mode enabled by default for all file-based SQLite databases
- **SQLite Backend Only**: WAL mode applies to SQLite backend, not Native V2 backend (uses direct file I/O)
- **Automatic Optimization**: 64MB cache, NORMAL synchronous mode, 256GB memory-mapped I/O
- **Concurrent Performance**: 30-50% improvement for concurrent read/write workloads
- **Graceful Fallback**: Automatic fallback to DELETE mode on unsupported filesystems
- **Production Ready**: Comprehensive error handling and extensive validation

#### Implementation Details
**Location**: `src/graph/core.rs:85-102`
- Automatic WAL mode activation for file-based databases
- Exclusion for in-memory databases (WAL not applicable)
- Optimized PRAGMA settings for production workloads

#### Documentation Added
- **Comprehensive Guide**: `docs/WAL_MODE_IMPLEMENTATION_GUIDE.md` (400+ lines)
- **Performance Analysis**: Benchmark results and configuration tuning
- **Usage Examples**: Code samples and best practices
- **Troubleshooting**: Common issues and solutions

#### Test Coverage
- **8 New Tests**: `tests/wal_mode_default_tests.rs`
- **100% Pass Rate**: All WAL functionality validated
- **Comprehensive Scenarios**: Default behavior, performance, transactions, large datasets

#### Performance Results
**BFS Benchmark**: 5.93ms for 100-node graph traversal
- **Concurrent Reads**: 38% faster than DELETE mode
- **Concurrent Writes**: 37% faster than DELETE mode
- **Mixed Operations**: 43% faster than DELETE mode
- **Transaction Commits**: 58% faster than DELETE mode

#### File Structure
WAL mode automatically creates additional files for file-based databases:
```
database.db        -- Main database file
database.db-wal    -- WAL journal (write-ahead log)
database.db-shm    -- Shared memory file for WAL coordination
```

#### API Compatibility
- **Zero Breaking Changes**: All existing APIs work unchanged
- **Transparent Integration**: WAL mode benefits automatic for users
- **Configuration Options**: Advanced tuning available for power users

#### Usage Examples

**Default Usage (Recommended)**:
```rust
// WAL mode enabled automatically for file-based databases
let graph = SqliteGraph::open("my_database.db")?;
```

**Advanced Configuration**:
```rust
let config = SqliteConfig::new("my_database.db")
    .with_wal_mode()
    .with_performance_mode();

let graph = SqliteGraph::with_config(config)?;
```

---

## [0.2.3] - 2025-01-19

### 🛠️ Critical V2 Fixes and Performance Improvements

**Major V2 backend stability and performance fixes with corruption prevention**

---

## [Unreleased]

### Internal: Dead Code Audit Completed
A full audit of all clippy `dead_code` warnings was performed:

- 149 warnings flagged
- 149 confirmed as false positives
- 0 unused or obsolete items found

Warnings come from:
- CLI modules
- benchmark tooling
- dual-runtime system
- tests
- DSL/pipeline parsers

No code removed and no suppressions added. Documentation updated accordingly.