use crate::utils::errors::Result;
use sqlx::postgres::PgPool;
use sqlx::sqlite::SqlitePool;

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
                    status: SlotStatus::from_str(row.get(3)),
                    block_hash: None,
                    block_height: None,
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

macro_rules! run_migrations_for {
    ($pool:expr) => {{
        if let Err(e) = sqlx::migrate!("./migrations").run($pool).await {
            let err_str = e.to_string();
            if err_str.contains("previously applied but has been modified") {
                tracing::warn!("Migration checksum mismatch detected. Resetting migration history...");
                sqlx::query("DELETE FROM _sqlx_migrations WHERE version = 1")
                    .execute($pool)
                    .await
                    .ok();
                sqlx::migrate!("./migrations").run($pool).await?;
            } else {
                return Err(e.into());
            }
        }
        Ok(())
    }};
}

pub async fn run_sqlite_migrations(pool: &SqlitePool) -> Result<()> {
    run_migrations_for!(pool)
}

pub async fn run_postgres_migrations(pool: &PgPool) -> Result<()> {
    run_migrations_for!(pool)
}
