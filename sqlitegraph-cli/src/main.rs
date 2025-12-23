use std::{env, fs, path::PathBuf, process};

use serde_json::json;
use sqlitegraph::{
    backend::{GraphBackend, SqliteGraphBackend},
    graph_opt::{bulk_insert_entities, bulk_insert_edges, GraphEntityCreate, GraphEdgeCreate},
    hnsw::{HnswConfigBuilder, DistanceMetric},
    recovery::{dump_graph_to_path, load_graph_from_path},
    GraphConfig, SqliteGraph, SqliteGraphError,
};
use sqlitegraph_cli::{cli::CommandLineConfig, client::BackendClient, reasoning};

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
    let client = match open_backend(&config, auto_migrate) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("{err}");
            process::exit(2);
        }
    };
    if let Err(err) = run_command(&client, &config.command, &config.command_args) {
        eprintln!("command failed: {err}");
        process::exit(1);
    }
}

fn open_backend(
    config: &CommandLineConfig,
    auto_migrate: bool,
) -> Result<BackendClient, String> {
    match config.backend.as_str() {
        "sqlite" => {
            if config.database == "memory" {
                let graph = if auto_migrate {
                    SqliteGraph::open_in_memory().map_err(|e| e.to_string())?
                } else {
                    SqliteGraph::open_in_memory_without_migrations().map_err(|e| e.to_string())?
                };
                Ok(BackendClient::new(SqliteGraphBackend::from_graph(graph)))
            } else {
                let path = PathBuf::from(&config.database);
                let graph = if auto_migrate {
                    SqliteGraph::open(&path).map_err(|e| e.to_string())?
                } else {
                    SqliteGraph::open_without_migrations(&path).map_err(|e| e.to_string())?
                };
                Ok(BackendClient::new(SqliteGraphBackend::from_graph(graph)))
            }
        }
        "native" | "native-v2" => {
            // Use the open_graph factory for Native backend
            let cfg = GraphConfig::native();
            let path = PathBuf::from(&config.database);

            // open_graph returns Box<dyn GraphBackend>
            let backend: Box<dyn GraphBackend> = sqlitegraph::open_graph(&path, &cfg)
                .map_err(|e| e.to_string())?;

            // Wrap in BackendClient using the from_dynamic method
            Ok(BackendClient::from_dynamic(backend))
        }
        other => Err(format!("unsupported backend {other}")),
    }
}

fn run_command(
    client: &BackendClient,
    command: &str,
    args: &[String],
) -> Result<(), SqliteGraphError> {
    if let Some(json) = reasoning::handle_command(client, command, args)? {
        println!("{json}");
        return Ok(());
    }
    match command {
        "status" => {
            let backend_type = client.backend_type();
            match client.graph() {
                Some(graph) => {
                    // SQLite backend - can get detailed info
                    let nodes = client.entity_ids()?.ok_or_else(|| SqliteGraphError::invalid_input("failed to get entity IDs"))?.len();
                    let version = graph.schema_version()?;
                    println!("backend={backend_type} schema_version={version} nodes={nodes}");
                }
                None => {
                    // Native/Dynamic backend - limited info
                    println!("backend={backend_type}");
                    println!("Note: Detailed status not available for {backend_type} backend");
                }
            }
            Ok(())
        }
        "dump-graph" => {
            let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("dump-graph command requires SQLite backend"))?;
            let output = required_flag_value(args, "--output")?;
            dump_graph_to_path(graph, &output)?;
            println!("dump_written=\"{output}\"");
            Ok(())
        }
        "load-graph" => {
            let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("load-graph command requires SQLite backend"))?;
            let input = required_flag_value(args, "--input")?;
            load_graph_from_path(graph, &input)?;
            println!("load_applied=\"{input}\"");
            Ok(())
        }
        "migrate" => run_migrate(client, args),
        "bulk-insert-entities" => run_bulk_insert_entities(client, args),
        "bulk-insert-edges" => run_bulk_insert_edges(client, args),
        "hnsw-create" => run_hnsw_create(client, args),
        "hnsw-insert" => run_hnsw_insert(client, args),
        "hnsw-search" => run_hnsw_search(client, args),
        "hnsw-stats" => run_hnsw_stats(client, args),
        // Reindex commands removed - not available in v0.2.5
        // "reindex-all" => run_reindex_all(client, args),
        // "reindex-syncore" => run_reindex_syncore(client, args),
        // "reindex-sync-graph" => run_reindex_sync_graph(client, args),
        "list" => {
            let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("list command requires SQLite backend"))?;
            for id in client.entity_ids()?.ok_or_else(|| SqliteGraphError::invalid_input("failed to get entity IDs"))? {
                let entity = graph.get_entity(id)?;
                println!("{}:{}", entity.id, entity.name);
            }
            Ok(())
        }
        other => {
            println!("unknown command {other}, defaulting to status");
            let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("status command requires SQLite backend"))?;
            let nodes = client.entity_ids()?.ok_or_else(|| SqliteGraphError::invalid_input("failed to get entity IDs"))?.len();
            let version = graph.schema_version()?;
            println!("backend=sqlite schema_version={version} nodes={nodes}");
            Ok(())
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
    let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("migrate command requires SQLite backend"))?;
    let report = graph.run_pending_migrations(dry_run)?;
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

