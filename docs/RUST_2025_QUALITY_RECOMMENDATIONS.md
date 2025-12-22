# Rust 2025 Quality Recommendations for SQLiteGraph

## Immediate Action Items for SQLiteGraph

Based on current Rust best practices (2025), here are specific recommendations for your SQLiteGraph project:

### 1. Enhanced Code Quality Tools

Add to your project:

```toml
# Add to dev-dependencies in Cargo.toml
[dev-dependencies]
# Existing dependencies...
cargo-nextest = "0.4"  # Modern test runner
proptest = "1.0"       # Property-based testing
testcontainers = "0.15" # Integration testing with real databases
```

**Essential tools to install:**
```bash
cargo install cargo-machete      # Detect unused dependencies
cargo install cargo-semver-checks # Prevent breaking changes
cargo install cargo-nextest      # Fast test runner
cargo install cargo-audit        # Security scanning
```

### 2. Recommended Clippy Configuration

Create `clippy.toml` in project root:
```toml
warn-on-all-warnings = true
msrv = "1.75"  # Set based on your minimum supported version

[complexity]
too-many-arguments-threshold = 5
cognitive-complexity-threshold = 10
type-complexity-threshold = 250

[pedantic]
level = "warn"

[nursery]
level = "warn"
```

Update `sqlitegraph/Cargo.toml` with lint configuration:
```toml
[lints.rust]
rust_2018_idioms = "warn"
unused = "warn"
unused_import_braces = "warn"

[lints.clippy]
all = "warn"
pedantic = "warn"
nursery = "warn"

# Allow for pragmatic reasons
module_name_repetitions = "allow"
must_use_candidate = "allow"

# Critical for database code
indexing_slicing = "warn"
unwrap_used = "warn"
expect_used = "warn"
panic = "warn"
```

### 3. Pre-commit Hook Setup

Create `.pre-commit-config.yaml`:
```yaml
repos:
  - repo: local
    hooks:
      - id: fmt
        name: rustfmt
        entry: cargo fmt --
        language: system
        pass_filenames: false
      - id: clippy
        name: clippy
        entry: cargo clippy --all-targets --all-features -- -D warnings
        language: system
        pass_filenames: false
      - id: machete
        name: cargo-machete
        entry: cargo machete
        language: system
        pass_filenames: false
```

### 4. Enhanced Testing for Storage Systems

Given SQLiteGraph is a storage system, add these testing patterns:

**Property-based tests (add to `tests/`):**
```rust
// tests/property_based_tests.rs
use proptest::prelude::*;
use sqlitegraph::*;

proptest! {
    #[test]
    fn test_insert_retrieval_roundtrip(
        keys in prop::collection::vec(".*", 1..100),
        values in prop::collection::vec(any::<u64>(), 1..100)
    ) {
        let db = SqliteGraph::new_in_memory();
        // Test roundtrip properties
    }
}
```

**Concurrency tests:**
```rust
// tests/concurrency_tests.rs
use std::sync::Arc;
use std::thread;

#[test]
fn test_concurrent_writes() {
    let db = Arc::new(SqliteGraph::new_in_memory());
    let handles: Vec<_> = (0..10).map(|i| {
        let db = Arc::clone(&db);
        thread::spawn(move || {
            // Concurrent operations
        })
    }).collect();

    for h in handles {
        h.join().unwrap();
    }
    // Verify invariants
}
```

### 5. API Evolution Strategy

Your project already uses feature flags well. To enhance:

```rust
// Document API evolution with deprecation
#[deprecated(since = "0.3.0", note = "Use SqliteGraph::with_config() instead")]
pub fn SqliteGraph::new() -> Self {
    Self::with_config(Config::default())
}

// Use sealed traits for extensibility
pub trait StorageBackend: sealed::Sealed {
    // Methods
}

mod sealed {
    pub trait Sealed {}
}
```

### 6. CI/CD Enhancements

