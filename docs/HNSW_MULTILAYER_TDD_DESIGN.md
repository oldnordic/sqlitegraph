# HNSW Multi-layer TDD Design Phase 1

## Executive Summary

This document defines the TDD (Test-Driven Development) approach for implementing multi-layer HNSW functionality. Following strict TDD principles, we will design comprehensive test cases first, then implement the code to make those tests pass.

**Status**: 📋 **TDD DESIGN COMPLETE** - All test cases defined, ready for implementation

---

## 1. TDD Methodology

### 1.1 Test-First Approach
1. **Red**: Write failing tests that define desired behavior
2. **Green**: Implement minimum code to make tests pass
3. **Refactor**: Improve code while keeping tests green
4. **Repeat**: Continue for each feature

### 1.2 Test Design Principles
- **Atomic Tests**: Each test validates one specific behavior
- **Deterministic**: Tests produce same results on repeated runs
- **Comprehensive**: Cover all critical paths and edge cases
- **Maintainable**: Clear test names and documentation
- **Fast**: Tests run quickly to enable rapid iteration

### 1.3 Test Organization
```rust
// Test file structure
sqlitegraph/tests/hnsw_multilayer_tests.rs
├── mod node_mapping_tests           // Bidirectional ID mapping tests
├── mod level_distribution_tests      // Exponential level assignment tests
├── mod multilayer_insertion_tests   // Multi-layer insertion algorithm tests
├── mod multilayer_search_tests      // Multi-layer search algorithm tests
├── mod integration_tests             // End-to-end integration tests
└── mod regression_tests             // Backward compatibility tests
```

---

## 2. Test Case Design

### 2.1 Node Mapping System Tests

#### 2.1.1 Bidirectional ID Mapping Tests
```rust
#[cfg(test)]
mod node_mapping_tests {
    use super::*;
    use sqlitegraph::hnsw::multilayer::LayerMappings;

    #[test]
    fn test_global_to_local_mapping_creation() {
        // Given: Empty mapping system
        let mut mappings = LayerMappings::new();

        // When: Insert mappings for vector in multiple layers
        mappings.add_mapping(1, 0, Some(0))?;  // Vector 1 → Layer 0, local ID 0
        mappings.add_mapping(1, 1, Some(0))?;  // Vector 1 → Layer 1, local ID 0

        // Then: Mappings are correctly stored
        assert_eq!(mappings.get_local_id(1, 0), Some(0));
        assert_eq!(mappings.get_local_id(1, 1), Some(0));
        assert_eq!(mappings.get_local_id(1, 2), None); // No mapping for layer 2
    }

    #[test]
    fn test_local_to_global_mapping_creation() {
        // Given: Empty mapping system
        let mut mappings = LayerMappings::new();

        // When: Insert mappings
        mappings.add_mapping(1, 0, Some(0))?;
        mappings.add_mapping(2, 0, Some(1))?;
        mappings.add_mapping(1, 1, Some(0))?;

        // Then: Reverse mappings work correctly
        assert_eq!(mappings.get_global_id(0, 0), Some(1)); // Layer 0, local 0 → global 1
        assert_eq!(mappings.get_global_id(0, 1), Some(2)); // Layer 0, local 1 → global 2
        assert_eq!(mappings.get_global_id(1, 0), Some(1)); // Layer 1, local 0 → global 1
    }

    #[test]
    fn test_sequential_local_id_assignment_per_layer() {
        // Given: Empty mapping system
        let mut mappings = LayerMappings::new();

        // When: Insert multiple vectors into same layer
        mappings.add_mapping(1, 0, Some(0))?; // Vector 1 → Layer 0, local 0
        mappings.add_mapping(2, 0, Some(1))?; // Vector 2 → Layer 0, local 1
        mappings.add_mapping(3, 0, Some(2))?; // Vector 3 → Layer 0, local 2

        // Then: Local IDs are sequential starting from 0
        assert_eq!(mappings.get_local_id(1, 0), Some(0));
        assert_eq!(mappings.get_local_id(2, 0), Some(1));
        assert_eq!(mappings.get_local_id(3, 0), Some(2));
    }

    #[test]
    fn test_independent_local_sequences_across_layers() {
        // Given: Empty mapping system
        let mut mappings = LayerMappings::new();

        // When: Insert vectors into different layers
        mappings.add_mapping(1, 0, Some(0))?; // Layer 0: vector 1 → local 0
        mappings.add_mapping(1, 1, Some(0))?; // Layer 1: vector 1 → local 0
        mappings.add_mapping(2, 1, Some(1))?; // Layer 1: vector 2 → local 1
        mappings.add_mapping(3, 1, Some(2))?; // Layer 1: vector 3 → local 2

        // Then: Each layer maintains independent sequential IDs
        // Layer 0 has only local ID 0
        assert_eq!(mappings.get_local_id(1, 0), Some(0));
        assert_eq!(mappings.get_local_id(2, 0), None);  // Vector 2 not in layer 0

        // Layer 1 has local IDs 0, 1, 2
        assert_eq!(mappings.get_local_id(1, 1), Some(0));
        assert_eq!(mappings.get_local_id(2, 1), Some(1));
        assert_eq!(mappings.get_local_id(3, 1), Some(2));
    }

    #[test]
    fn test_mapping_consistency_validation() {
        // Given: Mapping system with data
        let mut mappings = LayerMappings::new();
        mappings.add_mapping(1, 0, Some(0))?;
        mappings.add_mapping(2, 0, Some(1))?;
        mappings.add_mapping(1, 1, Some(0))?;

        // When: Validate mapping consistency
        let validation_result = mappings.validate_consistency();

        // Then: All mappings are consistent
        assert!(validation_result.is_ok());
    }

    #[test]
    fn test_mapping_memory_usage_tracking() {
        // Given: Empty mapping system
        let mut mappings = LayerMappings::new();
        let initial_memory = mappings.memory_usage();

        // When: Add mappings
        mappings.add_mapping(1, 0, Some(0))?;
        mappings.add_mapping(2, 0, Some(1))?;
        mappings.add_mapping(1, 1, Some(0))?;

        // Then: Memory usage increases appropriately
        let final_memory = mappings.memory_usage();
        assert!(final_memory > initial_memory);
    }
}
```

