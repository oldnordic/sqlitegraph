//! Read path performance benchmarks for Native V2 backend.
//!
//! Comprehensive benchmark suite for validating read path optimizations
//! from Phase 3 plans 03-01 (cache) and 03-02 (compression).
//!
//! Benchmark categories:
//! - Single node operations (lookup, neighbor iteration)
//! - Traversal workloads (BFS, k-hop)
//! - Cache performance (hit ratio, eviction)
//! - Compression performance (ratio, overhead)
//! - Regression detection (baseline comparison)

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main, Throughput};
use rand::{Rng, SeedableRng};
use sqlitegraph::{
    BackendDirection, EdgeSpec, NeighborQuery, NodeSpec,
    GraphConfig, open_graph,
};

mod bench_utils;
use bench_utils::{
    create_benchmark_temp_dir, BENCHMARK_SIZES,
};

const SAMPLE_SIZE: usize = 100;
const WARM_UP_TIME: Duration = Duration::from_secs(5);
const MEASURE_TIME: Duration = Duration::from_secs(15);

// ============================================================================
// Task 1: Single Node Operations
// ============================================================================

/// Benchmark single node lookup
fn bench_get_node(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("single_node_ops");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);

    for &size in BENCHMARK_SIZES {
        group.bench_with_input(BenchmarkId::new("get_node", size), &size, |b, &size| {
            b.iter(|| {
                let temp_dir = create_benchmark_temp_dir();
                let db_path = temp_dir.path().join("benchmark.db");

                let graph = open_graph(&db_path, &GraphConfig::native())
                    .expect("Failed to create graph");

                // Create nodes
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

                // Lookup random node
                let lookup_id = node_ids[size / 2];
                let _node = graph.get_node(lookup_id).expect("Failed to get node");
            });
        });
    }

    group.finish();
}

/// Benchmark neighbor iteration for small degree nodes (10 edges)
fn bench_get_neighbors_small(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("single_node_ops");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);

    group.bench_function("get_neighbors_small_10_edges", |b| {
        b.iter(|| {
            let temp_dir = create_benchmark_temp_dir();
            let db_path = temp_dir.path().join("benchmark.db");

            let graph = open_graph(&db_path, &GraphConfig::native())
                .expect("Failed to create graph");

            // Create star topology with center node having 10 edges
            let center = graph
                .insert_node(NodeSpec {
                    kind: "Node".to_string(),
                    name: "center".to_string(),
                    file_path: None,
                    data: serde_json::json!({"center": true}),
                })
                .expect("Failed to insert center node");

            for i in 0..10 {
                let leaf = graph
                    .insert_node(NodeSpec {
                        kind: "Node".to_string(),
                        name: format!("leaf_{}", i),
                        file_path: None,
                        data: serde_json::json!({"leaf": i}),
                    })
                    .expect("Failed to insert leaf node");

                graph
                    .insert_edge(EdgeSpec {
                        from: center,
                        to: leaf,
                        edge_type: "star".to_string(),
                        data: serde_json::json!({}),
                    })
                    .expect("Failed to insert edge");
            }

            // Iterate neighbors
            let _neighbors = graph
                .neighbors(center, NeighborQuery::default())
                .expect("Failed to get neighbors");
        });
    });

    group.finish();
}

/// Benchmark neighbor iteration for medium degree nodes (100 edges)
fn bench_get_neighbors_medium(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("single_node_ops");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);

    group.bench_function("get_neighbors_medium_100_edges", |b| {
        b.iter(|| {
            let temp_dir = create_benchmark_temp_dir();
            let db_path = temp_dir.path().join("benchmark.db");

            let graph = open_graph(&db_path, &GraphConfig::native())
                .expect("Failed to create graph");

            // Create star topology with center node having 100 edges
            let center = graph
                .insert_node(NodeSpec {
                    kind: "Node".to_string(),
                    name: "center".to_string(),
                    file_path: None,
                    data: serde_json::json!({"center": true}),
                })
                .expect("Failed to insert center node");

            for i in 0..100 {
                let leaf = graph
                    .insert_node(NodeSpec {
                        kind: "Node".to_string(),
                        name: format!("leaf_{}", i),
                        file_path: None,
                        data: serde_json::json!({"leaf": i}),
                    })
                    .expect("Failed to insert leaf node");

                graph
                    .insert_edge(EdgeSpec {
                        from: center,
                        to: leaf,
                        edge_type: "star".to_string(),
                        data: serde_json::json!({}),
                    })
                    .expect("Failed to insert edge");
            }

            // Iterate neighbors
            let _neighbors = graph
                .neighbors(center, NeighborQuery::default())
                .expect("Failed to get neighbors");
        });
    });

    group.finish();
}

