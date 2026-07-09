use super::*;

impl V3EdgeStore {
    pub(crate) fn read_page_from_disk(
        &self,
        file: &mut std::fs::File,
        db_path: &Path,
        page_id: u64,
    ) -> NativeResult<Vec<u8>> {
        use crate::backend::native::v3::constants::V3_HEADER_SIZE;
        use std::io::{Read, Seek, SeekFrom};

        let offset = V3_HEADER_SIZE + (page_id - 1) * (self.page_size as u64);
        file.seek(SeekFrom::Start(offset))
            .map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to seek to edge page {} offset {}", page_id, offset),
                source: e,
            })?;

        let mut buffer = vec![0u8; self.page_size as usize];
        file.read_exact(&mut buffer)
            .map_err(|e| NativeBackendError::IoError {
                context: format!(
                    "Failed to read edge page {} from {}",
                    page_id,
                    db_path.display()
                ),
                source: e,
            })?;
        Ok(buffer)
    }

    fn weighted_neighbors_from_cluster(
        &self,
        src: i64,
        dir: Direction,
        cluster: &V3EdgeCluster,
    ) -> Arc<[(i64, f32)]> {
        let mut edge_types = self.edge_types.write();
        let mut neighbors = Vec::with_capacity(cluster.edges.len());
        for e in &cluster.edges {
            let edge_type = V3EdgeCluster::extract_edge_type(&e.edge_data);
            if let Some(et) = edge_type {
                edge_types.insert((src, e.neighbor_id, dir), et);
            }
            let weight = V3EdgeCluster::extract_edge_weight(&e.edge_data);
            neighbors.push((e.neighbor_id, weight));
        }
        neighbors.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        Arc::from(neighbors.into_boxed_slice())
    }

    fn unweighted_neighbors_from_weighted(neighbors: &[(i64, f32)]) -> Arc<[i64]> {
        let dsts: Vec<i64> = neighbors.iter().map(|(dst, _)| *dst).collect();
        Arc::from(dsts.into_boxed_slice())
    }

    pub fn btree_lock(&self) -> &Arc<parking_lot::RwLock<BTreeManager>> {
        &self.btree
    }

    pub fn cache_lock(&self) -> &Arc<RwLock<HashMap<(i64, Direction), Arc<[i64]>>>> {
        &self.cache
    }

    pub fn cache_weighted_lock(
        &self,
    ) -> &Arc<RwLock<HashMap<(i64, Direction), Arc<[(i64, f32)]>>>> {
        &self.cache_weighted
    }

    pub fn edge_types_lock(&self) -> &Arc<RwLock<HashMap<(i64, i64, Direction), String>>> {
        &self.edge_types
    }

    pub fn page_size(&self) -> u64 {
        self.page_size as u64
    }

    /// Create new edge store (in-memory only)
    /// NOTE: Prefer with_path_and_allocator() for database-backed edge stores
    pub fn new(
        btree: BTreeManager,
        wal: Option<WALWriter>,
        allocator: Arc<RwLock<PageAllocator>>,
        page_size: u32,
    ) -> Self {
        Self {
            btree: Arc::new(parking_lot::RwLock::new(btree)),
            wal: wal.map(|w| Arc::new(RwLock::new(w))),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_weighted: Arc::new(RwLock::new(HashMap::new())),
            edge_types: Arc::new(RwLock::new(HashMap::new())),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            hit_time_ns: AtomicU64::new(0),
            miss_time_ns: AtomicU64::new(0),
            dirty_clusters: RwLock::new(HashMap::new()),
            db_path: None,
            allocator,
            page_size,
            file_coordinator: None,
        }
    }

    /// Create new edge store with database path and allocator
    /// This is the preferred constructor for database-backed edge stores
    pub fn with_path_and_allocator(
        btree: BTreeManager,
        wal: Option<WALWriter>,
        db_path: PathBuf,
        allocator: Arc<RwLock<PageAllocator>>,
        page_size: u32,
    ) -> Self {
        Self {
            btree: Arc::new(parking_lot::RwLock::new(btree)),
            wal: wal.map(|w| Arc::new(RwLock::new(w))),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_weighted: Arc::new(RwLock::new(HashMap::new())),
            edge_types: Arc::new(RwLock::new(HashMap::new())),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            hit_time_ns: AtomicU64::new(0),
            miss_time_ns: AtomicU64::new(0),
            dirty_clusters: RwLock::new(HashMap::new()),
            db_path: Some(db_path),
            allocator,
            page_size,
            file_coordinator: None,
        }
    }

    /// Create new edge store with disk persistence path
    /// NOTE: This creates a standalone allocator for backward compatibility.
    /// For proper page allocation, use with_path_and_allocator() instead.
    pub fn with_path(btree: BTreeManager, wal: Option<WALWriter>, db_path: PathBuf) -> Self {
        // Create a standalone allocator for backward compatibility
        // WARNING: This allocator is not shared with NodeStore, so page IDs
        // may collide. Always use with_path_and_allocator() in production.
        let header = PersistentHeaderV3::new_v3();
        Self {
            btree: Arc::new(parking_lot::RwLock::new(btree)),
            wal: wal.map(|w| Arc::new(RwLock::new(w))),
            cache: Arc::new(RwLock::new(HashMap::new())),
            cache_weighted: Arc::new(RwLock::new(HashMap::new())),
            edge_types: Arc::new(RwLock::new(HashMap::new())),
            cache_hits: AtomicU64::new(0),
            cache_misses: AtomicU64::new(0),
            hit_time_ns: AtomicU64::new(0),
            miss_time_ns: AtomicU64::new(0),
            dirty_clusters: RwLock::new(HashMap::new()),
            db_path: Some(db_path),
            allocator: Arc::new(RwLock::new(PageAllocator::new(&header))),
            page_size: header.page_size,
            file_coordinator: None,
        }
    }

    /// Set the file coordinator for coordinated I/O
    ///
    /// When set, all file reads/writes go through this coordinator to prevent
    /// race conditions and avoid per-cache-miss file open/close overhead.
    /// The coordinator is also propagated to the internal BTreeManager so
    /// edge-cluster lookups share the same persistent handle.
    pub fn set_file_coordinator(&mut self, coordinator: Arc<FileCoordinator>) {
        self.btree.write().set_file_coordinator(coordinator.clone());
        self.file_coordinator = Some(coordinator);
    }

    /// Get neighbors from cache - returns Arc<[i64]> for zero-copy!
    ///
    /// IMPROVED: On cache miss, attempts to load from disk if db_path is set.
    /// This enables recovery after reopening the edge store.
    pub fn neighbors(&self, src: i64, dir: Direction) -> NativeResult<Arc<[i64]>> {
        let key = (src, dir);

        #[cfg(feature = "v3-forensics")]
        FORENSIC_COUNTERS
            .logical_neighbors_calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // First check in-memory cache
        {
            let cache = self.cache.read();
            if let Some(neighbors) = cache.get(&key) {
                self.cache_hits.fetch_add(1, Ordering::Relaxed);
                #[cfg(feature = "v3-forensics")]
                FORENSIC_COUNTERS
                    .edge_cache_hit_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Ok(neighbors.clone()); // Arc clone is just pointer bump, no data copy
            }
        }

        self.cache_misses.fetch_add(1, Ordering::Relaxed);
        #[cfg(feature = "v3-forensics")]
        FORENSIC_COUNTERS
            .edge_cache_miss_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        // Cache miss - try to load from disk if we have a db_path
        if let Some(ref db_path) = self.db_path
            && let Ok(neighbors) = self.load_neighbors_from_disk(src, dir, db_path)
        {
            #[cfg(feature = "v3-forensics")]
            if !neighbors.is_empty() {
                FORENSIC_COUNTERS
                    .edge_page_read_count
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            if !neighbors.is_empty() {
                // Cache the loaded neighbors
                let mut cache = self.cache.write();
                cache.insert(key, neighbors.clone());
                return Ok(neighbors);
            }
        }

        Ok(Arc::from([])) // Empty slice, no allocation
    }

    /// Load neighbors from disk for recovery
    /// IMPORTANT: Also rebuilds edge_types HashMap from deserialized edge records
    /// CRITICAL FIX: Query B+Tree for page_id instead of using formula
    pub(crate) fn load_neighbors_from_disk(
        &self,
        src: i64,
        dir: Direction,
        db_path: &Path,
    ) -> NativeResult<Arc<[i64]>> {
        // CRITICAL FIX: Query B+Tree for page_id instead of calculating it
        // This prevents page ID collision with node storage
        let key = edge_key(src, dir);
        let btree = self.btree.read();

        // Try to get page_id from B+Tree
        let page_id = match btree.lookup(key) {
            Ok(Some(pid)) => pid,
            Ok(None) => {
                // No entry in B+Tree means no edges for this (src, dir)
                return Ok(Arc::from([]));
            }
            Err(_) => {
                // B+Tree lookup error - treat as no edges
                return Ok(Arc::from([]));
            }
        };
        drop(btree);

        match self.load_cluster_from_disk(page_id, src, dir, db_path) {
            Ok(cluster) => {
                let edges_with_types = cluster.edges_with_types();
                let mut edge_types = self.edge_types.write();
                for (dst, edge_type) in edges_with_types {
                    if let Some(et) = edge_type {
                        edge_types.insert((src, dst, dir), et);
                    }
                }

                let neighbors: Vec<i64> = cluster.dsts();
                Ok(Arc::from(neighbors.into_boxed_slice()))
            }
            Err(_) => Ok(Arc::from([])),
        }
    }

    /// Get outgoing neighbors
    pub fn outgoing(&self, src: i64) -> NativeResult<Arc<[i64]>> {
        self.neighbors(src, Direction::Outgoing)
    }

    /// Get incoming neighbors
    pub fn incoming(&self, src: i64) -> NativeResult<Arc<[i64]>> {
        self.neighbors(src, Direction::Incoming)
    }

    /// Get neighbors filtered by edge type
    /// Returns only neighbors connected by edges matching the specified edge_type
    pub fn neighbors_filtered(
        &self,
        src: i64,
        dir: Direction,
        edge_type: &str,
    ) -> NativeResult<Arc<[i64]>> {
        // Get all neighbors first
        let all_neighbors = self.neighbors(src, dir)?;

        // Filter by edge type
        let edge_types = self.edge_types.read();
        let filtered: Vec<i64> = all_neighbors
            .iter()
            .filter(|&&dst| {
                edge_types
                    .get(&(src, dst, dir))
                    .map(|stored_type| stored_type == edge_type)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        Ok(Arc::from(filtered.into_boxed_slice()))
    }

    /// Get the edge type for a specific edge
    pub fn get_edge_type(&self, src: i64, dst: i64, dir: Direction) -> Option<String> {
        let edge_types = self.edge_types.read();
        edge_types.get(&(src, dst, dir)).cloned()
    }

    /// Get weighted neighbors from cache - returns Arc<[(i64, f32)]> for zero-copy!
    pub fn neighbors_weighted(&self, src: i64, dir: Direction) -> NativeResult<Arc<[(i64, f32)]>> {
        let key = (src, dir);

        // First check in-memory cache
        {
            let cache = self.cache_weighted.read();
            if let Some(neighbors) = cache.get(&key) {
                self.cache_hits.fetch_add(1, Ordering::Relaxed);
                return Ok(neighbors.clone());
            }
        }

        self.cache_misses.fetch_add(1, Ordering::Relaxed);

        // Cache miss - try to load from disk if we have a db_path
        if let Some(ref db_path) = self.db_path
            && let Ok(neighbors) = self.load_neighbors_weighted_from_disk(src, dir, db_path)
        {
            if !neighbors.is_empty() {
                // Cache the loaded neighbors
                let mut cache = self.cache_weighted.write();
                cache.insert(key, neighbors.clone());
                return Ok(neighbors);
            }
        }

        Ok(Arc::from([]))
    }

    /// Load weighted neighbors from disk
    pub(crate) fn load_neighbors_weighted_from_disk(
        &self,
        src: i64,
        dir: Direction,
        db_path: &Path,
    ) -> NativeResult<Arc<[(i64, f32)]>> {
        let key = edge_key(src, dir);
        let btree = self.btree.read();

        let page_id = match btree.lookup(key) {
            Ok(Some(pid)) => pid,
            Ok(None) | Err(_) => return Ok(Arc::from([])),
        };
        drop(btree);

        match self.load_cluster_from_disk(page_id, src, dir, db_path) {
            Ok(cluster) => Ok(self.weighted_neighbors_from_cluster(src, dir, &cluster)),
            Err(_) => Ok(Arc::from([])),
        }
    }

    pub fn warm_weighted_neighbors(&self, sources: &[i64], dir: Direction) -> NativeResult<usize> {
        use std::collections::BTreeMap;

        let db_path = match &self.db_path {
            Some(path) => path,
            None => return Ok(0),
        };
        if sources.is_empty() {
            return Ok(0);
        }

        let misses: Vec<i64> = {
            let cache = self.cache_weighted.read();
            sources
                .iter()
                .copied()
                .filter(|src| !cache.contains_key(&(*src, dir)))
                .collect()
        };
        if misses.is_empty() {
            return Ok(0);
        }

        let lookups: Vec<(i64, u64)> = {
            let btree = self.btree.read();
            misses
                .into_iter()
                .filter_map(|src| match btree.lookup(edge_key(src, dir)) {
                    Ok(Some(page_id)) => Some((src, page_id)),
                    Ok(None) | Err(_) => None,
                })
                .collect()
        };
        if lookups.is_empty() {
            return Ok(0);
        }

        let mut page_groups: BTreeMap<u64, Vec<i64>> = BTreeMap::new();
        for (src, page_id) in lookups {
            page_groups.entry(page_id).or_default().push(src);
        }

        let mut file = std::fs::File::open(db_path).map_err(|e| NativeBackendError::IoError {
            context: format!(
                "Failed to open db file for edge warm read: {}",
                db_path.display()
            ),
            source: e,
        })?;

        let mut warmed = Vec::new();
        for (page_id, srcs) in page_groups {
            let buffer = self.read_page_from_disk(&mut file, db_path, page_id)?;
            if buffer.len() >= PACKED_EDGE_PAGE_HEADER_SIZE
                && buffer[0..4] == PACKED_EDGE_PAGE_MAGIC
            {
                for src in srcs {
                    if let Some(cluster_bytes) = decode_packed_edge_page(&buffer, src, dir)? {
                        let mut cluster = V3EdgeCluster::deserialize(&cluster_bytes, page_id)?;
                        cluster.page_id = 0;
                        cluster.sort_for_weighted_queries();
                        let weighted = self.weighted_neighbors_from_cluster(src, dir, &cluster);
                        let unweighted = Self::unweighted_neighbors_from_weighted(&weighted);
                        warmed.push((src, weighted, unweighted));
                    }
                }
                continue;
            }

            for src in srcs {
                let mut cluster = self.load_cluster_from_open_file(
                    &mut file,
                    page_id,
                    src,
                    dir,
                    db_path,
                    Some(buffer.clone()),
                )?;
                cluster.sort_for_weighted_queries();
                let weighted = self.weighted_neighbors_from_cluster(src, dir, &cluster);
                let unweighted = Self::unweighted_neighbors_from_weighted(&weighted);
                warmed.push((src, weighted, unweighted));
            }
        }

        if warmed.is_empty() {
            return Ok(0);
        }

        let warmed_count = warmed.len();
        let mut weighted_cache = self.cache_weighted.write();
        let mut unweighted_cache = self.cache.write();
        for (src, weighted, unweighted) in warmed {
            weighted_cache.insert((src, dir), weighted);
            unweighted_cache.insert((src, dir), unweighted);
        }
        Ok(warmed_count)
    }

    /// Get weighted neighbors filtered by edge type
    pub fn neighbors_weighted_filtered(
        &self,
        src: i64,
        dir: Direction,
        edge_type: &str,
    ) -> NativeResult<Arc<[(i64, f32)]>> {
        let all_neighbors = self.neighbors_weighted(src, dir)?;

        let edge_types = self.edge_types.read();
        let filtered: Vec<(i64, f32)> = all_neighbors
            .iter()
            .filter(|&&(dst, _)| {
                edge_types
                    .get(&(src, dst, dir))
                    .map(|stored_type| stored_type == edge_type)
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        Ok(Arc::from(filtered.into_boxed_slice()))
    }
}
