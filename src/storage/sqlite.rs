use crate::utils::errors::{IndexerError, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::PathBuf;
use std::str::FromStr;

pub struct SqliteStorage {
    pool: SqlitePool,
}

impl SqliteStorage {
    pub async fn new(path: PathBuf) -> Result<Self> {
        let url = format!("sqlite:{}", path.to_string_lossy());
        let opts = SqliteConnectOptions::from_str(&url)
            .map_err(|e| IndexerError::DatabaseError(e.to_string()))?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(opts)
            .await?;

        crate::storage::repository::run_sqlite_migrations(&pool).await?;
        Ok(Self { pool })
    }
}

crate::impl_database_storage!(SqliteStorage, sqlite, sqlite);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::{Slot, SlotStatus};
    use crate::storage::database::DatabaseStorage;

    #[tokio::test]
    async fn sqlite_persists_and_reads_slot() {
        let dir = tempfile::tempdir().unwrap();
        let storage = SqliteStorage::new(dir.path().join("test.db"))
            .await
            .expect("sqlite should init");

        let slot = Slot {
            slot: 99,
            parent: Some(98),
            status: SlotStatus::Confirmed,
            timestamp: 1_700_000_000,
            block_hash: Some("abc".into()),
            block_height: Some(100),
        };

        storage.store_slot(&slot).await.unwrap();
        let loaded = storage.get_slot(99).await.unwrap().expect("slot stored");
        assert_eq!(loaded.slot, 99);
        assert_eq!(loaded.block_hash.as_deref(), Some("abc"));
    }
}
