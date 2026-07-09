use super::V3Backend;
use crate::SqliteGraphError;
use crate::backend::PubSubEvent;
use crate::backend::SubscriptionFilter;
use crate::backend::native::types::KvValue as V2KvValue;
use crate::backend::native::v3::kv_store::types::hash_key;
use crate::backend::native::v3::pubsub::types::{
    PubSubEvent as V3Event, SubscriberId, SubscriptionFilter as V3Filter,
};
use crate::backend::native::v3::wal::V3WALRecord;
use crate::backend::native::v3::{KvStore, KvValue, Publisher};
use crate::snapshot::SnapshotId;
use std::sync::mpsc::Receiver;
use std::time::{SystemTime, UNIX_EPOCH};

impl V3Backend {
    /// Check if KV store has been initialized
    pub fn is_kv_initialized(&self) -> bool {
        self.kv_store.read().is_some()
    }

    /// Check if Publisher has been initialized
    pub fn is_pubsub_initialized(&self) -> bool {
        self.publisher.read().is_some()
    }

    /// Get a value from the KV store
    ///
    /// Returns None if the key doesn't exist or has been deleted (tombstone).
    pub fn kv_get_v3(&self, snapshot_id: SnapshotId, key: &[u8]) -> Option<KvValue> {
        let kv_guard = self.kv_store.read();
        kv_guard.as_ref().and_then(|kv| {
            kv.get_at_snapshot(key, snapshot_id)
                .filter(|v| !matches!(v, KvValue::Null))
        })
    }

    /// Set a value in the KV store
    pub fn kv_set_v3(&self, key: Vec<u8>, value: KvValue, ttl_seconds: Option<u64>) {
        let version = self.current_kv_version();
        let mut kv_guard = self.kv_store.write();
        kv_guard
            .get_or_insert_with(KvStore::new)
            .set(key, value, ttl_seconds, version);
    }

    /// Delete a key from the KV store
    pub fn kv_delete_v3(&self, key: &[u8]) {
        let version = self.current_kv_version();
        let mut kv_guard = self.kv_store.write();
        kv_guard
            .get_or_insert_with(KvStore::new)
            .delete(key, version);
    }

    /// Prefix scan for keys in the KV store using V3 types
    pub fn kv_prefix_scan_v3(
        &self,
        snapshot_id: SnapshotId,
        prefix: &[u8],
    ) -> Vec<(Vec<u8>, KvValue)> {
        let kv_guard = self.kv_store.read();
        kv_guard
            .as_ref()
            .map(|kv| kv.prefix_scan(prefix, snapshot_id))
            .unwrap_or_default()
    }

    pub(super) fn kv_get_v2(
        &self,
        snapshot_id: SnapshotId,
        key: &[u8],
    ) -> Result<Option<V2KvValue>, SqliteGraphError> {
        let kv_guard = self.kv_store.read();
        let v3_value = kv_guard
            .as_ref()
            .and_then(|kv| kv.get_at_snapshot(key, snapshot_id));

        let v2_value = v3_value.and_then(|v| match v {
            KvValue::Null => None,
            KvValue::Integer(i) => Some(V2KvValue::Integer(i)),
            KvValue::Float(f) => Some(V2KvValue::Float(f)),
            KvValue::String(s) => Some(V2KvValue::String(s)),
            KvValue::Boolean(b) => Some(V2KvValue::Boolean(b)),
            KvValue::Bytes(b) => Some(V2KvValue::Bytes(b)),
            KvValue::Json(j) => Some(V2KvValue::Json(j)),
        });

        Ok(v2_value)
    }

    pub(super) fn kv_set_v2(
        &self,
        key: Vec<u8>,
        value: V2KvValue,
        ttl_seconds: Option<u64>,
    ) -> Result<(), SqliteGraphError> {
        let v3_value = match &value {
            V2KvValue::Null => KvValue::Null,
            V2KvValue::Integer(i) => KvValue::Integer(*i),
            V2KvValue::Float(f) => KvValue::Float(*f),
            V2KvValue::String(s) => KvValue::String(s.clone()),
            V2KvValue::Boolean(b) => KvValue::Boolean(*b),
            V2KvValue::Bytes(b) => KvValue::Bytes(b.clone()),
            V2KvValue::Json(j) => KvValue::Json(j.clone()),
        };

        let version = self.current_kv_version();
        let key_hash = hash_key(&key);

        {
            let mut kv_guard = self.kv_store.write();
            kv_guard.get_or_insert_with(KvStore::new).set(
                key.clone(),
                v3_value,
                ttl_seconds,
                version,
            );
        }

        if let Some(ref wal) = self.wal {
            let mut wal_guard = wal.write();
            let value_bytes = match &value {
                V2KvValue::Null => vec![],
                V2KvValue::Integer(i) => i.to_le_bytes().to_vec(),
                V2KvValue::Float(f) => f.to_le_bytes().to_vec(),
                V2KvValue::String(s) => s.clone().into_bytes(),
                V2KvValue::Boolean(b) => vec![if *b { 1 } else { 0 }],
                V2KvValue::Bytes(b) => b.clone(),
                V2KvValue::Json(j) => serde_json::to_vec(j).unwrap_or_default(),
            };
            let value_type = match &value {
                V2KvValue::Null => 0,
                V2KvValue::Integer(_) => 1,
                V2KvValue::Float(_) => 2,
                V2KvValue::String(_) => 3,
                V2KvValue::Boolean(_) => 4,
                V2KvValue::Bytes(_) => 5,
                V2KvValue::Json(_) => 6,
            };

            let record = V3WALRecord::KvSet {
                lsn: version,
                key,
                value_bytes,
                value_type,
                ttl_seconds,
                timestamp: Self::unix_timestamp_secs(),
            };
            wal_guard
                .append(&record)
                .map_err(|e| SqliteGraphError::connection(format!("WAL write failed: {:?}", e)))?;
        }

        self.emit_kv_changed(key_hash, version);
        Ok(())
    }

