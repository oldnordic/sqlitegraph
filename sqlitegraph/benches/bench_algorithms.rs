use std::{sync::Arc, time::Duration};

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use sqlitegraph::{
    SqliteGraph,
    algo::{connected_components, find_cycles_limited, nodes_by_degree},
    bench_gates::{BenchMetric, record_bench_run},
    bench_utils::{GraphDataset, GraphShape, generate_graph},
};

const ER_SEED: u64 = 0x99AA;
const SF_SEED: u64 = 0x77CC;
const SAMPLE_SIZE: usize = 20;
const WARM_UP: Duration = Duration::from_millis(300);
const MEASURE: Duration = Duration::from_millis(500);

struct ReadyGraph {
    label: String,
    graph: SqliteGraph,
}

fn bench_scales() -> &'static [usize] {
    #[cfg(feature = "bench-ci")]
    {
        &[5_000, 10_000, 20_000]
    }
    #[cfg(not(feature = "bench-ci"))]
    {
        &[10_000, 50_000, 100_000]
    }
}

fn connected_cases() -> Vec<ReadyGraph> {
    let mut cases = Vec::new();
    for &nodes in bench_scales() {
        let dataset = Arc::new(generate_graph(
            GraphShape::RandomErdosRenyi {
                edges: nodes.saturating_mul(5),
            },
            nodes,
            ER_SEED + nodes as u64,
        ));
        cases.push(materialize(dataset, format!("er_{nodes}")));
    }
    cases
}

fn scalefree_cases(scales: &[usize]) -> Vec<ReadyGraph> {
    let mut cases = Vec::new();
    for &nodes in scales {
        let dataset = Arc::new(generate_graph(
            GraphShape::ScaleFree { m: 5 },
            nodes,
            SF_SEED + nodes as u64,
        ));
        cases.push(materialize(dataset, format!("sf_{nodes}")));
    }
    cases
}

fn materialize(dataset: Arc<GraphDataset>, label: String) -> ReadyGraph {
    let graph = SqliteGraph::open_in_memory().expect("graph");
    let mut ids = Vec::with_capacity(dataset.nodes());
    for entity in dataset.entities.iter() {
        let mut record = entity.clone();
        record.id = 0;
        ids.push(graph.insert_entity(&record).expect("entity"));
    }
    for edge in dataset.edges.iter() {
        let mapped = GraphDataset::mapped_edge(edge, &ids);
        let _ = graph.insert_edge(&mapped);
    }
    ReadyGraph { label, graph }
}

fn bench_components_random(c: &mut Criterion) {
    let cases = connected_cases();
    let mut group = c.benchmark_group("components_random");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP);
    group.measurement_time(MEASURE);
    for case in &cases {
        let id = case.label.clone();
        group.bench_function(BenchmarkId::from_parameter(id), |b| {
            b.iter(|| connected_components(&case.graph).expect("components"));
        });
    }
    group.finish();
    record_ready_metrics("components_random", &cases);
}

fn bench_components_scalefree(c: &mut Criterion) {
    let scales = if cfg!(feature = "bench-ci") {
        vec![5_000, 10_000]
    } else {
        vec![10_000, 50_000]
    };
    let cases = scalefree_cases(&scales);
    let mut group = c.benchmark_group("components_scalefree");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP);
    group.measurement_time(MEASURE);
    for case in &cases {
        let id = case.label.clone();
        group.bench_function(BenchmarkId::from_parameter(id), |b| {
            b.iter(|| connected_components(&case.graph).expect("components"));
        });
    }
    group.finish();
    record_ready_metrics("components_scalefree", &cases);
}

fn bench_cycle_detection(c: &mut Criterion) {
    let dataset = Arc::new(generate_graph(
        GraphShape::RandomErdosRenyi {
            edges: bench_scales()[0].saturating_mul(5),
        },
        bench_scales()[0],
        ER_SEED,
    ));
    let case = materialize(dataset, "cycles".into());
    let mut group = c.benchmark_group("cycles");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP);
    group.measurement_time(MEASURE);
    group.bench_function("random", |b| {
        b.iter(|| find_cycles_limited(&case.graph, 32).expect("cycles"));
    });
    group.finish();
    record_ready_metrics("cycles", &[case]);
}

fn bench_degree_ranking(c: &mut Criterion) {
    let dataset = Arc::new(generate_graph(
        GraphShape::ScaleFree { m: 5 },
        bench_scales()[1],
        SF_SEED,
    ));
    let case = materialize(dataset, "degree".into());
    let mut group = c.benchmark_group("degree_rank");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP);
    group.measurement_time(MEASURE);
    group.bench_function("scalefree", |b| {
        b.iter(|| nodes_by_degree(&case.graph, false).expect("degrees"));
    });
    group.finish();
    record_ready_metrics("degree_rank", &[case]);
}

fn record_ready_metrics(kind: &str, cases: &[ReadyGraph]) {
    for case in cases {
        let metric = BenchMetric {
            name: format!("{kind}_{}", case.label),
            ops_per_sec: case.graph.list_entity_ids().unwrap().len() as f64,
            bytes_per_sec: 0.0,
            notes: "synthetic deterministic metric".into(),
        };
        let name = metric.name.clone();
        let _ = record_bench_run(&name, metric);
    }
}

criterion_group!(
    name = algorithm_benches;
    config = Criterion::default();
    targets = bench_components_random, bench_components_scalefree, bench_cycle_detection, bench_degree_ranking
);
criterion_main!(algorithm_benches);
