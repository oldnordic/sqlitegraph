use rand::Rng;
use rusqlite::Connection;
use sqlitegraph::schema::{
    BASE_SCHEMA_VERSION, SCHEMA_VERSION, ensure_schema, read_schema_version, run_pending_migrations,
};

#[path = "fuzz_common.rs"]
mod fuzz_common;

fn reset_to_base(conn: &Connection) {
    conn.execute(
        "UPDATE graph_meta SET schema_version=?1 WHERE id=1",
        [BASE_SCHEMA_VERSION],
    )
    .unwrap();
    let _ = conn.execute("DROP TABLE IF EXISTS graph_meta_history", []);
}

#[test]
fn dry_run_reports_pending_steps_without_updating_version() {
    let conn = Connection::open_in_memory().unwrap();
    ensure_schema(&conn).unwrap();
    reset_to_base(&conn);

    let report = run_pending_migrations(&conn, true).unwrap();
    assert!(report.to_version > report.from_version);
    assert!(!report.statements.is_empty());
    assert!(report.dry_run);

    let version = read_schema_version(&conn).unwrap();
    assert_eq!(version, BASE_SCHEMA_VERSION);
}

#[test]
fn apply_migrations_updates_version() {
    let conn = Connection::open_in_memory().unwrap();
    ensure_schema(&conn).unwrap();
    reset_to_base(&conn);

    let report = run_pending_migrations(&conn, false).unwrap();
    assert!(report.to_version >= SCHEMA_VERSION);
    assert!(!report.statements.is_empty());
    assert!(!report.dry_run);

    let version = read_schema_version(&conn).unwrap();
    assert_eq!(version, SCHEMA_VERSION);

    let exists = conn
        .prepare("SELECT name FROM sqlite_master WHERE name='graph_meta_history'")
        .unwrap()
        .exists([])
        .unwrap();
    assert!(exists);
}

#[test]
fn random_migration_sequences_stabilize() {
    let iterations = fuzz_common::fuzz_iterations();
    let mut rng = fuzz_common::labeled_rng("migration-fuzz");
    for _ in 0..iterations {
        let conn = Connection::open_in_memory().unwrap();
        ensure_schema(&conn).unwrap();
        randomize_schema_state(&conn, &mut rng);
        let dry = run_pending_migrations(&conn, true).unwrap();
        if dry.to_version == dry.from_version {
            continue;
        }
        assert!(!dry.statements.is_empty());
        let applied = run_pending_migrations(&conn, false).unwrap();
        assert_eq!(applied.to_version, SCHEMA_VERSION);
        assert_eq!(read_schema_version(&conn).unwrap(), SCHEMA_VERSION);
    }
}

fn randomize_schema_state(conn: &Connection, rng: &mut impl Rng) {
    reset_to_base(conn);
    if rng.gen_bool(0.5) {
        let _ = conn.execute("DROP TABLE IF EXISTS graph_meta_history", []);
    }
    if rng.gen_bool(0.2) {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS graph_meta_history(version INTEGER NOT NULL, applied_at TEXT)",
            [],
        )
        .unwrap();
        let count = rng.gen_range(1..=3);
        for version in 0..count {
            let value = BASE_SCHEMA_VERSION + version as i64;
            let _ = conn.execute(
                "INSERT INTO graph_meta_history(version, applied_at) VALUES(?1, CURRENT_TIMESTAMP)",
                [value],
            );
        }
    }
}
