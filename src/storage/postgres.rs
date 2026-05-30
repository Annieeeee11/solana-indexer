use crate::core::types::{AccountState, Slot, Transaction};
use crate::storage::database::DatabaseStorage;
use crate::storage::repository;
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

        repository::run_postgres_migrations(&pool).await?;
        Ok(Self { pool })
    }
}

#[async_trait::async_trait]
impl DatabaseStorage for PostgresStorage {
    async fn store_slot(&self, slot: &Slot) -> Result<()> {
        sqlx::query(
            "INSERT INTO slots (slot_number, timestamp, parent, status, block_hash, block_height) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (slot_number) DO UPDATE SET timestamp = $2, parent = $3, status = $4, block_hash = $5, block_height = $6",
        )
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
        sqlx::query(
            "INSERT INTO accounts (address, slot, lamports, owner, executable, data, rent_epoch, updated_at) VALUES ($1, $2, $3, $4, $5, $6, $7, $8) ON CONFLICT (address) DO UPDATE SET slot = $2, lamports = $3, owner = $4, executable = $5, data = $6, rent_epoch = $7, updated_at = $8",
        )
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
        let row = sqlx::query(
            "SELECT address, slot, lamports, owner, executable, data, rent_epoch FROM accounts WHERE address = $1",
        )
        .bind(address)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(repository::postgres::map_account))
    }

    async fn get_slot(&self, slot: u64) -> Result<Option<Slot>> {
        let row = sqlx::query(
            "SELECT slot_number, timestamp, parent, status, block_hash, block_height FROM slots WHERE slot_number = $1",
        )
        .bind(slot as i64)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(repository::postgres::map_slot))
    }

    async fn get_latest_slot(&self) -> Result<Option<Slot>> {
        let row = sqlx::query(
            "SELECT slot_number, timestamp, parent, status, block_hash, block_height FROM slots ORDER BY slot_number DESC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(repository::postgres::map_slot))
    }

    async fn store_transaction(&self, tx: Transaction) -> Result<()> {
        let accounts = serde_json::to_string(&tx.accounts)?;
        sqlx::query(
            "INSERT INTO transactions (signature, slot, block_time, fee, success, accounts) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (signature) DO UPDATE SET slot = $2, block_time = $3, fee = $4, success = $5, accounts = $6",
        )
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
        let row = sqlx::query(
            "SELECT signature, slot, block_time, fee, success, accounts FROM transactions WHERE signature = $1",
        )
        .bind(sig)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.as_ref().map(repository::postgres::map_transaction))
    }

    async fn add_wallet(&self, address: String, name: Option<String>) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        sqlx::query(
            "INSERT INTO wallets (address, name, is_active, created_at) VALUES ($1, $2, TRUE, $3) ON CONFLICT (address) DO UPDATE SET name = $2, is_active = TRUE, created_at = $3",
        )
        .bind(&address)
        .bind(name.as_deref())
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn remove_wallet(&self, address: &str) -> Result<()> {
        sqlx::query("UPDATE wallets SET is_active = FALSE WHERE address = $1")
            .bind(address)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list_wallets(&self, active_only: bool) -> Result<Vec<(String, Option<String>, i64)>> {
        let sql = if active_only {
            "SELECT address, name, created_at FROM wallets WHERE is_active = TRUE ORDER BY created_at DESC"
        } else {
            "SELECT address, name, created_at FROM wallets ORDER BY created_at DESC"
        };
        let rows = sqlx::query(sql).fetch_all(&self.pool).await?;
        Ok(rows.iter().map(repository::postgres::map_wallet).collect())
    }

    async fn get_active_wallets(&self) -> Result<Vec<String>> {
        let rows = sqlx::query("SELECT address FROM wallets WHERE is_active = TRUE")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(|r| r.get(0)).collect())
    }
}
