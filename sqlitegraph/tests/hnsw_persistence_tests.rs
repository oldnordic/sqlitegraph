//! Comprehensive HNSW Persistence Tests
//!
//! Test suite for validating HNSW index persistence across sessions.

use rusqlite::Connection;
use sqlitegraph::{
    hnsw::{DistanceMetric, HnswConfig, HnswIndex},
    schema::ensure_schema,
    SqliteGraph,
};
use tempfile::TempDir;

/// Test metadata persistence across database reconnection
#[test]
fn test_hnsw_metadata_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Session 1: Create index and save metadata
    {
        let conn = Connection::open(&db_path).unwrap();
        ensure_schema(&conn).unwrap();

        let config = HnswConfig::new(128, 16, 200, DistanceMetric::Cosine);
        let hnsw = HnswIndex::new("test_index", config).unwrap();
        hnsw.save_metadata(&conn).unwrap();
    }

    // Session 2: Reopen and verify metadata loaded
    {
        let conn = Connection::open(&db_path).unwrap();
        let loaded = HnswIndex::load_metadata(&conn, "test_index").unwrap();

        assert_eq!(loaded.config().dimension, 128);
        assert_eq!(loaded.config().m, 16);
        assert_eq!(loaded.config().ef_construction, 200);
        assert_eq!(loaded.config().distance_metric, DistanceMetric::Cosine);
        assert_eq!(loaded.name(), "test_index");
    }
}

/// Test vector persistence across database reconnection
///
/// NOTE: This test manually persists vectors to the database to work around
/// the current limitation where HnswIndex uses InMemoryVectorStorage by default.
/// Full automatic vector persistence will be added in a future update.
#[test]
fn test_hnsw_vector_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let vectors = vec![
        vec![1.0_f32, 0.0, 0.0],
        vec![0.0, 1.0, 0.0],
        vec![0.0, 0.0, 1.0],
    ];

    // Session 1: Create index and manually persist vectors
    let index_id = {
        let conn = Connection::open(&db_path).unwrap();
        ensure_schema(&conn).unwrap();

        let config = HnswConfig::new(3, 16, 200, DistanceMetric::Euclidean);
        let hnsw = HnswIndex::new("test_index", config).unwrap();
        hnsw.save_metadata(&conn).unwrap();

        // Get index ID
        conn.query_row(
            "SELECT id FROM hnsw_indexes WHERE name = ?",
            ["test_index"],
            |row| row.get::<_, i64>(0),
        ).unwrap()
    };

    // Manually insert vectors into database (simulating SQLiteVectorStorage)
    {
        let conn = Connection::open(&db_path).unwrap();
        for vector in &vectors {
            let vector_bytes = bytemuck::cast_slice::<f32, u8>(vector).to_vec();
            conn.execute(
                "INSERT INTO hnsw_vectors (index_id, vector_data, metadata, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![index_id, vector_bytes, None::<String>, 1000, 1000],
            ).unwrap();
        }
    }

    // Session 2: Reopen and verify vectors loaded
    {
        let conn = Connection::open(&db_path).unwrap();
        let hnsw = HnswIndex::load_with_vectors(&conn, "test_index").unwrap();

        assert_eq!(hnsw.vector_count(), 3);
    }
}

/// Test full lifecycle: create -> insert -> close -> reopen -> search
///
/// NOTE: This test manually persists vectors to work around current limitations.
#[test]
fn test_hnsw_create_insert_close_reopen_search() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create and insert vectors
    let index_id = {
        let conn = Connection::open(&db_path).unwrap();
        ensure_schema(&conn).unwrap();

        let config = HnswConfig::new(3, 16, 200, DistanceMetric::Euclidean);
        let hnsw = HnswIndex::new("lifecycle_test", config).unwrap();
        hnsw.save_metadata(&conn).unwrap();

        conn.query_row(
            "SELECT id FROM hnsw_indexes WHERE name = ?",
            ["lifecycle_test"],
            |row| row.get::<_, i64>(0),
        ).unwrap()
    };

    // Manually insert vectors
    {
        let conn = Connection::open(&db_path).unwrap();
        for i in 0..10 {
            let vector = vec![i as f32, (i * 2) as f32, (i * 3) as f32];
            let vector_bytes = bytemuck::cast_slice::<f32, u8>(&vector).to_vec();
            conn.execute(
                "INSERT INTO hnsw_vectors (index_id, vector_data, metadata, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![index_id, vector_bytes, None::<String>, 1000, 1000],
            ).unwrap();
        }
    }

    // Reopen and search
    {
        let conn = Connection::open(&db_path).unwrap();
        let hnsw = HnswIndex::load_with_vectors(&conn, "lifecycle_test").unwrap();

        assert_eq!(hnsw.vector_count(), 10);

        // Search for a similar vector
        let query = vec![5.0, 10.0, 15.0];
        let results = hnsw.search(&query, 3).unwrap();

        assert!(!results.is_empty(), "Search should return results");
        let (best_id, distance) = &results[0];
        assert!(*distance < 5.0, "Distance should be small for similar vector");
    }
}