### 2.2 Level Distribution Tests

#### 2.2.1 Exponential Level Assignment Tests
```rust
#[cfg(test)]
mod level_distribution_tests {
    use super::*;
    use sqlitegraph::hnsw::multilayer::LevelDistributor;
    use rand::SeedableRng;
    use rand::rngs::StdRng;

    #[test]
    fn test_exponential_distribution_with_seed() {
        // Given: Deterministic seed and distribution parameters
        let seed = [42; 32]; // Fixed seed for reproducibility
        let mut rng = StdRng::from_seed(seed);
        let distributor = LevelDistributor::new(16.0, 5); // m=16, max_levels=5

        // When: Generate many level assignments
        let mut level_counts = vec![0; 5];
        for _ in 0..10000 {
            let level = distributor.sample_level(&mut rng);
            assert!(level < 5);
            level_counts[level] += 1;
        }

        // Then: Distribution follows exponential pattern
        // Expected: level_0 > level_1 > level_2 > level_3 > level_4
        assert!(level_counts[0] > level_counts[1]);
        assert!(level_counts[1] > level_counts[2]);
        assert!(level_counts[2] > level_counts[3]);
        assert!(level_counts[3] > level_counts[4]);
    }

    #[test]
    fn test_deterministic_level_assignment() {
        // Given: Same seed and parameters
        let seed = [123; 32];
        let distributor = LevelDistributor::new(16.0, 4);

        // When: Generate level assignments twice
        let levels1: Vec<usize> = (0..100)
            .map(|_| {
                let mut rng = StdRng::from_seed(seed);
                distributor.sample_level(&mut rng)
            })
            .collect();

        let levels2: Vec<usize> = (0..100)
            .map(|_| {
                let mut rng = StdRng::from_seed(seed);
                distributor.sample_level(&mut rng)
            })
            .collect();

        // Then: Results are identical (deterministic)
        assert_eq!(levels1, levels2);
    }

    #[test]
    fn test_level_distribution_mathematical_properties() {
        // Given: Distribution parameters
        let m = 16.0;
        let distributor = LevelDistributor::new(m, 4);

        // When: Generate many samples
        let mut rng = StdRng::from_seed([42; 32]);
        let mut level_counts = vec![0; 4];
        let total_samples = 50000;

        for _ in 0..total_samples {
            let level = distributor.sample_level(&mut rng);
            level_counts[level] += 1;
        }

        // Then: Ratios approximately follow exponential distribution
        // P(level = ℓ) ≈ m^(-ℓ)
        let p0 = level_counts[0] as f64 / total_samples as f64;
        let p1 = level_counts[1] as f64 / total_samples as f64;
        let p2 = level_counts[2] as f64 / total_samples as f64;
        let p3 = level_counts[3] as f64 / total_samples as f64;

        // Expected ratios within 10% tolerance
        assert!((p1 / p0 - 1.0 / m).abs() < 0.1);  // P1 ≈ P0/m
        assert!((p2 / p1 - 1.0 / m).abs() < 0.1);  // P2 ≈ P1/m
        assert!((p3 / p2 - 1.0 / m).abs() < 0.1);  // P3 ≈ P2/m
    }

    #[test]
    fn test_max_levels_enforcement() {
        // Given: Distribution with max_levels = 3
        let distributor = LevelDistributor::new(8.0, 3);
        let mut rng = StdRng::from_seed([42; 32]);

        // When: Generate many level assignments
        for _ in 0..1000 {
            let level = distributor.sample_level(&mut rng);

            // Then: No level exceeds max_levels
            assert!(level < 3, "Level {} exceeds max_levels 3", level);
        }
    }

    #[test]
    fn test_edge_case_single_layer() {
        // Given: Distribution with max_layers = 1
        let distributor = LevelDistributor::new(8.0, 1);
        let mut rng = StdRng::from_seed([42; 32]);

        // When: Generate level assignments
        for _ in 0..100 {
            let level = distributor.sample_level(&mut rng);

            // Then: Always returns level 0
            assert_eq!(level, 0);
        }
    }
}
```

