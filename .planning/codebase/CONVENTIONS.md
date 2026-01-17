# Coding Conventions

**Analysis Date:** 2026-01-17

## Naming Patterns

**Files:**
- `snake_case.rs` for all Rust source files
- `*_tests.rs` for test files (e.g., `algo_tests.rs`, `bfs_tests.rs`)
- `mod.rs` for module exports in directories

**Functions:**
- `snake_case` for all functions
- No special prefix for async functions (no async in codebase)
- Verb-noun pattern: `insert_node`, `get_neighbors`, `connected_components`
- Constructor: `new()`, `open_in_memory()`

**Variables:**
- `snake_case` for variables
- Descriptive names: `entity_id`, `edge_count`, `graph_file`
- No underscore prefix for private members

**Types:**
- `PascalCase` for structs and enums (no I prefix)
- `PascalCase` for type aliases
- `PascalCase` for enum names, `UPPER_CASE` for values
- Backend-prefixed types: `NativeNodeId`, `SlotId`, `ClusterId`

## Code Style

**Formatting:**
- Tool: `cargo fmt` (rustfmt)
- Edition: Rust 2024 (library), Rust 2021 (CLI)
- Line length: Default (100 characters suggested)
- Indentation: 4 spaces (Rust standard)
- No explicit rustfmt.toml (uses defaults)

**Linting:**
- Tool: Clippy with custom configuration
- Config file: `sqlitegraph/clippy.toml`
- Custom thresholds:
  - Cognitive complexity: 30 (higher than default)
  - Type complexity: 300 (much higher than default)
  - Too many arguments: 12
  - Too many lines: 150
  - MSRV: 1.70.0
- Run: `cargo clippy --workspace --all-features -- -D warnings`

## Import Organization

**Order:**
1. Standard library (`std::*`, `core::*`)
2. External crates (`rusqlite`, `serde`, etc.)
3. Internal modules (`crate::`)
4. Local imports (`super::`, `use super::*`)

**Grouping:**
- No blank lines strictly required but common for readability
- Alphabetical within groups suggested

**Path Aliases:**
- None defined (uses `crate::` for internal references)

## Error Handling

**Patterns:**
- `Result<T, E>` for fallible operations
- Custom `SqliteGraphError` enum via thiserror
- `?` operator for propagation
- Context-preserving error chains

**Error Types:**
- Location: `sqlitegraph/src/errors.rs`
- Throw on: Invalid input, I/O failures, invariant violations
- Return: Expected failures via `Result`
- Logging: Errors propagated with context

**Custom Error:**
```rust
// Define errors using thiserror
#[derive(Error, Debug)]
pub enum SqliteGraphError {
    #[error("Node not found: {0}")]
    NodeNotFound(i64),
    // ...
}
```

## Logging

**Framework:**
- Basic `log = "0.4"` support
- No structured logging framework
- Levels: debug, info, warn, error (standard log levels)

**Patterns:**
- Debug prints gated behind feature flags
- Environment variables for debugging:
  - `V2_SLOT_DEBUG`
  - `CLUSTER_DEBUG`
  - `TRACE_V2_IO` (requires feature)
- Minimal production logging

## Comments

**When to Comment:**
- Explain why, not what (for non-obvious code)
- Document business rules and invariants
- Explain complex algorithms
- Avoid obvious comments

**Rustdoc:**
- Required for public API
- Format: `///` for documentation comments
- Examples included for major functions
- Module-level docs with `//!`

**TODO Comments:**
- Document known issues in `docs/todo.md`
- TODO/FIXME in code for immediate action items
- Format: `// TODO: description`

## Function Design

**Size:**
- Preferred: Under 300 lines
- Maximum: 600 lines with justification
- Extract helpers for complex logic

**Parameters:**
- Preferred: 3 or fewer parameters
- More than 3: Use struct or builder pattern
- Destructure in parameter list for clarity

**Return Values:**
- Explicit `Result<T, E>` for fallible operations
- Early returns for guard clauses
- No implicit `unwrap()` in production paths

## Module Design

**Exports:**
- Named exports preferred (`pub use`)
- Public API re-exported from `lib.rs`
- Internal helpers kept private

**Barrel Files:**
- `mod.rs` for directory modules
- Re-exports of public types
- Avoid circular dependencies

## Special Patterns

**Feature Gates:**
```rust
#[cfg(feature = "native-v2")]
pub fn native_only_function() { }
```

**Testing Attributes:**
```rust
#[cfg(test)]
mod tests {
    // Unit tests co-located
}
```

**Benchmark Configuration:**
```toml
[[bench]]
name = "bfs"
harness = false
```

## Development Workflow

**Quality Gates (from CONTRIBUTING.md):**
```bash
cargo fmt --all
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace
cargo doc --workspace --no-deps
```

**Pre-commit Checklist:**
- Code formatted
- No clippy warnings
- All tests pass
- Documentation builds

---

*Convention analysis: 2026-01-17*
*Update when patterns change*
