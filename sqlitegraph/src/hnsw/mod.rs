//! Hierarchical Navigable Small World (HNSW) Vector Search
//!
//! This module provides high-performance approximate nearest neighbor search
//! capabilities using the HNSW algorithm with full SQLiteGraph integration.
//!
//! # Architecture
//!
//! The HNSW implementation is organized into focused modules:
//!
//! - **config**: HNSW algorithm configuration and parameters
//! - **builder**: Fluent configuration builder with validation
//! - **distance_metric**: SIMD-ready vector distance calculations
//! - **errors**: Comprehensive error handling for all HNSW operations
//!
//! # Features
//!
//! - **High Performance**: O(log N) average search complexity
//! - **Memory Efficient**: 2-3x vector size memory overhead
//! - **Dynamic Updates**: Insert and delete without full rebuilds
//! - **SIMD Optimized**: AVX2/AVX-512 support for distance calculations
//! - **SQLite Integration**: Persistent storage with SQLite backend
//!
//! # Quick Start
//!
//! ```rust
//! use sqlitegraph::hnsw::{HnswConfig, DistanceMetric};
//!
//! let config = HnswConfig::builder()
//!     .dimension(768)
//!     .m_connections(16)
//!     .ef_construction(200)
//!     .ef_search(50)
//!     .distance_metric(DistanceMetric::Cosine)
//!     .build()?;
//!
//! // HNSW index ready for use
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Performance Characteristics
//!
//! ## Search Performance
//! - **Time Complexity**: O(log N) average case
//! - **Space Complexity**: O(N * M) where M is connections per node
//! - **Accuracy**: 95%+ recall for typical workloads
//!
//! ## Construction Performance
//! - **Build Time**: O(N log N) with parallel construction support
//! - **Memory Usage**: 2.5x vector data size during construction
//! - **Batch Insert**: Optimized for bulk loading scenarios
//!
//! # Configuration Guidelines
//!
//! ## For High Accuracy
//! ```rust
//! let precise_config = HnswConfig::builder()
//!     .dimension(768)
//!     .m_connections(32)        // Higher M for better recall
//!     .ef_construction(400)     // Higher ef for better quality
//!     .ef_search(100)           // Higher ef for better search
//!     .build()?;
//! ```
//!
//! ## For Fast Construction
//! ```rust
//! let fast_config = HnswConfig::builder()
//!     .dimension(768)
//!     .m_connections(12)        // Lower M for faster build
//!     .ef_construction(100)     // Lower ef for faster build
//!     .ef_search(20)            // Lower ef for faster search
//!     .build()?;
//! ```
//!
//! # Distance Metrics
//!
//! The HNSW implementation supports multiple distance metrics:
//!
//! - **Cosine**: Ideal for normalized vectors and text embeddings
//! - **Euclidean**: L2 distance, suitable for general-purpose similarity
//! - **Dot Product**: Fast approximate cosine for normalized vectors
//! - **Manhattan**: L1 distance, robust to outliers
//!
//! # Error Handling
//!
//! All HNSW operations return Result types with comprehensive error information:
//!
//! ```rust
//! use sqlitegraph::hnsw::{HnswConfig, HnswConfigError};
//!
//! match HnswConfig::builder().dimension(0).build() {
//!     Ok(config) => println!("Valid config"),
//!     Err(HnswConfigError::InvalidDimension) => {
//!         println!("Vector dimension must be > 0");
//!     }
//!     Err(e) => println!("Other error: {}", e),
//! }
//! ```
//!
//! # Integration with SQLiteGraph
//!
//! The HNSW module is designed to integrate seamlessly with SQLiteGraph's
//! graph database capabilities, enabling vector-augmented graph queries
//! and semantic search over graph entities.
//!
//! ```rust
//! // Future integration example (planned)
//! let graph = SqliteGraph::open("example.db")?;
//! let hnsw = graph.hnsw_index("vectors")?;
//! let results = hnsw.vector_search(query_vector, 10)?;
//! let graph_results = graph.filter_entities_by_ids(results)?;
//! ```

// Re-export public API
pub use builder::HnswConfigBuilder;
pub use config::{HnswConfig, hnsw_config};
pub use distance_metric::{DistanceMetric, compute_distance};
pub use errors::{
    HnswConfigError, HnswError, HnswIndexError, HnswMultiLayerError, HnswStorageError,
};
pub use index::{HnswIndex, HnswIndexStats};
pub use storage::{
    InMemoryVectorStorage, VectorBatch, VectorRecord, VectorStorage, VectorStorageStats,
};

// Multi-layer HNSW components
pub use multilayer::{LayerMappings, LevelDistributor, MultiLayerNodeManager};

