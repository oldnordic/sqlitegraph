use std::{env, fs, path::PathBuf, process};

use serde_json::json;
use sqlitegraph::{
    backend::{BackendDirection, GraphBackend, SqliteGraphBackend},
    bfs::{bfs_neighbors, shortest_path},
    graph_opt::{bulk_insert_entities, bulk_insert_edges, GraphEntityCreate, GraphEdgeCreate},
    hnsw::{HnswConfigBuilder, DistanceMetric},
    multi_hop::k_hop,
    pattern_engine::PatternTriple,
    query::GraphQuery,
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
            // Create NativeGraphBackend directly to have access to WAL-specific methods
            let path = PathBuf::from(&config.database);

            // Check if file exists to decide whether to create or open
            let backend = if path.exists() {
                sqlitegraph::NativeGraphBackend::open(&path)
                    .map_err(|e| format!("Failed to open native backend: {}", e))?
            } else {
                sqlitegraph::NativeGraphBackend::new(&path)
                    .map_err(|e| format!("Failed to create native backend: {}", e))?
            };

            Ok(BackendClient::new_native(backend))
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
        "bfs" => run_bfs(client, args),
        "k-hop" => run_k_hop(client, args),
        "shortest-path" => run_shortest_path(client, args),
        "neighbors" => run_neighbors(client, args),
        "pattern-match" => run_pattern_match(client, args),
        "pattern-match-fast" => run_pattern_match_fast(client, args),
        "wal-checkpoint" => run_wal_checkpoint(client, args),
        "wal-metrics" => run_wal_metrics(client, args),
        "wal-config" => run_wal_config(client, args),
        "wal-stats" => run_wal_stats(client, args),
        "snapshot-create" => run_snapshot_create(client, args),
        "snapshot-load" => run_snapshot_load(client, args),
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

    // NOTE: HNSW indexes are stored in-memory within SqliteGraph and do NOT persist
    // across CLI invocations. Each CLI command creates a new SqliteGraph instance with
    // empty HNSW index storage. For persistent HNSW functionality, use the Rust API directly.
    //
    // See: https://github.com/your-repo/sqlitegraph/docs/hnsw_cli_persistence_issue_20241223.md

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
    let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("hnsw-insert requires SQLite backend"))?;

    // NOTE: HNSW indexes are stored in-memory within SqliteGraph and do NOT persist
    // across CLI invocations. The index created by 'hnsw-create' is lost when that CLI
    // process exits. Subsequent commands will fail with "index not found" unless used
    // within the same CLI session (which is not currently supported).
    //
    // For persistent HNSW functionality, use the Rust API directly:
    //   let graph = SqliteGraph::open("mydb.db")?;
    //   let hnsw = graph.hnsw_index("vectors", config)?;
    //   hnsw.insert_vector(&vector, metadata)?;
    //
    // See: https://github.com/your-repo/sqlitegraph/docs/hnsw_cli_persistence_issue_20241223.md

    // Get index name (default to "default" if not specified)
    let index_name = args.iter()
        .position(|arg| arg == "--name")
        .and_then(|idx| args.get(idx + 1))
        .map(|s| s.as_str())
        .unwrap_or("default");

    let input = required_flag_value(args, "--input")?;

    // Read JSON file with vectors
    let json_content = fs::read_to_string(&input)
        .map_err(|e| SqliteGraphError::invalid_input(format!("failed to read file: {e}")))?;

    let json_array: Vec<serde_json::Value> = serde_json::from_str(&json_content)
        .map_err(|e| SqliteGraphError::invalid_input(format!("failed to parse JSON array: {e}")))?;

    // Insert vectors into HNSW index
    let mut inserted_count = 0;
    let mut errors = Vec::new();

    for (idx, json_value) in json_array.iter().enumerate() {
        // Parse vector data
        let vector_array = json_value["vector"]
            .as_array()
            .ok_or_else(|| SqliteGraphError::invalid_input(format!("vector {} missing 'vector' field", idx)))?;

        let vector_data: Vec<f32> = vector_array.iter()
            .enumerate()
            .map(|(i, v)| {
                v.as_f64()
                    .ok_or_else(|| SqliteGraphError::invalid_input(format!("vector element at index {} not a number", i)))
                    .map(|f| f as f32)
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Get metadata if present
        let metadata = json_value.get("metadata").cloned();

        // Insert vector
        let insert_result = graph.get_hnsw_index_mut(index_name, |hnsw| {
            hnsw.insert_vector(&vector_data, metadata)
        });

        match insert_result {
            Ok(_vector_id) => {
                inserted_count += 1;
            }
            Err(e) => {
                errors.push(format!("Vector {}: {}", idx, e));
            }
        }
    }

    let payload = json!({
        "command": "hnsw-insert",
        "index_name": index_name,
        "input": input,
        "vectors_processed": json_array.len(),
        "vectors_inserted": inserted_count,
        "errors": errors,
        "status": if errors.is_empty() { "completed" } else { "completed_with_errors" }
    });
    println!("{payload}");
    Ok(())
}

fn run_hnsw_search(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("hnsw-search requires SQLite backend"))?;

    // NOTE: HNSW indexes are stored in-memory within SqliteGraph and do NOT persist
    // across CLI invocations. This command will fail with "index not found" unless the
    // index was created in the same CLI session (which is not currently supported).
    //
    // For persistent HNSW functionality, use the Rust API directly:
    //   let graph = SqliteGraph::open("mydb.db")?;
    //   let hnsw = graph.hnsw_index("vectors", config)?;
    //   let results = hnsw.search(&query_vector, k)?;
    //
    // See: https://github.com/your-repo/sqlitegraph/docs/hnsw_cli_persistence_issue_20241223.md

    // Get index name (default to "default" if not specified)
    let index_name = args.iter()
        .position(|arg| arg == "--name")
        .and_then(|idx| args.get(idx + 1))
        .map(|s| s.as_str())
        .unwrap_or("default");

    let input = required_flag_value(args, "--input")?;
    let k = required_flag_value(args, "--k")
        .and_then(|s| s.parse::<usize>().map_err(|e| SqliteGraphError::invalid_input(format!("invalid k: {e}"))))?;

    // Read query vector from file
    let json_content = fs::read_to_string(&input)
        .map_err(|e| SqliteGraphError::invalid_input(format!("failed to read file: {e}")))?;

    let json_value: serde_json::Value = serde_json::from_str(&json_content)
        .map_err(|e| SqliteGraphError::invalid_input(format!("failed to parse JSON: {e}")))?;

    // Parse query vector
    let query_array = json_value["vector"]
        .as_array()
        .ok_or_else(|| SqliteGraphError::invalid_input("query missing 'vector' field"))?;

    let query_vector: Vec<f32> = query_array.iter()
        .enumerate()
        .map(|(i, v)| {
            v.as_f64()
                .ok_or_else(|| SqliteGraphError::invalid_input(format!("query vector element at index {} not a number", i)))
                .map(|f| f as f32)
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Perform search
    let search_result = graph.get_hnsw_index_ref(index_name, |hnsw| {
        hnsw.search(&query_vector, k)
    });

    match search_result {
        Ok(search_result) => {
            match search_result {
                Ok(results) => {
                    let results_json: Vec<_> = results.iter()
                        .map(|(vector_id, distance)| json!({
                            "vector_id": vector_id,
                            "distance": distance
                        }))
                        .collect();

                    let payload = json!({
                        "command": "hnsw-search",
                        "index_name": index_name,
                        "k": k,
                        "results": results_json,
                        "found": results.len(),
                        "status": "completed"
                    });
                    println!("{payload}");
                    Ok(())
                }
                Err(e) => {
                    let payload = json!({
                        "command": "hnsw-search",
                        "index_name": index_name,
                        "error": e.to_string(),
                        "status": "error"
                    });
                    println!("{payload}");
                    Ok(())
                }
            }
        }
        Err(e) => {
            let payload = json!({
                "command": "hnsw-search",
                "index_name": index_name,
                "error": e.to_string(),
                "status": "error"
            });
            println!("{payload}");
            Ok(())
        }
    }
}

fn run_hnsw_stats(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("hnsw-stats requires SQLite backend"))?;

    // NOTE: HNSW indexes are stored in-memory within SqliteGraph and do NOT persist
    // across CLI invocations. This command will fail with "index not found" unless the
    // index was created in the same CLI session (which is not currently supported).
    //
    // For persistent HNSW functionality, use the Rust API directly:
    //   let graph = SqliteGraph::open("mydb.db")?;
    //   let hnsw = graph.hnsw_index("vectors", config)?;
    //   let stats = hnsw.statistics()?;
    //
    // See: https://github.com/your-repo/sqlitegraph/docs/hnsw_cli_persistence_issue_20241223.md

    // Get index name (default to "default" if not specified)
    let index_name = args.iter()
        .position(|arg| arg == "--name")
        .and_then(|idx| args.get(idx + 1))
        .map(|s| s.as_str())
        .unwrap_or("default");

    // Get HNSW index statistics using read-only access
    let stats_result = graph.get_hnsw_index_ref(index_name, |hnsw| {
        hnsw.statistics()
    });

    match stats_result {
        Ok(stats_result) => {
            match stats_result {
                Ok(stats) => {
                    let payload = json!({
                        "command": "hnsw-stats",
                        "index_name": index_name,
                        "vector_count": stats.vector_count,
                        "layer_count": stats.layer_count,
                        "entry_point_count": stats.entry_point_count,
                        "dimension": stats.dimension,
                        "distance_metric": format!("{:?}", stats.distance_metric),
                        "storage_stats": {
                            "vector_count": stats.storage_stats.vector_count,
                            "total_dimensions": stats.storage_stats.total_dimensions,
                            "average_dimension": stats.storage_stats.average_dimension,
                            "estimated_memory_bytes": stats.storage_stats.estimated_memory_bytes,
                            "backend_type": stats.storage_stats.backend_type,
                        },
                        "layer_stats": stats.layer_stats.iter()
                            .map(|(layer_id, node_count, avg_conn)| json!({
                                "layer": layer_id,
                                "node_count": node_count,
                                "avg_connections": avg_conn
                            }))
                            .collect::<Vec<_>>()
                    });
                    println!("{payload}");
                    Ok(())
                }
                Err(e) => {
                    let payload = json!({
                        "command": "hnsw-stats",
                        "index_name": index_name,
                        "error": e.to_string(),
                        "status": "error"
                    });
                    println!("{payload}");
                    Ok(())
                }
            }
        }
        Err(e) => {
            let payload = json!({
                "command": "hnsw-stats",
                "index_name": index_name,
                "error": e.to_string(),
                "status": "error"
            });
            println!("{payload}");
            Ok(())
        }
    }
}

fn run_bfs(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("bfs command requires SQLite backend"))?;

    let start = required_flag_value(args, "--start")
        .and_then(|s| s.parse::<i64>().map_err(|e| SqliteGraphError::invalid_input(format!("invalid start node: {e}"))))?;

    let max_depth = required_flag_value(args, "--max-depth")
        .and_then(|s| s.parse::<u32>().map_err(|e| SqliteGraphError::invalid_input(format!("invalid max-depth: {e}"))))?;

    let visited = bfs_neighbors(graph, start, max_depth)?;

    let payload = json!({
        "command": "bfs",
        "start": start,
        "max_depth": max_depth,
        "visited_count": visited.len(),
        "visited_nodes": visited
    });
    println!("{payload}");
    Ok(())
}

