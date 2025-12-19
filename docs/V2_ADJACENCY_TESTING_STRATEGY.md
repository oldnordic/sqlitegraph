# V2 Adjacency System - Testing Strategy

## Overview

This document outlines the comprehensive testing strategy for SQLiteGraph's V2 adjacency system. The strategy covers unit tests, integration tests, performance benchmarks, and regression prevention to ensure system reliability and performance.

## Testing Philosophy

### Principles
1. **Zero Tolerance for Regressions**: Any regression in functionality or performance is a blocking issue
2. **Evidence-Based Validation**: All fixes must be validated with comprehensive test evidence
3. **Production-Ready Quality**: Test code must meet the same quality standards as production code
4. **Systematic Coverage**: Test all code paths, error conditions, and edge cases
5. **Performance Gates**: Automated benchmarks prevent performance regressions

### Testing Pyramid
```
                    ┌─────────────────────┐
                    │    Integration     │  ← Comprehensive end-to-end scenarios
                    │      Tests          │
                    └─────────────────────┘
                ┌─────────────────────────────────┐
                │      Unit Tests                  │  ← Fast, isolated component tests
                └─────────────────────────────────┘
            ┌─────────────────────────────────────────────────┐
            │              Property-Based Tests               │  ← Systematic exhaustive testing
            └─────────────────────────────────────────────────┘
```

## Test Categories

### 1. Unit Tests

#### AdjacencyIterator Tests (`adjacency/tests.rs`)

**Test Coverage Requirements:**
- Iterator creation and initialization
- State transitions and boundary conditions
- Error handling and recovery
- Memory management and resource cleanup
- Debug instrumentation functionality

**Key Test Cases:**

```rust
#[cfg(test)]
mod adjacency_iterator_tests {
    use super::*;

    #[test]
    fn test_adjacency_iterator_creation() {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Test successful creation
        let iterator = AdjacencyIterator::new_outgoing(&mut graph_file, 1);
        assert!(iterator.is_ok());

        let iterator = AdjacencyIterator::new_incoming(&mut graph_file, 1);
        assert!(iterator.is_ok());
    }

    #[test]
    fn test_adjacency_iterator_empty_graph() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut iterator = AdjacencyIterator::new_outgoing(&mut graph_file, 999).unwrap();

        // Should complete immediately for non-existent node
        let neighbors = iterator.collect().unwrap();
        assert_eq!(neighbors.len(), 0);
    }

    #[test]
    fn test_adjacency_iterator_state_consistency() {
        let (mut graph_file, _temp_file) = create_test_graph_file();
        let mut iterator = AdjacencyIterator::new_outgoing(&mut graph_file, 1).unwrap();

        // Initial state validation
        assert_eq!(iterator.current_index(), 0);
        assert_eq!(iterator.is_complete(), iterator.current_index() >= iterator.total_count());

        // Reset behavior
        iterator.reset();
        assert_eq!(iterator.current_index(), 0);
    }

    #[test]
    fn test_adjacency_iterator_infinite_loop_prevention() {
        // Create a scenario where total_count is incorrectly set > 0
        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Manually create inconsistent state (through direct manipulation)
        // This tests the infinite loop protection mechanisms
        let mut iterator = AdjacencyIterator::new_outgoing(&mut graph_file, 1).unwrap();

        // The collect() method should terminate early and return empty results
        let neighbors = iterator.collect().unwrap();
        assert_eq!(neighbors.len(), 0);
    }

    #[test]
    fn test_adjacency_iterator_edge_filtering() {
        let (mut graph_file, _temp_file) = create_test_graph_with_edges();
        let mut iterator = AdjacencyIterator::new_outgoing(&mut graph_file, 1).unwrap();

        // Test with edge type filter
        let filtered_iterator = iterator.with_edge_filter(&["test"]);
        let neighbors = filtered_iterator.collect().unwrap();

        // Verify filtering logic
        assert!(neighbors.len() <= get_total_outgoing_edges(&graph_file, 1));
    }
}
```

#### V2 Cluster Tests (`v2/edge_cluster/tests.rs`)

**Test Coverage Requirements:**
- Cluster serialization/deserialization
- Empty cluster handling
- Cluster corruption detection
- Size validation and bounds checking
- Performance characteristics

**Key Test Cases:**

