use thiserror::Error;

#[derive(Error, Debug)]
pub enum IndexerError {
    #[error("RPC error: {0}")]
    RpcError(String),

    #[error("Database error: {0}")]
    DatabaseError(String),

    #[error("Network error: {0}")]
    NetworkError(#[from] reqwest::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("gRPC error: {0}")]
    GrpcError(#[from] tonic::Status),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Cache error: {0}")]
    CacheError(String),

    #[error("Storage error: {0}")]
    StorageError(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
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