fn run_k_hop(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("k-hop command requires SQLite backend"))?;

    let start = required_flag_value(args, "--start")
        .and_then(|s| s.parse::<i64>().map_err(|e| SqliteGraphError::invalid_input(format!("invalid start node: {e}"))))?;

    let depth = required_flag_value(args, "--depth")
        .and_then(|s| s.parse::<u32>().map_err(|e| SqliteGraphError::invalid_input(format!("invalid depth: {e}"))))?;

    let direction_str = required_flag_value(args, "--direction").unwrap_or_else(|_| "outgoing".to_string());
    let direction = match direction_str.as_str() {
        "incoming" => BackendDirection::Incoming,
        "outgoing" | _ => BackendDirection::Outgoing,
    };

    let neighbors = k_hop(graph, start, depth, direction)?;

    let payload = json!({
        "command": "k-hop",
        "start": start,
        "depth": depth,
        "direction": direction_str,
        "neighbor_count": neighbors.len(),
        "neighbors": neighbors
    });
    println!("{payload}");
    Ok(())
}

fn run_shortest_path(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("shortest-path command requires SQLite backend"))?;

    let start = required_flag_value(args, "--from")
        .and_then(|s| s.parse::<i64>().map_err(|e| SqliteGraphError::invalid_input(format!("invalid start node: {e}"))))?;

    let end = required_flag_value(args, "--to")
        .and_then(|s| s.parse::<i64>().map_err(|e| SqliteGraphError::invalid_input(format!("invalid end node: {e}"))))?;

    let path = shortest_path(graph, start, end)?;

    let payload = json!({
        "command": "shortest-path",
        "from": start,
        "to": end,
        "path_exists": path.is_some(),
        "path": path
    });
    println!("{payload}");
    Ok(())
}