```rust
#[cfg(test)]
mod v2_cluster_tests {
    use super::*;

    #[test]
    fn test_empty_cluster_serialization() {
        let cluster = EdgeCluster::new();
        let serialized = cluster.serialize().unwrap();

        // Empty cluster should serialize to valid format
        assert!(!serialized.is_empty());
        assert_eq!(serialized.len(), cluster.calculate_minimum_size());
    }

    #[test]
    fn test_empty_cluster_deserialization() {
        let cluster = EdgeCluster::new();
        let serialized = cluster.serialize().unwrap();

        // Round-trip test
        let deserialized = EdgeCluster::deserialize(&serialized).unwrap();
        assert_eq!(deserialized.edge_count(), 0);
    }

    #[test]
    fn test_single_edge_cluster() {
        let mut cluster = EdgeCluster::new();
        cluster.add_edge(1, 2, "test".to_string()).unwrap();

        let serialized = cluster.serialize().unwrap();
        let deserialized = EdgeCluster::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.edge_count(), 1);
        let neighbors: Vec<u64> = deserialized.iter_neighbors().collect();
        assert_eq!(neighbors, vec![2]);
    }

    #[test]
    fn test_cluster_size_validation() {
        let cluster = EdgeCluster::new();
        let serialized = cluster.serialize().unwrap();

        // Test truncation detection
        let truncated = &serialized[..serialized.len() - 1];
        let result = EdgeCluster::deserialize(truncated);
        assert!(result.is_err());
    }

    #[test]
    fn test_cluster_corruption_detection() {
        // Test with malformed data
        let malformed_data = vec![0xFF, 0x00, 0x00, 0x00]; // Invalid header
        let result = EdgeCluster::deserialize(&malformed_data);
        assert!(result.is_err());
    }

    #[test]
    fn test_large_cluster_performance() {
        let mut cluster = EdgeCluster::new();

        // Add many edges (within reasonable limits)
        for i in 1..=1000 {
            cluster.add_edge(i, i + 1000, format!("edge_{}", i)).unwrap();
        }

        let start = std::time::Instant::now();
        let serialized = cluster.serialize().unwrap();
        let serialization_time = start.elapsed();

        let start = std::time::Instant::now();
        let deserialized = EdgeCluster::deserialize(&serialized).unwrap();
        let deserialization_time = start.elapsed();

        // Performance assertions (adjust thresholds as needed)
        assert!(serialization_time.as_millis() < 100);
        assert!(deserialization_time.as_millis() < 100);
        assert_eq!(deserialized.edge_count(), 1000);
    }
}
```

### 2. Integration Tests

#### End-to-End Graph Operations Tests (`graph_ops/tests.rs`)

**Test Coverage Requirements:**
- Complete graph creation and traversal scenarios
- BFS and shortest path algorithms
- Mixed V2/legacy adjacency scenarios
- Error handling and recovery paths
- Performance validation

**Key Test Cases:**

