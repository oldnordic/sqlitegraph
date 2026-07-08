use std::time::Duration;

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use sqlitegraph::backend::{
    BackendDirection, CombinedGraphBackend, EdgeSpec, GraphBackend, NeighborQuery, NodeSpec,
};
use sqlitegraph::config::CombinedConfig;
use sqlitegraph::snapshot::SnapshotId;

mod bench_utils;
use bench_utils::{MEASURE, SAMPLE_SIZE, WARM_UP, create_v3_bench_context};

const FANOUT: usize = 100;
const BRANCH_FACTOR: usize = 10;

fn node(name: &str) -> NodeSpec {
    NodeSpec {
        kind: "Node".to_string(),
        name: name.to_string(),
        file_path: None,
        data: serde_json::json!({}),
    }
}

fn edge(from: i64, to: i64, edge_type: &str) -> EdgeSpec {
    EdgeSpec {
        from,
        to,
        edge_type: edge_type.to_string(),
        data: serde_json::json!({}),
    }
}

fn outgoing_query() -> NeighborQuery {
    NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    }
}

fn typed_outgoing_query(edge_type: &str) -> NeighborQuery {
    NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: Some(edge_type.to_string()),
    }
}

fn build_combined_graph(config: CombinedConfig) -> (CombinedGraphBackend, i64, i64) {
    let backend = CombinedGraphBackend::in_memory_with_config(config).expect("combined backend");
    let root = backend.insert_node(node("root")).expect("root");

    let mut first_leaf = None;
    for i in 0..FANOUT {
        let mid = backend
            .insert_node(node(&format!("mid_{i}")))
            .expect("mid node");
        backend
            .insert_edge(edge(root, mid, "LINKS"))
            .expect("root->mid");
        for j in 0..BRANCH_FACTOR {
            let leaf = backend
                .insert_node(node(&format!("leaf_{i}_{j}")))
                .expect("leaf");
            backend
                .insert_edge(edge(mid, leaf, "LINKS"))
                .expect("mid->leaf");
            if first_leaf.is_none() {
                first_leaf = Some(leaf);
            }
        }
    }

    (backend, root, first_leaf.expect("leaf"))
}

fn build_native_v3_graph() -> (bench_utils::V3BenchContext, i64) {
    let ctx = create_v3_bench_context("materialization_v3.db");
    let root = ctx.backend.insert_node(node("root")).expect("root");

    for i in 0..FANOUT {
        let linked = ctx
            .backend
            .insert_node(node(&format!("linked_{i}")))
            .expect("linked");
        ctx.backend
            .insert_edge(edge(root, linked, "LINKS"))
            .expect("links edge");

        let typed = ctx
            .backend
            .insert_node(node(&format!("typed_{i}")))
            .expect("typed");
        ctx.backend
            .insert_edge(edge(root, typed, "USES"))
            .expect("uses edge");
    }

    (ctx, root)
}

fn configure_group(group: &mut criterion::BenchmarkGroup<'_, criterion::measurement::WallTime>) {
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP);
    group.measurement_time(MEASURE);
}

