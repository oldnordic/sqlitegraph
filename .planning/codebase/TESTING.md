# Testing Patterns

**Analysis Date:** 2026-01-20

## Test Framework

**Runner:**
- `cargo test` - Built-in Rust test runner
- No custom test harness configuration
- Test profile: `opt-level = 2` (optimized for speed, see workspace `Cargo.toml`)

**Assertion Library:**
- Standard `assert!`, `assert_eq!`, `assert_ne!` macros
- Custom assertions for specific use cases

**Run Commands:**
```bash
cargo test                          # Run all tests
cargo test --package sqlitegraph    # Run library tests only
cargo test --features native-v2     # Run tests with feature flag
cargo test --release                # Run optimized tests (faster)
cargo test -- --nocapture           # Show print output
cargo test test_name                # Run specific test
```

**Coverage:**
```bash
cargo tarpaulin --out Html          # Generate HTML coverage (if tarpaulin installed)
# No official coverage target enforced
```

## Test File Organization

**Location:**
- Integration tests: `/tests/*.rs` (project root)
- Module tests: `/sqlitegraph/src/*/tests.rs` (co-located within modules)
- Benchmarks: `/sqlitegraph/benches/*.rs`

**Naming:**
- Integration tests: `{feature}_tests.rs` (e.g., `api_ergonomics_tests.rs`)
- Module tests: `tests.rs` within module directory
- Test functions: `test_{what_is_tested}`

**Directory structure:**
```
tests/
├── api_ergonomics_tests.rs         # API design tests
├── dsl_tests.rs                    # DSL functionality
├── pipeline_tests.rs               # Pipeline integration
├── reasoning_integration_tests.rs  # Reasoning feature tests
├── v2_*.rs                         # Native V2 backend tests (many files)
└── *_regression_tests.rs           # Regression test suites

sqlitegraph/src/
├── pattern_engine/tests.rs         # Module-level pattern engine tests
├── pattern_engine_cache/tests.rs   # Cache tests
├── backend/native/adjacency/tests.rs
└── backend/native/*/tests.rs       # More module tests
```

**Feature-gated tests:**
```rust
#![cfg(feature = "v2_experimental")]  // At top of test file

#[cfg(feature = "native-v2")]
mod tests {
    // ...
}
```

## Test Structure

**Suite Organization:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Helper functions (no #[test] attribute)
    fn create_test_graph() -> SqliteGraph {
        SqliteGraph::open_in_memory().expect("Failed to create test graph")
    }

    // Test functions
    #[test]
    fn test_basic_functionality() {
        // Arrange, Act, Assert pattern
    }

    #[test]
    fn test_edge_case() {
        // ...
    }
}
```

**Patterns:**
- **Setup:** Helper functions in test module for common setup
- **Teardown:** Rely on Drop implementations for cleanup
- **Assertion pattern:** `assert_eq!(expected, actual)` with descriptive messages
- **Temp files:** Use `tempfile::NamedTempFile` for file-based tests

**Helper function pattern:**
```rust
// From pattern_engine/tests.rs
fn create_test_graph() -> crate::graph::SqliteGraph {
    crate::graph::SqliteGraph::open_in_memory().expect("Failed to create test graph")
}

