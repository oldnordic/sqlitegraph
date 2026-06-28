use anyhow::{Context, Result};
use clap::Parser;
use serde_json::json;
use sqlitegraph_cli::cli::{AlgoCommands, Cli, Commands, Direction, HnswCommands};

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Check for write operations without --write flag
    if !cli.write {
        match &cli.command {
            Commands::Import { .. } | Commands::Insert { .. } | Commands::Export { .. } => {
                anyhow::bail!(
                    "This command modifies the database. Use --write flag to enable write mode"
                );
            }
            Commands::Hnsw { command } => match command {
                HnswCommands::Create { .. }
                | HnswCommands::Insert { .. }
                | HnswCommands::Delete { .. } => {
                    anyhow::bail!(
                        "This command modifies the database. Use --write flag to enable write mode"
                    );
                }
                HnswCommands::Search { .. } | HnswCommands::List => {}
            },
            _ => {}
        }
    }

    // Open client
    let client = if cli.db.to_string_lossy() == ":memory:" {
        sqlitegraph_cli::client::CliClient::open_in_memory(cli.backend)?
    } else {
        sqlitegraph_cli::client::CliClient::open(cli.backend, &cli.db)?
    };

    // Execute command
    match cli.command {
        Commands::Query { query } => run_query(&client, &query),
        Commands::Status { compact } => run_status(&client, compact),
        Commands::List { kind } => run_list(&client, kind),
        Commands::Bfs { start, depth } => run_bfs(&client, start, depth),
        Commands::Path { from, to } => run_path(&client, from, to),
        Commands::Neighbors { id, direction } => run_neighbors(&client, id, direction),
        Commands::Algo { command } => run_algo(&client, command),
        Commands::Export { output } => run_export(&client, &output),
        Commands::Import { input } => run_import(&client, &input),
        Commands::Insert { kind, name, data } => run_insert(&client, &kind, &name, data),
        Commands::Hnsw { command } => run_hnsw(&client, command),
    }
}

fn run_query(client: &sqlitegraph_cli::client::CliClient, query_str: &str) -> Result<()> {
    match client.backend_type() {
        sqlitegraph_cli::cli::BackendType::Sqlite => run_query_sqlite(client, query_str),
        #[cfg(feature = "native-v3")]
        sqlitegraph_cli::cli::BackendType::V3 => run_query_v3(client, query_str),
    }
}

fn run_query_sqlite(client: &sqlitegraph_cli::client::CliClient, query_str: &str) -> Result<()> {
    let sqlite_backend = client
        .sqlite_backend()
        .context("Cypher queries require SQLite backend")?;
    let result = sqlitegraph_cli::query::run(sqlite_backend, query_str)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

#[cfg(feature = "native-v3")]
fn run_query_v3(client: &sqlitegraph_cli::client::CliClient, query_str: &str) -> Result<()> {
    let v3_backend = client
        .v3_backend()
        .context("Cypher queries require native-v3 backend")?;
    let result = sqlitegraph_cli::query::run(v3_backend, query_str)?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    Ok(())
}

fn run_status(client: &sqlitegraph_cli::client::CliClient, compact: bool) -> Result<()> {
    use sqlitegraph::introspection::GraphIntrospection;

    // Get comprehensive introspection data
    let intro = if let Some(graph) = client.sqlite_graph() {
        graph.introspect()?
    } else {
        // For non-SQLite backends, use basic info
        GraphIntrospection {
            backend_type: client.backend_name().to_string(),
            node_count: client.node_count()?,
            edge_count: sqlitegraph::introspection::EdgeCount::Unavailable,
            cache_stats: sqlitegraph::cache::CacheStats {
                hits: 0,
                misses: 0,
                entries: 0,
            },
            memory_usage: None,
            file_size: None,
            wal_size: None,
            is_in_memory: false,
        }
    };

    if compact {
        // Compact format: single line, no pretty-print
        println!(
            "{}",
            serde_json::to_string(&json!({
                "backend": intro.backend_type,
                "nodes": intro.node_count,
                "edges": intro.edge_count.value(),
                "file_size_mb": intro.file_size.map(|s| s / 1_048_576),
                "is_in_memory": intro.is_in_memory,
            }))?
        );
    } else {
        // Pretty-print format
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "backend": intro.backend_type,
                "nodes": intro.node_count,
                "edges": intro.edge_count.value(),
                "cache_hit_ratio": intro.cache_stats.hit_ratio(),
                "file_size_mb": intro.file_size.map(|s| s / 1_048_576),
                "wal_size_mb": intro.wal_size.map(|s| s / 1_048_576),
                "is_in_memory": intro.is_in_memory,
            }))?
        );
    }
    Ok(())
}

