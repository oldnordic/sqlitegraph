use serde_json::json;
use sqlitegraph::{
    GraphEdge, GraphEntity, SqliteGraph,
    bfs::{bfs_neighbors, shortest_path},
};

fn build_graph(edges: &[(i64, i64)]) -> SqliteGraph {
    let graph = SqliteGraph::open_in_memory().expect("graph");
    for idx in 0..10 {
        let name = format!("node_{idx}");
        graph
            .insert_entity(&GraphEntity {
                id: 0,
                kind: "Node".to_string(),
                name,
                file_path: None,
                data: json!({}),
            })
            .unwrap();
    }
    for &(from, to) in edges {
        graph
            .insert_edge(&GraphEdge {
                id: 0,
                from_id: from,
                to_id: to,
                edge_type: "LINK".to_string(),
                data: json!({}),
            })
            .unwrap();
    }
    graph
}

#[test]
fn test_bfs_traversal_single_component() {
    let edges = vec![(1, 2), (2, 3), (3, 4), (4, 5)];
    let graph = build_graph(&edges);
    let visited = bfs_neighbors(&graph, 1, 10).expect("bfs");
    assert_eq!(visited, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_bfs_traversal_disconnected_graph() {
    let edges = vec![(1, 2), (2, 3), (6, 7)];
    let graph = build_graph(&edges);
    let visited = bfs_neighbors(&graph, 6, 10).expect("bfs");
    assert_eq!(visited, vec![6, 7]);
}

#[test]
fn test_shortest_path_exists() {
    let edges = vec![(1, 2), (2, 3), (1, 4), (4, 3)];
    let graph = build_graph(&edges);
    let path = shortest_path(&graph, 1, 3).expect("shortest");
    assert_eq!(path, Some(vec![1, 2, 3]));
}

#[test]
fn test_shortest_path_not_exists() {
    let edges = vec![(1, 2), (3, 4)];
    let graph = build_graph(&edges);
    let path = shortest_path(&graph, 1, 4).expect("shortest");
    assert_eq!(path, None);
}

#[test]
fn test_bfs_deterministic_with_same_insert_order() {
    let edges = vec![(1, 3), (1, 2), (2, 4), (2, 5)];
    let graph = build_graph(&edges);
    let visited = bfs_neighbors(&graph, 1, 10).expect("bfs");
    assert_eq!(visited, vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_bfs_deterministic_with_shuffled_insert_order() {
    let mut edges = vec![(2, 4), (1, 3), (1, 2), (2, 5)];
    let graph = build_graph(&edges);
    edges.reverse();
    let graph_b = build_graph(&edges);
    let visited_a = bfs_neighbors(&graph, 1, 10).expect("bfs A");
    let visited_b = bfs_neighbors(&graph_b, 1, 10).expect("bfs B");
    assert_eq!(visited_a, visited_b);
}

#[test]
fn test_bfs_respects_max_depth() {
    let edges = vec![(1, 2), (2, 3), (3, 4)];
    let graph = build_graph(&edges);
    let visited = bfs_neighbors(&graph, 1, 1).expect("bfs");
    assert_eq!(visited, vec![1, 2]);
}

#[test]
fn test_shortest_path_prefers_lexicographic_neighbors() {
    let edges = vec![(1, 2), (1, 3), (2, 4), (3, 4)];
    let graph = build_graph(&edges);
    let path = shortest_path(&graph, 1, 4).expect("shortest");
    assert_eq!(path, Some(vec![1, 2, 4]));
}
