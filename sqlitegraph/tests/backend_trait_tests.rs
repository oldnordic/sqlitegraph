use serde_json::json;
use sqlitegraph::{
    SqliteGraphError,
    backend::{
        BackendDirection, ChainStep, EdgeSpec, GraphBackend, NeighborQuery, NodeSpec,
        SqliteGraphBackend,
    },
    pattern::{NodeConstraint, PatternLeg, PatternQuery},
};

fn sample_node(name: &str) -> NodeSpec {
    NodeSpec {
        kind: "Node".into(),
        name: name.into(),
        file_path: None,
        data: json!({ "name": name }),
    }
}

fn sample_edge(from: i64, to: i64, edge_type: &str) -> EdgeSpec {
    EdgeSpec {
        from,
        to,
        edge_type: edge_type.into(),
        data: json!({}),
    }
}

#[test]
fn test_backend_inserts_and_neighbors() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let a = backend.insert_node(sample_node("A")).unwrap();
    let b = backend.insert_node(sample_node("B")).unwrap();
    let c = backend.insert_node(sample_node("C")).unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: a,
            to: b,
            edge_type: "LINK".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: a,
            to: c,
            edge_type: "LINK".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: c,
            to: a,
            edge_type: "CALL".into(),
            data: json!({}),
        })
        .unwrap();

    let outgoing = backend
        .neighbors(
            a,
            NeighborQuery {
                direction: BackendDirection::Outgoing,
                edge_type: Some("LINK".into()),
            },
        )
        .unwrap();
    assert_eq!(outgoing, vec![b, c]);

    let incoming = backend
        .neighbors(
            a,
            NeighborQuery {
                direction: BackendDirection::Incoming,
                edge_type: Some("CALL".into()),
            },
        )
        .unwrap();
    assert_eq!(incoming, vec![c]);
}

#[test]
fn test_backend_bfs_and_shortest_path() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let a = backend.insert_node(sample_node("A")).unwrap();
    let b = backend.insert_node(sample_node("B")).unwrap();
    let c = backend.insert_node(sample_node("C")).unwrap();
    let d = backend.insert_node(sample_node("D")).unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: a,
            to: b,
            edge_type: "LINK".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: b,
            to: c,
            edge_type: "LINK".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: c,
            to: d,
            edge_type: "LINK".into(),
            data: json!({}),
        })
        .unwrap();

    let bfs = backend.bfs(a, 2).unwrap();
    assert_eq!(bfs, vec![a, b, c]);

    let path = backend.shortest_path(a, d).unwrap();
    assert_eq!(path, Some(vec![a, b, c, d]));
}

#[test]
fn test_backend_degree_counts() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let a = backend.insert_node(sample_node("A")).unwrap();
    let b = backend.insert_node(sample_node("B")).unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: a,
            to: b,
            edge_type: "LINK".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: b,
            to: a,
            edge_type: "LINK".into(),
            data: json!({}),
        })
        .unwrap();

    let (out_a, in_a) = backend.node_degree(a).unwrap();
    assert_eq!((out_a, in_a), (1, 1));
}

#[test]
fn test_backend_multi_hop_and_chain_queries() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let a = backend.insert_node(sample_node("A")).unwrap();
    let b = backend.insert_node(sample_node("B")).unwrap();
    let c = backend.insert_node(sample_node("C")).unwrap();
    let d = backend.insert_node(sample_node("D")).unwrap();
    let e = backend.insert_node(sample_node("E")).unwrap();

    backend
        .insert_edge(EdgeSpec {
            from: a,
            to: b,
            edge_type: "CALLS".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: b,
            to: c,
            edge_type: "CALLS".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: c,
            to: d,
            edge_type: "USES".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: a,
            to: e,
            edge_type: "USES".into(),
            data: json!({}),
        })
        .unwrap();
    backend
        .insert_edge(EdgeSpec {
            from: e,
            to: d,
            edge_type: "CALLS".into(),
            data: json!({}),
        })
        .unwrap();

    let hops = backend
        .k_hop(a, 2, BackendDirection::Outgoing)
        .expect("k-hop");
    assert_eq!(hops, vec![b, e, c, d]);

    let filtered = backend
        .k_hop_filtered(a, 3, BackendDirection::Outgoing, &["CALLS"])
        .expect("filtered");
    assert_eq!(filtered, vec![b, c]);

    let chain = [
        ChainStep {
            direction: BackendDirection::Outgoing,
            edge_type: Some("CALLS".into()),
        },
        ChainStep {
            direction: BackendDirection::Outgoing,
            edge_type: Some("CALLS".into()),
        },
        ChainStep {
            direction: BackendDirection::Outgoing,
            edge_type: Some("USES".into()),
        },
    ];
    let matches = backend.chain_query(a, &chain).expect("chain");
    assert_eq!(matches, vec![d]);

    let pattern = PatternQuery {
        root: Some(NodeConstraint::kind("Node")),
        legs: vec![
            PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some("CALLS".into()),
                constraint: Some(NodeConstraint::kind("Node")),
            },
            PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some("CALLS".into()),
                constraint: Some(NodeConstraint::kind("Node")),
            },
            PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some("USES".into()),
                constraint: Some(NodeConstraint::name_prefix("D")),
            },
        ],
    };
    let pattern_matches = backend.pattern_search(a, &pattern).expect("pattern");
    let sequences: Vec<Vec<i64>> = pattern_matches.into_iter().map(|m| m.nodes).collect();
    assert_eq!(sequences, vec![vec![a, b, c, d]]);
}

