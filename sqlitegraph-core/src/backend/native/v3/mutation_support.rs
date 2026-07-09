use super::{V3Backend, map_v3_error};
use crate::SqliteGraphError;
use crate::backend::EdgeSpec;
use crate::backend::NodeSpec;
use crate::backend::native::types::NodeFlags;
use crate::backend::native::v3::NodeRecordV3;
use crate::backend::native::v3::edge_compat::Direction as EdgeDirection;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};

impl V3Backend {
    /// Insert node without syncing (internal use only)
    ///
    /// Used by WriteBatchGuard to accumulate changes.
    /// Marked pub for benchmark access.
    pub fn insert_node_inner(&self, node: NodeSpec) -> Result<i64, SqliteGraphError> {
        let kind_bytes = node.kind.as_bytes();
        let name_bytes = node.name.as_bytes();
        let data_bytes = serde_json::to_vec(&node.data).unwrap_or_default();

        let total_len = 2 + kind_bytes.len() + name_bytes.len() + data_bytes.len();
        const MAX_INLINE_DATA: usize = 64;

        let requested_id = node
            .data
            .get("id")
            .or_else(|| node.data.get("node_id"))
            .and_then(|v| v.as_i64());

        let node_record = if total_len <= MAX_INLINE_DATA {
            let mut inline_data = Vec::with_capacity(total_len);
            inline_data.push(kind_bytes.len() as u8);
            inline_data.extend_from_slice(kind_bytes);
            inline_data.push(name_bytes.len() as u8);
            inline_data.extend_from_slice(name_bytes);
            inline_data.extend_from_slice(&data_bytes);

            NodeRecordV3::new_inline(
                requested_id.unwrap_or(0),
                NodeFlags::empty(),
                0,
                0,
                inline_data,
                0,
                0,
                0,
                0,
            )
        } else {
            let mut external_data = Vec::with_capacity(total_len);
            external_data.push(kind_bytes.len() as u8);
            external_data.extend_from_slice(kind_bytes);
            external_data.push(name_bytes.len() as u8);
            external_data.extend_from_slice(name_bytes);
            external_data.extend_from_slice(&data_bytes);

            let data_len = external_data.len();
            let page_size = crate::backend::native::v3::constants::DEFAULT_PAGE_SIZE as usize;
            let pages_needed = data_len.div_ceil(page_size);

            let mut allocator = self.allocator.write();
            let start_page_id = allocator
                .allocate()
                .map_err(SqliteGraphError::NativeError)?;

            for _ in 1..pages_needed {
                allocator
                    .allocate()
                    .map_err(SqliteGraphError::NativeError)?;
            }

            let offset = Self::page_offset(start_page_id);

            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(&self.db_path)
                .map_err(|e| {
                    SqliteGraphError::ConnectionError(format!("Failed to open file: {}", e))
                })?;

            file.seek(SeekFrom::Start(offset))
                .map_err(|e| SqliteGraphError::ConnectionError(format!("Failed to seek: {}", e)))?;
            file.write_all(&external_data).map_err(|e| {
                SqliteGraphError::ConnectionError(format!("Failed to write: {}", e))
            })?;
            file.sync_all().map_err(|e| {
                SqliteGraphError::ConnectionError(format!("Failed to sync external data: {}", e))
            })?;

            NodeRecordV3::new_external(
                requested_id.unwrap_or(0),
                NodeFlags::empty(),
                0,
                0,
                offset,
                data_len as u16,
                0,
                0,
                0,
                0,
            )
        };

        let mut node_store = self.node_store.write();
        let node_id = node_store
            .insert_node(node_record, requested_id)
            .map_err(map_v3_error)?;

        self.kind_index.insert(node.kind.clone(), node_id);
        self.name_index.insert(node.name.clone(), node_id);

        let mut header = self.header.write();
        header.node_count += 1;

        if let Some(root_page) = node_store.btree_root_page_id() {
            header.root_index_page = root_page;
        }
        if let Some(tree_height) = node_store.btree_height() {
            header.btree_height = tree_height;
        }

        {
            let pub_guard = self.publisher.read();
            if let Some(ref publisher) = *pub_guard
                && publisher.has_subscribers()
            {
                let current_lsn = if let Some(ref wal) = self.wal {
                    wal.read().committed_lsn()
                } else {
                    *self.current_snapshot_version.read()
                };
                publisher.emit(
                    crate::backend::native::v3::pubsub::types::PubSubEvent::NodeChanged {
                        node_id,
                        snapshot_id: current_lsn,
                    },
                );
            }
        }

        Ok(node_id)
    }

    /// Calculate file offset for a given page ID
    ///
    /// Page 0 is the header at offset 0.
    /// Data pages start at page 1, which maps to offset V3_HEADER_SIZE.
    pub(super) fn page_offset(page_id: u64) -> u64 {
        if page_id == 0 {
            return 0;
        }
        let data_page_index = page_id.saturating_sub(1);
        crate::backend::native::v3::constants::V3_HEADER_SIZE
            + data_page_index * crate::backend::native::v3::constants::DEFAULT_PAGE_SIZE
    }

    /// Insert edge without syncing (internal use only)
    ///
    /// Used by WriteBatchGuard to accumulate changes.
    pub(super) fn insert_edge_inner(&self, edge: EdgeSpec) -> Result<i64, SqliteGraphError> {
        let from_exists = self.get_node_internal(edge.from)?.is_some();
        let to_exists = self.get_node_internal(edge.to)?.is_some();
        if !from_exists || !to_exists {
            return Err(SqliteGraphError::invalid_input(
                "edge endpoints must reference existing entities",
            ));
        }

        let edge_store = self.edge_store.write();

        let edge_type = if edge.edge_type.is_empty() {
            None
        } else {
            Some(edge.edge_type.clone())
        };

        let weight = edge
            .data
            .get("weight")
            .and_then(|v| v.as_f64())
            .map(|w| w as f32)
            .unwrap_or(1.0);

        edge_store
            .insert_edge_weighted(
                edge.from,
                edge.to,
                EdgeDirection::Outgoing,
                edge_type.clone(),
                weight,
            )
            .map_err(map_v3_error)?;
        edge_store
            .insert_edge_weighted(
                edge.to,
                edge.from,
                EdgeDirection::Incoming,
                edge_type,
                weight,
            )
            .map_err(map_v3_error)?;

        let mut header = self.header.write();
        header.edge_count += 1;
        let edge_id = header.edge_count as i64;

        {
            let pub_guard = self.publisher.read();
            if let Some(ref publisher) = *pub_guard
                && publisher.has_subscribers()
            {
                let current_lsn = if let Some(ref wal) = self.wal {
                    wal.read().committed_lsn()
                } else {
                    *self.current_snapshot_version.read()
                };
                publisher.emit(
                    crate::backend::native::v3::pubsub::types::PubSubEvent::EdgeChanged {
                        edge_id,
                        from_node: edge.from,
                        to_node: edge.to,
                        snapshot_id: current_lsn,
                    },
                );
            }
        }

        Ok(edge_id)
    }
}
