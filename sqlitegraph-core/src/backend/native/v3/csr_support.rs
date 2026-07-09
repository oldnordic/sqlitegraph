use super::{
    CSR_SHARD_INCOMING, CSR_SHARD_OUTGOING, CsrEdgeTypeLookup, CsrRuntimeRow, V3Backend,
    encode_csr_adjacency, map_v3_error,
};
use crate::SqliteGraphError;
use crate::backend::native::v3::edge_compat::Direction as EdgeDirection;
use crate::backend::{BackendDirection, NeighborQuery};
use crate::snapshot::SnapshotId;
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

impl V3Backend {
    pub(super) fn csr_shard_id(direction: BackendDirection) -> i64 {
        match direction {
            BackendDirection::Outgoing => CSR_SHARD_OUTGOING,
            BackendDirection::Incoming => CSR_SHARD_INCOMING,
        }
    }

    pub(super) fn lookup_csr_edge_type_id(
        &self,
        edge_type: &str,
    ) -> Result<CsrEdgeTypeLookup, SqliteGraphError> {
        let conn = self.sqlite_conn.lock();
        let mut stmt = match conn.prepare("SELECT type_id FROM csr_edge_types WHERE edge_type = ?1")
        {
            Ok(stmt) => stmt,
            Err(e) if e.to_string().contains("no such table: csr_edge_types") => {
                return Ok(CsrEdgeTypeLookup::MissingTable);
            }
            Err(e) => {
                return Err(SqliteGraphError::query(format!(
                    "Failed to prepare CSR edge type lookup: {}",
                    e
                )));
            }
        };

        let mut rows = stmt.query([edge_type]).map_err(|e| {
            SqliteGraphError::query(format!("Failed to execute CSR edge type lookup: {}", e))
        })?;

        let Some(row) = rows.next().map_err(|e| {
            SqliteGraphError::query(format!("Failed to read CSR edge type row: {}", e))
        })?
        else {
            return Ok(CsrEdgeTypeLookup::MissingType);
        };

        let type_id: i64 = row.get(0).map_err(|e| {
            SqliteGraphError::query(format!("Failed to load CSR edge type id: {}", e))
        })?;
        Ok(CsrEdgeTypeLookup::Found(type_id as u32))
    }

    pub(super) fn load_csr_adjacency(
        &self,
        node_id: i64,
        visible_version: u64,
        direction: BackendDirection,
        edge_type: Option<&str>,
    ) -> Result<Option<Vec<(i64, f32)>>, SqliteGraphError> {
        let expected_type_id = match edge_type {
            Some(edge_type) => match self.lookup_csr_edge_type_id(edge_type)? {
                CsrEdgeTypeLookup::MissingTable => return Ok(None),
                CsrEdgeTypeLookup::MissingType => return Ok(Some(Vec::new())),
                CsrEdgeTypeLookup::Found(type_id) => Some(type_id),
            },
            None => None,
        };

        let conn = self.sqlite_conn.lock();
        let mut stmt = match conn.prepare(
            "SELECT shard_data
             FROM csr_shards
             WHERE shard_id = ?1 AND node_id = ?2 AND visible_at <= ?3
             ORDER BY version DESC
             LIMIT 1",
        ) {
            Ok(stmt) => stmt,
            Err(e) if e.to_string().contains("no such table: csr_shards") => return Ok(None),
            Err(e) => {
                return Err(SqliteGraphError::query(format!(
                    "Failed to prepare CSR lookup: {}",
                    e
                )));
            }
        };

        let mut rows = stmt
            .query([
                Self::csr_shard_id(direction),
                node_id,
                visible_version as i64,
            ])
            .map_err(|e| SqliteGraphError::query(format!("Failed to execute CSR lookup: {}", e)))?;

        let Some(row) = rows
            .next()
            .map_err(|e| SqliteGraphError::query(format!("Failed to read CSR row: {}", e)))?
        else {
            return Ok(None);
        };

        let blob: Option<Vec<u8>> = row
            .get(0)
            .map_err(|e| SqliteGraphError::query(format!("Failed to load CSR blob: {}", e)))?;

        let Some(blob) = blob else {
            return Ok(Some(Vec::new()));
        };

        if blob.len() % 12 != 0 {
            return Err(SqliteGraphError::GraphCorruption(format!(
                "CSR shard_data for node {} has invalid length {} (expected 12-byte records)",
                node_id,
                blob.len()
            )));
        }

        let mut neighbors = Vec::with_capacity(blob.len() / 12);
        for chunk in blob.chunks_exact(12) {
            let dst = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as i64;
            let weight = f32::from_le_bytes([chunk[4], chunk[5], chunk[6], chunk[7]]);
            let flags = u32::from_le_bytes([chunk[8], chunk[9], chunk[10], chunk[11]]);
            if expected_type_id.is_none_or(|type_id| flags == type_id) {
                neighbors.push((dst, weight));
            }
        }

        Ok(Some(neighbors))
    }

