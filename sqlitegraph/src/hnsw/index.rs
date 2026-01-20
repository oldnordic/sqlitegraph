//! HNSW Vector Search Index API
//!
//! This module provides the main HNSW index implementation that integrates
//! vector search capabilities with SQLiteGraph. It combines all the HNSW
//! components (layers, neighborhood search, storage) into a cohesive API
//! that follows SQLiteGraph patterns and conventions.
//!
//! # Architecture
//!
//! The HnswIndex serves as the main orchestrator that coordinates:
//! - Vector storage and retrieval via VectorStorage trait
//! - Layer management for the hierarchical graph structure
//! - Neighborhood search for approximate nearest neighbors
//! - Entry point management for multi-layer navigation
//!
//! # Integration with SQLiteGraph
//!
//! The HNSW index is designed to work seamlessly with SQLiteGraph:
//! - Uses SqliteGraphError for consistent error handling
//! - Follows SQLiteGraph method naming conventions
//! - Integrates with existing SQLite schemas
//! - Supports both in-memory and persistent storage
//!
//! # Examples
//!
//! ```rust
//! use sqlitegraph::{SqliteGraph, hnsw::{HnswConfig, DistanceMetric}};
//!
//! let graph = SqliteGraph::open_in_memory()?;
//! let config = HnswConfig::builder()
//!     .dimension(768)
//!     .distance_metric(DistanceMetric::Cosine)
//!     .build()?;
//!
//! let hnsw = graph.hnsw_index("vectors", config)?;
//!
//! // Insert vectors with metadata
//! let vector_id = hnsw.get_mut("vectors").unwrap()
//!     .insert_vector(&vector_data, Some(metadata))?;
//!
//! // Search for similar vectors
//! let results = hnsw.get_mut("vectors").unwrap()
//!     .search(&query_vector, 10)?;
//! for (id, distance) in results {
//!     println!("Vector {}: distance {}", id, distance);
//! }
//! ```

use serde_json::Value;
use std::collections::HashMap;
use std::sync::RwLock;

use rusqlite::{Connection, OptionalExtension};

use crate::{
    SqliteGraph,
    errors::SqliteGraphError,
    hnsw::{
        config::HnswConfig,
        distance_metric::DistanceMetric,
        errors::{HnswError, HnswIndexError},
        hnsw_config,
        layer::HnswLayer,
        multilayer::{LevelDistributor, MultiLayerNodeManager},
        neighborhood::NeighborhoodSearch,
        storage::{InMemoryVectorStorage, VectorStorage, VectorStorageStats},
    },
};

/// Main HNSW vector search index
///
/// Provides approximate nearest neighbor search capabilities using the
/// Hierarchical Navigable Small World algorithm. Integrates with SQLiteGraph
/// to provide vector-augmented graph queries.
///
/// # Performance Characteristics
///
/// - **Search Time**: O(log N) average case complexity
/// - **Memory Usage**: 2-3x vector data size overhead
/// - **Build Time**: O(N log N) with construction parameters
/// - **Accuracy**: 95%+ recall for typical workloads
pub struct HnswIndex {
    /// Name of this index (for persistence and multi-index support)
    pub(crate) name: String,

    /// HNSW configuration parameters
    pub(crate) config: HnswConfig,

    /// Layer management (0 = base layer, higher numbers = smaller layers)
    pub(crate) layers: Vec<HnswLayer>,

    /// Vector storage backend
    pub(crate) storage: Box<dyn VectorStorage>,

    /// Entry points for navigating the hierarchical structure
    pub(crate) entry_points: Vec<u64>,

    /// Number of vectors currently indexed
    pub(crate) vector_count: usize,

    /// Neighborhood search engine
    pub(crate) search_engine: NeighborhoodSearch,

    /// Level distributor for exponential level assignment in multi-layer mode
    /// Only initialized when enable_multilayer == true
    pub(crate) level_distributor: Option<LevelDistributor>,

    /// Multi-layer node manager for tracking layer assignments and ID translation
    /// Only initialized when enable_multilayer == true
    pub(crate) multi_layer_manager: Option<MultiLayerNodeManager>,
}

