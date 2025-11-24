use crate::{SqliteGraphError, backend::SqliteGraphBackend};
use std::env;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackendKind {
    Sqlite,
    #[allow(dead_code)]
    Neo4j,
}

impl BackendKind {
    pub fn from_env() -> Self {
        match env::var("GRAPH_BACKEND").as_deref() {
            Ok("neo4j") => BackendKind::Neo4j,
            _ => BackendKind::Sqlite,
        }
    }
}

pub struct GraphBackendFactory {
    kind: BackendKind,
}

impl GraphBackendFactory {
    pub fn new(kind: BackendKind) -> Self {
        Self { kind }
    }

    pub fn from_env() -> Self {
        Self::new(BackendKind::from_env())
    }

    pub fn new_sqlite(&self) -> Result<SqliteGraphBackend, SqliteGraphError> {
        match self.kind {
            BackendKind::Sqlite => SqliteGraphBackend::in_memory(),
            BackendKind::Neo4j => panic!("Neo4j backend not implemented"),
        }
    }
}