#[test]
fn sqlite_backend_satisfies_trait_suite() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    run_trait_suite(&backend);
}

fn run_trait_suite(api: &impl GraphBackend) {
    let root = api.insert_node(sample_node("root")).unwrap();
    let mid = api.insert_node(sample_node("mid")).unwrap();
    let leaf = api.insert_node(sample_node("leaf")).unwrap();
    let module = api.insert_node(sample_node("module")).unwrap();

    api.insert_edge(EdgeSpec {
        from: root,
        to: mid,
        edge_type: "CALLS".into(),
        data: json!({}),
    })
    .unwrap();
    api.insert_edge(EdgeSpec {
        from: mid,
        to: leaf,
        edge_type: "CALLS".into(),
        data: json!({}),
    })
    .unwrap();
    api.insert_edge(EdgeSpec {
        from: leaf,
        to: module,
        edge_type: "USES".into(),
        data: json!({}),
    })
    .unwrap();
    api.insert_edge(EdgeSpec {
        from: root,
        to: module,
        edge_type: "USES".into(),
        data: json!({}),
    })
    .unwrap();
    api.insert_edge(EdgeSpec {
        from: leaf,
        to: root,
        edge_type: "LINK".into(),
        data: json!({}),
    })
    .unwrap();

    let entity = api.get_node(root).unwrap();
    assert_eq!(entity.name, "root");

    let calls = api
        .neighbors(
            root,
            NeighborQuery {
                direction: BackendDirection::Outgoing,
                edge_type: Some("CALLS".into()),
            },
        )
        .unwrap();
    assert_eq!(calls, vec![mid]);

    let uses_incoming = api
        .neighbors(
            module,
            NeighborQuery {
                direction: BackendDirection::Incoming,
                edge_type: Some("USES".into()),
            },
        )
        .unwrap();
    assert_eq!(uses_incoming, vec![root, leaf]);

    let bfs = api.bfs(root, 2).unwrap();
    assert_eq!(bfs, vec![root, mid, module, leaf]);

    let shortest = api.shortest_path(root, module).unwrap();
    assert_eq!(shortest, Some(vec![root, module]));

    let degrees = api.node_degree(root).unwrap();
    assert_eq!(degrees, (2, 1));

    let hops = api.k_hop(root, 2, BackendDirection::Outgoing).unwrap();
    assert_eq!(hops, vec![mid, module, leaf]);

    let allowed = vec!["CALLS"];
    let filtered = api
        .k_hop_filtered(root, 2, BackendDirection::Outgoing, &allowed)
        .unwrap();
    assert_eq!(filtered, vec![mid, leaf]);

    let chain = [
        ChainStep {
            direction: BackendDirection::Outgoing,
            edge_type: Some("CALLS".into()),
        },
        ChainStep {
            direction: BackendDirection::Outgoing,
            edge_type: Some("CALLS".into()),
        },
        ChainStep {
            direction: BackendDirection::Outgoing,
            edge_type: Some("USES".into()),
        },
    ];
    let chained = api.chain_query(root, &chain).unwrap();
    assert_eq!(chained, vec![module]);

    let pattern = PatternQuery {
        root: Some(NodeConstraint::kind("Node")),
        legs: vec![
            PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some("CALLS".into()),
                constraint: Some(NodeConstraint::kind("Node")),
            },
            PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some("CALLS".into()),
                constraint: Some(NodeConstraint::kind("Node")),
            },
            PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some("USES".into()),
                constraint: Some(NodeConstraint::kind("Node")),
            },
        ],
    };
    let matches = api.pattern_search(root, &pattern).unwrap();
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].nodes, vec![root, mid, leaf, module]);
}

