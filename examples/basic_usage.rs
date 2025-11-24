use serde_json::json;
use sqlitegraph::{
    ReasoningConfig,
    backend::BackendDirection,
    graph::{GraphEdge, GraphEntity, SqliteGraph},
    pattern::{NodeConstraint, PatternLeg, PatternQuery},
};

fn main() {
    if let Err(err) = run() {
        eprintln!("basic_usage error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let graph = SqliteGraph::open_in_memory()?;
    let root = insert_function(&graph, "Entry");
    let helper = insert_function(&graph, "Helper");
    let leaf = insert_function(&graph, "Leaf");
    let ty = insert_struct(&graph, "Widget");

    insert_edge(&graph, root, helper, "CALLS");
    insert_edge(&graph, helper, leaf, "CALLS");
    insert_edge(&graph, leaf, ty, "USES");

    let query = graph.query();
    let neighbors = query.neighbors(root)?;
    println!("neighbors: {:?}", neighbors);

    let pattern = PatternQuery {
        root: Some(NodeConstraint::kind("Function")),
        legs: vec![
            PatternLeg {
                direction: BackendDirection::Outgoing,
                edge_type: Some("CALLS".into()),
                constraint: Some(NodeConstraint::kind("Function")),
            },
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
    };
    let matches = query.pattern_matches(root, &pattern)?;
    println!(
        "pattern matches: {:?}",
        matches.iter().map(|m| &m.nodes).collect::<Vec<_>>()
    );

    let reasoner = graph.reasoner();
    let results = reasoner.analyze(root, &pattern, &ReasoningConfig::default())?;
    if let Some(best) = results.first() {
        println!(
            "reasoning score={} path={:?} expansion={:?}",
            best.score, best.pattern_path, best.expansion
        );
    }
    Ok(())
}

fn insert_function(graph: &SqliteGraph, name: &str) -> i64 {
    graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Function".into(),
            name: name.into(),
            file_path: None,
            data: json!({"name": name}),
        })
        .expect("insert function")
}

fn insert_struct(graph: &SqliteGraph, name: &str) -> i64 {
    graph
        .insert_entity(&GraphEntity {
            id: 0,
            kind: "Struct".into(),
            name: name.into(),
            file_path: None,
            data: json!({"name": name}),
        })
        .expect("insert struct")
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
