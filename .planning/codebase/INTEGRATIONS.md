# External Integrations

**Analysis Date:** 2026-01-20

## APIs & External Services

**None detected**

This is a self-contained embedded database with no external API integrations. The project operates entirely as a local library and CLI tool with no network dependencies.

## Data Storage

**Databases:**
- SQLite (bundled rusqlite 0.31)
  - Connection: Direct file path or in-memory
  - Client: rusqlite with bundled libsqlite3 (no system dependency)
  - Schema location: `sqlitegraph/src/schema.rs`

- Custom Native V2 Format
  - Binary file format defined in `sqlitegraph/src/backend/native/`
  - Memory-mapped I/O via memmap2
  - WAL (Write-Ahead Log) for transaction logging

**File Storage:**
- Local filesystem only
- Database files: `.db` extension for SQLite backend, custom binary for native
- WAL files: `-wal` and `-shm` suffixes (SQLite), separate WAL file for native backend

**Caching:**
- LRU-K adjacency cache: `sqlitegraph/src/cache.rs`
- Pattern engine cache: `sqlitegraph/src/pattern_engine_cache/`
- No external caching services

## Authentication & Identity

**Auth Provider:**
- None - Local embedded database with no authentication layer

**Implementation:**
- Direct file access with OS-level permissions
- No user management, sessions, or authentication

## Monitoring & Observability

**Error Tracking:**
- None - Uses thiserror for structured error types

**Logs:**
- `log` 0.4 facade with `debug` feature flag
- Debug tracing via `trace_v2_io` feature for development
- No external logging service integration

**Metrics:**
- Internal metrics: `sqlitegraph/src/introspection.rs` for graph statistics
- WAL metrics: `sqlitegraph/src/backend/native/v2/wal/metrics/`
- No external metrics export

## CI/CD & Deployment

**Hosting:**
- GitHub repository: https://github.com/oldnordic/sqlitegraph
- crates.io: Published as `sqlitegraph`

**CI Pipeline:**
- None detected (no `.github/workflows/` directory)
- Manual testing via cargo test/bench

**Deployment:**
- Library: Published to crates.io
- CLI: Compiled binary distribution
- No hosted services

## Environment Configuration

**Required env vars:**
- None required for basic operation

**Optional env vars:**
- `GRAPH_BACKEND` - Backend selection (SQLite|Native)
- `SQLITEGRAPH_BENCH_FILE` - Benchmark output path
- Debug flags (development only):
  - `EDGE_DEBUG` - Edge cluster debugging
  - `V2_CLUSTER_AUDIT` - V2 serialization auditing
  - `CLUSTER_VALIDATION_DEBUG` - Node record validation
  - `WRITEBUF_DEBUG` - Write buffer debugging
  - `HEADER_VALIDATE_DEBUG` - Header validation debugging
  - `TRUNC_AUDIT` - File truncation auditing
  - `SLOT_CORRUPTION_DEBUG` - Slot corruption debugging
  - `TX_BEGIN_AUDIT` - Transaction begin auditing

**Secrets location:**
- Not applicable (no external services requiring secrets)

## Webhooks & Callbacks

**Incoming:**
- None

**Outgoing:**
- None

## Data Import/Export

**Import Formats:**
- JSON - Graph entity/edge data via `load_graph_from_path()`
- Native binary format - Snapshot restoration

**Export Formats:**
- JSON - Graph dumps via `dump_graph_to_path()`
- Native binary format - Snapshots via snapshot API

**CLI Commands:**
- `dump-graph` - Export to JSON
- `load-graph` - Import from JSON
- `snapshot-create` - Create binary snapshot
- `snapshot-load` - Restore from snapshot

## Benchmarking Integration

**Python Scripts:**
- `scripts/networkx_benchmark.py` - Comparison with NetworkX
- `scripts/quick_comparison_demo.py` - Quick benchmark demos
- `scripts/run_comparative_benchmarks.sh` - Shell runner
- `scripts/run_simple_comparative_benchmarks.py` - Simple comparison
- `scripts/sqlite_fts5_benchmark.py` - SQLite FTS5 comparison

**Output Formats:**
- JSON results stored in project root
- Criterion HTML reports in `target/criterion/`

---

*Integration audit: 2026-01-20*