// ============================================================================
// INSERT_NODE ERROR CASES
// ============================================================================

#[test]
fn test_insert_node_invalid_empty_kind() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let invalid_node = NodeSpec {
        kind: "".to_string(), // Empty kind
        name: "test".to_string(),
        file_path: None,
        data: json!({}),
    };

    let result = backend.insert_node(invalid_node);
    assert!(result.is_err());
    match result.unwrap_err() {
        SqliteGraphError::InvalidInput(_) => {} // Expected
        other => panic!("Expected InvalidInput error, got: {:?}", other),
    }
}

#[test]
fn test_insert_node_invalid_empty_name() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let invalid_node = NodeSpec {
        kind: "Test".to_string(),
        name: "".to_string(), // Empty name
        file_path: None,
        data: json!({}),
    };

    let result = backend.insert_node(invalid_node);
    assert!(result.is_err());
    match result.unwrap_err() {
        SqliteGraphError::InvalidInput(_) => {} // Expected
        other => panic!("Expected InvalidInput error, got: {:?}", other),
    }
}

#[test]
fn test_insert_node_duplicate_names() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");

    // Insert first node
    let id1 = backend.insert_node(sample_node("duplicate")).unwrap();
    assert!(id1 > 0);

    // Insert second node with same name - should get different ID
    let id2 = backend.insert_node(sample_node("duplicate")).unwrap();
    assert!(id2 > 0);
    assert_ne!(id1, id2); // Different IDs for same name

    // Verify both nodes exist and have same name
    let node1 = backend.get_node(id1).unwrap();
    let node2 = backend.get_node(id2).unwrap();
    assert_eq!(node1.name, "duplicate");
    assert_eq!(node2.name, "duplicate");
    assert_ne!(node1.id, node2.id);
}

#[test]
fn test_insert_node_large_data() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let large_data = json!({
        "large_array": vec![0; 1000],
        "nested": {
            "deep": {
                "value": "test"
            }
        }
    });

    let node_with_large_data = NodeSpec {
        kind: "LargeNode".to_string(),
        name: "large_test".to_string(),
        file_path: None,
        data: large_data,
    };

    let result = backend.insert_node(node_with_large_data);
    assert!(result.is_ok());
    let node_id = result.unwrap();

    // Verify we can retrieve the node with large data
    let retrieved = backend.get_node(node_id).unwrap();
    assert_eq!(retrieved.name, "large_test");
    assert_eq!(
        retrieved.data["large_array"].as_array().unwrap().len(),
        1000
    );
}

// ============================================================================
// GET_NODE ERROR CASES
// ============================================================================

#[test]
fn test_get_node_invalid_negative_id() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");

    let result = backend.get_node(-1);
    assert!(result.is_err());
    match result.unwrap_err() {
        SqliteGraphError::NotFound(_) => {} // Expected
        other => panic!("Expected NotFound error, got: {:?}", other),
    }
}

#[test]
fn test_get_node_invalid_zero_id() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");

    let result = backend.get_node(0);
    assert!(result.is_err());
    match result.unwrap_err() {
        SqliteGraphError::NotFound(_) => {} // Expected
        other => panic!("Expected NotFound error, got: {:?}", other),
    }
}

#[test]
fn test_get_node_nonexistent() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");

    let result = backend.get_node(99999);
    assert!(result.is_err());
    match result.unwrap_err() {
        SqliteGraphError::NotFound(_) => {} // Expected
        other => panic!("Expected NotFound error, got: {:?}", other),
    }
}

// ============================================================================
// INSERT_EDGE ERROR CASES
// ============================================================================

