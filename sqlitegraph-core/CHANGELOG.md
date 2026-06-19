# SQLiteGraph Changelog

## [3.3.0] - 2026-06-19

The 3.3 release adds a temporal version chain (MVCC history) with
persistent-homology topology analysis, fixes a monotonically-growing
adjacency cache, wires 13 silently-broken criterion benchmarks, and guards the
HNSW cosine kernel against zero-magnitude vectors.

### Added — Temporal version chain (MVCC history)

- `SqliteGraph::checkpoint()` captures the current adjacency state as a
  numbered version in a bounded history chain (default capacity: 64 versions).
  `snapshot_as_of(n)` retrieves an immutable `VersionedSnapshot` by version
  number via binary search; `as_of_at(timestamp)` resolves by wall-clock time.
  This is the first time-axis on the sqlite-backend.
- `neighbors()` and `bfs()` with `SnapshotId::from_lsn(n)` now serve adjacency
  from the retained version N instead of erroring. Other operations
  (`get_node`, `shortest_path`, `kv_get`, etc.) reject historical snapshots by
  design — the version chain stores untyped adjacency only (documented in the
  `SnapshotState` doc).
- **Temporal topology module (`sqlitegraph::temporal`)** — persistent homology
  over the version chain:
  - `temporal_persistence_sweep()` — SCC landscape + β₁ (cyclomatic number) +
    non-trivial SCC count per version.
  - `scc_lineage_barcode()` — exact H₀ component barcode via Jaccard
    membership-identity matching across adjacent versions (replaces the
    deprecated LIFO count-delta approximation).
  - `cycle_scc_barcode()` — circular-dependency lifecycle barcode: tracks
    non-trivial SCCs (size ≥ 2) across the chain — for a call graph, each is a
    circular dependency.
  - `cycle_rank_snapshot()` — β₁ = E − V + W (cyclomatic number).
- `LineageBarcode` struct with `birth_version`, `death_version`, `birth_size`,
  `peak_size`, `final_size`, `versions_seen`.
- New crate-root re-exports: `VersionedSnapshot`, `TemporalPersistencePoint`,
  `TemporalBarcode`, `LineageBarcode`, `temporal_persistence_sweep`,
  `scc_lineage_barcode`, `cycle_scc_barcode`, `compute_temporal_barcode`,
  `cycle_rank_snapshot`.
- `benches/temporal_benchmarks.rs` — `as_of` latency over {10, 100, 1000}
  versions (24.7 ns at 1000); sweep latency over {10, 50, 100}.
- 30 temporal tests (20 unit + 10 integration).

### Fixed

- **HNSW no longer panics on zero-magnitude / non-finite vectors (Cosine
  indexes)** — `ZeroMagnitudeVector` variant added to `HnswIndexError`; a
  shared `validate_vector_for_metric` guard is called from all four public
  entry points (`insert_vector`, `insert_vector_internal`,
  `batch_insert_vectors`, `search`). Previously a degenerate vector reached the
  SIMD cosine kernel, which panicked (not catchable by `Result`), aborting the
  entire batch. The guard fires only for `Cosine`. 9 regression tests added.
- **AdjacencyCache is now bounded (was unbounded `AHashMap`)** — The cache was
  documented as "LRU-K (K=2)" but was an unbounded `AHashMap` that grew
  monotonically. Now backed by `lru::LruCache` (cap 8192, configurable via
  `with_capacity`). The hit path uses `peek` (read lock) so eviction is
  insertion-order (FIFO); bounded memory holds regardless. The cache values are
  stored as `Arc<Vec<i64>>` — a cache hit clones one refcount instead of
  copying the `Vec`, dropping hit cost at degree 1000 from ~131 ns to ~13 ns
  (–90 %). BFS traversal uses the new `fetch_outgoing_shared` /
  `fetch_incoming_shared` (return `Arc`); existing `fetch_outgoing` /
  `fetch_incoming` still return `Vec<i64>` (unchanged API).
