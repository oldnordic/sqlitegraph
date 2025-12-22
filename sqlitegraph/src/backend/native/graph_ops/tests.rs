//! Comprehensive test suite for graph operations

use super::super::{EdgeStore, NodeStore};
use super::*;
use crate::backend::native::clear_node_cache;
use crate::backend::{EdgeSpec, NodeSpec};
use tempfile::NamedTempFile;

fn create_test_graph_file() -> (GraphFile, NamedTempFile) {
    let temp_file = NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let graph_file = GraphFile::create(path).unwrap();
    (graph_file, temp_file)
}

#[test]
fn test_native_bfs_simple() {
    // Clear cache to ensure test isolation
    clear_node_cache();

    let (mut graph_file, _temp_file) = create_test_graph_file();

    // Create nodes
    let node1 = NodeRecord::new(
        1,
        "Test".to_string(),
        "node1".to_string(),
        serde_json::json!({}),
    );
    let node2 = NodeRecord::new(
        2,
        "Test".to_string(),
        "node2".to_string(),
        serde_json::json!({}),
    );
    let node3 = NodeRecord::new(
        3,
        "Test".to_string(),
        "node3".to_string(),
        serde_json::json!({}),
    );

    {
        let mut node_store = NodeStore::new(&mut graph_file);
        node_store.write_node(&node1).unwrap();
        node_store.write_node(&node2).unwrap();
        node_store.write_node(&node3).unwrap();
    }

    // Create edges: 1 -> 2 -> 3
    let edge1 = EdgeRecord::new(1, 1, 2, "test".to_string(), serde_json::json!({}));
    let edge2 = EdgeRecord::new(2, 2, 3, "test".to_string(), serde_json::json!({}));

    {
        let mut edge_store = EdgeStore::new(&mut graph_file);
        edge_store.write_edge(&edge1).unwrap();
        edge_store.write_edge(&edge2).unwrap();
    }

    let result = native_bfs(&mut graph_file, 1, 2).unwrap();
    assert!(
        result.contains(&2),
        "Expected to find node 2 in BFS result: {:?}",
        result
    );
    assert!(
        result.contains(&3),
        "Expected to find node 3 in BFS result: {:?}",
        result
    );
}

#[test]
fn test_native_shortest_path() {
    // Clear cache to ensure test isolation
    clear_node_cache();

    let (mut graph_file, _temp_file) = create_test_graph_file();

    // Create nodes
    let node1 = NodeRecord::new(
        1,
        "Test".to_string(),
        "node1".to_string(),
        serde_json::json!({}),
    );
    let node2 = NodeRecord::new(
        2,
        "Test".to_string(),
        "node2".to_string(),
        serde_json::json!({}),
    );
    let node3 = NodeRecord::new(
        3,
        "Test".to_string(),
        "node3".to_string(),
        serde_json::json!({}),
    );

    {
        let mut node_store = NodeStore::new(&mut graph_file);
        node_store.write_node(&node1).unwrap();
        node_store.write_node(&node2).unwrap();
        node_store.write_node(&node3).unwrap();
    }

    // Create edge: 1 -> 2 -> 3
    let edge1 = EdgeRecord::new(1, 1, 2, "test".to_string(), serde_json::json!({}));
    let edge2 = EdgeRecord::new(2, 2, 3, "test".to_string(), serde_json::json!({}));

    {
        let mut edge_store = EdgeStore::new(&mut graph_file);
        edge_store.write_edge(&edge1).unwrap();
        edge_store.write_edge(&edge2).unwrap();
    }

    let result = native_shortest_path(&mut graph_file, 1, 3).unwrap();
    assert!(result.is_some());
    let path = result.unwrap();
    assert_eq!(path, vec![1, 2, 3]);
}