fn run_bulk_insert_entities(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("bulk-insert-entities requires SQLite backend"))?;
    let input = required_flag_value(args, "--input")?;

    // Read JSON file
    let json_content = fs::read_to_string(&input)
        .map_err(|e| SqliteGraphError::invalid_input(format!("failed to read file: {e}")))?;

    // Parse JSON array manually since GraphEntityCreate doesn't implement Deserialize
    let json_array: Vec<serde_json::Value> = serde_json::from_str(&json_content)
        .map_err(|e| SqliteGraphError::invalid_input(format!("failed to parse JSON array: {e}")))?;

    let entities: Vec<GraphEntityCreate> = json_array.into_iter().map(|v| {
        let kind = v["kind"].as_str().unwrap_or("").to_string();
        let name = v["name"].as_str().unwrap_or("").to_string();
        let file_path = v["file_path"].as_str().map(|s| s.to_string());
        let data = v.get("data").cloned().unwrap_or(serde_json::json!({}));
        GraphEntityCreate { kind, name, file_path, data }
    }).collect();

    // Perform bulk insert
    let ids = bulk_insert_entities(graph, &entities)?;

    let payload = json!({
        "command": "bulk-insert-entities",
        "input": input,
        "entities_processed": entities.len(),
        "ids_created": ids,
    });
    println!("{payload}");
    Ok(())
}

fn run_bulk_insert_edges(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("bulk-insert-edges requires SQLite backend"))?;
    let input = required_flag_value(args, "--input")?;

    // Read JSON file
    let json_content = fs::read_to_string(&input)
        .map_err(|e| SqliteGraphError::invalid_input(format!("failed to read file: {e}")))?;

    // Parse JSON array manually since GraphEdgeCreate doesn't implement Deserialize
    let json_array: Vec<serde_json::Value> = serde_json::from_str(&json_content)
        .map_err(|e| SqliteGraphError::invalid_input(format!("failed to parse JSON array: {e}")))?;

    let edges: Vec<GraphEdgeCreate> = json_array.into_iter().map(|v| {
        let from_id = v["from_id"].as_i64().unwrap_or(0);
        let to_id = v["to_id"].as_i64().unwrap_or(0);
        let edge_type = v["edge_type"].as_str().unwrap_or("").to_string();
        let data = v.get("data").cloned().unwrap_or(serde_json::json!({}));
        GraphEdgeCreate { from_id, to_id, edge_type, data }
    }).collect();

    // Perform bulk insert
    let ids = bulk_insert_edges(graph, &edges)?;

    let payload = json!({
        "command": "bulk-insert-edges",
        "input": input,
        "edges_processed": edges.len(),
        "ids_created": ids,
    });
    println!("{payload}");
    Ok(())
}