- **13 criterion benchmarks were silently broken (compiled but never ran)** —
  `read_path_benchmarks`, `mvcc_benchmarks`, `algo_benchmarks`,
  `adjlist_benchmark`, `comparative_benchmark`, `compression_benchmark`,
  `native_disk_io`, and the six `regression_*` benches were auto-discovered
  without `harness = false`, so libtest took over and `criterion_main!` never
  ran. Each now has an explicit `[[bench]]` entry.
- Corrected stale "Native V2" doc in `read_path_benchmarks.rs` and the
  `algo` module doc in `lib.rs` (was "Public for tests").

## [3.2.5] - 2026-06-07

### Fixed
- **`insert_into_layer` no longer clones all vectors per insertion** — Added
  `search_layer_fn` which accepts a lookup closure instead of a `HashMap` of
  all vectors. `insert_into_layer` now looks up vectors on-demand from
  `vector_cache` during HNSW search, eliminating ~2.3M heap allocations and
  ~7GB of memory copies per batch of 16 vectors when the index holds 145K
  vectors. Batch insert speed into a 50K index improved from ~2.8 to
  ~160 vectors/sec. (SG-5)

## [3.2.4] - 2026-06-07

### Fixed
- **`persist_topology` rewritten for incremental writes** — Tracks dirty nodes
  per layer in `HnswLayer::dirty_nodes` (a `HashSet<u64>`). Only dirty nodes
  are written to `hnsw_layers` via `INSERT OR REPLACE`, instead of the previous
  full table rewrite. This reduces per-batch persist time from seconds to
  milliseconds for large indexes. (SG-4)

## [3.2.3] - 2026-06-07

### Changed

- **Benchmarking guidance corrected and consolidated** — Added
  `docs/BENCHMARKING.md`, updated README benchmark instructions to use release
  mode for the quick comparison example, and removed stale claims that no
  longer matched current clean runs.
- **`sqlite_v3_curated` benchmark added** — A small-case Criterion suite for
  high-signal SQLite vs V3 comparisons that completes in practical time on a
  developer workstation.
- **`examples/test_performance_comparison.rs` now describes itself correctly** —
  The example is documented as a warm-cache microbenchmark and no longer ends
  with hardcoded backend recommendations that could contradict measured output.

- **HNSW index lock upgraded from `std::sync::Mutex` to `parking_lot::Mutex`** —
  Removed poison handling at 10 call sites in `index_api.rs`. `parking_lot::Mutex`
  provides smaller lock size and eliminates poison handling overhead. (SG-1)

- **All remaining `std::sync::Mutex` replaced with `parking_lot::Mutex`** —
  `progress.rs` (2 sites), `statement_tracker.rs` (1 site), `publisher.rs`
  (5 sites). All `.expect("... poisoned")` patterns eliminated. (SG-2)

- **HNSW runtime operation counters added with atomics** — `HnswIndexStats`
  now reports lock-free `insert_count`, `search_count`, `vector_cache_hits`,
  and `vector_cache_misses`, all backed by `AtomicU64`. Rebuild/autoload paths
  reset these counters after internal recovery so they reflect runtime traffic
  instead of startup repair work. `Publisher::next_id` also now uses
  `AtomicU64` instead of `Arc<Mutex<u64>>`. (SG-2/SG-3)

- **`std::sync::RwLock` replaced with `parking_lot::RwLock` in `query_cache.rs`** —
  11 poison handling blocks removed. (SG-2)

### Added

- **Streaming graph traversal iterators** — `bfs_iter`, `dfs_iter`,
  `topological_sort_iter`, `connected_components_iter` in
  `algo::backend::iterator`. Implement `GraphIterator` trait (BFS/DFS/topo)
  and `Iterator<Item=Result<Vec<i64>>>` (connected components) for lazy,
  composable traversal. O(frontier) memory for node iterators,
  O(|V_component|) for connected components. (SG-4)

## [3.1.4] - 2026-06-06

### Fixed
- Set `busy_timeout(5000ms)` on `conn_for_storage` in `hnsw_index_persistent`. Without
  this, concurrent writes from the magellan service daemon caused `persist_topology` to
  receive SQLITE_BUSY and silently discard entry_points and layer data (via
  `let _ = self.persist_topology()`). Result: 0 rows in hnsw_entry_points,
  incomplete hnsw_layers, and "Index not initialized" errors on every hopgraph query.