#[test]
fn test_insert_edge_invalid_empty_type() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let node1 = backend.insert_node(sample_node("A")).unwrap();
    let node2 = backend.insert_node(sample_node("B")).unwrap();

    let invalid_edge = EdgeSpec {
        from: node1,
        to: node2,
        edge_type: "".to_string(), // Empty edge type
        data: json!({}),
    };

    let result = backend.insert_edge(invalid_edge);
    assert!(result.is_err());
    match result.unwrap_err() {
        SqliteGraphError::InvalidInput(_) => {} // Expected
        other => panic!("Expected InvalidInput error, got: {:?}", other),
    }
}

#[test]
fn test_insert_edge_invalid_negative_from() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let node2 = backend.insert_node(sample_node("B")).unwrap();

    let invalid_edge = EdgeSpec {
        from: -1, // Negative from_id
        to: node2,
        edge_type: "TEST".to_string(),
        data: json!({}),
    };

    let result = backend.insert_edge(invalid_edge);
    assert!(result.is_err());
    match result.unwrap_err() {
        SqliteGraphError::InvalidInput(_) => {} // Expected
        other => panic!("Expected InvalidInput error, got: {:?}", other),
    }
}

#[test]
fn test_insert_edge_invalid_negative_to() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let node1 = backend.insert_node(sample_node("A")).unwrap();

    let invalid_edge = EdgeSpec {
        from: node1,
        to: -1, // Negative to_id
        edge_type: "TEST".to_string(),
        data: json!({}),
    };

    let result = backend.insert_edge(invalid_edge);
    assert!(result.is_err());
    match result.unwrap_err() {
        SqliteGraphError::InvalidInput(_) => {} // Expected
        other => panic!("Expected InvalidInput error, got: {:?}", other),
    }
}

// ============================================================================
// NEIGHBORS ERROR CASES AND EDGE CASES
// ============================================================================

#[test]
fn test_neighbors_invalid_node_id() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");

    let query = NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    };

    let result = backend.neighbors(-1, query);
    assert!(result.is_ok());
    let neighbors = result.unwrap();
    assert_eq!(neighbors, Vec::<i64>::new()); // Returns empty result for invalid ID
}

#[test]
fn test_neighbors_nonexistent_node() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");

    let query = NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    };

    let result = backend.neighbors(99999, query);
    assert!(result.is_ok());
    let neighbors = result.unwrap();
    assert_eq!(neighbors, Vec::<i64>::new()); // Returns empty result for non-existent ID
}

#[test]
fn test_neighbors_no_neighbors_outgoing() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let isolated = backend.insert_node(sample_node("isolated")).unwrap();

    let query = NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    };

    let neighbors = backend.neighbors(isolated, query).unwrap();
    assert_eq!(neighbors, Vec::<i64>::new());
}

#[test]
fn test_neighbors_no_neighbors_incoming() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let isolated = backend.insert_node(sample_node("isolated")).unwrap();

    let query = NeighborQuery {
        direction: BackendDirection::Incoming,
        edge_type: None,
    };

    let neighbors = backend.neighbors(isolated, query).unwrap();
    assert_eq!(neighbors, Vec::<i64>::new());
}

#[test]
fn test_neighbors_nonexistent_edge_type() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let a = backend.insert_node(sample_node("A")).unwrap();
    let b = backend.insert_node(sample_node("B")).unwrap();

    // Insert edge with different type
    backend.insert_edge(sample_edge(a, b, "LINK")).unwrap();

    let query = NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: Some("NONEXISTENT".to_string()),
    };

    let neighbors = backend.neighbors(a, query).unwrap();
    assert_eq!(neighbors, Vec::<i64>::new());
}

// ============================================================================
// BFS EDGE CASES AND ERROR CASES
// ============================================================================

#[test]
fn test_bfs_invalid_start_node() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");

    let result = backend.bfs(-1, 2);
    assert!(result.is_err());
    match result.unwrap_err() {
        SqliteGraphError::NotFound(_) => {} // Expected
        other => panic!("Expected NotFound error, got: {:?}", other),
    }
}

#[test]
fn test_bfs_nonexistent_start_node() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");

    let result = backend.bfs(99999, 2);
    assert!(result.is_err());
    match result.unwrap_err() {
        SqliteGraphError::NotFound(_) => {} // Expected
        other => panic!("Expected NotFound error, got: {:?}", other),
    }
}

#[test]
fn test_bfs_zero_depth() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let start = backend.insert_node(sample_node("start")).unwrap();

    let result = backend.bfs(start, 0);
    assert!(result.is_ok());
    let visited = result.unwrap();
    assert_eq!(visited, vec![start]); // Only the start node
}

