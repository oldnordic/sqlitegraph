use thiserror::Error;

#[derive(Debug, Error)]
pub enum SqliteGraphError {
    #[error("connection error: {0}")]
    ConnectionError(String),
    #[error("schema error: {0}")]
    SchemaError(String),
    #[error("query error: {0}")]
    QueryError(String),
    #[error("entity not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
}

impl SqliteGraphError {
    pub fn connection<T: Into<String>>(msg: T) -> Self {
        SqliteGraphError::ConnectionError(msg.into())
    }

    pub fn schema<T: Into<String>>(msg: T) -> Self {
        SqliteGraphError::SchemaError(msg.into())
    }

    pub fn query<T: Into<String>>(msg: T) -> Self {
        SqliteGraphError::QueryError(msg.into())
    }

    pub fn not_found<T: Into<String>>(msg: T) -> Self {
        SqliteGraphError::NotFound(msg.into())
    }

    pub fn invalid_input<T: Into<String>>(msg: T) -> Self {
        SqliteGraphError::InvalidInput(msg.into())
    }
}
