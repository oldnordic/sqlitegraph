// HNSW Index Public API
//
// This module contains the public API methods for HnswIndex.
// Includes constructors, vector insertion, search, and query operations.
//
// Note: This file is included via include! macro in index.rs
// All imports are inherited from the parent module

// Note: HnswIndex and HnswIndexStats are defined in the parent (index.rs)
// This file is included via include! macro, so types are available in scope

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
    pub fn new(name: &str, config: crate::hnsw::config::HnswConfig) -> Result<Self, crate::hnsw::errors::HnswError> {
        let storage = Box::new(crate::hnsw::storage::InMemoryVectorStorage::new());
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
        config: crate::hnsw::config::HnswConfig,
        conn: rusqlite::Connection,
    ) -> Result<Self, crate::hnsw::errors::HnswError> {
        // First save metadata to get index_id
        let temp_index = Self::new(name, config.clone())?;
        temp_index.save_metadata(&conn)?;

        // Get the index_id
        let index_id = Self::get_index_id(&conn, name)?
            .ok_or_else(|| crate::hnsw::errors::HnswError::Storage(
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
        config: crate::hnsw::config::HnswConfig,
        storage: Box<dyn crate::hnsw::storage::VectorStorage>,
    ) -> Result<Self, crate::hnsw::errors::HnswError> {
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
            layers.push(crate::hnsw::layer::HnswLayer::new(level as u8, max_connections));
        }

        let search_engine = crate::hnsw::neighborhood::NeighborhoodSearch::new(config.distance_metric);

        // Initialize level distributor for multi-layer mode
        let level_distributor = if config.enable_multilayer {
            let seed = config.multilayer_deterministic_seed.unwrap_or(42);
            let base_m = config.multilayer_level_distribution_base.unwrap_or(config.m) as f64;
            Some(crate::hnsw::multilayer::LevelDistributor::new(base_m, config.ml as usize).with_seed(seed))
        } else {
            None
        };

        // Initialize multi-layer manager for tracking layer assignments
        let multi_layer_manager = if config.enable_multilayer {
            Some(crate::hnsw::multilayer::MultiLayerNodeManager::new(config.clone()).ok())
        } else {
            None
        }.flatten();

        Ok(Self {
            name: name.to_string(),
            config,
            layers,
            storage,
            entry_points: Vec::new(),
            vector_count: 0,
            search_engine,
            level_distributor,
            multi_layer_manager,
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
        metadata: Option<serde_json::Value>,
    ) -> Result<u64, crate::hnsw::errors::HnswError> {
        use crate::hnsw::errors::{HnswError, HnswIndexError};

        // Validate vector dimension
        if vector.len() != self.config.dimension {
            return Err(HnswError::Index(HnswIndexError::VectorDimensionMismatch {
                expected: self.config.dimension,
                actual: vector.len(),
            }));
        }

        // Store the vector
        let vector_id = self.storage.store_vector(vector, metadata)?;

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
        // In multi-layer mode, this uses the LayerMappings created above
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
    pub fn search(&self, query: &[f32], k: usize) -> Result<Vec<(u64, f32)>, crate::hnsw::errors::HnswError> {
        use crate::hnsw::errors::{HnswError, HnswIndexError};

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

        // Load vectors once for all layers
        let vectors_array = self.load_vectors_as_array()?;

        // Start from top layer entry point
        let mut entry_point = *self.entry_points.last()
            .ok_or(HnswError::Index(HnswIndexError::IndexNotInitialized))?;

        // Greedy descent through higher layers (k=1 for greedy)
        // Start from the top layer and go down to layer 1
        for level in (1..self.layers.len()).rev() {
            if self.layers[level].node_count() == 0 {
                continue;
            }

            let local_id = self.get_local_id_for_layer(entry_point, level)?;
            let result = self.search_engine.search_layer(
                &self.layers[level],
                query,
                &vectors_array,
                &[local_id],
                1, // k=1 for greedy descent
            )?;

            if !result.neighbors().is_empty() {
                entry_point = self.get_global_id_for_layer(level, result.neighbors()[0])?;
            }
        }

        // Layer 0: Full ef-search
        let local_entry = self.get_local_id_for_layer(entry_point, 0)?;
        let result = self.search_engine.search_layer(
            &self.layers[0],
            query,
            &vectors_array,
            &[local_entry],
            self.config.ef_search.max(k),
        )?;

        // Convert results to 1-based vector IDs
        let results: Vec<(u64, f32)> = result.neighbors()
            .iter()
            .zip(result.distances().iter())
            .map(|(&local_id, &dist)| (local_id + 1, dist))
            .take(k)
            .collect();

        Ok(results)
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
    pub fn get_vector(&self, vector_id: u64) -> Result<Option<(Vec<f32>, serde_json::Value)>, crate::hnsw::errors::HnswError> {
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
    pub fn statistics(&self) -> Result<HnswIndexStats, crate::hnsw::errors::HnswError> {
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
    pub fn config(&self) -> &crate::hnsw::config::HnswConfig {
        &self.config
    }

    #[cfg(test)]
    /// Check if level distributor is initialized (test-only)
    pub fn has_level_distributor(&self) -> bool {
        self.level_distributor.is_some()
    }
}

/// SQLiteGraph extension for HNSW vector search
impl crate::SqliteGraph {
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
        config: crate::hnsw::config::HnswConfig,
    ) -> Result<std::sync::RwLockWriteGuard<'_, std::collections::HashMap<String, HnswIndex>>, crate::SqliteGraphError> {
        use std::sync::RwLock;
        use std::collections::HashMap;

        // Check if index already exists
        {
            let indexes = self.hnsw_indexes.read().map_err(|e| crate::SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;
            if indexes.contains_key(name) {
                return Err(crate::SqliteGraphError::invalid_input(format!("HNSW index '{}' already exists. Use get_hnsw_index() to retrieve it.", name)));
            }
        }

        // Create new HNSW index
        let hnsw = HnswIndex::new(name, config).map_err(|e| crate::SqliteGraphError::invalid_input(e.to_string()))?;

        // Save metadata to database
        let conn = self.connection();
        let conn_ref = conn.underlying();
        hnsw.save_metadata(conn_ref).map_err(|e| crate::SqliteGraphError::invalid_input(format!("Failed to save HNSW index metadata: {}", e)))?;

        // Store the index
        let mut indexes = self.hnsw_indexes.write().map_err(|e| crate::SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;
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
        config: crate::hnsw::config::HnswConfig,
    ) -> Result<std::sync::RwLockWriteGuard<'_, std::collections::HashMap<String, HnswIndex>>, crate::SqliteGraphError> {
        use std::sync::RwLock;
        use std::collections::HashMap;

        // Check if index already exists
        {
            let indexes = self.hnsw_indexes.read().map_err(|e| crate::SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;
            if indexes.contains_key(name) {
                return Err(crate::SqliteGraphError::invalid_input(format!("HNSW index '{}' already exists. Use get_hnsw_index() to retrieve it.", name)));
            }
        }

        // Check if database is file-based (not in-memory)
        let is_file_based = !self.pool.is_in_memory();

        // Create index with appropriate storage backend
        let hnsw = if is_file_based {
            // For file-based databases, use persistent storage
            // Get a connection from the pool for metadata operations
            let conn = self.connection();
            let conn_ref = conn.underlying();

            // First, save metadata to ensure it persists
            let temp_index = HnswIndex::new(name, config.clone())
                .map_err(|e| crate::SqliteGraphError::invalid_input(format!("Failed to create HNSW index: {}", e)))?;
            temp_index.save_metadata(conn_ref)
                .map_err(|e| crate::SqliteGraphError::invalid_input(format!("Failed to save HNSW index metadata: {}", e)))?;

            // Get the index_id from the database
            let index_id = HnswIndex::get_index_id(conn_ref, name)
                .map_err(|e| crate::SqliteGraphError::invalid_input(format!("Failed to get index_id: {}", e)))?
                .ok_or_else(|| crate::SqliteGraphError::invalid_input(format!("Failed to get index_id after saving metadata")))?;

            // Get database path to open a new connection for storage
            let db_path = conn_ref.pragma_query_value(None, "database_list", |row| {
                let name: String = row.get(1)?;
                Ok(name)
            }).map_err(|e| crate::SqliteGraphError::invalid_input(format!("Failed to get database path: {}", e)))?;

            let conn_for_storage = rusqlite::Connection::open(&db_path)
                .map_err(|e| crate::SqliteGraphError::invalid_input(format!("Failed to open connection for storage: {}", e)))?;

            // Ensure schema is initialized on the new connection
            crate::schema::ensure_schema(&conn_for_storage)
                .map_err(|e| crate::SqliteGraphError::invalid_input(format!("Failed to ensure schema: {}", e)))?;

            // Create index with storage using the index_id we just retrieved
            let storage = Box::new(crate::hnsw::storage::SQLiteVectorStorage::new(index_id, conn_for_storage));
            HnswIndex::with_storage(name, config, storage)
                .map_err(|e| crate::SqliteGraphError::invalid_input(format!("Failed to create HNSW index: {}", e)))?
        } else {
            // For in-memory databases, use in-memory storage
            HnswIndex::new(name, config)
                .map_err(|e| crate::SqliteGraphError::invalid_input(format!("Failed to create HNSW index: {}", e)))?
        };

        // Store the index (metadata already saved to database above)
        let mut indexes = self.hnsw_indexes.write().map_err(|e| crate::SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;
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
    ) -> Result<Option<std::sync::RwLockWriteGuard<'_, std::collections::HashMap<String, HnswIndex>>>, crate::SqliteGraphError> {
        use std::sync::RwLock;
        use std::collections::HashMap;

        let indexes = self.hnsw_indexes.write().map_err(|e| crate::SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;

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
    ) -> Result<R, crate::SqliteGraphError>
    where
        F: FnOnce(&HnswIndex) -> R,
    {
        use std::sync::RwLock;

        let indexes = self.hnsw_indexes.read().map_err(|e| crate::SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;

        if let Some(hnsw) = indexes.get(name) {
            Ok(f(hnsw))
        } else {
            Err(crate::SqliteGraphError::invalid_input(format!("HNSW index '{}' not found", name)))
        }
    }

    /// Get a mutable reference to an HNSW index for modifications
    pub fn get_hnsw_index_mut<F, R>(
        &self,
        name: &str,
        f: F,
    ) -> Result<R, crate::SqliteGraphError>
    where
        F: FnOnce(&mut HnswIndex) -> R,
    {
        use std::sync::RwLock;

        let mut indexes = self.hnsw_indexes.write().map_err(|e| crate::SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;

        if let Some(hnsw) = indexes.get_mut(name) {
            Ok(f(hnsw))
        } else {
            Err(crate::SqliteGraphError::invalid_input(format!("HNSW index '{}' not found", name)))
        }
    }

    /// List all HNSW index names
    pub fn list_hnsw_indexes(&self) -> Result<Vec<String>, crate::SqliteGraphError> {
        use std::sync::RwLock;

        let indexes = self.hnsw_indexes.read().map_err(|e| crate::SqliteGraphError::invalid_input(format!("RwLock poisoned: {}", e)))?;
        Ok(indexes.keys().cloned().collect())
    }
}
