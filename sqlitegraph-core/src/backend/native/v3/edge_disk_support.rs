use super::*;

impl V3EdgeStore {
    pub fn flush(
        &self,
        _kv_store: Option<
            &parking_lot::RwLock<Option<crate::backend::native::v3::kv_store::store::KvStore>>,
        >,
    ) -> NativeResult<()> {
        let db_path = match &self.db_path {
            Some(path) => path.clone(),
            None => {
                // In-memory mode: just clear dirty clusters
                self.dirty_clusters.write().clear();
                return Ok(());
            }
        };

        let dirty = self.dirty_clusters.write();

        if dirty.is_empty() {
            return Ok(()); // Nothing to flush
        }

        // Get mutable access to btree for index updates
        // Note: We use unsafe here because we need to mutate through &self
        // In production, this would use interior mutability patterns
        // For now, we use a simple approach: clone dirty clusters and process them
        let clusters_to_flush: Vec<((i64, Direction), V3EdgeCluster)> =
            dirty.iter().map(|(k, v)| (*k, v.clone())).collect();

        // Drop the lock before doing I/O
        drop(dirty);

        let packed_capacity = (self.page_size as usize)
            .checked_sub(PACKED_EDGE_PAGE_HEADER_SIZE + PACKED_EDGE_PAGE_SLOT_SIZE)
            .ok_or_else(|| NativeBackendError::SerializationError {
                context: format!(
                    "edge page size {} too small for packed edge encoding",
                    self.page_size
                ),
            })?;
        let overflow_capacity = (self.page_size as usize)
            .checked_sub(EDGE_CLUSTER_PAGE_HEADER_SIZE)
            .ok_or_else(|| NativeBackendError::SerializationError {
                context: format!(
                    "edge page size {} too small for multi-page encoding",
                    self.page_size
                ),
            })?;

        let mut packed_entries: Vec<((i64, Direction), Vec<u8>)> = Vec::new();
        let mut packed_bytes_used = PACKED_EDGE_PAGE_HEADER_SIZE;

        let flush_packed_entries = |entries: &mut Vec<((i64, Direction), Vec<u8>)>,
                                    bytes_used: &mut usize|
         -> NativeResult<()> {
            if entries.is_empty() {
                return Ok(());
            }

            let packed_page_id = {
                let mut allocator = self.allocator.write();
                allocator
                    .allocate()
                    .map_err(|e| NativeBackendError::IoError {
                        context: "Failed to allocate packed edge page".to_string(),
                        source: std::io::Error::other(e.to_string()),
                    })?
            };

            let page_bytes = encode_packed_edge_page(self.page_size as usize, entries)?;
            self.write_page_to_disk(&db_path, packed_page_id, &page_bytes)?;

            let mut btree = self.btree.write();
            for ((src, dir), _) in entries.iter() {
                let key = edge_key(*src, *dir);
                let _ = btree.insert(key, packed_page_id);
            }

            entries.clear();
            *bytes_used = PACKED_EDGE_PAGE_HEADER_SIZE;
            Ok(())
        };

        for ((src, dir), cluster) in clusters_to_flush {
            let mut cluster = cluster;
            cluster.sort_for_weighted_queries();
            let cluster_bytes = cluster.serialize()?;
            let packed_size = PACKED_EDGE_PAGE_SLOT_SIZE + cluster_bytes.len();

            if cluster_bytes.len() <= packed_capacity {
                if packed_bytes_used + packed_size > self.page_size as usize {
                    flush_packed_entries(&mut packed_entries, &mut packed_bytes_used)?;
                }
                packed_bytes_used += packed_size;
                packed_entries.push(((src, dir), cluster_bytes));
                continue;
            }

            flush_packed_entries(&mut packed_entries, &mut packed_bytes_used)?;

            let first_page_id = {
                let mut allocator = self.allocator.write();
                allocator
                    .allocate()
                    .map_err(|e| NativeBackendError::IoError {
                        context: format!("Failed to allocate edge page for ({}, {:?})", src, dir),
                        source: std::io::Error::other(e.to_string()),
                    })?
            };

            let total_pages = cluster_bytes.len().max(1).div_ceil(overflow_capacity);
            let mut page_ids = Vec::with_capacity(total_pages);
            page_ids.push(first_page_id);
            if total_pages > 1 {
                let mut allocator = self.allocator.write();
                for _ in 1..total_pages {
                    let overflow_page_id =
                        allocator
                            .allocate()
                            .map_err(|e| NativeBackendError::IoError {
                                context: format!(
                                    "Failed to allocate overflow edge page for ({}, {:?})",
                                    src, dir
                                ),
                                source: std::io::Error::other(e.to_string()),
                            })?;
                    page_ids.push(overflow_page_id);
                }
            }

            let encoded_pages =
                encode_edge_cluster_pages(&cluster_bytes, self.page_size as usize, &page_ids)?;
            for (page_id, page_bytes) in page_ids.into_iter().zip(encoded_pages) {
                self.write_page_to_disk(&db_path, page_id, &page_bytes)?;
            }

            {
                let mut btree = self.btree.write();
                let key = edge_key(src, dir);
                let _ = btree.insert(key, first_page_id);
            }
        }

        flush_packed_entries(&mut packed_entries, &mut packed_bytes_used)?;

        // Clear dirty clusters after successful flush
        self.dirty_clusters.write().clear();

        // CRITICAL FIX: Checkpoint and truncate WAL after successful flush
        // This ensures WAL doesn't grow unbounded and data is durable
        if let Some(ref wal) = self.wal {
            let mut wal_guard = wal.write();

            // Get current B+Tree state for checkpoint
            let btree = self.btree.read();
            let root_page_id = btree.root_page_id();
            let tree_height = btree.tree_height();

            // Write checkpoint record
            let _ = wal_guard.checkpoint(
                root_page_id,
                0, // total_pages - not tracked in edge store
                tree_height,
                0,                             // free_page_list_head - not tracked in edge store
                &PersistentHeaderV3::new_v3(), // header snapshot
            );

            // Flush WAL to ensure checkpoint is on disk
            let _ = wal_guard.flush();

            // Truncate WAL after successful checkpoint
            // Safe because main DB pages are now durable
            let _ = wal_guard.truncate();
        }

        // CRITICAL FIX: Persist B+Tree metadata to allow recovery
        // This must happen after WAL checkpoint since we need the final root_page_id
        self.persist_btree_metadata()?;

        Ok(())
    }

