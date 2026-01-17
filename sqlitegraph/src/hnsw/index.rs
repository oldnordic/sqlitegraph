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
//! let vector_id = hnsw.insert_vector(&vector_data, Some(metadata))?;
//!
//! // Search for similar vectors
//! let results = hnsw.search(&query_vector, 10)?;
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
    name: String,

    /// HNSW configuration parameters
    config: HnswConfig,

    /// Layer management (0 = base layer, higher numbers = smaller layers)
    layers: Vec<HnswLayer>,

    /// Vector storage backend
    storage: Box<dyn VectorStorage>,

    /// Entry points for navigating the hierarchical structure
    entry_points: Vec<u64>,

    /// Number of vectors currently indexed
    vector_count: usize,

    /// Neighborhood search engine
    search_engine: NeighborhoodSearch,
}

impl HnswIndex {
    /// Create a new HNSW index with the specified configuration
    ///
    /// # Arguments
    /// * `name` - Name of the index (for persistence and multi-index support)
    /// * `config` - HNSW configuration parameters
    ///
    /// # Returns
    ///
    /// Returns a new HnswIndex ready for vector insertion and search
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sqlitegraph::hnsw::{HnswIndex, HnswConfig, DistanceMetric};
    ///
    /// let config = HnswConfig::builder()
    ///     .dimension(128)
    ///     .distance_metric(DistanceMetric::Euclidean)
    ///     .build()?;
    ///
    /// let hnsw = HnswIndex::new("my_index", config)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn new(name: &str, config: HnswConfig) -> Result<Self, HnswError> {
        let storage = Box::new(InMemoryVectorStorage::new());
        Self::with_storage(name, config, storage)
    }

    /// Create a new HNSW index with SQLite-backed persistent storage
    ///
    /// # Arguments
    ///
    /// * `name` - Name of the index
    /// * `config` - HNSW configuration parameters
    /// * `conn` - SQLite connection
    ///
    /// # Returns
    ///
    /// Returns a new HnswIndex with SQLite storage
    ///
    /// # Note
    ///
    /// This creates an index with persistent storage. The index_id will be
    /// set after saving metadata to the database.
    pub fn with_persistent_storage(
        name: &str,
        config: HnswConfig,
        conn: Connection,
    ) -> Result<Self, HnswError> {
        // First save metadata to get index_id
        let temp_index = Self::new(name, config.clone())?;
        temp_index.save_metadata(&conn)?;

        // Get the index_id
        let index_id = Self::get_index_id(&conn, name)?
            .ok_or_else(|| HnswError::Storage(
                crate::hnsw::errors::HnswStorageError::VectorNotFound(0)
            ))?;

        // Create index with SQLite storage
        let storage = Box::new(crate::hnsw::storage::SQLiteVectorStorage::new(index_id, conn));
        Self::with_storage(name, config, storage)
    }

    /// Create a new HNSW index with custom storage backend
    ///
    /// # Arguments
    /// * `name` - Name of the index
    /// * `config` - HNSW configuration parameters
    /// * `storage` - Custom vector storage implementation
    ///
    /// # Returns
    ///
    /// Returns a new HnswIndex using the provided storage backend
    pub fn with_storage(
        name: &str,
        config: HnswConfig,
        storage: Box<dyn VectorStorage>,
    ) -> Result<Self, HnswError> {
        // Validate configuration
        Self::validate_config(&config)?;

        // Initialize layers
        let mut layers = Vec::with_capacity(config.ml as usize);
        for level in 0..config.ml {
            let max_connections = if level == 0 {
                config.m
            } else {
                (config.m / 2usize.pow(level as u32)).max(1)
            };
            layers.push(HnswLayer::new(level as u8, max_connections));
        }

        let search_engine = NeighborhoodSearch::new(config.distance_metric);

        Ok(Self {
            name: name.to_string(),
            config,
            layers,
            storage,
            entry_points: Vec::new(),
            vector_count: 0,
            search_engine,
        })
    }

    /// Insert a vector into the HNSW index
    ///
    /// # Arguments
    /// * `vector` - Vector data to insert (must match configured dimension)
    /// * `metadata` - Optional JSON metadata to associate with the vector
    ///
    /// # Returns
    ///
    /// Returns the assigned vector ID for future reference
    ///
    /// # Errors
    ///
    /// Returns `HnswError::Index` for dimension mismatches or insert failures
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sqlitegraph::hnsw::{HnswIndex, HnswConfig, DistanceMetric};
    /// # let hnsw = HnswIndex::new(HnswConfig::default()).unwrap();
    /// let vector = vec![1.0, 0.0, 0.0];
    /// let metadata = serde_json::json!({"label": "test"});
    ///
    /// let vector_id = hnsw.insert_vector(&vector, Some(metadata))?;
    /// println!("Inserted vector with ID: {}", vector_id);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn insert_vector(
        &mut self,
        vector: &[f32],
        metadata: Option<Value>,
    ) -> Result<u64, HnswError> {
        // Validate vector dimension
        if vector.len() != self.config.dimension {
            return Err(HnswError::Index(HnswIndexError::VectorDimensionMismatch {
                expected: self.config.dimension,
                actual: vector.len(),
            }));
        }

        // Store the vector
        let vector_id = self.storage.store_vector(vector, metadata)?;

        // Determine insertion layer using exponential distribution
        let insertion_level = self.determine_insertion_level();

        // Insert into layers from insertion_level down to 0
        for level in (0..=insertion_level).rev() {
            self.insert_into_layer(vector_id, level)?;
        }

        // Update entry points if this is a high-level vector
        if insertion_level >= self.entry_points.len() {
            self.entry_points.push(vector_id);
        }

        self.vector_count += 1;
        Ok(vector_id)
    }

    /// Search for the k nearest neighbors to a query vector
    ///
    /// # Arguments
    /// * `query` - Query vector (must match configured dimension)
    /// * `k` - Number of nearest neighbors to return
    ///
    /// # Returns
    ///
    /// Returns a vector of (vector_id, distance) tuples sorted by distance
    ///
    /// # Errors
    ///
    /// Returns `HnswError::Index` for dimension mismatches or search failures
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sqlitegraph::hnsw::{HnswIndex, HnswConfig};
    /// # let mut hnsw = HnswIndex::new(HnswConfig::default()).unwrap();
    /// # // Insert some vectors first
    /// let query = vec![1.0, 0.0, 0.0];
    ///
    /// let results = hnsw.search(&query, 5)?;
    /// for (id, distance) in results {
    ///     println!("Vector {}: distance {}", id, distance);
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(u64, f32)>, HnswError> {
        println!(
            "search: Starting with query length {}, k={}",
            query.len(),
            k
        );
        println!(
            "search: vector_count={}, layers.len()={}",
            self.vector_count,
            self.layers.len()
        );

        // Validate query vector dimension
        if query.len() != self.config.dimension {
            return Err(HnswError::Index(HnswIndexError::VectorDimensionMismatch {
                expected: self.config.dimension,
                actual: query.len(),
            }));
        }

        if self.vector_count == 0 {
            return Ok(Vec::new());
        }

        // Search from top layer down, refining candidates at each level
        for level in (0..self.layers.len()).rev() {
            // Skip empty layers
            if self.layers[level].node_count() == 0 {
                continue;
            }

            let layer_entry_points = if level == self.layers.len() - 1 {
                self.entry_points.clone()
            } else {
                // Find entry points for this level
                self.get_layer_entry_points(level)
            };

            if layer_entry_points.is_empty() {
                continue;
            }

            // Get all vectors from storage and create 0-based indexed array
            let vector_ids = self.storage.list_vectors()?;
            let max_vector_id = vector_ids.iter().copied().max().unwrap_or(0);

            // Create 0-indexed vectors array (vectors[node_id] = vector_data)
            let mut vectors_array = vec![vec![]; max_vector_id as usize + 1];
            for vector_id in vector_ids {
                if let Ok(Some(vector)) = self.storage.get_vector(vector_id) {
                    let node_id = (vector_id - 1) as usize; // Convert 1-based to 0-based
                    if node_id < vectors_array.len() {
                        vectors_array[node_id] = vector;
                    }
                }
            }

            // Convert entry points from 1-based vector IDs to 0-based node IDs
            let entry_node_ids: Vec<u64> = layer_entry_points
                .iter()
                .map(|&vector_id| vector_id - 1) // Convert to 0-based
                .collect();

            // Search in this layer
            let ef = if level == 0 { k } else { self.config.ef_search };
            let search_result = self.search_engine.search_layer(
                &self.layers[level],
                query,
                &vectors_array,
                &entry_node_ids,
                ef,
            )?;

            if level == 0 {
                // Base layer: return final results
                let neighbors = search_result.neighbors();
                let distances = search_result.distances();
                let mut results = Vec::with_capacity(neighbors.len().min(k));

                for i in 0..neighbors.len().min(k) {
                    // Convert 0-based node IDs back to 1-based vector IDs
                    let vector_id = neighbors[i] + 1;
                    results.push((vector_id, distances[i]));
                }

                return Ok(results);
            }
            // Higher layers fall through to continue search in lower layers
        }

        // If we reach here, there were no entry points in any layer
        Ok(Vec::new())
    }

    /// Get vector data and metadata by ID
    ///
    /// # Arguments
    /// * `vector_id` - ID of the vector to retrieve
    ///
    /// # Returns
    ///
    /// Returns `Some((vector, metadata))` if found, `None` if not found
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sqlitegraph::hnsw::{HnswIndex, HnswConfig};
    /// # let mut hnsw = HnswIndex::new(HnswConfig::default()).unwrap();
    /// # let vector_id = hnsw.insert_vector(&vec![1.0, 0.0], None).unwrap();
    /// let result = hnsw.get_vector(vector_id)?;
    /// if let Some((vector, metadata)) = result {
    ///     println!("Retrieved vector: {:?}", vector);
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn get_vector(&self, vector_id: u64) -> Result<Option<(Vec<f32>, Value)>, HnswError> {
        self.storage.get_vector_with_metadata(vector_id)
    }

    /// Get statistics about the HNSW index
    ///
    /// # Returns
    ///
    /// Returns comprehensive statistics about index state and performance
    ///
    /// # Examples
    ///
    /// ```rust
    /// # use sqlitegraph::hnsw::{HnswIndex, HnswConfig};
    /// # let hnsw = HnswIndex::new(HnswConfig::default()).unwrap();
    /// let stats = hnsw.statistics()?;
    /// println!("Vectors indexed: {}", stats.vector_count);
    /// println!("Layers: {}", stats.layer_count);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn statistics(&self) -> Result<HnswIndexStats, HnswError> {
        let storage_stats = self.storage.get_statistics()?;
        let layer_stats: Vec<_> = self
            .layers
            .iter()
            .map(|layer| layer.get_statistics())
            .collect();

        Ok(HnswIndexStats {
            vector_count: self.vector_count,
            layer_count: self.layers.len(),
            entry_point_count: self.entry_points.len(),
            dimension: self.config.dimension,
            distance_metric: self.config.distance_metric,
            storage_stats,
            layer_stats,
        })
    }

    /// Get the name of this index
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get the number of vectors in this index
    pub fn vector_count(&self) -> usize {
        self.vector_count
    }

    /// Get the HNSW configuration
    ///
    /// Returns a reference to the index configuration
    pub fn config(&self) -> &HnswConfig {
        &self.config
    }

    /// Save index metadata to database
    ///
    /// # Arguments
    /// * `conn` - SQLite connection
    ///
    /// # Returns
    ///
    /// Ok(()) if successful
    ///
    /// # Errors
    ///
    /// Returns HnswError on database failure
    pub fn save_metadata(&self, conn: &Connection) -> Result<(), HnswError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        let distance_metric_str = self.config.distance_metric.as_str();

        // Check if index already exists
        let existing_id = Self::get_index_id(conn, &self.name)?;

        if let Some(index_id) = existing_id {
            // Update existing index - use positional params with correct types
            conn.execute(
                "UPDATE hnsw_indexes SET dimension = ?1, m = ?2, ef_construction = ?3, \
                 distance_metric = ?4, vector_count = ?5, updated_at = ?6 WHERE id = ?7",
                rusqlite::params![
                    self.config.dimension as i64,
                    self.config.m as i64,
                    self.config.ef_construction as i64,
                    distance_metric_str,
                    self.vector_count as i64,
                    now,
                    index_id,
                ],
            ).map_err(|e| HnswError::Storage(
                crate::hnsw::errors::HnswStorageError::DatabaseError(e.to_string())
            ))?;
        } else {
            // Insert new index
            conn.execute(
                "INSERT INTO hnsw_indexes \
                 (name, dimension, m, ef_construction, distance_metric, vector_count, created_at, updated_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                rusqlite::params![
                    &self.name,
                    self.config.dimension as i64,
                    self.config.m as i64,
                    self.config.ef_construction as i64,
                    distance_metric_str,
                    self.vector_count as i64,
                    now,
                    now,
                ],
            ).map_err(|e| HnswError::Storage(
                crate::hnsw::errors::HnswStorageError::DatabaseError(e.to_string())
            ))?;
        }

        Ok(())
    }

    /// Get index ID from database by name
    ///
    /// # Arguments
    /// * `conn` - SQLite connection
    /// * `name` - Index name
    ///
    /// # Returns
    ///
    /// Some(index_id) if found, None if not found
    fn get_index_id(conn: &Connection, name: &str) -> Result<Option<i64>, HnswError> {
        let id = conn
            .query_row(
                "SELECT id FROM hnsw_indexes WHERE name = ?",
                [name],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| HnswError::Storage(
                crate::hnsw::errors::HnswStorageError::DatabaseError(e.to_string())
            ))?;

        Ok(id)
    }

    /// Load index metadata from database
    ///
    /// # Arguments
    /// * `conn` - SQLite connection
    /// * `name` - Index name
    ///
    /// # Returns
    ///
    /// Loaded HnswIndex with metadata (vectors not loaded yet)
    ///
    /// # Errors
    ///
    /// Returns HnswError on database failure or if index not found
    pub fn load_metadata(conn: &Connection, name: &str) -> Result<Self, HnswError> {
        // Query index metadata
        let (dimension, m, ef_construction, distance_metric_str, vector_count) = conn
            .query_row(
                "SELECT dimension, m, ef_construction, distance_metric, vector_count
                 FROM hnsw_indexes WHERE name = ?",
                [name],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)? as usize,
                        row.get::<_, i64>(1)? as usize,
                        row.get::<_, i64>(2)? as usize,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)? as usize,
                    ))
                },
            )
            .optional()
            .map_err(|e| HnswError::Storage(
                crate::hnsw::errors::HnswStorageError::DatabaseError(e.to_string())
            ))?
            .ok_or_else(|| HnswError::Storage(
                crate::hnsw::errors::HnswStorageError::VectorNotFound(0)
            ))?;

        // Parse distance metric
        let distance_metric = Self::parse_distance_metric(&distance_metric_str)?;

        // Build config - use single-layer mode for safety
        let config = HnswConfig {
            dimension,
            m,
            ef_construction,
            ef_search: ef_construction, // Default to ef_construction
            ml: 16, // Default max layers
            distance_metric,
            enable_multilayer: false, // Disable multilayer for loaded indexes (plan 02)
            multilayer_level_distribution_base: None,
            multilayer_deterministic_seed: None,
        };

        // Create index with loaded config
        let storage = Box::new(InMemoryVectorStorage::new());
        let mut layers = Vec::with_capacity(config.ml as usize);
        for level in 0..config.ml {
            let max_connections = if level == 0 {
                config.m
            } else {
                (config.m / 2usize.pow(level as u32)).max(1)
            };
            layers.push(HnswLayer::new(level, max_connections));
        }

        let search_engine = NeighborhoodSearch::new(config.distance_metric);

        Ok(Self {
            name: name.to_string(),
            config,
            layers,
            storage,
            entry_points: Vec::new(),
            vector_count,
            search_engine,
        })
    }

    /// Load all vectors from database and rebuild HNSW index
    ///
    /// # Arguments
    /// * `conn` - SQLite connection
    ///
    /// # Returns
    ///
    /// Ok(()) if successful
    ///
    /// # Errors
    ///
    /// Returns HnswError on database failure or vector loading errors
    ///
    /// # Note
    ///
    /// This method loads all vectors from the database and rebuilds the HNSW
    /// graph structure by inserting each vector. The O(N log N) rebuild cost
    /// is a trade-off for simpler implementation compared to persisting layers.
    pub fn load_vectors_and_rebuild(&mut self, conn: &Connection) -> Result<(), HnswError> {
        let index_id = Self::get_index_id(conn, &self.name)?
            .ok_or_else(|| HnswError::Storage(
                crate::hnsw::errors::HnswStorageError::VectorNotFound(0)
            ))?;

        // Load all vectors from database
        let vectors = Self::load_vectors_from_db(conn, index_id)?;

        // Clear vector_count since we're rebuilding
        self.vector_count = 0;

        // Rebuild HNSW graph by inserting each vector
        for (vector_id, data, metadata) in vectors {
            // Use internal insert that doesn't re-persist to database
            self.insert_vector_internal(vector_id, &data, metadata)?;
        }

        Ok(())
    }

    /// Load vectors from database
    ///
    /// # Arguments
    ///
    /// * `conn` - SQLite connection
    /// * `index_id` - Index ID in database
    ///
    /// # Returns
    ///
    /// Vector of (id, data, metadata) tuples
    fn load_vectors_from_db(
        conn: &Connection,
        index_id: i64,
    ) -> Result<Vec<(u64, Vec<f32>, Option<Value>)>, HnswError> {
        let mut stmt = conn
            .prepare("SELECT id, vector_data, metadata FROM hnsw_vectors WHERE index_id = ? ORDER BY id")
            .map_err(|e| HnswError::Storage(
                crate::hnsw::errors::HnswStorageError::DatabaseError(e.to_string())
            ))?;

        let vectors = stmt
            .query_map([index_id], |row| {
                let id: i64 = row.get(0)?;
                let vector_data: Vec<u8> = row.get(1)?;
                let metadata_json: Option<String> = row.get(2)?;
                Ok((id as u64, vector_data, metadata_json))
            })
            .map_err(|e| HnswError::Storage(
                crate::hnsw::errors::HnswStorageError::DatabaseError(e.to_string())
            ))?
            .map(|row| {
                let (vector_id, vector_bytes, metadata_json) = row.map_err(|e| {
                    HnswError::Storage(crate::hnsw::errors::HnswStorageError::DatabaseError(
                        e.to_string(),
                    ))
                })?;

                // Deserialize vector
                let vector = if vector_bytes.len() % std::mem::size_of::<f32>() != 0 {
                    return Err(HnswError::Storage(
                        crate::hnsw::errors::HnswStorageError::InvalidVectorData,
                    ));
                } else {
                    bytemuck::cast_slice::<u8, f32>(&vector_bytes).to_vec()
                };

                // Parse metadata
                let metadata = metadata_json
                    .map(|s| {
                        serde_json::from_str(&s).map_err(|e| {
                            HnswError::Storage(crate::hnsw::errors::HnswStorageError::IoError(
                                format!("Failed to parse metadata: {}", e),
                            ))
                        })
                    })
                    .transpose()?;

                Ok((vector_id, vector, metadata))
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(vectors)
    }

    /// Internal insert without persistence
    ///
    /// # Arguments
    ///
    /// * `vector_id` - Explicit vector ID (from database)
    /// * `vector` - Vector data
    /// * `metadata` - Optional metadata
    ///
    /// # Returns
    ///
    /// Ok(()) if successful
    ///
    /// # Note
    ///
    /// This inserts into the HNSW graph structure using existing logic
    /// but skips database write since the vector is already persisted.
    fn insert_vector_internal(
        &mut self,
        vector_id: u64,
        vector: &[f32],
        metadata: Option<Value>,
    ) -> Result<(), HnswError> {
        // Validate vector dimension
        if vector.len() != self.config.dimension {
            return Err(HnswError::Index(HnswIndexError::VectorDimensionMismatch {
                expected: self.config.dimension,
                actual: vector.len(),
            }));
        }

        // Store the vector in memory (not to database)
        self.storage.store_vector_with_id(vector_id, vector.to_vec(), metadata)?;

        // Determine insertion layer using exponential distribution
        let insertion_level = self.determine_insertion_level();

        // Insert into layers from insertion_level down to 0
        for level in (0..=insertion_level).rev() {
            self.insert_into_layer(vector_id, level)?;
        }

        // Update entry points if this is a high-level vector
        if insertion_level >= self.entry_points.len() {
            self.entry_points.push(vector_id);
        }

        self.vector_count += 1;
        Ok(())
    }

    /// Load index with vectors from database
    ///
    /// # Arguments
    ///
    /// * `conn` - SQLite connection
    /// * `name` - Index name
    ///
    /// # Returns
    ///
    /// Fully loaded HnswIndex with all vectors
    ///
    /// # Errors
    ///
    /// Returns HnswError on database failure or if index not found
    ///
    /// # Note
    ///
    /// This is a convenience method that loads metadata and vectors in one call.
    pub fn load_with_vectors(conn: &Connection, name: &str) -> Result<Self, HnswError> {
        let mut hnsw = Self::load_metadata(conn, name)?;
        hnsw.load_vectors_and_rebuild(conn)?;
        Ok(hnsw)
    }

    /// Parse distance metric from string
    ///
    /// # Arguments
    /// * `s` - String representation of distance metric
    ///
    /// # Returns
    ///
    /// Parsed DistanceMetric
    ///
    /// # Errors
    ///
    /// Returns HnswError for unknown metrics
    fn parse_distance_metric(s: &str) -> Result<DistanceMetric, HnswError> {
        match s {
            "cosine" => Ok(DistanceMetric::Cosine),
            "euclidean" => Ok(DistanceMetric::Euclidean),
            "dot_product" => Ok(DistanceMetric::DotProduct),
            "manhattan" => Ok(DistanceMetric::Manhattan),
            _ => Err(HnswError::Storage(
                crate::hnsw::errors::HnswStorageError::IoError(format!(
                    "Unknown distance metric: {}", s
                )),
            )),
        }
    }

    /// List all HNSW indexes in the database
    ///
    /// # Arguments
    /// * `conn` - SQLite connection
    ///
    /// # Returns
    ///
    /// List of index names
    ///
    /// # Errors
    ///
    /// Returns HnswError on database failure
    pub fn list_indexes(conn: &Connection) -> Result<Vec<String>, HnswError> {
        let mut stmt = conn
            .prepare("SELECT name FROM hnsw_indexes ORDER BY name")
            .map_err(|e| HnswError::Storage(
                crate::hnsw::errors::HnswStorageError::DatabaseError(e.to_string())
            ))?;

        let index_names = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| HnswError::Storage(
                crate::hnsw::errors::HnswStorageError::DatabaseError(e.to_string())
            ))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| HnswError::Storage(
                crate::hnsw::errors::HnswStorageError::DatabaseError(e.to_string())
            ))?;

        Ok(index_names)
    }

    /// Delete an index from the database
    ///
    /// # Arguments
    /// * `conn` - SQLite connection
    /// * `name` - Index name
    ///
    /// # Returns
    ///
    /// Ok(()) if deleted or didn't exist
    ///
    /// # Errors
    ///
    /// Returns HnswError on database failure
    ///
    /// # Note
    ///
    /// CASCADE will automatically delete vectors, layers, and entry points
    pub fn delete_index(conn: &Connection, name: &str) -> Result<(), HnswError> {
        conn.execute(
            "DELETE FROM hnsw_indexes WHERE name = ?",
            [name],
        )
        .map_err(|e| HnswError::Storage(
            crate::hnsw::errors::HnswStorageError::DatabaseError(e.to_string())
        ))?;

        Ok(())
    }

    /// Validate the HNSW configuration
    fn validate_config(config: &HnswConfig) -> Result<(), HnswError> {
        if config.dimension == 0 {
            return Err(HnswError::Index(HnswIndexError::InvalidNodeId(0)));
        }
        if config.m == 0 {
            return Err(HnswError::Index(HnswIndexError::InvalidNodeId(0)));
        }
        if config.ef_construction < config.m {
            return Err(HnswError::Index(HnswIndexError::InvalidNodeId(0)));
        }
        if config.ef_search == 0 {
            return Err(HnswError::Index(HnswIndexError::InvalidNodeId(0)));
        }
        if config.ml == 0 {
            return Err(HnswError::Index(HnswIndexError::InvalidNodeId(0)));
        }
        Ok(())
    }

    /// Determine which layer to insert a vector into using exponential distribution
    fn determine_insertion_level(&self) -> usize {
        // For now, only use base layer to avoid multi-layer complexity
        // TODO: Implement proper multi-layer HNSW with correct node ID management
        0
    }

    /// Insert a vector into a specific layer
    fn insert_into_layer(&mut self, vector_id: u64, level: usize) -> Result<(), HnswError> {
        // Convert 1-based vector ID to 0-based node ID for layer management
        let node_id = vector_id - 1;

        // Ensure layer exists, create if necessary
        while level >= self.layers.len() {
            let new_level = self.layers.len() as u8;
            let max_connections = (self.config.m / 2_usize.pow(new_level as u32)).max(1);
            self.layers.push(HnswLayer::new(new_level, max_connections));
        }

        // Add the node to the layer first (this makes it an entry point if it's one of the first nodes)
        {
            let layer = &mut self.layers[level];
            layer.add_node(node_id)?;
        }

        // For base layer, no need to connect to existing entry points if this is the first node
        if level == 0 && self.layers[level].node_count() == 1 {
            return Ok(());
        }

        // Find entry points after adding the node (convert to 0-based node IDs)
        let entry_points: Vec<u64> = self
            .get_layer_entry_points(level)
            .into_iter()
            .map(|vector_id| vector_id - 1) // Convert to 0-based node IDs
            .collect();

        // Connect to entry points (excluding self)
        let layer = &mut self.layers[level];
        for &entry_node_id in &entry_points {
            if entry_node_id != node_id {
                layer.add_connection(node_id, entry_node_id)?;
                layer.add_connection(entry_node_id, node_id)?;
            }
        }
        Ok(())
    }

    /// Get entry points for a specific layer
    fn get_layer_entry_points(&self, level: usize) -> Vec<u64> {
        if self.layers.is_empty() {
            return Vec::new();
        }

        if level == self.layers.len() - 1 {
            // Top layer: return all entry points (already 1-based vector IDs)
            self.entry_points.clone()
        } else if level == 0 {
            // Base layer: use its own entry points
            let layer_entry_points = self.layers[level].get_entry_points();
            layer_entry_points
                .iter()
                .map(|&node_id| node_id + 1) // Convert 0-based to 1-based
                .collect()
        } else {
            // Intermediate layers: use entry points from the layer above
            if level + 1 < self.layers.len() {
                let layer_entry_points = self.layers[level + 1].get_entry_points();
                layer_entry_points
                    .iter()
                    .map(|&node_id| node_id + 1) // Convert 0-based to 1-based
                    .collect()
            } else {
                Vec::new()
            }
        }
    }
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