fn run_neighbors(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("neighbors command requires SQLite backend"))?;

    let id = required_flag_value(args, "--id")
        .and_then(|s| s.parse::<i64>().map_err(|e| SqliteGraphError::invalid_input(format!("invalid node id: {e}"))))?;

    let direction_str = required_flag_value(args, "--direction").unwrap_or_else(|_| "outgoing".to_string());
    let query = graph.query();

    let neighbors = match direction_str.as_str() {
        "incoming" => query.incoming(id)?,
        "outgoing" | _ => query.outgoing(id)?,
    };

    let payload = json!({
        "command": "neighbors",
        "id": id,
        "direction": direction_str,
        "neighbor_count": neighbors.len(),
        "neighbors": neighbors
    });
    println!("{payload}");
    Ok(())
}

fn run_pattern_match(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("pattern-match command requires SQLite backend"))?;

    // Parse required --edge-type parameter
    let edge_type = required_flag_value(args, "--edge-type")?;

    // Parse optional parameters
    let start_label = optional_flag_value(args, "--start-label");
    let end_label = optional_flag_value(args, "--end-label");
    let direction_str = optional_flag_value(args, "--direction").unwrap_or_else(|| "outgoing".to_string());

    // Parse property filters (format: --start-prop key:value --end-prop key:value)
    let mut start_props = std::collections::HashMap::new();
    let mut end_props = std::collections::HashMap::new();

    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if arg == "--start-prop" {
            if let Some(prop_value) = iter.next() {
                if let Some((key, value)) = prop_value.split_once(':') {
                    start_props.insert(key.to_string(), value.to_string());
                }
            }
        } else if arg == "--end-prop" {
            if let Some(prop_value) = iter.next() {
                if let Some((key, value)) = prop_value.split_once(':') {
                    end_props.insert(key.to_string(), value.to_string());
                }
            }
        }
    }

    // Build pattern triple
    let direction = match direction_str.as_str() {
        "incoming" => BackendDirection::Incoming,
        "outgoing" | _ => BackendDirection::Outgoing,
    };

    let mut pattern = PatternTriple::new(&edge_type)
        .direction(direction);

    if let Some(ref label) = start_label {
        pattern = pattern.start_label(label);
    }
    if let Some(ref label) = end_label {
        pattern = pattern.end_label(label);
    }

    // Add property filters
    for (key, value) in start_props {
        pattern = pattern.start_property(key, value);
    }
    for (key, value) in end_props {
        pattern = pattern.end_property(key, value);
    }

    // Execute pattern match
    let matches = graph.match_triples(&pattern)?;

    // Convert TripleMatch to serializable format
    let matches_json: Vec<serde_json::Value> = matches.into_iter().map(|m| {
        json!({
            "start_id": m.start_id,
            "end_id": m.end_id,
            "edge_id": m.edge_id
        })
    }).collect();

    let payload = json!({
        "command": "pattern-match",
        "edge_type": edge_type,
        "start_label": start_label,
        "end_label": end_label,
        "direction": direction_str,
        "match_count": matches_json.len(),
        "matches": matches_json
    });
    println!("{payload}");
    Ok(())
}

