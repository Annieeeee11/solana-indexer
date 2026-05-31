//! SQL for SQLite and PostgreSQL — per-backend macros (shared read shape, dialect-specific writes).

macro_rules! sqlite_backend {
    () => {
        pub mod sqlite {
            pub const GET_ACCOUNT: &str = concat!(
                "SELECT address, slot, lamports, owner, executable, data, rent_epoch ",
                "FROM accounts WHERE address = ?1"
            );
            pub const GET_SLOT: &str = concat!(
                "SELECT slot_number, timestamp, parent, status, block_hash, block_height ",
                "FROM slots WHERE slot_number = ?1"
            );
            pub const GET_LATEST_SLOT: &str = concat!(
                "SELECT slot_number, timestamp, parent, status, block_hash, block_height ",
                "FROM slots ORDER BY slot_number DESC LIMIT 1"
            );
            pub const GET_TRANSACTION: &str = concat!(
                "SELECT signature, slot, block_time, fee, success, accounts ",
                "FROM transactions WHERE signature = ?1"
            );
            pub const LIST_WALLETS_ALL: &str =
                "SELECT address, name, created_at FROM wallets ORDER BY created_at DESC";

            pub const STORE_SLOT: &str = "\
                INSERT OR REPLACE INTO slots \
                (slot_number, timestamp, parent, status, block_hash, block_height) \
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

            pub const STORE_ACCOUNT: &str = "\
                INSERT OR REPLACE INTO accounts \
                (address, slot, lamports, owner, executable, data, rent_epoch, updated_at) \
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)";

            pub const STORE_TRANSACTION: &str = "\
                INSERT OR REPLACE INTO transactions \
                (signature, slot, block_time, fee, success, accounts) \
                VALUES (?1, ?2, ?3, ?4, ?5, ?6)";

            pub const ADD_WALLET: &str = "\
                INSERT OR REPLACE INTO wallets (address, name, is_active, created_at) \
                VALUES (?1, ?2, 1, ?3)";

            pub const REMOVE_WALLET: &str =
                "UPDATE wallets SET is_active = 0 WHERE address = ?1";

            pub const LIST_WALLETS_ACTIVE: &str = "\
                SELECT address, name, created_at FROM wallets \
                WHERE is_active = 1 ORDER BY created_at DESC";

            pub const GET_ACTIVE_WALLETS: &str =
                "SELECT address FROM wallets WHERE is_active = 1";
        }
    };
}

macro_rules! postgres_backend {
    () => {
        pub mod postgres {
            pub const GET_ACCOUNT: &str = concat!(
                "SELECT address, slot, lamports, owner, executable, data, rent_epoch ",
                "FROM accounts WHERE address = $1"
            );
            pub const GET_SLOT: &str = concat!(
                "SELECT slot_number, timestamp, parent, status, block_hash, block_height ",
                "FROM slots WHERE slot_number = $1"
            );
            pub const GET_LATEST_SLOT: &str = concat!(
                "SELECT slot_number, timestamp, parent, status, block_hash, block_height ",
                "FROM slots ORDER BY slot_number DESC LIMIT 1"
            );
            pub const GET_TRANSACTION: &str = concat!(
                "SELECT signature, slot, block_time, fee, success, accounts ",
                "FROM transactions WHERE signature = $1"
            );
            pub const LIST_WALLETS_ALL: &str =
                "SELECT address, name, created_at FROM wallets ORDER BY created_at DESC";

            pub const STORE_SLOT: &str = "\
                INSERT INTO slots (slot_number, timestamp, parent, status, block_hash, block_height) \
                VALUES ($1, $2, $3, $4, $5, $6) \
                ON CONFLICT (slot_number) DO UPDATE SET \
                timestamp = $2, parent = $3, status = $4, block_hash = $5, block_height = $6";

            pub const STORE_ACCOUNT: &str = "\
                INSERT INTO accounts \
                (address, slot, lamports, owner, executable, data, rent_epoch, updated_at) \
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8) \
                ON CONFLICT (address) DO UPDATE SET \
                slot = $2, lamports = $3, owner = $4, executable = $5, data = $6, rent_epoch = $7, updated_at = $8";

            pub const STORE_TRANSACTION: &str = "\
                INSERT INTO transactions (signature, slot, block_time, fee, success, accounts) \
                VALUES ($1, $2, $3, $4, $5, $6) \
                ON CONFLICT (signature) DO UPDATE SET \
                slot = $2, block_time = $3, fee = $4, success = $5, accounts = $6";

            pub const ADD_WALLET: &str = "\
                INSERT INTO wallets (address, name, is_active, created_at) \
                VALUES ($1, $2, TRUE, $3) \
                ON CONFLICT (address) DO UPDATE SET name = $2, is_active = TRUE, created_at = $3";

            pub const REMOVE_WALLET: &str =
                "UPDATE wallets SET is_active = FALSE WHERE address = $1";

            pub const LIST_WALLETS_ACTIVE: &str = "\
                SELECT address, name, created_at FROM wallets \
                WHERE is_active = TRUE ORDER BY created_at DESC";

            pub const GET_ACTIVE_WALLETS: &str =
                "SELECT address FROM wallets WHERE is_active = TRUE";
        }
    };
}

sqlite_backend!();
postgres_backend!();

#[cfg(test)]
mod tests {
    use super::{postgres, sqlite};

    #[test]
    fn read_queries_share_latest_slot_sql() {
        assert_eq!(sqlite::GET_LATEST_SLOT, postgres::GET_LATEST_SLOT);
        assert_eq!(sqlite::LIST_WALLETS_ALL, postgres::LIST_WALLETS_ALL);
        assert!(sqlite::GET_SLOT.contains("?1"));
        assert!(postgres::GET_SLOT.contains("$1"));
    }
}
