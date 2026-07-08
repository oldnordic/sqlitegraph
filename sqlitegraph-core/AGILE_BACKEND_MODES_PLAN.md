# Agile Plan: Backend Modes and Combined Architecture

Status: Draft
Depends on: [SPEC.md](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/SPEC.md)

## Planning Basis

This plan is based on the current indexed architecture:

- public backend trait: [src/backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend.rs:116)
- stale selector surface: [src/backend_selector.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend_selector.rs:1)
- active config surface: [src/config/graph_config.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/config/graph_config.rs:1), [src/config/kinds.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/config/kinds.rs:1)
- SQLite engine open path: [src/graph/core.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/graph/core.rs:72)
- SQLite backend adapter: [src/backend/sqlite/impl_.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/sqlite/impl_.rs:207)
- native-v3 backend: [src/backend/native/v3/backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:70)
- native module contract mismatch: [src/backend/native/mod.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/mod.rs:1)
- schema entanglement: [src/schema.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/schema.rs:24), [src/schema.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/schema.rs:366)

## Phase 0: Contract Cleanup

Goal:
- make mode semantics explicit before storage changes

Changes:
- unify backend mode definitions
- deprecate or remove stale `backend_selector.rs` Neo4j model
- define one public mode/config contract

Files:
- [src/backend_selector.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend_selector.rs:1)
- [src/config/kinds.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/config/kinds.rs:1)
- [src/config/graph_config.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/config/graph_config.rs:1)
- [src/lib.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/lib.rs:1)

Why:
- current mode selection is duplicated and contradictory
- no further architecture work should proceed while the public mode surface is ambiguous

Verification:
- unit tests for config/builders
- doc tests for mode selection examples
- targeted graph queries confirming dead selector paths are removed or documented

## Phase 1: Open Path Separation

Goal:
- separate standalone backend construction from combined-mode construction

Changes:
- preserve standalone SQLite open path
- preserve standalone native-v3 open path
- add explicit combined-mode constructor/configuration

Files:
- [src/graph/core.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/graph/core.rs:72)
- [src/backend/sqlite/impl_.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/sqlite/impl_.rs:207)
- [src/backend/native/v3/backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:70)

Why:
- current code opens SQLite and HNSW together in one path
- native-v3 and SQLite are peers today; combined mode must be composition, not accidental reuse

Verification:
- standalone SQLite smoke tests unchanged
- standalone native-v3 smoke tests unchanged
- new combined-mode constructor tests compile and open cleanly

Status:
- complete
- implemented as:
  - `BackendKind::Combined`
  - `GraphConfig::combined()`
  - explicit `open_graph(..., combined)` branch

## Phase 2: Combined Mode Authority Boundary

Goal:
- make SQLite authoritative in combined mode

Changes:
- define authoritative SQL entities/edges/properties
- define derived graph materialization boundary
- define fallback behavior when materialization is stale or missing

Files:
- [src/backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend.rs:116)
- [src/backend/sqlite/impl_.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/sqlite/impl_.rs:283)
- [src/backend/native/v3/backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:3151)
- `src/sharding/*`

Why:
- the biggest risk is dual truth
- combined mode must never leave SQLite vs graph authority ambiguous

Verification:
- insert/update/delete tests proving SQL truth and graph view remain aligned
- fallback tests when graph materialization is absent

Status:
- in progress
- current completed slice:
  - public `CombinedGraphBackend`
  - combined mode now has a distinct SQLite-authoritative backend type
  - execution still delegates to SQLite by default
  - opt-in `CombinedReadMode::PreferMaterialized` now serves live untyped
    `neighbors()` / `bfs()` / `k_hop()` / `node_degree()` / `shortest_path()` from `csr_shards`
    with per-node or per-direction SQLite fallback
  - `PreferMaterialized` remains opt-in specialist behavior for now; benchmark
    evidence shows only modest cold-read gains against materially slower writes,
    and the current mixed-workload benchmarks still lose end-to-end
  - materialized combined reads are now guarded by an explicit freshness check
    against `graph_meta.authoritative_version` / `materialized_version`
  - `publish_materialized_views()` now rebuilds and publishes combined CSR
    state from SQLite truth
  - combined mode now incrementally refreshes affected CSR rows for edge
    inserts/deletes under `PreferMaterialized`, while node-only writes keep
    authoritative/materialized versions aligned