    pub(super) fn csr_neighbors_shared(
        &self,
        snapshot_id: SnapshotId,
        node: i64,
        query: &NeighborQuery,
    ) -> Result<Option<Arc<[i64]>>, SqliteGraphError> {
        let visible_version = if snapshot_id == SnapshotId::current() {
            *self.current_snapshot_version.read()
        } else {
            snapshot_id.as_u64()
        };

        let Some(adjacency) = self.load_csr_adjacency(
            node,
            visible_version,
            query.direction,
            query.edge_type.as_deref(),
        )?
        else {
            return Ok(None);
        };

        let ids: Vec<i64> = adjacency.into_iter().map(|(dst, _)| dst).collect();
        Ok(Some(Arc::from(ids.into_boxed_slice())))
    }

    pub(super) fn csr_neighbors_weighted_shared(
        &self,
        snapshot_id: SnapshotId,
        node: i64,
        query: &NeighborQuery,
    ) -> Result<Option<Arc<[(i64, f32)]>>, SqliteGraphError> {
        let visible_version = if snapshot_id == SnapshotId::current() {
            *self.current_snapshot_version.read()
        } else {
            snapshot_id.as_u64()
        };

        let Some(adjacency) = self.load_csr_adjacency(
            node,
            visible_version,
            query.direction,
            query.edge_type.as_deref(),
        )?
        else {
            return Ok(None);
        };

        Ok(Some(Arc::from(adjacency.into_boxed_slice())))
    }