fn run_pattern_match_fast(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let graph = client.graph().ok_or_else(|| SqliteGraphError::invalid_input("pattern-match-fast command requires SQLite backend"))?;

    // Parse required --edge-type parameter
    let edge_type = required_flag_value(args, "--edge-type")?;

    // Parse optional parameters
    let start_label = optional_flag_value(args, "--start-label");
    let end_label = optional_flag_value(args, "--end-label");
    let direction_str = optional_flag_value(args, "--direction").unwrap_or_else(|| "outgoing".to_string());

    // Parse property filters (format: --start-prop key:value --end-prop key:value)
    let mut start_props = std::collections::HashMap::new();
    let mut end_props = std::collections::HashMap::new();

    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        if arg == "--start-prop" {
            if let Some(prop_value) = iter.next() {
                if let Some((key, value)) = prop_value.split_once(':') {
                    start_props.insert(key.to_string(), value.to_string());
                }
            }
        } else if arg == "--end-prop" {
            if let Some(prop_value) = iter.next() {
                if let Some((key, value)) = prop_value.split_once(':') {
                    end_props.insert(key.to_string(), value.to_string());
                }
            }
        }
    }

    // Build pattern triple
    let direction = match direction_str.as_str() {
        "incoming" => BackendDirection::Incoming,
        "outgoing" | _ => BackendDirection::Outgoing,
    };

    let mut pattern = PatternTriple::new(&edge_type)
        .direction(direction);

    if let Some(ref label) = start_label {
        pattern = pattern.start_label(label);
    }
    if let Some(ref label) = end_label {
        pattern = pattern.end_label(label);
    }

    // Add property filters
    for (key, value) in start_props {
        pattern = pattern.start_property(key, value);
    }
    for (key, value) in end_props {
        pattern = pattern.end_property(key, value);
    }

    // Execute fast-path pattern match
    let matches = graph.match_triples_fast(&pattern)?;

    // Convert TripleMatch to serializable format
    let matches_json: Vec<serde_json::Value> = matches.into_iter().map(|m| {
        json!({
            "start_id": m.start_id,
            "end_id": m.end_id,
            "edge_id": m.edge_id
        })
    }).collect();

    let payload = json!({
        "command": "pattern-match-fast",
        "edge_type": edge_type,
        "start_label": start_label,
        "end_label": end_label,
        "direction": direction_str,
        "match_count": matches_json.len(),
        "matches": matches_json
    });
    println!("{payload}");
    Ok(())
}

