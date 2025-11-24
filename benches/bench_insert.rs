use std::{sync::Arc, time::Duration};

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use sqlitegraph::{
    GraphEdge, SqliteGraph,
    bench_utils::{GraphDataset, GraphShape, generate_graph},
};

const LINE_SEED: u64 = 0xA17C;
const ER_SEED: u64 = 0xB25F;
const SF_SEED: u64 = 0xC3D9;
const SAMPLE_SIZE: usize = 20;
const WARM_UP: Duration = Duration::from_millis(300);
const MEASURE: Duration = Duration::from_millis(500);

struct BenchCase {
    id: String,
    dataset: Arc<GraphDataset>,
}

fn bench_scales() -> &'static [usize] {
    #[cfg(feature = "bench-ci")]
    {
        &[1_000, 5_000, 10_000]
    }
    #[cfg(not(feature = "bench-ci"))]
    {
        &[10_000, 50_000, 100_000]
    }
}

fn random_shape(nodes: usize) -> GraphShape {
    GraphShape::RandomErdosRenyi {
        edges: nodes.saturating_mul(5),
    }
}

fn bench_cases() -> Vec<BenchCase> {
    let mut cases = Vec::new();
    for &nodes in bench_scales() {
        let line = generate_graph(GraphShape::Line, nodes, LINE_SEED + nodes as u64);
        cases.push(BenchCase {
            id: format!("line_{}", nodes),
            dataset: Arc::new(line),
        });
        let random = generate_graph(random_shape(nodes), nodes, ER_SEED + nodes as u64);
        cases.push(BenchCase {
            id: format!("er_{}", nodes),
            dataset: Arc::new(random),
        });
        let scale_free = generate_graph(
            GraphShape::ScaleFree { m: 5 },
            nodes,
            SF_SEED + nodes as u64,
        );
        cases.push(BenchCase {
            id: format!("scalefree_{}", nodes),
            dataset: Arc::new(scale_free),
        });
    }
    cases
}

fn bench_insert_entities(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_entities");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP);
    group.measurement_time(MEASURE);
    for case in bench_cases() {
        let name = case.id.clone();
        let dataset = case.dataset.clone();
        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.iter(|| {
                let graph = SqliteGraph::open_in_memory().expect("graph");
                let _ = insert_entities(&graph, &dataset);
            });
        });
    }
    group.finish();
}

fn bench_insert_edges(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_edges");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP);
    group.measurement_time(MEASURE);
    for case in bench_cases() {
        let name = case.id.clone();
        let dataset = case.dataset.clone();
        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.iter(|| {
                let graph = SqliteGraph::open_in_memory().expect("graph");
                let ids = insert_entities(&graph, &dataset);
                insert_edges(&graph, &dataset, &ids);
            });
        });
    }
    group.finish();
}

fn bench_insert_combined(c: &mut Criterion) {
    let mut group = c.benchmark_group("insert_combined");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP);
    group.measurement_time(MEASURE);
    for case in bench_cases() {
        let name = case.id.clone();
        let dataset = case.dataset.clone();
        group.bench_function(BenchmarkId::from_parameter(name), |b| {
            b.iter(|| {
                let graph = SqliteGraph::open_in_memory().expect("graph");
                let ids = insert_entities(&graph, &dataset);
                insert_edges(&graph, &dataset, &ids);
            });
        });
    }
    group.finish();
}

fn insert_entities(graph: &SqliteGraph, dataset: &GraphDataset) -> Vec<i64> {
    dataset
        .entities
        .iter()
        .map(|entity| {
            let mut record = entity.clone();
            record.id = 0;
            graph.insert_entity(&record).expect("entity insert")
        })
        .collect()
}

fn insert_edges(graph: &SqliteGraph, dataset: &GraphDataset, id_map: &[i64]) {
    for edge in &dataset.edges {
        let mapped = GraphDataset::mapped_edge(edge, id_map);
        let _ = graph.insert_edge(&mapped);
    }
}

criterion_group!(
    name = insert_benches;
    config = Criterion::default();
    targets = bench_insert_entities, bench_insert_edges, bench_insert_combined
);
criterion_main!(insert_benches);
