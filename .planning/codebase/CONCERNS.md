# Codebase Concerns

**Analysis Date:** 2026-01-20

## Tech Debt

### Incomplete HNSW Multi-Layer Implementation

**HNSW index is single-layer only**, missing the core HNSW feature.

- **Issue:** `determine_insertion_level()` always returns 0, disabling multi-layer graph structure
- **Files:** `sqlitegraph/src/hnsw/index.rs:919-923`
- **Impact:** HNSW performance degrades to O(n) linear search instead of O(log n) with proper layers; defeats the purpose of using HNSW
- **Fix approach:** Implement proper exponential distribution for layer selection: `-ln(uniform(0,1)) * ml` where `ml` is `1/ln(m)`

### Checkpoint Strategies Not Implemented

**Three checkpoint strategies return hardcoded `false`**, making them non-functional.

- **Issue:** `TransactionCount`, `SizeThreshold`, and `Adaptive` checkpoint strategies always return `false`
- **Files:** `sqlitegraph/src/backend/native/v2/wal/checkpoint/core.rs:676-683`
- **Impact:** Checkpointing only works with time-based strategy; other strategies do nothing
- **Fix approach:** Implement transaction counting, WAL size tracking, and adaptive decision logic

### Node Deletion Incomplete

**Node deletion doesn't clean up edges or reclaim space.**

- **Issue:** `delete_node()` only removes from index, leaving orphaned edges and wasted space
- **Files:** `sqlitegraph/src/backend/native/node_store.rs:393-399`, `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations_with_problematic_tests.rs:455-458`
- **Impact:** Database grows unbounded with orphaned edges; space never reclaimed
- **Fix approach:** Implement edge cascade deletion and free space management integration

### WAL Recovery Scanning Stub

**WAL recovery scanner returns empty results.**

- **Issue:** `scan_wal_for_transactions()` creates placeholder result instead of actual scanning
- **Files:** `sqlitegraph/src/backend/native/v2/wal/recovery/core.rs:445-465`
- **Impact:** WAL recovery after crash doesn't restore transactions; data loss possible
- **Fix approach:** Implement async WAL scanning with transaction reassembly

### Checkpoint Validation Disabled

**Checkpoint state validation is commented out entirely.**

- **Issue:** Validation code exists but is completely disabled due to struct mismatch
- **Files:** `sqlitegraph/src/backend/native/v2/wal/checkpoint/validation/invariants.rs:236-259`
- **Impact:** No integrity checks during checkpoint; silent corruption possible
- **Fix approach:** Update validation to match actual `CheckpointState` enum structure

### Schema Version Field Size Inconsistent

**Schema version read as 8 bytes but should be 4 bytes.**

- **Issue:** `get_slice_safe(bytes, offset, 8)` used for schema_version
- **Files:** `sqlitegraph/src/backend/native/graph_file/encoding.rs:134`
- **Impact:** Potential misalignment in binary format; compatibility issues
- **Fix approach:** Change to 4 bytes and update all serialization/deserialization

## Known Bugs

### Neo4j Backend Unimplemented

**Neo4j backend selection causes panic.**

- **Symptoms:** `panic!("Neo4j backend not implemented")`
- **Files:** `sqlitegraph/src/backend_selector.rs:36`
- **Trigger:** Selecting `BackendKind::Neo4j`
- **Workaround:** Don't use Neo4j backend; only SQLite and Native V2 are supported

### Default Checkpoint Strategy Panics

**Adaptive checkpoint strategy panics when accessed.**

- **Symptoms:** `panic!("Default strategy should be Adaptive")`
- **Files:** `sqlitegraph/src/backend/native/v2/wal/checkpoint/strategies.rs:533`
- **Trigger:** Using default checkpoint strategy without explicit configuration
- **Workaround:** Always explicitly configure checkpoint strategy

## Security Considerations

### Unsafe Lifetime Extension Pattern

**Transmute used to extend `GraphFile` lifetime to `'static` throughout WAL code.**

- **Risk:** Undefined behavior if `GraphFile` is moved or dropped while references exist
- **Files:**
  - `sqlitegraph/src/backend/native/v2/wal/checkpoint/operations.rs:447-459`
  - `sqlitegraph/src/backend/native/v2/wal/checkpoint/record/integrator.rs:38-50`
  - `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/rollback.rs:143,180,225,525,630,717,891`
  - `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations/edge_ops.rs:169,290,598`
  - `sqlitegraph/src/backend/native/v2/wal/recovery/validator.rs:142-154`