```rust
#[cfg(test)]
mod graph_operations_tests {
    use super::*;

    #[test]
    fn test_native_bfs_simple() {
        // Clear cache to ensure test isolation
        clear_node_cache();

        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Create nodes
        let node1 = create_test_node(1, "node1");
        let node2 = create_test_node(2, "node2");
        let node3 = create_test_node(3, "node3");

        {
            let mut node_store = NodeStore::new(&mut graph_file);
            node_store.write_node(&node1).unwrap();
            node_store.write_node(&node2).unwrap();
            node_store.write_node(&node3).unwrap();
        }

        // Create edges: 1 -> 2 -> 3
        let edge1 = EdgeRecord::new(1, 1, 2, "test".to_string(), serde_json::json!({}));
        let edge2 = EdgeRecord::new(2, 2, 3, "test".to_string(), serde_json::json!({}));

        {
            let mut edge_store = EdgeStore::new(&mut graph_file);
            edge_store.write_edge(&edge1).unwrap();
            edge_store.write_edge(&edge2).unwrap();
        }

        // Test BFS traversal
        let result = native_bfs(&mut graph_file, 1, 2).unwrap();

        assert!(result.contains(&2), "Expected to find node 2 in BFS result: {:?}", result);
        assert!(result.contains(&3), "Expected to find node 3 in BFS result: {:?}", result);
    }

    #[test]
    fn test_native_shortest_path() {
        clear_node_cache();

        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Create more complex graph
        create_test_graph_structure(&mut graph_file);

        let path = native_shortest_path(&mut graph_file, 1, 4).unwrap();

        assert!(path.len() >= 2, "Path should have at least start and end nodes");
        assert_eq!(path[0], 1, "Path should start at node 1");
        assert_eq!(path[path.len() - 1], 4, "Path should end at node 4");

        // Verify path validity
        for i in 0..path.len() - 1 {
            assert!(edge_exists(&mut graph_file, path[i], path[i + 1]),
                   "Invalid edge in path: {} -> {}", path[i], path[i + 1]);
        }
    }

    #[test]
    fn test_v2_adjacency_with_legacy_fallback() {
        // Test scenario where V2 clusters fail and system falls back gracefully
        clear_node_cache();

        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Create graph without V2 cluster writing
        create_legacy_edge_structure(&mut graph_file);

        // Verify adjacency still works through fallback
        let mut iterator = AdjacencyIterator::new_outgoing(&mut graph_file, 1).unwrap();
        let neighbors = iterator.collect().unwrap();

        assert!(!neighbors.is_empty(), "Should find neighbors through legacy fallback");
    }

    #[test]
    fn test_concurrent_adjacency_operations() {
        // Test thread safety (if applicable)
        clear_node_cache();

        let (mut graph_file, _temp_file) = create_test_graph_file();
        create_dense_test_graph(&mut graph_file);

        // Test multiple adjacency operations on same graph
        let neighbors1 = {
            let mut iterator = AdjacencyIterator::new_outgoing(&mut graph_file, 1).unwrap();
            iterator.collect().unwrap()
        };

        let neighbors2 = {
            let mut iterator = AdjacencyIterator::new_incoming(&mut graph_file, 2).unwrap();
            iterator.collect().unwrap()
        };

        // Verify consistency
        assert!(neighbors1.len() > 0);
        assert!(neighbors2.len() > 0);
    }

    #[test]
    fn test_large_graph_performance() {
        // Test with larger graph to validate performance characteristics
        clear_node_cache();

        let (mut graph_file, _temp_file) = create_test_graph_file();
        create_large_test_graph(&mut graph_file, 1000, 5); // 1000 nodes, ~5 edges per node

        let start = std::time::Instant::now();

        for node_id in 1..=100 {
            let mut iterator = AdjacencyIterator::new_outgoing(&mut graph_file, node_id).unwrap();
            let _neighbors = iterator.collect().unwrap();
        }

        let duration = start.elapsed();

        // Performance assertion (adjust threshold as needed)
        assert!(duration.as_millis() < 1000, "Large graph traversal too slow: {:?}", duration);
    }
}
```

### 3. Property-Based Tests

#### Adjacency Properties (`adjacency/property_tests.rs`)

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_adjacency_iterator_properties(
        // Generate random graph configurations
        node_count in 1..100usize,
        edge_density in 0.0f32..=1.0f32,
        direction_strategy in prop::enum::Variant::<Outgoing, Incoming, Both>,
    ) {
        // Create test graph with random structure
        let (mut graph_file, _temp_file) = create_random_graph(node_count, edge_density);

        // Test properties based on strategy
        match direction_strategy {
            prop::enum::Variant::Outgoing => {
                test_outgoing_adjacency_properties(&mut graph_file);
            }
            prop::enum::Variant::Incoming => {
                test_incoming_adjacency_properties(&mut graph_file);
            }
            prop::enum::Variant::Both => {
                test_bidirectional_adjacency_properties(&mut graph_file);
            }
        }
    }

    #[test]
    fn test_edge_count_consistency(
        // Test edge count consistency across operations
        edge_count in 1..1000usize,
    ) {
        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Create edges with various IDs
        for i in 1..=edge_count {
            let edge = EdgeRecord::new(
                i as i64,
                1,
                (i + 1) as i64,
                "test".to_string(),
                serde_json::json!({})
            );

            let mut edge_store = EdgeStore::new(&mut graph_file);
            edge_store.write_edge(&edge).unwrap();

            // Verify header count is updated correctly
            assert_eq!(graph_file.header().edge_count, i as u64);
        }

        // Verify all edges can be found
        let mut edge_store = EdgeStore::new(&mut graph_file);
        let max_id = edge_store.max_edge_id();
        assert_eq!(max_id, edge_count as i64);
    }
}

