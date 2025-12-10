//! BFS (Breadth-First Search) performance benchmarks for SQLite vs Native backends.
//!
//! Compares BFS traversal performance across different graph sizes and topologies
//! using the criterion benchmarking framework.

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use rand::SeedableRng;
use sqlitegraph::{BackendDirection, BackendKind, EdgeSpec, NeighborQuery, NodeSpec};

mod bench_utils;
use bench_utils::{
    BENCHMARK_SIZES, BenchmarkGraph, GraphTopology, MEASURE, WARM_UP, create_benchmark_temp_dir,
};

/// Benchmark BFS traversal on chain graphs
fn bfs_chain(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("bfs_chain");
    group.measurement_time(MEASURE);
    group.warm_up_time(WARM_UP);

    for &size in BENCHMARK_SIZES {
        // SQLite backend
        group.bench_with_input(BenchmarkId::new("sqlite", size), &size, |b, &size| {
            b.iter(|| {
                let temp_dir = create_benchmark_temp_dir();
                let db_path = temp_dir.path().join("benchmark.db");

                let graph = sqlitegraph::open_graph(&db_path, &sqlitegraph::GraphConfig::sqlite())
                    .expect("Failed to create graph");

                // Create chain graph using individual insertions
                let mut node_ids = Vec::new();
                for i in 0..size {
                    let node_id = graph
                        .insert_node(NodeSpec {
                            kind: "Node".to_string(),
                            name: format!("node_{}", i),
                            file_path: None,
                            data: serde_json::json!({"id": i}),
                        })
                        .expect("Failed to insert node");
                    node_ids.push(node_id);
                }

                // Create chain edges
                for i in 0..size - 1 {
                    graph
                        .insert_edge(EdgeSpec {
                            from: node_ids[i],
                            to: node_ids[i + 1],
                            edge_type: "chain".to_string(),
                            data: serde_json::json!({"order": i}),
                        })
                        .expect("Failed to insert edge");
                }

                // Perform BFS from first node
                let _bfs_result = graph
                    .bfs(node_ids[0], size as u32)
                    .expect("Failed to perform BFS");
            });
        });

        // Native backend
        group.bench_with_input(BenchmarkId::new("native", size), &size, |b, &size| {
            b.iter(|| {
                let temp_dir = create_benchmark_temp_dir();
                let db_path = temp_dir.path().join("benchmark.db");

                let graph = sqlitegraph::open_graph(&db_path, &sqlitegraph::GraphConfig::native())
                    .expect("Failed to create graph");

                // Create chain graph using individual insertions
                let mut node_ids = Vec::new();
                for i in 0..size {
                    let node_id = graph
                        .insert_node(NodeSpec {
                            kind: "Node".to_string(),
                            name: format!("node_{}", i),
                            file_path: None,
                            data: serde_json::json!({"id": i}),
                        })
                        .expect("Failed to insert node");
                    node_ids.push(node_id);
                }

                // Create chain edges
                for i in 0..size - 1 {
                    graph
                        .insert_edge(EdgeSpec {
                            from: node_ids[i],
                            to: node_ids[i + 1],
                            edge_type: "chain".to_string(),
                            data: serde_json::json!({"order": i}),
                        })
                        .expect("Failed to insert edge");
                }

                // Perform BFS from first node
                let _bfs_result = graph
                    .bfs(node_ids[0], size as u32)
                    .expect("Failed to perform BFS");
            });
        });
    }

    group.finish();
}

/// Benchmark BFS traversal on star graphs
fn bfs_star(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("bfs_star");
    group.measurement_time(MEASURE);
    group.warm_up_time(WARM_UP);

    for &size in BENCHMARK_SIZES {
        // SQLite backend
        group.bench_with_input(BenchmarkId::new("sqlite", size), &size, |b, &size| {
            b.iter(|| {
                let temp_dir = create_benchmark_temp_dir();
                let db_path = temp_dir.path().join("benchmark.db");

                let graph = sqlitegraph::open_graph(&db_path, &sqlitegraph::GraphConfig::sqlite())
                    .expect("Failed to create graph");

                // Create star graph using individual insertions
                let mut node_ids = Vec::new();
                for i in 0..size {
                    let node_id = graph
                        .insert_node(NodeSpec {
                            kind: "Node".to_string(),
                            name: format!("node_{}", i),
                            file_path: None,
                            data: serde_json::json!({"id": i}),
                        })
                        .expect("Failed to insert node");
                    node_ids.push(node_id);
                }

                // Create star edges (center node 0 connected to all others)
                for i in 1..size {
                    graph
                        .insert_edge(EdgeSpec {
                            from: node_ids[0],
                            to: node_ids[i],
                            edge_type: "star".to_string(),
                            data: serde_json::json!({"spoke": i}),
                        })
                        .expect("Failed to insert edge");
                }

                // Perform BFS from center node
                let _bfs_result = graph.bfs(node_ids[0], 2).expect("Failed to perform BFS");
            });
        });

        // Native backend
        group.bench_with_input(BenchmarkId::new("native", size), &size, |b, &size| {
            b.iter(|| {
                let temp_dir = create_benchmark_temp_dir();
                let db_path = temp_dir.path().join("benchmark.db");

                let graph = sqlitegraph::open_graph(&db_path, &sqlitegraph::GraphConfig::native())
                    .expect("Failed to create graph");

                // Create star graph using individual insertions
                let mut node_ids = Vec::new();
                for i in 0..size {
                    let node_id = graph
                        .insert_node(NodeSpec {
                            kind: "Node".to_string(),
                            name: format!("node_{}", i),
                            file_path: None,
                            data: serde_json::json!({"id": i}),
                        })
                        .expect("Failed to insert node");
                    node_ids.push(node_id);
                }

                // Create star edges (center node 0 connected to all others)
                for i in 1..size {
                    graph
                        .insert_edge(EdgeSpec {
                            from: node_ids[0],
                            to: node_ids[i],
                            edge_type: "star".to_string(),
                            data: serde_json::json!({"spoke": i}),
                        })
                        .expect("Failed to insert edge");
                }

                // Perform BFS from center node
                let _bfs_result = graph.bfs(node_ids[0], 2).expect("Failed to perform BFS");
            });
        });
    }

    group.finish();
}