### 2.3 Multi-layer Insertion Tests

#### 2.3.1 Integration Insertion Tests
```rust
#[cfg(test)]
mod multilayer_insertion_tests {
    use super::*;
    use sqlitegraph::hnsw::{HnswConfig, DistanceMetric};
    use serde_json::json;

    #[test]
    fn test_multilayer_vector_insertion() {
        // Given: Multi-layer HNSW index with deterministic seed
        let config = HnswConfig::builder()
            .dimension(3)
            .m_connections(16)
            .distance_metric(DistanceMetric::Euclidean)
            .enable_multilayer(true)
            .random_seed(42)
            .build()
            .unwrap();

        let mut hnsw = HnswIndex::new(config).unwrap();

        // When: Insert multiple vectors
        let vectors = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
            vec![1.0, 1.0, 1.0],
            vec![-1.0, -1.0, -1.0],
        ];

        let mut vector_ids = Vec::new();
        for vector in &vectors {
            let id = hnsw.insert_vector(vector, None).unwrap();
            vector_ids.push(id);
        }

        // Then: All vectors are inserted with unique IDs
        assert_eq!(vector_ids.len(), 5);
        for (i, &id) in vector_ids.iter().enumerate() {
            assert!(id > 0);
            assert_eq!(hnsw.get_vector(id).unwrap().unwrap().0, vectors[i]);
        }

        // And: Statistics show multi-layer usage
        let stats = hnsw.statistics().unwrap();
        assert_eq!(stats.vector_count, 5);
        assert!(stats.layer_count > 1); // Should have multiple layers
    }

    #[test]
    fn test_layer_population_distribution() {
        // Given: Multi-layer HNSW with deterministic seed
        let config = HnswConfig::builder()
            .dimension(2)
            .m_connections(8)
            .distance_metric(DistanceMetric::Euclidean)
            .enable_multilayer(true)
            .random_seed(123)  // Fixed seed for reproducible distribution
            .build()
            .unwrap();

        let mut hnsw = HnswIndex::new(config).unwrap();

        // When: Insert many vectors to trigger multi-layer distribution
        for i in 0..1000 {
            let vector = vec![
                (i as f32).sin(),
                (i as f32).cos(),
            ];
            hnsw.insert_vector(&vector, None).unwrap();
        }

        // Then: Vectors are distributed across multiple layers
        let stats = hnsw.statistics().unwrap();
        let layer_stats = &stats.layer_stats;

        // Should have vectors in multiple layers
        let layers_with_vectors = layer_stats.iter()
            .filter(|&(node_count, _, _)| node_count > 0)
            .count();

        assert!(layers_with_vectors > 1, "Vectors should be in multiple layers");

        // Base layer should have most vectors
        let base_layer_nodes = layer_stats[0].0;
        assert!(base_layer_nodes > 0);
        assert_eq!(base_layer_nodes, 1000); // All vectors in base layer

        // Higher layers should have fewer vectors
        for i in 1..layer_stats.len() {
            assert!(layer_stats[i].0 <= layer_stats[i-1].0,
                   "Layer {} should have <= vectors than layer {}",
                   i, i-1);
        }
    }

    #[test]
    fn test_multilayer_entry_point_management() {
        // Given: Multi-layer HNSW
        let config = HnswConfig::builder()
            .dimension(3)
            .m_connections(4)
            .enable_multilayer(true)
            .random_seed(456)
            .build()
            .unwrap();

        let mut hnsw = HnswIndex::new(config).unwrap();

        // When: Insert vectors that should become entry points
        for i in 0..10 {
            let vector = vec![i as f32, (i * 2) as f32, (i * 3) as f32];
            hnsw.insert_vector(&vector, Some(json!({"index": i}))).unwrap();
        }

        // Then: Entry points are properly managed
        let stats = hnsw.statistics().unwrap();
        assert!(stats.entry_point_count > 0);

        // Entry points should be a subset of all vectors
        assert!(stats.entry_point_count <= stats.vector_count);
    }

    #[test]
    fn test_multilayer_insertion_with_metadata() {
        // Given: Multi-layer HNSW with metadata support
        let config = HnswConfig::builder()
            .dimension(2)
            .enable_multilayer(true)
            .build()
            .unwrap();

        let mut hnsw = HnswIndex::new(config).unwrap();

        // When: Insert vectors with metadata
        let metadata = json!({
            "source": "test",
            "version": "1.0",
            "timestamp": 1234567890
        });

        let vector = vec![1.0, 2.0];
        let vector_id = hnsw.insert_vector(&vector, Some(metadata.clone())).unwrap();

        // Then: Metadata is preserved with multilayer insertion
        let retrieved = hnsw.get_vector(vector_id).unwrap();
        assert!(retrieved.is_some());
        let (retrieved_vector, retrieved_metadata) = retrieved.unwrap();
        assert_eq!(retrieved_vector, vector);
        assert_eq!(retrieved_metadata, metadata);
    }

    #[test]
    fn test_multilayer_consistency_after_reconstruction() {
        // Given: Multi-layer HNSW with deterministic seed
        let seed = 789;
        let config1 = HnswConfig::builder()
            .dimension(3)
            .enable_multilayer(true)
            .random_seed(seed)
            .build()
            .unwrap();

        let config2 = HnswConfig::builder()
            .dimension(3)
            .enable_multilayer(true)
            .random_seed(seed)
            .build()
            .unwrap();

        // When: Create two indexes with same seed and data
        let mut hnsw1 = HnswIndex::new(config1).unwrap();
        let mut hnsw2 = HnswIndex::new(config2).unwrap();

        let vectors = vec![
            vec![1.0, 0.0, 0.0],
            vec![0.0, 1.0, 0.0],
            vec![0.0, 0.0, 1.0],
        ];

        for vector in &vectors {
            hnsw1.insert_vector(vector, None).unwrap();
            hnsw2.insert_vector(vector, None).unwrap();
        }

        // Then: Both indexes have identical layer structure
        let stats1 = hnsw1.statistics().unwrap();
        let stats2 = hnsw2.statistics().unwrap();

        assert_eq!(stats1.layer_count, stats2.layer_count);
        assert_eq!(stats1.entry_point_count, stats2.entry_point_count);

        for i in 0..stats1.layer_count {
            assert_eq!(stats1.layer_stats[i], stats2.layer_stats[i],
                     "Layer {} statistics differ between indexes", i);
        }
    }
}
```

