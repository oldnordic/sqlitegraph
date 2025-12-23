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
  hnsw-create --dimension N --m M --ef-construction N --distance-metric TYPE  Create HNSW index
  hnsw-insert --input FILE  Insert vectors into HNSW index
  hnsw-search --input FILE --k N  Search HNSW index
  hnsw-stats                Show HNSW index statistics

HNSW Options:
  --dimension               Vector dimension (e.g., 768)
  --m                       Number of bi-directional links (default: 16)
  --ef-construction         HNSW ef_construction parameter (default: 200)
  --distance-metric         Distance metric: cosine|euclidean|dot|manhattan
  --k                       Number of nearest neighbors to return

Examples:
  sqlitegraph status
  sqlitegraph --db /path/to/graph.db list
  sqlitegraph bulk-insert-entities --input entities.json
  sqlitegraph hnsw-create --dimension 768 --m 16 --ef-construction 200 --distance-metric cosine
  sqlitegraph migrate --dry-run
"#
    }
}
