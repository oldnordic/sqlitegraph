use super::V3Backend;
use crate::SqliteGraphError;
use crate::backend::native::v3::wal::V3WALPaths;
use crate::backend::{
    BackupResult, EdgeSpec, GraphBackend, ImportMetadata, NodeSpec, SnapshotMetadata,
};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

impl V3Backend {
    pub(super) fn checkpoint_support(&self) -> Result<(), SqliteGraphError> {
        if let Some(ref wal) = self.wal {
            let header = self.header.read();
            let btree = self.btree.read();
            let allocator = self.allocator.read();

            wal.write()
                .checkpoint(
                    btree.root_page_id(),
                    allocator.total_pages(),
                    btree.tree_height(),
                    allocator.free_list_head(),
                    &header,
                )
                .map_err(|e| SqliteGraphError::connection(format!("Checkpoint failed: {:?}", e)))?;
        }

        Ok(())
    }

    pub(super) fn backup_support(
        &self,
        backup_dir: &Path,
    ) -> Result<BackupResult, SqliteGraphError> {
        std::fs::create_dir_all(backup_dir).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to create backup dir: {}", e))
        })?;

        let timestamp = Self::snapshot_unix_timestamp_secs();
        let backup_filename = format!("v3_backup_{}.graph", timestamp);
        let backup_path = backup_dir.join(&backup_filename);

        std::fs::copy(&self.db_path, &backup_path)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to copy database: {}", e)))?;

        let wal_path = V3WALPaths::wal_file(&self.db_path);
        if wal_path.exists() {
            let backup_wal_path = V3WALPaths::wal_file(&backup_path);
            std::fs::copy(&wal_path, &backup_wal_path)
                .map_err(|e| SqliteGraphError::connection(format!("Failed to copy WAL: {}", e)))?;
        }

        let metadata = std::fs::metadata(&backup_path).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to get backup metadata: {}", e))
        })?;

        Ok(BackupResult {
            snapshot_path: backup_path,
            manifest_path: backup_dir.join(format!("v3_backup_{}.manifest", timestamp)),
            size_bytes: metadata.len(),
            checksum: 0,
            record_count: self.header.read().node_count,
            duration_secs: 0.0,
            timestamp,
            checkpoint_performed: self.wal.is_some(),
        })
    }

    pub(super) fn snapshot_export_support(
        &self,
        export_dir: &Path,
    ) -> Result<SnapshotMetadata, SqliteGraphError> {
        std::fs::create_dir_all(export_dir).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to create export dir: {}", e))
        })?;

        let timestamp = Self::snapshot_unix_timestamp_secs();
        let snapshot_filename = format!("v3_snapshot_{}", timestamp);
        let snapshot_path = export_dir.join(&snapshot_filename);

        self.checkpoint_support()?;

        std::fs::copy(&self.db_path, &snapshot_path).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to export snapshot: {}", e))
        })?;

        let metadata = std::fs::metadata(&snapshot_path).map_err(|e| {
            SqliteGraphError::connection(format!("Failed to get snapshot metadata: {}", e))
        })?;

        let header = self.header.read();

        Ok(SnapshotMetadata {
            snapshot_path,
            size_bytes: metadata.len(),
            entity_count: header.node_count,
            edge_count: header.edge_count,
        })
    }

    pub(super) fn snapshot_import_support(
        &self,
        import_dir: &Path,
    ) -> Result<ImportMetadata, SqliteGraphError> {
        let snapshot_file = import_dir.join("snapshot.json");
        if !snapshot_file.exists() {
            return Err(SqliteGraphError::connection(format!(
                "Snapshot file not found: {}",
                snapshot_file.display()
            )));
        }

        let file = File::open(&snapshot_file)
            .map_err(|e| SqliteGraphError::connection(format!("Failed to open snapshot: {}", e)))?;
        let reader = BufReader::new(file);

        let mut labels_by_entity: HashMap<i64, Vec<String>> = HashMap::new();
        let mut props_by_entity: HashMap<i64, Vec<(String, serde_json::Value)>> = HashMap::new();
        let mut entity_records: Vec<serde_json::Value> = Vec::new();
        let mut edge_records: Vec<serde_json::Value> = Vec::new();

        for line in reader.lines() {
            let line = line.map_err(|e| {
                SqliteGraphError::invalid_input(format!("Failed to read line: {}", e))
            })?;
            if line.trim().is_empty() {
                continue;
            }
            let record: serde_json::Value = serde_json::from_str(&line).map_err(|e| {
                SqliteGraphError::invalid_input(format!("Failed to parse JSONL record: {}", e))
            })?;
            let rec_type = record.get("type").and_then(|v| v.as_str()).unwrap_or("");
            match rec_type {
                "entity" => entity_records.push(record),
                "edge" => edge_records.push(record),
                "label" => {
                    let entity_id = record
                        .get("entity_id")
                        .and_then(|v| v.as_i64())
                        .ok_or_else(|| {
                            SqliteGraphError::invalid_input(
                                "label record missing entity_id".to_string(),
                            )
                        })?;
                    let label = record
                        .get("label")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            SqliteGraphError::invalid_input(
                                "label record missing label".to_string(),
                            )
                        })?
                        .to_string();
                    labels_by_entity.entry(entity_id).or_default().push(label);
                }
                "property" => {
                    let entity_id = record
                        .get("entity_id")
                        .and_then(|v| v.as_i64())
                        .ok_or_else(|| {
                            SqliteGraphError::invalid_input(
                                "property record missing entity_id".to_string(),
                            )
                        })?;
                    let key = record
                        .get("key")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| {
                            SqliteGraphError::invalid_input(
                                "property record missing key".to_string(),
                            )
                        })?
                        .to_string();
                    let raw_value =
                        record
                            .get("value")
                            .and_then(|v| v.as_str())
                            .ok_or_else(|| {
                                SqliteGraphError::invalid_input(
                                    "property record missing value".to_string(),
                                )
                            })?;
                    let parsed: serde_json::Value = serde_json::from_str(raw_value)
                        .unwrap_or(serde_json::Value::String(raw_value.to_string()));
                    props_by_entity
                        .entry(entity_id)
                        .or_default()
                        .push((key, parsed));
                }
                "" => {
                    return Err(SqliteGraphError::invalid_input(
                        "JSONL record missing `type` field".to_string(),
                    ));
                }
                other => {
                    return Err(SqliteGraphError::invalid_input(format!(
                        "unknown JSONL record type: {other}"
                    )));
                }
            }
        }

        if entity_records.is_empty() && edge_records.is_empty() {
            return Ok(ImportMetadata {
                snapshot_path: snapshot_file,
                entities_imported: 0,
                edges_imported: 0,
            });
        }

        let mut id_map: HashMap<i64, i64> = HashMap::new();
        let mut entities_imported: u64 = 0;

        for rec in &entity_records {
            let original_id = rec.get("id").and_then(|v| v.as_i64()).ok_or_else(|| {
                SqliteGraphError::invalid_input("entity record missing id".to_string())
            })?;
            let kind = rec
                .get("kind")
                .and_then(|v| v.as_str())
                .unwrap_or("Entity")
                .to_string();
            let name = rec
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let file_path = rec
                .get("file_path")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let mut data = rec.get("data").cloned().unwrap_or(serde_json::Value::Null);

            let extra_labels = labels_by_entity.remove(&original_id);
            let extra_props = props_by_entity.remove(&original_id);
            if extra_labels.is_some() || extra_props.is_some() {
                if !data.is_object() {
                    data = serde_json::Value::Object(serde_json::Map::new());
                }
                if let Some(obj) = data.as_object_mut() {
                    if let Some(labels) = extra_labels {
                        obj.insert(
                            "_labels".to_string(),
                            serde_json::Value::Array(
                                labels.into_iter().map(serde_json::Value::String).collect(),
                            ),
                        );
                    }
                    if let Some(props) = extra_props {
                        for (k, v) in props {
                            obj.insert(k, v);
                        }
                    }
                }
            }

            let node_id = self.insert_node(NodeSpec {
                kind,
                name,
                file_path,
                data,
            })?;
            id_map.insert(original_id, node_id);
            entities_imported += 1;
        }

        let mut edges_imported: u64 = 0;
        for rec in &edge_records {
            let from_original = rec.get("from_id").and_then(|v| v.as_i64()).ok_or_else(|| {
                SqliteGraphError::invalid_input("edge record missing from_id".to_string())
            })?;
            let to_original = rec.get("to_id").and_then(|v| v.as_i64()).ok_or_else(|| {
                SqliteGraphError::invalid_input("edge record missing to_id".to_string())
            })?;
            let edge_type = rec
                .get("edge_type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let data = rec.get("data").cloned().unwrap_or(serde_json::Value::Null);

            let from = id_map.get(&from_original).copied().unwrap_or(from_original);
            let to = id_map.get(&to_original).copied().unwrap_or(to_original);

            self.insert_edge(EdgeSpec {
                from,
                to,
                edge_type,
                data,
            })?;
            edges_imported += 1;
        }

        Ok(ImportMetadata {
            snapshot_path: snapshot_file,
            entities_imported,
            edges_imported,
        })
    }

    fn snapshot_unix_timestamp_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}
