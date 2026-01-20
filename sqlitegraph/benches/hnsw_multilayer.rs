//! Multi-layer HNSW O(log N) scaling benchmarks.
//!
//! Benchmarks HNSW to verify search performance across dataset sizes.

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main, Throughput};
use sqlitegraph::hnsw::{HnswConfig, DistanceMetric, HnswIndex};

mod bench_utils;
use bench_utils::{MEASURE, WARM_UP};

/// Benchmark HNSW search scaling with dataset size
fn bench_hnsw_search_scaling(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("hnsw_scaling");
    group.sample_size(100);

    let dataset_sizes = vec![1_000, 10_000];
    let dimension = 128;

    for &size in &dataset_sizes {
        group.throughput(Throughput::Elements(size as u64));

        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |bencher, &size| {
                // Use single-layer mode for now (multi-layer has an issue in release mode)
                let config = HnswConfig {
                    dimension,
                    m: 16,
                    ef_construction: 200,
                    ef_search: 50,
                    ml: 16,
                    distance_metric: DistanceMetric::Euclidean,
                    enable_multilayer: false,  // Single-layer mode for stability
                    multilayer_level_distribution_base: None,
                    multilayer_deterministic_seed: None,
                };

                let mut hnsw = HnswIndex::new("bench", config).unwrap();

                // Insert vectors
                for i in 0..size {
                    let vector = (0..dimension)
                        .map(|j| ((i * dimension + j) as f32 * 0.01).sin())
                        .collect::<Vec<_>>();
                    hnsw.insert_vector(&vector, None).unwrap();
                }

                // Use first vector as query
                let query = (0..dimension)
                    .map(|j| (j as f32 * 0.01).sin())
                    .collect::<Vec<_>>();

                bencher.iter(|| {
                    hnsw.search(&query, 10).unwrap()
                });
            }
        );
    }

    group.finish();
}

/// Compare multi-layer vs single-layer search performance
fn bench_multilayer_vs_single(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("multilayer_comparison");
    group.measurement_time(MEASURE);
    group.warm_up_time(WARM_UP);

    let dataset_sizes = vec![1_000, 5_000, 10_000];
    let dimension = 128;

    for &size in &dataset_sizes {
        // Single-layer benchmark
        group.bench_with_input(
            BenchmarkId::new("single_layer", size),
            &size,
            |bencher, &size| {
                let config = HnswConfig {
                    dimension,
                    m: 16,
                    ef_construction: 200,
                    ef_search: 50,
                    ml: 16,
                    distance_metric: DistanceMetric::Euclidean,
                    enable_multilayer: false,  // Single-layer mode
                    multilayer_level_distribution_base: None,
                    multilayer_deterministic_seed: None,
                };

                let mut hnsw = HnswIndex::new("bench_single", config).unwrap();

                for i in 0..size {
                    let vector = (0..dimension)
                        .map(|j| ((i * dimension + j) as f32 * 0.01).sin())
                        .collect::<Vec<_>>();
                    hnsw.insert_vector(&vector, None).unwrap();
                }

                let query = (0..dimension)
                    .map(|j| (j as f32 * 0.01).sin())
                    .collect::<Vec<_>>();

                bencher.iter(|| {
                    hnsw.search(&query, 10).unwrap()
                });
            }
        );

        // Placeholder for future multi-layer comparison
        // Currently using single-layer for both to avoid the release-mode bug
        group.bench_with_input(
            BenchmarkId::new("multi_layer_placeholder", size),
            &size,
            |bencher, &size| {
                let config = HnswConfig {
                    dimension,
                    m: 16,
                    ef_construction: 200,
                    ef_search: 50,
                    ml: 16,
                    distance_metric: DistanceMetric::Euclidean,
                    enable_multilayer: false,  // Also single-layer for now
                    multilayer_level_distribution_base: None,
                    multilayer_deterministic_seed: None,
                };

                let mut hnsw = HnswIndex::new("bench_multi", config).unwrap();

                for i in 0..size {
                    let vector = (0..dimension)
                        .map(|j| ((i * dimension + j) as f32 * 0.01).sin())
                        .collect::<Vec<_>>();
                    hnsw.insert_vector(&vector, None).unwrap();
                }

                let query = (0..dimension)
                    .map(|j| (j as f32 * 0.01).sin())
                    .collect::<Vec<_>>();

                bencher.iter(|| {
                    hnsw.search(&query, 10).unwrap()
                });
            }
        );
    }

    group.finish();
}

/// Benchmark insertion performance with multi-layer enabled
fn bench_multilayer_insertion(criterion: &mut Criterion) {
    let mut group = criterion.benchmark_group("multilayer_insertion");
    group.measurement_time(MEASURE);
    group.warm_up_time(WARM_UP);

    let dataset_sizes = vec![100, 500, 1_000, 5_000];
    let dimension = 128;

    for &size in &dataset_sizes {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            &size,
            |bencher, &size| {
                let config = HnswConfig {
                    dimension,
                    m: 16,
                    ef_construction: 200,
                    ef_search: 50,
                    ml: 16,
                    distance_metric: DistanceMetric::Euclidean,
                    enable_multilayer: true,
                    multilayer_level_distribution_base: Some(16),
                    multilayer_deterministic_seed: Some(42),
                };

                bencher.iter(|| {
                    let mut hnsw = HnswIndex::new("bench_insert", config.clone()).unwrap();

                    for i in 0..size {
                        let vector = (0..dimension)
                            .map(|j| ((i * dimension + j) as f32 * 0.01).sin())
                            .collect::<Vec<_>>();
                        hnsw.insert_vector(&vector, None).unwrap();
                    }

                    hnsw
                });
            }
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_hnsw_search_scaling,
    bench_multilayer_vs_single,
    bench_multilayer_insertion
);

criterion_main!(benches);