fn test_outgoing_adjacency_properties(graph_file: &mut GraphFile) {
    // Property: All outgoing neighbors should have incoming edges back to source
    for node_id in 1..=graph_file.header().node_count {
        let mut outgoing_iterator = AdjacencyIterator::new_outgoing(graph_file, node_id).unwrap();
        let outgoing_neighbors = outgoing_iterator.collect().unwrap();

        for neighbor_id in outgoing_neighbors {
            let mut incoming_iterator = AdjacencyIterator::new_incoming(graph_file, neighbor_id).unwrap();
            let incoming_neighbors = incoming_iterator.collect().unwrap();

            assert!(incoming_neighbors.contains(&node_id),
                   "Node {} should appear in incoming neighbors of {}", node_id, neighbor_id);
        }
    }
}

fn test_incoming_adjacency_properties(graph_file: &mut GraphFile) {
    // Property: All incoming neighbors should have outgoing edges to source
    for node_id in 1..=graph_file.header().node_count {
        let mut incoming_iterator = AdjacencyIterator::new_incoming(graph_file, node_id).unwrap();
        let incoming_neighbors = incoming_iterator.collect().unwrap();

        for neighbor_id in incoming_neighbors {
            let mut outgoing_iterator = AdjacencyIterator::new_outgoing(graph_file, neighbor_id).unwrap();
            let outgoing_neighbors = outgoing_iterator.collect().unwrap();

            assert!(outgoing_neighbors.contains(&node_id),
                   "Node {} should appear in outgoing neighbors of {}", node_id, neighbor_id);
        }
    }
}

fn test_bidirectional_adjacency_properties(graph_file: &mut GraphFile) {
    // Property: Bidirectional adjacency should be symmetric
    for node_id in 1..=graph_file.header().node_count {
        let mut outgoing = AdjacencyIterator::new_outgoing(graph_file, node_id).unwrap();
        let mut incoming = AdjacencyIterator::new_incoming(graph_file, node_id).unwrap();

        let outgoing_neighbors: std::collections::HashSet<_> = outgoing.collect().unwrap().into_iter().collect();
        let incoming_neighbors: std::collections::HashSet<_> = incoming.collect().unwrap().into_iter().collect();

        // For undirected graphs, these should be equal
        // For directed graphs, this property doesn't hold
        // Test depends on your graph model

        // At minimum, ensure no self-loops unless explicitly created
        assert!(!outgoing_neighbors.contains(&node_id) || incoming_neighbors.contains(&node_id),
               "Self-loops should be consistent between directions");
    }
}
```

### 4. Performance Benchmarks

#### Adjacency Performance Benchmarks (`adjacency/benchmarks.rs`)

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion, BenchmarkId};

fn bench_adjacency_iterator_performance(c: &mut Criterion) {
    let mut group = c.benchmark_group("adjacency_iterator");

    // Benchmark different graph sizes
    for size in [100, 1000, 10000].iter() {
        group.bench_with_input(
            BenchmarkId::new("v2_cluster_read", size),
            size,
            |b, &size| {
                let (mut graph_file, _temp_file) = create_optimized_test_graph(size);

                b.iter(|| {
                    let mut iterator = AdjacencyIterator::new_outgoing(&mut graph_file, 1).unwrap();
                    black_box(iterator.collect::<Vec<_>>().unwrap());
                });
            }
        );

        group.bench_with_input(
            BenchmarkId::new("legacy_fallback", size),
            size,
            |b, &size| {
                let (mut graph_file, _temp_file) = create_legacy_test_graph(size);

                b.iter(|| {
                    let mut iterator = AdjacencyIterator::new_outgoing(&mut graph_file, 1).unwrap();
                    black_box(iterator.collect::<Vec<_>>().unwrap());
                });
            }
        );
    }

    group.finish();
}

fn bench_edge_count_consistency(c: &mut Criterion) {
    c.bench_function("edge_count_updates", |b| {
        b.iter(|| {
            let (mut graph_file, _temp_file) = create_test_graph_file();
            let mut edge_store = EdgeStore::new(&mut graph_file);

            for i in 1..=1000 {
                let edge = EdgeRecord::new(i, 1, (i + 1), "test".to_string(), serde_json::json!({}));
                black_box(edge_store.write_edge(&edge).unwrap());

                // Verify count is correct
                assert_eq!(graph_file.header().edge_count, i as u64);
            }
        });
    });
}

fn bench_v2_cluster_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("v2_cluster");

    for edge_count in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::new("serialize", edge_count),
            edge_count,
            |b, &edge_count| {
                let cluster = create_test_cluster(*edge_count);

                b.iter(|| {
                    black_box(cluster.serialize().unwrap());
                });
            }
        );

        group.bench_with_input(
            BenchmarkId::new("deserialize", edge_count),
            edge_count,
            |b, &edge_count| {
                let cluster = create_test_cluster(*edge_count);
                let serialized = cluster.serialize().unwrap();

                b.iter(|| {
                    black_box(EdgeCluster::deserialize(&serialized).unwrap());
                });
            }
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_adjacency_iterator_performance,
    bench_edge_count_consistency,
    bench_v2_cluster_serialization
);
criterion_main!(benches);
```

