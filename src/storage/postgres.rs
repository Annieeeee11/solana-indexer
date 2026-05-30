use crate::core::types::{AccountState, Slot, Transaction};
use crate::storage::database::DatabaseStorage;
use crate::storage::queries;
use crate::utils::errors::{IndexerError, Result};
use sqlx::postgres::{PgConnectOptions, PgPool, PgPoolOptions};
use sqlx::Row;
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

#[async_trait::async_trait]
impl DatabaseStorage for PostgresStorage {
    async fn store_slot(&self, slot: &Slot) -> Result<()> {
        sqlx::query(queries::postgres::STORE_SLOT)
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

    async fn store_account(&self, a: AccountState) -> Result<()> {
        sqlx::query(queries::postgres::STORE_ACCOUNT)
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

    async fn get_account(&self, address: &str) -> Result<Option<AccountState>> {
        let row = sqlx::query(queries::postgres::GET_ACCOUNT)
            .bind(address)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row
            .as_ref()
            .map(crate::storage::repository::postgres::map_account))
    }

    async fn get_slot(&self, slot: u64) -> Result<Option<Slot>> {
        let row = sqlx::query(queries::postgres::GET_SLOT)
            .bind(slot as i64)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(crate::storage::repository::postgres::map_slot))
    }

    async fn get_latest_slot(&self) -> Result<Option<Slot>> {
        let row = sqlx::query(queries::postgres::GET_LATEST_SLOT)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(crate::storage::repository::postgres::map_slot))
    }

    async fn store_transaction(&self, tx: Transaction) -> Result<()> {
        let accounts = serde_json::to_string(&tx.accounts)?;
        sqlx::query(queries::postgres::STORE_TRANSACTION)
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

    async fn get_transaction(&self, sig: &str) -> Result<Option<Transaction>> {
        let row = sqlx::query(queries::postgres::GET_TRANSACTION)
            .bind(sig)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row
            .as_ref()
            .map(crate::storage::repository::postgres::map_transaction))
    }

    async fn add_wallet(&self, address: String, name: Option<String>) -> Result<()> {
        sqlx::query(queries::postgres::ADD_WALLET)
            .bind(&address)
            .bind(name.as_deref())
            .bind(chrono::Utc::now().timestamp())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn remove_wallet(&self, address: &str) -> Result<()> {
        sqlx::query(queries::postgres::REMOVE_WALLET)
            .bind(address)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list_wallets(&self, active_only: bool) -> Result<Vec<(String, Option<String>, i64)>> {
        let sql = if active_only {
            queries::postgres::LIST_WALLETS_ACTIVE
        } else {
            queries::postgres::LIST_WALLETS_ALL
        };
        let rows = sqlx::query(sql).fetch_all(&self.pool).await?;
        Ok(rows
            .iter()
            .map(crate::storage::repository::postgres::map_wallet)
            .collect())
    }

    async fn get_active_wallets(&self) -> Result<Vec<String>> {
        let rows = sqlx::query(queries::postgres::GET_ACTIVE_WALLETS)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(|r| r.get(0)).collect())
    }
}