- **Current mitigation:** Comments claim this is a "production pattern" when GraphFile is owned by integrator
- **Recommendations:** Refactor to use proper ownership with `Arc<GraphFile>` or arena allocation

### Unaligned Pointer Reads

**Direct unaligned reads from byte slices in WAL code.**

- **Risk:** Undefined behavior on architectures that don't support unaligned access (some ARM, older CPUs)
- **Files:** `sqlitegraph/src/backend/native/v2/wal/reader.rs:207`, `sqlitegraph/src/backend/native/v2/wal/writer.rs:167`
- **Current mitigation:** x86_64 dominates target platforms; unaligned access is generally safe
- **Recommendations:** Use `bytemuck::read_unaligned` consistently or verify alignment

### Memory-Mapped File Without Synchronization

**MMAP operations use interior mutability without explicit memory barriers.**

- **Risk:** Concurrent writes may not be visible across threads due to CPU cache coherency issues
- **Files:** `sqlitegraph/src/backend/native/graph_file/memory_mapping.rs:30,33,99,248`
- **Current mitigation:** Single-threaded access pattern documented
- **Recommendations:** Add explicit memory ordering or use atomic operations for shared mutable state

## Performance Bottlenecks

### Excessive Cloning in HNSW Storage

**Vector data cloned on every read operation.**

- **Problem:** `get_vector()` clones entire vector for each access; `get_vector_with_metadata()` double-clones
- **Files:** `sqlitegraph/src/hnsw/storage.rs:768-775`
- **Cause:** Returning owned `Vec<f32>` instead of references or `Cow`
- **Improvement path:** Return `&[f32]` slices or use `Arc<Vec<f32>>` for shared storage

### Inefficient Candidate Sorting in HNSW Search

**Repeated sorting of candidate list during graph search.**

- **Problem:** `candidates.sort_by()` called on each iteration of search loop
- **Files:** `sqlitegraph/src/hnsw/neighborhood.rs:309-314`
- **Cause:** Linear search through candidates instead of using priority queue
- **Improvement path:** Replace `Vec` with `BinaryHeap` for O(log n) extraction instead of O(n log n) sorting

### Path Cloning in Cycle Detection

**Entire paths cloned on each cycle discovery.**

- **Problem:** `let mut cycle = path.clone()` and `let mut new_path = path.clone()` in tight loops
- **Files:** `sqlitegraph/src/algo.rs:248,260`
- **Cause:** Ownership semantics force copying
- **Improvement path:** Use `VecDeque` or indices to avoid full path duplication

## Fragile Areas

### WAL Recovery Rollback Operations

**Highly complex rollback code with many placeholders.**

- **Files:** `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/rollback.rs` (1654 lines)
- **Why fragile:** Mixes unsafe transmute, complex state management, and stubbed operations
- **Safe modification:** Write comprehensive tests before touching rollback logic; use property-based testing
- **Test coverage:** Many test cases are disabled with `#[ignore]` pending real implementation

### HNSW Index Integration Tests

**12 tests marked as failing until implementation complete.**

- **Files:** `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations_with_problematic_tests.rs:838,876,904,942,976,998,1022,1064`
- **Why fragile:** Tests document expected behavior that isn't implemented yet
- **Safe modification:** Enable tests one at a time and implement corresponding functionality
- **Test coverage:** Tests exist but are ignored; real coverage unknown

### Edge Cluster Serialization

**Multiple binrw serialization stubs.**

- **Files:** `tests/v2_edge_cluster_serialization_binrw_tests.rs:39,42`, `tests/v2_node_serialization_binrw_tests.rs:38,41`
- **Why fragile:** Hand-rolled serialization instead of using binrw as planned
- **Safe modification:** Replace hand-rolled serialization with binrw derive macros; verify byte-for-byte compatibility
- **Test coverage:** Serialization tests exist but use custom implementations

## Scaling Limits

### Single-Threaded Writes

**SqliteGraph is NOT thread-safe for concurrent writes.**

- **Current capacity:** Single writer thread; all writes must be serialized
- **Limit:** Write throughput bounded by single CPU core; ~10-100μs per operation
- **Scaling path:** Document requires external write coordination; no built-in concurrency

### RefCell Borrow Checking Overhead

**Interior mutability causes runtime borrow checks.**

- **Current capacity:** ~10,000-100,000 operations/second before borrow check overhead significant
- **Limit:** Each operation may panic on borrow violation; no compile-time safety
- **Where:** `sqlitegraph/src/lib.rs:96` documents RefCell usage throughout graph core
- **Scaling path:** Refactor to use `Arc<RwLock<T>>` or lock-free structures for concurrent access

### Snapshot Isolation

**Each snapshot clones entire graph state.**

