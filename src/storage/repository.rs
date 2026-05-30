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
