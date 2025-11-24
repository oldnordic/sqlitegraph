use rusqlite::Connection;
use sqlitegraph::schema::ensure_schema;

#[test]
fn test_schema_creates_label_and_property_tables() {
    let conn = Connection::open_in_memory().unwrap();
    ensure_schema(&conn).unwrap();
    assert!(table_exists(&conn, "graph_labels"));
    assert!(table_exists(&conn, "graph_properties"));
}

fn table_exists(conn: &Connection, name: &str) -> bool {
    conn.prepare("SELECT name FROM sqlite_master WHERE name=?1")
        .unwrap()
        .exists([name])
        .unwrap()
}
