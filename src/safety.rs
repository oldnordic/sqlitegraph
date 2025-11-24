use std::{fmt, result};

use rusqlite::OptionalExtension;
use serde::Serialize;

use crate::{SqliteGraphError, graph::SqliteGraph};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SafetyReport {
    pub total_nodes: i64,
    pub total_edges: i64,
    pub orphan_edges: i64,
    pub duplicate_edges: i64,
    pub invalid_labels: i64,
    pub invalid_properties: i64,
}

impl SafetyReport {
    pub fn merge(&mut self, other: &SafetyReport) {
        self.total_nodes = self.total_nodes.max(other.total_nodes);
        self.total_edges = self.total_edges.max(other.total_edges);
        self.orphan_edges += other.orphan_edges;
        self.duplicate_edges += other.duplicate_edges;
        self.invalid_labels += other.invalid_labels;
        self.invalid_properties += other.invalid_properties;
    }

    pub fn has_issues(&self) -> bool {
        self.orphan_edges > 0
            || self.duplicate_edges > 0
            || self.invalid_labels > 0
            || self.invalid_properties > 0
    }
}

#[derive(Debug)]
pub struct SafetyError {
    pub report: SafetyReport,
    pub source: Option<SqliteGraphError>,
}

impl fmt::Display for SafetyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "safety violations detected")
    }
}

impl std::error::Error for SafetyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_ref()
            .map(|err| err as &dyn std::error::Error)
    }
}

pub fn validate_referential_integrity(
    graph: &SqliteGraph,
) -> Result<SafetyReport, SqliteGraphError> {
    let mut report = base_report(graph)?;
    report.orphan_edges = query_single(
        graph,
        "SELECT COUNT(*) FROM graph_edges e \n         LEFT JOIN graph_entities src ON src.id = e.from_id \n         LEFT JOIN graph_entities dst ON dst.id = e.to_id \n         WHERE src.id IS NULL OR dst.id IS NULL",
    )?;
    Ok(report)
}

pub fn validate_no_duplicate_edges(graph: &SqliteGraph) -> Result<SafetyReport, SqliteGraphError> {
    let mut report = base_report(graph)?;
    report.duplicate_edges = query_single(
        graph,
        "SELECT COALESCE(SUM(cnt - 1), 0) FROM (\n             SELECT COUNT(*) AS cnt FROM graph_edges\n             GROUP BY from_id, to_id, edge_type\n             HAVING cnt > 1\n         )",
    )?;
    Ok(report)
}

pub fn validate_labels_properties(graph: &SqliteGraph) -> Result<SafetyReport, SqliteGraphError> {
    let mut report = base_report(graph)?;
    report.invalid_labels = query_single(
        graph,
        "SELECT COUNT(*) FROM graph_labels l\n         LEFT JOIN graph_entities e ON e.id = l.entity_id\n         WHERE e.id IS NULL",
    )?;
    report.invalid_properties = query_single(
        graph,
        "SELECT COUNT(*) FROM graph_properties p\n         LEFT JOIN graph_entities e ON e.id = p.entity_id\n         WHERE e.id IS NULL",
    )?;
    Ok(report)
}

pub fn run_safety_checks(graph: &SqliteGraph) -> Result<SafetyReport, SqliteGraphError> {
    let mut report = SafetyReport::default();
    report.merge(&validate_referential_integrity(graph)?);
    report.merge(&validate_no_duplicate_edges(graph)?);
    report.merge(&validate_labels_properties(graph)?);
    Ok(report)
}

pub fn run_strict_safety_checks(graph: &SqliteGraph) -> result::Result<(), SafetyError> {
    let report = run_safety_checks(graph).map_err(|err| SafetyError {
        report: SafetyReport::default(),
        source: Some(err),
    })?;
    if report.has_issues() {
        Err(SafetyError {
            report,
            source: None,
        })
    } else {
        Ok(())
    }
}

fn base_report(graph: &SqliteGraph) -> Result<SafetyReport, SqliteGraphError> {
    let total_nodes = query_single(graph, "SELECT COUNT(*) FROM graph_entities")?;
    let total_edges = query_single(graph, "SELECT COUNT(*) FROM graph_edges")?;
    Ok(SafetyReport {
        total_nodes,
        total_edges,
        ..SafetyReport::default()
    })
}

fn query_single(graph: &SqliteGraph, sql: &str) -> Result<i64, SqliteGraphError> {
    graph
        .connection()
        .query_row(sql, [], |row| row.get(0))
        .optional()
        .map(|opt| opt.unwrap_or(0))
        .map_err(|e| SqliteGraphError::query(e.to_string()))
}