/// Helper function to get optional flag value
fn optional_flag_value(args: &[String], flag: &str) -> Option<String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == flag {
            return iter.next().cloned();
        }
    }
    None
}

fn run_wal_checkpoint(client: &BackendClient, _args: &[String]) -> Result<(), SqliteGraphError> {
    let backend = client.backend();

    backend.checkpoint()?;

    let payload = json!({
        "command": "wal-checkpoint",
        "status": "completed"
    });
    println!("{payload}");
    Ok(())
}

fn run_snapshot_create(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let backend = client.backend();

    let dir_str = required_flag_value(args, "--dir")?;
    let export_dir = PathBuf::from(&dir_str);

    let metadata = backend.snapshot_export(&export_dir)?;

    let payload = json!({
        "command": "snapshot-create",
        "snapshot_path": metadata.snapshot_path,
        "size_bytes": metadata.size_bytes,
        "entity_count": metadata.entity_count,
        "edge_count": metadata.edge_count,
        "status": "completed"
    });
    println!("{payload}");
    Ok(())
}

fn run_snapshot_load(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    let backend = client.backend();

    let dir_str = required_flag_value(args, "--dir")?;
    let import_dir = PathBuf::from(&dir_str);

    let metadata = backend.snapshot_import(&import_dir)?;

    let payload = json!({
        "command": "snapshot-load",
        "snapshot_path": metadata.snapshot_path,
        "entities_imported": metadata.entities_imported,
        "edges_imported": metadata.edges_imported,
        "status": "completed"
    });
    println!("{payload}");
    Ok(())
}

