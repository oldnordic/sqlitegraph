use std::{env, path::PathBuf, process};

use sqlitegraph::{
    SqliteGraphError,
    backend::SqliteGraphBackend,
    client::{BackendClient, CommandLineConfig},
    graph::SqliteGraph,
};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("{}", CommandLineConfig::help());
        return;
    }
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    let config = match CommandLineConfig::from_args(&arg_refs) {
        Ok(cfg) => cfg,
        Err(err) => {
            eprintln!("error: {err}");
            process::exit(2);
        }
    };

    let backend = match open_backend(&config) {
        Ok(b) => b,
        Err(err) => {
            eprintln!("{err}");
            process::exit(2);
        }
    };

    let client = BackendClient::new(backend);
    if let Err(err) = run_command(&client, &config.command) {
        eprintln!("command failed: {err}");
        process::exit(1);
    }
}

fn open_backend(config: &CommandLineConfig) -> Result<SqliteGraphBackend, String> {
    match config.backend.as_str() {
        "sqlite" => {
            if config.database == "memory" {
                SqliteGraphBackend::in_memory().map_err(|e| e.to_string())
            } else {
                let path = PathBuf::from(&config.database);
                let graph = SqliteGraph::open(path).map_err(|e| e.to_string())?;
                Ok(SqliteGraphBackend::from_graph(graph))
            }
        }
        other => Err(format!("unsupported backend {other}")),
    }
}

fn run_command(
    client: &BackendClient<SqliteGraphBackend>,
    command: &str,
) -> Result<(), SqliteGraphError> {
    match command {
        "status" => {
            let nodes = client.backend().entity_ids()?.len();
            println!("backend=sqlite nodes={nodes}");
            Ok(())
        }
        "list" => {
            for id in client.backend().entity_ids()? {
                let entity = client.backend().graph().get_entity(id)?;
                println!("{}:{}", entity.id, entity.name);
            }
            Ok(())
        }
        other => {
            println!("unknown command {other}, defaulting to status");
            let nodes = client.backend().entity_ids()?.len();
            println!("backend=sqlite nodes={nodes}");
            Ok(())
        }
    }
}
