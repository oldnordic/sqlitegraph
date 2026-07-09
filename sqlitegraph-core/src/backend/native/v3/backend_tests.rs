use super::csr_support::encode_csr_adjacency;
use super::*;
use crate::backend::native::v3::{V3_FORMAT_VERSION, V3_MAGIC};
use tempfile::TempDir;

#[test]
fn test_v3_backend_create() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.graph");

    let backend = V3Backend::create(&db_path);
    assert!(backend.is_ok());
    assert!(db_path.exists());
}

#[test]
fn test_v3_backend_create_and_open() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.graph");

    // Create
    {
        let backend = V3Backend::create(&db_path).unwrap();
        assert!(!backend.is_wal_enabled());
    }

    // Open
    {
        let backend = V3Backend::open(&db_path).unwrap();
        assert_eq!(backend.header().magic, V3_MAGIC);
        assert_eq!(backend.header().version, V3_FORMAT_VERSION);
    }
}

#[test]
fn test_v3_backend_rejects_sqlite_base_path() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("reserved.sqlite");

    let create_error = match V3Backend::create(&db_path) {
        Ok(_) => panic!("expected .sqlite base path rejection"),
        Err(err) => err,
    };
    assert!(
        create_error
            .to_string()
            .contains("Base path must not use .sqlite extension"),
        "unexpected error: {create_error}"
    );

    std::fs::write(&db_path, b"not-a-v3-db").unwrap();
    let open_error = match V3Backend::open(&db_path) {
        Ok(_) => panic!("expected .sqlite base path rejection"),
        Err(err) => err,
    };
    assert!(
        open_error
            .to_string()
            .contains("Base path must not use .sqlite extension"),
        "unexpected error: {open_error}"
    );
}

#[cfg(feature = "turbovec")]
#[test]
fn test_hnsw_vector_search_normalizes_ids_and_supports_exact_override() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("hnsw.graph");
    let backend = V3Backend::create(&db_path).unwrap();
    let dimension = 128;

    backend.create_hnsw_index("vectors", dimension).unwrap();

    let make_vector = |seed: usize| -> Vec<f32> {
        (0..dimension)
            .map(|j| {
                let mut x = ((seed as u64) << 32) ^ j as u64 ^ 0x9E37_79B9_7F4A_7C15;
                x ^= x >> 30;
                x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
                x ^= x >> 27;
                x = x.wrapping_mul(0x94D0_49BB_1331_11EB);
                x ^= x >> 31;
                if x & 1 == 0 { -1.0 } else { 1.0 }
            })
            .collect()
    };

    for i in 0..100 {
        let node_id = 10_000 + i as i64;
        backend
            .insert_hnsw_vector(
                "vectors",
                &make_vector(i),
                Some(serde_json::json!({"node_id": node_id})),
            )
            .unwrap();
    }

    let small_result = backend
        .hnsw_vector_search("vectors", &make_vector(42), 1)
        .unwrap();
    assert!(
        (10_000..11_002).contains(&small_result[0].0),
        "expected normalized node_id, got {}",
        small_result[0].0
    );

    for i in 100..1002 {
        let node_id = 10_000 + i as i64;
        backend
            .insert_hnsw_vector(
                "vectors",
                &make_vector(i),
                Some(serde_json::json!({"node_id": node_id})),
            )
            .unwrap();
    }

    assert_eq!(backend.hnsw_embedding_count("vectors").unwrap(), 1002);
    assert!(backend.hnsw_turbovec_ready("vectors").unwrap());

    {
        let metadata = backend.hnsw_indexes.read();
        let index = metadata.get("vectors").unwrap();
        let mut turbovec = index.turbovec_index.lock().unwrap();
        *turbovec = None;
    }
    assert!(!backend.hnsw_turbovec_ready("vectors").unwrap());

    let exact_result = backend
        .hnsw_vector_search_with_config(
            "vectors",
            &make_vector(1001),
            1,
            HnswSearchConfig {
                force_exact: true,
                ef_search_override: Some(100),
            },
        )
        .unwrap();
    assert!(
        (10_000..11_002).contains(&exact_result[0].0),
        "expected normalized node_id, got {}",
        exact_result[0].0
    );
    assert!(
        !backend.hnsw_turbovec_ready("vectors").unwrap(),
        "force_exact should not rebuild the turbovec cache"
    );

    let turbovec_result = backend
        .hnsw_vector_search("vectors", &make_vector(1001), 1)
        .unwrap();
    assert!(
        (10_000..11_002).contains(&turbovec_result[0].0),
        "expected normalized node_id, got {}",
        turbovec_result[0].0
    );
    assert!(
        backend.hnsw_turbovec_ready("vectors").unwrap(),
        "default search should rebuild the turbovec cache when eligible"
    );
}

