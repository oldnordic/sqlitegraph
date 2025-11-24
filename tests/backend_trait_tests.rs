use serde_json::json;
use sqlitegraph::backend::{
    BackendDirection, ChainStep, EdgeSpec, GraphBackend, NeighborQuery, NodeSpec,
    SqliteGraphBackend,
};
use sqlitegraph::pattern::{NodeConstraint, PatternLeg, PatternQuery};

fn sample_node(name: &str) -> NodeSpec {
    NodeSpec {
        kind: "Node".into(),
        name: name.into(),
        file_path: None,
        data: json!({ "name": name }),
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
