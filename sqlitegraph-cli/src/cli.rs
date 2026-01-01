#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommandLineConfig {
    pub backend: String,
    pub database: String,
    pub command: String,
    pub command_args: Vec<String>,
}

impl CommandLineConfig {
    pub fn from_args(args: &[&str]) -> Result<Self, String> {
        let mut backend = String::from("sqlite");
        let mut database = String::from("memory");
        let mut command = String::from("status");
        let mut command_args = Vec::new();
        let mut command_set = false;
        let mut iter = args.iter().skip(1);
        while let Some(arg) = iter.next() {
            if command_set {
                command_args.push(arg.to_string());
                continue;
            }
            match *arg {
                "--backend" => {
                    backend = iter
                        .next()
                        .ok_or_else(|| "--backend requires a value".to_string())?
                        .to_string();
                }
                "--db" | "--database" => {
                    database = iter
                        .next()
                        .ok_or_else(|| "--db requires a value".to_string())?
                        .to_string();
                }
                "--command" => {
                    command = iter
                        .next()
                        .ok_or_else(|| "--command requires a value".to_string())?
                        .to_string();
                    command_set = true;
                }
                other if other.starts_with('-') => {
                    return Err(format!("unknown flag {other}"));
                }
                _ => {
                    command = arg.to_string();
                    command_set = true;
                }
            }
        }
        Ok(Self {
            backend,
            database,
            command,
            command_args,
        })
    }

    pub fn help() -> &'static str {
        r#"Usage: sqlitegraph [--backend sqlite|native] [--db memory|PATH] [command] [args]

Commands:
  status                    Show database status and statistics
  list                      List all entities in the graph
  migrate [--dry-run]       Run pending schema migrations
  dump-graph --output PATH  Dump graph data to file
  load-graph --input PATH   Load graph data from file
  bulk-insert-entities --input FILE  Bulk insert entities from JSON
  bulk-insert-edges --input FILE    Bulk insert edges from JSON
  bfs --start ID --max-depth N    Breadth-first search traversal
  k-hop --start ID --depth N [--direction incoming|outgoing]  K-hop neighbor query
  shortest-path --from ID --to ID    Find shortest path between nodes
  neighbors --id ID [--direction incoming|outgoing]  Direct neighbor query
  pattern-match --edge-type TYPE [--start-label LABEL] [--end-label LABEL] [--direction incoming|outgoing] [--start-prop KEY:VAL] [--end-prop KEY:VAL]  Match triple patterns
  pattern-match-fast --edge-type TYPE [--start-label LABEL] [--end-label LABEL] [--direction incoming|outgoing] [--start-prop KEY:VAL] [--end-prop KEY:VAL]  Fast-path pattern match
  hnsw-create --dimension N --m M --ef-construction N --distance-metric TYPE  Create HNSW index
  hnsw-insert --input FILE  Insert vectors into HNSW index
  hnsw-search --input FILE --k N  Search HNSW index
  hnsw-stats                Show HNSW index statistics
  wal-checkpoint            Trigger WAL checkpoint operation
  wal-metrics                Show WAL performance metrics and file sizes
  wal-config                 Show WAL configuration settings
  wal-stats                  Show detailed WAL statistics with derived metrics
  snapshot-create --dir DIR  Create database snapshot
  snapshot-load --dir DIR     Load database snapshot

Traversal Options:
  --start                   Starting node ID for traversal
  --max-depth                Maximum depth for BFS (default: 3)
  --depth                    Hop depth for k-hop (default: 2)
  --direction               Traversal direction: incoming|outgoing (default: outgoing)
  --from                     Source node ID for shortest path
  --to                       Target node ID for shortest path
  --id                       Node ID for neighbors query

Pattern Options:
  --edge-type                Edge type to match (required)
  --start-label              Start node label filter
  --end-label                End node label filter
  --start-prop               Start node property filter (format: key:value)
  --end-prop                 End node property filter (format: key:value)

HNSW Options:
  --dimension               Vector dimension (e.g., 768)
  --m                       Number of bi-directional links (default: 16)
  --ef-construction         HNSW ef_construction parameter (default: 200)
  --distance-metric         Distance metric: cosine|euclidean|dot|manhattan
  --k                       Number of nearest neighbors to return

Examples:
  sqlitegraph status
  sqlitegraph --db /path/to/graph.db list
  sqlitegraph bfs --start 123 --max-depth 3
  sqlitegraph k-hop --start 123 --depth 2 --direction outgoing
  sqlitegraph shortest-path --from 123 --to 456
  sqlitegraph neighbors --id 123 --direction incoming
  sqlitegraph bulk-insert-entities --input entities.json
  sqlitegraph pattern-match --edge-type DEPENDS_ON --start-label "Function" --end-label "Module"
  sqlitegraph pattern-match-fast --edge-type CALLS --direction outgoing
  sqlitegraph hnsw-create --dimension 768 --m 16 --ef-construction 200 --distance-metric cosine
  sqlitegraph migrate --dry-run
"#
    }
}
