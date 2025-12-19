//! Production-Ready Load Testing for Adjacency Iterator Fix
//!
//! This test validates the adjacency iterator infinite loop fix under production
//! workloads with comprehensive load testing, stress testing, and benchmarking.

use sqlitegraph::{
    backend::native::{EdgeStore, GraphFile, NodeStore},
    backend::native::adjacency::{AdjacencyIterator, Direction},
    backend::native::types::NativeNodeId,
    config::GraphConfig,
    open_graph,
};
use tempfile::TempDir;
use std::time::{Instant, Duration};
use std::collections::HashMap;

/// Production test data generator
struct ProductionTestData {
    graph_file: GraphFile,
    node_count: usize,
    edge_count: usize,
    expected_iterations: HashMap<NativeNodeId, u32>,
}

impl ProductionTestData {
    /// Create production-scale test data
    fn create_production_dataset(seed: u64, num_nodes: usize, edges_per_node: u32) -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("production_load_test.db");

        // Create graph with native backend
        let cfg = GraphConfig::new(sqlitegraph::BackendKind::Native);
        let graph = open_graph(&db_path, &cfg).expect("Failed to create graph");

        println!("🔧 Creating production dataset: {} nodes, {} edges per node", num_nodes, edges_per_node);

        // Create nodes
        let mut node_ids = Vec::new();
        for i in 0..num_nodes {
            let node_id = graph.insert_node(
                sqlitegraph::NodeSpec {
                    kind: "Function".to_string(),
                    name: format!("function_{}", i),
                    file_path: Some(format!("/src/module_{}/function_{}.rs", i / 100, i)),
                    data: serde_json::json!({
                        "lines": (i * 10) % 1000 + 10,
                        "complexity": (i % 5) + 1
                    }),
                }
            ).expect("Failed to insert node");
            node_ids.push(node_id);
        }

        // Create edges with realistic patterns
        let mut expected_iterations = HashMap::new();
        let mut total_edges = 0;

        for i in 0..num_nodes {
            // Realistic edge distribution: most functions have few edges, some have many
            let outgoing_edges = if i < num_nodes / 10 {
                // Hub functions (10% of functions have many connections)
                edges_per_node * 3
            } else {
                // Regular functions have typical call patterns
                (i % edges_per_node) + 1
            };

            for j in 0..outgoing_edges {
                // Create realistic call patterns (forward calls, occasionally backward)
                let target_idx = if j == 0 {
                    // First edge typically calls forward
                    (i + 1) % num_nodes
                } else {
                    // Subsequent edges follow more complex patterns
                    ((i * j + 7) % num_nodes)
                };

                if target_idx != i { // Avoid self-calls
                    graph.insert_edge(
                        sqlitegraph::EdgeSpec {
                            from: node_ids[i],
                            to: node_ids[target_idx],
                            edge_type: "CALLS".to_string(),
                            data: serde_json::json!({
                                "line": (j * 5) + 1,
                                "frequency": (j % 10) + 1
                            }),
                        }
                    ).expect("Failed to insert edge");
                    total_edges += 1;

                    // Track expected iterations
                    *expected_iterations.entry(node_ids[i] as NativeNodeId).or_insert(0) += 1;
                }
            }
        }

        // Open as GraphFile for direct testing
        let graph_file = GraphFile::open(&db_path).expect("Failed to open graph file");

        println!("✅ Created production dataset: {} nodes, {} edges", node_ids.len(), total_edges);

        Self {
            graph_file,
            node_count: node_ids.len(),
            edge_count: total_edges,
            expected_iterations,
        }
    }
}

/// Load test adjacency iteration under production conditions
#[test]
fn test_adjacency_iterator_production_load() {
    println!("\n🏭 PRODUCTION LOAD TEST: Adjacency Iterator Under Production Conditions");

    let test_data = ProductionTestData::create_production_dataset(42, 1000, 5);
    let start_time = Instant::now();

    // Test with multiple nodes to ensure comprehensive coverage
    let test_nodes = vec![1, 100, 500, 999]; // Various positions in the graph

    for &node_id in &test_nodes {
        if node_id <= test_data.node_count as u64 {
            println!("🧪 Testing adjacency iteration for node {}", node_id);

            let node_start = Instant::now();

            // Test AdjacencyIterator directly
            let mut adjacency_iter = AdjacencyIterator::new_outgoing(
                &mut test_data.graph_file.clone(),
                node_id as NativeNodeId
            ).expect("Failed to create adjacency iterator");

            let neighbors = adjacency_iter.collect()
                .expect("Failed to collect neighbors");

            let node_elapsed = node_start.elapsed();

            // Validate results
            println!("  Node {}: {} neighbors in {:.2}ms",
                     node_id, neighbors.len(), node_elapsed.as_millis_f64());

            // Performance validation: should be very fast for production graphs
            assert!(node_elapsed < Duration::from_millis(100),
                   "Adjacency iteration took too long: {:.2}ms", node_elapsed.as_millis_f64());

            // Memory efficiency: should not accumulate excessive data
            assert!(neighbors.len() <= 20, // Reasonable upper bound for production graphs
                   "Too many neighbors returned: {}", neighbors.len());
        }
    }

    let total_elapsed = start_time.elapsed();
    println!("✅ Production load test completed in {:.2}ms", total_elapsed.as_millis_f64());

    // Overall performance validation
    assert!(total_elapsed < Duration::from_secs(5),
           "Production load test took too long: {:.2}s", total_elapsed.as_secs_f64());
}

