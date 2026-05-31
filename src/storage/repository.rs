use crate::utils::errors::{IndexerError, Result};
#[cfg(feature = "postgres")]
use sqlx::postgres::PgPool;
#[cfg(feature = "sqlite")]
use sqlx::sqlite::SqlitePool;

fn is_modified_migration_error(err: &sqlx::migrate::MigrateError) -> bool {
    err.to_string()
        .contains("previously applied but has been modified")
}

macro_rules! run_migrations {
    ($pool:expr) => {{
        match sqlx::migrate!("./migrations").run($pool).await {
            Ok(()) => Ok(()),
            Err(e) if is_modified_migration_error(&e) => {
                if std::env::var("ALLOW_MIGRATION_RESET").ok().as_deref() != Some("1") {
                    return Err(IndexerError::ConfigError(format!(
                        "Migration checksum mismatch: {e}. \
                         Set ALLOW_MIGRATION_RESET=1 in development to re-apply migrations, \
                         or delete the database and start fresh."
                    )));
                }

                tracing::warn!(
                    "ALLOW_MIGRATION_RESET=1: clearing migration history and re-applying all migrations"
                );
                sqlx::query("DELETE FROM _sqlx_migrations")
                    .execute($pool)
                    .await?;
                sqlx::migrate!("./migrations").run($pool).await?;
                Ok(())
            }
            Err(e) => Err(e.into()),
        }
    }};
}

#[cfg(feature = "sqlite")]
pub async fn run_sqlite_migrations(pool: &SqlitePool) -> Result<()> {
    run_migrations!(pool)
}

#[cfg(feature = "postgres")]
pub async fn run_postgres_migrations(pool: &PgPool) -> Result<()> {
    run_migrations!(pool)
}

/// Shared field → domain mapping (one source of truth for SQLite and Postgres rows).
pub mod mappers {
    use crate::core::types::{AccountState, Slot, SlotStatus, Transaction};

    pub fn account(
        address: String,
        slot: i64,
        lamports: i64,
        owner: String,
        executable: bool,
        data: Vec<u8>,
        rent_epoch: i64,
    ) -> AccountState {
        AccountState {
            address,
            slot: slot as u64,
            lamports: lamports as u64,
            owner,
            executable,
            data,
            rent_epoch: rent_epoch as u64,
        }
    }

    pub fn slot(
        slot: i64,
        timestamp: i64,
        parent: Option<i64>,
        status: &str,
        block_hash: Option<String>,
        block_height: Option<i64>,
    ) -> Slot {
        Slot {
            slot: slot as u64,
            timestamp,
            parent: parent.map(|p| p as u64),
            status: status.parse().unwrap_or(SlotStatus::Processed),
            block_hash,
            block_height: block_height.map(|h| h as u64),
        }
    }

    pub fn transaction(
        signature: String,
        slot: i64,
        block_time: Option<i64>,
        fee: i64,
        success: bool,
        accounts_json: &str,
    ) -> Transaction {
        Transaction {
            signature,
            slot: slot as u64,
            block_time,
            fee: fee as u64,
            success,
            accounts: serde_json::from_str(accounts_json).unwrap_or_default(),
        }
    }

    pub fn wallet(address: String, name: Option<String>, created_at: i64) -> (String, Option<String>, i64) {
        (address, name, created_at)
    }
}

macro_rules! row_mappers {
    ($mod_name:ident, $row:ty) => {
        pub mod $mod_name {
            use sqlx::Row;

            pub fn map_account(row: &$row) -> crate::core::types::AccountState {
                crate::storage::repository::mappers::account(
                    row.get(0),
                    row.get(1),
                    row.get(2),
                    row.get(3),
                    row.get(4),
                    row.get(5),
                    row.get(6),
                )
            }

            pub fn map_slot(row: &$row) -> crate::core::types::Slot {
                crate::storage::repository::mappers::slot(
                    row.get(0),
                    row.get(1),
                    row.get(2),
                    row.get(3),
                    row.get(4),
                    row.get(5),
                )
            }

            pub fn map_transaction(row: &$row) -> crate::core::types::Transaction {
                crate::storage::repository::mappers::transaction(
                    row.get(0),
                    row.get(1),
                    row.get(2),
                    row.get(3),
                    row.get(4),
                    row.get(5),
                )
            }

            pub fn map_wallet(row: &$row) -> (String, Option<String>, i64) {
                crate::storage::repository::mappers::wallet(row.get(0), row.get(1), row.get(2))
            }
        }
    };
}

#[cfg(feature = "sqlite")]
row_mappers!(sqlite, sqlx::sqlite::SqliteRow);
#[cfg(feature = "postgres")]
row_mappers!(postgres, sqlx::postgres::PgRow);