#[test]
fn test_bfs_isolated_node() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let isolated = backend.insert_node(sample_node("isolated")).unwrap();

    let result = backend.bfs(isolated, 3);
    assert!(result.is_ok());
    let visited = result.unwrap();
    assert_eq!(visited, vec![isolated]); // Only the isolated node
}

// ============================================================================
// SHORTEST_PATH EDGE CASES AND ERROR CASES
// ============================================================================

#[test]
fn test_shortest_path_no_path_exists() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let a = backend.insert_node(sample_node("A")).unwrap();
    let b = backend.insert_node(sample_node("B")).unwrap();
    let c = backend.insert_node(sample_node("C")).unwrap();

    // Create disconnected components
    backend.insert_edge(sample_edge(a, b, "LINK")).unwrap();
    // C is isolated

    let result = backend.shortest_path(a, c);
    assert!(result.is_ok());
    let path = result.unwrap();
    assert_eq!(path, None); // No path exists
}

#[test]
fn test_shortest_path_same_node() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let node = backend.insert_node(sample_node("node")).unwrap();

    let result = backend.shortest_path(node, node);
    assert!(result.is_ok());
    let path = result.unwrap();
    // Behavior for same node - should return either None or [node]
    // Let's assert it's not an error and check what the actual implementation does
    assert!(path.is_none() || path.as_ref() == Some(&vec![node]));
}

#[test]
fn test_shortest_path_invalid_start() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let end = backend.insert_node(sample_node("end")).unwrap();

    let result = backend.shortest_path(-1, end);
    assert!(result.is_err());
    match result.unwrap_err() {
        SqliteGraphError::NotFound(_) => {} // Expected
        other => panic!("Expected NotFound error, got: {:?}", other),
    }
}

#[test]
fn test_shortest_path_invalid_end() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let start = backend.insert_node(sample_node("start")).unwrap();

    let result = backend.shortest_path(start, -1);
    assert!(result.is_err());
    match result.unwrap_err() {
        SqliteGraphError::NotFound(_) => {} // Expected
        other => panic!("Expected NotFound error, got: {:?}", other),
    }
}

// ============================================================================
// NODE_DEGREE EDGE CASES
// ============================================================================

#[test]
fn test_node_degree_isolated_node() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let isolated = backend.insert_node(sample_node("isolated")).unwrap();

    let result = backend.node_degree(isolated);
    assert!(result.is_ok());
    let (outgoing, incoming) = result.unwrap();
    assert_eq!((outgoing, incoming), (0, 0));
}

#[test]
fn test_node_degree_invalid_node() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");

    let result = backend.node_degree(-1);
    assert!(result.is_ok());
    let (outgoing, incoming) = result.unwrap();
    assert_eq!((outgoing, incoming), (0, 0)); // Returns (0,0) for invalid node
}

// ============================================================================
// K_HOP EDGE CASES
// ============================================================================

#[test]
fn test_k_hop_zero_depth() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let start = backend.insert_node(sample_node("start")).unwrap();
    let target = backend.insert_node(sample_node("target")).unwrap();

    backend
        .insert_edge(sample_edge(start, target, "LINK"))
        .unwrap();

    let result = backend.k_hop(start, 0, BackendDirection::Outgoing);
    assert!(result.is_ok());
    let hops = result.unwrap();
    // Zero depth returns empty result (actual behavior)
    assert_eq!(hops, Vec::<i64>::new());
}

#[test]
fn test_k_hop_isolated_node() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let isolated = backend.insert_node(sample_node("isolated")).unwrap();

    let result = backend.k_hop(isolated, 3, BackendDirection::Outgoing);
    assert!(result.is_ok());
    let hops = result.unwrap();
    assert_eq!(hops, Vec::<i64>::new()); // No hops from isolated node (actual behavior)
}

#[test]
fn test_k_hop_no_results() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let start = backend.insert_node(sample_node("start")).unwrap();
    let target = backend.insert_node(sample_node("target")).unwrap();

    backend
        .insert_edge(sample_edge(start, target, "LINK"))
        .unwrap();

    // Search for hops with different direction than the edge
    let result = backend.k_hop(start, 2, BackendDirection::Incoming);
    assert!(result.is_ok());
    let hops = result.unwrap();
    assert_eq!(hops, Vec::<i64>::new()); // No incoming hops (actual behavior)
}

