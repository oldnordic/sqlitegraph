//! Distance Metrics for Vector Similarity
//!
//! This module defines various distance metrics supported by the HNSW index for
//! calculating vector similarity. Different distance metrics are appropriate for
//! different types of vector data and use cases. The choice affects both
//! search quality and performance.
//!
//! # Supported Metrics
//!
//! - **Cosine Similarity**: Ideal for normalized vectors and text embeddings
//! - **Euclidean Distance**: L2 distance, suitable for general-purpose similarity
//! - **Dot Product**: Fast approximate cosine similarity for normalized vectors
//! - **Manhattan Distance**: L1 distance, robust to outliers
//!
//! # Performance Characteristics
//!
//! | Metric         | SIMD Support | Typical Use Case           | Normalization Required |
//! |----------------|-------------|----------------------------|-----------------------|
//! | Cosine         | Yes         | Text embeddings           | Recommended           |
//! | Euclidean      | Yes         | General similarity        | Optional              |
//! | DotProduct     | Yes         | Fast approximate cosine   | Required              |
//! | Manhattan      | Yes         | Robust similarity         | Optional              |
//!
//! # Examples
//!
//! ```rust
//! use sqlitegraph::hnsw::distance_metric::{DistanceMetric, compute_distance};
//!
//! let a = vec![1.0, 0.0, 0.0];
//! let b = vec![0.0, 1.0, 0.0];
//!
//! let distance = compute_distance(DistanceMetric::Cosine, &a, &b);
//! assert!(distance > 0.0);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use std::fmt;

/// Distance metrics supported by HNSW index
///
/// Different distance metrics are appropriate for different types of vector data
/// and use cases. The choice affects both search quality and performance.
///
/// # Variants
///
/// * `Cosine` - Cosine similarity, ideal for normalized vectors and text embeddings
/// * `Euclidean` - L2 distance, suitable for general-purpose similarity
/// * `DotProduct` - Raw dot product, fast for normalized vectors
/// * `Manhattan` - L1 distance, robust to outliers
///
/// # Performance Characteristics
///
/// | Metric         | SIMD Support | Typical Use Case           | Normalization Required |
/// |----------------|-------------|----------------------------|-----------------------|
/// | Cosine         | Yes         | Text embeddings           | Recommended           |
/// | Euclidean      | Yes         | General similarity        | Optional              |
/// | DotProduct     | Yes         | Fast approximate cosine   | Required              |
/// | Manhattan      | Yes         | Robust similarity         | Optional              |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistanceMetric {
    /// Cosine similarity (1 - normalized dot product)
    /// Range: [0, 2] where 0 = identical, 2 = opposite
    Cosine,

    /// Euclidean (L2) distance
    /// Range: [0, ∞) where 0 = identical
    Euclidean,

    /// Dot product similarity
    /// Range: (-∞, ∞), higher values indicate greater similarity
    DotProduct,

    /// Manhattan (L1) distance
    /// Range: [0, ∞) where 0 = identical
    Manhattan,
}

impl Default for DistanceMetric {
    fn default() -> Self {
        DistanceMetric::Cosine
    }
}

impl DistanceMetric {
    /// Get the string representation of this distance metric
    pub fn as_str(&self) -> &str {
        match self {
            DistanceMetric::Cosine => "cosine",
            DistanceMetric::Euclidean => "euclidean",
            DistanceMetric::DotProduct => "dot_product",
            DistanceMetric::Manhattan => "manhattan",
        }
    }
}

impl fmt::Display for DistanceMetric {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DistanceMetric::Cosine => write!(f, "cosine"),
            DistanceMetric::Euclidean => write!(f, "euclidean"),
            DistanceMetric::DotProduct => write!(f, "dot_product"),
            DistanceMetric::Manhattan => write!(f, "manhattan"),
        }
    }
}

