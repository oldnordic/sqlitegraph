#![cfg(feature = "v2_experimental")]

use serde_json::json;
use sqlitegraph::backend::native::{
    NativeNodeId, NodeRecord, edge_store::EdgeStore, graph_file::GraphFile, graph_ops::native_bfs,
    node_store::NodeStore,
};
use tempfile::NamedTempFile;

const NODE_SLOT_SIZE: u64 = 4096;

fn setup_graph_file() -> (GraphFile, NamedTempFile) {
    let temp_file = NamedTempFile::new().expect("temp file");
    let mut graph_file = GraphFile::create(temp_file.path()).expect("create graph file");

    // CRITICAL FIX: Force buffer initialization to prevent cross-test contamination
    graph_file.invalidate_read_buffer();
    graph_file.flush_write_buffer().expect("Failed to flush initial buffers");

    (graph_file, temp_file)
}


fn node_slot_offset(graph_file: &GraphFile, node_id: NativeNodeId) -> u64 {
    graph_file.header().node_data_offset + ((node_id - 1) as u64 * NODE_SLOT_SIZE)
}

fn read_version_byte(graph_file: &mut GraphFile, node_id: NativeNodeId) -> u8 {
    let offset = node_slot_offset(graph_file, node_id);
    let mut buf = [0u8; 1];
    graph_file
        .read_bytes(offset, &mut buf)
        .expect("read version");
    buf[0]
}

fn read_cluster_metadata(graph_file: &mut GraphFile, node_id: NativeNodeId) -> (u64, u32, u32) {
    let offset = node_slot_offset(graph_file, node_id);
    let mut header = [0u8; 21];
    graph_file
        .read_bytes(offset, &mut header)
        .expect("read header");
    let kind_len = u16::from_be_bytes([header[13], header[14]]) as usize;
    let name_len = u16::from_be_bytes([header[15], header[16]]) as usize;
    let data_len = u32::from_be_bytes([header[17], header[18], header[19], header[20]]) as usize;
    let total_size = 21 + kind_len + name_len + data_len + 32;
    let mut buffer = vec![0u8; total_size];
    graph_file
        .read_bytes(offset, &mut buffer)
        .expect("read record");
    let meta_start = total_size - 32;
    let outgoing_offset =
        u64::from_be_bytes(buffer[meta_start..meta_start + 8].try_into().unwrap());
    let outgoing_size =
        u32::from_be_bytes(buffer[meta_start + 8..meta_start + 12].try_into().unwrap());
    let outgoing_count =
        u32::from_be_bytes(buffer[meta_start + 12..meta_start + 16].try_into().unwrap());
    (outgoing_offset, outgoing_size, outgoing_count)
}

#[test]
fn default_insert_uses_v2_version_byte() {
    let (mut graph_file, _tmp) = setup_graph_file();
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let node = NodeRecord::new(
            1,
            "Node".to_string(),
            "primary".to_string(),
            json!({"key": "value"}),
        );
        node_store.write_node(&node).expect("write node");
    }
    let version = read_version_byte(&mut graph_file, 1);
    assert_eq!(
        version, 2,
        "Default NodeStore::write_node should emit V2 records"
    );
}

#[test]
fn index_rebuild_uses_v2_index_only() {
    let (mut graph_file, _tmp) = setup_graph_file();
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        for i in 1..=3 {
            let node = NodeRecord::new(i, "Node".into(), format!("node_{i}"), json!({}));
            node_store.write_node(&node).expect("write node");
        }
        node_store
            .rebuild_v2_index()
            .expect("force v2 index rebuild");
    }
    let version = read_version_byte(&mut graph_file, 2);
    assert_eq!(
        version, 2,
        "Rebuilt node index should validate data in V2 layout"
    );
}

#[test]
fn adjacency_uses_clustered_metadata_by_default() {
    let (mut graph_file, _tmp) = setup_graph_file();
    let source_id = 1;
    {
        let mut node_store = NodeStore::new(&mut graph_file);
        let source = NodeRecord::new(source_id, "Node".into(), "source".into(), json!({}));
        let target = NodeRecord::new(2, "Node".into(), "target".into(), json!({}));
        node_store.write_node(&source).expect("write source");
        node_store.write_node(&target).expect("write target");

        // CRITICAL FIX: Force buffer flush before edge operations
        graph_file.flush_write_buffer().expect("Failed to flush after node writes");
        graph_file.invalidate_read_buffer();
    }
    {
        let edge = sqlitegraph::backend::native::EdgeRecord::new(
            1,
            source_id,
            2,
            "LINK".into(),
            json!({"weight": 1}),
        );
        let mut edge_store = EdgeStore::new(&mut graph_file);
        edge_store.write_edge(&edge).expect("write edge");

        // CRITICAL FIX: Force buffer flush after edge operations
        graph_file.flush_write_buffer().expect("Failed to flush after edge writes");
        graph_file.invalidate_read_buffer();
    }

    // CRITICAL FIX: Invalidate buffer before BFS operations
    graph_file.invalidate_read_buffer();
    native_bfs(&mut graph_file, source_id, 1).expect("run bfs");

    // CRITICAL FIX: Invalidate buffer before reading cluster metadata
    graph_file.invalidate_read_buffer();
    let (offset, size, count) = read_cluster_metadata(&mut graph_file, source_id);
    assert!(
        offset > 0 && size > 0 && count > 0,
        "Cluster metadata should be populated after adjacency ops"
    );
}
