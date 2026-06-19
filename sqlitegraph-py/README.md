# sqlitegraph

Python bindings to [`sqlitegraph`](https://crates.io/crates/sqlitegraph) — an embedded graph database with 35+ graph algorithms and HNSW vector search. Rust core, no server required, single `.db` or `.graph` file.

## Install

    pip install sqlitegraph

## Quick start

```python
from sqlitegraph import Graph

g = Graph.open_in_memory()
alice = g.add_node(kind="User", name="Alice", data={"age": 30})
order = g.add_node(kind="Order", name="Order-123")
g.add_edge(alice, order, "placed")

print(g.neighbors(alice))
```

## Query language

`Graph.query()` exposes the same Cypher-inspired language as the CLI:

```python
from sqlitegraph import Graph

g = Graph.open_in_memory()
alice = g.add_node(kind="User", name="Alice", data={"age": 30})
bob = g.add_node(kind="User", name="Bob", data={"age": 31})
g.add_edge(alice, bob, "KNOWS")

result = g.query("MATCH (a:User)-[:KNOWS]->(b:User) RETURN a.name, b.name")
print(result["results"])
```

Supported query features include node scans, edge traversal, multi-hop chains,
star/multi-pattern joins, variable-depth edges, `WHERE` with regex/numeric
operators and parentheses, `LIMIT`, `CREATE`, `SET`, `DELETE`, and HNSW vector
search through `CALL db.index.vector.queryNodes(...)`.

## Algorithms

```python
from sqlitegraph import Graph

g = Graph.open_in_memory()
a = g.add_node(kind="Page", name="A")
b = g.add_node(kind="Page", name="B")
c = g.add_node(kind="Page", name="C")
g.add_edge(a, b, "LINKS")
g.add_edge(b, c, "LINKS")
g.add_edge(c, a, "LINKS")

print(g.pagerank(iterations=20))
print(g.connected_components())
print(g.strongly_connected_components())
print(g.label_propagation(50))
print(g.find_cycles(10))
print(g.dominators(a))
```

`critical_path()` is also available for directed acyclic graphs and returns
`{"path": [...], "distance": float, "path_length": int}`.

## HNSW vector search

```python
from sqlitegraph import Graph

g = Graph.open_in_memory()
idx = g.create_hnsw_index("embeddings", dimension=3, metric="cosine")

idx.insert_vector([1.0, 0.8, 0.1], {"label": "graph databases"})
idx.insert_vector([0.1, 0.2, 1.0], {"label": "baking"})

print(idx.search([1.0, 0.9, 0.0], 1))
print(g.list_hnsw_indexes())

g.delete_hnsw_index("embeddings")
```

## Examples

The [`examples/`](./examples/) directory contains runnable scripts:

| Example | What it shows |
|---------|---------------|
| [`01_basic_crud.py`](./examples/01_basic_crud.py) | Nodes, edges, update, delete, query by kind/pattern, degrees |
| [`02_graph_algorithms.py`](./examples/02_graph_algorithms.py) | BFS, k-hop, shortest path, PageRank, Louvain & label-propagation communities, connected components (WCC), strongly-connected components (SCC), cycle search, dominator tree, critical path |
| [`03_vector_search.py`](./examples/03_vector_search.py) | HNSW index creation, insert, search, bulk insert, index listing |
| [`04_social_network.py`](./examples/04_social_network.py) | Realistic network: influencers (PageRank), communities, connection paths, mutual follows |
| [`05_file_backed.py`](./examples/05_file_backed.py) | Persistent `Graph.open(path)`, checkpoint, reopen, cleanup |
| [`06_hybrid_sqlite_hnsw_query.py`](./examples/06_hybrid_sqlite_hnsw_query.py) | sqlite3 application rows + sqlitegraph metadata + HNSW + `Graph.query()` expansion |

Run any example from the repo root:

```bash
cd sqlitegraph-py
source .venv/bin/activate
python examples/01_basic_crud.py
```
