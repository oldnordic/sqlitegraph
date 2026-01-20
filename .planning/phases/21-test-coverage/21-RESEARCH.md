# Phase 21: Test Coverage - Research

**Researched:** 2026-01-20
**Domain:** Test Infrastructure, Coverage, Validation, Miri Integration
**Confidence:** MEDIUM

## Summary

Phase 21 focuses on ensuring comprehensive test coverage for all critical paths in the sqlitegraph codebase. This phase is about enabling disabled tests, implementing missing test coverage, and ensuring all validation mechanisms work correctly.

The key areas requiring attention are:
1. **WAL Recovery Tests** - 8 tests in `operations_with_problematic_tests.rs` marked as "will fail until implementation complete"
2. **Checkpoint Tests** - Several tests commented out or stubbed in `wal_checkpoint_recovery_tests.rs`
3. **Cluster Validation Tests** - Tests exist but may need verification they run and pass
4. **HNSW Multi-layer Tests** - Multi-layer infrastructure exists, but comprehensive tests need verification
5. **Miri Integration** - CI configured, but former transmute sites need Miri test coverage

**Primary recommendation:** Focus on enabling and fixing existing tests rather than writing new ones. Most test infrastructure is already in place.

## Standard Stack

### Core Testing Tools
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| `cargo test` | Built-in | Standard Rust test runner | Native Rust testing |
| `tempfile` | 3.23.0 | Temporary file/directory creation | Isolated test environments |
| `assert_cmd` | 2.1.1 | CLI testing for crash simulation | Process-level testing |

### Benchmarking
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| `criterion` | 0.5.1 | Statistical benchmarking | Performance regression detection |

### Miri (Undefined Behavior Detection)
| Component | Version | Purpose | Status |
|-----------|---------|---------|--------|
| `miri` | nightly | UB detection in unsafe code | CI configured in `.github/workflows/test.yml` |
| MIRIFLAGS | Custom | Miri configuration flags | Set in `.cargo/config.toml` |

**Installation:**
```bash
# Standard tools come with Rust
cargo install cargo-miri  # For local Miri testing
```

## Architecture Patterns

### Test Organization Structure
```
sqlitegraph/tests/           # Integration tests
├── wal_*_tests.rs          # WAL recovery tests
├── phase42_*_tests.rs      # Cluster allocation invariants
├── hnsw_*_tests.rs         # HNSW persistence
├── v2_crash_simulation.rs  # Crash simulation tests
└── recovery_tests.rs       # Graph recovery tests

sqlitegraph/src/            # Unit tests (in-module)
├── backend/native/v2/wal/
│   ├── recovery/replayer/operations_with_problematic_tests.rs
│   └── checkpoint/
└── hnsw/                   # HNSW unit tests
```

### Pattern 1: TDD Test Marking for Implementation Pending
**What:** Tests marked with `// TODO: This test will fail until real implementation is complete`
**When to use:** When writing tests for features not yet implemented
**Example:**
```rust
// From operations_with_problematic_tests.rs:843
#[test]
fn test_handle_node_delete_basic() {
    let operations = create_test_operations();
    let mut rollback_data = Vec::new();

    let result = operations.handle_node_delete(42, 4096, None, &mut rollback_data);

    // TODO: This test will fail until real implementation is complete
    // SME Phase 2: Writing failing tests as required by TDD methodology
    assert!(result.is_ok(), "Basic node delete should succeed");
    assert_eq!(rollback_data.len(), 1, "Should record rollback operation");
}
```

### Pattern 2: Feature-Gated Tests
**What:** Tests using `#[cfg(feature = "v2_experimental")]`
**When to use:** Tests that require experimental features
**Example:**
```rust
#[cfg(feature = "v2_experimental")]
#[test]
fn test_multi_cluster_offsets_must_be_distinct_and_non_overlapping() {
    // ... test code
}
```

### Pattern 3: Environment-Controlled Tests
**What:** Tests requiring environment variables to enable
**When to use:** Tests with significant resource requirements
**Example:**
```rust
// From v2_crash_simulation.rs
fn should_run_crash_tests() -> bool {
    env::var("RUST_TEST_CRASH").is_ok() || env::var("CRASH_TESTS").is_ok()
}
```