fn insert_entity(graph: &crate::graph::SqliteGraph, kind: &str, name: &str) -> i64 {
    graph.insert_entity(&GraphEntity {
        id: 0,
        kind: kind.into(),
        name: name.into(),
        file_path: None,
        data: json!({"name": name}),
    }).expect("Failed to insert entity")
}
```

## Mocking

**Framework:**
- No formal mocking framework (like `mockall`)
- Use `tempfile::NamedTempFile` for filesystem tests
- In-memory databases for SQLite backend tests
- Custom test doubles where needed

**Patterns:**

**In-memory database:**
```rust
let graph = SqliteGraph::open_in_memory().expect("Failed to create test graph");
```

**Temp file for native backend:**
```rust
let temp_file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
let graph_file = GraphFile::create(temp_file.path()).expect("Failed to create graph file");
```

**Setup pattern from tests:**
```rust
fn setup_test_graph() -> (GraphFile, NamedTempFile) {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let graph_file = GraphFile::create(temp_file.path()).expect("Failed to create graph file");
    (graph_file, temp_file)
}
```

**What to Mock:**
- Database connections: Use in-memory for SQLite
- File I/O: Use `tempfile` for native backend
- External dependencies: Implement test trait or use configuration

**What NOT to Mock:**
- Core business logic (test real implementation)
- Data structures (use real instances)
- Algorithms (validate correctness)

## Fixtures and Factories

**Test Data:**
- Helper functions create test entities on-the-fly
- No separate fixture files or modules
- JSON data constructed inline with `serde_json::json!`

**Pattern:**
```rust
fn insert_entity(graph: &SqliteGraph, kind: &str, name: &str) -> i64 {
    graph.insert_entity(&GraphEntity {
        id: 0,
        kind: kind.into(),
        name: name.into(),
        file_path: None,
        data: json!({"name": name}),  // Inline JSON construction
    }).expect("Failed to insert entity")
}
```

**Location:**
- Fixtures defined in test modules
- Shared fixtures in common test modules when needed

**Complex test data from v2_full_roundtrip_integration_tests.rs:**
```rust
let original_node = NodeRecord::new(
    42,
    "ComplexFunction".to_string(),
    "complex_function_v2".to_string(),
    json!({
        "signature": "(param: i32) -> i32",
        "parameters": [{"name": "param", "type": "i32", "default": 0}],
        "return_type": "i32",
        "body": "return param * 2;",
        "metadata": {
            "lines": 1,
            "cyclomatic_complexity": 1,
            "last_modified": "2024-01-15T10:30:00Z"
        }
    })
);
```

## Coverage

**Requirements:**
- No official coverage target enforced
- Tests cover critical paths (invariants, roundtrips, serialization)
- Module-level tests for complex algorithms

**View Coverage:**
- Use external tools like `cargo-tarpaulin`
- No integrated coverage reporting

**High-value test areas:**
- Serialization/deserialization roundtrips
- File format invariants
- WAL recovery and rollback
- Adjacency list consistency
- Cache correctness

## Test Types

**Unit Tests:**
- Scope: Single function or module
- Location: Co-located in `tests.rs` modules
- Approach: Direct function calls, mocked dependencies

**Integration Tests:**
- Scope: Multiple components working together
- Location: `/tests/*.rs` in project root
- Approach: Full graph operations, end-to-end workflows

**Examples:**
- `api_ergonomics_tests.rs`: High-level API contracts
- `v2_full_roundtrip_integration_tests.rs`: Complete roundtrips
- `reasoning_integration_tests.rs`: Multi-component reasoning

**E2E Tests:**
- Scope: Full database lifecycle
- Examples: Persistence across reopen, recovery scenarios
- Location: Integration test files

**Regression Tests:**
- Naming: `*_regression_tests.rs`
- Purpose: Prevent recurrence of fixed bugs
- Examples: `v2_node_version_regression_test.rs`, `header_architecture_regression_tests.rs`

**Invariant Tests:**
- Purpose: Verify critical properties hold
- Location: `v2_layout_invariant_tests.rs`
- Example: Non-overlapping node/edge regions, deterministic offsets

**Example invariant test:**
```rust
#[test]
fn test_v2_node_and_edge_regions_do_not_overlap() {
    let (mut graph_file, _tmp) = setup_test_graph();

    // ... setup ...

    let header = graph_file.header();
    let node_region_end = node_region_start + (header.node_count as u64 * NODE_SLOT_SIZE);

    // Critical invariant
    assert!(
        header.edge_data_offset >= node_region_end,
        "edge_data_offset ({}) must be >= node region end ({})",
        header.edge_data_offset, node_region_end
    );
}
```

## Common Patterns

**Async Testing:**
- Not applicable (this is a synchronous library)
- No async/await in the codebase

**Error Testing:**
```rust
#[test]
fn test_error_case() {
    let graph = create_test_graph();

    let result = graph.get_entity(999);  // Non-existent ID

    assert!(result.is_err());
    assert!(matches!(result, Err(SqliteGraphError::NotFound(_))));
}
```

**Determinism Testing:**
```rust
#[test]
fn test_v2_full_serialization_determinism() {
    // Create identical test data
    let test_node = NodeRecord::new(123, "Type", "name", json!({"data": "value"}));

    // Serialize and verify same input produces same output
    let bytes1 = node.to_bytes();
    let bytes2 = node.to_bytes();
    assert_eq!(bytes1, bytes2, "Serialization must be deterministic");
}
```

**Roundtrip Testing:**
```rust
#[test]
fn test_v2_full_node_roundtrip_integration() {
    // Create original
    let original_node = NodeRecord::new(...);

    // Write
    node_store.write_node(&original_node).expect("Should write node successfully");

    // Read back
    let retrieved_node = node_store.read_node(42).expect("Should read node successfully");

    // Verify complete roundtrip integrity
    assert_eq!(retrieved_node.id, original_node.id);
    assert_eq!(retrieved_node.kind, original_node.kind);
    assert_eq!(retrieved_node.data, original_node.data);
}
```

**Property-based style (without proptest):**
```rust
#[test]
fn test_traversal_preserves_invariants() {
    // Test that BFS from any node preserves connectivity
    for &start_node in &[1, 10, 100, 1000] {
        let result = bfs(start_node, max_depth);
        assert!(result.visited_nodes.contains(&start_node));
        // ... more invariants
    }
}
```

## Benchmark Tests

**Location:**
- `/sqlitegraph/benches/*.rs`

**Framework:**
- `criterion` 0.5 with HTML reports

**Run Commands:**
```bash
cargo bench                          # Run all benchmarks
cargo bench --bench bfs              # Run specific benchmark
cargo bench -- --save-baseline main  # Save baseline for comparison
```

**Benchmark file example from `bfs.rs`:**
```rust
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn bfs_chain(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("bfs_chain");
    group.measurement_time(MEASURE);
    group.warm_up_time(WARM_UP);

    for &size in BENCHMARK_SIZES {
        group.bench_with_input(BenchmarkId::new("sqlite", size), &size, |b, &size| {
            b.iter(|| {
                // Benchmark code here
            });
        });
    }
}

criterion_group!(benches, bfs_chain);
criterion_main!(benches);
```

## Test Organization by Feature

**SQLite Backend:**
- Graph operations tests in integration tests
- Pattern matching tests in `pattern_engine/tests.rs`

**Native V2 Backend:**
- Many `v2_*.rs` test files
- WAL recovery tests
- Serialization roundtrip tests
- Layout invariant tests

**HNSW Vector Search:**
- Tests within `hnsw/index.rs` (marked `#[cfg(test)]`)
- End-to-end tests with SQLite persistence

## CI/CD

**Status:**
- No `.github` directory detected
- No CI configuration files present
- Tests run manually or via external tooling

---

*Testing analysis: 2026-01-20*
