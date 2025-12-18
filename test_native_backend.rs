//! Test Native V2 backend functionality
use sqlitegraph::{GraphConfig, BackendKind, open_graph, NodeSpec, EdgeSpec};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Testing Native V2 backend...");

    // Use Native backend (V2)
    let mut cfg = GraphConfig::native();
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("test_v2.db");

    let graph = open_graph(&db_path, &cfg)?;
    println!("✅ Native V2 graph created successfully");

    // Insert some nodes
    let mut node_ids = Vec::new();
    for i in 0..10 {
        let node_spec = NodeSpec {
            kind: "TestNode".to_string(),
            name: format!("Node_{}", i),
            file_path: None,
            data: serde_json::json!({"index": i}),
        };
        let node_id = graph.insert_node(node_spec)?;
        node_ids.push(node_id);
        println!("✅ Inserted node {}: {}", i, node_id);
    }

    // Insert some edges
    for i in 0..20 {
        let from_idx = i % node_ids.len();
        let to_idx = (i + 1) % node_ids.len();

        let edge_spec = EdgeSpec {
            from_id: node_ids[from_idx],
            to_id: node_ids[to_idx],
            edge_type: "test_edge".to_string(),
            data: serde_json::json!({"edge_index": i}),
        };

        let edge_id = graph.insert_edge(edge_spec)?;
        println!("✅ Inserted edge {}: {} -> {} = {}", i, from_idx, to_idx, edge_id);
    }

    println!("🎉 Native V2 backend test completed successfully!");
    println!("📊 Created {} nodes and {} edges", node_ids.len(), 20);

    Ok(())
}