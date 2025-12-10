-- Initial database schema for Solana Indexer
-- Uses common SQL syntax compatible with both SQLite and PostgreSQL

-- Slots table
CREATE TABLE IF NOT EXISTS slots (
    slot_number BIGINT PRIMARY KEY,
    timestamp BIGINT NOT NULL,
    parent BIGINT,
    status TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_slots_timestamp ON slots(timestamp);
CREATE INDEX IF NOT EXISTS idx_slots_parent ON slots(parent);

-- Accounts table
CREATE TABLE IF NOT EXISTS accounts (
    address TEXT PRIMARY KEY,
    slot BIGINT NOT NULL,
    lamports BIGINT NOT NULL,
    owner TEXT NOT NULL,
    executable BOOLEAN NOT NULL,
    data BYTEA,
    rent_epoch BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_accounts_slot ON accounts(slot);
CREATE INDEX IF NOT EXISTS idx_accounts_owner ON accounts(owner);

-- Transactions table
CREATE TABLE IF NOT EXISTS transactions (
    signature TEXT PRIMARY KEY,
    slot BIGINT NOT NULL,
    block_time BIGINT,
    fee BIGINT NOT NULL,
    success BOOLEAN NOT NULL,
    accounts TEXT
);

CREATE INDEX IF NOT EXISTS idx_transactions_slot ON transactions(slot);
CREATE INDEX IF NOT EXISTS idx_transactions_block_time ON transactions(block_time);

-- Wallets table (for monitoring)
CREATE TABLE IF NOT EXISTS wallets (
    address TEXT PRIMARY KEY,
    name TEXT,
    created_at BIGINT NOT NULL,
    is_active BOOLEAN NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_wallets_active ON wallets(is_active);