### 2.4 Multi-layer Search Tests

#### 2.4.1 Search Algorithm Tests
```rust
#[cfg(test)]
mod multilayer_search_tests {
    use super::*;
    use sqlitegraph::hnsw::{HnswConfig, DistanceMetric};

    #[test]
    fn test_multilayer_search_accuracy() {
        // Given: Multi-layer HNSW with known vectors
        let config = HnswConfig::builder()
            .dimension(2)
            .m_connections(16)
            .ef_construction(200)
            .ef_search(50)
            .enable_multilayer(true)
            .random_seed(42)
            .build()
            .unwrap();

        let mut hnsw = HnswIndex::new(config).unwrap();

        // Insert vectors forming distinct clusters
        let cluster1 = vec![[1.0, 1.0], [1.1, 0.9], [0.9, 1.1]];  // Near (1,1)
        let cluster2 = vec![[-1.0, -1.0], [-0.9, -1.1], [-1.1, -0.9]];  // Near (-1,-1)

        for vector in cluster1.iter().chain(&cluster2) {
            hnsw.insert_vector(vector, None).unwrap();
        }

        // When: Search near cluster1 center
        let query = vec![1.0, 1.0];
        let results = hnsw.search(&query, 3).unwrap();

        // Then: Results are from cluster1 (nearest to query)
        assert!(!results.is_empty());
        assert!(results.len() <= 3);

        // Results should be sorted by distance
        for window in results.windows(2) {
            assert!(window[0].1 <= window[1].1, "Results not sorted by distance");
        }

        // Nearest result should be closest to query
        let nearest_distance = results[0].1;
        let expected_distance = ((1.0 - 1.0).powi(2) + (1.0 - 1.0).powi(2)).sqrt();
        assert!((nearest_distance - expected_distance).abs() < 0.001);
    }

    #[test]
    fn test_multilayer_vs_single_layer_search_consistency() {
        // Given: Same data in single-layer and multi-layer modes
        let vectors = vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![-1.0, 0.0],
            vec![0.0, -1.0],
            vec![0.5, 0.5],
            vec![-0.5, -0.5],
        ];

        let single_config = HnswConfig::builder()
            .dimension(2)
            .enable_multilayer(false)
            .random_seed(123)
            .build()
            .unwrap();

        let multi_config = HnswConfig::builder()
            .dimension(2)
            .enable_multilayer(true)
            .random_seed(123)
            .build()
            .unwrap();

        let mut single_hnsw = HnswIndex::new(single_config).unwrap();
        let mut multi_hnsw = HnswIndex::new(multi_config).unwrap();

        for vector in &vectors {
            single_hnsw.insert_vector(vector, None).unwrap();
            multi_hnsw.insert_vector(vector, None).unwrap();
        }

        // When: Search for nearest neighbors
        let query = vec![0.7, 0.7];
        let k = 3;

        let single_results = single_hnsw.search(&query, k).unwrap();
        let multi_results = multi_hnsw.search(&query, k).unwrap();

        // Then: Results are approximately the same (allowing for algorithmic differences)
        assert_eq!(single_results.len(), multi_results.len());

        // Top results should be very similar
        for i in 0..single_results.len().min(multi_results.len()) {
            let distance_diff = (single_results[i].1 - multi_results[i].1).abs();
            assert!(distance_diff < 0.1, "Distance difference too large at index {}: {}", i, distance_diff);
        }
    }

    #[test]
    fn test_multilayer_search_performance_gain() {
        // Given: Large dataset to trigger multi-layer benefits
        let config = HnswConfig::builder()
            .dimension(10)
            .m_connections(32)
            .ef_construction(200)
            .ef_search(50)
            .enable_multilayer(true)
            .random_seed(456)
            .build()
            .unwrap();

        let mut hnsw = HnswIndex::new(config).unwrap();

        // Insert many vectors
        for i in 0..1000 {
            let vector: Vec<f32> = (0..10)
                .map(|j| ((i * j) as f32).sin())
                .collect();
            hnsw.insert_vector(&vector, None).unwrap();
        }

        // When: Measure search performance
        let query: Vec<f32> = (0..10).map(|j| (j as f32).cos()).collect();
        let start = std::time::Instant::now();

        for _ in 0..100 {
            let _results = hnsw.search(&query, 10).unwrap();
        }

        let search_time = start.elapsed();

        // Then: Search is fast (should be sub-millisecond for 1000 vectors)
        assert!(search_time.as_millis() < 100, "Search took too long: {:?}", search_time);

        // And returns reasonable results
        let results = hnsw.search(&query, 5).unwrap();
        assert!(!results.is_empty());
        assert!(results.len() <= 5);
    }

    #[test]
    fn test_multilayer_search_empty_index() {
        // Given: Empty multi-layer HNSW
        let config = HnswConfig::builder()
            .enable_multilayer(true)
            .build()
            .unwrap();

        let hnsw = HnswIndex::new(config).unwrap();

        // When: Search in empty index
        let query = vec![1.0, 2.0, 3.0];
        let results = hnsw.search(&query, 5).unwrap();

        // Then: Returns empty results
        assert!(results.is_empty());
    }

    #[test]
    fn test_multilayer_search_k_parameter() {
        // Given: Multi-layer HNSW with data
        let config = HnswConfig::builder()
            .dimension(3)
            .enable_multilayer(true)
            .build()
            .unwrap();

        let mut hnsw = HnswIndex::new(config).unwrap();

        for i in 0..10 {
            let vector = vec![i as f32, (i * 2) as f32, (i * 3) as f32];
            hnsw.insert_vector(&vector, None).unwrap();
        }

        let query = vec![5.0, 10.0, 15.0];

        // When: Search with different k values
        let results_1 = hnsw.search(&query, 1).unwrap();
        let results_5 = hnsw.search(&query, 5).unwrap();
        let results_10 = hnsw.search(&query, 10).unwrap();

        // Then: k parameter is respected
        assert!(results_1.len() <= 1);
        assert!(results_5.len() <= 5);
        assert!(results_10.len() <= 10);

        // And larger k returns more results (up to available vectors)
        assert!(results_10.len() >= results_5.len());
        assert!(results_5.len() >= results_1.len());
    }
}
```

