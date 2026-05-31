use crate::utils::errors::{IndexerError, Result};
use sqlx::postgres::PgPool;
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

pub async fn run_sqlite_migrations(pool: &SqlitePool) -> Result<()> {
    run_migrations!(pool)
}

pub async fn run_postgres_migrations(pool: &PgPool) -> Result<()> {
    run_migrations!(pool)
}

macro_rules! row_mappers {
    ($mod_name:ident, $row:ty) => {
        pub mod $mod_name {
            use crate::core::types::{AccountState, Slot, SlotStatus, Transaction};
            use sqlx::Row;

            pub fn map_account(row: &$row) -> AccountState {
                AccountState {
                    address: row.get(0),
                    slot: row.get::<i64, _>(1) as u64,
                    lamports: row.get::<i64, _>(2) as u64,
                    owner: row.get(3),
                    executable: row.get(4),
                    data: row.get(5),
                    rent_epoch: row.get::<i64, _>(6) as u64,
                }
            }

            pub fn map_slot(row: &$row) -> Slot {
                Slot {
                    slot: row.get::<i64, _>(0) as u64,
                    timestamp: row.get(1),
                    parent: row.get::<Option<i64>, _>(2).map(|p| p as u64),
                    status: row.get::<&str, _>(3).parse().unwrap_or(SlotStatus::Processed),
                    block_hash: row.get(4),
                    block_height: row.get::<Option<i64>, _>(5).map(|h| h as u64),
                }
            }

            pub fn map_transaction(row: &$row) -> Transaction {
                Transaction {
                    signature: row.get(0),
                    slot: row.get::<i64, _>(1) as u64,
                    block_time: row.get(2),
                    fee: row.get::<i64, _>(3) as u64,
                    success: row.get(4),
                    accounts: serde_json::from_str(row.get(5)).unwrap_or_default(),
                }
            }

            pub fn map_wallet(row: &$row) -> (String, Option<String>, i64) {
                (row.get(0), row.get(1), row.get(2))
            }
        }
    };
}

row_mappers!(sqlite, sqlx::sqlite::SqliteRow);
row_mappers!(postgres, sqlx::postgres::PgRow);

/// Shared `DatabaseStorage` implementation for SQLite and PostgreSQL backends.
/// SQL strings come from `queries::$queries`; row mappers from `repository::$mapper`.
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
        }
    };
}