fn run_list(client: &sqlitegraph_cli::client::CliClient, kind: Option<String>) -> Result<()> {
    use sqlitegraph::snapshot::SnapshotId;

    let backend = client.backend();
    let node_ids = backend.entity_ids()?;
    let snapshot = SnapshotId::current();

    let mut nodes = Vec::new();
    for id in node_ids {
        if let Ok(node) = backend.get_node(snapshot, id) {
            if let Some(ref filter_kind) = kind {
                if node.kind != *filter_kind {
                    continue;
                }
            }
            nodes.push(json!({
                "id": node.id,
                "kind": node.kind,
                "name": node.name,
            }));
        }
    }

    println!("{}", serde_json::to_string_pretty(&nodes)?);
    Ok(())
}

fn run_bfs(client: &sqlitegraph_cli::client::CliClient, start: i64, depth: u32) -> Result<()> {
    use sqlitegraph::bfs::bfs_neighbors;

    let graph = client
        .sqlite_graph()
        .context("BFS requires SQLite backend")?;

    let visited = bfs_neighbors(graph, start, depth)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "start": start,
            "max_depth": depth,
            "visited": visited,
            "count": visited.len(),
        }))?
    );
    Ok(())
}

fn run_path(client: &sqlitegraph_cli::client::CliClient, from: i64, to: i64) -> Result<()> {
    use sqlitegraph::bfs::shortest_path;

    let graph = client
        .sqlite_graph()
        .context("Shortest path requires SQLite backend")?;

    let path = shortest_path(graph, from, to)?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "from": from,
            "to": to,
            "path": path,
            "found": path.is_some(),
        }))?
    );
    Ok(())
}

fn run_neighbors(
    client: &sqlitegraph_cli::client::CliClient,
    id: i64,
    direction: Direction,
) -> Result<()> {
    let graph = client
        .sqlite_graph()
        .context("Neighbors requires SQLite backend")?;

    let query = graph.query();
    let neighbors = match direction {
        Direction::Incoming => query.incoming(id)?,
        Direction::Outgoing => query.outgoing(id)?,
        Direction::Both => {
            let mut n = query.incoming(id)?;
            n.extend(query.outgoing(id)?);
            n
        }
    };

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "id": id,
            "direction": format!("{:?}", direction),
            "neighbors": neighbors,
            "count": neighbors.len(),
        }))?
    );
    Ok(())
}