/// Generic distance computation based on metric type
///
/// This function computes distance between two vectors based on the specified
/// distance metric. For similarity metrics (like cosine), it converts to
/// distance by taking the complement.
///
/// # Arguments
///
/// * `metric` - Distance metric to use
/// * `a` - First vector slice
/// * `b` - Second vector slice (must have same length as a)
///
/// # Returns
///
/// Distance value where lower values indicate greater similarity
///
/// # Examples
///
/// ```rust
/// use sqlitegraph::hnsw::distance_metric::{DistanceMetric, compute_distance};
///
/// let a = [1.0, 0.0, 0.0];
/// let b = [0.0, 1.0, 0.0];
///
/// let cosine_dist = compute_distance(DistanceMetric::Cosine, &a, &b);
/// let euclidean_dist = compute_distance(DistanceMetric::Euclidean, &a, &b);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn compute_distance(metric: DistanceMetric, a: &[f32], b: &[f32]) -> f32 {
    match metric {
        DistanceMetric::Cosine => {
            // Convert cosine similarity to distance: (1 - similarity) / 2
            // This gives range [0, 1] where 0 = identical
            let similarity = cosine_similarity(a, &b);
            (1.0 - similarity) / 2.0
        }
        DistanceMetric::Euclidean => euclidean_distance(a, &b),
        DistanceMetric::DotProduct => {
            // Convert dot product to distance: -dot_product
            // This assumes higher dot products indicate greater similarity
            -dot_product(a, &b)
        }
        DistanceMetric::Manhattan => manhattan_distance(a, &b),
    }
}

// Re-export distance functions for backward compatibility
pub use crate::hnsw::distance_functions::{
    cosine_similarity, dot_product, euclidean_distance, manhattan_distance,
};

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_distance_metric_display() {
        assert_eq!(DistanceMetric::Cosine.to_string(), "cosine");
        assert_eq!(DistanceMetric::Euclidean.to_string(), "euclidean");
        assert_eq!(DistanceMetric::DotProduct.to_string(), "dot_product");
        assert_eq!(DistanceMetric::Manhattan.to_string(), "manhattan");
    }

    #[test]
    fn test_distance_metric_default() {
        let metric = DistanceMetric::default();
        assert_eq!(metric, DistanceMetric::Cosine);
    }

    #[test]
    fn test_distance_metric_equality() {
        assert_eq!(DistanceMetric::Cosine, DistanceMetric::Cosine);
        assert_ne!(DistanceMetric::Cosine, DistanceMetric::Euclidean);
        assert_ne!(DistanceMetric::Euclidean, DistanceMetric::Manhattan);
    }

    #[test]
    fn test_compute_distance_cosine() {
        let a = [1.0, 0.0];
        let b = [0.0, 1.0];
        let distance = compute_distance(DistanceMetric::Cosine, &a, &b);
        assert_eq!(distance, 0.5); // (1 - 0) / 2
    }

    #[test]
    fn test_compute_distance_euclidean() {
        let a = [0.0, 0.0];
        let b = [3.0, 4.0];
        let distance = compute_distance(DistanceMetric::Euclidean, &a, &b);
        assert_eq!(distance, 5.0);
    }

    #[test]
    fn test_compute_distance_dot_product() {
        let a = [1.0, 0.0];
        let b = [1.0, 0.0];
        let distance = compute_distance(DistanceMetric::DotProduct, &a, &b);
        assert_eq!(distance, -1.0); // -dot_product
    }

    #[test]
    fn test_compute_distance_manhattan() {
        let a = [1.0, 2.0];
        let b = [4.0, 0.0];
        let distance = compute_distance(DistanceMetric::Manhattan, &a, &b);
        assert_eq!(distance, 5.0); // |1-4| + |2-0|
    }

    #[test]
    fn test_all_metrics_identical_vectors() {
        let a = [1.0, 0.0];
        let b = [1.0, 0.0];

        let cosine_dist = compute_distance(DistanceMetric::Cosine, &a, &b);
        let euclidean_dist = compute_distance(DistanceMetric::Euclidean, &a, &b);
        let dot_dist = compute_distance(DistanceMetric::DotProduct, &a, &b);
        let manhattan_dist = compute_distance(DistanceMetric::Manhattan, &a, &b);

        assert_eq!(cosine_dist, 0.0);
        assert_eq!(euclidean_dist, 0.0);
        assert_eq!(manhattan_dist, 0.0);
        assert_eq!(dot_dist, -1.0);
    }
}