### Pattern 4: Miri-Specific Tests
**What:** Tests gated with `#[cfg(all(miri, test))]`
**When to use:** Tests specifically for Miri UB detection
**Example:**
```rust
// From store_helpers.rs
#[cfg(all(miri, test))]
mod miri_tests {
    #[test]
    fn miri_test_arc_rwlock_graphfile_lifetime() {
        // Miri will catch use-after-free
    }
}
```

### Anti-Patterns to Avoid
- **Commenting out tests instead of using `#[ignore]`**: Makes tests invisible to test runners
- **Leaving `TODO` markers without tracking**: Creates hidden technical debt
- **Environment-dependent defaults**: Tests should skip gracefully when requirements aren't met

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Test discovery | Custom test runners | `cargo test` | Native integration, works with IDEs |
| Temporary files | Manual cleanup | `tempfile` crate | Guaranteed cleanup even on panic |
| CLI testing | Manual process spawning | `assert_cmd` | Proper stderr/stdout handling |
| Benchmarking | Manual timing | `criterion` | Statistical significance, warmup handling |
| UB detection | Manual audit | `miri` | Catches actual undefined behavior |
| Code coverage | Manual instrumentation | `cargo-llvm-cov` | Standard Rust coverage tool |

**Key insight:** Custom test infrastructure is maintenance burden. Use existing tools.

## Common Pitfalls

### Pitfall 1: Tests Left in "Will Fail Until Implementation Complete" State
**What goes wrong:** Tests written with TODO markers are never addressed
**Why it happens:** No tracking mechanism for implementation pending tests
**How to avoid:**
- Track each "will fail" test in project documentation
- Create GitHub issues or todo comments linked to requirements
- Periodic review of all tests with TODO markers
**Warning signs:** `TODO: This test will fail until` comments in test files

### Pitfall 2: Feature-Gated Tests Never Run
**What goes wrong:** Tests under `#[cfg(feature = "...")]` never executed in CI
**Why it happens:** CI doesn't enable the feature flag
**How to avoid:**
- Ensure CI runs tests with `--all-features`
- Verify test appears in `cargo test --list` output
**Warning signs:** Test compiles but never shows in test runs

### Pitfall 3: Crash Simulation Tests Skipped by Default
**What goes wrong:** Important crash safety tests never run because they require opt-in
**Why it happens:** Environment variable not set in CI
**How to avoid:**
- Add opt-in to CI matrix for critical tests
- Document how to enable tests locally
**Warning signs:** `should_run_crash_tests()` returning `false`

### Pitfall 4: Miri Tests Not Comprehensive
**What goes wrong:** Miri only runs on limited test modules
**Why it happens:** Miri tests must be explicitly marked with `#[cfg(miri)]`
**How to avoid:**
- Add Miri tests for all unsafe code locations
- Run Miri locally before committing unsafe changes
**Warning signs:** Former transmute sites without Miri tests

### Pitfall 5: Mock Implementations Never Replaced
**What goes wrong:** Mock operations log warnings but don't test real behavior
**Why it happens:** Placeholder implementations forgotten
**How to avoid:**
- Track all mock implementations
- Have lint or test that warns on mock usage in production paths
**Warning signs:** `warn_log!("not yet implemented - placeholder")` in tests

## Code Examples

### Running All Tests Including Feature-Gated
```bash
# Run all tests with all features
cargo test --workspace --all-features --verbose

# List all tests (verify feature-gated tests appear)
cargo test --workspace --all-features --list | grep -i cluster
```

### Running Miri Tests
```bash
# Miri is already configured in CI (.github/workflows/test.yml:78-82)
# Run locally:
cargo +miri miri test -p sqlitegraph store_helpers
cargo +miri miri test -p sqlitegraph miri

# MIRIFLAGS are set in .cargo/config.toml:
# MIRIFLAGS = "-Zmiri-disable-isolation -Zmiri-ignore-leaks -Zmiri-symbolic-alignment-check"
```