fn bench_combined_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("combined_reads");
    configure_group(&mut group);

    let (sqlite_backend, sqlite_root, sqlite_leaf) = build_combined_graph(CombinedConfig::new());
    let (materialized_backend, materialized_root, materialized_leaf) =
        build_combined_graph(CombinedConfig::new().with_materialized_reads());
    materialized_backend
        .publish_materialized_views()
        .expect("publish materialized views");

    group.bench_function(BenchmarkId::new("neighbors", "sqlite_only"), |b| {
        b.iter(|| {
            let backend = black_box(&sqlite_backend);
            let node = black_box(sqlite_root);
            let query = black_box(outgoing_query());
            black_box(
                backend
                    .neighbors(SnapshotId::current(), node, query)
                    .expect("combined neighbors sqlite"),
            );
        });
    });

    group.bench_function(BenchmarkId::new("neighbors", "prefer_materialized"), |b| {
        b.iter(|| {
            let backend = black_box(&materialized_backend);
            let node = black_box(materialized_root);
            let query = black_box(outgoing_query());
            black_box(
                backend
                    .neighbors(SnapshotId::current(), node, query)
                    .expect("combined neighbors materialized"),
            );
        });
    });

    group.bench_function(BenchmarkId::new("bfs_depth2", "sqlite_only"), |b| {
        b.iter(|| {
            let backend = black_box(&sqlite_backend);
            let start = black_box(sqlite_root);
            black_box(
                backend
                    .bfs(SnapshotId::current(), start, black_box(2))
                    .expect("combined bfs sqlite"),
            );
        });
    });

    group.bench_function(BenchmarkId::new("bfs_depth2", "prefer_materialized"), |b| {
        b.iter(|| {
            let backend = black_box(&materialized_backend);
            let start = black_box(materialized_root);
            black_box(
                backend
                    .bfs(SnapshotId::current(), start, black_box(2))
                    .expect("combined bfs materialized"),
            );
        });
    });

    group.bench_function(BenchmarkId::new("shortest_path", "sqlite_only"), |b| {
        b.iter(|| {
            let backend = black_box(&sqlite_backend);
            let start = black_box(sqlite_root);
            let end = black_box(sqlite_leaf);
            black_box(
                backend
                    .shortest_path(SnapshotId::current(), start, end)
                    .expect("combined shortest path sqlite"),
            );
        });
    });

    group.bench_function(
        BenchmarkId::new("shortest_path", "prefer_materialized"),
        |b| {
            b.iter(|| {
                let backend = black_box(&materialized_backend);
                let start = black_box(materialized_root);
                let end = black_box(materialized_leaf);
                black_box(
                    backend
                        .shortest_path(SnapshotId::current(), start, end)
                        .expect("combined shortest path materialized"),
                );
            });
        },
    );

    group.finish();
}