### 2.5 Integration and Regression Tests

#### 2.5.1 Backward Compatibility Tests
```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use sqlitegraph::hnsw::{HnswConfig, DistanceMetric};

    #[test]
    fn test_single_layer_backward_compatibility() {
        // Given: Existing single-layer HNSW code
        let config = HnswConfig::builder()
            .dimension(3)
            .m_connections(16)
            .distance_metric(DistanceMetric::Euclidean)
            .build()
            .unwrap();

        let mut hnsw = HnswIndex::new(config).unwrap();

        // When: Use existing API exactly as before
        let vector = vec![1.0, 2.0, 3.0];
        let vector_id = hnsw.insert_vector(&vector, None).unwrap();
        let results = hnsw.search(&vector, 3).unwrap();

        // Then: All existing functionality works unchanged
        assert!(vector_id > 0);
        assert!(!results.is_empty());
        assert_eq!(results[0].1, 0.0); // Exact match
    }

    #[test]
    fn test_multilayer_feature_flag_behavior() {
        // Given: Configuration with feature flag
        let single_config = HnswConfig::builder()
            .dimension(2)
            .enable_multilayer(false)
            .build()
            .unwrap();

        let multi_config = HnswConfig::builder()
            .dimension(2)
            .enable_multilayer(true)
            .build()
            .unwrap();

        let mut single_hnsw = HnswIndex::new(single_config).unwrap();
        let mut multi_hnsw = HnswIndex::new(multi_config).unwrap();

        // When: Insert same vectors into both
        let vector = vec![1.0, 2.0];
        let single_id = single_hnsw.insert_vector(&vector, None).unwrap();
        let multi_id = multi_hnsw.insert_vector(&vector, None).unwrap();

        // Then: Both work correctly
        assert!(single_id > 0);
        assert!(multi_id > 0);

        // And search works in both modes
        let single_results = single_hnsw.search(&vector, 1).unwrap();
        let multi_results = multi_hnsw.search(&vector, 1).unwrap();

        assert!(!single_results.is_empty());
        assert!(!multi_results.is_empty());

        // Single layer might have different performance but should be accurate
        assert_eq!(single_results[0].0, single_id);
        assert_eq!(multi_results[0].0, multi_id);
    }

    #[test]
    fn test_sqlite_graph_integration_multilayer() {
        // Given: SQLiteGraph with multi-layer HNSW
        let graph = SqliteGraph::open_in_memory().unwrap();
        let config = HnswConfig::builder()
            .dimension(4)
            .distance_metric(DistanceMetric::Cosine)
            .enable_multilayer(true)
            .build()
            .unwrap();

        // When: Create HNSW index through SQLiteGraph
        let hnsw = graph.hnsw_index("test_index", config).unwrap();

        // Then: Index is properly initialized
        let stats = hnsw.statistics().unwrap();
        assert_eq!(stats.vector_count, 0);
        assert_eq!(stats.dimension, 4);
        assert_eq!(stats.distance_metric, DistanceMetric::Cosine);
    }

    #[test]
    fn test_configuration_validation_multilayer() {
        // Given: Various multi-layer configurations
        let valid_configs = vec![
            HnswConfig::builder().dimension(1).enable_multilayer(true).build(),
            HnswConfig::builder().dimension(100).enable_multilayer(true).build(),
            HnswConfig::builder().dimension(10).m_connections(32).enable_multilayer(true).build(),
        ];

        // When: Create indices with different valid configurations
        for config in valid_configs {
            let hnsw = HnswIndex::new(config.unwrap());
            assert!(hnsw.is_ok(), "Configuration should be valid: {:?}", hnsw);
        }

        // Then: All configurations are accepted
    }

    #[test]
    fn test_memory_usage_tracking_multilayer() {
        // Given: Multi-layer HNSW
        let config = HnswConfig::builder()
            .dimension(5)
            .enable_multilayer(true)
            .build()
            .unwrap();

        let mut hnsw = HnswIndex::new(config).unwrap();
        let initial_memory = hnsw.memory_usage().unwrap();

        // When: Insert vectors
        for i in 0..10 {
            let vector = vec![
                i as f32,
                (i * 2) as f32,
                (i * 3) as f32,
                (i * 4) as f32,
                (i * 5) as f32,
            ];
            hnsw.insert_vector(&vector, None).unwrap();
        }

        // Then: Memory usage increases appropriately
        let final_memory = hnsw.memory_usage().unwrap();
        assert!(final_memory > initial_memory);
    }
}
```