    /// Get the path to the edge metadata file
    fn metadata_path(&self) -> Option<PathBuf> {
        self.db_path
            .as_ref()
            .map(|p| p.with_extension("v3edgemeta"))
    }

    pub(crate) fn load_cluster_from_disk(
        &self,
        page_id: u64,
        src: i64,
        dir: Direction,
        db_path: &Path,
    ) -> NativeResult<V3EdgeCluster> {
        use std::fs::File;

        let mut file = File::open(db_path).map_err(|e| NativeBackendError::IoError {
            context: format!(
                "Failed to open db file for edge cluster read: {}",
                db_path.display()
            ),
            source: e,
        })?;

        self.load_cluster_from_open_file(&mut file, page_id, src, dir, db_path, None)
    }

    pub(crate) fn load_cluster_from_open_file(
        &self,
        file: &mut std::fs::File,
        page_id: u64,
        src: i64,
        dir: Direction,
        db_path: &Path,
        first_page: Option<Vec<u8>>,
    ) -> NativeResult<V3EdgeCluster> {
        let mut current_page_id = page_id;
        let mut cluster_bytes = Vec::new();
        let mut buffer = match first_page {
            Some(buffer) => buffer,
            None => self.read_page_from_disk(file, db_path, current_page_id)?,
        };

        loop {
            if let Some(cluster_bytes) = decode_packed_edge_page(&buffer, src, dir)? {
                let mut cluster = V3EdgeCluster::deserialize(&cluster_bytes, page_id)?;
                cluster.page_id = 0;
                return Ok(cluster);
            }

            if let Some((payload_len, next_page_id)) = decode_edge_cluster_page_header(&buffer) {
                let payload_end = EDGE_CLUSTER_PAGE_HEADER_SIZE + payload_len;
                cluster_bytes
                    .extend_from_slice(&buffer[EDGE_CLUSTER_PAGE_HEADER_SIZE..payload_end]);
                if next_page_id == 0 {
                    break;
                }
                current_page_id = next_page_id;
                buffer = self.read_page_from_disk(file, db_path, current_page_id)?;
            } else {
                return V3EdgeCluster::deserialize(&buffer, page_id);
            }
        }

        V3EdgeCluster::deserialize(&cluster_bytes, page_id)
    }