## Phase 3: Atomic SQLite + Graph Materialization

Goal:
- one atomic commit boundary for SQL truth plus graph materialization

Changes:
- define commit sequencing
- materialize forward/reverse/typed graph adjacency under one visibility boundary
- publish one snapshot version for combined mode

Files:
- [src/schema.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/schema.rs:1)
- [src/mvcc.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/mvcc.rs:1)
- [src/snapshot.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/snapshot.rs:1)
- [src/graph/core.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/graph/core.rs:72)
- [src/backend/native/v3/backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:578)

Why:
- this is the actual architectural payoff
- without this, combined mode is only dual-write, not one consistent engine

Verification:
- snapshot visibility tests
- rollback tests
- crash/reopen durability tests
- mutation tests across insert/update/delete

## Phase 4: Derived Graph Maintenance Strategy

Goal:
- make graph views maintainable and rebuildable

Changes:
- explicit rebuild API
- explicit incremental maintenance API
- define when combined mode uses rebuild vs delta maintenance

Files:
- [src/backend/native/v3/backend.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/backend/native/v3/backend.rs:362)
- `src/sharding/*`
- [src/recovery.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/recovery.rs:1)

Why:
- derived state must be operationally recoverable
- rebuildability is the safety valve for future optimization

Verification:
- rebuild-from-truth tests
- stale-view recovery tests
- benchmark comparisons for rebuild vs incremental maintenance

## Phase 5: HNSW Decoupling

Goal:
- keep HNSW as optional feature, not part of strict combined-mode atomicity

Changes:
- document HNSW as optional accelerator
- decouple HNSW load/init from the mandatory SQLite open path
- preserve exact fallback semantics

Files:
- [src/graph/core.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/graph/core.rs:130)
- `src/hnsw/*`
- [src/schema.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/schema.rs:24)

Why:
- HNSW currently raises architectural complexity for little benefit to the core contract
- keeping it optional reduces risk materially

Verification:
- SQLite-only and combined-mode tests with HNSW disabled
- HNSW feature tests still passing when enabled

## Phase 6: Documentation and Contract Synchronization

Goal:
- remove ambiguity from user-facing docs

Changes:
- update README
- update manual
- update crate docs
- update architecture/API docs
- update changelog

Files:
- [README.md](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/README.md:1)
- [manual.md](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/manual.md:1)
- [src/lib.rs](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/src/lib.rs:1)
- [CHANGELOG.md](/home/feanor/Projects/sqlitegraph/sqlitegraph-core/CHANGELOG.md:1)

Verification:
- docs reviewed against spec
- feature matrix consistent across all docs

## Risk Register

### R1: dual-truth bugs

Risk:
- combined mode could accidentally let graph state diverge from SQL truth

Mitigation:
- explicit authority model
- rebuild API
- SQL truth fallback

### R2: public API ambiguity

Risk:
- users cannot tell what is atomic in each mode

Mitigation:
- mode matrix in README/manual/API docs
- spec-first implementation

### R3: HNSW complexity leakage

Risk:
- HNSW semantics contaminate the core transaction contract

Mitigation:
- keep HNSW optional and outside strict atomic boundary in first redesign

### R4: stale architectural dead code

Risk:
- stale selector/config code continues to mislead maintainers

Mitigation:
- Phase 0 cleanup before deeper backend work

## Recommended First Implementation Slice

Start with:

1. Phase 0 contract cleanup
2. Phase 1 explicit constructors/mode surface
3. docs for those changes

Do not begin atomic combined-mode writes until the public mode surface is singular and unambiguous.

## Verification Discipline

Before each phase is marked done:

- `rtk cargo fmt --check`
- `rtk cargo check --all-targets`
- relevant targeted tests for the changed mode
- index refresh if symbols materially change

Graph-backed checks to repeat during implementation:

- `rtk magellan context impact --db /home/feanor/.magellan/sqlitegraph/sqlitegraph-core.db --name "<symbol>" --depth 3`
- `rtk mirage cfg --db /home/feanor/.magellan/sqlitegraph/sqlitegraph-core.db --function "<function>"`
- `rtk mirage blast-zone --db /home/feanor/.magellan/sqlitegraph/sqlitegraph-core.db --function "<function>"`
