use serde_json::json;
use sqlitegraph::{
    backend::SqliteGraphBackend,
    graph::{GraphEdge, GraphEntity, SqliteGraph},
    subgraph::{Subgraph, SubgraphRequest, extract_subgraph, structural_signature},
};

#[test]
fn subgraph_captures_cycles_self_loops_and_disjoint_edges() {
    let GraphFixture {
        backend,
        root,
        caller,
        callee,
        branch,
        far,
        ..
    } = graph_fixture();
    let subgraph = extract_subgraph(&backend, request(root, 3)).unwrap();

    let mut expected_nodes = vec![root, caller, callee, branch, far];
    expected_nodes.sort_unstable();
    expected_nodes.dedup();
    assert_eq!(subgraph.nodes, expected_nodes);

    let mut expected_edges = vec![
        (root, caller, "CALLS".to_string()),
        (root, branch, "USES".to_string()),
        (caller, caller, "CALLS".to_string()),
        (caller, callee, "CALLS".to_string()),
        (callee, root, "CALLS".to_string()),
        (callee, branch, "USES".to_string()),
        (branch, far, "DEPENDS".to_string()),
    ];
    sort_edges(&mut expected_edges);
    assert_eq!(subgraph.edges, expected_edges);
}

#[test]
fn subgraph_depth_zero_returns_only_root() {
    let GraphFixture { backend, root, .. } = graph_fixture();
    let subgraph = extract_subgraph(&backend, request(root, 0)).unwrap();
    assert_eq!(subgraph.nodes, vec![root]);
    assert!(subgraph.edges.is_empty());
}

#[test]
fn subgraph_depth_one_limits_neighbors() {
    let GraphFixture {
        backend,
        root,
        caller,
        branch,
        ..
    } = graph_fixture();
    let subgraph = extract_subgraph(&backend, request(root, 1)).unwrap();

    let mut expected_nodes = vec![root, caller, branch];
    expected_nodes.sort_unstable();
    assert_eq!(subgraph.nodes, expected_nodes);

    let mut expected_edges = vec![
        (root, caller, "CALLS".to_string()),
        (root, branch, "USES".to_string()),
    ];
    sort_edges(&mut expected_edges);
    assert_eq!(subgraph.edges, expected_edges);
}

#[test]
fn subgraph_depth_n_traverses_full_component() {
    let GraphFixture {
        backend,
        root,
        caller,
        callee,
        branch,
        far,
        isolated_a,
        isolated_b,
    } = graph_fixture();
    let subgraph = extract_subgraph(&backend, request(root, 5)).unwrap();

    let mut expected_nodes = vec![root, caller, callee, branch, far];
    expected_nodes.sort_unstable();
    assert_eq!(subgraph.nodes, expected_nodes);
    assert!(!subgraph.nodes.contains(&isolated_a));
    assert!(!subgraph.nodes.contains(&isolated_b));
}

#[test]
fn subgraph_is_deterministic_across_runs() {
    let GraphFixture { backend, root, .. } = graph_fixture();
    let req = request(root, 4);
    let first = extract_subgraph(&backend, req.clone()).unwrap();
    let second = extract_subgraph(&backend, req).unwrap();
    assert_eq!(first, second);
}

#[test]
fn structural_signature_sorts_nodes_and_edges() {
    let subgraph = Subgraph {
        nodes: vec![3, 1, 2],
        edges: vec![
            (2, 3, "B".to_string()),
            (1, 2, "A".to_string()),
            (2, 3, "A".to_string()),
        ],
    };
    let signature = structural_signature(&subgraph);
    assert_eq!(signature, "N[1,2,3]|E[1->2:A,2->3:A,2->3:B]");
}

struct GraphFixture {
    backend: SqliteGraphBackend,
    root: i64,
    caller: i64,
    callee: i64,
    branch: i64,
    far: i64,
    isolated_a: i64,
    isolated_b: i64,
}

fn graph_fixture() -> GraphFixture {
    let graph = SqliteGraph::open_in_memory().expect("graph");
    let root = insert_node(&graph, "Fn", "root");
    let caller = insert_node(&graph, "Fn", "caller");
    let callee = insert_node(&graph, "Fn", "callee");
    let branch = insert_node(&graph, "Module", "branch");
    let far = insert_node(&graph, "Fn", "far");
    let isolated_a = insert_node(&graph, "Fn", "iso_a");
    let isolated_b = insert_node(&graph, "Fn", "iso_b");

    insert_edge(&graph, root, caller, "CALLS");
    insert_edge(&graph, caller, callee, "CALLS");
    insert_edge(&graph, callee, root, "CALLS");
    insert_edge(&graph, caller, caller, "CALLS");
    insert_edge(&graph, callee, branch, "USES");
    insert_edge(&graph, branch, far, "DEPENDS");
    insert_edge(&graph, root, branch, "USES");
    insert_edge(&graph, isolated_a, isolated_b, "CALLS");

    let backend = SqliteGraphBackend::from_graph(graph);
    GraphFixture {
        backend,
        root,
        caller,
        callee,
        branch,
        far,
        isolated_a,
        isolated_b,
    }
}

fn insert_node(graph: &SqliteGraph, kind: &str, name: &str) -> i64 {
    graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: kind.into(),
            name: name.into(),
            file_path: None,
            data: json!({}),
        })
        .unwrap()
}

fn insert_edge(graph: &SqliteGraph, from: i64, to: i64, edge_type: &str) {
    graph
        .insert_edge(&GraphEdge {
            id: 0,
            from_id: from,
            to_id: to,
            edge_type: edge_type.into(),
            data: json!({}),
        })
        .unwrap();
}

fn request(root: i64, depth: u32) -> SubgraphRequest {
    SubgraphRequest {
        root,
        depth,
        allowed_edge_types: Vec::new(),
        allowed_node_types: Vec::new(),
    }
}

fn sort_edges(edges: &mut [(i64, i64, String)]) {
    edges.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
}