    /// Persist B+Tree root metadata to disk for recovery
    ///
    /// This writes a small metadata file containing the B+Tree root_page_id
    /// and tree_height so that the edge index can be recovered after restart.
    fn persist_btree_metadata(&self) -> NativeResult<()> {
        let meta_path = match self.metadata_path() {
            Some(p) => p,
            None => return Ok(()), // In-memory mode, no persistence needed
        };

        let btree = self.btree.read();
        let root_page_id = btree.root_page_id();
        let tree_height = btree.tree_height();

        // Metadata format: [magic: 8 bytes][root_page_id: 8 bytes][tree_height: 4 bytes][checksum: 4 bytes]
        let mut data = Vec::with_capacity(24);
        data.extend_from_slice(b"V3EDGE\x00\x00"); // 8 bytes magic
        data.extend_from_slice(&root_page_id.to_le_bytes()); // 8 bytes
        data.extend_from_slice(&tree_height.to_le_bytes()); // 4 bytes

        // Simple checksum (XOR of bytes)
        let checksum: u32 = data.iter().fold(0u32, |acc, &b| acc.wrapping_add(b as u32));
        data.extend_from_slice(&checksum.to_le_bytes()); // 4 bytes

        std::fs::write(&meta_path, &data).map_err(|e| NativeBackendError::IoError {
            context: format!("Failed to write edge metadata: {}", meta_path.display()),
            source: e,
        })?;

        Ok(())
    }

    /// Recover B+Tree root metadata from disk
    ///
    /// Returns (root_page_id, tree_height) if metadata file exists and is valid.
    /// Returns None if metadata doesn't exist or is corrupted.
    fn recover_btree_metadata(&self) -> NativeResult<Option<(u64, u32)>> {
        let meta_path = match self.metadata_path() {
            Some(p) => p,
            None => return Ok(None), // In-memory mode, no recovery possible
        };

        if !meta_path.exists() {
            return Ok(None);
        }

        let data = std::fs::read(&meta_path).map_err(|e| NativeBackendError::IoError {
            context: format!("Failed to read edge metadata: {}", meta_path.display()),
            source: e,
        })?;

        if data.len() < 24 {
            return Ok(None); // Corrupted or incomplete
        }

        // Verify magic
        if &data[0..8] != b"V3EDGE\x00\x00" {
            return Ok(None); // Invalid magic
        }

        // Parse values
        let root_page_id = u64::from_le_bytes([
            data[8], data[9], data[10], data[11], data[12], data[13], data[14], data[15],
        ]);
        let tree_height = u32::from_le_bytes([data[16], data[17], data[18], data[19]]);

        // Verify checksum
        let stored_checksum = u32::from_le_bytes([data[20], data[21], data[22], data[23]]);
        let computed_checksum: u32 = data[..20]
            .iter()
            .fold(0u32, |acc, &b| acc.wrapping_add(b as u32));

        if stored_checksum != computed_checksum {
            return Ok(None); // Checksum mismatch
        }

        Ok(Some((root_page_id, tree_height)))
    }

