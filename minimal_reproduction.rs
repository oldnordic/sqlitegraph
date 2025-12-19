//! Minimal Reproduction Case for Adjacency Iterator Infinite Loop
//!
//! This script isolates the exact infinite loop behavior without the complexity
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

/// Direct AdjacencyIterator testing
fn test_adjacency_iterator_direct(graph_file: &mut GraphFile, node_id: NativeNodeId) {
    println!("\n🧪 Testing AdjacencyIterator directly...");

    let start_time = Instant::now();
    let mut adjacency_iter = AdjacencyIterator::new_outgoing(graph_file, node_id)
        .expect("Failed to create adjacency iterator");

    println!("Created iterator for node {}", node_id);
    println!("Iterator state: total_count={}, current_index={}",
             adjacency_iter.total_count(), adjacency_iter.current_index());

    let mut iteration_count = 0;
    let mut neighbors_found = Vec::new();

    for neighbor in &mut adjacency_iter {
        iteration_count += 1;
        neighbors_found.push(neighbor);

        let elapsed = start_time.elapsed();
        println!("Iteration {}: neighbor={}, elapsed={:.2}ms",
                 iteration_count, neighbor, elapsed.as_millis());

        // Safety check - prevent infinite loop
        if iteration_count > 50 {
            panic!("🚨 INFINITE LOOP DETECTED: {} iterations for simple graph", iteration_count);
        }

        if elapsed.as_secs() > 5 {
            panic!("🚨 TIMEOUT: Iterator taking too long ({:.2}s)", elapsed.as_secs_f64());
        }
    }

    println!("✅ AdjacencyIterator completed: {} iterations, {} neighbors",
             iteration_count, neighbors_found.len());
}

/// Test EdgeStore::iter_neighbors method (the problematic one)
fn test_edge_store_iterator(graph_file: &mut GraphFile, node_id: NativeNodeId) {
    println!("\n🧪 Testing EdgeStore::iter_neighbors (problematic method)...");

    let start_time = Instant::now();
    let mut edge_store = EdgeStore::new(graph_file);

    // This method returns Box<dyn Iterator> that wraps AdjacencyIterator
    let mut iterator = edge_store.iter_neighbors(node_id, Direction::Outgoing);

    println!("Created EdgeStore iterator for node {}", node_id);

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
}

/// Test adjacency collection (the collect() method that should show instrumentation)
fn test_adjacency_collection(graph_file: &mut GraphFile, node_id: NativeNodeId) {
    println!("\n🧪 Testing adjacency collection with instrumentation...");

    let start_time = Instant::now();
    let mut adjacency_iter = AdjacencyIterator::new_outgoing(graph_file, node_id)
        .expect("Failed to create adjacency iterator");

    println!("Calling collect() on iterator...");

    // This should trigger our instrumentation
    let neighbors = adjacency_iter.collect()
        .expect("Failed to collect neighbors");

    let elapsed = start_time.elapsed();
    println!("✅ Collection completed in {:.2}ms: {} neighbors",
             elapsed.as_millis(), neighbors.len());
}

fn main() {
    println!("🔍 Phase 2: Minimal Reproduction Case");
    println!("Testing adjacency iterator infinite loop with instrumentation");

    // Enable debug output
    std::env::set_var("RUST_LOG", "debug");

    let (node1_id, node2_id, mut graph_file) = create_minimal_test();

    // Test different approaches
    test_adjacency_iterator_direct(&mut graph_file, node1_id);
    test_edge_store_iterator(&mut graph_file, node1_id);
    test_adjacency_collection(&mut graph_file, node1_id);

    println!("\n🎉 All tests completed successfully!");
    println!("No infinite loops detected in current implementation");
}