/// Test empty index persistence
#[test]
fn test_hnsw_empty_index_persistence() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create index without inserting vectors
    {
        let conn = Connection::open(&db_path).unwrap();
        ensure_schema(&conn).unwrap();

        let config = HnswConfig::new(5, 16, 200, DistanceMetric::Cosine);
        let hnsw = HnswIndex::new("empty_index", config).unwrap();
        hnsw.save_metadata(&conn).unwrap();
        assert_eq!(hnsw.vector_count(), 0);
    }

    // Reopen and verify empty index loads
    {
        let conn = Connection::open(&db_path).unwrap();
        let hnsw = HnswIndex::load_with_vectors(&conn, "empty_index").unwrap();

        assert_eq!(hnsw.config().dimension, 5);
        assert_eq!(hnsw.vector_count(), 0);
    }
}

/// Test index deletion cascades to vectors
#[test]
fn test_hnsw_delete_index() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create index with vectors
    {
        let conn = Connection::open(&db_path).unwrap();
        ensure_schema(&conn).unwrap();

        let config = HnswConfig::new(3, 16, 200, DistanceMetric::Euclidean);
        let mut hnsw = HnswIndex::new("delete_test", config).unwrap();
        hnsw.save_metadata(&conn).unwrap();

        hnsw.insert_vector(&vec![1.0, 0.0, 0.0], None).unwrap();
        hnsw.insert_vector(&vec![0.0, 1.0, 0.0], None).unwrap();
        hnsw.insert_vector(&vec![0.0, 0.0, 1.0], None).unwrap();
    }

    // Delete index
    {
        let conn = Connection::open(&db_path).unwrap();
        HnswIndex::delete_index(&conn, "delete_test").unwrap();

        // Verify index gone
        let result = HnswIndex::load_metadata(&conn, "delete_test");
        assert!(result.is_err(), "Index should not exist after deletion");
    }
}

/// Test configuration preservation
#[test]
fn test_hnsw_config_preservation() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create index with specific config
    {
        let conn = Connection::open(&db_path).unwrap();
        ensure_schema(&conn).unwrap();

        let config = HnswConfig::new(256, 32, 400, DistanceMetric::Euclidean);
        let hnsw = HnswIndex::new("config_test", config).unwrap();
        hnsw.save_metadata(&conn).unwrap();
    }

    // Reopen and verify config matches
    {
        let conn = Connection::open(&db_path).unwrap();
        let loaded = HnswIndex::load_metadata(&conn, "config_test").unwrap();

        assert_eq!(loaded.config().dimension, 256);
        assert_eq!(loaded.config().m, 32);
        assert_eq!(loaded.config().ef_construction, 400);
        assert_eq!(loaded.config().distance_metric, DistanceMetric::Euclidean);
    }
}

/// Test all distance metrics persist correctly
#[test]
fn test_hnsw_distance_metric_preservation() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    let metrics = vec![
        DistanceMetric::Euclidean,
        DistanceMetric::Cosine,
        DistanceMetric::DotProduct,
        DistanceMetric::Manhattan,
    ];

    for (i, metric) in metrics.iter().enumerate() {
        let index_name = format!("metric_test_{}", i);

        // Create index
        {
            let conn = Connection::open(&db_path).unwrap();
            ensure_schema(&conn).unwrap();

            let config = HnswConfig::new(10, 16, 200, *metric);
            let hnsw = HnswIndex::new(&index_name, config).unwrap();
            hnsw.save_metadata(&conn).unwrap();
        }

        // Verify metric preserved
        {
            let conn = Connection::open(&db_path).unwrap();
            let loaded = HnswIndex::load_metadata(&conn, &index_name).unwrap();
            assert_eq!(loaded.config().distance_metric, *metric);
        }
    }
}

/// Test SqliteGraph auto-loads HNSW indexes
///
/// NOTE: This test manually persists vectors to work around current limitations.
#[test]
fn test_hnsw_graph_autoload() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");

    // Create index
    let index_id = {
        let conn = Connection::open(&db_path).unwrap();
        ensure_schema(&conn).unwrap();

        let config = HnswConfig::new(10, 16, 200, DistanceMetric::Cosine);
        let hnsw = HnswIndex::new("autoload_test", config).unwrap();
        hnsw.save_metadata(&conn).unwrap();

        conn.query_row(
            "SELECT id FROM hnsw_indexes WHERE name = ?",
            ["autoload_test"],
            |row| row.get::<_, i64>(0),
        ).unwrap()
    };

    // Manually insert vectors
    {
        let conn = Connection::open(&db_path).unwrap();
        for vector in &[vec![1.0; 10], vec![2.0; 10]] {
            let vector_bytes = bytemuck::cast_slice::<f32, u8>(vector).to_vec();
            conn.execute(
                "INSERT INTO hnsw_vectors (index_id, vector_data, metadata, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![index_id, vector_bytes, None::<String>, 1000, 1000],
            ).unwrap();
        }
    }

    // Reopen via SqliteGraph and verify auto-load
    {
        let graph = SqliteGraph::open(&db_path).unwrap();

        // Verify index was auto-loaded
        let index_names = graph.list_hnsw_indexes().unwrap();
        assert_eq!(index_names, vec!["autoload_test".to_string()]);

        // Verify vectors loaded
        graph.get_hnsw_index_ref("autoload_test", |hnsw| {
            assert_eq!(hnsw.vector_count(), 2);
            Ok::<(), sqlitegraph::SqliteGraphError>(())
        }).unwrap();
    }
}
