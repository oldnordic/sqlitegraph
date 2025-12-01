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
        r#"Usage: sqlitegraph [--backend sqlite] [--db memory|PATH] [--command status]

Commands:
  status                    Show database status and statistics
  list                      List all entities in the graph
  migrate [--dry-run]       Run pending schema migrations
  dump-graph --output PATH  Dump graph data to file
  load-graph --input PATH   Load graph data from file
  reindex-all [options]     Rebuild all indexes (syncore + sync graph)
  reindex-syncore [options] Rebuild core database indexes only
  reindex-sync-graph [options] Rebuild graph indexes and caches only

Reindexing Options:
  --progress                Show progress during reindexing
  --no-validate             Skip index validation after rebuilding
  --batch-size SIZE         Batch size for processing (default: 1000)

Examples:
  sqlitegraph status
  sqlitegraph --db /path/to/graph.db list
  sqlitegraph reindex-all --progress
  sqlitegraph reindex-syncore --batch-size 500
  sqlitegraph migrate --dry-run
"#
    }
}