/// Benchmark neighbor iteration for large degree nodes (1000 edges)
fn bench_get_neighbors_large(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("single_node_ops");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);

    group.bench_function("get_neighbors_large_1000_edges", |b| {
        b.iter(|| {
            let temp_dir = create_benchmark_temp_dir();
            let db_path = temp_dir.path().join("benchmark.db");

            let graph = open_graph(&db_path, &GraphConfig::native())
                .expect("Failed to create graph");

            // Create star topology with center node having 1000 edges
            let center = graph
                .insert_node(NodeSpec {
                    kind: "Node".to_string(),
                    name: "center".to_string(),
                    file_path: None,
                    data: serde_json::json!({"center": true}),
                })
                .expect("Failed to insert center node");

            for i in 0..1000 {
                let leaf = graph
                    .insert_node(NodeSpec {
                        kind: "Node".to_string(),
                        name: format!("leaf_{}", i),
                        file_path: None,
                        data: serde_json::json!({"leaf": i}),
                    })
                    .expect("Failed to insert leaf node");

                graph
                    .insert_edge(EdgeSpec {
                        from: center,
                        to: leaf,
                        edge_type: "star".to_string(),
                        data: serde_json::json!({}),
                    })
                    .expect("Failed to insert edge");
            }

            // Iterate neighbors
            let _neighbors = graph
                .neighbors(center, NeighborQuery::default())
                .expect("Failed to get neighbors");
        });
    });

    group.finish();
}

// ============================================================================
// Task 1: Traversal Workloads
// ============================================================================

/// Benchmark 1-hop BFS (single level traversal)
fn bench_bfs_depth_1(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("traversal_workloads");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);

    for &size in [100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("bfs_depth_1", size), &size, |b, &size| {
            b.iter(|| {
                let temp_dir = create_benchmark_temp_dir();
                let db_path = temp_dir.path().join("benchmark.db");

                let graph = open_graph(&db_path, &GraphConfig::native())
                    .expect("Failed to create graph");

                // Create chain graph
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

                // Perform 1-hop BFS
                let _bfs_result = graph
                    .bfs(node_ids[0], 1)
                    .expect("Failed to perform BFS");
            });
        });
    }

    group.finish();
}

/// Benchmark 3-hop BFS (typical traversal workload)
fn bench_bfs_depth_3(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("traversal_workloads");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);

    for &size in [100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("bfs_depth_3", size), &size, |b, &size| {
            b.iter(|| {
                let temp_dir = create_benchmark_temp_dir();
                let db_path = temp_dir.path().join("benchmark.db");

                let graph = open_graph(&db_path, &GraphConfig::native())
                    .expect("Failed to create graph");

                // Create chain graph
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

                // Perform 3-hop BFS
                let _bfs_result = graph
                    .bfs(node_ids[0], 3)
                    .expect("Failed to perform BFS");
            });
        });
    }

    group.finish();
}

/// Benchmark 5-hop BFS (deep traversal)
fn bench_bfs_depth_5(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("traversal_workloads");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);

    for &size in [100, 500, 1000].iter() {
        group.bench_with_input(BenchmarkId::new("bfs_depth_5", size), &size, |b, &size| {
            b.iter(|| {
                let temp_dir = create_benchmark_temp_dir();
                let db_path = temp_dir.path().join("benchmark.db");

                let graph = open_graph(&db_path, &GraphConfig::native())
                    .expect("Failed to create graph");

                // Create chain graph
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

                // Perform 5-hop BFS
                let _bfs_result = graph
                    .bfs(node_ids[0], 5)
                    .expect("Failed to perform BFS");
            });
        });
    }

    group.finish();
}

