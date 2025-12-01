use thiserror::Error;

/// Error type for SQLiteGraph operations.
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
    #[error("fault injected: {0}")]
    FaultInjected(String),
    #[error("transaction error: {0}")]
    TransactionError(String),
    #[error("validation error: {0}")]
    ValidationError(String),
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

    pub fn fault_injection<T: Into<String>>(msg: T) -> Self {
        SqliteGraphError::FaultInjected(msg.into())
    }

    pub fn transaction<T: Into<String>>(msg: T) -> Self {
        SqliteGraphError::TransactionError(msg.into())
    }

    pub fn validation<T: Into<String>>(msg: T) -> Self {
        SqliteGraphError::ValidationError(msg.into())
    }
}