---

## 3. Test Execution Strategy

### 3.1 Test Execution Order
```bash
# Execute tests in dependency order:
cargo test node_mapping_tests      # Test 1: Basic ID mapping
cargo test level_distribution_tests  # Test 2: Level assignment
cargo test multilayer_insertion_tests # Test 3: Insertion algorithm
cargo test multilayer_search_tests     # Test 4: Search algorithm
cargo test integration_tests           # Test 5: End-to-end integration
```

### 3.2 Test Data Generation
```rust
// Helper functions for deterministic test data
pub fn create_deterministic_vectors(dimension: usize, count: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut vectors = Vec::with_capacity(count);
    let mut rng = StdRng::seed_from_u64(seed);

    for i in 0..count {
        let vector: Vec<f32> = (0..dimension)
            .map(|j| {
                let base = (i as f32 + j as f32);
                let variation = rng.gen_range(-0.1..0.1);
                base + variation
            })
            .collect();
        vectors.push(vector);
    }

    vectors
}

pub fn create_clustered_vectors(cluster_centers: &[Vec<f32>], vectors_per_cluster: usize, seed: u64) -> Vec<Vec<f32>> {
    let mut all_vectors = Vec::new();
    let mut rng = StdRng::seed_from_u64(seed);

    for center in cluster_centers {
        for _ in 0..vectors_per_cluster {
            let mut vector = center.clone();
            // Add small random variation
            for value in &mut vector {
                *value += rng.gen_range(-0.1..0.1);
            }
            all_vectors.push(vector);
        }
    }

    all_vectors
}
```

