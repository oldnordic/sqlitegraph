# Requirements: SQLiteGraph v1.1 ACID & Reliability

**Defined:** 2026-01-20
**Milestone:** v1.1 ACID & Reliability
**Core Value:** Feature parity, performance, and reliability equally

## v1.1 Requirements

Requirements for ACID transaction correctness and resolution of all identified technical debt, security issues, and reliability concerns.

---

### ACID - Atomicity

Requirements for complete rollback of all operations.

- [ ] **ACID-01**: Node deletion captures before-image data (node record, all edges)
- [ ] **ACID-02**: Node deletion rollback restores node to previous state
- [ ] **ACID-03**: Node deletion rollback reclaims allocated slots
- [ ] **ACID-04**: Node deletion rollback restores all incoming and outgoing edges
- [ ] **ACID-05**: WAL recovery correctly handles IN_PROGRESS transactions (treat as ABORTED)
- [ ] **ACID-06**: Rollback operations are crash-safe (rollback state persisted to WAL)

### ACID - Consistency

Requirements for data integrity and validation.

- [x] **ACID-07**: Cluster overlap validation detects allocation corruption at runtime
- [x] **ACID-08**: Cluster overlap validation accounts for allocation sequencing timing
- [x] **ACID-09**: Checkpoint state invariants validation matches actual CheckpointState enum
- [x] **ACID-10**: Checkpoint state validation detects checkpoint corruption
- [x] **ACID-11**: Pre-commit validation checks database constraints
- [x] **ACID-12**: Post-recovery validation verifies database integrity

### ACID - Isolation

Requirements for concurrent access coordination.

- [ ] **ACID-13**: Transaction coordinator implements resource-level lock tracking
- [ ] **ACID-14**: Transaction coordinator builds wait-for graph for deadlock detection
- [ ] **ACID-15**: Transaction coordinator detects cycles in wait-for graph
- [ ] **ACID-16**: Transaction coordinator selects victim for abort (youngest transaction)
- [ ] **ACID-17**: Transaction isolation level API exists (ReadCommitted, RepeatableRead, Serializable)
- [ ] **ACID-18**: Concurrent write design document defines lock acquisition ordering

### ACID - Durability

Requirements for complete checkpoint strategies.

- [ ] **ACID-19**: Transaction-count checkpoint trigger fires after N transactions
- [ ] **ACID-20**: Size-based checkpoint trigger fires when WAL exceeds threshold
- [ ] **ACID-21**: WAL manager tracks transaction count since last checkpoint
- [ ] **ACID-22**: WAL manager tracks current WAL file size
- [ ] **ACID-23**: All three checkpoint strategies are tested and functional

---

### HNSW - Multi-Layer

Requirements for O(log N) HNSW search performance.

- [ ] **HNSW-01**: `determine_insertion_level()` uses exponential distribution with ml parameter
- [ ] **HNSW-02**: `determine_insertion_level()` respects max_layers configuration
- [ ] **HNSW-03**: HNSW index maintains separate graph layer for each level
- [ ] **HNSW-04**: HNSW insert adds node to all layers 0..=target_layer
- [ ] **HNSW-05**: HNSW search uses greedy descent through higher layers
- [ ] **HNSW-06**: HNSW search performs ef-search at layer 0
- [ ] **HNSW-07**: HNSW configuration includes M, ef_construction, ef_search, ml, max_layers
- [ ] **HNSW-08**: Multi-layer HNSW achieves O(log N) search complexity (verified by benchmarks)
- [ ] **HNSW-09**: Multi-layer HNSW maintains >95% recall vs exact nearest neighbor
- [ ] **HNSW-10**: HNSW persistence stores layer information per vector

---

### Checkpoint - Triggers

Requirements for functional checkpoint triggers.

- [ ] **CP-01**: Transaction-count checkpoint returns true when threshold exceeded
- [ ] **CP-02**: Size-based checkpoint returns true when WAL size exceeds threshold
- [ ] **CP-03**: Checkpoint triggers are configurable via NativeConfig
- [ ] **CP-04**: Checkpoint triggers reset their counters after checkpoint completes

