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

- [x] **ACID-13**: Transaction coordinator implements resource-level lock tracking
- [x] **ACID-14**: Transaction coordinator builds wait-for graph for deadlock detection
- [x] **ACID-15**: Transaction coordinator detects cycles in wait-for graph
- [x] **ACID-16**: Transaction coordinator selects victim for abort (youngest transaction)
- [x] **ACID-17**: Transaction isolation level API exists (ReadCommitted, RepeatableRead, Serializable)
- [x] **ACID-18**: Concurrent write design document defines lock acquisition ordering

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

- [x] **HNSW-01**: `determine_insertion_level()` uses exponential distribution with ml parameter
- [x] **HNSW-02**: `determine_insertion_level()` respects max_layers configuration
- [x] **HNSW-03**: HNSW index maintains separate graph layer for each level
- [x] **HNSW-04**: HNSW insert adds node to all layers 0..=target_layer
- [x] **HNSW-05**: HNSW search uses greedy descent through higher layers
- [x] **HNSW-06**: HNSW search performs ef-search at layer 0
- [x] **HNSW-07**: HNSW configuration includes M, ef_construction, ef_search, ml, max_layers
- [x] **HNSW-08**: Multi-layer HNSW achieves O(log N) search complexity (verified by benchmarks)
- [x] **HNSW-09**: Multi-layer HNSW maintains >95% recall vs exact nearest neighbor (achieved 100%)
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

- [x] **SCHEMA-01**: Schema version field uses 4 bytes instead of 8 bytes
- [x] **SCHEMA-02**: Schema version migration preserves backward compatibility
- [x] **SCHEMA-03**: File format version bump documents schema change

---

### Unsafe - Audit

Requirements for eliminating unsafe lifetime transmutation.

- [x] **UNSAFE-01**: All 19 transmute sites documented with lifetime analysis
- [x] **UNSAFE-02**: checkpoint/operations.rs transmute replaced with Arc<RwLock<GraphFile>>
- [x] **UNSAFE-03**: checkpoint/record/integrator.rs transmute replaced with Arc<RwLock<GraphFile>>
- [x] **UNSAFE-04**: replayer transmute sites (13) replaced with Arc<RwLock<GraphFile>>
- [x] **UNSAFE-05**: No unsafe transmute remains in codebase without documented justification
- [x] **UNSAFE-06**: Miri tests validate safety of all former transmute sites
- [x] **UNSAFE-07**: CI runs Miri tests on every commit

---

### Unsafe - Input Validation

Requirements for safe handling of external data.

- [x] **INPUT-01**: JSON payloads limited to 10MB default size
- [x] **INPUT-02**: JSON payloads limited to 128 levels depth
- [x] **INPUT-03**: Input validation tests cover malicious payloads
- [x] **INPUT-04**: Size/depth limits are configurable

---

### Refactoring - Large Files

Requirements for splitting files exceeding 600 LOC guidelines.

- [x] **REFAC-01**: rollback.rs (1654 LOC) split into focused submodules by operation type
- [x] **REFAC-02**: hnsw/index.rs (1605 LOC) split into modules (index, layer, search, insert)
- [x] **REFAC-03**: checkpoint/operations.rs (1594 LOC) split into modules (checkpoint, flush, restore)
- [x] **REFAC-04**: algo.rs (1398 LOC) split into modules (centrality, community, utility)
- [x] **REFAC-05**: validator.rs (1300 LOC) split into modules (header, cluster, wal)
- [x] **REFAC-06**: All split modules maintain test coverage
- [x] **REFAC-07**: All split modules maintain documentation

---

### Refactoring - Clones

Requirements for reducing unnecessary clone operations.

- [x] **CLONE-01**: All 263 clone() calls audited for necessity
- [x] **CLONE-02**: Unnecessary clone() calls replaced with references
- [x] **CLONE-03**: Clone audit documented with findings

---

### Refactoring - Connection Pooling

Requirements for SQLite backend concurrency.

- [x] **POOL-01**: Connection pool implemented for SQLite backend
- [x] **POOL-02**: Pool size is configurable
- [x] **POOL-03**: Connection reuse reduces open/close overhead

---

### Features - Concurrent Writes

Requirements for multi-writer support.

- [ ] **CW-01**: Concurrent write design document defines architecture
- [ ] **CW-02**: Lock acquisition ordering prevents deadlocks
- [ ] **CW-03**: Multiple writers can commit transactions concurrently

---

### Features - Migration

Requirements for automated file format migration.

- [x] **MIGRATE-01**: File migration API detects old format versions
- [x] **MIGRATE-02**: File migration API converts to current format
- [x] **MIGRATE-03**: Migration is atomic (write to new file, replace old)
- [x] **MIGRATE-04**: Migration can be rolled back

---

### Features - Backup/Restore

