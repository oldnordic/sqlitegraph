# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

SQLiteGraph is a deterministic, embedded graph database built entirely on SQLite, targeting SynCore/SPEC 16-17 requirements. The project provides graph storage, reasoning pipelines, pattern queries, and safety tooling with a focus on deterministic behavior and performance gating.

## Development Commands

### Building and Testing
```bash
# Build entire workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Run tests with verbose output
cargo test --workspace --verbose

# Run specific test suites
cargo test --test subgraph_tests
cargo test --test pipeline_tests
cargo test --test dsl_tests
cargo test --test backend_trait_tests
cargo test --test migration_tests
cargo test --test cli_reasoning_tests
cargo test --test cli_safety_tests
cargo test --test perf_gate_tests

# Run benchmarks
cargo bench

# Run specific benchmark suites
cargo bench --bench bench_insert
cargo bench --bench bench_traversal
cargo bench --bench bench_algorithms
cargo bench --bench bench_syncompat
```

### Code Quality
```bash
# Run clippy (workspace-wide)
cargo clippy --workspace --all-targets --all-features

# Format code
cargo fmt

# Build documentation
cargo doc --workspace --no-deps
```

### CLI Tool Usage
```bash
# Run CLI examples
cargo run --example basic_usage
cargo run --example migration_flow

# Use the CLI directly
cargo run --bin sqlitegraph -- --command status
cargo run --bin sqlitegraph -- --command list
cargo run --bin sqlitegraph -- --command safety-check --db memory
```

## Architecture Overview

### Workspace Structure
- **sqlitegraph/**: Core library crate with the graph database implementation
- **sqlitegraph-cli/**: Command-line interface crate
- **Root level**: Integration tests and workspace configuration

### Core Components

#### Graph Backend (`sqlitegraph/src/graph/`)
- `SqliteGraph`: Main SQLite-backed graph implementation
- `GraphEntity`, `GraphEdge`: Core data structures
- MVCC-lite snapshots for read isolation
- Deterministic indexing and adjacency management

#### Backend Abstraction (`sqlitegraph/src/backend.rs`)
- `GraphBackend` trait for backend independence
- `SqliteGraphBackend`: SQLite implementation
- `BackendClient`: Ergonomic wrapper with helper types

#### Pattern Engine (`sqlitegraph/src/pattern_engine/`)
- Triple pattern matching with fast-path caching
- Deterministic pattern queries and filtering
- Cache-enabled performance optimization

#### Reasoning System (`sqlitegraph/src/reasoning.rs`, `sqlitegraph/src/pipeline.rs`)
- `ReasoningPipeline`: Multi-step reasoning (pattern → k-hop → filter → score)
- DSL parser for query composition
- Deterministic candidate expansion and ranking

#### Migration & Dual Runtime (`sqlitegraph/src/migration.rs`, dual_*.rs)
- `MigrationManager`: Safe schema migrations with dual-write shadow reads
- `DualRuntime`: Compare two backends for consistency validation
- Shadow-read workflows and cutover procedures

#### Safety Tooling (`sqlitegraph/src/safety.rs`)
- Orphan edge detection and validation
- Duplicate edge detection
- Referential integrity checks
- `run_safety_checks()` with strict mode for CI

### Key Design Constraints

1. **Deterministic Behavior**: All operations use sorted adjacency and seeded RNG
2. **300 LOC File Limit**: Every public module capped at 300 lines for auditability
3. **No Async Dependencies**: Pure Rust + SQLite implementation
4. **TDD Workflow**: Tests-first development approach

## Testing Strategy

### Test Categories
- **Unit Tests**: Within individual modules
- **Integration Tests**: `/tests/*.rs` files for end-to-end workflows
- **CLI Tests**: Validate CLI commands and DSL parsing
- **Performance Gates**: Baseline enforcement via `sqlitegraph_bench.json`
- **Safety Tests**: Ensure invariants and error handling

### Benchmark Gating
Performance regression prevention is enforced via:
- `sqlitegraph_bench.json`: Contains deterministic baseline metrics
- `bench_gates` module: Records and validates performance thresholds
- CI integration ensures no performance regressions

## CLI Commands and DSL

The CLI supports deterministic subcommands for:
- `status`: Backend and entity count information
- `list`: Entity IDs and names (ascending order)
- `subgraph`: JSON neighborhood extraction with depth and type filters
- `pipeline`: Reasoning pipeline execution with DSL input
- `explain-pipeline`: Pipeline step breakdown and counts
- `dsl-parse`: DSL validation and classification
- `safety-check`: Integrity validation with `--strict` mode

### DSL Examples
- `CALLS->USES`: Pattern query for edge chains
- `3-hop type=Fn`: Subgraph extraction with depth and node type filter
- `pattern CALLS*3 filter type=Module`: Reasoning pipeline with repetition

## Important Implementation Notes

### Dead Code Warnings
The codebase contains many items used only through:
- CLI tooling
- Test suite execution
- Dynamic dispatch paths
- Benchmark harnesses

Clippy reports ~149 false-positive dead code warnings. A manual audit confirmed all flagged items are used - these warnings are informational and should be ignored.

### Schema Versioning
- Version 1: Base tables and indexes
- Version 2: Migration history tracking
- Future versions must maintain upgrade auditability
- Binary refuses to open databases with newer schema versions

### Performance Monitoring
Use the built-in metrics system:
```bash
sqlitegraph --command metrics [--reset-metrics]
```
This reports prepared statement counts, transaction metrics, and cache performance.

## Development Workflow

1. **Start with Tests**: Write failing tests first (TDD approach)
2. **Keep Files Small**: Maintain ≤300 LOC per module
3. **Deterministic Ordering**: Use sorted results throughout
4. **Safety Checks**: Run `safety-check --strict` before commits
5. **Benchmark Validation**: Ensure performance gates pass
6. **CLI Consistency**: Test DSL expressions through CLI before code integration

## File Locations

- Core library: `sqlitegraph/src/`
- CLI binary: `sqlitegraph/src/bin/sqlitegraph.rs`
- Examples: `sqlitegraph/examples/`
- Integration tests: `tests/`
- Benchmark baselines: `sqlitegraph/sqlitegraph_bench.json`
- Manual: `manual.md` (comprehensive operator guide)