---

### Checkpoint - Validation

Requirements for checkpoint integrity verification.

- [x] **CPV-01**: Checkpoint state validation code matches CheckpointState enum structure
- [x] **CPV-02**: Idle state validation passes for Idle variant
- [x] **CPV-03**: InProgress state validation verifies LSN and metadata file
- [x] **CPV-04**: Complete state validation verifies checkpoint file exists and LSN monotonicity
- [x] **CPV-05**: All checkpoint validation is enabled (not commented out)

---

### Schema - Format

Requirements for file format consistency.

- [ ] **SCHEMA-01**: Schema version field uses 4 bytes instead of 8 bytes
- [ ] **SCHEMA-02**: Schema version migration preserves backward compatibility
- [ ] **SCHEMA-03**: File format version bump documents schema change

---

### Unsafe - Audit

Requirements for eliminating unsafe lifetime transmutation.

- [ ] **UNSAFE-01**: All 10+ transmute sites documented with lifetime analysis
- [ ] **UNSAFE-02**: checkpoint/operations.rs transmute replaced with Arc<RwLock<GraphFile>>
- [ ] **UNSAFE-03**: checkpoint/record/integrator.rs transmute replaced with Arc<RwLock<GraphFile>>
- [ ] **UNSAFE-04**: recovery/replayer/rollback.rs transmute sites (6) replaced with Arc<RwLock<GraphFile>>
- [ ] **UNSAFE-05**: No unsafe transmute remains in codebase without documented justification
- [ ] **UNSAFE-06**: Miri tests validate safety of all former transmute sites
- [ ] **UNSAFE-07**: CI runs Miri tests on every commit

---

### Unsafe - Input Validation

Requirements for safe handling of external data.

- [ ] **INPUT-01**: JSON payloads limited to 10MB default size
- [ ] **INPUT-02**: JSON payloads limited to 128 levels depth
- [ ] **INPUT-03**: Input validation tests cover malicious payloads
- [ ] **INPUT-04**: Size/depth limits are configurable

---

### Refactoring - Large Files

Requirements for splitting files exceeding 600 LOC guidelines.

- [ ] **REFAC-01**: rollback.rs (1654 LOC) split into focused submodules by operation type
- [ ] **REFAC-02**: hnsw/index.rs (1605 LOC) split into modules (index, layer, search, insert)
- [ ] **REFAC-03**: checkpoint/operations.rs (1594 LOC) split into modules (checkpoint, flush, restore)
- [ ] **REFAC-04**: algo.rs (1398 LOC) split into modules (centrality, community, utility)
- [ ] **REFAC-05**: validator.rs (1300 LOC) split into modules (header, cluster, wal)
- [ ] **REFAC-06**: All split modules maintain test coverage
- [ ] **REFAC-07**: All split modules maintain documentation

---

### Refactoring - Clones

Requirements for reducing unnecessary clone operations.

- [ ] **CLONE-01**: All 263 clone() calls audited for necessity
- [ ] **CLONE-02**: Unnecessary clone() calls replaced with references
- [ ] **CLONE-03**: Clone audit documented with findings

---

### Refactoring - Connection Pooling

Requirements for SQLite backend concurrency.

- [ ] **POOL-01**: Connection pool implemented for SQLite backend
- [ ] **POOL-02**: Pool size is configurable
- [ ] **POOL-03**: Connection reuse reduces open/close overhead

---

### Features - Concurrent Writes

Requirements for multi-writer support.

- [ ] **CW-01**: Concurrent write design document defines architecture
- [ ] **CW-02**: Lock acquisition ordering prevents deadlocks
- [ ] **CW-03**: Multiple writers can commit transactions concurrently

---

### Features - Migration

Requirements for automated file format migration.