Requirements for high-level snapshot API.

- [x] **BACKUP-01**: Backup API creates consistent snapshot of database
- [x] **BACKUP-02**: Restore API loads snapshot and verifies integrity
- [x] **BACKUP-03**: Snapshot includes all data pages and WAL position

---

### Testing - WAL Recovery

Requirements for WAL recovery test coverage.

- [x] **TEST-WAL-01**: Node deletion rollback test passes (currently stubbed/TODO)
- [x] **TEST-WAL-02**: Crash simulation tests cover each WAL operation type
- [x] **TEST-WAL-03**: Recovery tests verify database state after crash
- [x] **TEST-WAL-04**: All 8 "will fail until implementation complete" tests pass

---

### Testing - Cluster Validation

Requirements for cluster allocation integrity tests.

- [x] **TEST-CLUS-01**: Cluster overlap validation tests are enabled (not commented out)
- [x] **TEST-CLUS-02**: Cluster overlap validation detects artificially corrupted clusters
- [x] **TEST-CLUS-03**: Cluster overlap validation timing issues are resolved

---

### Testing - Checkpoint

Requirements for checkpoint integrity tests.

- [x] **TEST-CP-01**: Checkpoint state invariants tests are enabled
- [x] **TEST-CP-02**: Checkpoint state validation detects corrupted checkpoints
- [x] **TEST-CP-03**: All checkpoint strategies have test coverage

---

### Testing - HNSW

Requirements for multi-layer HNSW tests.

- [x] **TEST-HNSW-01**: Layer distribution test verifies exponential distribution
- [x] **TEST-HNSW-02**: Multi-layer insert test verifies nodes in correct layers
- [x] **TEST-HNSW-03**: Multi-layer search test verifies correctness vs layer 0
- [x] **TEST-HNSW-04**: Search complexity benchmark demonstrates O(log N)

---

### Testing - Miri

Requirements for unsafe block validation.

- [x] **TEST-MIRI-01**: Miri is configured for the project
- [x] **TEST-MIRI-02**: All former transmute sites have Miri tests
- [x] **TEST-MIRI-03**: CI runs Miri tests on every commit
- [x] **TEST-MIRI-04**: No Miri errors in test suite

---

### Scaling - Checkpoint File

Requirements for large database checkpoint handling.

- [x] **SCALE-CP-01**: Checkpoint supports files larger than 1GB
- [x] **SCALE-CP-02**: Multi-file checkpointing or streaming checkpoint implemented
- [x] **SCALE-CP-03**: Large checkpoint tests verify correctness

---

### Scaling - Dirty Blocks

Requirements for tracking overflow handling.

- [x] **SCALE-DB-01**: Dirty block tracking overflow strategy implemented
- [x] **SCALE-DB-02**: Hierarchical tracking supports >50,000 global dirty blocks
- [x] **SCALE-DB-03**: Overflow handling tests verify correctness

---

### Scaling - Transaction Coordinator

Requirements for unbounded growth prevention.

- [x] **SCALE-TX-01**: Transaction ID bounds are enforced
- [x] **SCALE-TX-02**: Deadlock detection wait-for graph has cleanup
- [x] **SCALE-TX-03**: Transaction coordinator tests verify cleanup

---

### Scaling - HNSW Size

Requirements for memory-limited HNSW indexes.

- [x] **SCALE-HNSW-01**: Disk-based HNSW option for indexes larger than RAM
- [x] **SCALE-HNSW-02**: HNSW can spill to disk when memory threshold reached

---

### Dependencies - rusqlite

Requirements for SQLite dependency management.

- [x] **DEP-RUST-01**: rusqlite 0.31 monitored for security updates
- [x] **DEP-RUST-02**: System SQLite option evaluated for security patches

---

### Dependencies - bincode

Requirements for serialization upgrade planning.