#[cfg(feature = "native-v2")]
fn run_wal_metrics(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    use std::fs;

    // Get database path from args or use a default
    let db_path_str = args.iter()
        .position(|arg| arg == "--db")
        .and_then(|idx| args.get(idx + 1))
        .map(|s| s.as_str())
        .unwrap_or("/tmp/test.db"); // Default for testing

    let db_path = std::path::Path::new(db_path_str);
    let wal_path = db_path.with_extension("wal");
    let checkpoint_path = db_path.with_extension("checkpoint");

    let mut metrics = serde_json::json!({
        "command": "wal-metrics",
        "database_path": db_path_str,
        "wal_file": wal_path.display().to_string(),
        "checkpoint_file": checkpoint_path.display().to_string(),
    });

    // Check if WAL file exists and get its size
    if wal_path.exists() {
        if let Ok(metadata) = fs::metadata(&wal_path) {
            metrics["wal_size_bytes"] = json!(metadata.len());
            metrics["wal_size_mb"] = json!(metadata.len() as f64 / 1_048_576.0);
            metrics["wal_exists"] = json!(true);
        }
    } else {
        metrics["wal_exists"] = json!(false);
    }

    // Check checkpoint file
    if checkpoint_path.exists() {
        if let Ok(metadata) = fs::metadata(&checkpoint_path) {
            metrics["checkpoint_size_bytes"] = json!(metadata.len());
        }
    }

    // Get WAL manager metrics if available (only for Native backend)
    if let Some(wal_metrics) = client.get_wal_metrics() {
        metrics["total_transactions"] = json!(wal_metrics.total_transactions);
        metrics["committed_transactions"] = json!(wal_metrics.committed_transactions);
        metrics["rolled_back_transactions"] = json!(wal_metrics.rolled_back_transactions);
        metrics["avg_transaction_duration_us"] = json!(wal_metrics.avg_transaction_duration_us);
        metrics["total_records_written"] = json!(wal_metrics.total_records_written);
        metrics["checkpoint_count"] = json!(wal_metrics.checkpoint_count);
        metrics["recovery_count"] = json!(wal_metrics.recovery_count);
        metrics["group_commit_batches"] = json!(wal_metrics.group_commit_batches);
        metrics["avg_group_commit_size"] = json!(wal_metrics.avg_group_commit_size);
        metrics["compression_ratio"] = json!(wal_metrics.compression_ratio);

        // Get active transaction count
        if let Some(active_count) = client.get_active_transaction_count() {
            metrics["active_transactions"] = json!(active_count);
        }
    } else {
        metrics["note"] = json!("WAL metrics not available - may not be a Native backend or WAL not initialized");
    }

    println!("{metrics}");
    Ok(())
}

#[cfg(not(feature = "native-v2"))]
fn run_wal_metrics(_client: &BackendClient, _args: &[String]) -> Result<(), SqliteGraphError> {
    let payload = json!({
        "command": "wal-metrics",
        "error": "WAL metrics require native-v2 feature",
        "status": "unsupported"
    });
    println!("{payload}");
    Ok(())
}

#[cfg(feature = "native-v2")]
fn run_wal_config(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    use sqlitegraph::V2WALConfig;

    // Get database path from args or use a default
    let db_path_str = args.iter()
        .position(|arg| arg == "--db")
        .and_then(|idx| args.get(idx + 1))
        .map(|s| s.as_str())
        .unwrap_or("/tmp/test.db"); // Default for testing

    let db_path = std::path::Path::new(db_path_str);
    let config = V2WALConfig::for_graph_file(db_path);

    let payload = json!({
        "command": "wal-config",
        "database_path": db_path_str,
        "graph_path": config.graph_path.display().to_string(),
        "wal_path": config.wal_path.display().to_string(),
        "checkpoint_path": config.checkpoint_path.display().to_string(),
        "max_wal_size": config.max_wal_size,
        "max_wal_size_mb": config.max_wal_size / 1_048_576,
        "buffer_size": config.buffer_size,
        "buffer_size_kb": config.buffer_size / 1024,
        "checkpoint_interval": config.checkpoint_interval,
        "group_commit_timeout_ms": config.group_commit_timeout_ms,
        "max_group_commit_size": config.max_group_commit_size,
        "enable_compression": config.enable_compression,
        "compression_level": config.compression_level,
    });
    println!("{payload}");
    Ok(())
}

#[cfg(not(feature = "native-v2"))]
fn run_wal_config(_client: &BackendClient, _args: &[String]) -> Result<(), SqliteGraphError> {
    let payload = json!({
        "command": "wal-config",
        "error": "WAL config requires native-v2 feature",
        "status": "unsupported"
    });
    println!("{payload}");
    Ok(())
}

