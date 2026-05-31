use crate::utils::errors::{IndexerError, Result};
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use std::str::FromStr;
use std::time::Duration;

pub struct PostgresStorage {
    pool: PgPool,
}

impl PostgresStorage {
    pub async fn new(url: &str) -> Result<Self> {
        let opts = PgConnectOptions::from_str(url)
            .map_err(|e| IndexerError::DatabaseError(e.to_string()))?;

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .acquire_timeout(Duration::from_secs(30))
            .connect_with(opts)
            .await?;

        crate::storage::repository::run_postgres_migrations(&pool).await?;
        Ok(Self { pool })
    }
}

crate::impl_database_storage!(PostgresStorage, postgres, postgres);
