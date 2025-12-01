use std::{env, path::PathBuf, process};

use serde_json::json;
use sqlitegraph::{
    BackendClient, ReindexConfig, SqliteGraph, SqliteGraphError,
    backend::SqliteGraphBackend,
    backend_client::CommandLineConfig,
    handle_command,
    recovery::{dump_graph_to_path, load_graph_from_path},
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

    let auto_migrate = config.command != "migrate";
    let backend = match open_backend(&config, auto_migrate) {
        Ok(b) => b,
        Err(err) => {
            eprintln!("{err}");
            process::exit(2);
        }
    };

    let client = BackendClient::new(backend);
    if let Err(err) = run_command(&client, &config.command, &config.command_args) {
        eprintln!("command failed: {err}");
        process::exit(1);
    }
}

fn open_backend(
    config: &CommandLineConfig,
    auto_migrate: bool,
) -> Result<SqliteGraphBackend, String> {
    match config.backend.as_str() {
        "sqlite" => {
            if config.database == "memory" {
                let graph = if auto_migrate {
                    SqliteGraph::open_in_memory().map_err(|e| e.to_string())?
                } else {
                    SqliteGraph::open_in_memory_without_migrations().map_err(|e| e.to_string())?
                };
                Ok(SqliteGraphBackend::from_graph(graph))
            } else {
                let path = PathBuf::from(&config.database);
                let graph = if auto_migrate {
                    SqliteGraph::open(&path).map_err(|e| e.to_string())?
                } else {
                    SqliteGraph::open_without_migrations(&path).map_err(|e| e.to_string())?
                };
                Ok(SqliteGraphBackend::from_graph(graph))
            }
        }
        other => Err(format!("unsupported backend {other}")),
    }
}

fn run_command(
    client: &BackendClient,
    command: &str,
    args: &[String],
) -> Result<(), SqliteGraphError> {
    match command {
        "status" => {
            let nodes = client.backend().entity_ids()?.len();
            let version = client.backend().graph().schema_version()?;
            println!("backend=sqlite schema_version={version} nodes={nodes}");
            Ok(())
        }
        "dump-graph" => {
            let output = required_flag_value(args, "--output")?;
            dump_graph_to_path(client.backend().graph(), &output)?;
            println!("dump_written=\"{output}\"");
            Ok(())
        }
        "load-graph" => {
            let input = required_flag_value(args, "--input")?;
            load_graph_from_path(client.backend().graph(), &input)?;
            println!("load_applied=\"{input}\"");
            Ok(())
        }
        "migrate" => run_migrate(client, args),
        "reindex-all" => run_reindex_all(client, args),
        "reindex-syncore" => run_reindex_syncore(client, args),
        "reindex-sync-graph" => run_reindex_sync_graph(client, args),
        "list" => {
            for id in client.backend().entity_ids()? {
                let entity = client.backend().graph().get_entity(id)?;
                println!("{}:{}", entity.id, entity.name);
            }
            Ok(())
        }
        other => {
            // Try to handle with cli_reasoning module
            match handle_command(client, other, args) {
                Ok(Some(output)) => {
                    println!("{output}");
                    Ok(())
                }
                Ok(None) => {
                    println!("unknown command {other}, defaulting to status");
                    let nodes = client.backend().entity_ids()?.len();
                    let version = client.backend().graph().schema_version()?;
                    println!("backend=sqlite schema_version={version} nodes={nodes}");
                    Ok(())
                }
                Err(err) => Err(err),
            }
        }
    }
}

fn required_flag_value(args: &[String], flag: &str) -> Result<String, SqliteGraphError> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == flag {
            return iter.next().cloned().ok_or_else(|| {
                SqliteGraphError::invalid_input(format!("missing value for {flag}"))
            });
        }
    }
    Err(SqliteGraphError::invalid_input(format!(
        "{flag} is required"
    )))
}

fn run_migrate(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let dry_run = args.iter().any(|arg| arg == "--dry-run");
    let report = client.backend().graph().run_pending_migrations(dry_run)?;
    let payload = json!({
        "command": "migrate",
        "dry_run": dry_run,
        "from_version": report.from_version,
        "to_version": report.to_version,
        "statements": report.statements,
    });
    println!("{payload}");
    Ok(())
}

fn run_reindex_all(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let config = create_reindex_config(args)?;
    let result = client.backend().graph().reindex_with_config(config)?;

    let payload = json!({
        "command": "reindex-all",
        "success": result.success,
        "duration_ms": result.total_duration.as_millis(),
        "entities_processed": result.entities_processed,
        "edges_processed": result.edges_processed,
        "labels_processed": result.labels_processed,
        "properties_processed": result.properties_processed,
        "indexes_rebuilt": result.indexes_rebuilt,
        "validation_errors": result.validation_errors,
    });
    println!("{payload}");
    Ok(())
}

fn run_reindex_syncore(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let config = create_reindex_config(args)?;
    let result = client
        .backend()
        .graph()
        .reindex_with_config(ReindexConfig {
            syncore: true,
            sync_graph: false,
            ..config
        })?;

    let payload = json!({
        "command": "reindex-syncore",
        "success": result.success,
        "duration_ms": result.total_duration.as_millis(),
        "entities_processed": result.entities_processed,
        "edges_processed": result.edges_processed,
        "labels_processed": result.labels_processed,
        "properties_processed": result.properties_processed,
        "indexes_rebuilt": result.indexes_rebuilt,
        "validation_errors": result.validation_errors,
    });
    println!("{payload}");
    Ok(())
}

fn run_reindex_sync_graph(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let config = create_reindex_config(args)?;
    let result = client
        .backend()
        .graph()
        .reindex_with_config(ReindexConfig {
            syncore: false,
            sync_graph: true,
            ..config
        })?;

    let payload = json!({
        "command": "reindex-sync-graph",
        "success": result.success,
        "duration_ms": result.total_duration.as_millis(),
        "entities_processed": result.entities_processed,
        "edges_processed": result.edges_processed,
        "labels_processed": result.labels_processed,
        "properties_processed": result.properties_processed,
        "indexes_rebuilt": result.indexes_rebuilt,
        "validation_errors": result.validation_errors,
    });
    println!("{payload}");
    Ok(())
}

fn create_reindex_config(args: &[String]) -> Result<ReindexConfig, SqliteGraphError> {
    let validate = !args.iter().any(|arg| arg == "--no-validate");
    let batch_size = parse_optional_u32(args, "--batch-size").unwrap_or(1000) as usize;

    let progress_callback = if args.iter().any(|arg| arg == "--progress") {
        Some(Box::new(|progress: sqlitegraph::ReindexProgress| {
            eprintln!(
                "[{}] {:.1}% ({}/{}) - elapsed: {:?}, remaining: {:?}",
                format!("{:?}", progress.stage).to_lowercase(),
                progress.progress_percent(),
                progress.current,
                progress.total,
                progress.elapsed,
                progress.estimated_remaining
            );
        })
            as Box<dyn Fn(sqlitegraph::ReindexProgress) + Send + Sync>)
    } else {
        None
    };

    Ok(ReindexConfig {
        syncore: true,
        sync_graph: true,
        validate,
        batch_size,
        progress_callback,
    })
}

fn parse_optional_u32(args: &[String], flag: &str) -> Option<u32> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == flag {
            if let Some(value) = iter.next() {
                return value.parse::<u32>().ok();
            }
        }
    }
    None
}
