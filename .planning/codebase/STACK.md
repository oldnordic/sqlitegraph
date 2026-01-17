# Technology Stack

**Analysis Date:** 2026-01-17

## Languages

**Primary:**
- Rust 2024 - Core library (sqlitegraph/Cargo.toml:4)
- Rust 2021 - CLI application (sqlitegraph-cli/Cargo.toml:4)

**Secondary:**
- Markdown - Documentation
- TOML - Configuration
- Shell - Build scripts (if any)

## Runtime

**Environment:**
- Rust (no external runtime required)
- Target platforms: Linux, macOS, Windows
- MSRV: 1.70.0 (from sqlitegraph/clippy.toml:13)

**Package Manager:**
- Cargo - Rust's package manager
- Workspace configuration at root Cargo.toml
- Resolver: version 3
- Lockfile: Cargo.lock present

## Frameworks

**Core:**
- None (vanilla Rust library)

**CLI:**
- clap 4.x - Command-line argument parsing and UI
- serde/serde_json - Serialization for JSON output

**Testing:**
- Rust built-in test framework (#[test])
- criterion 0.5 - Benchmarking with HTML reports
- assert_cmd 2 - CLI command testing
- tempfile 3 - Temporary file handling

**Build/Dev:**
- Cargo - Build system and package manager
- rustfmt - Code formatting
- clippy - Linting with custom configuration

## Key Dependencies

**Critical:**
- rusqlite 0.31 - SQLite database backend (bundled)
- thiserror 1 - Error handling
- serde 1 - Serialization framework
- memmap2 0.9 - Memory-mapped file I/O for native backend

**Infrastructure:**
- parking_lot 0.12 - Efficient mutexes and RwLock
- ahash 0.8 - Fast hashing
- arc-swap 1 - Atomic reference swapping
- binrw 0.13 - Binary format reading/writing
- bytemuck 1.13 - Memory-safe byte operations

**Algorithms:**
- rand 0.8 - Random number generation for HNSW

## Configuration

**Environment:**
- No environment variables required
- Code-based configuration via GraphConfig
- Backend selection via BackendKind enum
- Configuration types in sqlitegraph/src/config/config.rs

**Build:**
- Workspace Cargo.toml with resolver version 3
- Custom clippy.toml with relaxed thresholds
- Optimization profiles for release, bench, and test

## Platform Requirements

**Development:**
- Rust 1.70.0 or later
- Cargo (comes with Rust)
- Linux/macOS/Windows (any platform with Rust support)

**Production:**
- Standalone native binary (no runtime dependencies)
- Cross-platform support (Linux, macOS, Windows)
- Bundled SQLite (no external sqlite3 required)

---

*Stack analysis: 2026-01-17*
*Update after major dependency changes*
