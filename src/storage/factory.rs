use crate::storage::database::DatabaseStorage;
use crate::utils::config::StorageConfig;
use crate::utils::errors::Result;
use std::sync::Arc;

#[cfg(not(any(feature = "sqlite", feature = "postgres")))]
compile_error!("At least one of `sqlite` or `postgres` features must be enabled");

pub async fn create_storage(config: &StorageConfig) -> Result<Arc<dyn DatabaseStorage>> {
    match &config.postgres_url {
        #[cfg(feature = "postgres")]
        Some(url) => {
            use crate::storage::postgres::PostgresStorage;
            use crate::utils::redact::redact_database_url;
            tracing::info!("Using PostgreSQL: {}", redact_database_url(url));
            Ok(Arc::new(PostgresStorage::new(url).await?))
        }
        #[cfg(not(feature = "postgres"))]
        Some(_) => Err(crate::utils::errors::IndexerError::ConfigError(
            "DATABASE_URL set but crate was built without the `postgres` feature".into(),
        )),
        None => {
            #[cfg(feature = "sqlite")]
            {
                use crate::storage::sqlite::SqliteStorage;
                tracing::info!("Using SQLite: {:?}", config.sqlite_path);
                Ok(Arc::new(SqliteStorage::new(config.sqlite_path.clone()).await?))
            }
            #[cfg(not(feature = "sqlite"))]
            {
                Err(crate::utils::errors::IndexerError::ConfigError(
                    "No DATABASE_URL and crate was built without the `sqlite` feature".into(),
                ))
            }
        }
    }
}
