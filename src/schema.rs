use rusqlite::Connection;

use crate::errors::SqliteGraphError;

pub fn ensure_schema(conn: &Connection) -> Result<(), SqliteGraphError> {
    conn.execute_batch(
        r#"
        PRAGMA foreign_keys = ON;
        CREATE TABLE IF NOT EXISTS graph_entities (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            kind      TEXT NOT NULL,
            name      TEXT NOT NULL,
            file_path TEXT,
            data      TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS graph_edges (
            id        INTEGER PRIMARY KEY AUTOINCREMENT,
            from_id   INTEGER NOT NULL,
            to_id     INTEGER NOT NULL,
            edge_type TEXT NOT NULL,
            data      TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS graph_labels (
            entity_id INTEGER NOT NULL,
            label     TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS graph_properties (
            entity_id INTEGER NOT NULL,
            key       TEXT NOT NULL,
            value     TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_edges_from ON graph_edges(from_id);
        CREATE INDEX IF NOT EXISTS idx_edges_to ON graph_edges(to_id);
        CREATE INDEX IF NOT EXISTS idx_edges_type ON graph_edges(edge_type);
        CREATE INDEX IF NOT EXISTS idx_labels_label ON graph_labels(label);
        CREATE INDEX IF NOT EXISTS idx_props_key_value ON graph_properties(key, value);
        "#,
    )
    .map_err(|e| SqliteGraphError::schema(e.to_string()))?;
    Ok(())
}