#[cfg(feature = "native-v2")]
fn run_wal_stats(client: &BackendClient, args: &[String]) -> Result<(), SqliteGraphError> {
    use std::fs;

    // Get database path from args or use a default
    let db_path_str = args.iter()
        .position(|arg| arg == "--db")
        .and_then(|idx| args.get(idx + 1))
        .map(|s| s.as_str())
        .unwrap_or("/tmp/test.db");

    let db_path = std::path::Path::new(db_path_str);
    let wal_path = db_path.with_extension("wal");
    let checkpoint_path = db_path.with_extension("checkpoint");

    // Check if backend is Native and has WAL metrics
    let backend_type = client.backend_type();

    if backend_type != "native" {
        let payload = json!({
            "command": "wal-stats",
            "error": format!("wal-stats requires Native backend, got: {}", backend_type),
            "status": "unsupported"
        });
        println!("{payload}");
        return Ok(());
    }

    // Get WAL file info
    let wal_exists = wal_path.exists();
    let wal_size = if wal_exists {
        fs::metadata(&wal_path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    let checkpoint_exists = checkpoint_path.exists();
    let checkpoint_size = if checkpoint_exists {
        fs::metadata(&checkpoint_path).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    // Get WAL manager metrics
    let (metrics, active_count) = match client.get_wal_metrics() {
        Some(m) => (m, client.get_active_transaction_count().unwrap_or(0)),
        None => {
            let payload = json!({
                "command": "wal-stats",
                "error": "WAL metrics not available - WAL may not be initialized",
                "status": "unavailable"
            });
            println!("{payload}");
            return Ok(());
        }
    };

    // Calculate derived statistics
    let tx_success_rate = if metrics.total_transactions > 0 {
        (metrics.committed_transactions as f64 / metrics.total_transactions as f64) * 100.0
    } else {
        0.0
    };

    let tx_failure_rate = if metrics.total_transactions > 0 {
        (metrics.rolled_back_transactions as f64 / metrics.total_transactions as f64) * 100.0
    } else {
        0.0
    };

    let avg_records_per_tx = if metrics.committed_transactions > 0 {
        metrics.total_records_written as f64 / metrics.committed_transactions as f64
    } else {
        0.0
    };

    let avg_tx_duration_ms = if metrics.committed_transactions > 0 {
        metrics.avg_transaction_duration_us as f64 / 1000.0
    } else {
        0.0
    };

    // Build stats response
    let stats = json!({
        "command": "wal-stats",
        "backend": backend_type,
        "wal_file": wal_path.display().to_string(),
        "checkpoint_file": checkpoint_path.display().to_string(),

        // File Status
        "wal_status": {
            "exists": wal_exists,
            "size_bytes": wal_size,
            "size_mb": wal_size as f64 / 1_048_576.0
        },
        "checkpoint_status": {
            "exists": checkpoint_exists,
            "size_bytes": checkpoint_size,
            "size_mb": checkpoint_size as f64 / 1_048_576.0
        },

        // Transaction Statistics
        "transaction_stats": {
            "total": metrics.total_transactions,
            "committed": metrics.committed_transactions,
            "rolled_back": metrics.rolled_back_transactions,
            "active": active_count,
            "success_rate_percent": tx_success_rate,
            "failure_rate_percent": tx_failure_rate
        },

        // Performance Metrics
        "performance": {
            "avg_duration_ms": avg_tx_duration_ms,
            "avg_records_per_tx": avg_records_per_tx,
            "total_records_written": metrics.total_records_written,
            "throughput_tx_per_sec": if metrics.avg_transaction_duration_us > 0 {
                1_000_000.0 / metrics.avg_transaction_duration_us as f64
            } else {
                0.0
            }
        },

        // Checkpoint & Recovery
        "maintenance": {
            "checkpoint_count": metrics.checkpoint_count,
            "recovery_count": metrics.recovery_count,
            "requires_checkpoint": wal_size > (1024 * 1024 * 1024) // 1GB threshold
        },

        // Group Commit Statistics
        "group_commit": {
            "batches": metrics.group_commit_batches,
            "avg_batch_size": metrics.avg_group_commit_size,
            "total_transactions_grouped": if metrics.avg_group_commit_size > 0.0 && metrics.group_commit_batches > 0 {
                (metrics.avg_group_commit_size * metrics.group_commit_batches as f64) as u64
            } else {
                0
            }
        },

        // Compression
        "compression": {
            "enabled": metrics.compression_ratio < 1.0,
            "ratio": metrics.compression_ratio
        }
    });

    println!("{stats}");
    Ok(())
}

#[cfg(not(feature = "native-v2"))]
fn run_wal_stats(_client: &BackendClient, _args: &[String]) -> Result<(), SqliteGraphError> {
    let payload = json!({
        "command": "wal-stats",
        "error": "WAL stats require native-v2 feature",
        "status": "unsupported"
    });
    println!("{payload}");
    Ok(())
}

// Reindex functions removed - not available in v0.2.5