// ============================================================================
// K_HOP_FILTERED EDGE CASES
// ============================================================================

#[test]
fn test_k_hop_filtered_empty_list() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let start = backend.insert_node(sample_node("start")).unwrap();
    let target = backend.insert_node(sample_node("target")).unwrap();

    backend
        .insert_edge(sample_edge(start, target, "LINK"))
        .unwrap();

    let result = backend.k_hop_filtered(start, 2, BackendDirection::Outgoing, &[]);
    assert!(result.is_ok());
    let hops = result.unwrap();
    assert_eq!(hops, Vec::<i64>::new()); // Empty list returns empty result (actual behavior)
}

#[test]
fn test_k_hop_filtered_no_matches() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let start = backend.insert_node(sample_node("start")).unwrap();
    let target = backend.insert_node(sample_node("target")).unwrap();

    backend
        .insert_edge(sample_edge(start, target, "LINK"))
        .unwrap();

    // Filter for different edge type
    let result = backend.k_hop_filtered(start, 2, BackendDirection::Outgoing, &["DIFFERENT"]);
    assert!(result.is_ok());
    let hops = result.unwrap();
    assert_eq!(hops, Vec::<i64>::new()); // No matches for different edge type (actual behavior)
}

// ============================================================================
// CHAIN_QUERY EDGE CASES
// ============================================================================

#[test]
fn test_chain_query_empty_chain() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let start = backend.insert_node(sample_node("start")).unwrap();

    let empty_chain: &[ChainStep] = &[];
    let result = backend.chain_query(start, empty_chain);
    assert!(result.is_ok());
    let matches = result.unwrap();
    assert_eq!(matches, vec![start]); // Empty chain should return start node
}

#[test]
fn test_chain_query_no_matches() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let start = backend.insert_node(sample_node("start")).unwrap();
    let target = backend.insert_node(sample_node("target")).unwrap();

    // Create edge with different type than what chain expects
    backend
        .insert_edge(sample_edge(start, target, "WRONG_TYPE"))
        .unwrap();

    let chain = [ChainStep {
        direction: BackendDirection::Outgoing,
        edge_type: Some("EXPECTED_TYPE".to_string()),
    }];

    let result = backend.chain_query(start, &chain);
    assert!(result.is_ok());
    let matches = result.unwrap();
    assert_eq!(matches, Vec::<i64>::new()); // No matches for expected edge type
}

#[test]
fn test_chain_query_invalid_start() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");

    let chain = [ChainStep {
        direction: BackendDirection::Outgoing,
        edge_type: Some("TEST".to_string()),
    }];

    let result = backend.chain_query(-1, &chain);
    assert!(result.is_ok());
    let matches = result.unwrap();
    assert_eq!(matches, Vec::<i64>::new()); // Invalid start returns empty results
}

// ============================================================================
// PATTERN_SEARCH EDGE CASES
// ============================================================================

#[test]
fn test_pattern_search_empty_pattern() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let start = backend.insert_node(sample_node("start")).unwrap();

    let empty_pattern = PatternQuery {
        root: None,
        legs: vec![],
    };

    let result = backend.pattern_search(start, &empty_pattern);
    assert!(result.is_ok());
    let matches = result.unwrap();
    // Empty pattern returns a match with just the start node (actual behavior)
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].nodes, vec![start]);
}

#[test]
fn test_pattern_search_no_matches() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let start = backend.insert_node(sample_node("start")).unwrap();
    let target = backend.insert_node(sample_node("target")).unwrap();

    backend
        .insert_edge(sample_edge(start, target, "LINK"))
        .unwrap();

    let pattern = PatternQuery {
        root: Some(NodeConstraint::name_prefix("nonexistent")),
        legs: vec![PatternLeg {
            direction: BackendDirection::Outgoing,
            edge_type: Some("LINK".to_string()),
            constraint: Some(NodeConstraint::name_prefix("nonexistent")),
        }],
    };

    let result = backend.pattern_search(start, &pattern);
    assert!(result.is_ok());
    let matches = result.unwrap();
    assert_eq!(matches, Vec::<sqlitegraph::pattern::PatternMatch>::new());
}