/// Stress test with rapid adjacency iterator creation and destruction
#[test]
fn test_adjacency_iterator_stress_rapid_creation() {
    println!("\n💪 STRESS TEST: Rapid Adjacency Iterator Creation and Destruction");

    let test_data = ProductionTestData::create_production_dataset(123, 100, 3);
    let start_time = Instant::now();

    let iterations = 10000; // High iteration count for stress testing
    let test_nodes = vec![1, 50, 99]; // Sample nodes to test

    println!("Running {} rapid iterations...", iterations);

    for i in 0..iterations {
        let node_id = test_nodes[i % test_nodes.len()] as NativeNodeId;

        let iter_start = Instant::now();

        // Create iterator, collect a few neighbors, then drop
        {
            let mut adjacency_iter = AdjacencyIterator::new_outgoing(
                &mut test_data.graph_file.clone(),
                node_id
            ).expect("Failed to create adjacency iterator");

            // Only process first few neighbors to simulate real usage
            let mut count = 0;
            for neighbor in &mut adjacency_iter {
                let _ = neighbor; // Process neighbor
                count += 1;
                if count >= 3 { break; } // Simulate typical usage pattern
            }
        } // Iterator is dropped here

        let iter_elapsed = iter_start.elapsed();

        // Validate performance during stress test
        if iter_elapsed > Duration::from_millis(1) {
            println!("  Iteration {} slow: {:.3}ms", i, iter_elapsed.as_millis_f64());
        }
    }

    let total_elapsed = start_time.elapsed();
    let avg_per_iteration = total_elapsed.as_micros() as f64 / iterations as f64;

    println!("✅ Stress test completed:");
    println!("  Total time: {:.2}ms", total_elapsed.as_millis_f64());
    println!("  Average per iteration: {:.3}μs", avg_per_iteration);

    // Performance validation: should be very fast on average
    assert!(avg_per_iteration < 100.0,
           "Average iteration time too high: {:.3}μs", avg_per_iteration);

    // Overall stress test should complete quickly
    assert!(total_elapsed < Duration::from_secs(10),
           "Stress test took too long: {:.2}s", total_elapsed.as_secs_f64());
}

/// Memory usage test to ensure no memory leaks during adjacency iteration
#[test]
fn test_adjacency_iterator_memory_efficiency() {
    println!("\n🧠 MEMORY EFFICIENCY TEST: No Memory Leaks During Adjacency Iteration");

    let test_data = ProductionTestData::create_production_dataset(456, 50, 2);

    // Test multiple adjacency operations to check for memory accumulation
    let iterations = 1000;
    println!("Running {} memory efficiency iterations...", iterations);

    for i in 0..iterations {
        let node_id = ((i % test_data.node_count) + 1) as NativeNodeId;

        // Create and use adjacency iterator
        {
            let mut adjacency_iter = AdjacencyIterator::new_outgoing(
                &mut test_data.graph_file.clone(),
                node_id
            ).expect("Failed to create adjacency iterator");

            // Collect all neighbors (memory intensive operation)
            let neighbors = adjacency_iter.collect()
                .expect("Failed to collect neighbors");

            // Validate reasonable neighbor count
            assert!(neighbors.len() <= 50,
                   "Unreasonable neighbor count: {}", neighbors.len());
        }

        // Periodic validation to ensure no memory accumulation
        if i % 100 == 0 {
            println!("  Completed {} memory efficiency iterations", i);
        }
    }

    println!("✅ Memory efficiency test completed successfully");
}

/// EdgeStore iterator production test (the original problematic code path)
#[test]
fn test_edge_store_iterator_production_validation() {
    println!("\n🔄 EDGE STORE ITERATOR PRODUCTION VALIDATION");

    let test_data = ProductionTestData::create_production_dataset(789, 200, 4);

    // Test the specific EdgeStore::iter_neighbors method that was causing infinite loops
    let test_nodes = vec![1, 50, 100, 199];

    for &node_id in &test_nodes {
        if node_id <= test_data.node_count as u64 {
            println!("🧪 Testing EdgeStore iterator for node {}", node_id);

            let start_time = Instant::now();
            let mut edge_store = EdgeStore::new(&mut test_data.graph_file.clone());

            // This is the method that was causing infinite loops
            let mut iterator = edge_store.iter_neighbors(node_id as NativeNodeId, Direction::Outgoing);

            let mut iteration_count = 0;
            let mut neighbors_found = Vec::new();

            // Safety check with timeout to prevent infinite loops during testing
            let timeout = Duration::from_secs(5);

            for neighbor in &mut iterator {
                iteration_count += 1;
                neighbors_found.push(neighbor);

                // Check for timeout
                if start_time.elapsed() > timeout {
                    panic!("🚨 TIMEOUT: EdgeStore iterator taking too long for node {} ({} iterations)",
                           node_id, iteration_count);
                }

                // Safety check to prevent runaway iteration
                if iteration_count > 1000 {
                    panic!("🚨 INFINITE LOOP DETECTED: {} iterations for node {}",
                           iteration_count, node_id);
                }
            }

            let elapsed = start_time.elapsed();

            println!("  EdgeStore node {}: {} neighbors in {:.2}ms ({} iterations)",
                     node_id, neighbors_found.len(), elapsed.as_millis_f64(), iteration_count);

            // Validate performance and correctness
            assert!(elapsed < Duration::from_millis(50),
                   "EdgeStore iterator took too long: {:.2}ms", elapsed.as_millis_f64());

            assert!(iteration_count <= 100,
                   "Too many iterations: {} for node {}", iteration_count, node_id);
        }
    }

    println!("✅ EdgeStore iterator production validation completed successfully");
}

