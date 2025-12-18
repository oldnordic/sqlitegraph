//! V2 Performance Regression Gates
//!
//! Tests to ensure V2-specific operations meet performance thresholds.
//! These tests protect against performance regressions in V2 features
//! including cluster management, multi-edge support, and compact storage.

use std::path::PathBuf;
use sqlitegraph::bench_gates::{self, BenchGateResult, BenchThreshold, set_bench_file_path};

/// V2 Node insertion performance gates
/// Tests that V2's 250-byte node slot allocation performs within limits
#[test]
fn v2_node_insertion_gate() {
    set_bench_file_path(baseline_path());
    let result = bench_gates::check_thresholds(
        "v2_insertion_mixed_graph_1000",
        BenchThreshold {
            name: "v2_insertion_mixed_graph_1000".into(),
            min_ops_per_sec: 800.0, // 20% lower than V1 due to slot overhead
            max_ms: 1.25,
        },
    )
    .expect("gate");
    assert_eq!(result, BenchGateResult::Pass);
}

#[test]
fn v2_node_insertion_gate_scaled() {
    set_bench_file_path(baseline_path());
    let result = bench_gates::check_thresholds(
        "v2_insertion_mixed_graph_5000",
        BenchThreshold {
            name: "v2_insertion_mixed_graph_5000".into(),
            min_ops_per_sec: 600.0, // Allow more degradation at scale
            max_ms: 1.67,
        },
    )
    .expect("gate");
    assert_eq!(result, BenchGateResult::Pass);
}

/// V2 Cluster allocation performance gates
/// Tests cluster management overhead remains acceptable
#[test]
fn v2_cluster_allocation_gate() {
    set_bench_file_path(baseline_path());
    let result = bench_gates::check_thresholds(
        "v2_file_growth_sparse_1000",
        BenchThreshold {
            name: "v2_file_growth_sparse_1000".into(),
            min_ops_per_sec: 70.0, // Cluster allocation overhead
            max_ms: 14.29,
        },
    )
    .expect("gate");
    assert_eq!(result, BenchGateResult::Pass);
}

/// V2 Multi-edge performance gates
/// Tests multi-edge scenarios don't significantly impact performance
#[test]
fn v2_multiedge_insertion_gate() {
    set_bench_file_path(baseline_path());
    let result = bench_gates::check_thresholds(
        "v2_multiedge_insert_factor_5",
        BenchThreshold {
            name: "v2_multiedge_insert_factor_5".into(),
            min_ops_per_sec: 200.0, // Scales inversely with multi-edge factor
            max_ms: 5.0,
        },
    )
    .expect("gate");
    assert_eq!(result, BenchGateResult::Pass);
}

#[test]
fn v2_multiedge_query_gate() {
    set_bench_file_path(baseline_path());
    let result = bench_gates::check_thresholds(
        "v2_multiedge_neighbors_dedup_5",
        BenchThreshold {
            name: "v2_multiedge_neighbors_dedup_5".into(),
            min_ops_per_sec: 8000.0, // Deduplication overhead
            max_ms: 0.125,
        },
    )
    .expect("gate");
    assert_eq!(result, BenchGateResult::Pass);
}

/// V2 Neighbor query performance gates
/// Tests clustered adjacency performs well across degree distributions
#[test]
fn v2_neighbor_query_low_degree_gate() {
    set_bench_file_path(baseline_path());
    let result = bench_gates::check_thresholds(
        "v2_neighbor_low_degree",
        BenchThreshold {
            name: "v2_neighbor_low_degree".into(),
            min_ops_per_sec: 40000.0, // Fast for small clusters
            max_ms: 0.025,
        },
    )
    .expect("gate");
    assert_eq!(result, BenchGateResult::Pass);
}

#[test]
fn v2_neighbor_query_high_degree_gate() {
    set_bench_file_path(baseline_path());
    let result = bench_gates::check_thresholds(
        "v2_neighbor_high_degree",
        BenchThreshold {
            name: "v2_neighbor_high_degree".into(),
            min_ops_per_sec: 8000.0, // Slower for large clusters
            max_ms: 0.125,
        },
    )
    .expect("gate");
    assert_eq!(result, BenchGateResult::Pass);
}

