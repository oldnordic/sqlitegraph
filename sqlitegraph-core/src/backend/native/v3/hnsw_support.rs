use super::V3Backend;
use crate::SqliteGraphError;
use crate::backend::native::v3::KvValue;
use crate::hnsw::{HnswConfigBuilder, HnswIndex};
use crate::snapshot::SnapshotId;
use std::sync::{Arc, MutexGuard};

#[cfg(feature = "turbovec")]
#[path = "hnsw_support/turbovec_support.rs"]
mod turbovec_support;

/// Metadata wrapper for HNSW index with turbovec integration.
///
/// Tracks embedding count and turbovec compression state.
/// Automatically activates turbovec (4-bit quantization) at 1K vectors.
#[derive(Clone)]
pub(super) struct HnswIndexMetadata {
    /// HNSW index (always available, works for small datasets)
    pub(super) hnsw_index: Arc<std::sync::Mutex<HnswIndex>>,
    /// Turbovec compressed index (feature-gated, activates at 1K vectors)
    #[cfg(feature = "turbovec")]
    pub(super) turbovec_index: Arc<std::sync::Mutex<Option<turbovec::IdMapIndex>>>,
    /// Embedding count tracker (triggers turbovec at threshold)
    #[cfg(feature = "turbovec")]
    pub(super) embedding_count: Arc<std::sync::Mutex<usize>>,
    /// Vector dimension (must match all insertions)
    pub(super) dimension: usize,
}

impl HnswIndexMetadata {
    pub(super) fn lock_hnsw(
        &self,
        index_name: &str,
    ) -> Result<MutexGuard<'_, HnswIndex>, SqliteGraphError> {
        self.hnsw_index.lock().map_err(|_| {
            SqliteGraphError::validation(format!("HNSW index lock poisoned: {}", index_name))
        })
    }

    #[cfg(feature = "turbovec")]
    pub(super) fn lock_turbovec(
        &self,
        index_name: &str,
    ) -> Result<MutexGuard<'_, Option<turbovec::IdMapIndex>>, SqliteGraphError> {
        self.turbovec_index.lock().map_err(|_| {
            SqliteGraphError::validation(format!("Turbovec index lock poisoned: {}", index_name))
        })
    }

    #[cfg(feature = "turbovec")]
    pub(super) fn lock_embedding_count(
        &self,
        index_name: &str,
    ) -> Result<MutexGuard<'_, usize>, SqliteGraphError> {
        self.embedding_count.lock().map_err(|_| {
            SqliteGraphError::validation(format!(
                "HNSW embedding count lock poisoned: {}",
                index_name
            ))
        })
    }
}

/// Turbovec activation threshold: 1K vectors triggers compression.
#[cfg(feature = "turbovec")]
pub(super) const TURBOVEC_THRESHOLD: usize = 1_000;

#[cfg(feature = "turbovec")]
pub(super) const TURBOVEC_BIT_WIDTH: usize = 4;

#[derive(Debug, Clone, Copy, Default, Eq, PartialEq)]
pub struct HnswSearchConfig {
    pub force_exact: bool,
    pub ef_search_override: Option<usize>,
}

impl V3Backend {
    fn normalize_vector_result_id(
        hnsw: &HnswIndex,
        vector_id: u64,
    ) -> Result<i64, SqliteGraphError> {
        let normalized_id = hnsw
            .get_vector(vector_id)
            .map_err(|e| {
                SqliteGraphError::validation(format!(
                    "HNSW metadata lookup failed for vector {}: {:?}",
                    vector_id, e
                ))
            })?
            .map(|(_, metadata)| {
                metadata
                    .get("node_id")
                    .and_then(|value| value.as_i64())
                    .unwrap_or(vector_id as i64)
            })
            .unwrap_or(vector_id as i64);
        Ok(normalized_id)
    }

    fn hnsw_exact_search(
        &self,
        metadata_arc: &HnswIndexMetadata,
        query_vector: &[f32],
        k: usize,
        ef_search_override: Option<usize>,
    ) -> Result<Vec<(i64, f32)>, SqliteGraphError> {
        let mut hnsw = metadata_arc.lock_hnsw("<exact-search>")?;
        let original_ef_search = hnsw.config.ef_search;
        if let Some(ef_search) = ef_search_override {
            hnsw.config.ef_search = ef_search;
        }

        let results = hnsw
            .search(query_vector, k)
            .map_err(|e| SqliteGraphError::validation(format!("HNSW search error: {:?}", e)));
        hnsw.config.ef_search = original_ef_search;

        let results = results?;
        results
            .into_iter()
            .map(|(vector_id, distance)| {
                Self::normalize_vector_result_id(&hnsw, vector_id).map(|id| (id, distance))
            })
            .collect()
    }

