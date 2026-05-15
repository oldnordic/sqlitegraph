# sqlitegraph (Python) — Changelog

This file tracks releases of the `sqlitegraph` package on PyPI. The Rust
crate of the same name has its own changelog at the repository root.

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
