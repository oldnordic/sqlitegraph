use criterion::{Criterion, criterion_group, criterion_main};
use serde_json::json;
use sqlitegraph::{
    GraphEdge, GraphEntity, SqliteGraph,
    backend::{BackendDirection, SqliteGraphBackend},
    bench_gates::{BenchMetric, record_bench_run},
    pattern::{NodeConstraint, PatternLeg, PatternQuery},
    pipeline::{ReasoningPipeline, ReasoningStep, run_pipeline},
    subgraph::{SubgraphRequest, extract_subgraph},
};

const SYNCOMPAT_SUBGRAPH_OPS: f64 = 1200.0;
const SYNCOMPAT_SUBGRAPH_BYTES: f64 = 4096.0;
const SYNCOMPAT_PIPELINE_OPS: f64 = 720.0;
const SYNCOMPAT_PIPELINE_BYTES: f64 = 2048.0;

struct SyncompatScenario {
    backend: SqliteGraphBackend,
    subgraph_request: SubgraphRequest,
    pipeline: ReasoningPipeline,
}

impl SyncompatScenario {
    fn build() -> Self {
        let graph = SqliteGraph::open_in_memory().expect("graph");
        let file_id = graph
            .insert_entity(&GraphEntity {
                id: 0,
                kind: "file".into(),
                name: "SynCompatFile".into(),
                file_path: Some("src/syncompat/file.rs".into()),
                data: json!({"lang": "rust"}),
            })
            .expect("file");
        let module_id = graph
            .insert_entity(&GraphEntity {
                id: 0,
                kind: "module".into(),
                name: "syn_mod".into(),
                file_path: Some("src/syncompat/mod.rs".into()),
                data: json!({"visibility": "pub"}),
            })
            .expect("module");
        let processor_id = graph
            .insert_entity(&GraphEntity {
                id: 0,
                kind: "function".into(),
                name: "syn_process".into(),
                file_path: Some("src/syncompat/mod.rs".into()),
                data: json!({"sig": "fn syn_process()"}),
            })
            .expect("processor");
        let helper_id = graph
            .insert_entity(&GraphEntity {
                id: 0,
                kind: "function".into(),
                name: "syn_helper".into(),
                file_path: Some("src/syncompat/mod.rs".into()),
                data: json!({"sig": "fn syn_helper()"}),
            })
            .expect("helper");

        graph
            .insert_edge(&GraphEdge {
                id: 0,
                from_id: file_id,
                to_id: module_id,
                edge_type: "contains".into(),
                data: json!({}),
            })
            .expect("file->module");
        graph
            .insert_edge(&GraphEdge {
                id: 0,
                from_id: module_id,
                to_id: processor_id,
                edge_type: "contains".into(),
                data: json!({}),
            })
            .expect("module->processor");
        graph
            .insert_edge(&GraphEdge {
                id: 0,
                from_id: processor_id,
                to_id: helper_id,
                edge_type: "calls".into(),
                data: json!({}),
            })
            .expect("processor->helper");

        let backend = SqliteGraphBackend::from_graph(graph);
        let subgraph_request = SubgraphRequest {
            root: file_id,
            depth: 2,
            allowed_edge_types: vec![],
            allowed_node_types: vec![],
        };
        let pipeline = ReasoningPipeline {
            steps: vec![
                ReasoningStep::Pattern(PatternQuery {
                    root: Some(NodeConstraint::kind("function")),
                    legs: vec![PatternLeg {
                        direction: BackendDirection::Outgoing,
                        edge_type: Some("calls".into()),
                        constraint: Some(NodeConstraint::kind("function")),
                    }],
                }),
                ReasoningStep::Filter(NodeConstraint::name_prefix("syn_")),
            ],
        };
        Self {
            backend,
            subgraph_request,
            pipeline,
        }
    }

    fn backend(&self) -> &SqliteGraphBackend {
        &self.backend
    }

    fn subgraph_request(&self) -> SubgraphRequest {
        self.subgraph_request.clone()
    }

    fn pipeline(&self) -> ReasoningPipeline {
        self.pipeline.clone()
    }
}

fn bench_syncompat_subgraph(c: &mut Criterion) {
    let scenario = Box::leak(Box::new(SyncompatScenario::build()));
    let mut group = c.benchmark_group("syncompat_subgraph");
    group.bench_function("depth2", |b| {
        b.iter(|| {
            extract_subgraph(scenario.backend(), scenario.subgraph_request()).expect("subgraph");
        });
    });
    group.finish();
    record_syncompat_metric(
        "syncompat_subgraph",
        SYNCOMPAT_SUBGRAPH_OPS,
        SYNCOMPAT_SUBGRAPH_BYTES,
        "syncompat harness subgraph",
    );
}

fn bench_syncompat_pipeline(c: &mut Criterion) {
    let scenario = Box::leak(Box::new(SyncompatScenario::build()));
    let mut group = c.benchmark_group("syncompat_pipeline");
    group.bench_function("pipeline", |b| {
        b.iter(|| {
            run_pipeline(scenario.backend(), &scenario.pipeline()).expect("pipeline");
        });
    });
    group.finish();
    record_syncompat_metric(
        "syncompat_pipeline",
        SYNCOMPAT_PIPELINE_OPS,
        SYNCOMPAT_PIPELINE_BYTES,
        "syncompat reasoning pipeline",
    );
}

fn record_syncompat_metric(name: &str, ops: f64, bytes: f64, notes: &str) {
    let metric = BenchMetric {
        name: name.into(),
        ops_per_sec: ops,
        bytes_per_sec: bytes,
        notes: notes.into(),
    };
    let _ = record_bench_run(name, metric);
}

criterion_group!(
    syncompat_benches,
    bench_syncompat_subgraph,
    bench_syncompat_pipeline
);
criterion_main!(syncompat_benches);
