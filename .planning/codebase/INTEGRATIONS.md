# External Integrations

**Analysis Date:** 2026-01-20

## APIs & External Services

**None** - This is an embedded, self-contained database library.

The project provides database functionality rather than consuming external APIs:
- No REST API clients
- No cloud service integrations
- No third-party API dependencies

## Data Storage

**Databases:**
- **SQLite** (via rusqlite 0.31)
  - Connection: File path or `:memory:`
  - Client: rusqlite with bundled libsqlite3
  - Usage: Default backend for ACID transactions

**Native Storage:**
- **Custom Binary Format** (Native V2 backend)
  - Files: `*.db`, `*.db-wal`, `*.db-shm`
  - Format: Clustered adjacency with WAL
  - Client: Pure Rust implementation (`src/backend/native/`)

**File Storage:**
- Local filesystem only
- Memory-mapped I/O via memmap2
- Atomic file operations for crash safety

**Caching:**
- LRU-K adjacency cache (`src/cache.rs`)
- Query result cache (`src/query_cache.rs`)
- In-memory only (no external cache)

## Authentication & Identity

**Auth Provider:**
- None (embedded database with no network access)

**Implementation:**
- No authentication layer
- No user management
- Direct file-based access control via OS permissions

## Monitoring & Observability

**Error Tracking:**
- None (no external error tracking service)

**Logs:**
- log crate facade (0.4) - structured logging interface
- Console output via debug commands
- No log aggregation service

**Introspection:**
- `GraphIntrospection` trait for stats (`src/introspection.rs`)
- Cache statistics, edge counts, file sizes
- Debug CLI commands: `debug-stats`, `debug-dump`, `debug-trace`

**Progress Tracking:**
- `ProgressCallback` trait (`src/progress.rs`)
- `ConsoleProgress` for terminal progress bars
- Algorithm progress reporting (PageRank, Louvain, etc.)

## CI/CD & Deployment

**Hosting:**
- crates.io: https://crates.io/crates/sqlitegraph
- GitHub: https://github.com/oldnordic/sqlitegraph
- Docs.rs: https://docs.rs/sqlitegraph

**CI Pipeline:**
- No detected CI configuration (.github, .gitlab-ci.yml, etc.)
- Manual testing via `cargo test --workspace`

**Deployment:**
- Static binary compilation via `cargo build --release`
- No containerization detected
- No cloud deployment configuration

## Environment Configuration

**Required env vars:**
- None for core operation

**Optional env vars:**
- None (configuration via `GraphConfig` struct)

**Secrets location:**
- Not applicable (no authentication/external services)

## Webhooks & Callbacks

**Incoming:**
- None (no HTTP server)

**Outgoing:**
- None (no HTTP client)

**Internal Callbacks:**
- `ProgressCallback` - Long-running algorithm progress
- `WALManagerMetrics` - WAL operation metrics
- Cache eviction callbacks

## Algorithm Libraries

**Graph Algorithms (Internal):**
- PageRank - `src/algo.rs`
- Betweenness Centrality - `src/algo.rs`
- Label Propagation - `src/algo.rs`
- Louvain Method - `src/algo.rs`
- BFS, k-hop, shortest path - `src/bfs.rs`, `src/multi_hop.rs`

**Vector Search (Internal):**
- HNSW (Hierarchical Navigable Small World) - `src/hnsw/`
- Distance metrics: Cosine, Euclidean, Dot Product, Manhattan
- Pluggable vector storage (in-memory or SQLite)

## Testing Infrastructure

**Test Framework:**
- Rust built-in (`cargo test`)
- Criterion for benchmarks (`cargo bench`)

**Test Utilities:**
- tempfile - Temporary test databases
- assert_cmd - CLI testing
- rand - Test data generation

**Coverage:**
- No tarpaulin or coverage tool configured
- Manual test coverage tracking via test modules

## Developer Tools

**Binary Format Tools:**
- binrw derive macros for serialization
- bytemuck for unsafe byte casting

**Debugging:**
- CLI debug commands
- Introspection API
- Pattern engine tracing

---

*Integration audit: 2026-01-20*
