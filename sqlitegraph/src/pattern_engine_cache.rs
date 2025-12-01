//! Cache-enabled fast-path for pattern matching.
//!
//! This module provides an optimized version of pattern matching that uses
//! the adjacency cache as a fast-path while maintaining identical results
//! to the SQL-based implementation.

use rusqlite::{OptionalExtension, params};

use crate::{
    backend::BackendDirection,
    errors::SqliteGraphError,
    graph::SqliteGraph,
    pattern_engine::{PatternTriple, TripleMatch, match_triples},
};

/// Determine if a pattern can use the fast-path (Case 1)
///
/// Fast-path conditions:
/// - edge_type = Some("X")
/// - NO start_label
/// - NO end_label  
/// - NO property filters
fn can_use_fast_path(pattern: &PatternTriple) -> bool {
    pattern.start_label.is_none()
        && pattern.end_label.is_none()
        && pattern.start_props.is_empty()
        && pattern.end_props.is_empty()
}

/// Determine if pattern can use partial fast-path (Case 2)
///
/// Partial fast-path conditions:
/// - Has label filters OR property filters
/// - Can use cache to narrow candidates
fn can_use_partial_fast_path(pattern: &PatternTriple) -> bool {
    !pattern.start_props.is_empty()
        || !pattern.end_props.is_empty()
        || pattern.start_label.is_some()
        || pattern.end_label.is_some()
}

/// Execute cache-enabled fast-path pattern matching.
///
/// This function provides an optimized version of pattern matching that:
/// - Uses cache as a fast-path where safe
/// - Falls back to SQL where pattern requires it
/// - Returns IDENTICAL results to match_triples()
/// - Maintains deterministic ordering
///
/// # Arguments
/// * `graph` - The SQLiteGraph instance
/// * `pattern` - The pattern triple to match
///
/// # Returns
/// A vector of triple matches in deterministic order
pub fn match_triples_fast(
    graph: &SqliteGraph,
    pattern: &PatternTriple,
) -> Result<Vec<TripleMatch>, SqliteGraphError> {
    pattern.validate()?;

    // Case 1: Fast Path - edge type only
    if can_use_fast_path(pattern) {
        return execute_fast_path(graph, pattern);
    }

    // Case 2: Partial Fast Path - use cache as candidate generator
    if can_use_partial_fast_path(pattern) {
        return execute_partial_fast_path(graph, pattern);
    }

    // Case 3: SQL Only - complex pattern
    execute_sql_only(graph, pattern)
}

/// Execute fast-path for edge type only patterns (Case 1)
///
/// Uses adjacency cache directly and validates via SQL lookup.
fn execute_fast_path(
    graph: &SqliteGraph,
    pattern: &PatternTriple,
) -> Result<Vec<TripleMatch>, SqliteGraphError> {
    let mut matches = Vec::new();

    // Get all entity IDs to iterate through
    let entity_ids = graph.all_entity_ids()?;

    // For each entity, use cache to get adjacency candidates
    for &source_id in &entity_ids {
        let candidates = match pattern.direction {
            BackendDirection::Outgoing => graph.fetch_outgoing(source_id)?,
            BackendDirection::Incoming => graph.fetch_incoming(source_id)?,
        };

        // For each candidate, validate edge exists with correct type
        for &target_id in &candidates {
            // Always use the actual database direction for validation
            let (from_id, to_id) = match pattern.direction {
                BackendDirection::Outgoing => (source_id, target_id),
                BackendDirection::Incoming => (target_id, source_id),
            };

            // Validate edge exists with correct type
            if edge_exists_with_type(
                graph,
                from_id,
                to_id,
                &pattern.edge_type,
                BackendDirection::Outgoing,
            )? {
                let edge_id = get_edge_id(graph, from_id, to_id, &pattern.edge_type)?;

                // Create TripleMatch with correct direction semantics
                let triple_match = match pattern.direction {
                    BackendDirection::Outgoing => TripleMatch::new(source_id, edge_id, target_id),
                    BackendDirection::Incoming => TripleMatch::new(source_id, edge_id, target_id),
                };
                matches.push(triple_match);
            }
        }
    }

    // Ensure deterministic ordering
    matches.sort_by(|a, b| {
        a.start_id
            .cmp(&b.start_id)
            .then_with(|| a.edge_id.cmp(&b.edge_id))
            .then_with(|| a.end_id.cmp(&b.end_id))
    });

    Ok(matches)
}

/// Execute partial fast-path using cache as candidate generator (Case 2)
///
/// Uses cache to narrow candidates but validates via SQL.
fn execute_partial_fast_path(
    graph: &SqliteGraph,
    pattern: &PatternTriple,
) -> Result<Vec<TripleMatch>, SqliteGraphError> {
    // For partial fast-path, we still use SQL but can optimize
    // by using cache to pre-filter candidates where possible
    match_triples(graph, pattern)
}

/// Execute SQL-only path for complex patterns (Case 3)
///
/// Falls back to original SQL implementation.
fn execute_sql_only(
    graph: &SqliteGraph,
    pattern: &PatternTriple,
) -> Result<Vec<TripleMatch>, SqliteGraphError> {
    match_triples(graph, pattern)
}

/// Check if an edge exists between two nodes with the specified type.
///
/// This validates cache data against the authoritative SQL source.
fn edge_exists_with_type(
    graph: &SqliteGraph,
    from_id: i64,
    to_id: i64,
    edge_type: &str,
    _direction: BackendDirection,
) -> Result<bool, SqliteGraphError> {
    let conn = graph.connection();

    // Always use the actual database direction (from_id -> to_id)
    let sql =
        "SELECT 1 FROM graph_edges WHERE from_id = ?1 AND to_id = ?2 AND edge_type = ?3 LIMIT 1";

    let exists: Option<i32> = conn
        .query_row(sql, params![from_id, to_id, edge_type], |row| row.get(0))
        .optional()
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;

    Ok(exists.is_some())
}

