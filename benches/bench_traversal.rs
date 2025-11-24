use std::{sync::Arc, time::Duration};

use criterion::{Criterion, criterion_group, criterion_main};
use sqlitegraph::{
    SqliteGraph,
    bench_utils::{GraphDataset, GraphShape, generate_graph},
    bfs::{bfs_neighbors, shortest_path},
};

const LINE_SEED: u64 = 0xDD21;
const ER_SEED: u64 = 0xEE45;
const SF_SEED: u64 = 0xFF89;
const SAMPLE_SIZE: usize = 20;
const WARM_UP: Duration = Duration::from_millis(300);
const MEASURE: Duration = Duration::from_millis(500);

struct PreparedGraph {
    dataset: Arc<GraphDataset>,
    graph: SqliteGraph,
    id_map: Vec<i64>,
    label: &'static str,
}

fn bench_scale() -> usize {
    #[cfg(feature = "bench-ci")]
    {
        10_000
    }
    #[cfg(not(feature = "bench-ci"))]
    {
        50_000
    }
}

fn prepared_graphs() -> Vec<PreparedGraph> {
    let nodes = bench_scale();
    let mut graphs = Vec::new();
    let line = Arc::new(generate_graph(GraphShape::Line, nodes, LINE_SEED));
    graphs.push(materialize(line, "line"));
    let random = Arc::new(generate_graph(
        GraphShape::RandomErdosRenyi {
            edges: nodes.saturating_mul(5),
        },
        nodes,
        ER_SEED,
    ));
    graphs.push(materialize(random, "er"));
    let sf = Arc::new(generate_graph(
        GraphShape::ScaleFree { m: 5 },
        nodes,
        SF_SEED,
    ));
    graphs.push(materialize(sf, "scalefree"));
    graphs
}

fn bench_neighbors(c: &mut Criterion) {
    let graphs = prepared_graphs();
    let mut group = c.benchmark_group("neighbors");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP);
    group.measurement_time(MEASURE);
    for prepared in &graphs {
        let start = start_node(prepared);
        group.bench_function(prepared.label, |b| {
            b.iter(|| prepared.graph.query().neighbors(start).expect("neighbors"));
        });
    }
    group.finish();
}

fn bench_bfs(c: &mut Criterion) {
    let graphs = prepared_graphs();
    let mut group = c.benchmark_group("bfs");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP);
    group.measurement_time(MEASURE);
    for prepared in &graphs {
        let start = start_node(prepared);
        group.bench_function(prepared.label, |b| {
            b.iter(|| bfs_neighbors(&prepared.graph, start, 3).expect("bfs"));
        });
    }
    group.finish();
}

fn bench_shortest_paths(c: &mut Criterion) {
    let graphs = prepared_graphs();
    let mut group = c.benchmark_group("shortest_path");
    group.sample_size(SAMPLE_SIZE);
    group.warm_up_time(WARM_UP);
    group.measurement_time(MEASURE);
    for prepared in &graphs {
        let (start_idx, end_idx) = match prepared.label {
            "line" => (0usize, prepared.dataset.nodes() - 1),
            "er" => edge_pair(&prepared.dataset, 0),
            _ => (prepared.dataset.hub_index(), prepared.dataset.nodes() - 1),
        };
        let start = prepared.id_map[start_idx];
        let end = prepared.id_map[end_idx];
        group.bench_function(prepared.label, |b| {
            b.iter(|| shortest_path(&prepared.graph, start, end).expect("shortest"));
        });
    }
    group.finish();
}

fn materialize(dataset: Arc<GraphDataset>, label: &'static str) -> PreparedGraph {
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
    PreparedGraph {
        dataset,
        graph,
        id_map: ids,
        label,
    }
}

fn start_node(prepared: &PreparedGraph) -> i64 {
    match prepared.label {
        "line" => prepared.id_map[prepared.dataset.nodes() / 2],
        "er" => {
            let (_, to) = edge_pair(&prepared.dataset, 1);
            prepared.id_map[to]
        }
        _ => prepared.id_map[prepared.dataset.hub_index()],
    }
}

fn edge_pair(dataset: &GraphDataset, index: usize) -> (usize, usize) {
    let edge = dataset
        .edges
        .get(index.min(dataset.edges() - 1))
        .expect("edge");
    (edge.from_id as usize, edge.to_id as usize)
}

criterion_group!(
    name = traversal_benches;
    config = Criterion::default();
    targets = bench_neighbors, bench_bfs, bench_shortest_paths
);
criterion_main!(traversal_benches);
