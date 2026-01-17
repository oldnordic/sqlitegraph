# sqlitegraph-cli

Command-line interface for SQLiteGraph graph database.

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
# Create an in-memory graph
sqlitegraph --backend sqlite --db :memory: entity-insert --kind User --name Alice

# Create a file-based graph
sqlitegraph --backend sqlite --db mygraph.db entity-insert --kind User --name Bob

# Use Native V2 backend (high performance)
sqlitegraph --backend native --db mygraph.db entity-insert --kind User --name Charlie
```

## Backend Selection

### SQLite Backend (Default)
- Mature, ACID-compliant storage
- WAL mode enabled by default for concurrent performance
- Best for general-purpose use

```bash
sqlitegraph --backend sqlite --db mygraph.db [command]
```

### Native V2 Backend
- High-performance clustered adjacency
- Custom binary format for maximum speed
- Best for performance-critical applications

```bash
sqlitegraph --backend native --db mygraph.db [command]
```

## Commands

### Entity Operations

#### Insert Entity
```bash
sqlitegraph --backend sqlite --db mygraph.db entity-insert \
  --kind User \
  --name "Alice Smith" \
  --file-path "/users/alice" \
  --data '{"age": 30, "city": "NYC"}'
```

#### Get Entity
```bash
sqlitegraph --backend sqlite --db mygraph.db entity-get --id 1
```

#### Update Entity
```bash
sqlitegraph --backend sqlite --db mygraph.db entity-update \
  --id 1 \
  --name "Alice Johnson" \
  --data '{"age": 31, "city": "LA"}'
```

#### Delete Entity
```bash
sqlitegraph --backend sqlite --db mygraph.db entity-delete --id 1
```

#### List Entities
```bash
# List all entities
sqlitegraph --backend sqlite --db mygraph.db entity-list

# Filter by kind
sqlitegraph --backend sqlite --db mygraph.db entity-list --kind User

# Filter by name pattern
sqlitegraph --backend sqlite --db mygraph.db entity-list --name-pattern "Alice%"
```

### Edge Operations

#### Insert Edge
```bash
sqlitegraph --backend sqlite --db mygraph.db edge-insert \
  --from-id 1 \
  --to-id 2 \
  --type "knows" \
  --data '{"since": 2020}'
```

#### Get Edge
```bash
sqlitegraph --backend sqlite --db mygraph.db edge-get --id 1
```

#### Update Edge
```bash
sqlitegraph --backend sqlite --db mygraph.db edge-update \
  --id 1 \
  --type "knows" \
  --data '{"since": 2019, "strength": "strong"}'
```

#### Delete Edge
```bash
sqlitegraph --backend sqlite --db mygraph.db edge-delete --id 1
```

#### List Edges
```bash
# List all edges
sqlitegraph --backend sqlite --db mygraph.db edge-list

# Edges from entity
sqlitegraph --backend sqlite --db mygraph.db edge-list --from-id 1

# Edges to entity
sqlitegraph --backend sqlite --db mygraph.db edge-list --to-id 2

# Filter by type
sqlitegraph --backend sqlite --db mygraph.db edge-list --type knows
```

### Query Operations

#### Neighbors
```bash
# Get outgoing neighbors
sqlitegraph --backend sqlite --db mygraph.db neighbors \
  --entity-id 1 \
  --direction outgoing

# Get incoming neighbors
sqlitegraph --backend sqlite --db mygraph.db neighbors \
  --entity-id 1 \
  --direction incoming

# Get all neighbors
sqlitegraph --backend sqlite --db mygraph.db neighbors \
  --entity-id 1 \
  --direction both
```

#### K-Hop Query
```bash
sqlitegraph --backend sqlite --db mygraph.db k-hop \
  --entity-id 1 \
  --max-depth 2 \
  --max-results 100
```

#### BFS Traversal
```bash
sqlitegraph --backend sqlite --db mygraph.db bfs \
  --start-entity-id 1 \
  --max-depth 3
```

### Bulk Operations

#### Bulk Insert
```bash
# Insert from JSON file
sqlitegraph --backend sqlite --db mygraph.db bulk-insert \
  --input entities.json

# Input format:
# {
#   "entities": [
#     {"kind": "User", "name": "Alice", "data": {"age": 30}},
#     {"kind": "User", "name": "Bob", "data": {"age": 25}}
#   ],
#   "edges": [
#     {"from": 0, "to": 1, "type": "knows", "data": {}}
#   ]
# }
```

#### Snapshot Export
```bash
sqlitegraph --backend sqlite --db mygraph.db snapshot-export \
  --output /backups/graph.snapshot
```

#### Snapshot Import
```bash
sqlitegraph --backend sqlite --db mygraph.db snapshot-import \
  --input /backups/graph.snapshot
