//! Minimal Reproduction Test for Adjacency Iterator Infinite Loop
//!
//! This test isolates the exact infinite loop behavior without the complexity
//! of the full test infrastructure. It directly exercises the problematic code path.

use sqlitegraph::{
    backend::native::{EdgeStore, GraphFile, NodeStore},
    backend::native::adjacency::{AdjacencyIterator, Direction},
    backend::native::types::NativeNodeId,
    config::GraphConfig,
    open_graph,
};
use tempfile::TempDir;
use std::time::Instant;

/// Create minimal test setup
fn create_minimal_test() -> (NativeNodeId, NativeNodeId, GraphFile) {
    println!("🔧 Creating minimal test setup...");

    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let db_path = temp_dir.path().join("minimal_test.db");

    // Create graph with native backend
    let cfg = GraphConfig::new(sqlitegraph::BackendKind::Native);
    let graph = open_graph(&db_path, &cfg).expect("Failed to create graph");

    // Insert nodes
    let node1_id = graph.insert_node(
        sqlitegraph::NodeSpec {
            kind: "Function".to_string(),
            name: "node1".to_string(),
            file_path: Some("/src/node1.rs".to_string()),
            data: serde_json::json!({"lines": 10}),
        }
    ).expect("Failed to insert node1");

    let node2_id = graph.insert_node(
        sqlitegraph::NodeSpec {
            kind: "Function".to_string(),
            name: "node2".to_string(),
            file_path: Some("/src/node2.rs".to_string()),
            data: serde_json::json!({"lines": 20}),
        }
    ).expect("Failed to insert node2");

    // Create edge
    graph.insert_edge(
        sqlitegraph::EdgeSpec {
            from: node1_id,
            to: node2_id,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({"line": 5}),
        }
    ).expect("Failed to insert edge");

    // Open as GraphFile for direct testing
    let graph_file = GraphFile::open(&db_path).expect("Failed to open graph file");

    println!("✅ Created test: node1 ({}) -> node2 ({})", node1_id, node2_id);
    (node1_id as NativeNodeId, node2_id as NativeNodeId, graph_file)
}

/// Test adjacency collection (the collect() method that should show instrumentation)
#[test]
fn test_adjacency_collection_instrumentation() {
    let (node1_id, _node2_id, mut graph_file) = create_minimal_test();

    println!("\n🧪 Testing adjacency collection with instrumentation...");

    let start_time = Instant::now();
    let mut adjacency_iter = AdjacencyIterator::new_outgoing(&mut graph_file, node1_id)
        .expect("Failed to create adjacency iterator");

    println!("Calling collect() on iterator...");

    // This should trigger our instrumentation
    let neighbors = adjacency_iter.collect()
        .expect("Failed to collect neighbors");

    let elapsed = start_time.elapsed();
    println!("✅ Collection completed in {:.2}ms: {} neighbors",
             elapsed.as_millis(), neighbors.len());

    // Verify results
    assert_eq!(neighbors.len(), 1, "Should find exactly 1 neighbor");
}

/// Test EdgeStore::iter_neighbors method (the problematic one)
#[test]
fn test_edge_store_iterator_reproduction() {
    let (node1_id, _node2_id, mut graph_file) = create_minimal_test();

    println!("\n🧪 Testing EdgeStore::iter_neighbors (problematic method)...");

    let start_time = Instant::now();
    let mut edge_store = EdgeStore::new(&mut graph_file);

    // This method returns Box<dyn Iterator> that wraps AdjacencyIterator
    let mut iterator = edge_store.iter_neighbors(node1_id, Direction::Outgoing);

    println!("Created EdgeStore iterator for node {}", node1_id);

    let mut iteration_count = 0;
    let mut neighbors_found = Vec::new();

    for neighbor in &mut iterator {
        iteration_count += 1;
        neighbors_found.push(neighbor);

        let elapsed = start_time.elapsed();
        println!("EdgeStore iteration {}: neighbor={}, elapsed={:.2}ms",
                 iteration_count, neighbor, elapsed.as_millis());

        // Safety check - prevent infinite loop
        if iteration_count > 50 {
            panic!("🚨 INFINITE LOOP DETECTED in EdgeStore: {} iterations", iteration_count);
        }

        if elapsed.as_secs() > 5 {
            panic!("🚨 TIMEOUT: EdgeStore iterator taking too long ({:.2}s)", elapsed.as_secs_f64());
        }
    }

    println!("✅ EdgeStore iterator completed: {} iterations, {} neighbors",
             iteration_count, neighbors_found.len());

    // Verify results
    assert_eq!(neighbors_found.len(), 1, "Should find exactly 1 neighbor");
    assert_eq!(iteration_count, 1, "Should only need 1 iteration for 1 neighbor");
}

/// Test with instrumentation data collection
#[test]
fn test_instrumentation_data_collection() {
    let (node1_id, _node2_id, mut graph_file) = create_minimal_test();

    println!("\n🧪 Testing instrumentation data collection...");

    // Reset instrumentation metrics
    #[cfg(debug_assertions)]
    {
        use sqlitegraph::backend::native::adjacency::instrumentation::convenience::get_global_metrics;
        get_global_metrics().reset();
    }

    let start_time = Instant::now();

    // Perform multiple adjacency operations to trigger instrumentation
    for i in 0..5 {
        println!("Running iteration {}...", i + 1);

        let mut adjacency_iter = AdjacencyIterator::new_outgoing(&mut graph_file, node1_id)
            .expect("Failed to create adjacency iterator");

        let neighbors = adjacency_iter.collect()
            .expect("Failed to collect neighbors");

        assert_eq!(neighbors.len(), 1, "Should always find 1 neighbor");
    }

    let elapsed = start_time.elapsed();
    println!("✅ All operations completed in {:.2}ms", elapsed.as_millis());

    // Check instrumentation data (only in debug builds)
    #[cfg(debug_assertions)]
    {
        use sqlitegraph::backend::native::adjacency::instrumentation::convenience::get_global_metrics;
        let metrics = get_global_metrics().get_snapshot();

        println!("\n📊 Instrumentation Metrics:");
        println!("  Total iterations: {}", metrics.total_iterations);
        println!("  Total V2 reads: {}", metrics.total_v2_reads);
        println!("  Infinite loop detections: {}", metrics.infinite_loop_detections);
        println!("  Iteration efficiency: {:.2}", metrics.iteration_efficiency());

        // Verify reasonable metrics
        assert!(metrics.total_iterations > 0, "Should record iterations");
        assert_eq!(metrics.infinite_loop_detections, 0, "Should not detect infinite loops");
        assert!(metrics.iteration_efficiency() > 0.0, "Should have positive efficiency");
    }
}