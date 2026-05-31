use crate::utils::errors::{IndexerError, Result};
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use std::str::FromStr;

pub struct PostgresStorage {
    pool: PgPool,
}

impl PostgresStorage {
    pub async fn new(url: &str) -> Result<Self> {
        let opts = PgConnectOptions::from_str(url)
            .map_err(|e| IndexerError::DatabaseError(e.to_string()))?;

        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect_with(opts)
            .await?;

        crate::storage::repository::run_postgres_migrations(&pool).await?;
        Ok(Self { pool })
    }
}

crate::impl_database_storage!(PostgresStorage, postgres, postgres);
