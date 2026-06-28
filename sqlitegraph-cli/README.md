# sqlitegraph-cli

[![crates.io](https://img.shields.io/crates/v/sqlitegraph-cli.svg)](https://crates.io/crates/sqlitegraph-cli)
[![Documentation](https://docs.rs/sqlitegraph-cli/badge.svg)](https://docs.rs/sqlitegraph-cli)

Command-line interface for SQLiteGraph — an embedded graph and vector runtime.

## Installation

```bash
cargo install sqlitegraph-cli
```

Or install from source:

```bash
git clone https://github.com/oldnordic/sqlitegraph
cd sqlitegraph/sqlitegraph-cli
cargo install --path .
```

## Quick Start

```bash
# Build a small database from scratch
rm -f /tmp/sqlitegraph-demo.db
sqlitegraph --db /tmp/sqlitegraph-demo.db --write insert --kind User --name Alice --data '{"age":30}'
sqlitegraph --db /tmp/sqlitegraph-demo.db --write insert --kind User --name Bob --data '{"age":31}'
sqlitegraph --db /tmp/sqlitegraph-demo.db --write query 'CREATE (1)-[:KNOWS]->(2)'

# Cypher-inspired queries with edge traversal, WHERE, LIMIT
sqlitegraph --db /tmp/sqlitegraph-demo.db query 'MATCH (n:User) RETURN n.name'
sqlitegraph --db /tmp/sqlitegraph-demo.db query 'MATCH (a:User)-[:KNOWS]->(b:User) RETURN a.name, b.name'

# Graph algorithms
sqlitegraph --db /tmp/sqlitegraph-demo.db bfs --start 1 --depth 3
sqlitegraph --db /tmp/sqlitegraph-demo.db algo pagerank --iterations 100

# Insert data (requires --write flag)
sqlitegraph --db code.db --write insert --kind Function --name main
```

## Query Language

SQLiteGraph supports a Cypher-inspired query language. See [docs/QUERY_LANGUAGE.md](../docs/QUERY_LANGUAGE.md) for the full reference.

### Supported Patterns

```sql
-- Node scan
MATCH (n:Function) RETURN n.name

-- Node with property filter
MATCH (n:Function {lang: "rust"}) RETURN n

-- Edge traversal
MATCH (a)-[:CALLS]->(b) RETURN a.name, b.name

-- With WHERE and LIMIT
MATCH (a)-[:CALLS]->(b) WHERE b.lang = "python" RETURN a.name LIMIT 5
```

## Backend Selection

### SQLite Backend (Default)
- Mature, ACID-compliant storage
- Debuggable with standard SQL tools
- Required for Cypher queries

```bash
sqlitegraph --backend sqlite --db mygraph.db [command]
```

### Native V3 Backend
- Binary format for graph workloads
- Cache, KV, and pub/sub support
- Parallel traversal support for supported commands
- Workload-dependent performance; see the repository benchmarks

```bash
sqlitegraph --backend v3 --db mygraph.db [command]
```

## Commands

### Query Commands (Read-Only)
```bash
# Cypher-inspired queries (works with both backends)
# SQLite backend (default, always available)
sqlitegraph --backend sqlite query "MATCH (n:User) RETURN n.name"
sqlitegraph --backend sqlite query "MATCH (a)-[:CALLS]->(b) RETURN a, b"

# Native-v3 backend (requires --features native-v3 at compile time)
sqlitegraph --backend v3 query "MATCH (n:User) RETURN n.name"
sqlitegraph --backend v3 query "MATCH (n {role: 'admin'}) RETURN n"

# Graph traversal
sqlitegraph bfs --start 1 --depth 3
sqlitegraph path --from 1 --to 10
sqlitegraph neighbors --id 5 --direction outgoing

# Algorithms
sqlitegraph algo pagerank --iterations 100
sqlitegraph algo betweenness
sqlitegraph algo components             # weakly-connected components
sqlitegraph algo scc                    # strongly-connected components (Tarjan)
sqlitegraph algo louvain -i 100         # Louvain community detection
sqlitegraph algo label-prop -i 50       # label-propagation communities
sqlitegraph algo cycles -l 100          # find up to N cycles
sqlitegraph algo dominators -e 1        # dominator tree from entry node 1
sqlitegraph algo critical-path          # longest path through the DAG
sqlitegraph algo topo                   # topological sort

# HNSW vector index (read-only ops; --write needed for create/insert/delete)
sqlitegraph --write hnsw create --name embeddings --dim 384 --metric cosine
sqlitegraph --write hnsw insert --name embeddings --vector "0.1,0.2,0.3,..."
sqlitegraph hnsw search --name embeddings --k 5 --vector "0.1,0.2,0.3,..."
sqlitegraph hnsw list
sqlitegraph --write hnsw delete --name embeddings
```

### Data Modification (Requires --write)
```bash
# Insert nodes
sqlitegraph --write insert --kind User --name Alice

# Import/Export
sqlitegraph --write export --output graph.json
sqlitegraph --write import --input graph.json
```

### Status
```bash
sqlitegraph status
sqlitegraph list
sqlitegraph list --kind User
```

## Read-Only by Default

Direct mutation commands are guarded by default. Use `--write` to enable
modifications:

```bash
# This will fail (tries to modify without --write)
sqlitegraph --db graph.db insert --kind User --name Alice

# This works
sqlitegraph --db graph.db --write insert --kind User --name Alice
```

When running write-intended query statements such as `CREATE`, `SET`, or
`DELETE`, include `--write` in your command line.

## Ecosystem

This CLI is part of the SQLiteGraph ecosystem:

| Tool | Purpose | Repository |
|------|---------|------------|
| **sqlitegraph** | Core library | [crates.io/crates/sqlitegraph](https://crates.io/crates/sqlitegraph) |
| **sqlitegraph-cli** | This CLI | [crates.io/crates/sqlitegraph-cli](https://crates.io/crates/sqlitegraph-cli) |
| **magellan** | Code graph indexing | [crates.io/crates/magellan](https://crates.io/crates/magellan) |
| **llmgrep** | Semantic code search | [crates.io/crates/llmgrep](https://crates.io/crates/llmgrep) |
| **mirage** | CFG analysis | [crates.io/crates/mirage-analyzer](https://crates.io/crates/mirage-analyzer) |
| **splice** | Code editing | [crates.io/crates/splice](https://crates.io/crates/splice) |

## License

GPL-3.0-only