- [ ] **MIGRATE-01**: File migration API detects old format versions
- [ ] **MIGRATE-02**: File migration API converts to current format
- [ ] **MIGRATE-03**: Migration is atomic (write to new file, replace old)
- [ ] **MIGRATE-04**: Migration can be rolled back

---

### Features - Backup/Restore

Requirements for high-level snapshot API.

- [ ] **BACKUP-01**: Backup API creates consistent snapshot of database
- [ ] **BACKUP-02**: Restore API loads snapshot and verifies integrity
- [ ] **BACKUP-03**: Snapshot includes all data pages and WAL position

---

### Testing - WAL Recovery

Requirements for WAL recovery test coverage.

- [ ] **TEST-WAL-01**: Node deletion rollback test passes (currently stubbed/TODO)
- [ ] **TEST-WAL-02**: Crash simulation tests cover each WAL operation type
- [ ] **TEST-WAL-03**: Recovery tests verify database state after crash
- [ ] **TEST-WAL-04**: All 8 "will fail until implementation complete" tests pass

---

### Testing - Cluster Validation

Requirements for cluster allocation integrity tests.

- [ ] **TEST-CLUS-01**: Cluster overlap validation tests are enabled (not commented out)
- [ ] **TEST-CLUS-02**: Cluster overlap validation detects artificially corrupted clusters
- [ ] **TEST-CLUS-03**: Cluster overlap validation timing issues are resolved

---

### Testing - Checkpoint

Requirements for checkpoint integrity tests.

- [ ] **TEST-CP-01**: Checkpoint state invariants tests are enabled
- [ ] **TEST-CP-02**: Checkpoint state validation detects corrupted checkpoints
- [ ] **TEST-CP-03**: All checkpoint strategies have test coverage

---

### Testing - HNSW

Requirements for multi-layer HNSW tests.

- [ ] **TEST-HNSW-01**: Layer distribution test verifies exponential distribution
- [ ] **TEST-HNSW-02**: Multi-layer insert test verifies nodes in correct layers
- [ ] **TEST-HNSW-03**: Multi-layer search test verifies correctness vs layer 0
- [ ] **TEST-HNSW-04**: Search complexity benchmark demonstrates O(log N)

---

### Testing - Miri

Requirements for unsafe block validation.

- [ ] **TEST-MIRI-01**: Miri is configured for the project
- [ ] **TEST-MIRI-02**: All former transmute sites have Miri tests
- [ ] **TEST-MIRI-03**: CI runs Miri tests on every commit
- [ ] **TEST-MIRI-04**: No Miri errors in test suite

---

### Scaling - Checkpoint File

Requirements for large database checkpoint handling.

- [ ] **SCALE-CP-01**: Checkpoint supports files larger than 1GB
- [ ] **SCALE-CP-02**: Multi-file checkpointing or streaming checkpoint implemented
- [ ] **SCALE-CP-03**: Large checkpoint tests verify correctness

---

### Scaling - Dirty Blocks

Requirements for tracking overflow handling.

- [ ] **SCALE-DB-01**: Dirty block tracking overflow strategy implemented
- [ ] **SCALE-DB-02**: Hierarchical tracking supports >50,000 global dirty blocks
- [ ] **SCALE-DB-03**: Overflow handling tests verify correctness

---

### Scaling - Transaction Coordinator

Requirements for unbounded growth prevention.

- [ ] **SCALE-TX-01**: Transaction ID bounds are enforced
- [ ] **SCALE-TX-02**: Deadlock detection wait-for graph has cleanup
- [ ] **SCALE-TX-03**: Transaction coordinator tests verify cleanup

---

### Scaling - HNSW Size

Requirements for memory-limited HNSW indexes.

- [ ] **SCALE-HNSW-01**: Disk-based HNSW option for indexes larger than RAM
- [ ] **SCALE-HNSW-02**: HNSW can spill to disk when memory threshold reached

---

### Dependencies - rusqlite

Requirements for SQLite dependency management.

- [ ] **DEP-RUST-01**: rusqlite 0.31 monitored for security updates
- [ ] **DEP-RUST-02**: System SQLite option evaluated for security patches

