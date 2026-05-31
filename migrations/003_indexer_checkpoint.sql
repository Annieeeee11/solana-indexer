-- Durable cursor for crash recovery and backfill (single-row table, id = 1).

CREATE TABLE IF NOT EXISTS indexer_checkpoint (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    last_processed_slot BIGINT NOT NULL,
    updated_at BIGINT NOT NULL
);
