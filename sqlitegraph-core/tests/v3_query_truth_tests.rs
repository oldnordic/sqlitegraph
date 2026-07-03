//! V3 Backend Query Truth Audit Tests
//!
//! Tests that verify V3 backend behavior is honest about its capabilities.
//! Per the truth audit principle: stubbed methods must explicitly fail
//! rather than returning fake or incomplete results.

#![cfg(feature = "native-v3")]

use sqlitegraph::{
    SqliteGraphError,
    backend::{BackendDirection, GraphBackend, NodeSpec},
    pattern::{NodeConstraint, PatternLeg, PatternQuery},
};

// Note: V3Backend is conditionally compiled and only available with native-v3 feature
// We use it through the re-export from backend module
use sqlitegraph::backend::native::v3::V3Backend;

use tempfile::TempDir;

/// Helper to create a V3-backed graph
fn create_v3_backend() -> (TempDir, V3Backend) {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_v3.graph");
    let backend = V3Backend::create(&db_path).unwrap();
    (temp_dir, backend)
}

#[test]
fn test_v3_pattern_search_matches_kind_chain() {
    let (_temp_dir, backend) = create_v3_backend();

    let f1 = backend
        .insert_node(NodeSpec {
            kind: "Function".to_string(),
            name: "A_func".to_string(),
            file_path: None,
            data: serde_json::json!({"name": "A_func"}),
        })
        .unwrap();
    let f2 = backend
        .insert_node(NodeSpec {
            kind: "Function".to_string(),
            name: "B_func".to_string(),
            file_path: None,
            data: serde_json::json!({"name": "B_func"}),
        })
        .unwrap();
    let f3 = backend
        .insert_node(NodeSpec {
            kind: "Function".to_string(),
            name: "C_func".to_string(),
            file_path: None,
            data: serde_json::json!({"name": "C_func"}),
        })
        .unwrap();
    let s1 = backend
        .insert_node(NodeSpec {
            kind: "Struct".to_string(),
            name: "S_alpha".to_string(),
            file_path: None,
            data: serde_json::json!({"name": "S_alpha"}),
        })
        .unwrap();
    let s2 = backend
        .insert_node(NodeSpec {
            kind: "Struct".to_string(),
            name: "S_beta".to_string(),
            file_path: None,
            data: serde_json::json!({"name": "S_beta"}),
        })
        .unwrap();

    backend
        .insert_edge(sqlitegraph::backend::EdgeSpec {
            from: f1,
            to: f2,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(sqlitegraph::backend::EdgeSpec {
            from: f2,
            to: s1,
            edge_type: "USES".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(sqlitegraph::backend::EdgeSpec {
            from: f1,
            to: f3,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(sqlitegraph::backend::EdgeSpec {
            from: f3,
            to: s2,
            edge_type: "USES".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();
    backend
        .insert_edge(sqlitegraph::backend::EdgeSpec {
            from: f2,
            to: f3,
            edge_type: "CALLS".to_string(),
            data: serde_json::json!({}),
        })
        .unwrap();

    let pattern = PatternQuery {
        root: Some(NodeConstraint::kind("Function")),
        legs: vec![
            PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some("CALLS".into()),
                constraint: Some(NodeConstraint::kind("Function")),
            },
            PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some("USES".into()),
                constraint: Some(NodeConstraint::kind("Struct")),
            },
        ],
    };

    let matches = backend
        .pattern_search(sqlitegraph::SnapshotId::current(), f1, &pattern)
        .expect("pattern search should succeed");
    let sequences: Vec<Vec<i64>> = matches.into_iter().map(|m| m.nodes).collect();

    assert_eq!(sequences, vec![vec![f1, f2, s1], vec![f1, f3, s2]]);
}

#[test]
fn test_v3_snapshot_import_returns_unimplemented_error() {
    // BEFORE FIX: snapshot_import returned ImportMetadata with zeros (pretending success)
    // AFTER FIX: snapshot_import should return explicit Unsupported error
    let (_temp_dir, backend) = create_v3_backend();

    // Attempt snapshot import - should explicitly fail
    let result = backend.snapshot_import(_temp_dir.path());

    // Verify we get an explicit Unsupported error, not silent fake success
    match result {
        Err(SqliteGraphError::Unsupported(msg)) => {
            // Success! Error message should be informative
            assert!(
                msg.contains("snapshot_import"),
                "Error message should mention snapshot_import"
            );
            assert!(
                msg.contains("V3"),
                "Error message should mention V3 backend"
            );
        }
        Ok(metadata) => {
            panic!(
                "snapshot_import should NOT succeed with fake zeros! Got: {:?} \
                   This indicates the stub is still pretending to succeed.",
                metadata
            );
        }
        Err(other) => {
            panic!("Expected Unsupported error, got: {:?}", other);
        }
    }
}

#[test]
fn test_v3_query_nodes_by_name_pattern_substring_not_glob() {
    // This test documents the SEMANTIC_MISMATCH between V3 and SQLite:
    // - SQLite: GLOB pattern matching (wildcards: *, ?, [chars])
    // - V3: Substring matching (contains), case-sensitive
    let (_temp_dir, backend) = create_v3_backend();

    // Create nodes with specific names
    let _user = backend
        .insert_node(NodeSpec {
            kind: "User".to_string(),
            name: "SuperUser".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    let _admin = backend
        .insert_node(NodeSpec {
            kind: "Admin".to_string(),
            name: "UserAdmin".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    let _root = backend
        .insert_node(NodeSpec {
            kind: "Root".to_string(),
            name: "root".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    // V3 uses case-sensitive substring matching - "User" matches both "SuperUser" and "UserAdmin"
    let result = backend
        .query_nodes_by_name_pattern(sqlitegraph::SnapshotId::current(), "User")
        .unwrap();

    // With case-sensitive substring matching, we expect "SuperUser" AND "UserAdmin" to match
    assert_eq!(
        result.len(),
        2,
        "V3 substring matching should find both 'SuperUser' and 'UserAdmin'"
    );

    // If this were GLOB (SQLite), pattern "User" would match exactly "User" only
    // This test documents the behavioral difference
}

#[test]
fn test_v3_query_nodes_by_kind_uses_kind_index_correctly() {
    let (_temp_dir, backend) = create_v3_backend();

    // Create nodes of different kinds
    let _user1 = backend
        .insert_node(NodeSpec {
            kind: "User".to_string(),
            name: "alice".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    let _user2 = backend
        .insert_node(NodeSpec {
            kind: "User".to_string(),
            name: "bob".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    let _doc = backend
        .insert_node(NodeSpec {
            kind: "Document".to_string(),
            name: "test.txt".to_string(),
            file_path: None,
            data: serde_json::json!({}),
        })
        .unwrap();

    // Query by kind - results should be correct
    let result = backend
        .query_nodes_by_kind(sqlitegraph::SnapshotId::current(), "User")
        .unwrap();

    assert_eq!(result.len(), 2, "Should find exactly 2 User nodes");

    // Results are correct; current implementation uses the kind index.
}