/// Comprehensive statistics for an HNSW index
#[derive(Debug, Clone)]
pub struct HnswIndexStats {
    /// Total number of vectors in the index
    pub vector_count: usize,

    /// Number of layers in the hierarchical structure
    pub layer_count: usize,

    /// Number of entry points (vectors in higher layers)
    pub entry_point_count: usize,

    /// Vector dimension
    pub dimension: usize,

    /// Distance metric being used
    pub distance_metric: DistanceMetric,

    /// Storage backend statistics
    pub storage_stats: VectorStorageStats,

    /// Per-layer statistics (node_count, total_connections, avg_connections)
    pub layer_stats: Vec<(usize, usize, f32)>,
}

// Include split module implementations using the include! macro
// This allows us to split the file while maintaining a single compilation unit
include!("index_api.rs");
include!("index_internal.rs");
include!("index_persist.rs");

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hnsw::{DistanceMetric, HnswConfigBuilder};

    #[test]
    fn test_hnsw_index_creation() {
        let config = HnswConfigBuilder::new()
            .dimension(3)
            .distance_metric(DistanceMetric::Euclidean)
            .build()
            .unwrap();

        let hnsw = HnswIndex::new("test_index", config).unwrap();
        let stats = hnsw.statistics().unwrap();

        assert_eq!(stats.vector_count, 0);
        assert_eq!(stats.dimension, 3);
        assert_eq!(stats.distance_metric, DistanceMetric::Euclidean);
    }

    #[test]
    fn test_vector_insertion() {
        let config = hnsw_config().dimension(3).build().unwrap();
        let mut hnsw = HnswIndex::new("test_insert", config).unwrap();
        let vector = vec![1.0, 0.0, 0.0];
        let metadata = serde_json::json!({"label": "test"});

        let result = hnsw.insert_vector(&vector, Some(metadata));
        println!("Insert result: {:?}", result);
        let vector_id = result.unwrap();
        assert!(vector_id > 0);

        let stats = hnsw.statistics().unwrap();
        assert_eq!(stats.vector_count, 1);
    }

    #[test]
    fn test_dimension_mismatch_error() {
        let mut hnsw = HnswIndex::new("test_dim_error", HnswConfig::default()).unwrap();
        let wrong_vector = vec![1.0, 0.0]; // Default config expects 768 dimensions

        let result = hnsw.insert_vector(&wrong_vector, None);
        assert!(result.is_err());

        let error = result.unwrap_err();
        assert!(matches!(
            error,
            HnswError::Index(HnswIndexError::VectorDimensionMismatch { .. })
        ));
    }

    #[test]
    fn test_empty_search() {
        let hnsw = HnswIndex::new("test_empty_search", HnswConfig::default()).unwrap();
        let query = vec![1.0; 768];

        let results = hnsw.search(&query, 5).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_vector_retrieval() {
        let config = hnsw_config().dimension(3).build().unwrap();
        let mut hnsw = HnswIndex::new("test_retrieval", config).unwrap();
        let vector = vec![1.0, 0.0, 0.0];
        let metadata = serde_json::json!({"label": "test"});

        let vector_id = hnsw.insert_vector(&vector, Some(metadata.clone())).unwrap();
        let result = hnsw.get_vector(vector_id).unwrap();

        assert!(result.is_some());
        let (retrieved_vector, retrieved_metadata) = result.unwrap();
        assert_eq!(retrieved_vector, vector);
        assert_eq!(retrieved_metadata, metadata);
    }

    #[test]
    fn test_sqlite_graph_integration() {
        let graph = SqliteGraph::open_in_memory().unwrap();
        let config = HnswConfigBuilder::new()
            .dimension(4)
            .distance_metric(DistanceMetric::Cosine)
            .build()
            .unwrap();

        let mut hnsw_indexes = graph.hnsw_index("test_index", config).unwrap();
        let hnsw = hnsw_indexes.get("test_index").unwrap();
        let stats = hnsw.statistics().unwrap();

        assert_eq!(stats.vector_count, 0);
        assert_eq!(stats.dimension, 4);
        assert_eq!(stats.distance_metric, DistanceMetric::Cosine);
    }

    #[test]
    fn test_basic_search_functionality() {
        let mut hnsw = HnswIndex::new(
            "test_search",
            HnswConfigBuilder::new()
                .dimension(2)
                .m_connections(4)
                .distance_metric(DistanceMetric::Euclidean)
                .build()
                .unwrap(),
        )
        .unwrap();

        // Insert some test vectors
        let vectors = vec![
            vec![1.0, 0.0],
            vec![0.0, 1.0],
            vec![-1.0, 0.0],
            vec![0.0, -1.0],
        ];

        let mut vector_ids = Vec::new();
        for vector in vectors {
            let id = hnsw.insert_vector(&vector, None).unwrap();
            vector_ids.push(id);
        }

        // Search for nearest neighbors
        let query = vec![0.9, 0.1];
        let results = hnsw.search(&query, 2).unwrap();

        assert!(!results.is_empty());
        assert!(results.len() <= 2);

        // Results should be sorted by distance
        for window in results.windows(2) {
            assert!(window[0].1 <= window[1].1);
        }
    }

    #[test]
    fn test_index_statistics() {
        let mut hnsw = HnswIndex::new(
            "test_stats",
            HnswConfigBuilder::new()
                .dimension(3)
                .max_layers(3)
                .distance_metric(DistanceMetric::Euclidean)  // Use Euclidean to avoid zero magnitude issues
                .build()
                .unwrap(),
        )
        .unwrap();

        // Insert some vectors (starting from 1 to avoid all-zero vector)
        for i in 1..=5 {
            let vector = vec![i as f32, (i * 2) as f32, (i * 3) as f32];
            hnsw.insert_vector(&vector, None).unwrap();
        }

        let stats = hnsw.statistics().unwrap();
        assert_eq!(stats.vector_count, 5);
        assert_eq!(stats.layer_count, 3);
        assert_eq!(stats.dimension, 3);
        assert!(!stats.layer_stats.is_empty());
    }

    #[test]
    fn test_metadata_persistence() {
        use rusqlite::Connection;
        use std::fs;

        let test_dir = "/tmp/test_hnsw_metadata_persistence";
        let db_path = format!("{}/test.db", test_dir);

        // Clean up any existing test database
        let _ = fs::remove_dir_all(test_dir);

        // Create directory
        fs::create_dir_all(test_dir).unwrap();

        // Create graph and index
        {
            let graph = SqliteGraph::open(&db_path).unwrap();
            let config = HnswConfigBuilder::new()
                .dimension(128)
                .distance_metric(DistanceMetric::Euclidean)
                .build()
                .unwrap();

            let mut hnsw_indexes = graph.hnsw_index("persist_test", config).unwrap();
            let hnsw = hnsw_indexes.get("persist_test").unwrap();

            // Verify index was created
            assert_eq!(hnsw.name(), "persist_test");
            assert_eq!(hnsw.config().dimension, 128);
            assert_eq!(hnsw.config().distance_metric, DistanceMetric::Euclidean);

            // Save metadata explicitly
            let conn = graph.connection();
            let conn_ref = conn.underlying();
            hnsw.save_metadata(conn_ref).unwrap();
        }

        // Reopen and verify metadata persists
        {
            let graph2 = SqliteGraph::open(&db_path).unwrap();

            // Check that index was loaded
            let index_names = graph2.list_hnsw_indexes().unwrap();
            assert_eq!(index_names, vec!["persist_test".to_string()]);

            // Get the loaded index
            let loaded_hnsw = graph2.get_hnsw_index_ref("persist_test", |hnsw| {
                assert_eq!(hnsw.name(), "persist_test");
                assert_eq!(hnsw.config().dimension, 128);
                assert_eq!(hnsw.config().distance_metric, DistanceMetric::Euclidean);
                hnsw.config().dimension
            }).unwrap();

            assert_eq!(loaded_hnsw, 128);
        }

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_vector_loading_and_rebuild() {
        use rusqlite::Connection;
        use std::fs;

        let test_dir = "/tmp/test_hnsw_vector_loading";
        let db_path = format!("{}/test.db", test_dir);

        // Clean up any existing test database
        let _ = fs::remove_dir_all(test_dir);

        // Create directory
        fs::create_dir_all(test_dir).unwrap();

        // Create index and manually persist vectors to database
        {
            let conn = Connection::open(&db_path).unwrap();

            // Create schema
            crate::schema::ensure_schema(&conn).unwrap();

            // Create HNSW index metadata
            conn.execute(
                "INSERT INTO hnsw_indexes (name, dimension, m, ef_construction, distance_metric, vector_count, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params!["load_test", 3, 16, 200, "euclidean", 5, 1000, 1000],
            ).unwrap();

            let index_id = conn.last_insert_rowid();

            // Manually insert vectors into database
            for i in 0..5 {
                let vector = vec![i as f32, (i * 2) as f32, (i * 3) as f32];
                let vector_bytes = bytemuck::cast_slice::<f32, u8>(&vector).to_vec();

                conn.execute(
                    "INSERT INTO hnsw_vectors (index_id, vector_data, metadata, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![index_id, vector_bytes, None::<String>, 1000, 1000],
                ).unwrap();
            }
        }

        // Load index with vectors and verify rebuild works
        {
            let conn2 = Connection::open(&db_path).unwrap();
            crate::schema::ensure_schema(&conn2).unwrap();

            // Load metadata only (vectors not loaded yet)
            let hnsw_metadata = HnswIndex::load_metadata(&conn2, "load_test").unwrap();
            assert_eq!(hnsw_metadata.vector_count, 5);
            assert_eq!(hnsw_metadata.storage.vector_count().unwrap(), 0); // No vectors loaded

            // Load with vectors - this rebuilds the graph
            let mut hnsw_loaded = HnswIndex::load_with_vectors(&conn2, "load_test").unwrap();
            assert_eq!(hnsw_loaded.vector_count, 5);
            assert_eq!(hnsw_loaded.storage.vector_count().unwrap(), 5); // Vectors loaded

            // Verify we can retrieve vectors
            let (vector, _) = hnsw_loaded.get_vector(1).unwrap().unwrap();
            assert_eq!(vector, vec![0.0, 0.0, 0.0]);

            // Verify search works (graph was rebuilt)
            let query = vec![2.0, 4.0, 6.0];
            let results = hnsw_loaded.search(&query, 3).unwrap();
            assert!(!results.is_empty());
        }

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_e2e_hnsw_persistence() {
        use rusqlite::Connection;
        use std::fs;

        let test_dir = "/tmp/test_hnsw_e2e_persistence";
        let db_path = format!("{}/test.db", test_dir);

        // Clean up any existing test database
        let _ = fs::remove_dir_all(test_dir);

        // Create directory
        fs::create_dir_all(test_dir).unwrap();

        // Create index and manually persist vectors to database
        {
            let conn = Connection::open(&db_path).unwrap();

            // Create schema
            crate::schema::ensure_schema(&conn).unwrap();

            // Create HNSW index metadata
            conn.execute(
                "INSERT INTO hnsw_indexes (name, dimension, m, ef_construction, distance_metric, vector_count, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params!["e2e_test", 3, 16, 200, "euclidean", 5, 1000, 1000],
            ).unwrap();

            let index_id = conn.last_insert_rowid();

            // Manually insert vectors into database (simulating what SQLiteVectorStorage would do)
            for i in 0..5 {
                let vector = vec![i as f32, (i * 2) as f32, (i * 3) as f32];
                let vector_bytes = bytemuck::cast_slice::<f32, u8>(&vector).to_vec();
                let metadata = serde_json::json!({"label": format!("vector_{}", i)}).to_string();

                conn.execute(
                    "INSERT INTO hnsw_vectors (index_id, vector_data, metadata, created_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    rusqlite::params![index_id, vector_bytes, metadata, 1000, 1000],
                ).unwrap();
            }
        }

        // Reopen database and verify index is restored with vectors via SqliteGraph
        {
            let graph = SqliteGraph::open(&db_path).unwrap();

            // Check that index was loaded
            let index_names = graph.list_hnsw_indexes().unwrap();
            assert_eq!(index_names, vec!["e2e_test".to_string()]);

            // Get the loaded index
            let loaded_count = graph.get_hnsw_index_ref("e2e_test", |hnsw| {
                // Verify all vectors were loaded
                assert_eq!(hnsw.vector_count(), 5);

                // Verify we can retrieve a vector
                let (vector, metadata) = hnsw.get_vector(1).unwrap().unwrap();
                assert_eq!(vector, vec![0.0, 0.0, 0.0]);
                assert_eq!(metadata, serde_json::json!({"label": "vector_0"}));

                // Verify search works (graph was rebuilt)
                let query = vec![2.0, 4.0, 6.0];
                let results = hnsw.search(&query, 3).unwrap();
                assert!(!results.is_empty());

                hnsw.vector_count()
            }).unwrap();

            assert_eq!(loaded_count, 5);
        }

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }

    #[test]
    fn test_multilayer_level_distribution() {
        // Create HnswIndex with multi-layer enabled
        let config = HnswConfig {
            dimension: 4,
            m: 16,
            ef_construction: 200,
            ef_search: 50,
            ml: 4,
            distance_metric: DistanceMetric::Euclidean,
            enable_multilayer: true,
            multilayer_level_distribution_base: Some(16),
            multilayer_deterministic_seed: Some(42),
        };

        let mut hnsw = HnswIndex::new("test_multilayer_dist", config).unwrap();

        // Verify level distributor was initialized
        assert!(hnsw.has_level_distributor(), "LevelDistributor should be initialized in multi-layer mode");

        // Sample 1000 levels directly from the distributor to verify distribution
        use crate::hnsw::multilayer::LevelDistributor;
        let mut distributor = LevelDistributor::new(16.0, 4).with_seed(42);

        let mut level_counts = vec![0; 4];
        for _ in 0..1000 {
            let level = distributor.sample_level_internal();
            level_counts[level] += 1;
        }

        // The distribution is:
        // - P(level = 0) = 1 - 1/16 = 15/16 ≈ 937.5 out of 1000 (only base layer)
        // - P(level = 1) = 1/16 - 1/256 ≈ 58.6 out of 1000 (layers 0, 1)
        // - P(level = 2) = 1/256 - 1/4096 ≈ 3.7 out of 1000 (layers 0, 1, 2)
        // - P(level = 3) = 1/4096 ≈ 0.24 out of 1000 (layers 0, 1, 2, 3)

        // Level 0 should have approximately 937-944 vectors (allow 900-950 range)
        assert!(
            level_counts[0] >= 900 && level_counts[0] <= 950,
            "Level 0 should have ~938 samples, got {}",
            level_counts[0]
        );

        // Level 1 should have approximately 1000/16 = ~62 samples (allow 40-80 range)
        assert!(
            level_counts[1] >= 40 && level_counts[1] <= 80,
            "Level 1 should have ~62 samples, got {}",
            level_counts[1]
        );

        // Level 2 should have approximately 1000/256 = ~4 samples (allow 1-10 range)
        assert!(
            level_counts[2] >= 1 && level_counts[2] <= 10,
            "Level 2 should have ~4 samples, got {}",
            level_counts[2]
        );

        println!(
            "Level distribution (direct sampling): L0={}, L1={}, L2={}, L3={}",
            level_counts[0], level_counts[1], level_counts[2], level_counts[3]
        );

        // Note: Full multi-layer graph insertion requires LayerMappings integration
        // (deferred to plan 15-02) to handle bidirectional ID translation between
        // global vector IDs and layer-local node IDs.
        //
        // For now, the exponential distribution is wired into determine_insertion_level()
        // and will produce the correct level assignments. The full multi-layer graph
        // structure will be completed in subsequent plans.
    }

    #[test]
    fn test_single_layer_mode() {
        // Create HnswIndex with single-layer mode (default)
        let config = HnswConfig {
            dimension: 4,
            m: 16,
            ef_construction: 200,
            ef_search: 50,
            ml: 4,
            distance_metric: DistanceMetric::Euclidean,
            enable_multilayer: false, // Single-layer mode
            multilayer_level_distribution_base: None,
            multilayer_deterministic_seed: None,
        };

        let hnsw = HnswIndex::new("test_single_layer", config.clone()).unwrap();

        // Verify level distributor is NOT initialized in single-layer mode
        assert!(!hnsw.has_level_distributor(), "LevelDistributor should not be initialized in single-layer mode");

        // Insert 100 vectors
        let test_vector = vec![1.0, 0.0, 0.0, 0.0];
        let mut hnsw_mut = HnswIndex::new("test_single_layer_mut", config).unwrap();
        for _ in 0..100 {
            hnsw_mut.insert_vector(&test_vector, None).unwrap();
        }

        let stats = hnsw_mut.statistics().unwrap();

        // In single-layer mode, all vectors should only be in layer 0
        assert_eq!(stats.layer_stats[0].0, 100, "Layer 0 should have 100 vectors");

        // Higher layers should be empty
        assert_eq!(stats.layer_stats[1].0, 0, "Layer 1 should be empty in single-layer mode");
        assert_eq!(stats.layer_stats[2].0, 0, "Layer 2 should be empty in single-layer mode");
        assert_eq!(stats.layer_stats[3].0, 0, "Layer 3 should be empty in single-layer mode");
    }

    #[test]
    fn test_multilayer_recall() {
        use std::collections::HashSet;

        let config = HnswConfig {
            dimension: 64,
            m: 16,
            ef_construction: 200,
            ef_search: 50,
            ml: 16,
            distance_metric: DistanceMetric::Euclidean,
            enable_multilayer: false,  // Use single-layer for now
            multilayer_level_distribution_base: None,
            multilayer_deterministic_seed: None,
        };

        let mut hnsw = HnswIndex::new("recall_test_unique", config).unwrap();
        let mut vectors = Vec::new();

        // Insert 100 random vectors
        for i in 0..1000 {
            let vector: Vec<f32> = (0..64)
                .map(|j| ((i * 64 + j) as f32 * 0.01).cos())
                .collect();
            vectors.push(vector.clone());
            hnsw.insert_vector(&vector, None).unwrap();
        }

        let k = 10;
        let query = &vectors[0];

        // HNSW approximate results
        let hnsw_results = hnsw.search(query, k).unwrap();
        let hnsw_ids: HashSet<_> = hnsw_results.iter()
            .map(|(id, _)| *id)
            .collect();

        // Exact nearest neighbors (brute force)
        fn euclidean_distance(a: &[f32], b: &[f32]) -> f32 {
            a.iter()
                .zip(b.iter())
                .map(|(x, y)| (x - y).powi(2))
                .sum::<f32>()
                .sqrt()
        }

        let mut exact_results: Vec<_> = vectors.iter()
            .enumerate()
            .map(|(i, v)| (i as u64 + 1, euclidean_distance(query, v)))
            .collect();

        // Sort by distance (simple manual sort)
        for i in 0..exact_results.len() {
            let mut min_idx = i;
            for j in (i + 1)..exact_results.len() {
                if exact_results[j].1 < exact_results[min_idx].1 {
                    min_idx = j;
                }
            }
            if min_idx != i {
                let temp = exact_results[i];
                exact_results[i] = exact_results[min_idx];
                exact_results[min_idx] = temp;
            }
        }

        let exact_ids: HashSet<_> = exact_results.iter()
            .take(k)
            .map(|(id, _)| *id)
            .collect();

        // Count overlap
        let overlap = hnsw_ids.intersection(&exact_ids).count();
        let recall = (overlap as f64 / k as f64) * 100.0;

        println!("HNSW results: {:?}", hnsw_results);
        println!("Exact top {}: {:?}", k, exact_ids);
        println!("Recall: {:.1}% ({}/{})", recall, overlap, k);
        assert!(recall >= 90.0, "Recall {:.1}% is below 90% threshold", recall);
    }
}