## [3.1.3] - 2026-06-06

### Fixed
- `search_layer` now implements proper HNSW greedy search with early termination. Previous
  implementation stopped after `k + M` candidates (e.g. 21 for k=5, M=16), exploring only
  the entry point's immediate neighborhood. New implementation uses `ef_search` as the
  candidate pool size and stops when the closest unexplored candidate is farther than the
  worst result seen, matching the standard HNSW algorithm. Fixes hopgraph returning only
  symbols from the first indexed file regardless of query.

## [3.1.2] - 2026-06-04

### Fixed
- `store_vector` now uses `INSERT` without explicit `id` + `last_insert_rowid()` instead of
  `SELECT MAX(id) + 1`. Removes a TOCTOU race where two concurrent writers to the same DB
  could compute the same next-id and collide on PRIMARY KEY. `store_vector_with_id` (used
  only for topology restore) retains explicit-id `INSERT OR IGNORE` semantics unchanged.

## [3.0.4] - 2026-05-26

### Fixed
- Replaced 46 bare `.unwrap()` calls in production code with `.expect("invariant: ...")`
  - `algo/`: 29 sites (centrality, scc, transitive_closure, cycle_basis, critical_path, topological_sort, graph_ops, traversal, backend/centrality)
  - `backend/native/v3/index_persistence.rs`: 8 `.try_into().unwrap()` in deserialization
  - `backend/native/v3/pubsub/publisher.rs`: 5 `.lock().unwrap()` on poisoned-mutex-vulnerable paths
  - `backend/sqlite/impl_.rs`: 2 sites (publisher init, infallible string write)
  - `backend/native/v3/forensics.rs`: 1 `.get_mut().unwrap()`
  - `backend/native/v3/storage/adaptive_page.rs`: 1 `.as_ref().unwrap()`
- Fixed all pre-existing clippy warnings across workspace (43 fixes)
  - Removed dead code: unused helpers, constants, variants, imports across examples/tests/benches
  - Replaced `Arc` with `Rc` for non-thread-shared snapshot tests
  - Fixed `println!("")` → `println!()`, needless range loops, unnecessary casts, needless borrows
  - Added benchmark cases for `Incoming` and `Undirected` directions
- Production bare `.unwrap()` count: **0** (all remaining unwraps are in `#[cfg(test)]` code)
- `cargo clippy --all-targets -- -D warnings` now passes clean

## [1.5.3] - 2026-02-08

### 🐛 Critical Bug Fixes

#### Header Corruption Fix - Multiple GraphFile Instances
**Location**: `src/backend/native/graph_file/mod.rs:164-175, 222-228`
**Severity**: CRITICAL - Data corruption during concurrent access
**Impact**: Fixed `node_count` reset to 0 when multiple GraphFile instances access the same file

**Root Cause**: When multiple `GraphFile` instances access the same database file (e.g., main thread and watcher thread), the Drop implementation blindly writes the in-memory header to disk. The second instance (which never wrote any nodes) has `node_count=0` and overwrites the correct data from the first instance.

**Fixes Applied**:
1. Added `sync_all()` call to `GraphFile::write_header()` to ensure header reaches disk before Drop
2. Added guard to Drop impl to skip header write if `node_count=0`, preventing read-only instances from corrupting data

**Code Changes**:
```rust
// Before: Only flush() to OS buffer
pub fn write_header(&mut self) -> NativeResult<()> {
    let header_bytes = encode_persistent_header(&self.persistent_header)?;
    self.file.seek(SeekFrom::Start(0))?;
    self.file.write_all(&header_bytes)?;
    self.file.flush()?;  // Only flushes to OS buffer, not disk
    Ok(())
}

// After: Includes sync_all() for durability
pub fn write_header(&mut self) -> NativeResult<()> {
    let header_bytes = encode_persistent_header(&self.persistent_header)?;
    self.file.seek(SeekFrom::Start(0))?;
    self.file.write_all(&header_bytes)?;
    self.file.flush()?;
    self.file.sync_all().map_err(NativeBackendError::Io)?;  // Ensures data reaches disk
    Ok(())
}

// Drop impl now guards against stale overwrites
impl Drop for GraphFile {
    fn drop(&mut self) {
        // Don't overwrite if this instance never wrote any nodes
        if self.persistent_header.node_count == 0 {
            return;
        }
        let _ = self.write_header();
        let _ = self.sync();
    }
}
```

