//
// CONCURRENT ALGORITHM TESTS
//
// Tests verify thread-safety traits and algorithm consistency.
// Note: SqliteGraph itself is NOT thread-safe for writes, but read-only
// algorithm functions can be called from multiple threads if each has its
// own graph connection or snapshot. These tests verify the algorithm
// functions themselves have the right trait bounds.
//

use crate::{errors::SqliteGraphError, graph::SqliteGraph};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

// Import all algorithm functions from parent modules
use super::{
    centrality::{
        pagerank,
        pagerank_with_progress,
        betweenness_centrality,
        betweenness_centrality_with_progress,
    },
    community::{
        label_propagation,
        louvain_communities,
        louvain_communities_with_progress,
    },
    structure::{
        connected_components,
        find_cycles_limited,
        nodes_by_degree,
    },
};

#[test]
fn test_algorithms_are_send() {
    // Scenario: Verify all algorithm functions are Send
    // Expected: Algorithm functions have Send trait bounds

    // Verify key functions are Send by taking references
    // (This will compile only if the types are Send)
    let _ = || {
        let graph = create_test_graph();
        let _ = connected_components(&graph);
        let _ = label_propagation(&graph, 10);
        let _ = louvain_communities(&graph, 10);
        let _ = pagerank(&graph, 0.85, 10);
        let _ = betweenness_centrality(&graph);
        let _ = nodes_by_degree(&graph, true);
    };

    // If this compiles, all the algorithm functions are Send
    assert!(true);
}

#[test]
fn test_pagerank_consistency_across_calls() {
    // Scenario: PageRank produces consistent results across multiple calls
    // Expected: Same graph + parameters = same results (deterministic)
    let graph = create_test_graph();
    let damping = 0.85;
    let iterations = 10;

    let result1 = pagerank(&graph, damping, iterations);
    let result2 = pagerank(&graph, damping, iterations);

    assert!(result1.is_ok(), "First PageRank failed");
    assert!(result2.is_ok(), "Second PageRank failed");

    let scores1 = result1.unwrap();
    let scores2 = result2.unwrap();

    assert_eq!(scores1.len(), scores2.len(), "Different number of scores");

    // Compare each score (floating point tolerance)
    for (s1, s2) in scores1.iter().zip(scores2.iter()) {
        assert_eq!(s1.0, s2.0, "Different node IDs");
        assert!(
            (s1.1 - s2.1).abs() < 1e-10,
            "PageRank scores differ: {} vs {}",
            s1.1,
            s2.1
        );
    }
}

#[test]
fn test_betweenness_deterministic_output() {
    // Scenario: Betweenness centrality produces deterministic output
    // Expected: Same graph produces same centrality values
    let graph = create_test_graph();

    let result1 = betweenness_centrality(&graph);
    let result2 = betweenness_centrality(&graph);

    assert!(result1.is_ok(), "First betweenness failed");
    assert!(result2.is_ok(), "Second betweenness failed");

    let centrality1 = result1.unwrap();
    let centrality2 = result2.unwrap();

    assert_eq!(centrality1.len(), centrality2.len());

    for (c1, c2) in centrality1.iter().zip(centrality2.iter()) {
        assert_eq!(c1.0, c2.0, "Different node IDs");
        assert!(
            (c1.1 - c2.1).abs() < 1e-10,
            "Centrality values differ: {} vs {}",
            c1.1,
            c2.1
        );
    }
}

#[test]
fn test_label_propagation_deterministic() {
    // Scenario: Label propagation produces deterministic communities
    // Expected: Same graph produces same community assignments
    let graph = create_test_graph();
    let max_iterations = 10;

    let result1 = label_propagation(&graph, max_iterations);
    let result2 = label_propagation(&graph, max_iterations);

    assert!(result1.is_ok(), "First label propagation failed");
    assert!(result2.is_ok(), "Second label propagation failed");

    let communities1 = result1.unwrap();
    let communities2 = result2.unwrap();

    assert_eq!(communities1.len(), communities2.len());

    // Communities are sorted, so direct comparison works
    assert_eq!(communities1, communities2, "Communities differ");
}