    pub fn create_hnsw_index(
        &self,
        index_name: &str,
        dimension: usize,
    ) -> Result<(), SqliteGraphError> {
        self.ensure_hnsw_not_in_graph_transaction("create_hnsw_index")?;

        {
            let indexes = self.hnsw_indexes.read();
            if indexes.contains_key(index_name) {
                return Ok(());
            }
        }

        let config = HnswConfigBuilder::new()
            .dimension(dimension)
            .distance_metric(crate::hnsw::DistanceMetric::Euclidean)
            .build()
            .map_err(|e| SqliteGraphError::validation(format!("HNSW config error: {:?}", e)))?;

        #[cfg(feature = "native-v3")]
        let hnsw_index = {
            let storage_handle =
                crate::hnsw::v3_storage::V3VectorStorageHandle::new(self, index_name);
            HnswIndex::with_storage(index_name, config, Box::new(storage_handle)).map_err(|e| {
                SqliteGraphError::validation(format!("HNSW index creation error: {:?}", e))
            })?
        };

        #[cfg(not(feature = "native-v3"))]
        let hnsw_index = {
            HnswIndex::new(index_name, config).map_err(|e| {
                SqliteGraphError::validation(format!("HNSW index creation error: {:?}", e))
            })?
        };

        let metadata_key = format!("hnsw:{}:metadata", index_name);
        let metadata_value = serde_json::json!({
            "name": index_name,
            "dimension": dimension,
            "created_at": std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| {
                    SqliteGraphError::validation(format!(
                        "System clock before UNIX epoch during HNSW metadata creation: {}",
                        e
                    ))
                })?
                .as_secs()
        });
        self.kv_set_v3(
            metadata_key.into_bytes(),
            KvValue::Json(metadata_value),
            None,
        );

        let mut indexes = self.hnsw_indexes.write();
        indexes.insert(
            index_name.to_string(),
            HnswIndexMetadata {
                hnsw_index: Arc::new(std::sync::Mutex::new(hnsw_index)),
                #[cfg(feature = "turbovec")]
                turbovec_index: Arc::new(std::sync::Mutex::new(None)),
                #[cfg(feature = "turbovec")]
                embedding_count: Arc::new(std::sync::Mutex::new(0)),
                dimension,
            },
        );