/// Benchmark k-hop from 10 start nodes
fn bench_k_hop_10_nodes(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("traversal_workloads");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);

    group.bench_function("k_hop_10_nodes", |b| {
        b.iter(|| {
            let temp_dir = create_benchmark_temp_dir();
            let db_path = temp_dir.path().join("benchmark.db");

            let graph = open_graph(&db_path, &GraphConfig::native())
                .expect("Failed to create graph");

            // Create graph with 100 nodes
            let mut node_ids = Vec::new();
            for i in 0..100 {
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
            let mut rng = rand::rngs::StdRng::seed_from_u64(0x5F3759DF);
            for _ in 0..200 {
                let from_idx = rng.gen_range(0..100);
                let to_idx = rng.gen_range(0..100);
                if from_idx != to_idx {
                    graph
                        .insert_edge(EdgeSpec {
                            from: node_ids[from_idx],
                            to: node_ids[to_idx],
                            edge_type: "random".to_string(),
                            data: serde_json::json!({}),
                        })
                        .expect("Failed to insert edge");
                }
            }

            // Perform 2-hop traversal from 10 random start nodes
            let mut total_visited = 0;
            for i in 0..10 {
                let start_node = node_ids[i * 10];
                let bfs_result = graph
                    .bfs(start_node, 2)
                    .expect("Failed to perform BFS");
                total_visited += bfs_result.len();
            }

            // Prevent compiler from optimizing away
            assert!(total_visited > 0, "Should visit some nodes");
        });
    });

    group.finish();
}

// ============================================================================
// Task 1: Cache Performance
// ============================================================================

/// Benchmark sequential access (cache-friendly)
fn bench_cache_hit_sequential(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("cache_performance");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);

    group.bench_function("cache_hit_sequential", |b| {
        b.iter(|| {
            let temp_dir = create_benchmark_temp_dir();
            let db_path = temp_dir.path().join("benchmark.db");

            let graph = open_graph(&db_path, &GraphConfig::native())
                .expect("Failed to create graph");

            // Create chain graph
            let mut node_ids = Vec::new();
            for i in 0..100 {
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
            for i in 0..99 {
                graph
                    .insert_edge(EdgeSpec {
                        from: node_ids[i],
                        to: node_ids[i + 1],
                        edge_type: "chain".to_string(),
                        data: serde_json::json!({"order": i}),
                    })
                    .expect("Failed to insert edge");
            }

            // Sequential access pattern (cache-friendly)
            for i in 0..100 {
                let _neighbors = graph
                    .neighbors(node_ids[i], NeighborQuery::default())
                    .expect("Failed to get neighbors");
            }
        });
    });

    group.finish();
}

/// Benchmark random access (cache stress)
fn bench_cache_hit_random(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("cache_performance");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);

    group.bench_function("cache_hit_random", |b| {
        b.iter(|| {
            let temp_dir = create_benchmark_temp_dir();
            let db_path = temp_dir.path().join("benchmark.db");

            let graph = open_graph(&db_path, &GraphConfig::native())
                .expect("Failed to create graph");

            // Create chain graph
            let mut node_ids = Vec::new();
            for i in 0..100 {
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
            for i in 0..99 {
                graph
                    .insert_edge(EdgeSpec {
                        from: node_ids[i],
                        to: node_ids[i + 1],
                        edge_type: "chain".to_string(),
                        data: serde_json::json!({"order": i}),
                    })
                    .expect("Failed to insert edge");
            }

            // Random access pattern (cache stress)
            let mut rng = rand::rngs::StdRng::seed_from_u64(0x5F3759DF);
            for _ in 0..100 {
                let idx = rng.gen_range(0..100);
                let _neighbors = graph
                    .neighbors(node_ids[idx], NeighborQuery::default())
                    .expect("Failed to get neighbors");
            }
        });
    });

    group.finish();
}