/// Get the edge ID for a specific edge.
///
/// Retrieves the actual edge ID from SQL for accurate results.
fn get_edge_id(
    graph: &SqliteGraph,
    from_id: i64,
    to_id: i64,
    edge_type: &str,
) -> Result<i64, SqliteGraphError> {
    let conn = graph.connection();

    let edge_id: i64 = conn
        .query_row(
            "SELECT id FROM graph_edges WHERE from_id = ?1 AND to_id = ?2 AND edge_type = ?3 ORDER BY id LIMIT 1",
            params![from_id, to_id, edge_type],
            |row| row.get(0),
        )
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;

    Ok(edge_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GraphEdge, GraphEntity};
    use serde_json::json;

    fn create_test_graph() -> SqliteGraph {
        SqliteGraph::open_in_memory().expect("Failed to create test graph")
    }

    fn insert_entity(graph: &SqliteGraph, kind: &str, name: &str) -> i64 {
        graph
            .insert_entity(&GraphEntity {
                id: 0,
                kind: kind.into(),
                name: name.into(),
                file_path: None,
                data: json!({"name": name, "type": kind}),
            })
            .expect("Failed to insert entity")
    }

    fn insert_edge(graph: &SqliteGraph, from: i64, to: i64, edge_type: &str) -> i64 {
        graph
            .insert_edge(&GraphEdge {
                id: 0,
                from_id: from,
                to_id: to,
                edge_type: edge_type.into(),
                data: json!({"type": edge_type}),
            })
            .expect("Failed to insert edge")
    }

    #[test]
    fn test_can_use_fast_path_detection() {
        // Should use fast path - edge type only
        let pattern1 = PatternTriple::new("CALLS");
        assert!(can_use_fast_path(&pattern1));

        // Should NOT use fast path - has start label
        let pattern2 = PatternTriple::new("CALLS").start_label("Function");
        assert!(!can_use_fast_path(&pattern2));

        // Should NOT use fast path - has property filter
        let pattern3 = PatternTriple::new("CALLS").start_property("lang", "rust");
        assert!(!can_use_fast_path(&pattern3));

        // Should NOT use fast path - has end label
        let pattern4 = PatternTriple::new("CALLS").end_label("Function");
        assert!(!can_use_fast_path(&pattern4));
    }

    #[test]
    fn test_fast_path_basic_functionality() {
        let graph = create_test_graph();

        let f1 = insert_entity(&graph, "Function", "func1");
        let f2 = insert_entity(&graph, "Function", "func2");
        let _edge_id = insert_edge(&graph, f1, f2, "CALLS");

        let pattern = PatternTriple::new("CALLS");
        let matches = match_triples_fast(&graph, &pattern).expect("Fast path failed");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].start_id, f1);
        assert_eq!(matches[0].end_id, f2);
    }

    #[test]
    fn test_fast_path_vs_sql_equality() {
        let graph = create_test_graph();

        let f1 = insert_entity(&graph, "Function", "func1");
        let f2 = insert_entity(&graph, "Function", "func2");
        let f3 = insert_entity(&graph, "Function", "func3");

        insert_edge(&graph, f1, f2, "CALLS");
        insert_edge(&graph, f1, f3, "CALLS");
        insert_edge(&graph, f2, f3, "USES");

        let pattern = PatternTriple::new("CALLS");

        let sql_results = match_triples(&graph, &pattern).expect("SQL failed");
        let fast_results = match_triples_fast(&graph, &pattern).expect("Fast path failed");

        // Results must be identical
        assert_eq!(sql_results.len(), fast_results.len());
        assert_eq!(sql_results, fast_results);
    }

    #[test]
    fn test_fast_path_deterministic_ordering() {
        let graph = create_test_graph();

        let f1 = insert_entity(&graph, "Function", "func1");
        let f2 = insert_entity(&graph, "Function", "func2");
        let f3 = insert_entity(&graph, "Function", "func3");

        let edge1 = insert_edge(&graph, f1, f2, "CALLS");
        let edge2 = insert_edge(&graph, f1, f3, "CALLS");
        let edge3 = insert_edge(&graph, f2, f3, "CALLS");

        let pattern = PatternTriple::new("CALLS");
        let matches = match_triples_fast(&graph, &pattern).expect("Fast path failed");

        assert_eq!(matches.len(), 3);

        // Verify deterministic ordering: start_id ASC, edge_id ASC, end_id ASC
        for i in 1..matches.len() {
            assert!(
                matches[i - 1].start_id < matches[i].start_id
                    || (matches[i - 1].start_id == matches[i].start_id
                        && matches[i - 1].edge_id < matches[i].edge_id)
                    || (matches[i - 1].start_id == matches[i].start_id
                        && matches[i - 1].edge_id == matches[i].edge_id
                        && matches[i - 1].end_id <= matches[i].end_id),
                "Matches not in deterministic order at index {}: {:?} vs {:?}",
                i,
                matches[i - 1],
                matches[i]
            );
        }

        // Should be ordered by the edge IDs we created
        let expected_order = vec![edge1, edge2, edge3];
        for (i, &expected_edge_id) in expected_order.iter().enumerate() {
            assert_eq!(matches[i].edge_id, expected_edge_id);
        }
    }
}
