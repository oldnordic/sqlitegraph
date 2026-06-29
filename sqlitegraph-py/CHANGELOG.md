# sqlitegraph (Python) — Changelog

This file tracks releases of the `sqlitegraph` package on PyPI. The Rust
crate of the same name has its own changelog at the repository root.

## [0.5.4] - 2026-06-29

### Changed

- Bundled core crate is now **sqlitegraph 3.4.2** — brings weighted neighbor retrieval, bulk weighted edge insertion, experimental CSR sharding, and key bug fixes.

## [0.5.3] - 2026-06-20

### Security

- **Bumped PyO3 0.28 → 0.29** to clear two advisories present in the 0.5.2
  wheel: RUSTSEC-2026-0176 (out-of-bounds read in `PyList`/`PyTuple` nth
  indexing) and RUSTSEC-2026-0177 (missing `Sync` bound on
  `PyCFunction::new_closure`). No 0.28.x backport exists; the minor bump
  was required. `numpy` tracked the bump (0.28 → 0.29).

### Changed

- Bundled core crate is now **sqlitegraph 3.3.0** — brings the temporal
  topology API (`checkpoint`, `as_of`, `scc_lineage_barcode`,
  `cycle_scc_barcode`) and the bounded HNSW cache fix to the Python wheel.

## [0.5.2] - 2026-06-07

### Changed

- **Removed poison handling from HNSW index lock calls** — The upstream Rust
  crate switched `hnsw_indexes` from `std::sync::Mutex` to `parking_lot::Mutex`,
  which does not return `LockResult`. Removed 7 `.map_err(...)` calls in the
  PyO3 bindings that handled mutex poisoning (no longer possible). (SG-1)

## [0.4.1] - 2026-05-18

### Fixed

- **`Graph.query("MATCH (a:User)-[:KNOWS]->(b:User) ...")` returned
  zero rows** after plain `Graph.add_node(kind="User", ...)` calls.
  Root cause was in the core `sqlitegraph` crate (fixed in 3.0.1):
  `insert_entity` only populated `graph_entities.kind` and left
  `graph_labels` empty, so `match_triples` label joins missed
  everything. The Python wheel picks up the fix automatically by
  bumping the bundled core to 3.0.1 — no Python-side code changes
  beyond the version bump. Workarounds that called the underlying
  Rust `add_label` helper are no longer necessary.

## [0.4.0] - 2026-05-18

### Highlights

Lines up with the Rust `sqlitegraph` 3.0.0 release: the full Cypher
engine (parens, mixed AND/OR, star/multi-pattern joins, `CALL` vector
queries) plus broader algorithm coverage and the HNSW autoload-
persistence fix are all reachable from Python now.

### Added
- **`Graph.query(query_str)`** — Exposes the Cypher-inspired query runtime from
  Python. Returns a dict with `results` and `count`.
- **Additional algorithm bindings**:
  - `Graph.strongly_connected_components()`
  - `Graph.label_propagation(max_iterations=50)`
  - `Graph.find_cycles(limit=100)`
  - `Graph.dominators(entry)`
  - `Graph.critical_path()`
- **`Graph.delete_hnsw_index(name)`** — Deletes an HNSW index from both the
  in-memory registry and the SQLite-backed vector tables.

## [0.3.0] - 2026-05-16

### Added
- **`Graph.add_nodes_bulk(items: list[dict])`** — Insert many nodes in a
  single FFI call inside one transaction. Each dict must have `kind` and
  `name`; `data` (dict) and `file_path` (str) are optional. Returns
  IDs in input order.
- **`Graph.add_edges_bulk(items: list[dict])`** — Insert many edges in a
  single FFI call inside one transaction. Each dict must have `from_id`,
  `to_id`, and `edge_type`; `data` (dict) is optional. Returns IDs in
  input order.
- **10 new pytest cases** in `tests/test_bulk_insert.py` covering both
  bulk paths, missing-field validation, data round-trip, and parity
  with per-item single-insert.

### Notes
- Built against `sqlitegraph` (Rust) **v2.4.0**, which adds the
  underlying `GraphBackend::insert_nodes_bulk` and `insert_edges_bulk`
  trait methods.
- All existing `add_node`/`add_edge` signatures are unchanged.

## [0.2.0] - 2026-05-15

### Added
- **`bfs(start, depth, edge_types=None, direction=None)`** — `bfs` now accepts
  an optional `edge_types` list and `direction` (`"outgoing"` or
  `"incoming"`). When `edge_types` is provided, traversal only follows edges
  whose type is in the list, dispatching to the new
  `GraphBackend::bfs_filtered`. With `edge_types=None`, behavior is unchanged
  (outgoing traversal, all edge types).
- **`shortest_path(start, end, edge_types=None)`** — Optional `edge_types`
  list restricts which edge types the path can traverse, dispatching to
  `GraphBackend::shortest_path_filtered`. Empty list returns `None`.
- **`k_hop(start, depth, direction=None, edge_types=None)`** — The existing
  `k_hop` now exposes `edge_types`, dispatching to
  `GraphBackend::k_hop_filtered` when provided. Empty list returns an empty
  result.
- **11 new pytest tests** in `tests/test_filtered_traversal.py` covering each
  new kwarg plus backwards-compatibility checks for the old kwargless calls.

### Notes
- Built against `sqlitegraph` (Rust) **v2.3.0**, which adds the underlying
  `bfs_filtered` / `shortest_path_filtered` trait methods.
- All existing tests continue to pass without modification — the new kwargs
  are strictly additive.

## [0.1.1] - 2026-05-15

### Fixed
- **`create_hnsw_index` now calls `hnsw_index_persistent`** — Previously the
  Python binding called the non-persistent `hnsw_index()`, so vector indexes
  created from Python were lost when the `Graph` object was dropped. Now it
  calls `hnsw_index_persistent()`, matching the expected durability contract.
  Requires `sqlitegraph` (Rust) **>= 2.2.5** (the release that fixes the
  underlying `database_list` column read).

## [0.1.0] — unreleased

### Added
- First Python release. Thin wrapper around the audited Rust core via PyO3
  + maturin. Single `abi3` wheel per platform; works on Python 3.10+.
- `Graph` class:
  - File-backed (`Graph.open(path)`) and in-memory (`Graph.open_in_memory()`).
  - Node CRUD (`add_node`, `get_node`, `update_node`, `delete_node`,
    `node_ids`, `nodes_by_kind`, `nodes_by_name_pattern`, `node_degree`).
  - Edge CRUD (`add_edge`, `get_edge`, `delete_edge`, `neighbors`).
  - Traversal (`bfs`, `k_hop`, `shortest_path`).
  - Algorithms (`pagerank`, `louvain_communities`, `connected_components`).
  - HNSW vector indexes (`create_hnsw_index`, `get_hnsw_index`,
    `list_hnsw_indexes`).
- `HnswIndex` class: `insert_vector`, `bulk_insert_vectors`, `search`,
  `get_vector`, `vector_count`, `name`.
- Typed exception hierarchy: `GraphError` (base), `NotFoundError`,
  `InvalidArgumentError`, `BackendError`.
- Type stubs (`_native.pyi`) and `py.typed` marker for editor support.
- 39 pytest tests covering CRUD, traversal, algorithms, HNSW, and the
  exception hierarchy.

### Notes
- Built against `sqlitegraph` (Rust) **v2.2.4**.
- The optional `inference` cargo feature pulls in `numpy` + `ndarray` for
  the experimental sparse-inference engine; it is **off by default** in
  PyPI wheels.
