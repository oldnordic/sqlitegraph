use rusqlite::{Connection, OptionalExtension};

use crate::errors::SqliteGraphError;

pub const BASE_SCHEMA_VERSION: i64 = 1;

struct MigrationStep {
    target_version: i64,
    statements: &'static [&'static str],
}

const MIGRATION_STEPS: &[MigrationStep] = &[MigrationStep {
    target_version: 2,
    statements: &[
        "CREATE TABLE IF NOT EXISTS graph_meta_history(version INTEGER NOT NULL, applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP)",
        "INSERT INTO graph_meta_history(version) VALUES(2)",
    ],
}];

pub const SCHEMA_VERSION: i64 = BASE_SCHEMA_VERSION + MIGRATION_STEPS.len() as i64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    pub from_version: i64,
    pub to_version: i64,
    pub statements: Vec<&'static str>,
    pub dry_run: bool,
}

pub fn ensure_schema(conn: &Connection) -> Result<(), SqliteGraphError> {
    ensure_base_schema(conn)?;
    ensure_meta(conn)?;
    run_pending_migrations(conn, false)?;
    Ok(())
}

pub fn ensure_schema_without_migrations(conn: &Connection) -> Result<(), SqliteGraphError> {
    ensure_base_schema(conn)?;
    ensure_meta(conn)?;
    Ok(())
}

fn ensure_base_schema(conn: &Connection) -> Result<(), SqliteGraphError> {
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
        CREATE INDEX IF NOT EXISTS idx_labels_label_entity_id ON graph_labels(label, entity_id);
        CREATE INDEX IF NOT EXISTS idx_props_key_value ON graph_properties(key, value);
        CREATE INDEX IF NOT EXISTS idx_props_key_value_entity_id ON graph_properties(key, value, entity_id);
        CREATE INDEX IF NOT EXISTS idx_entities_kind_id ON graph_entities(kind, id);
        CREATE TABLE IF NOT EXISTS graph_meta (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            schema_version INTEGER NOT NULL
        );
        "#,
    )
    .map_err(|e| SqliteGraphError::schema(e.to_string()))
}

pub fn read_schema_version(conn: &Connection) -> Result<i64, SqliteGraphError> {
    conn.query_row(
        "SELECT schema_version FROM graph_meta WHERE id=1",
        [],
        |row| row.get(0),
    )
    .map_err(|e| SqliteGraphError::schema(e.to_string()))
}

pub fn run_pending_migrations(
    conn: &Connection,
    dry_run: bool,
) -> Result<MigrationReport, SqliteGraphError> {
    let current = read_schema_version(conn)?;
    let mut statements: Vec<&'static str> = Vec::new();
    let mut target = current;
    for step in MIGRATION_STEPS {
        if step.target_version > current {
            target = step.target_version;
            statements.extend_from_slice(step.statements);
        }
    }
    if statements.is_empty() {
        return Ok(MigrationReport {
            from_version: current,
            to_version: current,
            statements,
            dry_run,
        });
    }
    if dry_run {
        return Ok(MigrationReport {
            from_version: current,
            to_version: target,
            statements,
            dry_run,
        });
    }
    conn.execute("BEGIN IMMEDIATE", [])
        .map_err(|e| SqliteGraphError::schema(e.to_string()))?;
    let result: Result<(), SqliteGraphError> = (|| {
        for sql in statements.iter().copied() {
            conn.execute(sql, [])
                .map_err(|e| SqliteGraphError::schema(e.to_string()))?;
        }
        conn.execute(
            "UPDATE graph_meta SET schema_version=?1 WHERE id=1",
            [target],
        )
        .map_err(|e| SqliteGraphError::schema(e.to_string()))?;
        Ok(())
    })();
    match result {
        Ok(()) => {
            conn.execute("COMMIT", [])
                .map_err(|e| SqliteGraphError::schema(e.to_string()))?;
        }
        Err(err) => {
            let _ = conn.execute("ROLLBACK", []);
            return Err(err);
        }
    }
    Ok(MigrationReport {
        from_version: current,
        to_version: target,
        statements,
        dry_run,
    })
}

fn ensure_meta(conn: &Connection) -> Result<(), SqliteGraphError> {
    let version: Option<i64> = conn
        .query_row(
            "SELECT schema_version FROM graph_meta WHERE id=1",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| SqliteGraphError::schema(e.to_string()))?;
    match version {
        Some(existing) => {
            if existing > SCHEMA_VERSION {
                return Err(SqliteGraphError::schema(format!(
                    "database schema version {existing} is newer than supported {SCHEMA_VERSION}"
                )));
            }
            if existing < BASE_SCHEMA_VERSION {
                conn.execute(
                    "UPDATE graph_meta SET schema_version=?1 WHERE id=1",
                    [BASE_SCHEMA_VERSION],
                )
                .map_err(|e| SqliteGraphError::schema(e.to_string()))?;
            }
        }
        None => {
            conn.execute(
                "INSERT INTO graph_meta(id, schema_version) VALUES(1, ?1)",
                [BASE_SCHEMA_VERSION],
            )
            .map_err(|e| SqliteGraphError::schema(e.to_string()))?;
        }
    }
    Ok(())
}
