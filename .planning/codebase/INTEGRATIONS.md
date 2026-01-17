# External Integrations

**Analysis Date:** 2026-01-17

## APIs & External Services

**Payment Processing:**
- Not applicable (embedded database, no payment processing)

**Email/SMS:**
- Not applicable (no notification features)

**External APIs:**
- None - Standalone embedded database with no external API calls

## Data Storage

**Databases:**
- SQLite (rusqlite 0.31 with bundled sqlite3-sys)
  - Connection: In-process via rusqlite library
  - Client: rusqlite crate
  - Migrations: Handled via native backend upgrade logic

**Native V2 Storage:**
- Custom binary format with WAL
  - Location: sqlitegraph/src/backend/native/v2/
  - Features: 10-20x performance improvement, 70%+ storage efficiency
  - Cross-platform atomic operations

**File Storage:**
- Local filesystem only
  - No external file storage services
  - Memory-mapped file I/O via memmap2

**Caching:**
- In-memory only (no Redis or external cache)
  - HNSW indexes stored in memory
  - arc-swap for atomic reference swapping

## Authentication & Identity

**Auth Provider:**
- None (embedded database, no authentication)

**OAuth Integrations:**
- Not applicable

## Monitoring & Observability

**Error Tracking:**
- None (standard Rust error handling with thiserror)

**Analytics:**
- None

**Logs:**
- Basic log support (log 0.4 dependency)
  - No structured logging framework
  - Debug prints gated behind feature flags (trace_v2_io)

## CI/CD & Deployment

**Hosting:**
- Not applicable (library/CLI distribution)

**CI Pipeline:**
- No GitHub Actions detected
- Manual testing and benchmarking

**Distribution:**
- crates.io for Rust package distribution
- Binary releases via GitHub (implied)

## Environment Configuration

**Development:**
- Required env vars: None
- Configuration: Code-based via GraphConfig
- No mock/stub services

**Staging:**
- Not applicable (no staging environment)

**Production:**
- Secrets management: Not applicable (no secrets)
- Configuration: Compile-time feature flags

## Webhooks & Callbacks

**Incoming:**
- None

**Outgoing:**
- None

## Feature Flags

**Development Features:**
- `sqlite-backend` (default) - SQLite storage backend
- `native-v2` - High-performance native backend with WAL
- `bench-ci` - CI benchmarking
- `trace_v2_io` - Debug features for V2 I/O operations

## Third-Party Libraries Summary

**Core Dependencies:**
| Library | Version | Purpose |
|---------|---------|---------|
| rusqlite | 0.31 | SQLite database interface |
| serde | 1 | Serialization framework |
| serde_json | 1 | JSON serialization |
| thiserror | 1 | Error handling |
| parking_lot | 0.12 | Efficient synchronization primitives |
| ahash | 0.8 | Fast hashing |
| memmap2 | 0.9 | Memory-mapped file I/O |
| binrw | 0.13 | Binary format parsing |
| bytemuck | 1.13 | Safe byte operations |
| arc-swap | 1 | Atomic reference swapping |
| rand | 0.8 | Random number generation |

**CLI-Specific:**
| Library | Version | Purpose |
|---------|---------|---------|
| clap | 4 | Command-line interface |

**Testing:**
| Library | Version | Purpose |
|---------|---------|---------|
| criterion | 0.5 | Benchmarking |
| assert_cmd | 2 | CLI testing |
| tempfile | 3 | Temporary file handling |

---

*Integration audit: 2026-01-17*
*Update when adding/removing external services*
