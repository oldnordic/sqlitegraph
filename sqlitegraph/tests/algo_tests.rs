use serde_json::json;
use sqlitegraph::{
    GraphEdge, GraphEntity, SqliteGraph,
    algo::{connected_components, find_cycles_limited, nodes_by_degree},
};

fn insert_entity(graph: &SqliteGraph, name: &str) -> i64 {
    graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Node".into(),
            name: name.into(),
            file_path: None,
            data: json!({ "name": name }),
        })
        .expect("insert entity")
}

fn insert_edge(graph: &SqliteGraph, from: i64, to: i64, label: &str) {
    let _ = graph
        .insert_edge(&GraphEdge {
            id: 0,
            from_id: from,
            to_id: to,
            edge_type: label.into(),
            data: json!({ "label": label }),
        })
        .expect("insert edge");
}

#[test]
fn test_connected_components_returns_sorted_groups() {
    let graph = SqliteGraph::open_in_memory().unwrap();
    let a = insert_entity(&graph, "A");
    let b = insert_entity(&graph, "B");
    let c = insert_entity(&graph, "C");
    let d = insert_entity(&graph, "D");
    let e = insert_entity(&graph, "E");

    insert_edge(&graph, a, b, "LINK");
    insert_edge(&graph, b, c, "LINK");
    insert_edge(&graph, d, e, "LINK");

    let components = connected_components(&graph).expect("components");
    assert_eq!(components.len(), 2);
    assert_eq!(components[0], vec![a, b, c]);
    assert_eq!(components[1], vec![d, e]);
}

#[test]
fn test_find_cycles_limited_returns_deterministic_cycle() {
    let graph = SqliteGraph::open_in_memory().unwrap();
    let a = insert_entity(&graph, "A");
    let b = insert_entity(&graph, "B");
    let c = insert_entity(&graph, "C");

    insert_edge(&graph, a, b, "LINK");
    insert_edge(&graph, b, c, "LINK");
    insert_edge(&graph, c, a, "LINK");

    let cycles = find_cycles_limited(&graph, 1).expect("cycles");
    assert_eq!(cycles.len(), 1);
    assert_eq!(cycles[0], vec![a, b, c, a]);
}

#[test]
fn test_nodes_by_degree_orders_descending() {
    let graph = SqliteGraph::open_in_memory().unwrap();
    let a = insert_entity(&graph, "A");
    let b = insert_entity(&graph, "B");
    let c = insert_entity(&graph, "C");

    insert_edge(&graph, a, b, "LINK");
    insert_edge(&graph, a, c, "LINK");
    insert_edge(&graph, b, c, "LINK");
    insert_edge(&graph, c, a, "LINK");
    insert_edge(&graph, b, a, "LINK");

    let descending = nodes_by_degree(&graph, true).expect("degrees");
    assert_eq!(descending[0].0, a);
    assert!(descending[0].1 > descending[1].1);

    let ascending = nodes_by_degree(&graph, false).expect("degrees");
    assert_eq!(ascending.last().unwrap().0, a);
}
