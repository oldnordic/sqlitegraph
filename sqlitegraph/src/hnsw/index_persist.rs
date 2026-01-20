// HNSW Index Persistence Operations
//
// This module contains persistence operations for HnswIndex.
// Includes metadata save/load, vector loading, and database operations.
//
// Note: This file is included via include! macro in index.rs
// All imports are inherited from the parent module

// Note: HnswIndex is defined in the parent (index.rs)
// This file is included via include! macro, so types are available in scope

impl HnswIndex {
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
    pub fn save_metadata(&self, conn: &rusqlite::Connection) -> Result<(), crate::hnsw::errors::HnswError> {
        use crate::hnsw::errors::HnswStorageError;

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
                HnswStorageError::DatabaseError(e.to_string())
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
                HnswStorageError::DatabaseError(e.to_string())
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
    pub(crate) fn get_index_id(conn: &rusqlite::Connection, name: &str) -> Result<Option<i64>, crate::hnsw::errors::HnswError> {
        use crate::hnsw::errors::HnswStorageError;

        let id = conn
            .query_row(
                "SELECT id FROM hnsw_indexes WHERE name = ?",
                [name],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| HnswError::Storage(
                HnswStorageError::DatabaseError(e.to_string())
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
    pub fn load_metadata(conn: &rusqlite::Connection, name: &str) -> Result<Self, crate::hnsw::errors::HnswError> {
        use crate::hnsw::{distance_metric::DistanceMetric, layer::HnswLayer, multilayer::LevelDistributor, neighborhood::NeighborhoodSearch, storage::InMemoryVectorStorage};

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
            .map_err(|e| {
                let stor_err = crate::hnsw::errors::HnswStorageError::DatabaseError(e.to_string());
                crate::hnsw::errors::HnswError::Storage(stor_err)
            })?
            .ok_or_else(|| {
                let stor_err = crate::hnsw::errors::HnswStorageError::VectorNotFound(0);
                crate::hnsw::errors::HnswError::Storage(stor_err)
            })?;

        // Parse distance metric
        let distance_metric = Self::parse_distance_metric(&distance_metric_str)?;

        // Build config - use single-layer mode for safety
        let config = crate::hnsw::config::HnswConfig {
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

        // No level distributor for loaded indexes (enable_multilayer=false)
        let level_distributor = None;

        // No multi-layer manager for loaded indexes (enable_multilayer=false)
        let multi_layer_manager = None;

        Ok(Self {
            name: name.to_string(),
            config,
            layers,
            storage,
            entry_points: Vec::new(),
            vector_count,
            search_engine,
            level_distributor,
            multi_layer_manager,
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
    pub fn load_vectors_and_rebuild(&mut self, conn: &rusqlite::Connection) -> Result<(), crate::hnsw::errors::HnswError> {
        use crate::hnsw::errors::HnswStorageError;

        let index_id = Self::get_index_id(conn, &self.name)?
            .ok_or_else(|| {
                crate::hnsw::errors::HnswError::Storage(HnswStorageError::VectorNotFound(0))
            })?;

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
    pub(crate) fn load_vectors_from_db(
        conn: &rusqlite::Connection,
        index_id: i64,
    ) -> Result<Vec<(u64, Vec<f32>, Option<serde_json::Value>)>, crate::hnsw::errors::HnswError> {
        use crate::hnsw::errors::HnswStorageError;

        let mut stmt = conn
            .prepare("SELECT id, vector_data, metadata FROM hnsw_vectors WHERE index_id = ? ORDER BY id")
            .map_err(|e| {
                crate::hnsw::errors::HnswError::Storage(HnswStorageError::DatabaseError(e.to_string()))
            })?;

        let vectors = stmt
            .query_map([index_id], |row| {
                let id: i64 = row.get(0)?;
                let vector_data: Vec<u8> = row.get(1)?;
                let metadata_json: Option<String> = row.get(2)?;
                Ok((id as u64, vector_data, metadata_json))
            })
            .map_err(|e| {
                crate::hnsw::errors::HnswError::Storage(HnswStorageError::DatabaseError(
                    e.to_string(),
                ))
            })?
            .map(|row| {
                let (vector_id, vector_bytes, metadata_json) = row.map_err(|e| {
                    crate::hnsw::errors::HnswError::Storage(HnswStorageError::DatabaseError(
                        e.to_string(),
                    ))
                })?;

                // Deserialize vector
                let vector = if vector_bytes.len() % std::mem::size_of::<f32>() != 0 {
                    return Err(crate::hnsw::errors::HnswError::Storage(
                        HnswStorageError::InvalidVectorData,
                    ));
                } else {
                    bytemuck::cast_slice::<u8, f32>(&vector_bytes).to_vec()
                };

                // Parse metadata
                let metadata = metadata_json
                    .map(|s| {
                        serde_json::from_str(&s).map_err(|e| {
                            crate::hnsw::errors::HnswError::Storage(HnswStorageError::IoError(
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
    pub(crate) fn insert_vector_internal(
        &mut self,
        vector_id: u64,
        vector: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<(), crate::hnsw::errors::HnswError> {
        use crate::hnsw::errors::{HnswError, HnswIndexError};

        // Validate vector dimension
        if vector.len() != self.config.dimension {
            return Err(HnswError::Index(HnswIndexError::VectorDimensionMismatch {
                expected: self.config.dimension,
                actual: vector.len(),
            }));
        }

        // Store the vector in memory (not to database)
        self.storage.store_vector_with_id(vector_id, vector.to_vec(), metadata)?;

        // Determine insertion layer and register with multi-layer manager
        // In multi-layer mode, the manager determines the level and creates mappings
        // In single-layer mode, we use the level distributor
        let insertion_level = if let Some(manager) = &mut self.multi_layer_manager {
            // Multi-layer mode: let the manager determine the level and create mappings
            let (highest_level, _layer_assignments) = manager.insert_vector(vector_id)?;
            highest_level
        } else {
            // Single-layer mode: use level distributor
            self.determine_insertion_level()
        };

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
    pub fn load_with_vectors(conn: &rusqlite::Connection, name: &str) -> Result<Self, crate::hnsw::errors::HnswError> {
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
    pub(crate) fn parse_distance_metric(s: &str) -> Result<crate::hnsw::distance_metric::DistanceMetric, crate::hnsw::errors::HnswError> {
        use crate::hnsw::errors::HnswStorageError;

        match s {
            "cosine" => Ok(crate::hnsw::distance_metric::DistanceMetric::Cosine),
            "euclidean" => Ok(crate::hnsw::distance_metric::DistanceMetric::Euclidean),
            "dot_product" => Ok(crate::hnsw::distance_metric::DistanceMetric::DotProduct),
            "manhattan" => Ok(crate::hnsw::distance_metric::DistanceMetric::Manhattan),
            _ => Err(crate::hnsw::errors::HnswError::Storage(
                HnswStorageError::IoError(format!(
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
    pub fn list_indexes(conn: &rusqlite::Connection) -> Result<Vec<String>, crate::hnsw::errors::HnswError> {
        use crate::hnsw::errors::HnswStorageError;

        let mut stmt = conn
            .prepare("SELECT name FROM hnsw_indexes ORDER BY name")
            .map_err(|e| {
                crate::hnsw::errors::HnswError::Storage(HnswStorageError::DatabaseError(e.to_string()))
            })?;

        let index_names = stmt
            .query_map([], |row| row.get(0))
            .map_err(|e| {
                crate::hnsw::errors::HnswError::Storage(HnswStorageError::DatabaseError(e.to_string()))
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                crate::hnsw::errors::HnswError::Storage(HnswStorageError::DatabaseError(e.to_string()))
            })?;

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
    pub fn delete_index(conn: &rusqlite::Connection, name: &str) -> Result<(), crate::hnsw::errors::HnswError> {
        use crate::hnsw::errors::HnswStorageError;

        conn.execute(
            "DELETE FROM hnsw_indexes WHERE name = ?",
            [name],
        )
        .map_err(|e| {
            crate::hnsw::errors::HnswError::Storage(HnswStorageError::DatabaseError(e.to_string()))
        })?;

        Ok(())
    }
}