```

### HNSW Vector Search Commands

The CLI provides HNSW vector search commands for testing and development.

**Important**: HNSW indexes do not persist across CLI invocations. Each CLI command creates a new database connection with empty HNSW storage. For persistent vector search functionality, use the Rust API directly in your application.

#### Create HNSW Index
```bash
sqlitegraph --backend sqlite --db :memory: hnsw-create \
  --dimension 768 \
  --m 16 \
  --ef-construction 200 \
  --ef-search 50 \
  --distance-metric cosine
```

#### Insert Vectors
```bash
# Create JSON file with vectors
cat > vectors.json <<EOF
{
  "vectors": [
    {
      "id": "vec1",
      "vector": [0.1, 0.2, 0.3],
      "metadata": {"label": "sample1"}
    },
    {
      "id": "vec2",
      "vector": [0.4, 0.5, 0.6],
      "metadata": {"label": "sample2"}
    }
  ]
}
EOF

# Insert vectors
sqlitegraph --backend sqlite --db :memory: hnsw-insert --input vectors.json
```

#### Search Vectors
```bash
# Create query file
cat > query.json <<EOF
{
  "vector": [0.15, 0.25, 0.35],
  "k": 5
}
EOF

# Search
sqlitegraph --backend sqlite --db :memory: hnsw-search --input query.json
```

#### Get Statistics
```bash
sqlitegraph --backend sqlite --db :memory: hnsw-stats
```

**Note on Persistence**: The HNSW CLI commands are useful for single-session testing. For production use with persistent vector indexes, use the sqlitegraph Rust library API.

### Utility Commands

#### Statistics
```bash
sqlitegraph --backend sqlite --db mygraph.db stats
```

#### Export Schema
```bash
sqlitegraph --backend sqlite --db mygraph.db schema-export
```

#### Validate Graph
```bash
sqlitegraph --backend sqlite --db mygraph.db validate
```

## Output Formats

Most commands output JSON by default:

```json
{
  "status": "success",
  "entity_id": 1,
  "kind": "User",
  "name": "Alice",
  "data": {"age": 30}
}
```

Use `jq` for pretty-printing:

```bash
sqlitegraph --backend sqlite --db mygraph.db entity-get --id 1 | jq '.'
```

## Exit Codes

- `0` - Success
- `1` - Error (check error message in JSON output)

## Examples

### Social Network
```bash
# Create graph
sqlitegraph --backend sqlite --db social.db entity-insert --kind User --name Alice
sqlitegraph --backend sqlite --db social.db entity-insert --kind User --name Bob
sqlitegraph --backend sqlite --db social.db entity-insert --kind User --name Charlie

# Create connections
sqlitegraph --backend sqlite --db social.db edge-insert --from-id 1 --to-id 2 --type "friend"
sqlitegraph --backend sqlite --db social.db edge-insert --from-id 2 --to-id 3 --type "friend"

# Find friends of friends
sqlitegraph --backend sqlite --db social.db neighbors --entity-id 1 --direction outgoing
sqlitegraph --backend sqlite --db social.db bfs --start-entity-id 1 --max-depth 2
```

### Knowledge Graph
```bash
# Create entities
sqlitegraph --backend sqlite --db knowledge.db entity-insert --kind Concept --name "Graph"
sqlitegraph --backend sqlite --db knowledge.db entity-insert --kind Concept --name "Database"
sqlitegraph --backend sqlite --db knowledge.db entity-insert --kind Concept --name "SQL"

# Link concepts
sqlitegraph --backend sqlite --db knowledge.db edge-insert --from-id 1 --to-id 2 --type "related-to"
sqlitegraph --backend sqlite --db knowledge.db edge-insert --from-id 2 --to-id 3 --type "uses"

# Query relationships
sqlitegraph --backend sqlite --db knowledge.db k-hop --entity-id 1 --max-depth 2
```

## Performance Tips

1. **Use Native V2 backend** for high-performance scenarios
2. **Enable WAL mode** (automatic for SQLite backend with file databases)
3. **Use bulk operations** for inserting large datasets
4. **Use snapshot export/import** for fast backups

## Limitations

### HNSW CLI Commands
- HNSW indexes do not persist across CLI invocations
- Each command creates a new process with empty HNSW storage
- For persistent HNSW functionality, use the Rust API

### General CLI
- Single-threaded operations
- No interactive mode
- Limited error recovery options

## See Also

- [sqlitegraph Library](https://crates.io/crates/sqlitegraph) - Rust library API
- [SQLiteGraph GitHub](https://github.com/oldnordic/sqlitegraph) - Source code
- [Documentation](https://docs.rs/sqlitegraph) - API documentation

## License

GPL-3.0-or-later
