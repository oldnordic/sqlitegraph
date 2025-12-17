use sqlitegraph::backend::native::{GraphFile, NodeStore};
use sqlitegraph::backend::native::types::NativeNodeId;
use sqlitegraph::backend::{NodeSpec};
use tempfile::NamedTempFile;

#[test]
fn regression_v2_node_version_mismatch() -> Result<(), Box<dyn std::error::Error>> {
    // Create a native graph with default V2 settings
    let temp_file = NamedTempFile::new()?;
    let path = temp_file.path();

    let mut graph_file = GraphFile::create(path)?;
    let mut node_store = NodeStore::new(&mut graph_file);

    // Insert a node with non-empty data
    let node_spec = NodeSpec {
        kind: "TestNode".to_string(),
        name: "test_node".to_string(),
        file_path: None,
        data: serde_json::json!({"key": "value", "number": 42}),
    };

    let node_id = node_store.insert_node(&node_spec)?;

    // Try to read the node back - this should fail with "Invalid V2 node version 1"
    let result = node_store.read_node(node_id);

    match result {
        Ok(_) => panic!("Expected V2 node version error, but read succeeded"),
        Err(e) => {
            let error_msg = e.to_string();
            if !error_msg.contains("Invalid V2 node version 1") {
                panic!("Expected 'Invalid V2 node version 1', got: {}", error_msg);
            }
            println!("REGRESSION TEST CAPTURED EXPECTED ERROR: {}", error_msg);
        }
    }

    Ok(())
}