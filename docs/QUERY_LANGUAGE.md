# SQLiteGraph Query Language

SQLiteGraph supports a Cypher-inspired query language for pattern matching and traversal. This is the primary interface for both the CLI (`sqlitegraph query`) and the Python API (`Graph.query()`).

## Supported Syntax

```
MATCH <pattern> [WHERE <conditions>] [RETURN <fields>] [LIMIT <n>]
                [SET <var>.<field> = <value> | DELETE <var>]
CREATE (n:Label {prop: "value"})
CREATE (<from-id>)-[:REL]->(<to-id>)
CALL db.index.vector.queryNodes('idx', k, [v1, v2, ...])
```

### Node Patterns

Find all nodes with a label:

```sql
MATCH (n:Function) RETURN n.name
```

Find nodes with inline property filters:

```sql
MATCH (n:Function {lang: "rust"}) RETURN n
```

Match any node (no label filter):

```sql
MATCH (n) RETURN n
```

### Edge Patterns

Traverse edges by type:

```sql
MATCH (a)-[:CALLS]->(b) RETURN a, b
```

With label filters on endpoints:

```sql
MATCH (a:Function)-[:CALLS]->(b:Function) RETURN a.name, b.name
```

### Star and Multi-Pattern Queries

Comma-separated patterns in a single `MATCH` are evaluated together and joined
on any shared variables. The common case is a *star* where every leg starts
from the same root:

```sql
MATCH (r)-[:OWNS]->(x), (r)-[:LIKES]->(y) RETURN r.name, x.name, y.name
```

Legs may also share *non-root* variables, which lets you express chains via
commas:

```sql
MATCH (a)-[:X]->(b), (b)-[:Y]->(c) RETURN a.name, b.name, c.name
```

is equivalent to the multi-hop form `(a)-[:X]->(b)-[:Y]->(c)`. Disjoint
legs with no shared variables produce a cartesian product:

```sql
MATCH (a)-[:X]->(b), (c)-[:Y]->(d) RETURN a, b, c, d
```

Internally the executor performs a hash-join across legs on every shared
variable name.

### WHERE Clause

Filter results by property values:

```sql
MATCH (n:Function) WHERE n.lang = "rust" RETURN n.name
```

Supported operators: `=`, `!=`, `<`, `<=`, `>`, `>=`, and `=~` (regex match).

```sql
MATCH (n) WHERE n.count > 5 RETURN n
MATCH (n) WHERE n.name =~ "main.*" RETURN n.name
```

Combine predicates with `AND` and `OR`. `OR` binds looser than `AND` (standard
precedence), so `a AND b OR c` is `(a AND b) OR c`:

```sql
MATCH (n) WHERE n.lang = "rust" AND n.name = "main" OR n.name = "util" RETURN n
```

Parentheses override the default precedence:

```sql
MATCH (n) WHERE (n.kind = "Function" OR n.kind = "Module") AND n.lang = "rust" RETURN n
```

On edge patterns, filter by either endpoint:

```sql
MATCH (a)-[:CALLS]->(b) WHERE b.lang = "python" RETURN a.name
```

### RETURN Clause

Return specific fields:

```sql
MATCH (n:Function) RETURN n.name, n.kind
```

Return entire nodes:

```sql
MATCH (n:Function) RETURN n
```

Return everything (default when RETURN is omitted):

```sql
MATCH (n:Function)
```

### LIMIT

Cap the number of results:

```sql
MATCH (n:Function) RETURN n.name LIMIT 10
```

### Vector Search via CALL

Query an HNSW vector index for the `k` nearest neighbours of a vector:

```sql
CALL db.index.vector.queryNodes('embeddings', 5, [0.1, 0.2, 0.3, ...])
```

Arguments are positional:
1. **Index name** — a single- or double-quoted string. The index must already
   be loaded (e.g. via `Graph.create_hnsw_index(...)` in Python or
   `hnsw_index_persistent(...)` in Rust); CALL does not create indices.
2. **k** — non-negative integer, how many neighbours to return.
3. **Query vector** — a bracketed list of floats. Negative, decimal, and
   scientific notation (`1e-3`) are all accepted. Length must match the
   index's configured dimension.

Result shape:

```json
{
  "results": [
    {"id": 0, "score": 0.10},
    {"id": 2, "score": 0.42}
  ],
  "count": 2
}
```

`id` is the HNSW-assigned identifier from `insert_vector` (currently a `u64`,
independent of graph node ids). `score` is the configured distance metric
(Euclidean, Cosine, or DotProduct).

## Field Reference

When accessing node fields in RETURN or WHERE:

| Field | Meaning |
|-------|---------|
| `n.id` | Node ID |
| `n.kind` | Node type (the "label" in Cypher maps to `kind`) |
| `n.name` | Node name |
| `n.<key>` | Property from the node's `data` JSON |

## CLI Usage

```bash
sqlitegraph --db graph.db query "MATCH (n:Function) RETURN n.name"

# Write-intended statements should be run with --write.
sqlitegraph --db graph.db --write query 'CREATE (1)-[:CALLS]->(2)'
```

## Python Usage

```python
import sqlitegraph as sg

g = sg.Graph.open("graph.db")
result = g.query('MATCH (n:Function) WHERE n.lang = "rust" RETURN n.name')
for row in result["results"]:
    print(row)
```

## Supported Labels

Labels in node scan patterns (e.g., `MATCH (n:Function)`) map to the node
`kind` field. For edge traversal patterns, endpoint label filters are routed
through the triple-pattern label index; for nodes inserted through normal
node APIs, use an unlabeled edge pattern plus `WHERE a.kind = "Function"` until
that label metadata has been registered.

## Limitations

- No aggregation (COUNT, SUM, AVG, etc.)
- No ORDER BY
- No WITH or UNWIND
- No variable bindings carried across patterns (each MATCH is independent)
- No shortestPath() function
- Native-v3 query support is not at full SQLite parity. The audited gaps are documented in [../NATIVE_V3_IMPLEMENTATION_STATUS.md](/home/feanor/Projects/sqlitegraph/NATIVE_V3_IMPLEMENTATION_STATUS.md:1) and [../NATIVE_V3_AUDIT.md](/home/feanor/Projects/sqlitegraph/NATIVE_V3_AUDIT.md:1).
- In particular, native-v3 still does not implement `snapshot_import`, does not yet have fully proven multi-hop Cypher parity, and still differs from SQLite in helper name-pattern semantics.
