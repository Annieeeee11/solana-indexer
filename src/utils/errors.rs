use thiserror::Error;

#[derive(Error, Debug)]
pub enum IndexerError {
    #[error("RPC error: {0}")]
    RpcError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Channel error: {0}")]
    ChannelError(String),
}

impl From<sqlx::Error> for IndexerError {
    fn from(e: sqlx::Error) -> Self {
        IndexerError::DatabaseError(e.to_string())
    }
}

impl From<sqlx::migrate::MigrateError> for IndexerError {
    fn from(e: sqlx::migrate::MigrateError) -> Self {
        IndexerError::DatabaseError(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, IndexerError>;
