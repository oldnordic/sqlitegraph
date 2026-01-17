use serde_json::json;
use sqlitegraph::{
    GraphEdge, GraphEntity, SqliteGraph,
    algo::{connected_components, find_cycles_limited, nodes_by_degree, pagerank, betweenness_centrality, label_propagation, louvain_communities},
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

// PageRank tests

#[test]
fn test_pagerank_cycle_graph() {
    let graph = SqliteGraph::open_in_memory().unwrap();
    let a = insert_entity(&graph, "A");
    let b = insert_entity(&graph, "B");
    let c = insert_entity(&graph, "C");

    // Create a 3-node cycle: A -> B -> C -> A
    insert_edge(&graph, a, b, "LINK");
    insert_edge(&graph, b, c, "LINK");
    insert_edge(&graph, c, a, "LINK");

    // Run PageRank with typical damping factor
    let scores = pagerank(&graph, 0.85, 20).expect("pagerank");

    // All nodes should have equal scores (~0.333) in a cycle
    assert_eq!(scores.len(), 3);
    assert!((scores[0].1 - 0.333).abs() < 0.01);
    assert!((scores[1].1 - 0.333).abs() < 0.01);
    assert!((scores[2].1 - 0.333).abs() < 0.01);
}

#[test]
fn test_pagerank_star_graph() {
    let graph = SqliteGraph::open_in_memory().unwrap();
    let center = insert_entity(&graph, "Center");
    let leaf1 = insert_entity(&graph, "Leaf1");
    let leaf2 = insert_entity(&graph, "Leaf2");
    let leaf3 = insert_entity(&graph, "Leaf3");

    // Create star: all leaves point to center
    insert_edge(&graph, leaf1, center, "LINK");
    insert_edge(&graph, leaf2, center, "LINK");
    insert_edge(&graph, leaf3, center, "LINK");

    let scores = pagerank(&graph, 0.85, 20).expect("pagerank");

    // Center should have highest score (receiving all links)
    assert_eq!(scores.len(), 4);
    assert_eq!(scores[0].0, center);
    assert!(scores[0].1 > scores[1].1); // Center > leaves
}

#[test]
fn test_pagerank_dangling_nodes() {
    let graph = SqliteGraph::open_in_memory().unwrap();
    let a = insert_entity(&graph, "A");
    let b = insert_entity(&graph, "B");
    let c = insert_entity(&graph, "C");

    // A -> B (C is dangling - no outgoing edges)
    insert_edge(&graph, a, b, "LINK");

    let scores = pagerank(&graph, 0.85, 20).expect("pagerank");

    // Should handle dangling nodes gracefully
    assert_eq!(scores.len(), 3);
    // All scores should be valid (not NaN, not infinite)
    for (_, score) in scores {
        assert!(score.is_finite());
        assert!(score > 0.0);
    }
}

// Betweenness Centrality tests

#[test]
fn test_betweenness_line_graph() {
    let graph = SqliteGraph::open_in_memory().unwrap();
    let a = insert_entity(&graph, "A");
    let b = insert_entity(&graph, "B");
    let c = insert_entity(&graph, "C");
    let d = insert_entity(&graph, "D");

    // Create line: A -> B -> C -> D
    insert_edge(&graph, a, b, "LINK");
    insert_edge(&graph, b, c, "LINK");
    insert_edge(&graph, c, d, "LINK");

    let centrality = betweenness_centrality(&graph).expect("betweenness");

    // Middle nodes (B, C) should have higher centrality than ends (A, D)
    assert_eq!(centrality.len(), 4);

    let centrality_map: std::collections::HashMap<i64, f64> =
        centrality.into_iter().collect();

    // B and C should have higher centrality than A and D
    assert!(centrality_map[&b] > centrality_map[&a]);
    assert!(centrality_map[&c] > centrality_map[&d]);
}

#[test]
fn test_betweenness_star_graph() {
    let graph = SqliteGraph::open_in_memory().unwrap();
    let center = insert_entity(&graph, "Center");
    let leaf1 = insert_entity(&graph, "Leaf1");
    let leaf2 = insert_entity(&graph, "Leaf2");
    let leaf3 = insert_entity(&graph, "Leaf3");

    // Create star: all paths go through center
    insert_edge(&graph, leaf1, center, "LINK");
    insert_edge(&graph, center, leaf2, "LINK");
    insert_edge(&graph, center, leaf3, "LINK");

    let centrality = betweenness_centrality(&graph).expect("betweenness");

    assert_eq!(centrality.len(), 4);

    // Center should have highest centrality (all paths go through it)
    assert_eq!(centrality[0].0, center);
    assert!(centrality[0].1 > 0.0);

    // Leaves should have zero or very low centrality
    let leaf_values: Vec<(i64, f64)> = centrality.into_iter()
        .filter(|(id, _)| *id != center)
        .collect();

    for (_, value) in leaf_values {
        assert!(value < centrality[0].1);
    }
}

#[test]
fn test_betweenness_disconnected() {
    let graph = SqliteGraph::open_in_memory().unwrap();
    let a = insert_entity(&graph, "A");
    let b = insert_entity(&graph, "B");
    let c = insert_entity(&graph, "C");
    let d = insert_entity(&graph, "D");

    // Two disconnected components: A -> B, C -> D
    insert_edge(&graph, a, b, "LINK");
    insert_edge(&graph, c, d, "LINK");

    let centrality = betweenness_centrality(&graph).expect("betweenness");

    // Should handle disconnected components gracefully
    assert_eq!(centrality.len(), 4);

    // All values should be valid (no NaN, no infinity)
    for (_, value) in &centrality {
        assert!(value.is_finite());
        assert!(*value >= 0.0);
    }
}

// Label Propagation tests

#[test]
fn test_label_propagation_disconnected() {
    let graph = SqliteGraph::open_in_memory().unwrap();
    let a = insert_entity(&graph, "A");
    let b = insert_entity(&graph, "B");
    let c = insert_entity(&graph, "C");
    let d = insert_entity(&graph, "D");
    let e = insert_entity(&graph, "E");
    let f = insert_entity(&graph, "F");

    // Create two disconnected triangles
    insert_edge(&graph, a, b, "LINK");
    insert_edge(&graph, b, c, "LINK");
    insert_edge(&graph, c, a, "LINK");

    insert_edge(&graph, d, e, "LINK");
    insert_edge(&graph, e, f, "LINK");
    insert_edge(&graph, f, d, "LINK");

    let communities = label_propagation(&graph, 10).expect("label propagation");

    // Should detect 2 communities
    assert_eq!(communities.len(), 2);

    // Each community should have 3 nodes
    assert_eq!(communities[0].len(), 3);
    assert_eq!(communities[1].len(), 3);
}

#[test]
fn test_label_propagation_clique() {
    let graph = SqliteGraph::open_in_memory().unwrap();
    let nodes: Vec<i64> = (0..5)
        .map(|_| insert_entity(&graph, "Node"))
        .collect();

    // Create fully connected graph (clique)
    for i in 0..nodes.len() {
        for j in (i+1)..nodes.len() {
            insert_edge(&graph, nodes[i], nodes[j], "LINK");
        }
    }

    let communities = label_propagation(&graph, 10).expect("label propagation");

    // Should detect 1 community (all nodes connected)
    assert_eq!(communities.len(), 1);
    assert_eq!(communities[0].len(), 5);
}

#[test]
fn test_label_propagation_line() {
    let graph = SqliteGraph::open_in_memory().unwrap();
    let a = insert_entity(&graph, "A");
    let b = insert_entity(&graph, "B");
    let c = insert_entity(&graph, "C");
    let d = insert_entity(&graph, "D");

    // Create line: A -> B -> C -> D
    insert_edge(&graph, a, b, "LINK");
    insert_edge(&graph, b, c, "LINK");
    insert_edge(&graph, c, d, "LINK");

    let communities = label_propagation(&graph, 10).expect("label propagation");

    // Line graphs tend to form 1-2 communities depending on convergence
    // The key is that it's deterministic and valid
    assert!(!communities.is_empty());
    assert!(communities.len() <= 4);

    // All nodes should be assigned
    let total_nodes: usize = communities.iter().map(|c| c.len()).sum();
    assert_eq!(total_nodes, 4);
}

// Louvain Community Detection tests

#[test]
fn test_louvain_barbell() {
    let graph = SqliteGraph::open_in_memory().unwrap();

    // Create two cliques (cliques of 3 nodes each)
    let clique1: Vec<i64> = (0..3)
        .map(|_| insert_entity(&graph, "C1"))
        .collect();
    let clique2: Vec<i64> = (0..3)
        .map(|_| insert_entity(&graph, "C2"))
        .collect();

    // Connect clique1 internally
    for i in 0..clique1.len() {
        for j in (i+1)..clique1.len() {
            insert_edge(&graph, clique1[i], clique1[j], "LINK");
        }
    }

    // Connect clique2 internally
    for i in 0..clique2.len() {
        for j in (i+1)..clique2.len() {
            insert_edge(&graph, clique2[i], clique2[j], "LINK");
        }
    }

    // Add bridge edge between cliques (barbell)
    insert_edge(&graph, clique1[0], clique2[0], "BRIDGE");

    let communities = louvain_communities(&graph, 10).expect("louvain");

    // Should detect 2 communities (the two cliques)
    assert_eq!(communities.len(), 2);

    // Each community should have 3 nodes
    assert_eq!(communities[0].len(), 3);
    assert_eq!(communities[1].len(), 3);
}

#[test]
fn test_louvain_star() {
    let graph = SqliteGraph::open_in_memory().unwrap();
    let center = insert_entity(&graph, "Center");
    let leaves: Vec<i64> = (0..4)
        .map(|_| insert_entity(&graph, "Leaf"))
        .collect();

    // Create star: all leaves connected to center
    for leaf in &leaves {
        insert_edge(&graph, *leaf, center, "LINK");
    }

    let communities = louvain_communities(&graph, 10).expect("louvain");

    // Star graph typically forms 1-2 communities
    // Key is deterministic, valid grouping
    assert!(!communities.is_empty());
    assert!(communities.len() <= 5);

    // All nodes should be assigned
    let total_nodes: usize = communities.iter().map(|c| c.len()).sum();
    assert_eq!(total_nodes, 5);
}

#[test]
fn test_louvain_convergence() {
    let graph = SqliteGraph::open_in_memory().unwrap();
    let a = insert_entity(&graph, "A");
    let b = insert_entity(&graph, "B");
    let c = insert_entity(&graph, "C");

    // Simple triangle
    insert_edge(&graph, a, b, "LINK");
    insert_edge(&graph, b, c, "LINK");
    insert_edge(&graph, c, a, "LINK");

    // Run with high max_iterations
    let communities1 = louvain_communities(&graph, 100).expect("louvain");

    // Run with low max_iterations
    let communities2 = louvain_communities(&graph, 5).expect("louvain");

    // Both should converge to same result (triangle is simple)
    assert_eq!(communities1.len(), communities2.len());

    // Triangle typically forms 1 community
    assert_eq!(communities1.len(), 1);
    assert_eq!(communities1[0].len(), 3);
}
