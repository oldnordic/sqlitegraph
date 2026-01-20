# Testing Patterns

**Analysis Date:** 2026-01-20

## Test Framework

**Runner:**
- Rust built-in `cargo test`
- No external test runner config (using default libtest)

**Assertion Library:**
- Standard `assert!`, `assert_eq!`, `assert_ne!` macros
- No additional assertion libraries

**Config:**
- Profile configuration in `Cargo.toml`:
  ```toml
  [profile.test]
  opt-level = 2  # Optimized for speed while maintaining debuggability
  ```

**Run Commands:**
```bash
cargo test              # Run all tests
cargo test --no-fail-fast  # Run all tests regardless of failure
cargo test test_name    # Run specific test
cargo test -- --ignored # Run ignored tests
cargo test --release    # Run tests in release mode
```

## Test File Organization

**Location:**
- Integration tests: `/home/feanor/Projects/sqlitegraph/tests/*.rs` (workspace root)
- Library unit tests: `/home/feanor/Projects/sqlitegraph/sqlitegraph/tests/*.rs` (crate-local)
- In-source unit tests: `#[cfg(test)] mod tests { ... }` blocks within source files

**Naming:**
- Integration tests: `<topic>_tests.rs` or `<topic>_tests.rs` (e.g., `cache_tests.rs`, `integration_tests.rs`)
- Regression tests: `<phase>_<description>_regression.rs` (e.g., `phase38_graphfile_io_tests.rs`)
- TDD tests: `<feature>_tdd_tests.rs` (e.g., `v2_clustered_adjacency_tdd_tests.rs`)

**Structure:**
```
tests/
├── cache_tests.rs
├── integration_tests.rs
├── pattern_engine_tests.rs
├── snapshot_invariants_tests.rs
├── v2_clustered_adjacency_tdd_tests.rs
├── phase38_graphfile_io_tests.rs
├── helpers/
│   ├── mod.rs
│   └── v2_fixture_builders.rs

sqlitegraph/tests/
├── (crate-specific integration tests)
```

## Test Structure

**Suite Organization:**
```rust
//! Tests for the lightweight pattern engine.
//!
//! This test suite validates the deterministic triple pattern matching functionality
//! using TDD approach to ensure correctness and performance.

use serde_json::json;
use sqlitegraph::{
    GraphEdge, GraphEntity, PatternTriple, SqliteGraph,
    backend::BackendDirection,
    index::{add_label, add_property},
    match_triples,
};

/// Helper function to create test graph
fn create_test_graph() -> SqliteGraph {
    let graph = SqliteGraph::open_in_memory().expect("Failed to create test graph");
    // ... setup code
    graph
}

#[test]
fn test_pattern_triple_basic_functionality() {
    let graph = create_test_graph();
    // ... test code
    assert_eq!(matches.len(), 3);
}

#[test]
fn test_pattern_triple_with_label_filters() {
    // ... another test
}
```

**Patterns:**

**Setup pattern:**
- Helper functions named `create_*`, `insert_*`, `setup_*`
- Common helpers defined at top of test file
- Test fixtures created with `SqliteGraph::open_in_memory()`
- Assertive setup: `.expect("descriptive message")` on setup operations

**Teardown pattern:**
- RAII: in-memory databases auto-cleanup on drop
- Tempfile cleanup via `tempfile` crate for file-based tests
- No explicit teardown functions needed

**Assertion pattern:**
```rust
// Equality assertions
assert_eq!(matches.len(), 3);
assert_eq!(neighbors, vec![b, c]);

// Presence assertions
assert!(f1_to_f2_match.is_some());
assert!(result.is_err());

// Custom assertions with context
assert!(
    duration.as_secs() < 1,
    "Pattern matching took too long: {:?}",
    duration
);

// Deterministic ordering assertions
for i in 1..matches.len() {
    assert!(
        matches[i - 1].start_id < matches[i].start_id,
        "Matches not ordered by start_id"
    );
}
```

## Mocking

**Framework:** No dedicated mocking framework

**Patterns:**
- In-memory databases for fast tests: `SqliteGraph::open_in_memory()`
- Helper functions that create test fixtures instead of mocks
- Real implementations used in tests (not interface mocks)

**What to Mock:**
- External services: not applicable (embedded database)
- File I/O: use tempfiles
- Time: not mocked (use deterministic algorithms)

**What NOT to Mock:**
- Database operations (use in-memory SQLite)
- Graph algorithms (test against real implementation)
- Serialization/deserialization (test real data)

## Fixtures and Factories