fn bench_combined_cold_reads(c: &mut Criterion) {
    let mut group = c.benchmark_group("combined_cold_reads");
    configure_group(&mut group);

    group.bench_function(BenchmarkId::new("neighbors", "sqlite_only"), |b| {
        b.iter_batched(
            || build_combined_graph(CombinedConfig::new()),
            |(backend, root, _)| {
                black_box(
                    backend
                        .neighbors(
                            SnapshotId::current(),
                            black_box(root),
                            black_box(outgoing_query()),
                        )
                        .expect("cold combined neighbors sqlite"),
                );
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function(BenchmarkId::new("neighbors", "prefer_materialized"), |b| {
        b.iter_batched(
            || {
                let built = build_combined_graph(CombinedConfig::new().with_materialized_reads());
                built
                    .0
                    .publish_materialized_views()
                    .expect("publish materialized");
                built
            },
            |(backend, root, _)| {
                black_box(
                    backend
                        .neighbors(
                            SnapshotId::current(),
                            black_box(root),
                            black_box(outgoing_query()),
                        )
                        .expect("cold combined neighbors materialized"),
                );
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function(BenchmarkId::new("bfs_depth2", "sqlite_only"), |b| {
        b.iter_batched(
            || build_combined_graph(CombinedConfig::new()),
            |(backend, root, _)| {
                black_box(
                    backend
                        .bfs(SnapshotId::current(), black_box(root), black_box(2))
                        .expect("cold combined bfs sqlite"),
                );
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function(BenchmarkId::new("bfs_depth2", "prefer_materialized"), |b| {
        b.iter_batched(
            || {
                let built = build_combined_graph(CombinedConfig::new().with_materialized_reads());
                built
                    .0
                    .publish_materialized_views()
                    .expect("publish materialized");
                built
            },
            |(backend, root, _)| {
                black_box(
                    backend
                        .bfs(SnapshotId::current(), black_box(root), black_box(2))
                        .expect("cold combined bfs materialized"),
                );
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function(BenchmarkId::new("shortest_path", "sqlite_only"), |b| {
        b.iter_batched(
            || build_combined_graph(CombinedConfig::new()),
            |(backend, root, leaf)| {
                black_box(
                    backend
                        .shortest_path(SnapshotId::current(), black_box(root), black_box(leaf))
                        .expect("cold combined shortest path sqlite"),
                );
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function(
        BenchmarkId::new("shortest_path", "prefer_materialized"),
        |b| {
            b.iter_batched(
                || {
                    let built =
                        build_combined_graph(CombinedConfig::new().with_materialized_reads());
                    built
                        .0
                        .publish_materialized_views()
                        .expect("publish materialized");
                    built
                },
                |(backend, root, leaf)| {
                    black_box(
                        backend
                            .shortest_path(SnapshotId::current(), black_box(root), black_box(leaf))
                            .expect("cold combined shortest path materialized"),
                    );
                },
                criterion::BatchSize::SmallInput,
            );
        },
    );

    group.finish();
}

fn bench_combined_incremental_writes(c: &mut Criterion) {
    let mut group = c.benchmark_group("combined_incremental_writes");
    configure_group(&mut group);

    group.bench_function("insert_edge/sqlite_only", |b| {
        b.iter_batched(
            || {
                let backend = CombinedGraphBackend::in_memory_with_config(CombinedConfig::new())
                    .expect("combined sqlite-only");
                let a = backend.insert_node(node("a")).expect("a");
                let z = backend.insert_node(node("z")).expect("z");
                (backend, a, z)
            },
            |(backend, a, z)| {
                black_box(
                    backend
                        .insert_edge(edge(a, z, "LINKS"))
                        .expect("insert edge"),
                );
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("insert_edge/prefer_materialized", |b| {
        b.iter_batched(
            || {
                let backend = CombinedGraphBackend::in_memory_with_config(
                    CombinedConfig::new().with_materialized_reads(),
                )
                .expect("combined materialized");
                let a = backend.insert_node(node("a")).expect("a");
                let z = backend.insert_node(node("z")).expect("z");
                (backend, a, z)
            },
            |(backend, a, z)| {
                black_box(
                    backend
                        .insert_edge(edge(a, z, "LINKS"))
                        .expect("insert edge"),
                );
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("delete_entity/sqlite_only", |b| {
        b.iter_batched(
            || {
                let backend = CombinedGraphBackend::in_memory_with_config(CombinedConfig::new())
                    .expect("combined sqlite-only");
                let a = backend.insert_node(node("a")).expect("a");
                let b = backend.insert_node(node("b")).expect("b");
                let c = backend.insert_node(node("c")).expect("c");
                backend.insert_edge(edge(a, b, "LINKS")).expect("a->b");
                backend.insert_edge(edge(b, c, "LINKS")).expect("b->c");
                (backend, b)
            },
            |(backend, victim)| {
                backend.delete_entity(victim).expect("delete entity");
                black_box(());
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.bench_function("delete_entity/prefer_materialized", |b| {
        b.iter_batched(
            || {
                let backend = CombinedGraphBackend::in_memory_with_config(
                    CombinedConfig::new().with_materialized_reads(),
                )
                .expect("combined materialized");
                let a = backend.insert_node(node("a")).expect("a");
                let b = backend.insert_node(node("b")).expect("b");
                let c = backend.insert_node(node("c")).expect("c");
                backend.insert_edge(edge(a, b, "LINKS")).expect("a->b");
                backend.insert_edge(edge(b, c, "LINKS")).expect("b->c");
                (backend, b)
            },
            |(backend, victim)| {
                backend.delete_entity(victim).expect("delete entity");
                black_box(());
            },
            criterion::BatchSize::SmallInput,
        );
    });

    group.finish();
}

fn bench_native_v3_csr_vs_filtered(c: &mut Criterion) {
    let mut group = c.benchmark_group("native_v3_reads");
    configure_group(&mut group);
    group.measurement_time(Duration::from_millis(800));

    let (ctx, root) = build_native_v3_graph();

    group.bench_function(
        BenchmarkId::new("neighbors", "outgoing_unfiltered_csr"),
        |b| {
            b.iter(|| {
                let backend = black_box(&ctx.backend);
                let node = black_box(root);
                let query = black_box(outgoing_query());
                black_box(
                    backend
                        .neighbors(SnapshotId::current(), node, query)
                        .expect("v3 unfiltered neighbors"),
                );
            });
        },
    );

    group.bench_function(
        BenchmarkId::new("neighbors", "outgoing_typed_filtered"),
        |b| {
            b.iter(|| {
                let backend = black_box(&ctx.backend);
                let node = black_box(root);
                let query = black_box(typed_outgoing_query("LINKS"));
                black_box(
                    backend
                        .neighbors(SnapshotId::current(), node, query)
                        .expect("v3 typed neighbors"),
                );
            });
        },
    );

    group.finish();
}

criterion_group!(
    backend_mode_materialization,
    bench_combined_reads,
    bench_combined_cold_reads,
    bench_combined_incremental_writes,
    bench_native_v3_csr_vs_filtered
);
criterion_main!(backend_mode_materialization);