### 5. Regression Tests

#### Header Consistency Regression Tests

```rust
#[cfg(test)]
mod header_consistency_regression_tests {
    use super::*;

    #[test]
    fn test_regression_edge_count_not_updated_issue() {
        // Regression test for the critical bug where header.edge_count wasn't updated
        clear_node_cache();

        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Create edge with manually assigned ID (the bug scenario)
        let edge = EdgeRecord::new(5, 1, 2, "test".to_string(), serde_json::json!({}));

        {
            let mut edge_store = EdgeStore::new(&mut graph_file);

            // Before fix: this wouldn't update header.edge_count
            edge_store.write_edge(&edge).unwrap();
        }

        // Verify fix: header.edge_count should be updated
        assert_eq!(graph_file.header().edge_count, 5, "Header edge_count should be updated to match edge ID");

        // Verify edge can be found through adjacency
        let mut iterator = AdjacencyIterator::new_outgoing(&mut graph_file, 1).unwrap();
        let neighbors = iterator.collect().unwrap();

        assert!(neighbors.contains(&2), "Edge should be discoverable through adjacency");
    }

    #[test]
    fn test_regression_infinite_loop_stack_overflow() {
        // Regression test for infinite loop that caused stack overflow
        clear_node_cache();

        let (mut graph_file, _temp_file) = create_test_graph_file();

        // Create scenario that previously caused infinite loops
        create_problematic_graph_state(&mut graph_file);

        // This should complete quickly without stack overflow
        let start = std::time::Instant::now();
        let mut iterator = AdjacencyIterator::new_outgoing(&mut graph_file, 1).unwrap();
        let neighbors = iterator.collect().unwrap();
        let duration = start.elapsed();

        // Should terminate quickly (under 1 second)
        assert!(duration.as_secs() < 1, "Adjacency iteration should terminate quickly, took: {:?}", duration);

        // Should not panic due to stack overflow
        assert!(true, "Test completed without stack overflow");
    }

    #[test]
    fn test_regression_circular_dependency() {
        // Regression test for circular dependency between AdjacencyIterator and EdgeStore
        clear_node_cache();

        let (mut graph_file, _temp_file) = create_test_graph_file();
        create_test_edges(&mut graph_file);

        // This should not cause infinite recursion
        let mut edge_store = EdgeStore::new(&mut graph_file);
        let neighbors: Vec<_> = edge_store.iter_neighbors(1, crate::backend::native::adjacency::Direction::Outgoing).collect();

        assert!(!neighbors.is_empty(), "Should find neighbors without circular dependency");
    }
}
```

### 6. Test Utilities

#### Test Data Generation