fn run_algo(client: &sqlitegraph_cli::client::CliClient, command: AlgoCommands) -> Result<()> {
    use sqlitegraph::algo::*;
    use sqlitegraph::progress::ConsoleProgress;

    let graph = client
        .sqlite_graph()
        .context("Algorithms require SQLite backend")?;
    let progress = ConsoleProgress::new();

    match command {
        AlgoCommands::Pagerank { iterations } => {
            let scores = pagerank_with_progress(graph, 0.85, iterations, &progress)?;
            let top: Vec<_> = scores
                .iter()
                .take(10)
                .map(|(id, score)| json!({"id": id, "score": score}))
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "algorithm": "pagerank",
                    "iterations": iterations,
                    "top_scores": top,
                }))?
            );
        }
        AlgoCommands::Betweenness => {
            let scores = betweenness_centrality_with_progress(graph, &progress)?;
            let top: Vec<_> = scores
                .iter()
                .take(10)
                .map(|(id, score)| json!({"id": id, "score": score}))
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "algorithm": "betweenness",
                    "top_scores": top,
                }))?
            );
        }
        AlgoCommands::Components => {
            let components = weakly_connected_components_with_progress(graph, &progress)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "algorithm": "wcc",
                    "component_count": components.len(),
                    "components": components.iter().take(10).collect::<Vec<_>>(),
                }))?
            );
        }
        AlgoCommands::Topo => {
            let order = topological_sort(graph)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "algorithm": "topological_sort",
                    "node_count": order.len(),
                    "order": order.iter().take(100).collect::<Vec<_>>(),
                }))?
            );
        }
        AlgoCommands::Scc => {
            let result = strongly_connected_components(graph)?;
            let components_out: Vec<Vec<i64>> = result
                .components
                .iter()
                .map(|c| {
                    let mut ids: Vec<i64> = c.iter().copied().collect();
                    ids.sort_unstable();
                    ids
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "algorithm": "scc",
                    "component_count": components_out.len(),
                    "components": components_out.iter().take(10).collect::<Vec<_>>(),
                }))?
            );
        }
        AlgoCommands::Louvain { max_iterations } => {
            let communities = louvain_communities(graph, max_iterations)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "algorithm": "louvain",
                    "max_iterations": max_iterations,
                    "community_count": communities.len(),
                    "communities": communities.iter().take(10).collect::<Vec<_>>(),
                }))?
            );
        }
        AlgoCommands::LabelProp { max_iterations } => {
            let communities = label_propagation(graph, max_iterations)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "algorithm": "label_propagation",
                    "max_iterations": max_iterations,
                    "community_count": communities.len(),
                    "communities": communities.iter().take(10).collect::<Vec<_>>(),
                }))?
            );
        }
        AlgoCommands::Cycles { limit } => {
            let cycles = find_cycles_limited(graph, limit)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "algorithm": "cycles",
                    "limit": limit,
                    "cycle_count": cycles.len(),
                    "cycles": cycles.iter().take(10).collect::<Vec<_>>(),
                }))?
            );
        }
        AlgoCommands::Dominators { entry } => {
            let result = dominators(graph, entry)?;
            let idom_out: serde_json::Map<String, serde_json::Value> = result
                .idom
                .iter()
                .map(|(node, parent)| {
                    (
                        node.to_string(),
                        match parent {
                            Some(p) => json!(p),
                            None => json!(null),
                        },
                    )
                })
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "algorithm": "dominators",
                    "entry": entry,
                    "node_count": result.idom.len(),
                    "idom": idom_out,
                }))?
            );
        }
        AlgoCommands::CriticalPath => {
            // Uniform edge weights: critical path == longest path by edge count.
            let weight_fn: &WeightCallback = &default_weight_fn;
            let result = critical_path(graph, weight_fn)?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "algorithm": "critical_path",
                    "path_length": result.path.len(),
                    "distance": result.distance,
                    "path": result.path,
                }))?
            );
        }
    }

    Ok(())
}

fn run_export(client: &sqlitegraph_cli::client::CliClient, output: &std::path::Path) -> Result<()> {
    use sqlitegraph::recovery::dump_graph_to_path;

    let graph = client
        .sqlite_graph()
        .context("Export requires SQLite backend")?;

    dump_graph_to_path(graph, output)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "exported_to": output,
            "status": "ok",
        }))?
    );
    Ok(())
}

fn run_import(client: &sqlitegraph_cli::client::CliClient, input: &std::path::Path) -> Result<()> {
    use sqlitegraph::recovery::load_graph_from_path;

    let graph = client
        .sqlite_graph()
        .context("Import requires SQLite backend")?;

    load_graph_from_path(graph, input)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "imported_from": input,
            "status": "ok",
        }))?
    );
    Ok(())
}