/// Shared `DatabaseStorage` implementation for SQLite and PostgreSQL backends.
#[macro_export]
macro_rules! impl_database_storage {
    ($storage:ty, $queries:ident, $mapper:ident) => {
        #[async_trait::async_trait]
        impl $crate::storage::database::DatabaseStorage for $storage {
            async fn store_slot(
                &self,
                slot: &$crate::core::types::Slot,
            ) -> $crate::utils::errors::Result<()> {
                sqlx::query($crate::storage::queries::$queries::STORE_SLOT)
                    .bind(slot.slot as i64)
                    .bind(slot.timestamp)
                    .bind(slot.parent.map(|p| p as i64))
                    .bind(slot.status.as_str())
                    .bind(&slot.block_hash)
                    .bind(slot.block_height.map(|h| h as i64))
                    .execute(&self.pool)
                    .await?;
                Ok(())
            }

            async fn store_account(
                &self,
                a: $crate::core::types::AccountState,
            ) -> $crate::utils::errors::Result<()> {
                sqlx::query($crate::storage::queries::$queries::STORE_ACCOUNT)
                    .bind(&a.address)
                    .bind(a.slot as i64)
                    .bind(a.lamports as i64)
                    .bind(&a.owner)
                    .bind(a.executable)
                    .bind(&a.data)
                    .bind(a.rent_epoch as i64)
                    .bind(chrono::Utc::now().timestamp())
                    .execute(&self.pool)
                    .await?;
                Ok(())
            }

            async fn get_account(
                &self,
                address: &str,
            ) -> $crate::utils::errors::Result<
                Option<$crate::core::types::AccountState>,
            > {
                let row = sqlx::query($crate::storage::queries::$queries::GET_ACCOUNT)
                    .bind(address)
                    .fetch_optional(&self.pool)
                    .await?;
                Ok(row
                    .as_ref()
                    .map($crate::storage::repository::$mapper::map_account))
            }

            async fn get_slot(
                &self,
                slot: u64,
            ) -> $crate::utils::errors::Result<Option<$crate::core::types::Slot>> {
                let row = sqlx::query($crate::storage::queries::$queries::GET_SLOT)
                    .bind(slot as i64)
                    .fetch_optional(&self.pool)
                    .await?;
                Ok(row
                    .as_ref()
                    .map($crate::storage::repository::$mapper::map_slot))
            }

            async fn get_latest_slot(
                &self,
            ) -> $crate::utils::errors::Result<Option<$crate::core::types::Slot>> {
                let row = sqlx::query($crate::storage::queries::$queries::GET_LATEST_SLOT)
                    .fetch_optional(&self.pool)
                    .await?;
                Ok(row
                    .as_ref()
                    .map($crate::storage::repository::$mapper::map_slot))
            }

            async fn store_transaction(
                &self,
                tx: $crate::core::types::Transaction,
            ) -> $crate::utils::errors::Result<()> {
                let accounts = serde_json::to_string(&tx.accounts)?;
                sqlx::query($crate::storage::queries::$queries::STORE_TRANSACTION)
                    .bind(&tx.signature)
                    .bind(tx.slot as i64)
                    .bind(tx.block_time)
                    .bind(tx.fee as i64)
                    .bind(tx.success)
                    .bind(&accounts)
                    .execute(&self.pool)
                    .await?;
                Ok(())
            }

            async fn get_transaction(
                &self,
                sig: &str,
            ) -> $crate::utils::errors::Result<Option<$crate::core::types::Transaction>> {
                let row = sqlx::query($crate::storage::queries::$queries::GET_TRANSACTION)
                    .bind(sig)
                    .fetch_optional(&self.pool)
                    .await?;
                Ok(row
                    .as_ref()
                    .map($crate::storage::repository::$mapper::map_transaction))
            }

            async fn add_wallet(
                &self,
                address: String,
                name: Option<String>,
            ) -> $crate::utils::errors::Result<()> {
                sqlx::query($crate::storage::queries::$queries::ADD_WALLET)
                    .bind(&address)
                    .bind(name.as_deref())
                    .bind(chrono::Utc::now().timestamp())
                    .execute(&self.pool)
                    .await?;
                Ok(())
            }

            async fn remove_wallet(&self, address: &str) -> $crate::utils::errors::Result<()> {
                sqlx::query($crate::storage::queries::$queries::REMOVE_WALLET)
                    .bind(address)
                    .execute(&self.pool)
                    .await?;
                Ok(())
            }

            async fn list_wallets(
                &self,
                active_only: bool,
            ) -> $crate::utils::errors::Result<Vec<(String, Option<String>, i64)>> {
                let sql = if active_only {
                    $crate::storage::queries::$queries::LIST_WALLETS_ACTIVE
                } else {
                    $crate::storage::queries::$queries::LIST_WALLETS_ALL
                };
                let rows = sqlx::query(sql).fetch_all(&self.pool).await?;
                Ok(rows
                    .iter()
                    .map($crate::storage::repository::$mapper::map_wallet)
                    .collect())
            }

            async fn get_active_wallets(&self) -> $crate::utils::errors::Result<Vec<String>> {
                let rows = sqlx::query($crate::storage::queries::$queries::GET_ACTIVE_WALLETS)
                    .fetch_all(&self.pool)
                    .await?;
                Ok(rows.iter().map(|r| sqlx::Row::get(r, 0)).collect())
            }

            async fn get_checkpoint(&self) -> $crate::utils::errors::Result<Option<u64>> {
                let row = sqlx::query($crate::storage::queries::$queries::GET_CHECKPOINT)
                    .fetch_optional(&self.pool)
                    .await?;
                Ok(row.as_ref().map(|r| sqlx::Row::get::<i64, _>(r, 0) as u64))
            }

            async fn set_checkpoint(&self, slot: u64) -> $crate::utils::errors::Result<()> {
                sqlx::query($crate::storage::queries::$queries::SET_CHECKPOINT)
                    .bind(slot as i64)
                    .bind(chrono::Utc::now().timestamp())
                    .execute(&self.pool)
                    .await?;
                Ok(())
            }
        }
    };
}