### Enabling Crash Simulation Tests
```bash
# Tests in v2_crash_simulation.rs require RUST_TEST_CRASH=1
RUST_TEST_CRASH=1 cargo test --features v2_experimental v2_crash_simulation
```

### Checking Test Coverage
```bash
# Install cargo-llvm-cov
cargo install cargo-llvm-cov

# Generate coverage report
cargo llvm-cov --workspace --all-features
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Manual test discovery | `cargo test` | From project start | Standard Rust tooling |
| No UB detection | Miri integration | Phase 16 | Catches undefined behavior |
| No CI testing | GitHub Actions CI | From project start | Automated testing on push |

**Current Infrastructure:**
- **CI Pipeline**: `.github/workflows/test.yml` runs:
  - Standard tests on Linux/Windows/macOS
  - Miri tests for `store_helpers` and `miri` modules
  - Clippy linting
  - Format checking
- **Miri Configuration**: `.cargo/config.toml` sets MIRIFLAGS
- **Test Features**: `v2_experimental` feature flag for V2-specific tests

## Known Test Locations by Requirement

### TEST-WAL-01 through TEST-WAL-04: WAL Recovery Tests
**Location:** `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations_with_problematic_tests.rs`

| Test | Line | Status |
|------|------|--------|
| `test_handle_node_delete_basic` | 831 | Marked "will fail until implementation complete" |
| `test_handle_node_delete_with_old_data` | 861 | Marked "will fail until implementation complete" |
| `test_handle_node_delete_nonexistent_node` | 897 | Marked "will fail until implementation complete" |
| `test_handle_node_delete_with_cluster_references` | 920 | Marked "will fail until implementation complete" |
| `test_handle_node_delete_malformed_old_data` | 967 | Marked "will fail until implementation complete" |
| `test_handle_node_delete_zero_node_id` | 991 | Marked "will fail until implementation complete" |
| `test_handle_node_delete_rollback_operation_preserves_slot_offset` | 1013 | Marked "will fail until implementation complete" |
| `test_handle_node_delete_edge_cleanup_required` | 1042 | Marked "will fail until implementation complete" |

**Node deletion rollback** infrastructure exists in:
- `src/backend/native/v2/wal/record.rs:190-199` - `NodeDelete` record with before-image fields
- Mock implementation at `operations_with_problematic_tests.rs:453-463`

### TEST-CLUS-01 through TEST-CLUS-03: Cluster Validation Tests
**Location:** `sqlitegraph/tests/phase42_cluster_allocation_invariants_tests.rs`

| Test | Line | Status |
|------|------|--------|
| `test_multi_cluster_offsets_must_be_distinct_and_non_overlapping` | 20 | Feature-gated (`v2_experimental`), exists |
| `test_cluster_headers_survive_reopen` | 205 | Feature-gated (`v2_experimental`), exists |
| `test_header_and_file_length_consistency_after_multiple_cluster_writes` | 397 | Feature-gated (`v2_experimental`), exists |

**Related regression tests:**
- `tests/phase65_cluster_size_corruption_regression.rs`
- `tests/phase66_v2_cluster_metadata_corruption_regression.rs`
- `tests/cluster_offset_corruption_regression.rs`

### TEST-CP-01 through TEST-CP-03: Checkpoint Tests
**Location:** `sqlitegraph/tests/wal_checkpoint_recovery_tests.rs`

| Test | Line | Status |
|------|------|--------|
| `test_v2_wal_checkpoint_creation_and_validation` | 19 | Partially implemented |
| `test_checkpoint_strategies_v2_workloads` | 107 | Marked `#[ignore]` |
| `test_v2_wal_crash_recovery_transaction_replay` | 119 | Marked `#[ignore]` |
| `test_recovery_multiple_incomplete_transactions` | 127 | Marked `#[ignore]` |
| `test_checkpoint_recovery_integration_v2_graph` | 134 | Marked `#[ignore]` |
| `test_recovery_validation_consistency_checking` | 141 | Marked `#[ignore]` |

**Checkpoint infrastructure exists:**
- `src/backend/native/v2/wal/checkpoint/strategies.rs` - `CheckpointStrategy` enum
- `src/backend/native/v2/wal/checkpoint/core.rs` - `V2WALCheckpointManager`