    pub(super) fn rebuild_csr_runtime_views(
        &self,
        visible_version: u64,
    ) -> Result<(), SqliteGraphError> {
        let node_ids = self.get_all_node_ids()?;

        let (outgoing_rows, incoming_rows, distinct_types) = {
            let edge_store = self.edge_store.read();
            let edge_types = edge_store.edge_types_lock().read();
            let mut distinct_types = BTreeSet::new();

            let collect_rows =
                |direction: EdgeDirection| -> Result<Vec<CsrRuntimeRow>, SqliteGraphError> {
                    node_ids
                        .iter()
                        .map(|&node_id| {
                            let weighted = edge_store
                                .neighbors_weighted(node_id, direction)
                                .map_err(map_v3_error)?;
                            let entries = weighted
                                .iter()
                                .filter_map(|&(dst, weight)| {
                                    edge_types
                                        .get(&(node_id, dst, direction))
                                        .cloned()
                                        .map(|edge_type| (dst as u32, weight, edge_type))
                                })
                                .collect();
                            Ok((node_id, entries))
                        })
                        .collect()
                };

            let outgoing_rows = collect_rows(EdgeDirection::Outgoing)?;
            let incoming_rows = collect_rows(EdgeDirection::Incoming)?;

            for (_, entries) in outgoing_rows.iter().chain(incoming_rows.iter()) {
                for (_, _, edge_type) in entries {
                    distinct_types.insert(edge_type.clone());
                }
            }

            (outgoing_rows, incoming_rows, distinct_types)
        };

        let type_ids = self.ensure_csr_edge_type_ids(distinct_types.into_iter().collect())?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let conn = self.sqlite_conn.lock();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS csr_shards (
                shard_id INTEGER NOT NULL,
                node_id INTEGER NOT NULL,
                shard_data BLOB,
                version INTEGER NOT NULL DEFAULT 1,
                created_at INTEGER NOT NULL,
                visible_at INTEGER NOT NULL,
                PRIMARY KEY (shard_id, node_id, version)
            )",
            [],
        )
        .map_err(|e| {
            SqliteGraphError::connection(format!("Failed to ensure csr_shards table: {}", e))
        })?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS csr_edge_types (
                type_id INTEGER PRIMARY KEY,
                edge_type TEXT NOT NULL UNIQUE
            )",
            [],
        )
        .map_err(|e| {
            SqliteGraphError::connection(format!("Failed to ensure csr_edge_types table: {}", e))
        })?;

        let insert_rows = |shard_id: i64,
                           rows: &Vec<(i64, Vec<(u32, f32, String)>)>|
         -> Result<(), SqliteGraphError> {
            for (node_id, entries) in rows {
                let encoded_entries: Vec<(u32, f32, u32)> = entries
                    .iter()
                    .map(|&(dst, weight, ref edge_type)| {
                        let type_id = *type_ids.get(edge_type).expect("CSR edge type id missing");
                        (dst, weight, type_id)
                    })
                    .collect();
                let blob = encode_csr_adjacency(&encoded_entries);
                conn.execute(
                    "INSERT OR REPLACE INTO csr_shards
                     (shard_id, node_id, shard_data, version, created_at, visible_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![
                        shard_id,
                        *node_id,
                        blob,
                        visible_version as i64,
                        timestamp,
                        visible_version as i64
                    ],
                )
                .map_err(|e| {
                    SqliteGraphError::connection(format!("Failed to write CSR shard row: {}", e))
                })?;
            }
            Ok(())
        };

        insert_rows(CSR_SHARD_OUTGOING, &outgoing_rows)?;
        insert_rows(CSR_SHARD_INCOMING, &incoming_rows)?;

        Ok(())
    }

    pub(super) fn ensure_csr_edge_type_ids(
        &self,
        edge_types: Vec<String>,
    ) -> Result<HashMap<String, u32>, SqliteGraphError> {
        let conn = self.sqlite_conn.lock();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS csr_edge_types (
                type_id INTEGER PRIMARY KEY,
                edge_type TEXT NOT NULL UNIQUE
            )",
            [],
        )
        .map_err(|e| {
            SqliteGraphError::connection(format!("Failed to ensure csr_edge_types table: {}", e))
        })?;

        let mut existing = HashMap::new();
        let mut next_type_id = 1_u32;
        {
            let mut stmt = conn
                .prepare("SELECT type_id, edge_type FROM csr_edge_types")
                .map_err(|e| {
                    SqliteGraphError::query(format!("Failed to prepare CSR edge type scan: {}", e))
                })?;
            let rows = stmt
                .query_map([], |row| {
                    Ok((row.get::<_, i64>(0)? as u32, row.get::<_, String>(1)?))
                })
                .map_err(|e| {
                    SqliteGraphError::query(format!("Failed to scan CSR edge types: {}", e))
                })?;
            for row in rows {
                let (type_id, edge_type) = row.map_err(|e| {
                    SqliteGraphError::query(format!("Failed to read CSR edge type row: {}", e))
                })?;
                next_type_id = next_type_id.max(type_id.saturating_add(1));
                existing.insert(edge_type, type_id);
            }
        }

        for edge_type in edge_types {
            if existing.contains_key(&edge_type) {
                continue;
            }
            conn.execute(
                "INSERT INTO csr_edge_types (type_id, edge_type) VALUES (?1, ?2)",
                rusqlite::params![next_type_id as i64, edge_type],
            )
            .map_err(|e| {
                SqliteGraphError::connection(format!("Failed to insert CSR edge type: {}", e))
            })?;
            existing.insert(edge_type, next_type_id);
            next_type_id = next_type_id.saturating_add(1);
        }

        Ok(existing)
    }
}