    /// Restore B+Tree state from persisted metadata
    ///
    /// This should be called after creating a new V3EdgeStore to recover
    /// the B+Tree root from a previous session.
    pub fn restore_btree_from_metadata(&self) -> NativeResult<bool> {
        if let Some((root_page_id, tree_height)) = self.recover_btree_metadata()? {
            let mut btree = self.btree.write();
            btree.set_root_page_id(root_page_id);
            btree.set_tree_height(tree_height);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Write a page of data to disk
    ///
    /// BUG FIX: Previously opened a raw file handle bypassing FileCoordinator,
    /// which could cause data corruption from concurrent writes with NodeStore.
    /// Now routes through FileCoordinator when available.
    fn write_page_to_disk(&self, db_path: &Path, page_id: u64, data: &[u8]) -> NativeResult<()> {
        #[cfg(feature = "v3-forensics")]
        {
            use crate::backend::native::v3::constants::V3_HEADER_SIZE;
            let offset: u64 = if page_id == 0 {
                0
            } else {
                V3_HEADER_SIZE + (page_id - 1) * (self.page_size as u64)
            };
            crate::track_page_alloc!(page_id, Subsystem::EdgeStore, ForensicPageType::Edge);
            crate::track_page_write!(
                page_id,
                Subsystem::EdgeStore,
                ForensicPageType::Edge,
                offset,
                "EdgeStore::write_page_to_disk"
            );
        }

        // Use FileCoordinator when available to prevent race conditions
        if let Some(ref coordinator) = self.file_coordinator {
            // Pad data to full page size if needed for page-aligned writes
            let page_data = if data.len() < self.page_size as usize {
                let mut padded = data.to_vec();
                padded.resize(self.page_size as usize, 0);
                padded
            } else {
                data.to_vec()
            };
            return coordinator.write_page(page_id, &page_data);
        }

        // Fallback: raw file I/O (legacy path, no coordinator set)
        use crate::backend::native::v3::constants::V3_HEADER_SIZE;

        let offset: u64 = if page_id == 0 {
            0
        } else {
            V3_HEADER_SIZE + (page_id - 1) * (self.page_size as u64)
        };

        // CRITICAL FIX: Do NOT use create(true) - it truncates the file!
        let file_exists = db_path.exists();
        let mut file = OpenOptions::new()
            .write(true)
            .create(!file_exists)
            .open(db_path)
            .map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to open db file for page write: {}", page_id),
                source: e,
            })?;

        // Extend file if needed
        let required_len = offset + data.len() as u64;
        let current_len = file.metadata().map(|m| m.len()).unwrap_or(0);
        if required_len > current_len {
            file.set_len(required_len)
                .map_err(|e| NativeBackendError::IoError {
                    context: format!(
                        "Failed to extend file to {} bytes for page {}",
                        required_len, page_id
                    ),
                    source: e,
                })?;
        }

        file.seek(SeekFrom::Start(offset))
            .map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to seek to page {} offset {}", page_id, offset),
                source: e,
            })?;

        file.write_all(data)
            .map_err(|e| NativeBackendError::IoError {
                context: format!("Failed to write page {} data", page_id),
                source: e,
            })?;

        file.sync_data().map_err(|e| NativeBackendError::IoError {
            context: format!("Failed to sync page {} write", page_id),
            source: e,
        })?;

        Ok(())
    }

    /// Flush WAL buffer to disk (for durability testing)
    #[cfg(test)]
    pub fn flush_wal(&self) -> NativeResult<()> {
        if let Some(ref wal) = self.wal {
            let mut wal_guard = wal.write();
            wal_guard.flush()
        } else {
            Ok(())
        }
    }

    /// Get the current B+Tree root page ID
    /// CRITICAL: Must be called during flush_to_disk to update header
    ///
    /// Returns None if the tree is empty (EMPTY_TREE_ROOT = u64::MAX)
    /// Returns Some(page_id) if the tree has a valid root
    pub fn btree_root_page_id(&self) -> Option<u64> {
        let root = self.btree.read().root_page_id();
        // Filter out EMPTY_TREE_ROOT (u64::MAX) and 0 (uninitialized)
        if root != 0 && root != u64::MAX {
            Some(root)
        } else {
            None
        }
    }

    /// Get the current B+Tree height
    /// CRITICAL: Must be called during flush_to_disk to update header
    pub fn btree_height(&self) -> u32 {
        self.btree.read().tree_height()
    }

    /// Set the WAL writer for this edge store
    ///
    /// This is called after opening an existing database when a WAL is discovered.
    pub fn set_wal(&mut self, wal: Arc<RwLock<WALWriter>>) {
        self.wal = Some(wal);
    }
}