fn run_hnsw_create(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("hnsw-create requires SQLite backend"))?;

    // Parse HNSW configuration from command-line arguments
    let dimension = required_flag_value(args, "--dimension")
        .and_then(|s| s.parse::<usize>().map_err(|e| SqliteGraphError::invalid_input(format!("invalid dimension: {e}"))))?;

    let m = required_flag_value(args, "--m")
        .and_then(|s| s.parse::<usize>().map_err(|e| SqliteGraphError::invalid_input(format!("invalid m: {e}"))))?;

    let ef_construction = required_flag_value(args, "--ef-construction")
        .and_then(|s| s.parse::<usize>().map_err(|e| SqliteGraphError::invalid_input(format!("invalid ef-construction: {e}"))))?;

    let distance_metric_str = required_flag_value(args, "--distance-metric")?;
    let distance_metric = match distance_metric_str.as_str() {
        "cosine" => DistanceMetric::Cosine,
        "euclidean" => DistanceMetric::Euclidean,
        "dot" | "dotproduct" => DistanceMetric::DotProduct,
        "manhattan" => DistanceMetric::Manhattan,
        _ => return Err(SqliteGraphError::invalid_input(format!("unsupported distance metric: {distance_metric_str}"))),
    };

    // Build HNSW configuration
    let config = HnswConfigBuilder::new()
        .dimension(dimension)
        .m_connections(m)
        .ef_construction(ef_construction)
        .ef_search(50) // Default ef_search
        .distance_metric(distance_metric)
        .build()
        .map_err(|e| SqliteGraphError::invalid_input(format!("invalid HNSW config: {e}")))?;

    // Create HNSW index
    let _hnsw = graph.hnsw_index("default", config)?;

    let payload = json!({
        "command": "hnsw-create",
        "dimension": dimension,
        "m": m,
        "ef_construction": ef_construction,
        "distance_metric": distance_metric_str,
        "status": "created"
    });
    println!("{payload}");
    Ok(())
}

fn run_hnsw_insert(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    // For now, HNSW operations require direct graph access
    // This is a placeholder - the actual implementation would need HNSW instance management
    let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("hnsw-insert requires SQLite backend"))?;

    let input = required_flag_value(args, "--input")?;

    // Read JSON file with vectors
    let json_content = fs::read_to_string(&input)
        .map_err(|e| SqliteGraphError::invalid_input(format!("failed to read file: {e}")))?;

    let json_array: Vec<serde_json::Value> = serde_json::from_str(&json_content)
        .map_err(|e| SqliteGraphError::invalid_input(format!("failed to parse JSON array: {e}")))?;

    // Note: This is a simplified implementation
    // Full implementation would require HNSW instance persistence
    let payload = json!({
        "command": "hnsw-insert",
        "input": input,
        "vectors_processed": json_array.len(),
        "status": "HNSW instance management not yet implemented"
    });
    println!("{payload}");
    Ok(())
}

fn run_hnsw_search(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("hnsw-search requires SQLite backend"))?;

    let input = required_flag_value(args, "--input")?;
    let k = required_flag_value(args, "--k")
        .and_then(|s| s.parse::<usize>().map_err(|e| SqliteGraphError::invalid_input(format!("invalid k: {e}"))))?;

    // Read query vector from file
    let json_content = fs::read_to_string(&input)
        .map_err(|e| SqliteGraphError::invalid_input(format!("failed to read file: {e}")))?;

    let json_value: serde_json::Value = serde_json::from_str(&json_content)
        .map_err(|e| SqliteGraphError::invalid_input(format!("failed to parse JSON: {e}")))?;

    // Note: This is a simplified implementation
    let payload = json!({
        "command": "hnsw-search",
        "input": input,
        "k": k,
        "status": "HNSW instance management not yet implemented"
    });
    println!("{payload}");
    Ok(())
}

fn run_hnsw_stats(client: &BackendClient, _args: &[String]) -> Result<(), SqliteGraphError> {
    let _graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("hnsw-stats requires SQLite backend"))?;

    // Note: This is a simplified implementation
    let payload = json!({
        "command": "hnsw-stats",
        "status": "HNSW instance management not yet implemented"
    });
    println!("{payload}");
    Ok(())
}

// Reindex functions removed - not available in v0.2.5