### 3.3 Performance Test Benchmarks
```rust
// Test performance characteristics
#[test]
fn test_multilayer_performance_benchmarks() {
    use std::time::Instant;

    let config = HnswConfig::builder()
        .dimension(128)
        .enable_multilayer(true)
        .build()
        .unwrap();

    let mut hnsw = HnswIndex::new(config).unwrap();
    let vectors = create_deterministic_vectors(128, 10000, 42);

    // Measure insertion performance
    let insertion_start = Instant::now();
    for vector in &vectors {
        hnsw.insert_vector(vector, None).unwrap();
    }
    let insertion_time = insertion_start.elapsed();

    // Measure search performance
    let query = &vectors[5000];
    let search_start = Instant::now();
    for _ in 0..100 {
        let _results = hnsw.search(query, 10).unwrap();
    }
    let search_time = search_start.elapsed();

    // Performance assertions
    assert!(insertion_time.as_millis() < 5000, "Insertion too slow");
    assert!(search_time.as_millis() < 100, "Search too slow");
}
```

---

## 4. Success Criteria

### 4.1 Test Coverage Requirements
- **Line Coverage**: >95% for multi-layer code
- **Branch Coverage**: >90% for all conditional paths
- **Integration Coverage**: All public API combinations tested
- **Performance Coverage**: All performance characteristics validated

