use serde_json::json;
use sqlitegraph::backend::BackendDirection;
use sqlitegraph::pattern::{NodeConstraint, PatternLeg, PatternQuery};
use sqlitegraph::reasoning::{GraphReasoner, ReasoningConfig};
use sqlitegraph::{GraphEdge, GraphEntity, SqliteGraph};

fn insert_node(graph: &SqliteGraph, kind: &str, name: &str) -> i64 {
    graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: kind.into(),
            name: name.into(),
            file_path: None,
            data: json!({"name": name}),
        })
        .expect("insert node")
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
        .expect("insert edge");
}

fn reasoning_graph() -> (SqliteGraph, Vec<i64>) {
    let graph = SqliteGraph::open_in_memory().expect("graph");
    let f1 = insert_node(&graph, "Function", "entry");
    let f2 = insert_node(&graph, "Function", "helper_a");
    let f3 = insert_node(&graph, "Function", "helper_b");
    let s1 = insert_node(&graph, "Struct", "Alpha");
    let s2 = insert_node(&graph, "Struct", "Beta");
    let h1 = insert_node(&graph, "Type", "Leaf1");
    let h2 = insert_node(&graph, "Type", "Leaf2");
    let h3 = insert_node(&graph, "Type", "Leaf3");

    insert_edge(&graph, f1, f2, "CALLS");
    insert_edge(&graph, f2, s1, "USES");
    insert_edge(&graph, f1, f3, "CALLS");
    insert_edge(&graph, f3, s2, "USES");
    insert_edge(&graph, f2, f3, "CALLS");

    insert_edge(&graph, s1, h1, "REL");
    insert_edge(&graph, h1, h3, "REL");
    insert_edge(&graph, s2, h1, "REL");
    insert_edge(&graph, s2, h2, "REL");
    insert_edge(&graph, h2, h3, "REL");

    (graph, vec![f1, f2, f3, s1, s2, h1, h2, h3])
}

fn pattern() -> PatternQuery {
    PatternQuery {
        root: Some(NodeConstraint::kind("Function")),
        legs: vec![
            PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some("CALLS".into()),
                constraint: Some(NodeConstraint::kind("Function")),
            },
            PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some("USES".into()),
                constraint: Some(NodeConstraint::kind("Struct")),
            },
        ],
    }
}

#[test]
fn test_reasoner_ranks_candidates_by_score() {
    let (graph, ids) = reasoning_graph();
    let reasoner = graph.reasoner();
    let results = reasoner
        .analyze(ids[0], &pattern(), &ReasoningConfig::default())
        .expect("analysis");
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].pattern_path, vec![ids[0], ids[2], ids[4]]);
    assert_eq!(results[0].expansion, vec![ids[5], ids[6], ids[7]]);
    assert_eq!(results[1].pattern_path, vec![ids[0], ids[1], ids[3]]);
    assert_eq!(results[1].expansion, vec![ids[5], ids[7]]);
    assert!(results[0].score > results[1].score);
}

#[test]
fn test_reasoner_respects_expansion_depth() {
    let (graph, ids) = reasoning_graph();
    let reasoner = GraphReasoner::new(&graph);
    let config = ReasoningConfig {
        expansion_depth: 1,
        direction: BackendDirection::Outgoing,
    };
    let results = reasoner
        .analyze(ids[0], &pattern(), &config)
        .expect("analysis");
    assert_eq!(results[0].expansion, vec![ids[5], ids[6]]);
    assert_eq!(results[1].expansion, vec![ids[5]]);
}