---

### Dependencies - bincode

Requirements for serialization upgrade planning.

- [ ] **DEP-BIN-01**: bincode 2.0 migration plan documented
- [ ] **DEP-BIN-02**: bincode 2.0 migration preserves existing data (format version bump)

---

## v2 Requirements

Deferred to future milestone (v1.2 or beyond).

(None currently - all CONCERNS.md items included in v1.1)

---

## Out of Scope

Explicitly excluded from this milestone.

| Feature | Reason |
|---------|--------|
| Breaking API changes | Must maintain backward compatibility with existing databases and APIs |
| New external integrations | Focus remains on embedded standalone database |
| Web services or network protocol | In-process embedded database only |
| Alternative storage backends | SQLite and Native V2 only |

---

## Traceability

Requirements mapped to phases in roadmap.

| Requirement | Phase | Status |
|-------------|-------|--------|
| ACID-01 | Phase 11 | Complete |
| ACID-02 | Phase 11 | Complete |
| ACID-03 | Phase 11 | Complete |
| ACID-04 | Phase 11 | Complete |
| ACID-05 | Phase 11 | Complete |
| ACID-06 | Phase 11 | Complete |
| ACID-07 | Phase 12 | Complete |
| ACID-08 | Phase 12 | Complete |
| ACID-09 | Phase 12 | Complete |
| ACID-10 | Phase 12 | Complete |
| ACID-11 | Phase 12 | Complete |
| ACID-12 | Phase 12 | Complete |
| ACID-13 | Phase 13 | Pending |
| ACID-14 | Phase 13 | Pending |
| ACID-15 | Phase 13 | Pending |
| ACID-16 | Phase 13 | Pending |
| ACID-17 | Phase 13 | Pending |
| ACID-18 | Phase 13 | Pending |
| ACID-19 | Phase 14 | Pending |
| ACID-20 | Phase 14 | Pending |
| ACID-21 | Phase 14 | Pending |
| ACID-22 | Phase 14 | Pending |
| ACID-23 | Phase 14 | Pending |
| HNSW-01 | Phase 15 | Pending |
| HNSW-02 | Phase 15 | Pending |
| HNSW-03 | Phase 15 | Pending |
| HNSW-04 | Phase 15 | Pending |
| HNSW-05 | Phase 15 | Pending |
| HNSW-06 | Phase 15 | Pending |
| HNSW-07 | Phase 15 | Pending |
| HNSW-08 | Phase 15 | Pending |
| HNSW-09 | Phase 15 | Pending |
| HNSW-10 | Phase 15 | Pending |
| CP-01 | Phase 14 | Pending |
| CP-02 | Phase 14 | Pending |
| CP-03 | Phase 14 | Pending |
| CP-04 | Phase 14 | Pending |
| CPV-01 | Phase 12 | Complete |
| CPV-02 | Phase 12 | Complete |
| CPV-03 | Phase 12 | Complete |
| CPV-04 | Phase 12 | Complete |
| CPV-05 | Phase 12 | Complete |
| SCHEMA-01 | Phase 20 | Pending |
| SCHEMA-02 | Phase 20 | Pending |
| SCHEMA-03 | Phase 20 | Pending |
| UNSAFE-01 | Phase 16 | Pending |
| UNSAFE-02 | Phase 16 | Pending |
| UNSAFE-03 | Phase 16 | Pending |
| UNSAFE-04 | Phase 16 | Pending |
| UNSAFE-05 | Phase 16 | Pending |
| UNSAFE-06 | Phase 16 | Pending |
| UNSAFE-07 | Phase 16 | Pending |
| INPUT-01 | Phase 16 | Pending |
| INPUT-02 | Phase 16 | Pending |
| INPUT-03 | Phase 17 | Pending |
| INPUT-04 | Phase 17 | Pending |
| REFAC-01 | Phase 18 | Pending |
| REFAC-02 | Phase 18 | Pending |
| REFAC-03 | Phase 18 | Pending |
| REFAC-04 | Phase 18 | Pending |
| REFAC-05 | Phase 18 | Pending |
| REFAC-06 | Phase 18 | Pending |
| REFAC-07 | Phase 18 | Pending |
| CLONE-01 | Phase 18 | Pending |
| CLONE-02 | Phase 18 | Pending |
| CLONE-03 | Phase 18 | Pending |
| POOL-01 | Phase 19 | Pending |
| POOL-02 | Phase 19 | Pending |
| POOL-03 | Phase 19 | Pending |
| CW-01 | Phase 13 | Pending |
| CW-02 | Phase 13 | Pending |
| CW-03 | Phase 13 | Pending |
| MIGRATE-01 | Phase 20 | Pending |
| MIGRATE-02 | Phase 20 | Pending |
| MIGRATE-03 | Phase 20 | Pending |
| MIGRATE-04 | Phase 20 | Pending |
| BACKUP-01 | Phase 20 | Pending |
| BACKUP-02 | Phase 20 | Pending |
| BACKUP-03 | Phase 20 | Pending |
| TEST-WAL-01 | Phase 21 | Pending |
| TEST-WAL-02 | Phase 21 | Pending |
| TEST-WAL-03 | Phase 21 | Pending |
| TEST-WAL-04 | Phase 21 | Pending |
| TEST-CLUS-01 | Phase 21 | Pending |
| TEST-CLUS-02 | Phase 21 | Pending |
| TEST-CLUS-03 | Phase 21 | Pending |
| TEST-CP-01 | Phase 21 | Pending |
| TEST-CP-02 | Phase 21 | Pending |
| TEST-CP-03 | Phase 21 | Pending |
| TEST-HNSW-01 | Phase 21 | Pending |
| TEST-HNSW-02 | Phase 21 | Pending |
| TEST-HNSW-03 | Phase 21 | Pending |
| TEST-HNSW-04 | Phase 21 | Pending |
| TEST-MIRI-01 | Phase 21 | Pending |
| TEST-MIRI-02 | Phase 21 | Pending |
| TEST-MIRI-03 | Phase 21 | Pending |
| TEST-MIRI-04 | Phase 21 | Pending |
| SCALE-CP-01 | Phase 22 | Pending |
| SCALE-CP-02 | Phase 22 | Pending |
| SCALE-CP-03 | Phase 22 | Pending |
| SCALE-DB-01 | Phase 22 | Pending |
| SCALE-DB-02 | Phase 22 | Pending |
| SCALE-DB-03 | Phase 22 | Pending |
| SCALE-TX-01 | Phase 22 | Pending |
| SCALE-TX-02 | Phase 22 | Pending |
| SCALE-TX-03 | Phase 22 | Pending |
| SCALE-HNSW-01 | Phase 22 | Pending |
| SCALE-HNSW-02 | Phase 22 | Pending |
| DEP-RUST-01 | Phase 22 | Pending |
| DEP-RUST-02 | Phase 22 | Pending |
| DEP-BIN-01 | Phase 22 | Pending |
| DEP-BIN-02 | Phase 22 | Pending |

**Summary:**
- v1.1 requirements: 78 total
- Coverage: 78/78 mapped (100%)
- Phase 11 (Atomicity): 6 requirements
- Phase 12 (Consistency): 10 requirements
- Phase 13 (Isolation): 9 requirements
- Phase 14 (Durability): 9 requirements
- Phase 15 (HNSW): 10 requirements
- Phase 16 (Memory Safety): 11 requirements
- Phase 17 (Input Validation): 2 requirements
- Phase 18 (Code Structure): 10 requirements
- Phase 19 (Concurrent Features): 3 requirements
- Phase 20 (Data Management): 10 requirements
- Phase 21 (Test Coverage): 16 requirements
- Phase 22 (Scaling & Dependencies): 14 requirements

---
*Requirements defined: 2026-01-20*
*Last updated: 2026-01-20 after roadmap creation*
