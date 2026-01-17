# Codebase Concerns

**Analysis Date:** 2026-01-17

## Tech Debt

**Large files exceeding 600 LOC limit:**
- Issue: Several files significantly exceed recommended 300-600 LOC limit
- Files affected:
  - `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs` (4,113 lines)
  - `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/rollback.rs` (1,657 lines)
  - `sqlitegraph-cli/src/main.rs` (1,186 lines)
- Why: WAL recovery operations are complex and have grown organically
- Impact: Difficult to navigate, test, and maintain
- Fix approach: Break down into smaller, focused modules with clear responsibilities

**Unused imports and dead code:**
- Issue: Multiple unused imports in WAL modules
- Files: `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs`
  - Unused: `std::fs`, `memmap2::MmapMut`, `std::path::PathBuf`, `std::path::Path`
  - Unused: `serde_json::Value`, `std::time::Instant`
  - Unused macros: `info!`, `error!`
- Why: Incomplete cleanup from refactoring
- Impact: Code clutter, potential confusion
- Fix approach: Run `cargo clippy -- -W unused_imports` and clean up

**Excessive debug scaffolding:**
- Issue: Debug prints scattered throughout codebase without central gating
- Files: Multiple files in `sqlitegraph/src/backend/native/v2/`
- Why: Development debugging left in place
- Impact: Code noise, performance in debug builds
- Fix approach: Gate all debug prints behind single feature flag or environment variable

## Known Bugs

**HNSW CLI persistence limitation:**
- Symptoms: HNSW indexes do not persist across CLI command invocations
- Trigger: Creating index in one command and attempting to use it in another
- Files: `sqlitegraph-cli/src/main.rs`, `sqlitegraph/src/hnsw/`
- Workaround: Use Rust API directly for persistent vector search
- Root cause: Each CLI invocation creates new `SqliteGraph` instance
- Documentation: `docs/hnsw_cli_known_limitations_20241223.md`
- Fix: Implement persistent index storage in Native backend

**Splice tool enum variant limitation:**
- Symptoms: Splice cannot target individual enum variants for patching
- Trigger: Attempting to modify specific enum variants
- Documentation: `docs/splice_limitations_enum_variants.md`
- Workaround: Manual editing for enum modifications
- Fix: Add variant-level targeting to Splice tool

## Security Considerations

**Positive findings:**
- No `unsafe` blocks detected in codebase scan
- No `transmute` usage found
- No obvious memory safety violations

**Areas requiring attention:**
- Risk: Bundled SQLite version should be regularly updated for security patches
- Current mitigation: Uses `rusqlite` with bundled feature
- Recommendations: Pin `rusqlite` version and audit for security updates
- Risk: File I/O operations should validate paths
- Current mitigation: Rust's standard library provides path safety
- Recommendations: Add path validation for user-provided paths

## Performance Bottlenecks

**WAL recovery sequential processing:**
- Problem: Large WAL files processed sequentially
- File: `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs`
- Measurement: Not explicitly measured but large file handling is sequential
- Cause: Single-threaded replay design
- Improvement path: Consider parallel replay for independent operations

**Lock contention potential:**
- Problem: Heavy use of `Arc<Mutex<>>` and `Arc<RwLock<>>`
- Files: Multiple files in `sqlitegraph/src/backend/native/v2/`
- Measurement: No explicit benchmarks for contention
- Cause: Fine-grained locking not implemented
- Improvement path: Profile under concurrent load, consider lock-free structures

**In-memory HNSW index growth:**
- Problem: Index size grows with dataset
- File: `sqlitegraph/src/hnsw/`
- Measurement: O(N) memory usage
- Cause: In-memory index design
- Improvement path: Implement tiered storage or spill to disk

## Fragile Areas

**WAL recovery modules:**
- File: `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/`
- Why fragile: 4,000+ line file with complex logic, many interdependent functions
- Common failures: Recovery edge cases, partial operation handling
- Safe modification: Add tests before changing, verify with corrupted WAL files
- Test coverage: Recovery has specific tests but needs more edge case coverage

**CLI command dispatcher:**
- File: `sqlitegraph-cli/src/main.rs`
- Why fragile: 1,186 lines handling multiple commands with backend-specific logic
- Common failures: Adding new commands may affect existing ones
- Safe modification: Extract commands to separate modules
- Test coverage: Integration tests cover CLI but could be more comprehensive

## Scaling Limits

**Current capacity:**
- Not explicitly documented in code
- Limited by available memory for in-memory indexes
- Limited by filesystem for Native backend

**Limit:**
- HNSW index size bounded by RAM
- WAL file growth bounded by disk space

**Symptoms at limit:**
- Out-of-memory errors with large HNSW indexes
- Slow recovery with large WAL files

**Scaling path:**
- Implement persistent HNSW storage
- Implement WAL checkpointing more aggressively
- Consider sharding for very large graphs

## Dependencies at Risk

**Current dependencies:**
- `rusqlite` 0.31 - Recent version, bundled SQLite
- `bincode` 1.3 - Older but stable
- Other dependencies appear current

**Risk assessment:**
- No deprecated or unmaintained dependencies detected
- All dependencies are mature crates

## Missing Critical Features

**WAL integration incomplete:**
- Problem: Dozens of validator/replayer modules exist but many are placeholder functions
- Files: `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations.rs`
- Documentation: `docs/todo.md` as "Wire the WAL/Checkpoint helpers"
- Blocks: Complete WAL recovery and checkpoint functionality
- Implementation complexity: Medium (connecting existing modules)

**No automatic checkpointing:**
- Problem: WAL files grow without automatic checkpointing
- Current workaround: Manual checkpoint calls
- Blocks: Long-running database stability
- Implementation complexity: Low (add periodic checkpoint trigger)

## Test Coverage Gaps

**WAL recovery edge cases:**
- What's not tested: All possible corruption scenarios
- Risk: Silent data loss in rare edge cases
- Priority: High
- Difficulty to test: Requires creating corrupted WAL files

**Concurrent operations:**
- what's not tested: Multi-threaded read/write patterns
- Risk: Race conditions, deadlocks
- Priority: Medium
- Difficulty to test: Requires concurrent test harness

**HNSW persistence:**
- What's not tested: Index save/restore (not implemented)
- Risk: Data loss on restart if implemented incorrectly
- Priority: High (when feature is added)
- Difficulty to test: Medium

## Documentation Gaps

**Missing module documentation:**
- ~1,093 files lack module-level `///` documentation
- Core modules have docs but many implementation files don't
- Risk: Lost knowledge, difficult onboarding
- Priority: Medium
- Files affected: Mostly in `sqlitegraph/src/backend/native/v2/`

**Complex algorithms underdocumented:**
- HNSW implementation has sparse inline comments
- Pattern matching engine needs more explanation
- Risk: Difficult to maintain and optimize
- Priority: Low (code is readable)

---

*Concerns audit: 2026-01-17*
*Update as issues are fixed or new ones discovered*