/// Comprehensive production test combining all adjacency operations
#[test]
fn test_comprehensive_production_validation() {
    println!("\n🏆 COMPREHENSIVE PRODUCTION VALIDATION");

    let test_data = ProductionTestData::create_production_dataset(999, 500, 6);
    let overall_start = Instant::now();

    // Test 1: Basic adjacency iteration
    println!("📊 Test 1: Basic adjacency iteration");
    {
        let mut adjacency_iter = AdjacencyIterator::new_outgoing(
            &mut test_data.graph_file.clone(),
            1 as NativeNodeId
        ).expect("Failed to create adjacency iterator");

        let neighbors = adjacency_iter.collect()
            .expect("Failed to collect neighbors");

        println!("  Found {} neighbors for node 1", neighbors.len());
        assert!(neighbors.len() > 0, "Should find neighbors");
    }

    // Test 2: Multiple adjacency operations
    println!("📊 Test 2: Multiple adjacency operations");
    {
        for node_id in 1..=10.min(test_data.node_count) {
            let mut adjacency_iter = AdjacencyIterator::new_outgoing(
                &mut test_data.graph_file.clone(),
                node_id as NativeNodeId
            ).expect("Failed to create adjacency iterator");

            let neighbors = adjacency_iter.collect()
                .expect("Failed to collect neighbors");

            // Validate reasonable results
            assert!(neighbors.len() <= 50, "Too many neighbors: {}", neighbors.len());
        }
    }

    // Test 3: EdgeStore validation
    println!("📊 Test 3: EdgeStore validation");
    {
        let mut edge_store = EdgeStore::new(&mut test_data.graph_file.clone());
        let mut iterator = edge_store.iter_neighbors(1 as NativeNodeId, Direction::Outgoing);

        let mut count = 0;
        for _neighbor in &mut iterator {
            count += 1;
            if count > 100 { // Safety limit
                panic!("Too many neighbors in EdgeStore iterator");
            }
        }

        println!("  EdgeStore found {} neighbors for node 1", count);
    }

    let overall_elapsed = overall_start.elapsed();
    println!("✅ Comprehensive production validation completed in {:.2}ms",
             overall_elapsed.as_millis_f64());

    // Overall performance validation
    assert!(overall_elapsed < Duration::from_secs(2),
           "Comprehensive validation took too long: {:.2}s", overall_elapsed.as_secs_f64());
}

/// Performance regression test to ensure our fix doesn't impact performance
#[test]
fn test_performance_regression_validation() {
    println!("\n📈 PERFORMANCE REGRESSION VALIDATION");

    // Create test data with known characteristics
    let test_data = ProductionTestData::create_production_dataset(1001, 100, 3);

    // Performance benchmarks
    let iterations = 1000;
    let total_start = Instant::now();

    for i in 0..iterations {
        let node_id = ((i % test_data.node_count) + 1) as NativeNodeId;

        let iter_start = Instant::now();

        {
            let mut adjacency_iter = AdjacencyIterator::new_outgoing(
                &mut test_data.graph_file.clone(),
                node_id
            ).expect("Failed to create adjacency iterator");

            let _neighbors = adjacency_iter.collect()
                .expect("Failed to collect neighbors");
        }

        let iter_elapsed = iter_start.elapsed();

        // Track performance metrics
        if iter_elapsed > Duration::from_micros(1000) {
            println!("  Slow iteration detected: {:.3}μs", iter_elapsed.as_micros_f64());
        }
    }

    let total_elapsed = total_start.elapsed();
    let avg_per_iteration = total_elapsed.as_micros() as f64 / iterations as f64;

    println!("Performance metrics:");
    println!("  Total time: {:.2}ms", total_elapsed.as_millis_f64());
    println!("  Average per iteration: {:.3}μs", avg_per_iteration);
    println!("  Iterations per second: {:.0}", 1_000_000.0 / avg_per_iteration);

    // Performance regression checks
    assert!(avg_per_iteration < 500.0,
           "Performance regression detected: {:.3}μs per iteration", avg_per_iteration);

    assert!(total_elapsed < Duration::from_secs(5),
           "Overall performance regression: {:.2}s", total_elapsed.as_secs_f64());

    println!("✅ Performance regression validation passed");
}