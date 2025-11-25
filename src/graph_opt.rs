use ahash::AHashSet;
use serde_json::Value;

use crate::{
    SqliteGraphError,
    cache::CacheStats,
    graph::{InstrumentedConnection, SqliteGraph},
};

#[derive(Clone, Debug)]
pub struct GraphEntityCreate {
    pub kind: String,
    pub name: String,
    pub file_path: Option<String>,
    pub data: Value,
}

#[derive(Clone, Debug)]
pub struct GraphEdgeCreate {
    pub from_id: i64,
    pub to_id: i64,
    pub edge_type: String,
    pub data: Value,
}

pub fn bulk_insert_entities(
    graph: &SqliteGraph,
    entries: &[GraphEntityCreate],
) -> Result<Vec<i64>, SqliteGraphError> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    let conn = graph.connection();
    conn.execute("BEGIN IMMEDIATE", [])
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let result = (|| {
        let mut stmt = conn
            .prepare_cached(
                "INSERT INTO graph_entities(kind,name,file_path,data) VALUES(?1,?2,?3,?4)",
            )
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        let mut ids = Vec::new();
        for entry in entries {
            validate_entity_create(entry)?;
            let payload = serde_json::to_string(&entry.data)
                .map_err(|e| SqliteGraphError::invalid_input(e.to_string()))?;
            stmt.execute(rusqlite::params![
                entry.kind,
                entry.name,
                entry.file_path,
                payload
            ])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
            ids.push(conn.last_insert_rowid());
        }
        Ok(ids)
    })();
    finalize_transaction(graph, conn, result)
}

pub fn bulk_insert_edges(
    graph: &SqliteGraph,
    entries: &[GraphEdgeCreate],
) -> Result<Vec<i64>, SqliteGraphError> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    let conn = graph.connection();
    conn.execute("BEGIN IMMEDIATE", [])
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let result = (|| {
        let mut stmt = conn
            .prepare_cached(
                "INSERT INTO graph_edges(from_id,to_id,edge_type,data) VALUES(?1,?2,?3,?4)",
            )
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
        let mut ids = Vec::new();
        let mut seen = AHashSet::new();
        for entry in entries {
            validate_edge_create(entry)?;
            if !seen.insert((entry.from_id, entry.to_id, entry.edge_type.clone())) {
                continue;
            }
            validate_endpoints_exist(&conn, entry.from_id, entry.to_id)?;
            let payload = serde_json::to_string(&entry.data)
                .map_err(|e| SqliteGraphError::invalid_input(e.to_string()))?;
            stmt.execute(rusqlite::params![
                entry.from_id,
                entry.to_id,
                entry.edge_type,
                payload
            ])
            .map_err(|e| SqliteGraphError::query(e.to_string()))?;
            ids.push(conn.last_insert_rowid());
        }
        Ok(ids)
    })();
    finalize_transaction(graph, conn, result)
}

pub fn adjacency_fetch_outgoing_batch(
    graph: &SqliteGraph,
    ids: &[i64],
) -> Result<Vec<(i64, Vec<i64>)>, SqliteGraphError> {
    let mut results = Vec::new();
    for &id in ids {
        results.push((id, graph.fetch_outgoing(id)?));
    }
    results.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(results)
}

pub fn adjacency_fetch_incoming_batch(
    graph: &SqliteGraph,
    ids: &[i64],
) -> Result<Vec<(i64, Vec<i64>)>, SqliteGraphError> {
    let mut results = Vec::new();
    for &id in ids {
        results.push((id, graph.fetch_incoming(id)?));
    }
    results.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(results)
}

pub fn cache_clear_ranges(graph: &SqliteGraph, ids: &[i64]) {
    for &id in ids {
        graph.outgoing_cache_ref().remove(id);
        graph.incoming_cache_ref().remove(id);
    }
}

pub fn cache_stats(graph: &SqliteGraph) -> CacheStats {
    let outgoing = graph.outgoing_cache_ref().stats();
    let incoming = graph.incoming_cache_ref().stats();
    CacheStats {
        hits: outgoing.hits + incoming.hits,
        misses: outgoing.misses + incoming.misses,
        entries: outgoing.entries + incoming.entries,
    }
}

fn finalize_transaction(
    graph: &SqliteGraph,
    conn: InstrumentedConnection<'_>,
    result: Result<Vec<i64>, SqliteGraphError>,
) -> Result<Vec<i64>, SqliteGraphError> {
    match result {
        Ok(ids) => {
            conn.execute("COMMIT", [])
                .map_err(|e| SqliteGraphError::query(e.to_string()))?;
            graph.invalidate_caches();
            Ok(ids)
        }
        Err(err) => {
            let _ = conn.execute("ROLLBACK", []);
            Err(err)
        }
    }
}

fn validate_entity_create(entry: &GraphEntityCreate) -> Result<(), SqliteGraphError> {
    if entry.kind.trim().is_empty() {
        return Err(SqliteGraphError::invalid_input("entity kind must be set"));
    }
    if entry.name.trim().is_empty() {
        return Err(SqliteGraphError::invalid_input("entity name must be set"));
    }
    Ok(())
}

fn validate_edge_create(entry: &GraphEdgeCreate) -> Result<(), SqliteGraphError> {
    if entry.edge_type.trim().is_empty() {
        return Err(SqliteGraphError::invalid_input("edge type must be set"));
    }
    if entry.from_id <= 0 || entry.to_id <= 0 {
        return Err(SqliteGraphError::invalid_input(
            "edge endpoints must be positive ids",
        ));
    }
    Ok(())
}

fn validate_endpoints_exist(
    conn: &InstrumentedConnection<'_>,
    from: i64,
    to: i64,
) -> Result<(), SqliteGraphError> {
    let mut stmt = conn
        .prepare_cached("SELECT COUNT(1) FROM graph_entities WHERE id IN (?1, ?2)")
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    let count: i64 = stmt
        .query_row(rusqlite::params![from, to], |row| row.get(0))
        .map_err(|e| SqliteGraphError::query(e.to_string()))?;
    if count < 2 {
        return Err(SqliteGraphError::invalid_input("edge endpoints must exist"));
    }
    Ok(())
}
