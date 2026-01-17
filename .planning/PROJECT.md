# SQLiteGraph

## What This Is

SQLiteGraph is an embedded graph database library in Rust with dual backend support (SQLite and Native V2), HNSW vector search, and CLI interface. It provides high-performance graph operations with MVCC concurrency, WAL-based durability, and vector similarity search capabilities.

## Core Value

**Feature parity, performance, and reliability equally.** Native V2 must match or exceed SQLite backend capabilities while maintaining rock-solid MVCC correctness and achieving best-in-class embedded graph database performance.

## Requirements

### Validated

*Existing capabilities from the current codebase:*

- ✓ **Dual backend architecture** — GraphBackend trait with SQLite and Native V2 implementations
- ✓ **Core graph operations** — insert_node, get_node, neighbors, insert_edge, bfs
- ✓ **MVCC snapshot system** — ArcSwap<Arc<SnapshotState>> for concurrent reads
- ✓ **WAL for Native V2** — Write-Ahead Logging with recovery/replayer modules
- ✓ **HNSW vector search** — In-memory hierarchical navigable small world index with multiple distance metrics
- ✓ **Graph algorithms** — BFS, k-hop, shortest path, pattern engine
- ✓ **CLI interface** — Command-line tool with backend selection
- ✓ **Configuration system** — Builder pattern for GraphConfig, SqliteConfig, NativeConfig
- ✓ **Error handling** — thiserror-based SqliteGraphError with comprehensive variants

### Active

*Completion and improvement goals for this project:*

- [ ] **Native V2 read completion** — Implement betree and read path optimizations (currently missing)
- [ ] **MVCC gap fixes** — Address identified MVCC limitations and edge cases
- [ ] **Performance optimization** — WAL recovery, lock contention, HNSW memory efficiency
- [ ] **WAL integration completion** — Wire placeholder validator/replayer functions, enable automatic checkpointing
- [ ] **HNSW persistence** — Enable index save/restore to disk (currently in-memory only)
- [ ] **HNSW CLI persistence** — Fix indexes lost across CLI invocations
- [ ] **Advanced graph algorithms** — Centrality measures, community detection
- [ ] **Developer tooling** — Debugging, profiling, and introspection utilities

### Out of Scope

- **Breaking API changes** — Must maintain backward compatibility with existing databases and APIs
- **New external integrations** — Focus remains on embedded standalone database
- **Web services or network protocol** — In-process embedded database only
- **Alternative storage backends** — SQLite and Native V2 only

## Context

**Current codebase state:**

- **Architecture:** Layered design with Public API → Backend Abstraction → Storage (SQLite/Native) → Infrastructure
- **File organization:** Modular structure in `sqlitegraph/src/` with backend/, graph/, hnsw/, pattern_engine/, config/
- **Tech debt identified:**
  - Large WAL recovery files (4,113 lines in operations.rs)
  - Unused imports and debug scaffolding
  - Missing module documentation (~1,093 files)
- **Known limitations:**
  - HNSW indexes don't persist across CLI commands
  - WAL recovery is single-threaded and sequential
  - Lock contention potential from heavy Arc<Mutex<>>/Arc<RwLock<>> usage
  - No automatic WAL checkpointing

**Technical environment:**
- Rust 2024 (library) / Rust 2021 (CLI), MSRV 1.70.0
- Dependencies: rusqlite 0.31, thiserror, serde, parking_lot, memmap2, criterion for benchmarks
- Cross-platform: Linux, macOS, Windows
- No external services or cloud dependencies

## Constraints

- **Backward compatibility:** Must maintain compatibility with existing databases and APIs — no breaking changes to file formats or public interfaces
- **Timeline:** No deadline — systematic completion until everything is done
- **Performance:** Must meet or exceed current performance characteristics for all operations
- **Safety:** No unsafe Rust in production paths (current standard)
- **Testing:** All code paths must have test coverage per CONTRIBUTING.md

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Dual backend architecture | Leverages SQLite's maturity while enabling high-performance native path | — Pending |
| MVCC with ArcSwap | Lock-free snapshot updates for concurrent readers | — Pending |
| In-memory HNSW | Simple design, limits index size to RAM | ⚠️ Revisit (persistence needed) |
| WAL for Native V2 | Enables durability and crash recovery | — Pending |
| No breaking changes | Existing users and databases must continue working | ✓ Enforced |

---
*Last updated: 2026-01-17 after initialization*