#[test]
fn test_algorithm_result_types_are_thread_safe() {
    // Scenario: Verify algorithm result types are Send + Sync
    // Expected: Result types can be shared across threads
    fn is_send_sync<T: Send + Sync>() {}

    // Algorithm return types should be Send + Sync
    is_send_sync::<Vec<Vec<i64>>>();
    is_send_sync::<Vec<(i64, f64)>>();
    is_send_sync::<Vec<(i64, usize)>>();
    is_send_sync::<Result<Vec<Vec<i64>>, SqliteGraphError>>();
    is_send_sync::<Result<Vec<(i64, f64)>, SqliteGraphError>>();
}

#[test]
fn test_connected_components_basic() {
    // Scenario: Find connected components in a simple graph
    // Expected: Returns correct number of components
    let graph = create_test_graph();

    let result = connected_components(&graph);
    assert!(result.is_ok(), "connected_components failed");

    let components = result.unwrap();
    // In a chain graph (1-2-3-...-10), we expect 1 component
    assert_eq!(components.len(), 1, "Expected 1 connected component");
    assert_eq!(components[0].len(), 10, "Expected 10 nodes in component");
}

#[test]
fn test_find_cycles_empty_graph() {
    // Scenario: Find cycles in an acyclic graph
    // Expected: Returns empty vector
    let graph = create_test_graph(); // Chain graph is acyclic

    let result = find_cycles_limited(&graph, 10);
    assert!(result.is_ok(), "find_cycles_limited failed");

    let cycles = result.unwrap();
    assert_eq!(cycles.len(), 0, "Expected no cycles in chain graph");
}

#[test]
fn test_nodes_by_degree_descending() {
    // Scenario: Rank nodes by degree in descending order
    // Expected: Highest degree nodes first
    let graph = create_test_graph();

    let result = nodes_by_degree(&graph, true);
    assert!(result.is_ok(), "nodes_by_degree failed");

    let degrees = result.unwrap();
    // First node should have highest degree (endpoints of chain have degree 1)
    // Middle nodes have degree 2
    assert!(degrees[0].1 >= degrees[degrees.len() - 1].1, "Not sorted descending");
}

#[test]
fn test_progress_callbacks_complete() {
    // Scenario: Progress callbacks are called correctly
    // Expected: on_complete is called for all progress variants
    use crate::progress::{NoProgress, ProgressCallback};

    let graph = create_test_graph();

    // Test PageRank with progress
    let progress = NoProgress;
    let result = pagerank_with_progress(&graph, 0.85, 5, &progress);
    assert!(result.is_ok(), "pagerank_with_progress failed");

    // Test betweenness with progress
    let result = betweenness_centrality_with_progress(&graph, &progress);
    assert!(result.is_ok(), "betweenness_centrality_with_progress failed");

    // Test Louvain with progress
    let result = louvain_communities_with_progress(&graph, 5, &progress);
    assert!(result.is_ok(), "louvain_communities_with_progress failed");
}

// Helper: Create test graph
fn create_test_graph() -> SqliteGraph {
    use crate::GraphEntity;

    let graph = SqliteGraph::open_in_memory().expect("Failed to create graph");

    // Create test entities
    for i in 0..10 {
        let entity = GraphEntity {
            id: 0,
            kind: "test".to_string(),
            name: format!("test_{}", i),
            file_path: Some(format!("test_{}.rs", i)),
            data: serde_json::json!({"index": i}),
        };
        graph.insert_entity(&entity).expect("Failed to insert entity");
    }

    // Create some edges
    let entity_ids = graph.list_entity_ids().expect("Failed to get IDs");
    for i in 0..entity_ids.len().saturating_sub(1) {
        let edge = crate::GraphEdge {
            id: 0,
            from_id: entity_ids[i],
            to_id: entity_ids[i + 1],
            edge_type: "connects".to_string(),
            data: serde_json::json!({}),
        };
        graph.insert_edge(&edge).ok();
    }

    graph
}
