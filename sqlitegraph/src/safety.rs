use std::{fmt, result};

use rusqlite::OptionalExtension;
use serde::Serialize;
use serde_json::Value;

use crate::{SqliteGraphError, graph::SqliteGraph};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SafetyReport {
    pub total_nodes: i64,
    pub total_edges: i64,
    pub orphan_edges: i64,
    pub duplicate_edges: i64,
    pub invalid_labels: i64,
    pub invalid_properties: i64,
    pub integrity_errors: i64,
    pub integrity_messages: Vec<String>,
}

impl SafetyReport {
    pub fn merge(&mut self, other: &SafetyReport) {
        self.total_nodes = self.total_nodes.max(other.total_nodes);
        self.total_edges = self.total_edges.max(other.total_edges);
        self.orphan_edges += other.orphan_edges;
        self.duplicate_edges += other.duplicate_edges;
        self.invalid_labels += other.invalid_labels;
        self.invalid_properties += other.invalid_properties;
        self.integrity_errors += other.integrity_errors;
        if !other.integrity_messages.is_empty() {
            self.integrity_messages
                .extend_from_slice(&other.integrity_messages);
        }
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

pub fn run_deep_safety_checks(graph: &SqliteGraph) -> Result<SafetyReport, SqliteGraphError> {
    let mut report = run_safety_checks(graph)?;
    let integrity = integrity_check(graph)?;
    report.integrity_errors = integrity.len() as i64;
    report.integrity_messages = integrity;
    Ok(report)
}

pub fn run_integrity_sweep(graph: &SqliteGraph) -> Result<Vec<String>, SqliteGraphError> {
    let conn = graph.connection();
    let mut issues = Vec::new();
    sweep_entities(&conn, &mut issues)?;
    sweep_edges(&conn, &mut issues)?;
    sweep_labels(&conn, &mut issues)?;
    sweep_properties(&conn, &mut issues)?;
    Ok(issues)
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

fn integrity_check(graph: &SqliteGraph) -> Result<Vec<String>, SqliteGraphError> {
    let conn = graph.connection();
    let mut stmt = conn
        .prepare_cached("PRAGMA integrity_check")
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let mut errors = Vec::new();
    for row in rows {
        let message = row.map_err(|e| SqliteGraphError::query(e.to_string()))?;
        if message != "ok" {
            errors.push(message);
        }
    }
    Ok(errors)
}

fn sweep_entities(
    conn: &crate::graph::InstrumentedConnection<'_>,
    issues: &mut Vec<String>,
) -> Result<(), SqliteGraphError> {
    let mut stmt = conn
        .prepare_cached("SELECT id, data FROM graph_entities ORDER BY id")
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let mut prev_id: Option<i64> = None;
    for row in rows {
        let (id, data) = row.map_err(|e| SqliteGraphError::query(e.to_string()))?;
        if let Some(prev) = prev_id
            && id <= prev
        {
            issues.push(format!("entity ids out of order: {} <= {}", id, prev));
        }
        prev_id = Some(id);
        if serde_json::from_str::<Value>(&data).is_err() {
            issues.push(format!("entity {id} has invalid JSON payload"));
        }
    }
    Ok(())
}

fn sweep_edges(
    conn: &crate::graph::InstrumentedConnection<'_>,
    issues: &mut Vec<String>,
) -> Result<(), SqliteGraphError> {
    let mut stmt = conn
        .prepare_cached("SELECT id, from_id, to_id FROM graph_edges ORDER BY id")
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let mut prev_id: Option<i64> = None;
    let mut verify = conn
        .prepare_cached("SELECT COUNT(*) FROM graph_entities WHERE id IN (?1, ?2)")
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    for row in rows {
        let (id, from_id, to_id) = row.map_err(|e| SqliteGraphError::query(e.to_string()))?;
        if let Some(prev) = prev_id
            && id <= prev
        {
            issues.push(format!("edge ids out of order: {} <= {}", id, prev));
        }
        prev_id = Some(id);
        let count: i64 = verify
            .query_row(rusqlite::params![from_id, to_id], |r| r.get(0))
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        if count < 2 {
            issues.push(format!(
                "edge {id} references missing entities ({from_id}->{to_id})"
            ));
        }
    }
    Ok(())
}

fn sweep_labels(
    conn: &crate::graph::InstrumentedConnection<'_>,
    issues: &mut Vec<String>,
) -> Result<(), SqliteGraphError> {
    let mut stmt = conn
        .prepare_cached("SELECT entity_id, label FROM graph_labels ORDER BY entity_id, label")
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let mut verify = conn
        .prepare_cached("SELECT 1 FROM graph_entities WHERE id=?1")
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    for row in rows {
        let (entity_id, label) = row.map_err(|e| SqliteGraphError::query(e.to_string()))?;
        let exists: Option<i64> = verify
            .query_row(rusqlite::params![entity_id], |r| r.get(0))
            .optional()
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        if exists.is_none() {
            issues.push(format!(
                "label '{}' references missing entity {}",
                label, entity_id
            ));
        }
    }
    Ok(())
}

fn sweep_properties(
    conn: &crate::graph::InstrumentedConnection<'_>,
    issues: &mut Vec<String>,
) -> Result<(), SqliteGraphError> {
    let mut stmt = conn
        .prepare_cached("SELECT entity_id, key FROM graph_properties ORDER BY entity_id, key")
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let mut verify = conn
        .prepare_cached("SELECT 1 FROM graph_entities WHERE id=?1")
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    for row in rows {
        let (entity_id, key) = row.map_err(|e| SqliteGraphError::query(e.to_string()))?;
        let exists: Option<i64> = verify
            .query_row(rusqlite::params![entity_id], |r| r.get(0))
            .optional()
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        if exists.is_none() {
            issues.push(format!(
                "property '{}' references missing entity {}",
                key, entity_id
            ));
        }
    }
    Ok(())
}