#[test]
fn test_hnsw_vector_search_rejects_invalid_ef_override() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("invalid_ef.graph");
    let backend = V3Backend::create(&db_path).unwrap();

    backend.create_hnsw_index("vectors", 64).unwrap();

    let error = backend
        .hnsw_vector_search_with_config(
            "vectors",
            &[0.0; 64],
            1,
            HnswSearchConfig {
                force_exact: true,
                ef_search_override: Some(0),
            },
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("Invalid ef_search_override"),
        "unexpected error: {error}"
    );
}

#[test]
fn test_v3_backend_insert_node() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.graph");

    let backend = V3Backend::create(&db_path).unwrap();

    let node_id = backend
        .insert_node(NodeSpec {
            kind: "Test".to_string(),
            name: "test_node".to_string(),
            file_path: None,
            data: serde_json::json!({"key": "value"}),
        })
        .unwrap();

    assert_eq!(node_id, 1);

    // Verify entity count
    let ids = backend.entity_ids().unwrap();
    assert_eq!(ids.len(), 1);
}

#[test]
fn test_v3_backend_batch_insert_node_persists_node_properties() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("batch_node.graph");
    let backend = V3Backend::create(&db_path).unwrap();

    let mut batch = backend.begin_batch();
    let node_id = batch
        .insert_node(NodeSpec {
            kind: "Batch".to_string(),
            name: "batched_node".to_string(),
            file_path: None,
            data: serde_json::json!({"mode": "batch"}),
        })
        .unwrap();
    batch.commit().unwrap();

    let props = backend.get_node_properties(node_id).unwrap();
    let Some((kind, name, data)) = props else {
        panic!("batched node properties missing");
    };
    assert_eq!(kind, "Batch");
    assert_eq!(name, "batched_node");
    assert_eq!(data["mode"], serde_json::json!("batch"));
}

#[test]
fn test_neighbors_shared_uses_csr_for_outgoing_unfiltered() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("csr_neighbors.graph");
    let backend = V3Backend::create(&db_path).unwrap();

    let n1 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n1".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n2 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n2".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n3 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n3".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_edge(EdgeSpec {
            from: n1,
            to: n2,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: n1,
            to: n3,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();

    let manual_version = backend.current_snapshot_version() + 1;
    let csr_blob = encode_csr_adjacency(&[(n3 as u32, 0.7, 0), (n2 as u32, 0.2, 0)]);
    let conn = backend.sqlite_conn.lock();
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
    .unwrap();
    conn.execute(
        "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
                 VALUES (?1, ?2, ?3, ?4, 0, ?4)",
        rusqlite::params![0_i64, n1, csr_blob, manual_version as i64],
    )
    .unwrap();
    drop(conn);
    backend.set_graph_version(manual_version);

    let neighbors = backend
        .neighbors_shared(
            SnapshotId::current(),
            n1,
            NeighborQuery {
                direction: BackendDirection::Outgoing,
                edge_type: None,
            },
        )
        .unwrap();
    assert_eq!(
        &*neighbors,
        &[n3, n2],
        "outgoing unfiltered path should prefer CSR"
    );

    let weighted = backend
        .neighbors_weighted_shared(
            SnapshotId::current(),
            n1,
            NeighborQuery {
                direction: BackendDirection::Outgoing,
                edge_type: None,
            },
        )
        .unwrap();
    assert_eq!(&*weighted, &[(n3, 0.7), (n2, 0.2)]);
}

#[test]
fn test_neighbors_shared_keeps_filtered_queries_on_edge_store() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("csr_fallback.graph");
    let backend = V3Backend::create(&db_path).unwrap();

    let n1 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n1".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n2 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n2".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n3 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n3".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_edge(EdgeSpec {
            from: n1,
            to: n2,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: n1,
            to: n3,
            edge_type: "USES".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();

    let csr_blob = encode_csr_adjacency(&[(n3 as u32, 0.7, 0), (n2 as u32, 0.2, 0)]);
    let conn = backend.sqlite_conn.lock();
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
    .unwrap();
    conn.execute(
        "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
                 VALUES (?1, ?2, ?3, 1, 0, 0)",
        rusqlite::params![0_i64, n1, csr_blob],
    )
    .unwrap();
    drop(conn);

    let filtered = backend
        .neighbors_shared(
            SnapshotId::current(),
            n1,
            NeighborQuery {
                direction: BackendDirection::Outgoing,
                edge_type: Some("CALLS".to_string()),
            },
        )
        .unwrap();
    assert_eq!(
        &*filtered,
        &[n2],
        "typed queries should stay on edge-store filtering"
    );
}

