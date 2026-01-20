# Technology Stack

**Analysis Date:** 2026-01-20

## Languages

**Primary:**
- Rust 2024 edition - Main library (`sqlitegraph/Cargo.toml` uses `edition = "2024"`)
- Rust 2021 edition - CLI tool (`sqlitegraph-cli/Cargo.toml` uses `edition = "2021"`)

**Secondary:**
- Python - Used in benchmark scripts in `/scripts` (comparison scripts against NetworkX)

## Runtime

**Environment:**
- Rust 1.70.0+ (Minimum Supported Rust Version per `sqlitegraph/clippy.toml`)

**Package Manager:**
- Cargo (workspace resolver v3)
- Lockfile: Present (`Cargo.lock` at workspace root)

## Frameworks

**Core:**
- rusqlite 0.31 - SQLite bindings with bundled libsqlite3
- None - Pure Rust embedded database (no runtime required)

**Testing:**
- criterion 0.5 - Benchmarking framework with HTML reports
- assert_cmd 2.1 - CLI testing utilities
- tempfile 3.23 - Temporary file/directory management

**Build/Dev:**
- clap 4.5 - CLI argument parsing with derive macros
- binrw 0.13 - Binary serialization for native backend
- bincode 1.3 - Binary serialization for graph data
- rayon 1.10 - Parallelism (optional via features)

## Key Dependencies

**Critical:**
- rusqlite 0.31 (bundled) - Core SQLite backend integration
- serde 1.0 + serde_json 1.0 - JSON serialization for entity/edge metadata
- thiserror 1.0 - Error handling derivations
- anyhow 1.0 - CLI error handling

**Infrastructure:**
- bytemuck 1.13 + bytemuck_derive - Zero-copy byte casting (performance)
- memmap2 0.9 - Memory-mapped file I/O for native backend
- ahash 0.8 - Fast non-cryptographic hashing
- parking_lot 0.12 - High-performance mutex types
- arc-swap 1.7 - Atomic arc swapping for concurrent access
- rand 0.8 - Random number generation for graph algorithms
- log 0.4 - Logging facade

**Binary Format:**
- binrw 0.13 - Declarative binary reader/writer for V2 backend structures
- bincode 1.3 - Compact binary serialization

**Performance/Parallelism:**
- rayon 1.10 - Data parallelism (graph algorithms, WAL recovery)

## Configuration

**Environment:**
- `GRAPH_BACKEND` - Runtime backend selection (SQLite|Native) via `backend_selector.rs`
- `SQLITEGRAPH_BENCH_FILE` - Benchmark output file path
- Debug flags: `EDGE_DEBUG`, `V2_CLUSTER_AUDIT`, `CLUSTER_VALIDATION_DEBUG`, `WRITEBUF_DEBUG`, `HEADER_VALIDATE_DEBUG`, `TRUNC_AUDIT`, `SLOT_CORRUPTION_DEBUG`, `TX_BEGIN_AUDIT`

**Build:**
- Workspace `Cargo.toml` with shared release profiles
- Feature flags: `sqlite-backend` (default), `native-v2`, `debug`, `trace_v2_io`, `bench-ci`
- Clippy configuration: `sqlitegraph/clippy.toml`

**Profile Configurations:**
- Release: `opt-level = 3`, `codegen-units = 1`, `lto = "thin"`, `panic = "abort"`
- Bench: Inherits release with `debug = true` for flamegraph profiling
- Test: `opt-level = 2` for faster test execution

## Platform Requirements

**Development:**
- Rust 1.70.0+ toolchain
- Cargo with workspace resolver v3

**Production:**
- Embedded deployment: No external dependencies for native backend
- SQLite backend: Uses bundled libsqlite3 (no system SQLite required)
- Platforms: Linux, macOS, Windows (cross-platform atomic operations)

**Feature-Specific:**
- HNSW vector search: Optional, works with both backends
- WAL mode: SQLite backend supports WAL; native backend has custom WAL implementation

---

*Stack analysis: 2026-01-20*
