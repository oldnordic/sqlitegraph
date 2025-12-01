use rand::{Rng, seq::SliceRandom};
use serde_json::json;
use sqlitegraph::{graph::SqliteGraph, recovery::load_graph_from_reader, run_deep_safety_checks};

#[path = "fuzz_common.rs"]
mod fuzz_common;

#[test]
fn fuzz_recovery_loader_handles_random_inputs() {
    let iterations = fuzz_common::fuzz_iterations();
    let mut rng = fuzz_common::labeled_rng("recovery-fuzz");
    for _ in 0..iterations {
        let graph = SqliteGraph::open_in_memory().unwrap();
        let payload = random_dump_lines(&mut rng);
        let result = load_graph_from_reader(&graph, payload.as_bytes());
        match result {
            Ok(()) => {
                let report = run_deep_safety_checks(&graph).unwrap();
                assert_eq!(report.integrity_errors, 0);
            }
            Err(_) => {
                // On failure the loader should roll back completely.
                assert_eq!(graph.list_entity_ids().unwrap().len(), 0);
            }
        }
    }
}

fn random_dump_lines(rng: &mut impl Rng) -> String {
    let mut lines = Vec::new();
    let entity_count = rng.gen_range(1..=5);
    for idx in 0..entity_count {
        let record = json!({
            "type": "entity",
            "id": idx as i64 + 1,
            "kind": if rng.gen_bool(0.5) { "Module" } else { "Fn" },
            "name": format!("N{idx}"),
            "file_path": if rng.gen_bool(0.3) { Some(format!("file{idx}.rs")) } else { None },
            "data": { "seed": idx }
        });
        lines.push(record.to_string());
    }
    let edge_targets: Vec<i64> = (1..=entity_count as i64).collect();
    for _ in 0..rng.gen_range(0..entity_count) {
        let from = *edge_targets.choose(rng).unwrap();
        let to = *edge_targets.choose(rng).unwrap();
        let record = json!({
            "type": "edge",
            "id": rng.gen_range(1..=1000),
            "from_id": from,
            "to_id": to,
            "edge_type": if rng.gen_bool(0.5) { "CALLS" } else { "USES" },
            "data": {}
        });
        lines.push(record.to_string());
    }
    for &entity_id in edge_targets.iter() {
        if rng.gen_bool(0.4) {
            let label = json!({
                "type": "label",
                "entity_id": entity_id,
                "label": if rng.gen_bool(0.5) { "Fn" } else { "Module" }
            });
            lines.push(label.to_string());
        }
        if rng.gen_bool(0.4) {
            let property = json!({
                "type": "property",
                "entity_id": entity_id,
                "key": "role",
                "value": if rng.gen_bool(0.5) { "entry" } else { "helper" }
            });
            lines.push(property.to_string());
        }
    }
    if rng.gen_bool(0.1) {
        lines.push("{\"type\":\"entity\"".into()); // malformed line to ensure error path
    }
    lines.join("\n")
}
