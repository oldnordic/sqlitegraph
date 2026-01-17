# Testing Patterns

**Analysis Date:** 2026-01-17

## Test Framework

**Runner:**
- Rust built-in test framework
- No separate test runner configuration

**Assertion Library:**
- Built-in `assert!`, `assert_eq!`, `assert_ne!`
- Custom result comparison helpers

**Run Commands:**
```bash
cargo test                              # Run all tests
cargo test --package sqlitegraph         # Test library only
cargo test --package sqlitegraph-cli     # Test CLI only
cargo test --test bfs_tests             # Specific test file
cargo test -- --nocapture               # Show print output
RUST_BACKTRACE=1 cargo test             # With backtrace on failure
```

## Test File Organization

**Location:**
- Unit tests: Co-located with source using `#[cfg(test)]`
- Integration tests: Separate files in `sqlitegraph/tests/` directory
- CLI tests: Use `assert_cmd` crate

**Naming:**
- `*_tests.rs` for integration test files
- Examples: `algo_tests.rs`, `bfs_tests.rs`, `backend_selector_tests.rs`
- Unit test functions: `test_{function_name}` or descriptive names

**Structure:**
```
sqlitegraph/
  src/
    algo.rs
    algo.rs (contains #[cfg(test)] mod tests)
  tests/
    algo_tests.rs
    bfs_tests.rs
    backend_selector_tests.rs
    # ... more test files
```

## Test Structure

**Suite Organization:**
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_descriptive_name() {
        // arrange
        let input = create_test_data();

        // act
        let result = function_under_test(input);

        // assert
        assert_eq!(result, expected);
    }

    #[test]
    fn test_edge_case() {
        // test code
    }
}
```

**Patterns:**
- No `beforeEach` fixtures (use helper functions)
- Tests are independent (no shared state)
- Helper functions for common setup (`insert_entity`, `insert_edge`)
- Real database instances (not mocks)

## Mocking

**Framework:**
- No mocking framework used
- Real implementations only

**Patterns:**
- In-memory SQLite databases for fast tests
- Temporary files via `tempfile` crate
- Direct instantiation of backends

**What to Mock:**
- Generally nothing - use real implementations

**What NOT to Mock:**
- Database operations (use in-memory SQLite)
- File I/O (use temp files)
- Core business logic

## Fixtures and Factories

**Test Data:**
```rust
// Helper functions in test modules
fn insert_entity(graph: &Graph, label: &str, properties: Value) -> i64 {
    graph.insert_node(NodeSpec {
        label: label.to_string(),
        properties,
    }).unwrap()
}
```

**Location:**
- Helper functions: Co-located in test modules
- Shared fixtures: None (each test is self-contained)
- Test databases: Created per test or per suite

## Coverage

**Requirements:**
- All code paths must have test coverage (from CONTRIBUTING.md)
- Coverage tracked for awareness
- Focus on critical paths (parsers, backend logic)

**Configuration:**
- No explicit coverage tools configured
- Manual review of test coverage

**View Coverage:**
- Not automated
- Review via `cargo test` output

## Test Types

**Unit Tests:**
- Location: Co-located with source code
- Scope: Test single module in isolation
- Speed: Fast (ms per test)
- Examples: Inline `#[cfg(test)]` modules

**Integration Tests:**
- Location: `sqlitegraph/tests/` directory
- Scope: Test multiple modules together
- Real backends: In-memory SQLite, temp files
- Examples: `algo_tests.rs`, `bfs_tests.rs`

**CLI Tests:**
- Framework: `assert_cmd` crate
- Location: CLI test files
- Scope: End-to-end CLI command testing

**Benchmarks:**
- Framework: Criterion.rs
- Location: `sqlitegraph/benches/`
- Harness: `harness = false` in Cargo.toml
- Examples: `bfs.rs`, `insert.rs`, `k_hop.rs`, `hnsw.rs`

## Common Patterns

**Helper Functions:**
```rust
fn setup_test_graph() -> Graph {
    Graph::open_in_memory().unwrap()
}

fn insert_test_data(graph: &Graph) -> Vec<i64> {
    // Insert and return IDs
}
```

**Error Testing:**
```rust
#[test]
fn test_invalid_input_returns_error() {
    let graph = setup_test_graph();
    let result = graph.insert_node(invalid_spec);
    assert!(result.is_err());
}
```

**Deterministic Testing:**
- No random data (use fixed seeds if needed)
- Reproducible results required
- No reliance on system time

## Quality Gates

**Pre-commit Testing:**
```bash
cargo fmt --all
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace
```

**CI Testing:**
- No explicit CI configuration detected
- Manual testing before commits

**Benchmark Regression:**
- Baseline tracked in `sqlitegraph_bench.json`
- Criterion generates comparison reports

---

*Testing analysis: 2026-01-17*
*Update when test patterns change*
