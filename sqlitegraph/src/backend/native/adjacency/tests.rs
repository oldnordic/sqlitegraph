//! Tests for adjacency iteration functionality

use crate::backend::native::graph_file::GraphFile;
use crate::backend::native::node_store::NodeStore;
use crate::backend::native::types::*;
use super::{AdjacencyIterator, AdjacencyHelpers};

#[cfg(test)]
fn create_test_graph_file() -> (GraphFile, tempfile::NamedTempFile) {
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let path = temp_file.path();
    let graph_file = GraphFile::create(path).unwrap();
    (graph_file, temp_file)
}

#[test]
fn test_adjacency_iterator_empty() {
    let (mut graph_file, _temp_file) = create_test_graph_file();

    // Create a node with no edges
    let node = NodeRecord::new(
        1,
        "Test".to_string(),
        "node1".to_string(),
        serde_json::json!({}),
    );
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        node_store.write_node(&node).unwrap();
    }

    // Test outgoing iterator
    let iterator = AdjacencyIterator::new_outgoing(&mut graph_file, 1).unwrap();
    assert_eq!(iterator.total_count(), 0);
    assert!(iterator.is_complete());

    // Test incoming iterator
    let iterator = AdjacencyIterator::new_incoming(&mut graph_file, 1).unwrap();
    assert_eq!(iterator.total_count(), 0);
    assert!(iterator.is_complete());
}


#[test]
fn test_adjacency_validation() {
    let (mut graph_file, _temp_file) = create_test_graph_file();
    let mut node_store = NodeStore::new(&mut graph_file);

    // Create a node
    let node = NodeRecord::new(
        1,
        "Test".to_string(),
        "node1".to_string(),
        serde_json::json!({}),
    );
    node_store.write_node(&node).unwrap();

    // Validate adjacency (should pass for node with no edges)
    let result = AdjacencyHelpers::validate_node_adjacency(&mut graph_file, 1);
    assert!(result.is_ok());
}