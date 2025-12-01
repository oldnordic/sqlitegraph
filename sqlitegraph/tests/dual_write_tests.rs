use serde_json::json;
use sqlitegraph::backend::{EdgeSpec, NodeSpec};
use sqlitegraph::backend_selector::{BackendKind, GraphBackendFactory};
use sqlitegraph::dual_write::DualWriter;

fn dual_factory() -> GraphBackendFactory {
    GraphBackendFactory::new(BackendKind::Sqlite)
}

#[test]
fn test_dual_writer_inserts_nodes_and_edges() {
    let factory = dual_factory();
    let base = factory.new_sqlite().unwrap();
    let mirror = factory.new_sqlite().unwrap();
    let mut writer = DualWriter::new(base, mirror);

    let node_spec = NodeSpec {
        kind: "Node".into(),
        name: "A".into(),
        file_path: None,
        data: json!({}),
    };
    let ids = writer.insert_node(node_spec.clone()).unwrap();
    assert!(ids.base > 0 && ids.mirror > 0);

    let other_node = writer
        .insert_node(NodeSpec {
            kind: "Node".into(),
            name: "B".into(),
            file_path: None,
            data: json!({}),
        })
        .unwrap();
    let edge_spec = EdgeSpec {
        from: ids.base,
        to: other_node.base,
        edge_type: "LINK".into(),
        data: json!({}),
    };
    writer.insert_edge(edge_spec).unwrap();

    let stats = writer.stats();
    assert_eq!(stats.nodes_written, 2);
    assert_eq!(stats.edges_written, 1);
}
