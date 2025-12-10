use crate::storage::database::DatabaseStorage;
use crate::storage::postgres::PostgresStorage;
use crate::storage::sqlite::SqliteStorage;
use crate::utils::config::StorageConfig;
use crate::utils::errors::Result;
use std::sync::Arc;

pub async fn create_storage(config: &StorageConfig) -> Result<Arc<dyn DatabaseStorage>> {
    match &config.postgres_url {
        Some(url) => {
            tracing::info!("Using PostgreSQL: {}", url);
            Ok(Arc::new(PostgresStorage::new(url).await?))
        }
        None => {
            tracing::info!("Using SQLite: {:?}", config.sqlite_path);
            Ok(Arc::new(SqliteStorage::new(config.sqlite_path.clone()).await?))
        }
    }
}
