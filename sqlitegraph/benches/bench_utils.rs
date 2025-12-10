//! Common benchmark utilities for backend performance comparison.
//!
//! Provides utilities for creating deterministic test graphs and running
//! fair performance comparisons between SQLite and Native backends.

use std::collections::HashMap;
use std::time::Duration;
use tempfile::TempDir;

use rand::{RngCore, SeedableRng};
use sqlitegraph::{BackendKind, EdgeSpec, GraphConfig, NodeSpec, open_graph};

/// Common benchmark configuration
pub const SAMPLE_SIZE: usize = 20;
pub const WARM_UP: Duration = Duration::from_millis(300);
pub const MEASURE: Duration = Duration::from_millis(500);

/// Graph topology types for benchmarking
#[derive(Debug, Clone, Copy)]
pub enum GraphTopology {
    /// Linear chain of nodes
    Chain,
    /// Star topology (one central node)
    Star,
    /// Grid topology (2D grid)
    Grid,
    /// Random graph
    Random,
}

/// Benchmark graph specification
#[derive(Debug, Clone)]
pub struct BenchmarkGraph {
    pub node_count: usize,
    pub edge_count: usize,
    pub topology: GraphTopology,
    pub seed: u64,
}

impl BenchmarkGraph {
    /// Create a new benchmark graph specification
    pub fn new(node_count: usize, edge_count: usize, topology: GraphTopology) -> Self {
        Self {
            node_count,
            edge_count,
            topology,
            seed: 0x5F3759DF, // Deterministic seed
        }
    }
}

/// Create a temporary directory for benchmark files
pub fn create_benchmark_temp_dir() -> TempDir {
    TempDir::new().expect("Failed to create temp directory for benchmark")
}

/// Create a benchmark graph with the specified backend and return statistics
pub fn create_benchmark_graph(backend: BackendKind, spec: &BenchmarkGraph) -> BenchmarkResult {
    let temp_dir = create_benchmark_temp_dir();
    let db_path = temp_dir.path().join("benchmark.db");

    let config = match backend {
        BackendKind::SQLite => GraphConfig::sqlite(),
        BackendKind::Native => GraphConfig::native(),
    };
    let graph = open_graph(&db_path, &config).expect("Failed to create benchmark graph");

    let start_time = std::time::Instant::now();

    // Generate nodes using individual insertions
    let mut node_ids = Vec::with_capacity(spec.node_count);
    for i in 0..spec.node_count {
        let node_id = graph
            .insert_node(NodeSpec {
                kind: "Node".to_string(),
                name: format!("node_{}", i),
                file_path: None,
                data: serde_json::json!({
                    "id": i,
                    "created_at": "benchmark",
                }),
            })
            .expect("Failed to insert node");
        node_ids.push(node_id);
    }

    // Generate edges based on topology using individual insertions
    let edge_count = generate_edges(&graph, &node_ids, spec);

    let creation_time = start_time.elapsed();

    BenchmarkResult {
        backend,
        node_count: node_ids.len(),
        edge_count: edge_count,
        creation_time,
        temp_dir,
        db_path,
    }
}

/// Result of creating a benchmark graph
pub struct BenchmarkResult {
    pub backend: BackendKind,
    pub node_count: usize,
    pub edge_count: usize,
    pub creation_time: Duration,
    pub temp_dir: TempDir,
    pub db_path: std::path::PathBuf,
}

/// Generate edges based on the specified topology
fn generate_edges(
    graph: &Box<dyn sqlitegraph::GraphBackend>,
    node_ids: &[i64],
    spec: &BenchmarkGraph,
) -> usize {
    let mut edge_count = 0;
    let mut rng = rand::rngs::StdRng::seed_from_u64(spec.seed);

    match spec.topology {
        GraphTopology::Chain => {
            for i in 0..node_ids.len().min(spec.edge_count) {
                if i + 1 < node_ids.len() {
                    graph
                        .insert_edge(EdgeSpec {
                            from: node_ids[i],
                            to: node_ids[i + 1],
                            edge_type: "chain".to_string(),
                            data: serde_json::json!({"order": i}),
                        })
                        .expect("Failed to insert edge");
                    edge_count += 1;
                }
            }
        }
        GraphTopology::Star => {
            if node_ids.is_empty() {
                return 0;
            }
            let center = node_ids[0];
            for i in 1..node_ids.len().min(spec.edge_count + 1) {
                graph
                    .insert_edge(EdgeSpec {
                        from: center,
                        to: node_ids[i],
                        edge_type: "star".to_string(),
                        data: serde_json::json!({"spoke": i}),
                    })
                    .expect("Failed to insert edge");
                edge_count += 1;
            }
        }
        GraphTopology::Grid => {
            let grid_size = (node_ids.len() as f64).sqrt() as usize;
            for i in 0..node_ids.len().min(spec.edge_count) {
                let row = i / grid_size;
                let col = i % grid_size;

                // Right neighbor
                if col + 1 < grid_size && i + 1 < node_ids.len() && edge_count < spec.edge_count {
                    graph
                        .insert_edge(EdgeSpec {
                            from: node_ids[i],
                            to: node_ids[i + 1],
                            edge_type: "grid".to_string(),
                            data: serde_json::json!({"direction": "right"}),
                        })
                        .expect("Failed to insert edge");
                    edge_count += 1;
                }

                // Bottom neighbor
                if row + 1 < grid_size
                    && i + grid_size < node_ids.len()
                    && edge_count < spec.edge_count
                {
                    graph
                        .insert_edge(EdgeSpec {
                            from: node_ids[i],
                            to: node_ids[i + grid_size],
                            edge_type: "grid".to_string(),
                            data: serde_json::json!({"direction": "down"}),
                        })
                        .expect("Failed to insert edge");
                    edge_count += 1;
                }
            }
        }
        GraphTopology::Random => {
            use rand::Rng;
            for _ in 0..spec.edge_count {
                let from_idx = rng.gen_range(0..node_ids.len());
                let mut to_idx = rng.gen_range(0..node_ids.len());
                while to_idx == from_idx {
                    to_idx = rng.gen_range(0..node_ids.len());
                }

                graph
                    .insert_edge(EdgeSpec {
                        from: node_ids[from_idx],
                        to: node_ids[to_idx],
                        edge_type: "random".to_string(),
                        data: serde_json::json!({"random_id": rng.next_u64()}),
                    })
                    .expect("Failed to insert edge");
                edge_count += 1;
            }
        }
    }

    edge_count
}

/// Common benchmark graph sizes
pub const BENCHMARK_SIZES: &[usize] = &[100, 1_000, 10_000];

/// Small benchmark graphs for quick testing
pub const SMALL_SIZES: &[usize] = &[10, 50];

/// Medium benchmark graphs for typical performance testing
pub const MEDIUM_SIZES: &[usize] = &[1_000, 5_000];

/// Large benchmark graphs for stress testing
pub const LARGE_SIZES: &[usize] = &[10_000, 20_000];