**Test Data:**
```rust
fn create_test_graph() -> SqliteGraph {
    let graph = SqliteGraph::open_in_memory().expect("graph");
    // Insert nodes
    let f1 = insert_entity(&graph, "Function", "process_data");
    let f2 = insert_entity(&graph, "Function", "validate_input");
    // Insert edges
    insert_edge(&graph, f1, f2, "CALLS");
    graph
}

fn insert_entity(graph: &SqliteGraph, kind: &str, name: &str) -> i64 {
    graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: kind.into(),
            name: name.into(),
            file_path: None,
            data: json!({"name": name, "type": kind}),
        })
        .expect("Failed to insert entity")
}

fn insert_edge(graph: &SqliteGraph, from: i64, to: i64, edge_type: &str) -> i64 {
    graph
        .insert_edge(&GraphEdge {
            id: 0,
            from_id: from,
            to_id: to,
            edge_type: edge_type.into(),
            data: json!({"type": edge_type}),
        })
        .expect("Failed to insert edge")
}
```

**Location:**
- Fixture helpers defined in test files
- Shared helpers in `tests/helpers/mod.rs` and `tests/helpers/v2_fixture_builders.rs`
- In-source fixtures in `#[cfg(test)]` modules

**V2 Fixture Builders:**
- `tests/helpers/v2_fixture_builders.rs` - specialized builders for native backend v2 tests

## Coverage

**Requirements:** None enforced

**View Coverage:**
- No coverage tool configured (no `tarpaulin` or similar)
- Manual review of test coverage

**Test Distribution:**
- ~100 test files in `tests/`
- ~90 test files in `sqlitegraph/tests/`
- In-source tests in key modules: `mvcc.rs`, `cache.rs`

## Test Types

**Unit Tests:**
- In-source tests in `#[cfg(test)]` modules
- Test private functions and internal logic
- Fast, focused, no external dependencies

**Example from `cache.rs`:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_state_creation() {
        let mut outgoing = HashMap::new();
        let mut incoming = HashMap::new();

        outgoing.insert(1, vec![2, 3]);
        incoming.insert(1, vec![]);

        let state = SnapshotState::new(&outgoing, &incoming);

        assert_eq!(state.node_count(), 1);
        assert_eq!(state.edge_count(), 2);
        assert!(state.contains_node(1));
        assert!(!state.contains_node(2));
    }
}
```

**Integration Tests:**
- Tests in `tests/` directory
- Test public API behavior
- Multi-file tests for complex scenarios
- Load test scenarios

**Example from `integration_tests.rs`:**
```rust
#[test]
fn test_integration_call_graph_traversal() {
    let graph = complex_graph();
    let visited = bfs_neighbors(&graph, 1, 6).expect("bfs");
    assert_eq!(
        visited,
        vec![1, 2, 3, 8, 4, 11, 9, 5, 12, 10, 17, 6, 13, 18, 7, 19]
    );
}
```

**E2E Tests:**
- Full system tests using real database operations
- Snapshot serialization/deserialization tests
- Recovery and backup tests

**Regression Tests:**
- Named by phase/issue: `phase38_graphfile_io_tests.rs`
- Specific bug reproducers: `v2_node_257_boundary_regression.rs`
- Corruption detection tests: `v2_edge_cluster_corruption_regression.rs`

**Invariant Tests:**
- Property-based testing style
- Named `*_invariants_tests.rs`
- Example: `v2_layout_invariant_tests.rs`, `snapshot_invariants_tests.rs`

**TDD Tests:**
- Named with `_tdd_` pattern: `v2_clustered_adjacency_tdd_tests.rs`
- Test-driven development approach for new features

**Performance/Load Tests:**
- `production_adjacency_load_test.rs`
- `v2_performance_validation.rs`
- Benchmarks in `benches/` directory

## Common Patterns

**Async Testing:**
- Not applicable (synchronous codebase)

**Error Testing:**
```rust
#[test]
fn test_pattern_triple_validation() {
    let graph = create_test_graph();

    // Test empty edge type validation
    let pattern = PatternTriple::new("");
    let result = match_triples(&graph, &pattern);
    assert!(result.is_err());

    // Test whitespace-only edge type validation
    let pattern = PatternTriple::new("   ");
    let result = match_triples(&graph, &pattern);
    assert!(result.is_err());
}
```

**Deterministic Ordering Tests:**
```rust
#[test]
fn test_pattern_triple_deterministic_ordering() {
    let graph = create_test_graph();
    let pattern = PatternTriple::new("BELONGS_TO");
    let matches = match_triples(&graph, &pattern).expect("Failed to match triples");

    assert_eq!(matches.len(), 4);

    // Verify deterministic ordering: sorted by start_id, then edge_id, then end_id
    for i in 1..matches.len() {
        assert!(
            matches[i - 1].start_id < matches[i].start_id
                || (matches[i - 1].start_id == matches[i].start_id
                    && matches[i - 1].edge_id < matches[i].edge_id)
                || (matches[i - 1].start_id == matches[i].start_id
                    && matches[i - 1].edge_id == matches[i].edge_id
                    && matches[i - 1].end_id <= matches[i].end_id),
            "Matches not in deterministic order at index {}: {:?} vs {:?}",
            i,
            matches[i - 1],
            matches[i]
        );
    }
}
```

**Performance Tests:**
```rust
#[test]
fn test_pattern_triple_performance_with_large_dataset() {
    let graph = SqliteGraph::open_in_memory().expect("Failed to create test graph");

    // Create a larger dataset for performance testing
    let mut entity_ids = Vec::new();
    for i in 0..100 {
        let id = insert_entity(&graph, "Node", &format!("node_{}", i));
        entity_ids.push(id);
    }

    let start = std::time::Instant::now();
    let matches = match_triples(&graph, &pattern).expect("Failed to match triples");
    let duration = start.elapsed();

    assert!(
        duration.as_secs() < 1,
        "Pattern matching took too long: {:?}",
        duration
    );
}
```

**Snapshot Isolation Tests:**
```rust
#[test]
fn test_snapshot_consistency_during_modifications() -> Result<(), SqliteGraphError> {
    let graph = SqliteGraph::open_in_memory()?;
    let node1 = insert_node(&graph, "node1")?;
    let snapshot = graph.snapshot()?;

    // Add more nodes after snapshot
    let node2 = insert_node(&graph, "node2")?;

    // Snapshot should not see new nodes
    let ids = snapshot.all_entity_ids();
    assert_eq!(ids.len(), 1);
    assert!(ids.contains(&node1));
    assert!(!ids.contains(&node2));

    Ok(())
}
```

## Benchmarking

**Framework:** `criterion` 0.5

**Config:**
- HTML reports enabled: `features = ["html_reports"]`
- Benchmark config in `sqlitegraph/Cargo.toml`

**Run Commands:**
```bash
cargo bench               # Run all benchmarks
cargo bench bfs           # Run specific benchmark
```

**Location:**
- `/home/feanor/Projects/sqlitegraph/sqlitegraph/benches/`

**Files:**
- `bfs.rs` - Breadth-first search benchmarks
- `k_hop.rs` - K-hop traversal benchmarks
- `insert.rs` - Insert operation benchmarks
- `hnsw.rs` - Vector search benchmarks
- `comprehensive_performance.rs` - Comprehensive performance tests
- `wal_recovery_benchmarks.rs` - WAL recovery benchmarks

**Benchmark structure:**
```rust
use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