#[test]
fn test_pattern_search_invalid_start() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");

    let pattern = PatternQuery {
        root: Some(NodeConstraint::kind("Node")),
        legs: vec![PatternLeg {
            direction: BackendDirection::Outgoing,
            edge_type: Some("TEST".to_string()),
            constraint: Some(NodeConstraint::kind("Node")),
        }],
    };

    let result = backend.pattern_search(-1, &pattern);
    assert!(result.is_err());
    match result.unwrap_err() {
        SqliteGraphError::NotFound(_) => {} // Expected
        other => panic!("Expected NotFound error, got: {:?}", other),
    }
}

// ============================================================================
// DETERMINISTIC BEHAVIOR TESTS
// ============================================================================

#[test]
fn test_neighbors_deterministic_ordering() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let center = backend.insert_node(sample_node("center")).unwrap();

    // Insert multiple neighbors in non-sequential order
    let ids: Vec<i64> = (0..5)
        .map(|i| {
            backend
                .insert_node(sample_node(&format!("node{}", i)))
                .unwrap()
        })
        .collect();

    // Create edges to neighbors
    for &id in &ids {
        backend
            .insert_edge(sample_edge(center, id, "LINK"))
            .unwrap();
    }

    let query = NeighborQuery {
        direction: BackendDirection::Outgoing,
        edge_type: None,
    };

    // Run same query multiple times
    let result1 = backend.neighbors(center, query.clone()).unwrap();
    let result2 = backend.neighbors(center, query.clone()).unwrap();
    let result3 = backend.neighbors(center, query).unwrap();

    // Should be identical
    assert_eq!(result1, result2);
    assert_eq!(result2, result3);

    // Should contain all neighbor IDs
    let mut expected_ids = ids.clone();
    expected_ids.sort_unstable(); // Sort for comparison
    assert_eq!(result1, expected_ids);
}

#[test]
fn test_bfs_deterministic_ordering() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let start = backend.insert_node(sample_node("start")).unwrap();

    // Create a small graph
    let ids: Vec<i64> = (0..6)
        .map(|i| {
            backend
                .insert_node(sample_node(&format!("node{}", i)))
                .unwrap()
        })
        .collect();

    // Create edges to ensure deterministic BFS ordering
    backend
        .insert_edge(sample_edge(start, ids[0], "LINK"))
        .unwrap();
    backend
        .insert_edge(sample_edge(start, ids[1], "LINK"))
        .unwrap();
    backend
        .insert_edge(sample_edge(ids[0], ids[2], "LINK"))
        .unwrap();
    backend
        .insert_edge(sample_edge(ids[1], ids[3], "LINK"))
        .unwrap();
    backend
        .insert_edge(sample_edge(ids[2], ids[4], "LINK"))
        .unwrap();
    backend
        .insert_edge(sample_edge(ids[3], ids[5], "LINK"))
        .unwrap();

    // Run BFS multiple times
    let result1 = backend.bfs(start, 3).unwrap();
    let result2 = backend.bfs(start, 3).unwrap();
    let result3 = backend.bfs(start, 3).unwrap();

    // Should be identical
    assert_eq!(result1, result2);
    assert_eq!(result2, result3);
}

#[test]
fn test_k_hop_deterministic_ordering() {
    let backend = SqliteGraphBackend::in_memory().expect("backend");
    let start = backend.insert_node(sample_node("start")).unwrap();

    // Create a graph with multiple hops
    let ids: Vec<i64> = (0..9)
        .map(|i| {
            backend
                .insert_node(sample_node(&format!("node{}", i)))
                .unwrap()
        })
        .collect();

    // Create edges: start -> [0,1,2], [0,1,2] -> [3,4,5], [3,4,5] -> [6,7,8]
    for i in 0..3 {
        backend
            .insert_edge(sample_edge(start, ids[i], "LINK"))
            .unwrap();
        backend
            .insert_edge(sample_edge(ids[i], ids[i + 3], "LINK"))
            .unwrap();
        backend
            .insert_edge(sample_edge(ids[i + 3], ids[i + 6], "LINK"))
            .unwrap();
    }

    // Run k_hop multiple times
    let result1 = backend.k_hop(start, 3, BackendDirection::Outgoing).unwrap();
    let result2 = backend.k_hop(start, 3, BackendDirection::Outgoing).unwrap();
    let result3 = backend.k_hop(start, 3, BackendDirection::Outgoing).unwrap();

    // Should be identical
    assert_eq!(result1, result2);
    assert_eq!(result2, result3);
}
