-- Add block metadata columns to slots (aligns with core::types::Slot)
ALTER TABLE slots ADD COLUMN block_hash TEXT;
ALTER TABLE slots ADD COLUMN block_height BIGINT;