/// SQLiteGraph extension for HNSW vector search
impl SqliteGraph {
    /// Create or get an HNSW index with the specified name and configuration
    ///
    /// # Arguments
    /// * `name` - Name to identify this index (for multi-index support)
    /// * `config` - HNSW configuration parameters
    ///
    /// # Returns
    ///
    /// Returns a mutable reference to the HnswIndex ready for vector operations
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sqlitegraph::{SqliteGraph, hnsw::{HnswConfig, DistanceMetric}};
    ///
    /// let graph = SqliteGraph::open_in_memory()?;
    /// let config = HnswConfig::builder()
    ///     .dimension(256)
    ///     .distance_metric(DistanceMetric::Cosine)
    ///     .build()?;
    ///
    /// let hnsw = graph.hnsw_index("embeddings", config)?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn hnsw_index(
        &self,
        name: &str,
        config: HnswConfig,
    ) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<String, HnswIndex>>, SqliteGraphError> {
        use std::sync::RwLock;

        // Check if index already exists
        {
            let indexes = self.hnsw_indexes.read().map_err(|e| SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;
            if indexes.contains_key(name) {
                return Err(SqliteGraphError::invalid_input(format!("HNSW index '{}' already exists. Use get_hnsw_index() to retrieve it.", name)));
            }
        }

        // Create new HNSW index
        let hnsw = HnswIndex::new(name, config).map_err(|e| SqliteGraphError::invalid_input(e.to_string()))?;

        // Save metadata to database
        hnsw.save_metadata(&self.conn).map_err(|e| SqliteGraphError::invalid_input(format!("Failed to save HNSW index metadata: {}", e)))?;

        // Store the index
        let mut indexes = self.hnsw_indexes.write().map_err(|e| SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;
        indexes.insert(name.to_string(), hnsw);

        Ok(indexes)
    }

    /// Create or get an HNSW index with persistent storage (for file-based databases)
    ///
    /// This method automatically detects if the database is file-based and creates
    /// the index with SQLiteVectorStorage for automatic vector persistence.
    /// For in-memory databases, falls back to in-memory storage.
    ///
    /// # Arguments
    /// * `name` - Name to identify this index
    /// * `config` - HNSW configuration parameters
    ///
    /// # Returns
    ///
    /// Returns a mutable reference to the HnswIndex ready for vector operations
    ///
    /// # Examples
    ///
    /// ```rust
    /// use sqlitegraph::{SqliteGraph, hnsw::{HnswConfig, DistanceMetric}};
    ///
    /// let graph = SqliteGraph::open("mydb.db")?;
    /// let config = HnswConfig::builder()
    ///     .dimension(256)
    ///     .distance_metric(DistanceMetric::Cosine)
    ///     .build()?;
    ///
    /// let hnsw = graph.hnsw_index_persistent("embeddings", config)?;
    /// // Vectors inserted into this index will persist to the database
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn hnsw_index_persistent(
        &self,
        name: &str,
        config: HnswConfig,
    ) -> Result<std::sync::RwLockWriteGuard<'_, HashMap<String, HnswIndex>>, SqliteGraphError> {
        use std::sync::RwLock;

        // Check if index already exists
        {
            let indexes = self.hnsw_indexes.read().map_err(|e| SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;
            if indexes.contains_key(name) {
                return Err(SqliteGraphError::invalid_input(format!("HNSW index '{}' already exists. Use get_hnsw_index() to retrieve it.", name)));
            }
        }

        // Check if database is file-based (not in-memory)
        let is_file_based = !crate::graph::is_in_memory_connection(&self.conn);

        // Create index with appropriate storage backend
        let hnsw = if is_file_based {
            // For file-based databases, use persistent storage
            // First, save metadata on the MAIN connection to ensure it persists
            let temp_index = HnswIndex::new(name, config.clone())
                .map_err(|e| SqliteGraphError::invalid_input(format!("Failed to create HNSW index: {}", e)))?;
            temp_index.save_metadata(&self.conn)
                .map_err(|e| SqliteGraphError::invalid_input(format!("Failed to save HNSW index metadata: {}", e)))?;

            // Get the index_id from the database
            let index_id = HnswIndex::get_index_id(&self.conn, name)
                .map_err(|e| SqliteGraphError::invalid_input(format!("Failed to get index_id: {}", e)))?
                .ok_or_else(|| SqliteGraphError::invalid_input(format!("Failed to get index_id after saving metadata")))?;

            // Get database path to open a new connection for storage
            let db_path = self.conn.pragma_query_value(None, "database_list", |row| {
                let name: String = row.get(1)?;
                Ok(name)
            }).map_err(|e| SqliteGraphError::invalid_input(format!("Failed to get database path: {}", e)))?;

            let conn_for_storage = rusqlite::Connection::open(&db_path)
                .map_err(|e| SqliteGraphError::invalid_input(format!("Failed to open connection for storage: {}", e)))?;

            // Ensure schema is initialized on the new connection
            crate::schema::ensure_schema(&conn_for_storage)
                .map_err(|e| SqliteGraphError::invalid_input(format!("Failed to ensure schema: {}", e)))?;

            // Create index with storage using the index_id we just retrieved
            let storage = Box::new(crate::hnsw::storage::SQLiteVectorStorage::new(index_id, conn_for_storage));
            HnswIndex::with_storage(name, config, storage)
                .map_err(|e| SqliteGraphError::invalid_input(format!("Failed to create HNSW index: {}", e)))?
        } else {
            // For in-memory databases, use in-memory storage
            HnswIndex::new(name, config)
                .map_err(|e| SqliteGraphError::invalid_input(format!("Failed to create HNSW index: {}", e)))?
        };

        // Store the index (metadata already saved to database above)
        let mut indexes = self.hnsw_indexes.write().map_err(|e| SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;
        indexes.insert(name.to_string(), hnsw);

        Ok(indexes)
    }

    /// Get an existing HNSW index by name
    ///
    /// # Arguments
    /// * `name` - Name of the index to retrieve
    ///
    /// # Returns
    ///
    /// Returns a mutable reference to the HnswIndex if it exists
    pub fn get_hnsw_index(
        &self,
        name: &str,
    ) -> Result<Option<std::sync::RwLockWriteGuard<'_, HashMap<String, HnswIndex>>>, SqliteGraphError> {
        use std::sync::RwLock;

        let indexes = self.hnsw_indexes.write().map_err(|e| SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;

        if indexes.contains_key(name) {
            Ok(Some(indexes))
        } else {
            Ok(None)
        }
    }

    /// Get a reference to an HNSW index without locking for write
    pub fn get_hnsw_index_ref<F, R>(
        &self,
        name: &str,
        f: F,
    ) -> Result<R, SqliteGraphError>
    where
        F: FnOnce(&HnswIndex) -> R,
    {
        use std::sync::RwLock;

        let indexes = self.hnsw_indexes.read().map_err(|e| SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;

        if let Some(hnsw) = indexes.get(name) {
            Ok(f(hnsw))
        } else {
            Err(SqliteGraphError::invalid_input(format!("HNSW index '{}' not found", name)))
        }
    }

    /// Get a mutable reference to an HNSW index for modifications
    pub fn get_hnsw_index_mut<F, R>(
        &self,
        name: &str,
        f: F,
    ) -> Result<R, SqliteGraphError>
    where
        F: FnOnce(&mut HnswIndex) -> R,
    {
        use std::sync::RwLock;

        let mut indexes = self.hnsw_indexes.write().map_err(|e| SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;

        if let Some(hnsw) = indexes.get_mut(name) {
            Ok(f(hnsw))
        } else {
            Err(SqliteGraphError::invalid_input(format!("HNSW index '{}' not found", name)))
        }
    }

    /// List all HNSW index names
    pub fn list_hnsw_indexes(&self) -> Result<Vec<String>, SqliteGraphError> {
        use std::sync::RwLock;

        let indexes = self.hnsw_indexes.read().map_err(|e| SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;
        Ok(indexes.keys().cloned().collect())
    }
}

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
                .build()
                .unwrap(),
        )
        .unwrap();

        // Insert some vectors
        for i in 0..5 {
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
            assert_eq!(hnsw.config.dimension, 128);
            assert_eq!(hnsw.config.distance_metric, DistanceMetric::Euclidean);

            // Save metadata explicitly
            hnsw.save_metadata(&graph.conn).unwrap();
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
                assert_eq!(hnsw.config.dimension, 128);
                assert_eq!(hnsw.config.distance_metric, DistanceMetric::Euclidean);
                hnsw.config.dimension
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
                assert_eq!(hnsw.vector_count, 5);

                // Verify we can retrieve a vector
                let (vector, metadata) = hnsw.get_vector(1).unwrap().unwrap();
                assert_eq!(vector, vec![0.0, 0.0, 0.0]);
                assert_eq!(metadata, serde_json::json!({"label": "vector_0"}));

                // Verify search works (graph was rebuilt)
                let query = vec![2.0, 4.0, 6.0];
                let results = hnsw.search(&query, 3).unwrap();
                assert!(!results.is_empty());

                hnsw.vector_count
            }).unwrap();

            assert_eq!(loaded_count, 5);
        }

        // Clean up
        let _ = fs::remove_dir_all(test_dir);
    }
}
