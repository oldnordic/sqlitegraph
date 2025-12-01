//! Lightweight triple pattern matcher for SQLiteGraph.
//!
//! Provides a minimal, deterministic pattern matcher for basic Neo4j-style patterns:
//! (start_label)-\[edge_type\]->(end_label) with optional property filters.
//!
//! This is designed to be a simpler alternative to the full pattern system,
//! focusing on single-hop patterns with equality-based property filtering.

use rusqlite::{OptionalExtension, params};
use std::collections::HashMap;

use crate::{backend::BackendDirection, errors::SqliteGraphError, graph::SqliteGraph};

/// A lightweight triple pattern for basic graph pattern matching.
///
/// Represents a single-hop pattern: (start_label)-\[edge_type\]->(end_label)
/// with optional property filters on start and end nodes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternTriple {
    /// Optional label filter for the start node
    pub start_label: Option<String>,
    /// Edge type to match (required)
    pub edge_type: String,
    /// Optional label filter for the end node  
    pub end_label: Option<String>,
    /// Optional property filters for the start node (key -> value)
    pub start_props: HashMap<String, String>,
    /// Optional property filters for the end node (key -> value)
    pub end_props: HashMap<String, String>,
    /// Direction of the pattern (default: Outgoing)
    pub direction: BackendDirection,
}

impl Default for PatternTriple {
    fn default() -> Self {
        Self {
            start_label: None,
            edge_type: String::new(),
            end_label: None,
            start_props: HashMap::new(),
            end_props: HashMap::new(),
            direction: BackendDirection::Outgoing,
        }
    }
}

impl PatternTriple {
    /// Create a new pattern triple with the given edge type.
    pub fn new(edge_type: impl Into<String>) -> Self {
        Self {
            edge_type: edge_type.into(),
            ..Self::default()
        }
    }

    /// Set the start node label filter.
    pub fn start_label(mut self, label: impl Into<String>) -> Self {
        self.start_label = Some(label.into());
        self
    }

    /// Set the end node label filter.
    pub fn end_label(mut self, label: impl Into<String>) -> Self {
        self.end_label = Some(label.into());
        self
    }

    /// Add a property filter for the start node.
    pub fn start_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.start_props.insert(key.into(), value.into());
        self
    }

    /// Add a property filter for the end node.
    pub fn end_property(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.end_props.insert(key.into(), value.into());
        self
    }

    /// Set the direction of the pattern.
    pub fn direction(mut self, direction: BackendDirection) -> Self {
        self.direction = direction;
        self
    }

    /// Validate that the pattern is well-formed.
    pub fn validate(&self) -> Result<(), SqliteGraphError> {
        if self.edge_type.trim().is_empty() {
            return Err(SqliteGraphError::invalid_input("edge_type is required"));
        }
        Ok(())
    }
}

/// Result of a triple pattern match.
///
/// Represents a matched triple containing the start node, edge, and end node IDs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TripleMatch {
    /// ID of the start node
    pub start_id: i64,
    /// ID of the end node
    pub end_id: i64,
    /// ID of the matching edge
    pub edge_id: i64,
}

impl TripleMatch {
    /// Create a new triple match.
    pub fn new(start_id: i64, edge_id: i64, end_id: i64) -> Self {
        Self {
            start_id,
            end_id,
            edge_id,
        }
    }
}

/// Execute a lightweight triple pattern match.
///
/// This function provides a simple, deterministic way to match single-hop patterns
/// using the existing SQLite indexes for optimal performance.
///
/// # Arguments
/// * `graph` - The SQLiteGraph instance
/// * `pattern` - The pattern triple to match
///
/// # Returns
/// A vector of triple matches in deterministic order
pub fn match_triples(
    graph: &SqliteGraph,
    pattern: &PatternTriple,
) -> Result<Vec<TripleMatch>, SqliteGraphError> {
    pattern.validate()?;

    let _conn = graph.connection();

    // Build and execute the query based on pattern complexity
    let matches = if pattern.start_label.is_none() && pattern.end_label.is_none() {
        // Simple case: no label filters
        execute_simple_edge_query(graph, pattern)?
    } else {
        // Complex case: with label filters
        execute_complex_edge_query(graph, pattern)?
    };

    // Apply property filters if specified
    let mut filtered_matches = Vec::new();
    for triple_match in matches {
        if matches_property_filters(graph, &triple_match, pattern)? {
            filtered_matches.push(triple_match);
        }
    }

    // Ensure deterministic ordering
    filtered_matches.sort_by(|a, b| {
        a.start_id
            .cmp(&b.start_id)
            .then_with(|| a.edge_id.cmp(&b.edge_id))
            .then_with(|| a.end_id.cmp(&b.end_id))
    });

    Ok(filtered_matches)
}