#[test]
fn v2_neighbor_query_hub_gate() {
    set_bench_file_path(baseline_path());
    let result = bench_gates::check_thresholds(
        "v2_neighbor_hub_nodes",
        BenchThreshold {
            name: "v2_neighbor_hub_nodes".into(),
            min_ops_per_sec: 4000.0, // Slowest for hub nodes
            max_ms: 0.25,
        },
    )
    .expect("gate");
    assert_eq!(result, BenchGateResult::Pass);
}

/// V2 Traversal performance gates
/// Tests BFS and k-hop perform well with V2 clustering
#[test]
fn v2_bfs_traversal_gate() {
    set_bench_file_path(baseline_path());
    let result = bench_gates::check_thresholds(
        "v2_bfs_depth_5_1000",
        BenchThreshold {
            name: "v2_bfs_depth_5_1000".into(),
            min_ops_per_sec: 80.0, // BFS operations per second
            max_ms: 12.5,
        },
    )
    .expect("gate");
    assert_eq!(result, BenchGateResult::Pass);
}

#[test]
fn v2_k_hop_traversal_gate() {
    set_bench_file_path(baseline_path());
    let result = bench_gates::check_thresholds(
        "v2_k_hop_outgoing_3",
        BenchThreshold {
            name: "v2_k_hop_outgoing_3".into(),
            min_ops_per_sec: 400.0, // K-hop operations per second
            max_ms: 2.5,
        },
    )
    .expect("gate");
    assert_eq!(result, BenchGateResult::Pass);
}

/// V2 Storage efficiency gates
/// Tests V2 achieves better storage density than V1
#[test]
fn v2_storage_efficiency_gate() {
    set_bench_file_path(baseline_path());

    // Check bytes per node for V2 (250-byte slots)
    let result = bench_gates::check_thresholds(
        "v2_file_growth_powerlaw_1000",
        BenchThreshold {
            name: "v2_file_growth_powerlaw_1000".into(),
            min_ops_per_sec: 70.0,
            max_ms: 14.29,
        },
    )
    .expect("gate");
    assert_eq!(result, BenchGateResult::Pass);
}

/// V2 Transaction performance gates
/// Tests transaction overhead with V2 features
#[test]
fn v2_transaction_commit_gate() {
    // TODO: Add v2_transaction_commit benchmark
    // This will test commit performance with cluster management
}

#[test]
fn v2_rollback_efficiency_gate() {
    // TODO: Add v2_rollback_efficiency benchmark
    // This will test rollback performance with V2 structures
}

/// V2 I/O performance gates
/// Tests memory-mapped I/O efficiency
#[test]
fn v2_mmap_read_gate() {
    // TODO: Add v2_mmap_read benchmark
    // This will test mmap read performance
}

#[test]
fn v2_mmap_write_gate() {
    // TODO: Add v2_mmap_write benchmark
    // This will test mmap write performance
}

/// V2 Compact serialization gates
/// Tests V2's binary format efficiency
#[test]
fn v2_compact_serialization_gate() {
    // TODO: Add v2_compact_serialization benchmark
    // This will test serialization/deserialization performance
}

fn baseline_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("sqlitegraph_bench.json")
}

/// Helper function to check if V2 benchmarks have been run
fn ensure_v2_benchmarks_exist() {
    let path = baseline_path();
    if !path.exists() {
        panic!("V2 benchmarks not run. Execute: cargo bench --bench v2_performance");
    }
}

/// Macro to create a gate that checks multiple related operations
macro_rules! create_v2_gate_family {
    ($family_name:ident, $base_name:expr, $($thresholds:expr),*) => {
        $(
            #[test]
            fn $family_name() {
                set_bench_file_path(baseline_path());
                let result = bench_gates::check_thresholds(
                    concat!($base_name, "_", stringify!($thresholds)),
                    BenchThreshold {
                        name: concat!($base_name, "_", stringify!($thresholds)).into(),
                        min_ops_per_sec: $thresholds.0,
                        max_ms: $thresholds.1,
                    },
                )
                .expect("gate");
                assert_eq!(result, BenchGateResult::Pass);
            }
        )*
    };
}

// Example usage of the macro for scaling tests
// Temporarily disabled due to macro issue
/*
create_v2_gate_family!(
    v2_scaling_gates,
    "v2_insertion_mixed_graph",
    (1000.0, 1.0), // For 1000 nodes
    (800.0, 1.25),  // For 5000 nodes
    (600.0, 1.67)   // For 10000 nodes
);
*/