    pub(super) fn kv_delete_v2(&self, key: &[u8]) -> Result<(), SqliteGraphError> {
        let version = self.current_kv_version();

        {
            let mut kv_guard = self.kv_store.write();
            kv_guard
                .get_or_insert_with(KvStore::new)
                .delete(key, version);
        }

        if let Some(ref wal) = self.wal {
            let mut wal_guard = wal.write();
            let record = V3WALRecord::KvDelete {
                lsn: version,
                key: key.to_vec(),
                timestamp: Self::unix_timestamp_secs(),
            };
            wal_guard
                .append(&record)
                .map_err(|e| SqliteGraphError::connection(format!("WAL write failed: {:?}", e)))?;
        }

        self.emit_kv_changed(hash_key(key), version);
        Ok(())
    }

    pub(super) fn subscribe_pubsub(
        &self,
        filter: SubscriptionFilter,
    ) -> Result<(u64, Receiver<PubSubEvent>), SqliteGraphError> {
        let v3_filter = V3Filter {
            node_changes: filter.node_changes,
            edge_changes: filter.edge_changes,
            kv_changes: filter.kv_changes,
            snapshot_commits: filter.snapshot_commits,
        };

        let (sub_id, v3_rx) = {
            let mut pub_guard = self.publisher.write();
            pub_guard
                .get_or_insert_with(Publisher::new)
                .subscribe(v3_filter)
        };

        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            while let Ok(v3_event) = v3_rx.recv() {
                let event = match v3_event {
                    V3Event::NodeChanged {
                        node_id,
                        snapshot_id,
                    } => PubSubEvent::NodeChanged {
                        node_id,
                        snapshot_id,
                    },
                    V3Event::EdgeChanged {
                        edge_id,
                        from_node,
                        to_node,
                        snapshot_id,
                    } => PubSubEvent::EdgeChanged {
                        edge_id,
                        from_node,
                        to_node,
                        snapshot_id,
                    },
                    V3Event::KvChanged {
                        key_hash,
                        snapshot_id,
                    } => PubSubEvent::KvChanged {
                        key_hash,
                        snapshot_id,
                    },
                    V3Event::SnapshotCommitted { snapshot_id } => {
                        PubSubEvent::SnapshotCommitted { snapshot_id }
                    }
                };
                if tx.send(event).is_err() {
                    break;
                }
            }
        });

        Ok((sub_id.as_u64(), rx))
    }

    pub(super) fn unsubscribe_pubsub(&self, subscriber_id: u64) -> Result<bool, SqliteGraphError> {
        let pub_guard = self.publisher.read();
        let removed = pub_guard
            .as_ref()
            .map(|p| p.unsubscribe(SubscriberId::from_raw(subscriber_id)))
            .unwrap_or(false);
        Ok(removed)
    }

    pub(super) fn kv_prefix_scan_v2(
        &self,
        snapshot_id: SnapshotId,
        prefix: &[u8],
    ) -> Result<Vec<(Vec<u8>, V2KvValue)>, SqliteGraphError> {
        let v3_results = self.kv_prefix_scan_v3(snapshot_id, prefix);
        let v2_results = v3_results
            .into_iter()
            .filter_map(|(k, v)| {
                let v2_value = match v {
                    KvValue::Null => return None,
                    KvValue::Integer(i) => V2KvValue::Integer(i),
                    KvValue::Float(f) => V2KvValue::Float(f),
                    KvValue::String(s) => V2KvValue::String(s),
                    KvValue::Boolean(b) => V2KvValue::Boolean(b),
                    KvValue::Bytes(b) => V2KvValue::Bytes(b),
                    KvValue::Json(j) => V2KvValue::Json(j),
                };
                Some((k, v2_value))
            })
            .collect();
        Ok(v2_results)
    }

    fn current_kv_version(&self) -> u64 {
        if let Some(ref wal) = self.wal {
            wal.read().committed_lsn()
        } else {
            1
        }
    }

    fn emit_kv_changed(&self, key_hash: u64, snapshot_id: u64) {
        let mut pub_guard = self.publisher.write();
        pub_guard.get_or_insert_with(Publisher::new).emit(
            crate::backend::native::v3::pubsub::types::PubSubEvent::KvChanged {
                key_hash,
                snapshot_id,
            },
        );
    }

    fn unix_timestamp_secs() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}