/// Execute simple edge query without label filters.
fn execute_simple_edge_query(
    graph: &SqliteGraph,
    pattern: &PatternTriple,
) -> Result<Vec<TripleMatch>, SqliteGraphError> {
    let conn = graph.connection();

    let sql = match pattern.direction {
        BackendDirection::Outgoing => {
            "SELECT ge.from_id, ge.id, ge.to_id FROM graph_edges ge WHERE ge.edge_type = ?1 ORDER BY ge.from_id, ge.id, ge.to_id"
        }
        BackendDirection::Incoming => {
            "SELECT ge.to_id, ge.id, ge.from_id FROM graph_edges ge WHERE ge.edge_type = ?1 ORDER BY ge.to_id, ge.id, ge.from_id"
        }
    };

    let mut stmt = conn
        .prepare_cached(sql)
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;

    let rows = stmt
        .query_map(params![&pattern.edge_type], |row| {
            Ok(TripleMatch::new(
                row.get(0)?, // start_id
                row.get(1)?, // edge_id
                row.get(2)?, // end_id
            ))
        })
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;

    let mut matches = Vec::new();
    for row in rows {
        matches.push(row.map_err(|e| SqliteGraphError::query(e.to_string()))?);
    }

    Ok(matches)
}

/// Execute complex edge query with label filters.
fn execute_complex_edge_query(
    graph: &SqliteGraph,
    pattern: &PatternTriple,
) -> Result<Vec<TripleMatch>, SqliteGraphError> {
    let conn = graph.connection();

    // Build SQL dynamically based on which labels are present
    let mut sql = match pattern.direction {
        BackendDirection::Outgoing => {
            "SELECT ge.from_id, ge.id, ge.to_id FROM graph_edges ge".to_string()
        }
        BackendDirection::Incoming => {
            "SELECT ge.to_id, ge.id, ge.from_id FROM graph_edges ge".to_string()
        }
    };

    sql.push_str(" WHERE ge.edge_type = ?1");

    let mut param_count = 1;

    // Add start label filter
    if let Some(_start_label) = &pattern.start_label {
        param_count += 1;
        sql.push_str(" AND EXISTS (");
        sql.push_str("  SELECT 1 FROM graph_labels gl");
        sql.push_str("  WHERE gl.entity_id = ");
        sql.push_str(if pattern.direction == BackendDirection::Outgoing {
            "ge.from_id"
        } else {
            "ge.to_id"
        });
        sql.push_str(&format!("  AND gl.label = ?{}", param_count));
        sql.push_str(" )");
    }

    // Add end label filter
    if let Some(_end_label) = &pattern.end_label {
        param_count += 1;
        sql.push_str(" AND EXISTS (");
        sql.push_str("  SELECT 1 FROM graph_labels gl");
        sql.push_str("  WHERE gl.entity_id = ");
        sql.push_str(if pattern.direction == BackendDirection::Outgoing {
            "ge.to_id"
        } else {
            "ge.from_id"
        });
        sql.push_str(&format!("  AND gl.label = ?{}", param_count));
        sql.push_str(" )");
    }

    // Add deterministic ordering
    sql.push_str(" ORDER BY ");
    if pattern.direction == BackendDirection::Outgoing {
        sql.push_str("ge.from_id, ge.id, ge.to_id");
    } else {
        sql.push_str("ge.to_id, ge.id, ge.from_id");
    }

    // Execute query with appropriate parameters
    let matches =
        if let (Some(start_label), Some(end_label)) = (&pattern.start_label, &pattern.end_label) {
            let mut stmt = conn
                .prepare_cached(&sql)
                .map_err(|e| SqliteGraphError::query(e.to_string()))?;

            let rows = stmt
                .query_map(params![&pattern.edge_type, start_label, end_label], |row| {
                    Ok(TripleMatch::new(row.get(0)?, row.get(1)?, row.get(2)?))
                })
                .map_err(|e| SqliteGraphError::query(e.to_string()))?;

            collect_triple_matches(rows)?
        } else if let Some(start_label) = &pattern.start_label {
            let mut stmt = conn
                .prepare_cached(&sql)
                .map_err(|e| SqliteGraphError::query(e.to_string()))?;

            let rows = stmt
                .query_map(params![&pattern.edge_type, start_label], |row| {
                    Ok(TripleMatch::new(row.get(0)?, row.get(1)?, row.get(2)?))
                })
                .map_err(|e| SqliteGraphError::query(e.to_string()))?;

            collect_triple_matches(rows)?
        } else if let Some(end_label) = &pattern.end_label {
            let mut stmt = conn
                .prepare_cached(&sql)
                .map_err(|e| SqliteGraphError::query(e.to_string()))?;

            let rows = stmt
                .query_map(params![&pattern.edge_type, end_label], |row| {
                    Ok(TripleMatch::new(row.get(0)?, row.get(1)?, row.get(2)?))
                })
                .map_err(|e| SqliteGraphError::query(e.to_string()))?;

            collect_triple_matches(rows)?
        } else {
            unreachable!("This case should be handled by execute_simple_edge_query")
        };

    Ok(matches)
}