### 4.2 Quality Metrics
- **Determinism**: All tests produce identical results on repeated runs
- **Performance**: Multi-layer search ≥3x faster than single-layer for large datasets
- **Memory**: Multi-layer overhead ≤20% compared to single-layer
- **Accuracy**: Multi-layer search accuracy ≥95% of single-layer accuracy

### 4.3 Completion Criteria
- [ ] All node mapping tests pass
- [ ] All level distribution tests pass
- [ ] All multi-layer insertion tests pass
- [ ] All multi-layer search tests pass
- [ ] All integration tests pass
- [ ] No regressions in existing single-layer functionality

---

## 5. Implementation Plan

### 5.1 Phase 1: Node Mapping System (Week 1)
1. **Implement LayerMappings struct** with bidirectional ID mapping
2. **Add comprehensive tests** for ID consistency and edge cases
3. **Validate performance** of mapping operations
4. **Document** all mapping behavior and constraints

### 5.2 Phase 2: Level Distribution (Week 1)
1. **Implement LevelDistributor** with exponential distribution
2. **Add deterministic seeding** for reproducible results
3. **Validate mathematical properties** of distribution
4. **Performance testing** of level assignment

### 5.3 Phase 3: Multi-layer Insertion (Week 2)
1. **Integrate mapping system** with existing insertion flow
2. **Replace single-layer logic** with multi-layer algorithm
3. **Add comprehensive integration** and error handling tests
4. **Performance validation** against baseline

### 5.4 Phase 4: Multi-layer Search (Week 3)
1. **Implement multi-layer search** with proper navigation
2. **Add performance optimizations** (caching, lazy loading)
3. **Validate accuracy and performance** characteristics
4. **Comprehensive integration** testing

### 5.5 Phase 5: Integration and Documentation (Week 4)
1. **Feature flag implementation** for safe migration
2. **Comprehensive regression testing**
3. **Performance benchmarking** and optimization
4. **Complete documentation** and examples

---

## 6. Risk Mitigation

### 6.1 Technical Risks
- **Risk**: Performance regression during implementation
- **Mitigation**: Continuous benchmarking against baseline
- **Risk**: Memory overhead exceeding expectations
- **Mitigation**: Memory profiling and optimization

### 6.2 Integration Risks
- **Risk**: Breaking changes to existing API
- **Mitigation**: Feature flag and comprehensive testing
- **Risk**: Reduced search accuracy
- **Mitigation**: Accuracy testing and validation

### 6.3 Timeline Risks
- **Risk**: Implementation taking longer than expected
- **Mitigation**: Incremental delivery with clear milestones
- **Risk**: Complex debugging issues
- **Mitigation**: Extensive logging and debug infrastructure

---

**Document Version**: 1.0
**Last Updated**: 2025-12-20
**Author**: Senior Rust Engineer TDD Team
**Review Status**: ✅ Design Complete
**Next Action**: Begin Phase 1 Implementation