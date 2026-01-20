# SQLiteGraph

## What This Is

SQLiteGraph is an embedded graph database library in Rust with dual backend support (SQLite and Native V2), HNSW vector search, and CLI interface. It provides high-performance graph operations with MVCC concurrency, WAL-based durability, and vector similarity search capabilities.

## Core Value

**Feature parity, performance, and reliability equally.** Native V2 must match or exceed SQLite backend capabilities while maintaining rock-solid MVCC correctness and achieving best-in-class embedded graph database performance.

## Current Milestone: v1.1 ACID & Reliability

**Goal:** Complete ACID transaction correctness for Native V2 backend and resolve all identified technical debt, security issues, and reliability concerns.

**Target capabilities:**
- Full ACID guarantees: Atomicity, Consistency, Isolation, Durability
- WAL recovery completeness (including node deletion rollback)
- Memory safety verification (eliminate/reduce unsafe transmute, add miri tests)
- Checkpoint system completion (all strategies functional, validation enabled)
- Concurrent write support with proper deadlock detection
- HNSW multi-layer implementation for O(log N) search
- Cluster allocation validation and corruption detection
- File refactoring (split large modules into focused submodules)

### Validated

*v1.0 Production capabilities (shipped 2026-01-17):*

- ✓ **Dual backend architecture** — GraphBackend trait with SQLite and Native V2 implementations
- ✓ **Core graph operations** — insert_node, get_node, neighbors, insert_edge, bfs
- ✓ **MVCC snapshot system** — ArcSwap<Arc<SnapshotState>> for concurrent reads
- ✓ **WAL for Native V2** — Write-Ahead Logging with recovery/replayer modules
- ✓ **HNSW vector search** — Hierarchical navigable small world index with persistence
- ✓ **Graph algorithms** — BFS, k-hop, shortest path, PageRank, Betweenness, Louvain, Label Propagation
- ✓ **CLI interface** — Command-line tool with backend selection
- ✓ **Configuration system** — Builder pattern for GraphConfig, SqliteConfig, NativeConfig
- ✓ **Error handling** — thiserror-based SqliteGraphError with comprehensive variants
- ✓ **Introspection APIs** — GraphIntrospection for LLM tooling
- ✓ **Progress tracking** — ProgressCallback for long-running operations

### Active

*v1.1 ACID & Reliability goals:*

**ACID Transaction Guarantees:**
- [ ] **Atomicity** — Complete rollback for all operations (including node deletion)
- [ ] **Consistency** — Cluster overlap validation, checkpoint state validation, constraint enforcement
- [ ] **Isolation** — Concurrent write support, proper transaction isolation levels, deadlock detection
- [ ] **Durability** — All checkpoint strategies functional, WAL replay completeness

**Technical Debt (6 items):**
- [ ] **HNSW multi-layer** — Implement `determine_insertion_level()` with exponential distribution
- [ ] **Checkpoint strategies** — Wire up transaction-count and size-based triggers
- [ ] **Node deletion WAL replay** — Implement rollback data capture and slot reclamation
- [ ] **Cluster overlap validation** — Re-enable with proper sequencing support
- [ ] **Checkpoint validation** — Fix invariants validation to match CheckpointState enum
- [ ] **Schema version size** — Migrate to 4-byte field with format version bump

**Security & Safety (3 items):**
- [ ] **Unsafe transmute audit** — Audit all 10+ sites, add miri tests, replace with Arc<RwLock<GraphFile>> where appropriate
- [ ] **Deadlock detection** — Implement resource-level deadlock detection in transaction coordinator
- [ ] **Input sanitization** — Add size/depth limits for JSON payloads

**Performance & Structure (7 items):**
- [ ] **Large file refactoring** — Split rollback.rs (1654 LOC), hnsw/index.rs (1605 LOC), checkpoint/operations.rs (1594 LOC), algo.rs (1398 LOC), validator.rs (1300 LOC)
- [ ] **Clone operations** — Audit 263 clone() calls, reduce unnecessary clones
- [ ] **Connection pooling** — Implement for SQLite backend concurrency