- [x] **DEP-BIN-01**: bincode 2.0 migration plan documented
- [x] **DEP-BIN-02**: bincode 2.0 migration preserves existing data (format version bump)

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
| ACID-13 | Phase 13 | Complete |
| ACID-14 | Phase 13 | Complete |
| ACID-15 | Phase 13 | Complete |
| ACID-16 | Phase 13 | Complete |
| ACID-17 | Phase 13 | Complete |
| ACID-18 | Phase 13 | Complete |
| ACID-19 | Phase 14 | Complete |
| ACID-20 | Phase 14 | Complete |
| ACID-21 | Phase 14 | Complete |
| ACID-22 | Phase 14 | Complete |
| ACID-23 | Phase 14 | Complete |
| HNSW-01 | Phase 15 | Complete |
| HNSW-02 | Phase 15 | Complete |
| HNSW-03 | Phase 15 | Complete |
| HNSW-04 | Phase 15 | Complete |
| HNSW-05 | Phase 15 | Complete |
| HNSW-06 | Phase 15 | Complete |
| HNSW-07 | Phase 15 | Complete |
| HNSW-08 | Phase 15 | Complete |
| HNSW-09 | Phase 15 | Complete |
| HNSW-10 | Phase 15 | Deferred* |
| CP-01 | Phase 14 | Complete |
| CP-02 | Phase 14 | Complete |
| CP-03 | Phase 14 | Complete |
| CP-04 | Phase 14 | Complete |
| CPV-01 | Phase 12 | Complete |
| CPV-02 | Phase 12 | Complete |
| CPV-03 | Phase 12 | Complete |
| CPV-04 | Phase 12 | Complete |
| CPV-05 | Phase 12 | Complete |
| SCHEMA-01 | Phase 20 | Complete |
| SCHEMA-02 | Phase 20 | Complete |
| SCHEMA-03 | Phase 20 | Complete |
| UNSAFE-01 | Phase 16 | Complete |
| UNSAFE-02 | Phase 16 | Complete |
| UNSAFE-03 | Phase 16 | Complete |
| UNSAFE-04 | Phase 16 | Complete |
| UNSAFE-05 | Phase 16 | Complete |
| UNSAFE-06 | Phase 16 | Complete |
| UNSAFE-07 | Phase 16 | Complete |
| INPUT-01 | Phase 16 | Complete |
| INPUT-02 | Phase 16 | Complete |
| INPUT-03 | Phase 16 | Complete |
| INPUT-04 | Phase 16 | Complete |
| REFAC-01 | Phase 18 | Complete |
| REFAC-02 | Phase 18 | Complete |
| REFAC-03 | Phase 18 | Complete |
| REFAC-04 | Phase 18 | Complete |
| REFAC-05 | Phase 18 | Complete |
| REFAC-06 | Phase 18 | Complete |
| REFAC-07 | Phase 18 | Complete |
| CLONE-01 | Phase 18 | Complete |
| CLONE-02 | Phase 18 | Complete |
| CLONE-03 | Phase 18 | Complete |
| POOL-01 | Phase 19 | Complete |
| POOL-02 | Phase 19 | Complete |
| POOL-03 | Phase 19 | Complete |
| CW-01 | Phase 13 | Complete |
| CW-02 | Phase 13 | Complete |
| CW-03 | Phase 13 | Complete |
| MIGRATE-01 | Phase 20 | Complete |
| MIGRATE-02 | Phase 20 | Complete |
| MIGRATE-03 | Phase 20 | Complete |
| MIGRATE-04 | Phase 20 | Complete |
| BACKUP-01 | Phase 20 | Complete |
| BACKUP-02 | Phase 20 | Complete |
| BACKUP-03 | Phase 20 | Complete |
| TEST-WAL-01 | Phase 21 | Complete |
| TEST-WAL-02 | Phase 21 | Complete |
| TEST-WAL-03 | Phase 21 | Complete |
| TEST-WAL-04 | Phase 21 | Complete |
| TEST-CLUS-01 | Phase 21 | Complete |
| TEST-CLUS-02 | Phase 21 | Complete |
| TEST-CLUS-03 | Phase 21 | Complete |
| TEST-CP-01 | Phase 21 | Complete |
| TEST-CP-02 | Phase 21 | Complete |
| TEST-CP-03 | Phase 21 | Complete |
| TEST-HNSW-01 | Phase 21 | Complete |
| TEST-HNSW-02 | Phase 21 | Complete |
| TEST-HNSW-03 | Phase 21 | Complete |
| TEST-HNSW-04 | Phase 21 | Complete |
| TEST-MIRI-01 | Phase 21 | Complete |
| TEST-MIRI-02 | Phase 21 | Complete |
| TEST-MIRI-03 | Phase 21 | Complete |
| TEST-MIRI-04 | Phase 21 | Complete |
| SCALE-CP-01 | Phase 22 | Complete |
| SCALE-CP-02 | Phase 22 | Complete |
| SCALE-CP-03 | Phase 22 | Complete |
| SCALE-DB-01 | Phase 22 | Complete |
| SCALE-DB-02 | Phase 22 | Complete |
| SCALE-DB-03 | Phase 22 | Complete |
| SCALE-TX-01 | Phase 22 | Complete |
| SCALE-TX-02 | Phase 22 | Complete |
| SCALE-TX-03 | Phase 22 | Complete |
| SCALE-HNSW-01 | Phase 22 | Complete |
| SCALE-HNSW-02 | Phase 22 | Complete |
| DEP-RUST-01 | Phase 22 | Complete |
| DEP-RUST-02 | Phase 22 | Complete |
| DEP-BIN-01 | Phase 22 | Complete |
| DEP-BIN-02 | Phase 22 | Complete |

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