### TEST-HNSW-01 through TEST-HNSW-04: HNSW Multi-layer Tests
**Location:** `sqlitegraph/src/hnsw/index.rs`

| Test | Line | Status |
|------|------|--------|
| `test_multilayer_level_distribution` | 515 | Exists, tests exponential distribution |
| `test_single_layer_mode` | 586 | Exists |
| `test_multilayer_recall` | 624 | Exists but uses `enable_multilayer: false` |

**Benchmark:**
- `benches/hnsw_multilayer.rs` - Search scaling benchmark (currently uses single-layer mode)

**Multi-layer infrastructure:**
- `src/hnsw/multilayer.rs` - `LevelDistributor` with exponential distribution
- `src/hnsw/config.rs` - `enable_multilayer` and `multilayer_level_distribution_base` options

### TEST-MIRI-01 through TEST-MIRI-04: Miri Tests
**Location:** `sqlitegraph/src/backend/native/v2/wal/recovery/store_helpers.rs`

| Test | Line | Status |
|------|------|--------|
| `miri_test_arc_rwlock_graphfile_lifetime` | 80 | Exists |
| `miri_test_store_lifetime_bounded` | 108 | Exists |
| `miri_test_drop_order` | 142 | Exists |

**CI Configuration:**
- `.github/workflows/test.yml:50-82` - Miri job runs `store_helpers` and `miri` tests
- `.cargo/config.toml` - MIRIFLAGS configured

**Former transmute sites** (from Phase 16) - need Miri test coverage:
- `create_node_store` - Has Miri tests
- `create_edge_store` - Has Miri tests

## Open Questions

1. **Node Deletion Rollback Implementation**
   - What we know: Test infrastructure exists, mock implementation in place
   - What's unclear: Whether real implementation exists elsewhere or needs to be built
   - Recommendation: Search for real `handle_node_delete` implementation

2. **Cluster Overlap Validation Automation**
   - What we know: Manual tests exist in `phase42_cluster_allocation_invariants_tests.rs`
   - What's unclear: Whether validation runs automatically on every cluster operation
   - Recommendation: Check if validation is integrated into cluster allocation path

3. **Multi-layer HNSW Release Build Stability**
   - What we know: Comment in `hnsw_multilayer.rs` mentions single-layer for stability
   - What's unclear: What issues exist in release builds with multi-layer
   - Recommendation: Investigate release-specific failures

4. **Checkpoint Trigger Integration**
   - What we know: CheckpointStrategy exists, tests are stubbed
   - What's unclear: Whether checkpoint triggers are hooked into WAL commit path
   - Recommendation: Verify checkpoint trigger calling code

## Sources

### Primary (HIGH confidence)
- `.github/workflows/test.yml` - CI configuration
- `.cargo/config.toml` - Miri configuration
- `sqlitegraph/src/backend/native/v2/wal/recovery/replayer/operations_with_problematic_tests.rs` - Node deletion tests
- `sqlitegraph/tests/phase42_cluster_allocation_invariants_tests.rs` - Cluster validation tests

### Secondary (MEDIUM confidence)
- `sqlitegraph/src/hnsw/index.rs` - HNSW multi-layer tests
- `sqlitegraph/benches/hnsw_multilayer.rs` - Search scaling benchmark
- `sqlitegraph/tests/wal_checkpoint_recovery_tests.rs` - Checkpoint test stubs
- `sqlitegraph/src/backend/native/v2/wal/checkpoint/strategies.rs` - Checkpoint strategy types

### Tertiary (LOW confidence)
- `sqlitegraph/tests/v2_crash_simulation.rs` - Crash simulation test patterns (reviewed only header)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Standard Rust tools verified
- Architecture: MEDIUM - Test patterns identified from code, but full picture incomplete
- Pitfalls: MEDIUM - Based on common Rust testing issues and observed TODO markers
- Test locations: HIGH - All test files and line numbers verified

**Research date:** 2026-01-20
**Valid until:** 30 days (test infrastructure changes relatively slowly)