#[test]
fn test_bfs_uses_csr_for_outgoing_unfiltered() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("csr_bfs.graph");
    let backend = V3Backend::create(&db_path).unwrap();

    let n1 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n1".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n2 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n2".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n3 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n3".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n4 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n4".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_edge(EdgeSpec {
            from: n1,
            to: n2,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: n1,
            to: n3,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: n3,
            to: n4,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();

    let manual_version = backend.current_snapshot_version() + 1;
    let conn = backend.sqlite_conn.lock();
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
    .unwrap();
    conn.execute(
        "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
        rusqlite::params![
            0_i64,
            n1,
            encode_csr_adjacency(&[(n3 as u32, 0.7, 0), (n2 as u32, 0.2, 0)]),
            manual_version as i64
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
        rusqlite::params![
            0_i64,
            n3,
            encode_csr_adjacency(&[(n4 as u32, 0.9, 0)]),
            manual_version as i64
        ],
    )
    .unwrap();
    drop(conn);
    backend.set_graph_version(manual_version);

    let bfs = backend.bfs(SnapshotId::current(), n1, 2).unwrap();
    assert_eq!(bfs, vec![n1, n3, n2, n4]);
}

#[test]
fn test_shortest_path_uses_csr_for_outgoing_unfiltered() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("csr_shortest_path.graph");
    let backend = V3Backend::create(&db_path).unwrap();

    let n1 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n1".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n2 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n2".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n3 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n3".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n4 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n4".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_edge(EdgeSpec {
            from: n1,
            to: n2,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: n1,
            to: n3,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: n3,
            to: n4,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: n2,
            to: n4,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();

    let manual_version = backend.current_snapshot_version() + 1;
    let conn = backend.sqlite_conn.lock();
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
    .unwrap();
    conn.execute(
        "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
        rusqlite::params![
            0_i64,
            n1,
            encode_csr_adjacency(&[(n3 as u32, 0.9, 0), (n2 as u32, 0.2, 0)]),
            manual_version as i64
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
        rusqlite::params![
            0_i64,
            n3,
            encode_csr_adjacency(&[(n4 as u32, 0.8, 0)]),
            manual_version as i64
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
        rusqlite::params![
            0_i64,
            n2,
            encode_csr_adjacency(&[(n4 as u32, 0.1, 0)]),
            manual_version as i64
        ],
    )
    .unwrap();
    drop(conn);
    backend.set_graph_version(manual_version);

    let path = backend
        .shortest_path(SnapshotId::current(), n1, n4)
        .unwrap()
        .unwrap();
    assert_eq!(path, vec![n1, n3, n4]);
}

#[test]
fn test_k_hop_uses_csr_for_outgoing_unfiltered() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("csr_k_hop.graph");
    let backend = V3Backend::create(&db_path).unwrap();

    let n1 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n1".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n2 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n2".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n3 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n3".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n4 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n4".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_edge(EdgeSpec {
            from: n1,
            to: n2,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: n1,
            to: n3,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: n3,
            to: n4,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();

    let manual_version = backend.current_snapshot_version() + 1;
    let conn = backend.sqlite_conn.lock();
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
    .unwrap();
    conn.execute(
        "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
        rusqlite::params![
            0_i64,
            n1,
            encode_csr_adjacency(&[(n3 as u32, 0.7, 0), (n2 as u32, 0.2, 0)]),
            manual_version as i64
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
        rusqlite::params![
            0_i64,
            n3,
            encode_csr_adjacency(&[(n4 as u32, 0.9, 0)]),
            manual_version as i64
        ],
    )
    .unwrap();
    drop(conn);
    backend.set_graph_version(manual_version);

    let hop = backend
        .k_hop(SnapshotId::current(), n1, 2, BackendDirection::Outgoing)
        .unwrap();
    assert_eq!(hop, vec![n3, n2, n4]);
}

#[test]
fn test_node_degree_uses_csr_for_outgoing_unfiltered() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("csr_node_degree.graph");
    let backend = V3Backend::create(&db_path).unwrap();

    let n1 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n1".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n2 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n2".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n3 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n3".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_edge(EdgeSpec {
            from: n1,
            to: n2,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: n3,
            to: n1,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();

    let manual_version = backend.current_snapshot_version() + 1;
    let conn = backend.sqlite_conn.lock();
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
    .unwrap();
    conn.execute(
        "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
        rusqlite::params![
            0_i64,
            n1,
            encode_csr_adjacency(&[(n2 as u32, 0.7, 0), (n3 as u32, 0.3, 0)]),
            manual_version as i64
        ],
    )
    .unwrap();
    drop(conn);
    backend.set_graph_version(manual_version);

    let degree = backend.node_degree(SnapshotId::current(), n1).unwrap();
    assert_eq!(degree, (2, 1));
}

#[test]
fn test_neighbors_shared_uses_csr_for_incoming_unfiltered() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("csr_incoming_neighbors.graph");
    let backend = V3Backend::create(&db_path).unwrap();

    let n1 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n1".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n2 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n2".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n3 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n3".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_edge(EdgeSpec {
            from: n1,
            to: n3,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: n2,
            to: n3,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();

    let manual_version = backend.current_snapshot_version() + 1;
    let conn = backend.sqlite_conn.lock();
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
    .unwrap();
    conn.execute(
        "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
        rusqlite::params![
            1_i64,
            n3,
            encode_csr_adjacency(&[(n2 as u32, 0.9, 0), (n1 as u32, 0.4, 0)]),
            manual_version as i64
        ],
    )
    .unwrap();
    drop(conn);
    backend.set_graph_version(manual_version);

    let incoming = backend
        .neighbors_shared(
            SnapshotId::current(),
            n3,
            NeighborQuery {
                direction: BackendDirection::Incoming,
                edge_type: None,
            },
        )
        .unwrap();
    assert_eq!(&*incoming, &[n2, n1]);
}

#[test]
fn test_neighbors_shared_uses_csr_for_typed_outgoing() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("csr_typed_neighbors.graph");
    let backend = V3Backend::create(&db_path).unwrap();

    let n1 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n1".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n2 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n2".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n3 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n3".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();
    let n4 = backend
        .insert_node(NodeSpec {
            kind: "Fn".to_string(),
            name: "n4".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_edge(EdgeSpec {
            from: n1,
            to: n2,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: n1,
            to: n3,
            edge_type: "USES".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: n1,
            to: n4,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();

    let manual_version = backend.current_snapshot_version() + 1;
    let conn = backend.sqlite_conn.lock();
    let calls_type_id: i64 = conn
        .query_row(
            "SELECT type_id FROM csr_edge_types WHERE edge_type = 'CALLS'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let uses_type_id: i64 = conn
        .query_row(
            "SELECT type_id FROM csr_edge_types WHERE edge_type = 'USES'",
            [],
            |row| row.get(0),
        )
        .unwrap();
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
    .unwrap();
    conn.execute(
        "INSERT INTO csr_shards (shard_id, node_id, shard_data, version, created_at, visible_at)
             VALUES (?1, ?2, ?3, ?4, 0, ?4)",
        rusqlite::params![
            0_i64,
            n1,
            encode_csr_adjacency(&[
                (n4 as u32, 0.9, calls_type_id as u32),
                (n3 as u32, 0.7, uses_type_id as u32),
                (n2 as u32, 0.5, calls_type_id as u32),
            ]),
            manual_version as i64
        ],
    )
    .unwrap();
    drop(conn);
    backend.set_graph_version(manual_version);

    let typed = backend
        .neighbors_shared(
            SnapshotId::current(),
            n1,
            NeighborQuery {
                direction: BackendDirection::Outgoing,
                edge_type: Some("CALLS".to_string()),
            },
        )
        .unwrap();
    assert_eq!(&*typed, &[n4, n2]);
}

/// Test inserting a node with large data (>64 bytes) that requires external storage
/// This test verifies the fix for the bug where large node data would panic
#[test]
fn test_v3_backend_insert_node_with_large_data() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_large.graph");

    let backend = V3Backend::create(&db_path).unwrap();

    // Create node data that exceeds MAX_INLINE_DATA (64 bytes)
    // The inline buffer format is: [kind_len:1][kind][name_len:1][name][data]
    // So we need data that pushes total over 64 bytes
    let large_data = serde_json::json!({
        "path": "src/components/user/authentication/handlers/login.rs",
        "hash": "abcdef1234567890abcdef1234567890abcdef1234567890",
        "last_indexed_at": 1234567890_i64,
        "last_modified": 1234567890_i64,
        "metadata": {
            "language": "rust",
            "lines": 150,
            "size_bytes": 4096
        }
    });

    // This should NOT panic - it should use external storage
    let node_id = backend
        .insert_node(NodeSpec {
            kind: "File".to_string(),
            name: "login.rs".to_string(),
            file_path: Some("src/components/user/authentication/handlers/login.rs".to_string()),
            data: large_data,
        })
        .unwrap();

    assert_eq!(node_id, 1);

    // Verify entity count
    let ids = backend.entity_ids().unwrap();
    assert_eq!(ids.len(), 1);

    // Verify we can retrieve the node
    use crate::SnapshotId;
    let snapshot = SnapshotId::current();
    let node = backend.get_node(snapshot, node_id).unwrap();
    assert_eq!(node.kind, "File");
    assert_eq!(node.name, "login.rs");
}

#[test]
fn test_v3_backend_open_nonexistent() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("nonexistent.graph");

    let result = V3Backend::open(&db_path);
    assert!(result.is_err());
}

#[test]
fn test_query_nodes_by_kind() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.graph");

    let backend = V3Backend::create(&db_path).unwrap();

    // Insert nodes of different kinds
    backend
        .insert_node(NodeSpec {
            kind: "Function".to_string(),
            name: "func_a".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_node(NodeSpec {
            kind: "Function".to_string(),
            name: "func_b".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_node(NodeSpec {
            kind: "Class".to_string(),
            name: "class_a".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    use crate::SnapshotId;
    let snapshot = SnapshotId::current();

    // Query by kind
    let functions = backend.query_nodes_by_kind(snapshot, "Function").unwrap();
    assert_eq!(functions.len(), 2);

    let classes = backend.query_nodes_by_kind(snapshot, "Class").unwrap();
    assert_eq!(classes.len(), 1);

    let unknown = backend.query_nodes_by_kind(snapshot, "Unknown").unwrap();
    assert!(unknown.is_empty());
}

#[test]
fn test_query_nodes_by_name_pattern_exact() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.graph");

    let backend = V3Backend::create(&db_path).unwrap();

    backend
        .insert_node(NodeSpec {
            kind: "Function".to_string(),
            name: "func_a".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_node(NodeSpec {
            kind: "Function".to_string(),
            name: "func_b".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    use crate::SnapshotId;
    let snapshot = SnapshotId::current();

    // Exact match
    let results = backend
        .query_nodes_by_name_pattern(snapshot, "func_a")
        .unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn test_query_nodes_by_name_pattern_prefix() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.graph");

    let backend = V3Backend::create(&db_path).unwrap();

    backend
        .insert_node(NodeSpec {
            kind: "Function".to_string(),
            name: "func_a".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_node(NodeSpec {
            kind: "Function".to_string(),
            name: "func_b".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_node(NodeSpec {
            kind: "Class".to_string(),
            name: "class_a".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    use crate::SnapshotId;
    let snapshot = SnapshotId::current();

    // Prefix match
    let results = backend
        .query_nodes_by_name_pattern(snapshot, "func*")
        .unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn test_query_nodes_by_name_pattern_substring() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.graph");

    let backend = V3Backend::create(&db_path).unwrap();

    backend
        .insert_node(NodeSpec {
            kind: "Function".to_string(),
            name: "my_func_a".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    backend
        .insert_node(NodeSpec {
            kind: "Function".to_string(),
            name: "my_func_b".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    use crate::SnapshotId;
    let snapshot = SnapshotId::current();

    // Substring match
    let results = backend
        .query_nodes_by_name_pattern(snapshot, "*func*")
        .unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn test_index_persistence_across_open() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.graph");

    // Create and populate
    {
        let backend = V3Backend::create(&db_path).unwrap();
        backend
            .insert_node(NodeSpec {
                kind: "Function".to_string(),
                name: "func_a".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        backend.flush_to_disk().unwrap();
    }

    // Verify .v3index sidecar was created
    let index_path = crate::backend::native::v3::index_persistence::index_path_for_db(&db_path);
    assert!(
        index_path.exists(),
        "Index sidecar should be created on flush"
    );

    // Reopen and query
    {
        let backend = V3Backend::open(&db_path).unwrap();
        use crate::SnapshotId;
        let snapshot = SnapshotId::current();

        let results = backend.query_nodes_by_kind(snapshot, "Function").unwrap();
        assert_eq!(
            results.len(),
            1,
            "Kind index should be restored from sidecar"
        );
    }
}

#[test]
fn test_index_rebuild_on_open_without_sidecar() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.graph");

    // Create and populate
    {
        let backend = V3Backend::create(&db_path).unwrap();
        backend
            .insert_node(NodeSpec {
                kind: "Class".to_string(),
                name: "class_a".to_string(),
                file_path: None,
                data: serde_json::json!({}),
            })
            .unwrap();
        backend.flush_to_disk().unwrap();
    }

    // Delete the sidecar
    let index_path = crate::backend::native::v3::index_persistence::index_path_for_db(&db_path);
    let _ = std::fs::remove_file(&index_path);

    // Reopen — should rebuild indexes from node scan
    {
        let backend = V3Backend::open(&db_path).unwrap();
        use crate::SnapshotId;
        let snapshot = SnapshotId::current();

        let results = backend.query_nodes_by_kind(snapshot, "Class").unwrap();
        assert_eq!(
            results.len(),
            1,
            "Kind index should be rebuilt from node scan"
        );
    }
}

#[test]
fn test_v3_snapshot_import() {
    use std::io::Write;

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.graph");
    let backend = V3Backend::create(&db_path).unwrap();

    // Build a snapshot.json in the same JSONL format that
    // `recovery::dump_graph_to_path` produces on the sqlite-backend:
    // three entities, two edges, plus label/property records.
    let snapshot_path = temp_dir.path().join("snapshot.json");
    let mut file = std::fs::File::create(&snapshot_path).unwrap();
    writeln!(
            file,
            r#"{{"type":"entity","id":1,"kind":"Class","name":"NodeA","file_path":null,"data":{{"score":42}}}}"#
        )
        .unwrap();
    writeln!(
            file,
            r#"{{"type":"entity","id":2,"kind":"Class","name":"NodeB","file_path":"src/b.rs","data":{{"score":7}}}}"#
        )
        .unwrap();
    writeln!(
        file,
        r#"{{"type":"entity","id":3,"kind":"Method","name":"NodeC","file_path":null,"data":{{}}}}"#
    )
    .unwrap();
    writeln!(
            file,
            r#"{{"type":"edge","id":10,"from_id":1,"to_id":2,"edge_type":"calls","data":{{"weight":2.0}}}}"#
        )
        .unwrap();
    writeln!(
        file,
        r#"{{"type":"edge","id":11,"from_id":2,"to_id":3,"edge_type":"defines","data":{{}}}}"#
    )
    .unwrap();
    writeln!(file, r#"{{"type":"label","entity_id":1,"label":"public"}}"#).unwrap();
    writeln!(
        file,
        r#"{{"type":"property","entity_id":1,"key":"lang","value":"\"rust\""}}"#
    )
    .unwrap();
    drop(file);

    let meta = backend.snapshot_import(temp_dir.path()).unwrap();
    assert_eq!(meta.entities_imported, 3, "three entities should import");
    assert_eq!(meta.edges_imported, 2, "two edges should import");

    // Node and edge counts should match the import.
    let header = backend.header.read();
    assert_eq!(header.node_count, 3, "header node_count should be 3");
    assert_eq!(header.edge_count, 2, "header edge_count should be 2");
    drop(header);

    // Sample node properties should round-trip, including the merged
    // label ("_labels") and property ("lang") from the JSONL dump.
    let ids = backend.entity_ids().unwrap();
    assert_eq!(ids.len(), 3, "entity_ids should report 3 nodes");

    use crate::snapshot::SnapshotId;
    let node = backend.get_node(SnapshotId::current(), ids[0]).unwrap();
    assert_eq!(node.kind, "Class");
    assert_eq!(node.name, "NodeA");
    assert_eq!(node.data.get("score").and_then(|v| v.as_i64()), Some(42));
    assert_eq!(
        node.data
            .get("_labels")
            .and_then(|v| v.as_array())
            .map(|a| a.len()),
        Some(1),
        "merged label should be present in data"
    );
    assert_eq!(
        node.data.get("lang").and_then(|v| v.as_str()),
        Some("rust"),
        "merged property should be present in data"
    );
}

#[test]
fn test_v3_snapshot_import_missing_file() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.graph");
    let backend = V3Backend::create(&db_path).unwrap();
    let result = backend.snapshot_import(temp_dir.path());
    assert!(result.is_err(), "import without snapshot.json should fail");
}

#[test]
fn test_snapshot_creation() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.graph");
    let backend = V3Backend::create(&db_path).unwrap();

    // Create initial snapshot
    let snapshot_name = "snapshot_v1";
    let lsn_v1 = backend.create_snapshot(snapshot_name).unwrap();
    assert!(lsn_v1 > 0, "Snapshot LSN should be positive");

    // Insert a node after snapshot
    backend
        .insert_node(NodeSpec {
            kind: "Agent".to_string(),
            name: "agent1".to_string(),
            file_path: None,
            data: serde_json::json!({"version": 1}),
        })
        .unwrap();

    // Create second snapshot
    let snapshot_name_v2 = "snapshot_v2";
    let lsn_v2 = backend.create_snapshot(snapshot_name_v2).unwrap();
    assert!(lsn_v2 > lsn_v1, "Snapshot LSNs should increment");

    // Insert another node
    backend
        .insert_node(NodeSpec {
            kind: "Agent".to_string(),
            name: "agent2".to_string(),
            file_path: None,
            data: serde_json::json!({"version": 2}),
        })
        .unwrap();

    // Verify snapshot metadata persists
    let retrieved_lsn = backend.get_snapshot_lsn(snapshot_name);
    assert_eq!(retrieved_lsn, Some(lsn_v1), "Should retrieve stored LSN");

    let retrieved_lsn_v2 = backend.get_snapshot_lsn(snapshot_name_v2);
    assert_eq!(retrieved_lsn_v2, Some(lsn_v2), "Should retrieve second LSN");

    // List snapshots
    let snapshots = backend.list_snapshots();
    assert_eq!(snapshots.len(), 2, "Should have 2 snapshots");
    assert!(
        snapshots
            .iter()
            .any(|(name, lsn)| name == snapshot_name && *lsn == lsn_v1)
    );

    // Delete a snapshot
    backend.delete_snapshot(snapshot_name).unwrap();
    let snapshots_after_delete = backend.list_snapshots();
    assert_eq!(
        snapshots_after_delete.len(),
        1,
        "Should have 1 snapshot after delete"
    );
    assert!(
        !snapshots_after_delete
            .iter()
            .any(|(name, _)| name == snapshot_name)
    );

    // Delete non-existent snapshot should fail
    let result = backend.delete_snapshot("nonexistent");
    assert!(
        result.is_err(),
        "Deleting non-existent snapshot should fail"
    );
}

#[test]
fn test_snapshot_version_increments() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.graph");
    let backend = V3Backend::create(&db_path).unwrap();

    // Initial version
    let version_0 = backend.current_snapshot_version();
    assert_eq!(version_0, 1, "Initial version should be 1");

    // Insert node - version should increment
    backend
        .insert_node(NodeSpec {
            kind: "Node".to_string(),
            name: "node1".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    let version_1 = backend.current_snapshot_version();
    assert_eq!(version_1, 2, "Version should increment after insert");

    // Update node - version should increment
    let node_id = backend.entity_ids().unwrap()[0];
    backend
        .update_node(
            node_id,
            NodeSpec {
                kind: "Node".to_string(),
                name: "node1".to_string(),
                file_path: None,
                data: serde_json::json!({"updated": true}),
            },
        )
        .unwrap();

    let version_2 = backend.current_snapshot_version();
    assert_eq!(version_2, 3, "Version should increment after update");

    // Delete node - version should increment
    backend.delete_entity(node_id).unwrap();

    let version_3 = backend.current_snapshot_version();
    assert_eq!(version_3, 4, "Version should increment after delete");
}

#[test]
fn test_current_snapshot_validation() {
    use crate::SnapshotId;

    // SnapshotId(0) should always be valid (current state)
    let current = SnapshotId::current();
    assert!(V3Backend::require_current_snapshot(current).is_ok());

    // Historical snapshots should be valid
    let historical = SnapshotId::from_lsn(12345);
    assert!(V3Backend::require_current_snapshot(historical).is_ok());

    // Invalid snapshot should fail
    let invalid = SnapshotId::invalid();
    assert!(V3Backend::require_current_snapshot(invalid).is_err());
}