fn bfs_chain(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("bfs_chain");
    group.measurement_time(MEASURE);
    group.warm_up_time(WARM_UP);

    for &size in BENCHMARK_SIZES {
        group.bench_with_input(BenchmarkId::new("sqlite", size), &size, |b, &size| {
            b.iter(|| {
                // benchmark code
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bfs_chain, bfs_star, bfs_random);
criterion_main!(benches);
```

## Test Organization by Backend

**SQLite Backend Tests:**
- Use `SqliteGraph::open_in_memory()` or `SqliteGraph::open()`
- Tests in `tests/` directory

**Native Backend Tests:**
- Use `NativeGraphBackend` with temp files
- Tests named `v2_*` or `native_*`
- Many in `sqlitegraph/tests/`

**Backend-Agnostic Tests:**
- Test `GraphBackend` trait behavior
- Use `BackendKind` enum for runtime selection
- Integration tests in `tests/integration_tests.rs`

## Debug Tests

**Conditional compilation:**
```rust
#[cfg(debug_assertions)]
{
    // Verify state is fully constructed
    assert_eq!(new_state.node_count(), outgoing.len(),
        "Snapshot state node count mismatch");
}
```

## Test Isolation

**No shared state:**
- Each test creates its own graph/database
- In-memory databases for isolation
- Tempfile with unique names for file-based tests

**Parallel execution:**
- Default `cargo test` runs tests in parallel
- Each test is isolated (no shared state issues)

## Fuzz Testing

**Files:**
- `tests/fuzz_common.rs` - Common fuzzing utilities
- Not fully implemented (partial support)

## Property-Based Testing

**Invariant tests:**
- `snapshot_invariants_tests.rs` - Tests that snapshot invariants hold
- `v2_layout_invariant_tests.rs` - V2 file layout invariants

---

*Testing analysis: 2026-01-20*