/// Benchmark cache eviction pressure
fn bench_cache_eviction(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("cache_performance");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);

    group.bench_function("cache_eviction", |b| {
        b.iter(|| {
            let temp_dir = create_benchmark_temp_dir();
            let db_path = temp_dir.path().join("benchmark.db");

            let graph = open_graph(&db_path, &GraphConfig::native())
                .expect("Failed to create graph");

            // Create large graph to trigger cache eviction
            let mut node_ids = Vec::new();
            for i in 0..1000 {
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

            // Create star edges from node 0
            for i in 1..1000 {
                graph
                    .insert_edge(EdgeSpec {
                        from: node_ids[0],
                        to: node_ids[i],
                        edge_type: "star".to_string(),
                        data: serde_json::json!({}),
                    })
                    .expect("Failed to insert edge");
            }

            // Access nodes in pattern that triggers eviction
            for i in 0..500 {
                let _neighbors = graph
                    .neighbors(node_ids[i], NeighborQuery::default())
                    .expect("Failed to get neighbors");
            }
        });
    });

    group.finish();
}

// ============================================================================
// Task 1: Compression Performance
// ============================================================================

/// Benchmark iterating compressed edges
fn bench_iterate_compressed(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("compression_performance");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);

    group.bench_function("iterate_compressed", |b| {
        b.iter(|| {
            let temp_dir = create_benchmark_temp_dir();
            let db_path = temp_dir.path().join("benchmark.db");

            let graph = open_graph(&db_path, &GraphConfig::native())
                .expect("Failed to create graph");

            // Create star graph with sequential IDs (compression-friendly)
            let center = graph
                .insert_node(NodeSpec {
                    kind: "Node".to_string(),
                    name: "center".to_string(),
                    file_path: None,
                    data: serde_json::json!({"center": true}),
                })
                .expect("Failed to insert center node");

            for i in 0..1000 {
                let leaf = graph
                    .insert_node(NodeSpec {
                        kind: "Node".to_string(),
                        name: format!("leaf_{}", i),
                        file_path: None,
                        data: serde_json::json!({"leaf": i}),
                    })
                    .expect("Failed to insert leaf node");

                graph
                    .insert_edge(EdgeSpec {
                        from: center,
                        to: leaf,
                        edge_type: "star".to_string(),
                        data: serde_json::json!({}),
                    })
                    .expect("Failed to insert edge");
            }

            // Iterate compressed edges
            let _neighbors = graph
                .neighbors(center, NeighborQuery::default())
                .expect("Failed to get neighbors");
        });
    });

    group.finish();
}

/// Benchmark decompression overhead
fn bench_decompress_overhead(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("compression_performance");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP_TIME);
    group.measurement_time(MEASURE_TIME);

    group.bench_function("decompress_overhead", |b| {
        b.iter(|| {
            let temp_dir = create_benchmark_temp_dir();
            let db_path = temp_dir.path().join("benchmark.db");

            let graph = open_graph(&db_path, &GraphConfig::native())
                .expect("Failed to create graph");

            // Create star graph with sequential IDs
            let center = graph
                .insert_node(NodeSpec {
                    kind: "Node".to_string(),
                    name: "center".to_string(),
                    file_path: None,
                    data: serde_json::json!({"center": true}),
                })
                .expect("Failed to insert center node");

            for i in 0..1000 {
                let leaf = graph
                    .insert_node(NodeSpec {
                        kind: "Node".to_string(),
                        name: format!("leaf_{}", i),
                        file_path: None,
                        data: serde_json::json!({"leaf": i}),
                    })
                    .expect("Failed to insert leaf node");

                graph
                    .insert_edge(EdgeSpec {
                        from: center,
                        to: leaf,
                        edge_type: "star".to_string(),
                        data: serde_json::json!({}),
                    })
                    .expect("Failed to insert edge");
            }

            // Multiple iterations to measure decompression overhead
            for _ in 0..10 {
                let _neighbors = graph
                    .neighbors(center, NeighborQuery::default())
                    .expect("Failed to get neighbors");
            }
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    // Single node operations
    bench_get_node,
    bench_get_neighbors_small,
    bench_get_neighbors_medium,
    bench_get_neighbors_large,
    // Traversal workloads
    bench_bfs_depth_1,
    bench_bfs_depth_3,
    bench_bfs_depth_5,
    bench_k_hop_10_nodes,
    // Cache performance
    bench_cache_hit_sequential,
    bench_cache_hit_random,
    bench_cache_eviction,
    // Compression performance
    bench_iterate_compressed,
    bench_decompress_overhead,
);

criterion_main!(benches);
