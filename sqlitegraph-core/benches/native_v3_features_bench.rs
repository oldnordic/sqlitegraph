//! Native-v3 Feature Benchmarks
//!
//! Benchmarks ALL native-v3 features per NATIVE_V3_SPEC.md:
//! - SQL Layer (property queries)
//! - Graph Topology (CSR neighbor queries)
//! - HNSW (vector similarity search)
//! - MVCC (snapshot read overhead)
//! - PubSub (event delivery latency)
//!
//! Run with: cargo bench --features native-v3 --bench native_v3_features

use criterion::{Criterion, Throughput, black_box, criterion_group, criterion_main};
use std::time::Duration;

use sqlitegraph::backend::{EdgeSpec, GraphBackend, NodeSpec, SubscriptionFilter};
use sqlitegraph::snapshot::SnapshotId;

mod bench_utils;
use bench_utils::create_v3_bench_context;

/// Benchmark: SQL Layer - Node property lookup
pub fn bench_sql_node_properties(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_node_properties");
    group.measurement_time(Duration::from_secs(4));
    group.sample_size(10);

    for count in [100usize, 1000, 10000] {
        group.throughput(Throughput::Elements(count as u64));
        let name = format!("{}", count);

        group.bench_function(name, |b| {
            b.iter_batched(
                || {
                    let ctx = create_v3_bench_context("sql_props.db");
                    let mut node_ids = Vec::new();

                    for i in 0..count {
                        let node_id = ctx
                            .backend
                            .insert_node(NodeSpec {
                                kind: "User".to_string(),
                                name: format!("user_{}", i),
                                file_path: None,
                                data: serde_json::json!({"index": i, "role": "member"}),
                            })
                            .expect("Failed to insert node");
                        node_ids.push(node_id);
                    }
                    (ctx, node_ids)
                },
                |(ctx, node_ids)| {
                    for &node_id in &node_ids {
                        let props = ctx
                            .backend
                            .get_node_properties(node_id)
                            .expect("Failed to get properties");
                        black_box(props);
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark: SQL Layer - Edge attribute lookup
pub fn bench_sql_edge_attributes(c: &mut Criterion) {
    let mut group = c.benchmark_group("sql_edge_attributes");
    group.measurement_time(Duration::from_secs(4));
    group.sample_size(10);

    for count in [100usize, 1000, 10000] {
        group.throughput(Throughput::Elements(count as u64));
        let name = format!("{}", count);

        group.bench_function(name, |b| {
            b.iter_batched(
                || {
                    let ctx = create_v3_bench_context("sql_edge_attrs.db");
                    let mut edge_pairs = Vec::new();

                    for i in 0..count {
                        let from = ctx
                            .backend
                            .insert_node(NodeSpec {
                                kind: "Document".to_string(),
                                name: format!("doc_{}", i),
                                file_path: None,
                                data: serde_json::json!({}),
                            })
                            .expect("Failed to insert node");

                        let to = ctx
                            .backend
                            .insert_node(NodeSpec {
                                kind: "Document".to_string(),
                                name: format!("doc_{}", i + 1),
                                file_path: None,
                                data: serde_json::json!({}),
                            })
                            .expect("Failed to insert node");

                        ctx.backend
                            .insert_edge(EdgeSpec {
                                from,
                                to,
                                edge_type: "CITES".to_string(),
                                data: serde_json::json!({"weight": 0.5 + (i as f32 * 0.01)}),
                            })
                            .expect("Failed to insert edge");

                        edge_pairs.push((from, to));
                    }
                    (ctx, edge_pairs)
                },
                |(ctx, edge_pairs)| {
                    for &(from, to) in &edge_pairs {
                        let attrs = ctx
                            .backend
                            .get_edge_attributes(from, to)
                            .expect("Failed to get attributes");
                        black_box(attrs);
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark: HNSW - Vector search
pub fn bench_hnsw_vector_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("hnsw_vector_search");
    group.measurement_time(Duration::from_secs(8));
    group.sample_size(10);

    // Test scales: 1K (pre-turbovec), 5K (cliff location), 10K (post-cliff), 50K (turbovec scaling)
    for (vector_count, dimension) in [(1000, 64usize), (5000, 64), (10000, 64), (50000, 64)] {
        let name = format!("{}_{}", vector_count, dimension);
        group.throughput(Throughput::Elements(vector_count as u64));

        group.bench_function(name, |b| {
            b.iter_batched(
                || {
                    let ctx = create_v3_bench_context(&format!("hnsw_{}.db", vector_count));

                    ctx.backend
                        .create_hnsw_index("test_index", dimension, 4)
                        .expect("Failed to create HNSW index");

                    let query_vec: Vec<f32> = (0..dimension).map(|i| i as f32 * 0.01).collect();

                    // Phase 1: Insert all vectors (includes turbovec build at threshold - NOT MEASURED)
                    for i in 0..vector_count {
                        let vec: Vec<f32> = (0..dimension)
                            .map(|j| (j as f32 * 0.01 + i as f32).cos())
                            .collect();
                        let metadata = serde_json::json!({"id": i});

                        ctx.backend
                            .insert_hnsw_vector("test_index", &vec, Some(metadata))
                            .expect("Failed to insert vector");
                    }

                    // Phase 2: Warm the index with dummy search (trigger any lazy builds)
                    let warm_vec: Vec<f32> =
                        (0..dimension).map(|i| (i as f32 * 0.01).sin()).collect();
                    let _ = ctx.backend.hnsw_vector_search("test_index", &warm_vec, 10);

                    (ctx, query_vec)
                },
                |(ctx, query_vec)| {
                    // Phase 3: Measure search only with warm index (THIS IS WHAT WE MEASURE)
                    let results = ctx
                        .backend
                        .hnsw_vector_search("test_index", &query_vec, 10)
                        .expect("Search failed");
                    black_box(results);
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark: CSR - Neighbor query performance
pub fn bench_csr_neighbors(c: &mut Criterion) {
    let mut group = c.benchmark_group("csr_neighbors");
    group.measurement_time(Duration::from_secs(4));
    group.sample_size(10);

    for (nodes, edges_per_node) in [(1000usize, 5usize), (1000, 20), (10000, 5), (10000, 20)] {
        let name = format!("sparse_{}_{}", nodes, edges_per_node);
        group.throughput(Throughput::Elements(nodes as u64));

        group.bench_function(name, |b| {
            b.iter_batched(
                || {
                    let ctx = create_v3_bench_context("csr.db");
                    let mut node_ids = Vec::new();

                    for i in 0..nodes {
                        let node_id = ctx
                            .backend
                            .insert_node(NodeSpec {
                                kind: "Node".to_string(),
                                name: format!("node_{}", i),
                                file_path: None,
                                data: serde_json::json!({}),
                            })
                            .expect("Failed to insert node");
                        node_ids.push(node_id);
                    }

                    for i in 0..nodes.saturating_sub(1) {
                        let from = node_ids[i];
                        for j in 0..edges_per_node {
                            let to = node_ids[(i + j + 1) % node_ids.len()];
                            ctx.backend
                                .insert_edge(EdgeSpec {
                                    from,
                                    to,
                                    edge_type: "LINKS".to_string(),
                                    data: serde_json::json!({}),
                                })
                                .expect("Failed to insert edge");
                        }
                    }

                    (ctx, node_ids)
                },
                |(ctx, node_ids)| {
                    let center = node_ids[node_ids.len() / 2];
                    let neighbors = ctx
                        .backend
                        .neighbors(
                            SnapshotId::current(),
                            center,
                            sqlitegraph::backend::NeighborQuery {
                                direction: sqlitegraph::backend::BackendDirection::Outgoing,
                                edge_type: None,
                            },
                        )
                        .expect("Failed to get neighbors");
                    black_box(neighbors.len());
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark: MVCC - Snapshot read overhead
pub fn bench_mvcc_snapshot_overhead(c: &mut Criterion) {
    let mut group = c.benchmark_group("mvcc_snapshot_overhead");
    group.measurement_time(Duration::from_secs(4));
    group.sample_size(10);

    for count in [100usize, 1000, 10000] {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_function(format!("current_{}", count), |b| {
            b.iter_batched(
                || {
                    let ctx = create_v3_bench_context("mvcc_current.db");
                    let mut node_ids = Vec::new();

                    for i in 0..count {
                        let node_id = ctx
                            .backend
                            .insert_node(NodeSpec {
                                kind: "Node".to_string(),
                                name: format!("node_{}", i),
                                file_path: None,
                                data: serde_json::json!({}),
                            })
                            .expect("Failed to insert node");
                        node_ids.push(node_id);
                    }
                    (ctx, node_ids)
                },
                |(ctx, node_ids)| {
                    for &node_id in &node_ids {
                        let node = ctx
                            .backend
                            .get_node(SnapshotId::current(), node_id)
                            .expect("Failed to get node");
                        black_box(node);
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });

        group.bench_function(format!("historical_{}", count), |b| {
            b.iter_batched(
                || {
                    let ctx = create_v3_bench_context("mvcc_historical.db");
                    let mut node_ids = Vec::new();

                    let half = count / 2;
                    for i in 0..half {
                        let node_id = ctx
                            .backend
                            .insert_node(NodeSpec {
                                kind: "Node".to_string(),
                                name: format!("node_{}", i),
                                file_path: None,
                                data: serde_json::json!({}),
                            })
                            .expect("Failed to insert node");
                        node_ids.push(node_id);
                    }

                    let snapshot_lsn = ctx
                        .backend
                        .create_snapshot("mid")
                        .expect("Failed to create snapshot");

                    for i in half..count {
                        let node_id = ctx
                            .backend
                            .insert_node(NodeSpec {
                                kind: "Node".to_string(),
                                name: format!("node_{}", i),
                                file_path: None,
                                data: serde_json::json!({}),
                            })
                            .expect("Failed to insert node");
                        node_ids.push(node_id);
                    }

                    (ctx, snapshot_lsn, node_ids)
                },
                |(ctx, snapshot_lsn, node_ids)| {
                    for &node_id in &node_ids {
                        let _ = ctx
                            .backend
                            .get_node(SnapshotId::from_lsn(snapshot_lsn), node_id);
                        black_box(());
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark: PubSub - Event delivery latency
pub fn bench_pubsub_event_delivery(c: &mut Criterion) {
    let mut group = c.benchmark_group("pubsub_event_delivery");
    group.measurement_time(Duration::from_secs(4));
    group.sample_size(10);

    for count in [10usize, 100, 1000] {
        group.throughput(Throughput::Elements(count as u64));

        group.bench_function(format!("{}", count), |b| {
            b.iter_batched(
                || {
                    let ctx = create_v3_bench_context("pubsub.db");

                    let (_sub_id, rx) = ctx
                        .backend
                        .subscribe(SubscriptionFilter::all())
                        .expect("Failed to subscribe");

                    (ctx, rx, count)
                },
                |(ctx, rx, count)| {
                    for i in 0..count {
                        let _node_id = ctx
                            .backend
                            .insert_node(NodeSpec {
                                kind: "User".to_string(),
                                name: format!("user_{}", i),
                                file_path: None,
                                data: serde_json::json!({}),
                            })
                            .expect("Failed to insert node");

                        ctx.backend.flush().expect("Failed to flush");

                        let event = rx
                            .recv_timeout(std::time::Duration::from_millis(100))
                            .expect("Should receive event");
                        black_box(event);
                    }
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

/// Benchmark: All features working together
pub fn bench_all_features_integration(c: &mut Criterion) {
    let mut group = c.benchmark_group("integration");
    group.measurement_time(Duration::from_secs(10));
    group.sample_size(10);

    for nodes in [100usize, 1000] {
        group.throughput(Throughput::Elements(nodes as u64));

        group.bench_function(format!("{}", nodes), |b| {
            b.iter_batched(
                || {
                    let ctx = create_v3_bench_context("integration.db");

                    let (_sub_id, _rx) = ctx
                        .backend
                        .subscribe(SubscriptionFilter::all())
                        .expect("Failed to subscribe");

                    let snapshot_lsn = ctx
                        .backend
                        .create_snapshot("initial")
                        .expect("Failed to create snapshot");

                    ctx.backend
                        .create_hnsw_index("semantic", 64, 4)
                        .expect("Failed to create HNSW index");

                    (ctx, snapshot_lsn, nodes)
                },
                |(ctx, _snapshot_lsn, nodes)| {
                    let mut node_ids = Vec::new();
                    for i in 0..nodes {
                        let node_id = ctx
                            .backend
                            .insert_node(NodeSpec {
                                kind: "Document".to_string(),
                                name: format!("doc_{}", i),
                                file_path: None,
                                data: serde_json::json!({"topic": "AI", "importance": 0.9}),
                            })
                            .expect("Failed to insert node");
                        node_ids.push(node_id);

                        if let Ok(Some(index)) = ctx.backend.get_hnsw_index("semantic") {
                            let mut index = index.lock().unwrap();
                            let vec: Vec<f32> = (0..64).map(|j| j as f32 * 0.01).collect();
                            let _ = index
                                .insert_vector(&vec, Some(serde_json::json!({"node": node_id})));
                        }
                    }

                    for i in 0..nodes.saturating_sub(1) {
                        ctx.backend
                            .insert_edge(EdgeSpec {
                                from: node_ids[i],
                                to: node_ids[i + 1],
                                edge_type: "CITES".to_string(),
                                data: serde_json::json!({"weight": 0.7}),
                            })
                            .expect("Failed to insert edge");
                    }

                    let center = node_ids[node_ids.len() / 2];
                    let neighbors = ctx
                        .backend
                        .neighbors(
                            SnapshotId::current(),
                            center,
                            sqlitegraph::backend::NeighborQuery {
                                direction: sqlitegraph::backend::BackendDirection::Outgoing,
                                edge_type: None,
                            },
                        )
                        .expect("Failed to get neighbors");

                    black_box(neighbors.len());
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }

    group.finish();
}

criterion_group!(
    native_v3_features,
    bench_sql_node_properties,
    bench_sql_edge_attributes,
    bench_hnsw_vector_search,
    bench_csr_neighbors,
    bench_mvcc_snapshot_overhead,
    bench_pubsub_event_delivery,
    bench_all_features_integration,
);

criterion_main!(native_v3_features);