// Module organization
pub mod builder;
pub mod config;
pub mod distance_functions;
pub mod distance_metric;
pub mod errors;
pub mod index;
pub mod layer;
pub mod multilayer;
pub mod neighborhood;
pub mod storage;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hnsw_module_imports() {
        // Test that all modules can be imported successfully
        let _config = HnswConfig::default();
        let _builder = HnswConfigBuilder::new();
        let _metric = DistanceMetric::Cosine;

        // Test default configuration
        assert_eq!(_config.dimension, 768);
        assert_eq!(_config.distance_metric, DistanceMetric::Cosine);
    }

    #[test]
    fn test_hnsw_config_builder() {
        let config = HnswConfigBuilder::new()
            .dimension(256)
            .m_connections(12)
            .ef_construction(150)
            .ef_search(40)
            .max_layers(12)
            .distance_metric(DistanceMetric::Euclidean)
            .build()
            .unwrap();

        assert_eq!(config.dimension, 256);
        assert_eq!(config.m, 12);
        assert_eq!(config.ef_construction, 150);
        assert_eq!(config.ef_search, 40);
        assert_eq!(config.ml, 12);
        assert_eq!(config.distance_metric, DistanceMetric::Euclidean);
    }

    #[test]
    fn test_distance_metrics() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];

        let cosine_dist = compute_distance(DistanceMetric::Cosine, &a, &b);
        let euclidean_dist = compute_distance(DistanceMetric::Euclidean, &a, &b);
        let manhattan_dist = compute_distance(DistanceMetric::Manhattan, &a, &b);
        let dot_dist = compute_distance(DistanceMetric::DotProduct, &a, &b);

        assert!((cosine_dist - 0.5).abs() < f32::EPSILON); // (1 - 0) / 2
        assert!((euclidean_dist - 1.41421356).abs() < f32::EPSILON);
        assert_eq!(manhattan_dist, 2.0);
        assert_eq!(dot_dist, 0.0); // -dot_product
    }

    #[test]
    fn test_error_handling() {
        // Test that the build() method catches validation errors
        let result = HnswConfigBuilder::new().build(); // Default has dimension 768, so this should pass
        assert!(result.is_ok());

        // Test dimension 0 should be caught by build() method
        let mut builder = HnswConfigBuilder::new();
        builder.config.dimension = 0; // Direct field access for testing
        let result = builder.build();
        assert!(matches!(result, Err(HnswConfigError::InvalidDimension)));
    }

    #[test]
    fn test_hnsw_config_function() {
        let config = hnsw_config()
            .dimension(512)
            .m_connections(24)
            .distance_metric(DistanceMetric::Manhattan)
            .build()
            .unwrap();

        assert_eq!(config.dimension, 512);
        assert_eq!(config.m, 24);
        assert_eq!(config.distance_metric, DistanceMetric::Manhattan);
    }

    #[test]
    fn test_default_configuration() {
        let config = HnswConfig::default();

        // Verify reasonable defaults
        assert_eq!(config.dimension, 768); // Common embedding size
        assert_eq!(config.m, 16); // Balanced connectivity
        assert_eq!(config.ef_construction, 200); // Good construction quality
        assert_eq!(config.ef_search, 50); // Balanced search speed/quality
        assert_eq!(config.ml, 16); // Reasonable depth
        assert_eq!(config.distance_metric, DistanceMetric::Cosine);
    }

    #[test]
    fn test_high_precision_configuration() {
        let config = HnswConfig {
            dimension: 1536,
            m: 32,
            ef_construction: 400,
            ef_search: 100,
            ml: 24,
            distance_metric: DistanceMetric::Cosine,
            enable_multilayer: false,
            multilayer_level_distribution_base: None,
            multilayer_deterministic_seed: None,
        };

        assert_eq!(config.dimension, 1536);
        assert_eq!(config.m, 32);
        assert!(config.ef_construction >= config.m);
        assert_eq!(config.distance_metric, DistanceMetric::Cosine);
        assert!(!config.enable_multilayer);
    }

    #[test]
    fn test_fast_construction_configuration() {
        let config = HnswConfig {
            dimension: 384,
            m: 8,
            ef_construction: 100,
            ef_search: 20,
            ml: 12,
            distance_metric: DistanceMetric::Euclidean,
            enable_multilayer: false,
            multilayer_level_distribution_base: None,
            multilayer_deterministic_seed: None,
        };

        assert_eq!(config.dimension, 384);
        assert_eq!(config.m, 8);
        assert_eq!(config.ef_construction, 100);
        assert_eq!(config.ef_search, 20);
        assert_eq!(config.distance_metric, DistanceMetric::Euclidean);
        assert!(!config.enable_multilayer);
    }
}
