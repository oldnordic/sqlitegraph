#![cfg(feature = "v2_experimental")]
use serde_json::json;
use sqlitegraph::backend::native::{GraphFile, NodeRecordV2, node_store::NodeStore};
use tempfile::TempDir;

/// Test that reproduces "Invalid node ID" error during BFS operations
/// This test should FAIL before the fix and PASS after the fix
#[test]
fn test_v2_native_bfs_invalid_node_id_regression() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_v2_bfs_nodes.db");

    let mut graph_file = GraphFile::create(&db_path).unwrap();
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        for id in 1..=3 {
            let node = NodeRecordV2::new(
                id,
                "Node".to_string(),
                format!("Node{id}"),
                json!({ "payload": id }),
            );
            node_store.write_node_v2(&node).unwrap();
        }
    }

    {
        let mut node_store = NodeStore::new(&mut graph_file);
        node_store.rebuild_v2_index().unwrap();
        for id in 1..=3 {
            let record = node_store
                .read_node_v2(id)
                .unwrap_or_else(|e| panic!("node {} failed with {:?}", id, e));
            assert_eq!(record.id, id);
            assert_eq!(record.name, format!("Node{id}"));
        }
    }
}

/// Test that reproduces the "Invalid node ID" error during k-hop operations
#[test]
fn test_v2_native_khop_invalid_node_id_regression() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_v2_khop_nodes.db");

    let mut graph_file = GraphFile::create(&db_path).unwrap();
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        for id in 1..=6 {
            let node = NodeRecordV2::new(
                id,
                "Node".to_string(),
                format!("Leaf{id}"),
                json!({ "index": id }),
            );
            node_store.write_node_v2(&node).unwrap();
        }
    }

    {
        let mut node_store = NodeStore::new(&mut graph_file);
        node_store.rebuild_v2_index().unwrap();
        for id in 1..=6 {
            let record = node_store.read_node_v2(id).unwrap();
            assert_eq!(record.id, id);
            assert_eq!(record.kind, "Node");
        }
    }
}

fn build_v2_node(id: i64, kind: &str, name: &str, edge_count: u32) -> NodeRecordV2 {
    let mut node = NodeRecordV2::new(
        id,
        kind.to_string(),
        name.to_string(),
        json!({"payload": id}),
    );
    node.set_outgoing_cluster(2048, 512, edge_count);
    node.set_incoming_cluster(4096, 256, edge_count / 2);
    node
}

#[test]
fn v2_node_store_roundtrip_preserves_cluster_metadata() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("v2_roundtrip.db");
    let mut graph_file = GraphFile::create(&db_path).unwrap();
    let mut node_store = NodeStore::new(&mut graph_file);

    let original = build_v2_node(1, "Function", "roundtrip_node", 6);
    node_store.write_node_v2(&original).unwrap();

    let loaded = node_store.read_node_v2(1).unwrap();
    assert_eq!(loaded.id, original.id);
    assert_eq!(loaded.kind, original.kind);
    assert_eq!(loaded.name, original.name);
    assert_eq!(loaded.data, original.data);
    assert_eq!(
        loaded.outgoing_cluster_offset,
        original.outgoing_cluster_offset
    );
    assert_eq!(loaded.outgoing_cluster_size, original.outgoing_cluster_size);
    assert_eq!(loaded.outgoing_edge_count, original.outgoing_edge_count);
    assert_eq!(
        loaded.incoming_cluster_offset,
        original.incoming_cluster_offset
    );
    assert_eq!(loaded.incoming_cluster_size, original.incoming_cluster_size);
    assert_eq!(loaded.incoming_edge_count, original.incoming_edge_count);
}

#[test]
fn v2_node_store_rebuilds_index_for_multiple_nodes() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("v2_rebuild.db");

    let mut graph_file = GraphFile::create(&db_path).unwrap();
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        node_store
            .write_node_v2(&build_v2_node(1, "Kind", "A", 4))
            .unwrap();
        node_store
            .write_node_v2(&build_v2_node(2, "Kind", "B", 2))
            .unwrap();
    }

    {
        let mut node_store = NodeStore::new(&mut graph_file);
        node_store.rebuild_v2_index().unwrap();

        let node_two = node_store.read_node_v2(2).unwrap();
        assert_eq!(node_two.name, "B");
        assert_eq!(node_two.outgoing_edge_count, 2);
    }
}