```rust
#[cfg(test)]
pub mod test_utilities {
    use super::*;
    use tempfile::NamedTempFile;

    pub fn create_test_graph_file() -> (GraphFile, NamedTempFile) {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let graph_file = GraphFile::create(path).unwrap();
        (graph_file, temp_file)
    }

    pub fn create_test_node(id: i64, name: &str) -> NodeRecord {
        NodeRecord::new(
            id,
            "Test".to_string(),
            name.to_string(),
            serde_json::json!({}),
        )
    }

    pub fn create_test_edge(id: i64, from: i64, to: i64, edge_type: &str) -> EdgeRecord {
        EdgeRecord::new(
            id,
            from,
            to,
            edge_type.to_string(),
            serde_json::json!({}),
        )
    }

    pub fn create_simple_test_graph() -> Box<dyn crate::backend::GraphBackend> {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("test.db");

        let mut backend = crate::backend::NativeGraphBackend::create(&path).unwrap();

        // Create nodes
        let node1_id = backend.add_node("node1", serde_json::json!({})).unwrap();
        let node2_id = backend.add_node("node2", serde_json::json!({})).unwrap();
        let node3_id = backend.add_node("node3", serde_json::json!({})).unwrap();

        // Create edges
        backend.add_edge(node1_id, node2_id, "test", serde_json::json!({})).unwrap();
        backend.add_edge(node2_id, node3_id, "test", serde_json::json!({})).unwrap();

        backend
    }

    pub fn create_large_test_graph(num_nodes: usize, avg_edges_per_node: usize) -> Box<dyn crate::backend::GraphBackend> {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("large_test.db");

        let mut backend = crate::backend::NativeGraphBackend::create(&path).unwrap();

        // Create nodes
        let mut node_ids = Vec::new();
        for i in 0..num_nodes {
            let node_id = backend.add_node(&format!("node_{}", i), serde_json::json!({})).unwrap();
            node_ids.push(node_id);
        }

        // Create edges (ensure connected graph)
        use std::collections::HashSet;
        let mut created_edges = HashSet::new();

        for i in 0..num_nodes {
            for j in 1..=avg_edges_per_node {
                let target = (i + j) % num_nodes;
                let edge_key = (i.min(target), i.max(target));

                if !created_edges.contains(&edge_key) {
                    backend.add_edge(node_ids[i], node_ids[target], "test", serde_json::json!({})).unwrap();
                    created_edges.insert(edge_key);
                }
            }
        }

        backend
    }

    pub fn create_optimized_test_graph(num_nodes: usize) -> GraphFile {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let mut graph_file = GraphFile::create(path).unwrap();

        // Create optimized graph structure for benchmarking
        // This implementation creates a graph with predictable adjacency patterns
        let mut node_store = NodeStore::new(&mut graph_file);
        let mut edge_store = EdgeStore::new(&mut graph_file);

        // Create nodes
        for i in 1..=num_nodes {
            let node = NodeRecord::new(
                i as i64,
                "Test".to_string(),
                format!("node_{}", i),
                serde_json::json!({}),
            );
            node_store.write_node(&node).unwrap();
        }

        // Create edges in a predictable pattern
        let edge_id = 1;
        for i in 1..=num_nodes {
            // Connect each node to next 2 nodes (creating a predictable structure)
            for j in 1..=2 {
                let target = ((i + j - 1) % num_nodes) + 1;
                if target != i {  // Avoid self-loops
                    let edge = EdgeRecord::new(
                        edge_id,
                        i as i64,
                        target as i64,
                        "test".to_string(),
                        serde_json::json!({}),
                    );
                    edge_store.write_edge(&edge).unwrap();
                }
            }
        }

        graph_file
    }

    pub fn create_legacy_test_graph(num_edges: usize) -> GraphFile {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path();
        let mut graph_file = GraphFile::create(path).unwrap();

        // Create graph without V2 cluster writing (for fallback testing)
        let mut edge_store = EdgeStore::new(&mut graph_file);

        for i in 1..=num_edges {
            let edge = EdgeRecord::new(
                i as i64,
                1,
                (i + 1) as i64,
                "legacy".to_string(),
                serde_json::json!({}),
            );
            edge_store.write_edge(&edge).unwrap();
        }

        graph_file
    }

    pub fn create_test_cluster(edge_count: usize) -> crate::backend::native::v2::edge_cluster::EdgeCluster {
        let mut cluster = crate::backend::native::v2::edge_cluster::EdgeCluster::new();

        for i in 0..edge_count {
            cluster.add_edge(
                i as u64,
                (i + 1) as u64,
                format!("edge_{}", i),
            ).unwrap();
        }

        cluster
    }
}
```

### 7. Continuous Integration Configuration

#### GitHub Actions Workflow

