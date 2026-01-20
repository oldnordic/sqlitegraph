# Technology Stack

**Analysis Date:** 2026-01-20

## Languages

**Primary:**
- Rust 2024 Edition - Core library and CLI (`sqlitegraph/`, `sqlitegraph-cli/`)
- Rust 2021 Edition - CLI binary (for ecosystem compatibility)

**Secondary:**
- SQL - SQLite queries and schema definitions
- Markdown - Documentation (`docs/`, `*.md`)

## Runtime

**Environment:**
- Rust 1.70.0+ (MSRV defined in `clippy.toml`)
- Edition 2024 for library, Edition 2021 for CLI

**Package Manager:**
- Cargo (workspace with resolver = "3")
- Lockfile: `Cargo.lock` (present, committed)

**Workspace Structure:**
```
sqlitegraph/          # Core library crate (lib)
sqlitegraph-cli/      # CLI binary crate
```

## Frameworks

**Core:**
- rusqlite 0.31 - SQLite database access with bundled libsqlite3
- None - Pure Rust embedded database (no external framework)

**Testing:**
- Criterion 0.5 - Benchmark framework with HTML reports
- assert_cmd 2.1 - CLI command testing
- tempfile 3.23 - Temporary file/directory for tests

**Build/Dev:**
- clap 4.5 - CLI argument parsing (derive API)
- rayon 1.10 - Parallel data processing

## Key Dependencies

**Critical:**
- rusqlite 0.31 (bundled) - SQLite backend for ACID transactions
- serde 1.0 + serde_json 1.0 - JSON serialization for metadata
- binrw 0.13 - Binary serialization for Native V2 format
- bytemuck 1.13 - Byte-level casting for memory-mapped I/O
- memmap2 0.9 - Memory-mapped file access

**Infrastructure:**
- ahash 0.8 - Fast non-cryptographic hashing
- parking_lot 0.12 - High-performance mutexes
- rand 0.8 - Random number generation
- arc-swap 1.7 - Atomic pointer swapping
- log 0.4 - Logging facade
- thiserror 1.0 - Error derivation

**Binary I/O:**
- bincode 1.3 - Binary serialization
- binrw 0.13 - Declarative binary read/write

**CLI:**
- clap 4.5 (derive) - CLI parsing
- anyhow 1.0 - Error handling in CLI

## Configuration

**Environment:**
- Feature-based backend selection (`default`, `sqlite-backend`, `native-v2`)
- Debug features: `debug`, `trace_v2_io`, `bench-ci`
- No environment variables required for core operation

**Build:**
- Workspace `Cargo.toml` with profile configurations
- Release profile: `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`
- Bench profile: Same as release with `debug = true` for profiling
- Test profile: `opt-level = 2` for faster test execution

**Linting:**
- Clippy configured in `sqlitegraph/clippy.toml`
- MSRV: 1.70.0
- Complexity thresholds adjusted for graph algorithms

## Platform Requirements

**Development:**
- Rust 1.70.0+ toolchain
- Cargo with workspace support
- SQLite (bundled with rusqlite)

**Production:**
- Linux, macOS, Windows (cross-platform via Rust)
- No external runtime dependencies for Native backend
- libsqlite3 dynamically linked for SQLite backend (unless bundled)

**Native Backend (V2):**
- Pure Rust, no external dependencies
- Custom binary format with clustered adjacency
- WAL (Write-Ahead Log) for durability

**SQLite Backend:**
- Uses bundled libsqlite3 via rusqlite feature
- Supports WAL mode for concurrent access
- Standard SQLite file format

---

*Stack analysis: 2026-01-20*