/// Benchmark BFS traversal on random graphs
fn bfs_random(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("bfs_random");
    group.measurement_time(MEASURE);
    group.warm_up_time(WARM_UP);

    for &size in &[100, 1_000] {
        // Smaller sizes for random graphs
        let edge_count = size * 2; // 2x edges for random connectivity

        // SQLite backend
        group.bench_with_input(BenchmarkId::new("sqlite", size), &size, |b, &size| {
            b.iter(|| {
                let temp_dir = create_benchmark_temp_dir();
                let db_path = temp_dir.path().join("benchmark.db");

                let graph = sqlitegraph::open_graph(&db_path, &sqlitegraph::GraphConfig::sqlite())
                    .expect("Failed to create graph");

                // Create random graph using individual insertions
                let mut node_ids = Vec::new();
                for i in 0..size {
                    let node_id = graph
                        .insert_node(NodeSpec {
                            kind: "Node".to_string(),
                            name: format!("node_{}", i),
                            file_path: None,
                            data: serde_json::json!({"id": i}),
                        })
                        .expect("Failed to insert node");
                    node_ids.push(node_id);
                }

                // Create random edges
                use rand::RngCore;
                let mut rng = rand::rngs::StdRng::seed_from_u64(0x5F3759DF);

                for _ in 0..edge_count {
                    let from_idx = (rng.next_u32() as usize) % size;
                    let mut to_idx = (rng.next_u32() as usize) % size;
                    while to_idx == from_idx {
                        to_idx = (rng.next_u32() as usize) % size;
                    }

                    graph
                        .insert_edge(EdgeSpec {
                            from: node_ids[from_idx],
                            to: node_ids[to_idx],
                            edge_type: "random".to_string(),
                            data: serde_json::json!({"random_id": rng.next_u64()}),
                        })
                        .expect("Failed to insert edge");
                }

                // Perform BFS from first node
                let _bfs_result = graph.bfs(node_ids[0], 3).expect("Failed to perform BFS");
            });
        });

        // Native backend
        group.bench_with_input(BenchmarkId::new("native", size), &size, |b, &size| {
            b.iter(|| {
                let temp_dir = create_benchmark_temp_dir();
                let db_path = temp_dir.path().join("benchmark.db");

                let graph = sqlitegraph::open_graph(&db_path, &sqlitegraph::GraphConfig::native())
                    .expect("Failed to create graph");

                // Create random graph using individual insertions
                let mut node_ids = Vec::new();
                for i in 0..size {
                    let node_id = graph
                        .insert_node(NodeSpec {
                            kind: "Node".to_string(),
                            name: format!("node_{}", i),
                            file_path: None,
                            data: serde_json::json!({"id": i}),
                        })
                        .expect("Failed to insert node");
                    node_ids.push(node_id);
                }

                // Create random edges
                use rand::RngCore;
                let mut rng = rand::rngs::StdRng::seed_from_u64(0x5F3759DF);

                for _ in 0..edge_count {
                    let from_idx = (rng.next_u32() as usize) % size;
                    let mut to_idx = (rng.next_u32() as usize) % size;
                    while to_idx == from_idx {
                        to_idx = (rng.next_u32() as usize) % size;
                    }

                    graph
                        .insert_edge(EdgeSpec {
                            from: node_ids[from_idx],
                            to: node_ids[to_idx],
                            edge_type: "random".to_string(),
                            data: serde_json::json!({"random_id": rng.next_u64()}),
                        })
                        .expect("Failed to insert edge");
                }

                // Perform BFS from first node
                let _bfs_result = graph.bfs(node_ids[0], 3).expect("Failed to perform BFS");
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bfs_chain, bfs_star, bfs_random);
criterion_main!(benches);