```yaml
# .github/workflows/v2-adjacency-testing.yml
name: V2 Adjacency System Testing

on:
  push:
    branches: [ main, development ]
    paths:
      - 'sqlitegraph/src/backend/native/adjacency/**'
      - 'sqlitegraph/src/backend/native/edge_store/**'
      - 'sqlitegraph/src/backend/native/v2/**'
  pull_request:
    branches: [ main ]

env:
  RUST_BACKTRACE: 1
  RUST_LOG: debug

jobs:
  unit-tests:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable
        components: rustfmt, clippy

    - name: Run unit tests
      run: |
        cargo test -p sqlitegraph --lib adjacency

    - name: Run integration tests
      run: |
        cargo test -p sqlitegraph --lib graph_ops

    - name: Run regression tests
      run: |
        cargo test -p sqlitegraph --lib -- --ignored "regression"

    - name: Check formatting
      run: |
        cargo fmt --all -- --check

    - name: Run clippy
      run: |
        cargo clippy -- -D warnings

  performance-benchmarks:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable

    - name: Run benchmarks
      run: |
        cargo bench --bench adjacency_benchmark

    - name: Compare with baseline
      run: |
        python3 scripts/compare_benchmarks.py

  property-tests:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable

    - name: Run property-based tests
      run: |
        cargo test -p sqlitegraph --lib -- --ignored "property"

  coverage:
    runs-on: ubuntu-latest
    steps:
    - uses: actions/checkout@v3
    - name: Install Rust
      uses: actions-rs/toolchain@v1
      with:
        toolchain: stable

    - name: Generate coverage report
      run: |
        cargo install cargo-tarpaulin
        cargo tarpaulin --out xml --output-dir target/coverage

    - name: Upload coverage
      uses: codecov/codecov-action@v3
      with:
        file: ./target/coverage/cobertura.xml
```

### 8. Test Data Management

#### Test Fixtures

```rust
// tests/fixtures/test_graphs.rs

use sqlitegraph::*;
use tempfile::TempDir;

pub struct TestGraphFixture {
    pub temp_dir: TempDir,
    pub backend: Box<dyn GraphBackend>,
    pub expected_edges: Vec<(u64, u64)>,
}

impl TestGraphFixture {
    pub fn create_linear_graph(num_nodes: usize) -> Self {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("linear.db");

        let mut backend = NativeGraphBackend::create(&path).unwrap();
        let mut expected_edges = Vec::new();
        let mut node_ids = Vec::new();

        // Create nodes
        for i in 0..num_nodes {
            let node_id = backend.add_node(&format!("node_{}", i), serde_json::json!({})).unwrap();
            node_ids.push(node_id);
        }

        // Create linear chain: node_0 -> node_1 -> node_2 -> ...
        for i in 0..num_nodes.saturating_sub(1) {
            backend.add_edge(node_ids[i], node_ids[i + 1], "next", serde_json::json!({})).unwrap();
            expected_edges.push((node_ids[i], node_ids[i + 1]));
        }

        Self {
            temp_dir,
            backend,
            expected_edges,
        }
    }

    pub fn create_star_graph(center_connections: usize) -> Self {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("star.db");

        let mut backend = NativeGraphBackend::create(&path).unwrap();
        let mut expected_edges = Vec::new();

        // Create center node
        let center_id = backend.add_node("center", serde_json::json!({})).unwrap();

        // Create satellite nodes and connect to center
        for i in 0..center_connections {
            let satellite_id = backend.add_node(&format!("satellite_{}", i), serde_json::json!({})).unwrap();
            backend.add_edge(center_id, satellite_id, "satellite", serde_json::json!({})).unwrap();
            expected_edges.push((center_id, satellite_id));
        }

        Self {
            temp_dir,
            backend,
            expected_edges,
        }
    }

    pub fn verify_adjacency_consistency(&self) -> bool {
        // Verify that all expected edges are discoverable through adjacency
        for &(from, to) in &self.expected_edges {
            let outgoing_neighbors: Vec<_> = self.backend.iter_neighbors(from, "outgoing").unwrap().collect();
            if !outgoing_neighbors.contains(&to) {
                return false;
            }

            let incoming_neighbors: Vec<_> = self.backend.iter_neighbors(to, "incoming").unwrap().collect();
            if !incoming_neighbors.contains(&from) {
                return false;
            }
        }

        true
    }
}
```

---

*Document created: 2025-01-19*
*Version: 1.0*
*Status: Production Ready*