- **Current capacity:** Limited by available memory; each snapshot duplicates node/edge indexes
- **Limit:** ~10-100MB per snapshot for large graphs
- **Where:** `sqlitegraph/src/graph/snapshot.rs` implements snapshot via ArcSwap but still has overhead
- **Scaling path:** Implement copy-on-write at storage layer rather than snapshot layer

## Dependencies at Risk

### Rusqlite 0.31 with Bundled SQLite

**Using `bundled` feature compiles SQLite from source.**

- **Risk:** Bundled version may lag behind SQLite releases; security patches delayed
- **Impact:** Database backend; core functionality depends on rusqlite
- **Migration plan:** Switch to system SQLite via `pkg-config` or track rusqlite updates closely

### Rand 0.8

**Older version of rand; newer features unavailable.**

- **Risk:** Missing thread-local RNG, SIMD optimizations
- **Impact:** HNSW layer selection uses random; algorithms may be slower than necessary
- **Migration plan:** Upgrade to rand 0.9 when ecosystem catches up

### Rayon 1.10

**Thread pool not optimally configured.**

- **Risk:** Default rayon thread pool may oversubscribe CPU
- **Impact:** Parallel WAL recovery may have diminishing returns beyond 4-8 threads
- **Migration plan:** Expose thread pool configuration in `GraphConfig`

## Missing Critical Features

### Async I/O Support

**All I/O operations are synchronous.**

- **Problem:** No async/await support; blocking I/O on every database operation
- **Blocks:** Async application integration (tokio, async-std)
- **Impact:** Cannot integrate with async runtimes without thread pool

### Graphviz Export

**No visualization export capability.**

- **Problem:** No DOT/Graphviz export for debugging or documentation
- **Blocks:** Graph visualization tools integration
- **Impact:** Debugging complex traversals requires manual inspection

### Backup/Restore API Incomplete

**Only basic dump/load utilities exist.**

- **Problem:** `dump_graph_to_path` and `load_graph_from_path` are basic
- **Blocks:** Incremental backups, point-in-time recovery
- **Impact:** Production deployments need external backup solutions

## Test Coverage Gaps

### V1 Prevention Tests Disabled

**8 compilation tests for V1 prevention are always ignored.**

- **What's not tested:** Runtime enforcement of V1 feature blocking
- **Files:** `tests/v1_prevention_compilation_tests.rs:18,26,34,43,68,77`
- **Risk:** V1 code could accidentally be reintroduced without detection
- **Priority:** Medium - V1 is permanently removed, but guard could fail

### Checkpoint/Recovery Integration Tests Disabled

**7 WAL checkpoint tests are ignored.**

- **What's not tested:** End-to-end checkpoint and recovery workflows
- **Files:** `tests/wal_checkpoint_recovery_tests.rs:108,119,126,133,140`
- **Risk:** Crash recovery may not work correctly
- **Priority:** High - Data loss possible if recovery fails

### Stress Tests Disabled

**4 stress/integrity tests require manual enabling.**

- **What's not tested:** Concurrent crash simulation, integrity under load
- **Files:** `tests/v2_crash_simulation.rs:328,362`, `tests/v2_stress_integrity.rs:69,107,426`
- **Risk:** Race conditions and corruption under concurrent access
- **Priority:** High - Production workloads may trigger undiscovered bugs

### Tokio Async Tests Disabled

**3 async tests are ignored due to missing runtime.**

- **What's not tested:** Async transaction coordinator functionality
- **Files:** `sqlitegraph/src/backend/native/v2/wal/transaction_coordinator.rs:968,976`, `sqlitegraph/src/backend/native/v2/wal/v2_integration.rs:1005`
- **Risk:** Deadlock detection and async coordination are untested
- **Priority:** Medium - Async support not production-ready

### MMAP I/O Tests Are Stubs

**10 MMAP tests are placeholders awaiting implementation.**

- **What's not tested:** Memory-mapped I/O correctness and performance
- **Files:** `tests/v2_mmap_io_invariants_tests.rs:28,48,102,120,167,196,241`
- **Risk:** MMAP path may have data corruption issues
- **Priority:** Medium - MMAP is optional feature, but should work if enabled

### Snapshot Edge Validation Unimplemented

**Edge validation in snapshot tests is hardcoded `true`.**

- **What's not tested:** Actual edge consistency during snapshot/restore
- **Files:** `tests/snapshot_integration_tests.rs:452,476,479`
- **Risk:** Corrupted snapshots may not be detected
- **Priority:** Medium - Snapshot corruption could cause silent data loss

---

*Concerns audit: 2026-01-20*