/// Collect triple matches from query rows.
fn collect_triple_matches(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<TripleMatch>>,
) -> Result<Vec<TripleMatch>, SqliteGraphError> {
    let mut matches = Vec::new();
    for row in rows {
        matches.push(row.map_err(|e| SqliteGraphError::query(e.to_string()))?);
    }
    Ok(matches)
}

/// Check if a triple match satisfies the property filters.
fn matches_property_filters(
    graph: &SqliteGraph,
    triple_match: &TripleMatch,
    pattern: &PatternTriple,
) -> Result<bool, SqliteGraphError> {
    // Check start node properties
    if !pattern.start_props.is_empty() {
        if !entity_has_properties(graph, triple_match.start_id, &pattern.start_props)? {
            return Ok(false);
        }
    }

    // Check end node properties
    if !pattern.end_props.is_empty() {
        if !entity_has_properties(graph, triple_match.end_id, &pattern.end_props)? {
            return Ok(false);
        }
    }

    Ok(true)
}

/// Check if an entity has all the specified properties with matching values.
fn entity_has_properties(
    graph: &SqliteGraph,
    entity_id: i64,
    required_props: &HashMap<String, String>,
) -> Result<bool, SqliteGraphError> {
    let conn = graph.connection();

    for (key, expected_value) in required_props {
        let mut stmt = conn
            .prepare_cached(
                "SELECT 1 FROM graph_properties WHERE entity_id = ?1 AND key = ?2 AND value = ?3 LIMIT 1"
            )
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;

        let exists: Option<i32> = stmt
            .query_row(params![entity_id, key, expected_value], |row| row.get(0))
            .optional()
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;

        if exists.is_none() {
            return Ok(false);
        }
    }

    Ok(true)
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
                data: json!({"name": name}),
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
                data: json!({}),
            })
            .expect("Failed to insert edge")
    }

    fn add_label_to_entity(graph: &SqliteGraph, entity_id: i64, label: &str) {
        crate::index::add_label(graph, entity_id, label).expect("Failed to add label");
    }

    fn add_property_to_entity(graph: &SqliteGraph, entity_id: i64, key: &str, value: &str) {
        crate::index::add_property(graph, entity_id, key, value).expect("Failed to add property");
    }

    #[test]
    fn test_pattern_triple_builder() {
        let pattern = PatternTriple::new("CALLS")
            .start_label("Function")
            .end_label("Function")
            .start_property("language", "rust")
            .end_property("language", "rust")
            .direction(BackendDirection::Outgoing);

        assert_eq!(pattern.edge_type, "CALLS");
        assert_eq!(pattern.start_label, Some("Function".to_string()));
        assert_eq!(pattern.end_label, Some("Function".to_string()));
        assert_eq!(
            pattern.start_props.get("language"),
            Some(&"rust".to_string())
        );
        assert_eq!(pattern.end_props.get("language"), Some(&"rust".to_string()));
        assert_eq!(pattern.direction, BackendDirection::Outgoing);
    }

    #[test]
    fn test_pattern_triple_validation() {
        let valid_pattern = PatternTriple::new("CALLS");
        assert!(valid_pattern.validate().is_ok());

        let invalid_pattern = PatternTriple::new("");
        assert!(invalid_pattern.validate().is_err());
    }

    #[test]
    fn test_match_triples_basic() {
        let graph = create_test_graph();

        let f1 = insert_entity(&graph, "Function", "func1");
        let f2 = insert_entity(&graph, "Function", "func2");
        let _edge_id = insert_edge(&graph, f1, f2, "CALLS");

        let pattern = PatternTriple::new("CALLS");
        let matches = match_triples(&graph, &pattern).expect("Failed to match triples");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].start_id, f1);
        assert_eq!(matches[0].end_id, f2);
    }

    #[test]
    fn test_match_triples_with_labels() {
        let graph = create_test_graph();

        let f1 = insert_entity(&graph, "Function", "func1");
        let f2 = insert_entity(&graph, "Function", "func2");
        let s1 = insert_entity(&graph, "Struct", "struct1");

        add_label_to_entity(&graph, f1, "Function");
        add_label_to_entity(&graph, f2, "Function");
        add_label_to_entity(&graph, s1, "Struct");

        let _edge1 = insert_edge(&graph, f1, f2, "CALLS");
        let _edge2 = insert_edge(&graph, f2, s1, "USES");

        let pattern = PatternTriple::new("CALLS")
            .start_label("Function")
            .end_label("Function");

        let matches = match_triples(&graph, &pattern).expect("Failed to match triples");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].start_id, f1);
        assert_eq!(matches[0].end_id, f2);
    }

    #[test]
    fn test_match_triples_with_properties() {
        let graph = create_test_graph();

        let f1 = insert_entity(&graph, "Function", "func1");
        let f2 = insert_entity(&graph, "Function", "func2");
        let f3 = insert_entity(&graph, "Function", "func3");

        add_property_to_entity(&graph, f1, "language", "rust");
        add_property_to_entity(&graph, f2, "language", "rust");
        add_property_to_entity(&graph, f3, "language", "python");

        let _edge1 = insert_edge(&graph, f1, f2, "CALLS");
        let _edge2 = insert_edge(&graph, f1, f3, "CALLS");

        let pattern = PatternTriple::new("CALLS")
            .start_property("language", "rust")
            .end_property("language", "rust");

        let matches = match_triples(&graph, &pattern).expect("Failed to match triples");

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].start_id, f1);
        assert_eq!(matches[0].end_id, f2);
    }

    #[test]
    fn test_match_triples_direction() {
        let graph = create_test_graph();

        let f1 = insert_entity(&graph, "Function", "func1");
        let f2 = insert_entity(&graph, "Function", "func2");
        let edge_id = insert_edge(&graph, f1, f2, "CALLS");

        // Test outgoing direction
        let pattern_outgoing = PatternTriple::new("CALLS").direction(BackendDirection::Outgoing);

        let matches_outgoing =
            match_triples(&graph, &pattern_outgoing).expect("Failed to match triples");
        assert_eq!(matches_outgoing.len(), 1);
        assert_eq!(matches_outgoing[0].start_id, f1);
        assert_eq!(matches_outgoing[0].end_id, f2);

        // Test incoming direction
        let pattern_incoming = PatternTriple::new("CALLS").direction(BackendDirection::Incoming);

        let matches_incoming =
            match_triples(&graph, &pattern_incoming).expect("Failed to match triples");
        assert_eq!(matches_incoming.len(), 1);
        assert_eq!(matches_incoming[0].start_id, f2); // Start is now the original target
        assert_eq!(matches_incoming[0].end_id, f1); // End is now the original source
        assert_eq!(matches_incoming[0].edge_id, edge_id);
    }
}