        Ok(())
    }

    pub fn has_hnsw_index(&self, index_name: &str) -> bool {
        let indexes = self.hnsw_indexes.read();
        indexes.contains_key(index_name)
    }

    pub fn hnsw_embedding_count(&self, index_name: &str) -> Result<usize, SqliteGraphError> {
        let indexes = self.hnsw_indexes.read();
        let metadata = indexes.get(index_name).ok_or_else(|| {
            SqliteGraphError::validation(format!("HNSW index not found: {}", index_name))
        })?;

        #[cfg(feature = "turbovec")]
        {
            return Ok(*metadata.lock_embedding_count(index_name)?);
        }

        #[cfg(not(feature = "turbovec"))]
        {
            let hnsw = metadata.lock_hnsw(index_name)?;
            Ok(hnsw.vector_count())
        }
    }

    pub fn hnsw_turbovec_ready(&self, index_name: &str) -> Result<bool, SqliteGraphError> {
        let indexes = self.hnsw_indexes.read();
        let metadata = indexes.get(index_name).ok_or_else(|| {
            SqliteGraphError::validation(format!("HNSW index not found: {}", index_name))
        })?;

        #[cfg(feature = "turbovec")]
        {
            Ok(metadata.lock_turbovec(index_name)?.is_some())
        }

        #[cfg(not(feature = "turbovec"))]
        {
            let _ = metadata;
            Ok(false)
        }
    }

    pub fn get_hnsw_index(
        &self,
        index_name: &str,
    ) -> Result<Option<Arc<std::sync::Mutex<HnswIndex>>>, SqliteGraphError> {
        self.ensure_hnsw_not_in_graph_transaction("get_hnsw_index")?;
        let indexes = self.hnsw_indexes.read();
        Ok(indexes
            .get(index_name)
            .map(|metadata| metadata.hnsw_index.clone()))
    }

    #[cfg(feature = "turbovec")]
    pub fn insert_hnsw_vector(
        &self,
        index_name: &str,
        vector: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<(), SqliteGraphError> {
        self.ensure_hnsw_not_in_graph_transaction("insert_hnsw_vector")?;
        let metadata_arc = {
            let indexes = self.hnsw_indexes.read();
            indexes.get(index_name).cloned().ok_or_else(|| {
                SqliteGraphError::validation(format!("HNSW index not found: {}", index_name))
            })?
        };

        if vector.len() != metadata_arc.dimension {
            return Err(SqliteGraphError::validation(format!(
                "Vector dimension mismatch: expected {}, got {}",
                metadata_arc.dimension,
                vector.len()
            )));
        }

        let mut hnsw = metadata_arc.lock_hnsw(index_name)?;
        hnsw.insert_vector(vector, metadata)
            .map_err(|e| SqliteGraphError::validation(format!("HNSW insert failed: {:?}", e)))?;
        drop(hnsw);

        let mut count = metadata_arc.lock_embedding_count(index_name)?;
        *count += 1;
        let current_count = *count;
        drop(count);

        if current_count == TURBOVEC_THRESHOLD + 1 {
            self.build_turbovec_index(index_name)?;
        }

        Ok(())
    }

    #[cfg(not(feature = "turbovec"))]
    pub fn insert_hnsw_vector(
        &self,
        index_name: &str,
        vector: &[f32],
        metadata: Option<serde_json::Value>,
    ) -> Result<(), SqliteGraphError> {
        self.ensure_hnsw_not_in_graph_transaction("insert_hnsw_vector")?;
        let metadata_arc = {
            let indexes = self.hnsw_indexes.read();
            indexes.get(index_name).cloned().ok_or_else(|| {
                SqliteGraphError::validation(format!("HNSW index not found: {}", index_name))
            })?
        };

        if vector.len() != metadata_arc.dimension {
            return Err(SqliteGraphError::validation(format!(
                "Vector dimension mismatch: expected {}, got {}",
                metadata_arc.dimension,
                vector.len()
            )));
        }

        let mut hnsw = metadata_arc.lock_hnsw(index_name)?;
        hnsw.insert_vector(vector, metadata)
            .map_err(|e| SqliteGraphError::validation(format!("HNSW insert failed: {:?}", e)))?;

        Ok(())
    }

    pub fn delete_hnsw_index(&self, index_name: &str) -> Result<(), SqliteGraphError> {
        self.ensure_hnsw_not_in_graph_transaction("delete_hnsw_index")?;
        let mut indexes = self.hnsw_indexes.write();
        indexes.remove(index_name);

        let metadata_key = format!("hnsw:{}:metadata", index_name);
        self.kv_delete_v3(metadata_key.as_bytes());

        let vector_prefix = format!("hnsw:{}:vector:", index_name);
        let snapshot_id = SnapshotId::current();
        let vectors = self.kv_prefix_scan_v3(snapshot_id, vector_prefix.as_bytes());
        for (key, _) in vectors {
            self.kv_delete_v3(&key);
        }

        Ok(())
    }

    #[cfg(feature = "turbovec")]
    fn build_turbovec_index(&self, index_name: &str) -> Result<(), SqliteGraphError> {
        turbovec_support::build_turbovec_index(self, index_name)
    }

    #[cfg(feature = "turbovec")]
    fn ensure_turbovec_index(&self, index_name: &str) -> Result<(), SqliteGraphError> {
        turbovec_support::ensure_turbovec_index(self, index_name)
    }

    #[cfg(feature = "turbovec")]
    fn turbovec_search(
        &self,
        index: &turbovec::IdMapIndex,
        query_vector: &[f32],
        k: usize,
    ) -> Vec<(i64, f32)> {
        turbovec_support::turbovec_search(index, query_vector, k)
    }

    pub fn hnsw_vector_search(
        &self,
        index_name: &str,
        query_vector: &[f32],
        k: usize,
    ) -> Result<Vec<(i64, f32)>, SqliteGraphError> {
        self.hnsw_vector_search_with_config(
            index_name,
            query_vector,
            k,
            HnswSearchConfig::default(),
        )
    }

    pub fn hnsw_vector_search_with_config(
        &self,
        index_name: &str,
        query_vector: &[f32],
        k: usize,
        config: HnswSearchConfig,
    ) -> Result<Vec<(i64, f32)>, SqliteGraphError> {
        let metadata_arc = {
            let indexes = self.hnsw_indexes.read();
            indexes.get(index_name).cloned().ok_or_else(|| {
                SqliteGraphError::validation(format!("HNSW index not found: {}", index_name))
            })?
        };

        if query_vector.len() != metadata_arc.dimension {
            return Err(SqliteGraphError::validation(format!(
                "Query vector dimension mismatch: expected {}, got {}",
                metadata_arc.dimension,
                query_vector.len()
            )));
        }

        if let Some(ef_search) = config.ef_search_override
            && (ef_search == 0 || ef_search > 200)
        {
            return Err(SqliteGraphError::validation(format!(
                "Invalid ef_search_override: {}. Must be in 1..=200",
                ef_search
            )));
        }

        #[cfg(feature = "turbovec")]
        {
            let count = *metadata_arc.lock_embedding_count(index_name)?;
            if count > TURBOVEC_THRESHOLD && !config.force_exact {
                self.ensure_turbovec_index(index_name)?;

                let turbovec = metadata_arc.lock_turbovec(index_name)?;
                if let Some(ref index) = *turbovec {
                    return Ok(self.turbovec_search(index, query_vector, k));
                }
            }
        }

        self.hnsw_exact_search(&metadata_arc, query_vector, k, config.ef_search_override)
    }
}