**Missing Features (4 items):**
- [ ] **Concurrent write support** — Multi-writer scenarios with proper coordination
- [ ] **Graph file migration** — Automated migration between storage format versions
- [ ] **Backup/Restore API** — High-level API for V2 backend snapshots
- [ ] **Node deletion WAL replay** — Complete crash recovery consistency

**Test Coverage (5 items):**
- [ ] **WAL edge cases** — Complete node deletion rollback scenarios
- [ ] **Cluster overlap validation** — Re-enable commented validation tests
- [ ] **Checkpoint transitions** — Fix and enable commented invariants validation
- [ ] **HNSW multi-layer** — Tests for multi-layer insertion and search
- [ ] **Unsafe blocks** — Miri testing for all transmute sites

**Scaling Limits (4 items):**
- [ ] **Checkpoint file size** — Multi-file checkpointing or streaming
- [ ] **Dirty block tracking** — Overflow strategy or hierarchical tracking
- [ ] **WAL transaction coordinator** — Transaction ID bounds and cleanup verification
- [ ] **HNSW index size** — Disk-based HNSW for large indexes

**Dependencies (2 items):**
- [ ] **rusqlite 0.31** — Monitor updates, consider system SQLite for security
- [ ] **bincode 1.3** — Plan migration to bincode 2.0 with format version bump

### Out of Scope

- **Breaking API changes** — Must maintain backward compatibility with existing databases and APIs
- **New external integrations** — Focus remains on embedded standalone database
- **Web services or network protocol** — In-process embedded database only
- **Alternative storage backends** — SQLite and Native V2 only

## Context

**Current codebase state (v1.0 complete):**

- **Architecture:** Layered design with Public API → Backend Abstraction → Storage (SQLite/Native) → Infrastructure
- **File organization:** Modular structure in `sqlitegraph/src/` with backend/, graph/, hnsw/, pattern_engine/, config/
- **v1.0 achievements:**
  - Native V2 backend with clustered adjacency and LRU-K caching
  - HNSW vector search with disk persistence
  - 4 production graph algorithms (PageRank, Betweenness, Louvain, Label Propagation)
  - Introspection APIs and progress tracking
  - CLI debug commands
  - 65 MVCC tests, 134 HNSW tests, comprehensive benchmarks

**Identified concerns (from codebase map):**
- 32 items across tech debt, security, performance, fragile areas, scaling limits, dependencies, missing features, test gaps
- Highest priority: Data integrity risks (node deletion WAL replay, cluster validation disabled), memory safety (unsafe transmute), ACID incompleteness

**Technical environment:**
- Rust 2024 (library) / Rust 2021 (CLI), MSRV 1.70.0
- Dependencies: rusqlite 0.31, thiserror, serde, parking_lot, memmap2, criterion for benchmarks
- Cross-platform: Linux, macOS, Windows
- No external services or cloud dependencies

## Constraints

- **Backward compatibility:** Must maintain compatibility with existing databases and APIs — no breaking changes to file formats or public interfaces
- **Timeline:** No deadline — systematic completion until everything is done
- **Performance:** Must meet or exceed current performance characteristics for all operations
- **Safety:** Eliminate or justify all unsafe Rust; add miri testing for remaining unsafe blocks
- **Testing:** All code paths must have test coverage per CONTRIBUTING.md

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Dual backend architecture | Leverages SQLite's maturity while enabling high-performance native path | — Pending |
| MVCC with ArcSwap | Lock-free snapshot updates for concurrent readers | — Pending |
| In-memory HNSW → Disk persistence | Simple design evolved to include persistence | ⚠️ Revisit (multi-layer needed) |
| WAL for Native V2 | Enables durability and crash recovery | — Pending (ACID incomplete) |
| No breaking changes | Existing users and databases must continue working | ✓ Enforced |
| Split large files | Reduces fragility, improves maintainability, increases confidence | — Pending (v1.1) |

---
*Last updated: 2026-01-20 after v1.1 ACID & Reliability milestone initialization*