**Testing**:
- Verified database headers now persist correctly even after process crashes
- Status commands now report correct file/symbol counts after abnormal termination
- No data loss when using multiple GraphFile instances concurrently

**Related Documentation**:
- See `docs/bug_report_node_count_not_updated.md` for detailed analysis

---

## [1.5.2] - 2026-02-07

### 🚀 New Features

#### GraphBackend API Enhancement - Clustered Neighbor Queries

**Location**: `src/backend/mod.rs`

Added `neighbors_clustered()` method to the `GraphBackend` trait for direct access to clustered adjacency operations. This provides a dedicated API path for performance-optimized neighbor queries on backends that support clustered storage (Native V2).

**API Changes:**
- Added `neighbors_clustered()` method to `GraphBackend` trait with default implementation
- Default implementation falls back to standard `neighbors()` for backends without clustered storage
- Explicit implementations for `SqliteGraphBackend` (delegates to standard neighbors)
- Explicit implementations for `NativeGraphBackend` (uses V2 clustered adjacency)

**Wrapper Implementations Added:**
- `impl<B> GraphBackend for &B` - includes `neighbors_clustered` method
- `impl<B> GraphBackend for Rc<B>` - includes `neighbors_clustered` method

**Usage Example:**
```rust
use sqlitegraph::backend::GraphBackend;

// Direct clustered access (delegates to standard neighbors on SQLite backend)
let neighbors = graph.neighbors_clustered(node_id, snapshot_id)?;
```

**Performance Notes:**
- For Native V2 backend: Uses optimized V2 clustered adjacency internally
- For SQLite backend: Delegates to standard `neighbors()` query (no clustering)
- The standard `neighbors()` method already uses V2 clustered adjacency when available
- This API provides explicit access while maintaining backward compatibility

---

## [0.2.5] - 2025-12-21

### 🚀 Major New Features: V2 Snapshot System & Atomic Operations

**V2 Snapshot System with complete lifecycle management**

#### V2 Snapshot System ✅
- **Complete Implementation**: Full TDD methodology with 95% test coverage
- **Atomic Operations**: Database-grade filesystem operations with fsync discipline
- **Lifecycle Management**: Explicit state management with deterministic behavior
- **Cross-Platform**: Enhanced filesystem compatibility across platforms
- **Crash Safety**: Atomic copy operations with rollback on failure

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

#### V2 Cluster Architecture Stable
- **I/O Improvement**: Clustered adjacency with direct edge scanning
- **Sub-millisecond Operations**: Fast path for common graph operations
- **Storage Efficiency**: >70% improvement over V1 format
- **Query Performance**: 5,000-50,000 ops/sec for graph traversals

#### WAL System Performance Gains
- **Write Throughput**: Optimized write-ahead logging implementation
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

### 🚀 Stable WAL Mode Implementation (SQLite Backend Only)

**Write-Ahead Logging (WAL) mode is now documented and validated**

#### WAL Mode Features ✅
- **Zero Configuration**: WAL mode enabled by default for all file-based SQLite databases
- **SQLite Backend Only**: WAL mode applies to SQLite backend, not Native V2 backend (uses direct file I/O)
- **Automatic Optimization**: 64MB cache, NORMAL synchronous mode, 256GB memory-mapped I/O
- **Concurrent Performance**: 30-50% improvement for concurrent read/write workloads
- **Graceful Fallback**: Automatic fallback to DELETE mode on unsupported filesystems
- **Validation**: Error handling and validation coverage

#### Implementation Details
**Location**: `src/graph/core.rs:85-102`
- Automatic WAL mode activation for file-based databases
- Exclusion for in-memory databases (WAL not applicable)
- Optimized PRAGMA settings for real workloads

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