fn run_insert(
    client: &sqlitegraph_cli::client::CliClient,
    kind: &str,
    name: &str,
    data: Option<String>,
) -> Result<()> {
    use sqlitegraph::backend::NodeSpec;

    let backend = client.backend();
    let data_json = match data {
        Some(s) => serde_json::from_str(&s).unwrap_or_else(|_| serde_json::json!({"data": s})),
        None => serde_json::json!({}),
    };

    let id = backend.insert_node(NodeSpec {
        kind: kind.to_string(),
        name: name.to_string(),
        file_path: None,
        data: data_json,
    })?;

    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "id": id,
            "kind": kind,
            "name": name,
            "status": "created",
        }))?
    );
    Ok(())
}

fn run_hnsw(client: &sqlitegraph_cli::client::CliClient, command: HnswCommands) -> Result<()> {
    use sqlitegraph::hnsw::config::HnswConfig;
    use sqlitegraph::hnsw::distance_metric::DistanceMetric;

    let graph = client
        .sqlite_graph()
        .context("HNSW operations require SQLite backend")?;

    match command {
        HnswCommands::Create {
            name,
            dim,
            metric,
            m,
            ef_construction,
        } => {
            let dist = match metric.as_str() {
                "cosine" => DistanceMetric::Cosine,
                "euclidean" => DistanceMetric::Euclidean,
                "dot" => DistanceMetric::DotProduct,
                other => anyhow::bail!(
                    "unknown distance metric `{other}` (expected: cosine, euclidean, dot)"
                ),
            };
            let config = HnswConfig::new(dim, m, ef_construction, dist);
            let _guard = graph
                .hnsw_index_persistent(&name, config)
                .context("create HNSW index")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "operation": "create",
                    "name": name,
                    "dim": dim,
                    "metric": metric,
                    "m": m,
                    "ef_construction": ef_construction,
                    "status": "created",
                }))?
            );
        }
        HnswCommands::Insert {
            name,
            vector,
            metadata,
        } => {
            let vec = parse_vector_string(&vector)?;
            let meta: Option<serde_json::Value> = match metadata {
                Some(m) => Some(
                    serde_json::from_str(&m)
                        .with_context(|| format!("parse metadata JSON `{m}`"))?,
                ),
                None => None,
            };
            let id = graph
                .get_hnsw_index_mut(&name, |idx| idx.insert_vector(&vec, meta))
                .context("look up HNSW index")?
                .context("insert vector")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "operation": "insert",
                    "name": name,
                    "id": id,
                }))?
            );
        }
        HnswCommands::Search { name, k, vector } => {
            let vec = parse_vector_string(&vector)?;
            let results = graph
                .get_hnsw_index_ref(&name, |idx| idx.search(&vec, k))
                .context("look up HNSW index")?
                .context("HNSW search")?;
            let rows: Vec<serde_json::Value> = results
                .iter()
                .map(|(id, score)| json!({"id": id, "score": score}))
                .collect();
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "operation": "search",
                    "name": name,
                    "k": k,
                    "count": rows.len(),
                    "results": rows,
                }))?
            );
        }
        HnswCommands::List => {
            let names = graph.list_hnsw_indexes().context("list HNSW indexes")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "operation": "list",
                    "count": names.len(),
                    "indexes": names,
                }))?
            );
        }
        HnswCommands::Delete { name } => {
            graph
                .delete_hnsw_index(&name)
                .context("delete HNSW index")?;
            println!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "operation": "delete",
                    "name": name,
                    "status": "deleted",
                }))?
            );
        }
    }

    Ok(())
}

/// Parse a comma-separated string of floats into a `Vec<f32>`.
fn parse_vector_string(s: &str) -> Result<Vec<f32>> {
    let mut out = Vec::new();
    for part in s.split(',') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        let v: f32 = p
            .parse()
            .with_context(|| format!("invalid float `{p}` in vector"))?;
        out.push(v);
    }
    if out.is_empty() {
        anyhow::bail!("vector is empty");
    }
    Ok(out)
}