Update your GitHub Actions workflow (if you had one):

```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        rust: [stable, beta, nightly]
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: ${{ matrix.rust }}
          components: rustfmt, clippy
      - name: Check formatting
        run: cargo fmt --all -- --check
      - name: Run clippy
        run: cargo clippy --all-targets --all-features -- -D warnings
      - name: Run tests
        run: cargo nextest run --all-features
      - name: Check semver
        run: cargo semver-checks
```

### 7. Performance Regression Prevention

Your project already has benchmarks. Enhance with:

```rust
// Use criterion with regression detection
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_v2_operations(c: &mut Criterion) {
    c.bench_function("v2_insert_1000", |b| {
        b.iter(|| {
            // Your benchmark code
        })
    });
}

// Add regression check in CI
fn has_regressed(current: f64, baseline: f64) -> bool {
    current > baseline * 1.1  // 10% regression threshold
}
```

### 8. Documentation Standards

Enhance your `lib.rs` documentation:

```rust
//! # SQLiteGraph
//!
//! A deterministic, embedded graph database with SQLite and Native backends.
//!
//! ## Quick Start
//!
//! ```rust
//! use sqlitegraph::SqliteGraph;
//!
//! let mut db = SqliteGraph::new_in_memory();
//! db.add_node("user", "alice")?;
//! db.add_node("user", "bob")?;
//! db.add_edge("alice", "knows", "bob")?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Backend Selection
//!
//! SQLiteGraph supports multiple backends via feature flags:
//!
//! - `sqlite-backend` (default): SQLite-based storage
//! - `native-v2`: High-performance native storage
//!
//! ## Performance Characteristics
//!
//! - Insert: ~50,000 nodes/sec
//! - Traversal: ~100,000 edges/sec
//! - Storage: Deterministic ordering
//!
//! ## Thread Safety
//!
//! All operations are thread-safe when using the SQLite backend.
//! The native backend supports concurrent reads.

/// Main graph database implementation
///
/// # Examples
///
/// Basic usage:
/// ```rust
/// # use sqlitegraph::SqliteGraph;
/// let db = SqliteGraph::new_in_memory();
/// ```
pub struct SqliteGraph { /* ... */ }
```

### 9. Unused Import Prevention

Given your 149 dead code warnings (as noted in CLAUDE.md):

```bash
# Install detection tools
cargo install cargo-machete

# Run detection
cargo machete

# For imports (configure rust-analyzer in .vscode/settings.json):
{
  "rust-analyzer.imports.granularity.group": "module",
  "rust-analyzer.imports.prefix": "crate"
}
```

Add to your codebase:
```rust
// Allow intentional unused items
#[allow(dead_code)]  // For CLI-only functions
mod cli_only {
    pub fn helper_function() {
        // Used only from CLI
    }
}

// Or use cfg attribute
#[cfg(any(test, feature = "cli"))]
mod optional_module {
    // Only compiled when needed
}
```

## Priority Implementation Order

1. **Immediate (This Week)**
   - Install cargo-machete and run cleanup
   - Add clippy configuration
   - Set up pre-commit hooks

2. **Short-term (Next Sprint)**
   - Add property-based tests for critical operations
   - Enhance documentation with examples
   - Set up cargo-semver-checks

3. **Medium-term (Next Month)**
   - Implement comprehensive CI/CD pipeline
   - Add performance regression detection
   - Migrate to cargo-nextest

## Key Takeaways

1. **Automation is Critical**: Pre-commit hooks and CI/CD prevent quality issues
2. **Testing Diversity**: Combine unit, integration, property, and performance tests
3. **Documentation Evolution**: Keep API changes well-documented
4. **Tool Integration**: Modern tools like cargo-nextest and cargo-machete save time
5. **Strictness Pays**: Start with strict clippy rules for long-term maintainability

These recommendations will help SQLiteGraph meet and exceed 2025 Rust quality standards while maintaining its focus on deterministic behavior and performance.