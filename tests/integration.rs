//! Live network smoke tests (ignored by default).
//!
//! Run with env vars set:
//!   cargo test --test integration -- --ignored --test-threads=1
//!
//! Requires:
//!   SOLANA_RPC_URL          — for RPC tests
//!   YELLOWSTONE_GRPC_URL    — for Yellowstone test
//!   YELLOWSTONE_GRPC_TOKEN  — optional
//!   DATABASE_URL            — for PostgreSQL test

use solana_indexer::core::types::{Slot, SlotStatus};
use solana_indexer::data_sources::solana_rpc::SolanaRpc;
use solana_indexer::data_sources::yellowstone_grpc::YellowstoneGrpc;
use solana_indexer::data_sources::{AccountSource, SlotSource, YellowstoneSource};
use solana_indexer::storage::database::DatabaseStorage;
use solana_indexer::storage::factory::create_storage;
use solana_indexer::utils::config::StorageConfig;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

fn load_dotenv() {
    let _ = dotenvy::dotenv();
}

#[tokio::test]
#[ignore = "requires SOLANA_RPC_URL"]
async fn rpc_returns_slot_leader() {
    load_dotenv();
    let url = std::env::var("SOLANA_RPC_URL").expect("set SOLANA_RPC_URL for this test");
    let rpc = SolanaRpc::new(&url, solana_indexer::utils::metrics::IndexerMetrics::new());
    let slot = rpc.current_slot().await.expect("current_slot should succeed");
    let leader = rpc
        .get_leader_at_slot(slot)
        .await
        .expect("get_leader_at_slot should succeed");
    assert!(!leader.is_empty(), "leader pubkey should be non-empty");
}

#[tokio::test]
#[ignore = "requires SOLANA_RPC_URL"]
async fn rpc_fetches_wrapped_sol_account() {
    load_dotenv();
    let url = std::env::var("SOLANA_RPC_URL").expect("set SOLANA_RPC_URL for this test");
    let rpc = Arc::new(SolanaRpc::new(
        &url,
        solana_indexer::utils::metrics::IndexerMetrics::new(),
    )) as Arc<dyn AccountSource>;

    let account = rpc
        .get_account("So11111111111111111111111111111111111111112")
        .await
        .expect("get_account should succeed");
    assert!(account.lamports > 0);
}

#[tokio::test]
#[ignore = "requires YELLOWSTONE_GRPC_URL"]
async fn yellowstone_streams_a_slot() {
    load_dotenv();
    let url =
        std::env::var("YELLOWSTONE_GRPC_URL").expect("set YELLOWSTONE_GRPC_URL for this test");
    let token = std::env::var("YELLOWSTONE_GRPC_TOKEN").ok();

    let client = YellowstoneGrpc::new(&url, token);
    let grpc = Arc::new(client) as Arc<dyn YellowstoneSource>;

    let (mut slot_rx, _tx_rx) = grpc
        .subscribe_with_transactions()
        .await
        .expect("Yellowstone subscribe should connect");

    let slot = tokio::time::timeout(Duration::from_secs(45), slot_rx.recv())
        .await
        .expect("should receive a slot within 45s")
        .expect("slot channel should not close immediately");

    assert!(slot.slot > 0, "slot number should be positive");
}

#[tokio::test]
#[ignore = "requires DATABASE_URL"]
async fn postgres_stores_and_reads_slot() {
    load_dotenv();
    let url = std::env::var("DATABASE_URL").expect("set DATABASE_URL for this test");

    let storage = create_storage(&StorageConfig {
        sqlite_path: PathBuf::from("indexer.db"),
        postgres_url: Some(url),
    })
    .await
    .expect("postgres storage should connect");

    let slot = Slot {
        slot: 9_999_999_999,
        parent: Some(9_999_999_998),
        status: SlotStatus::Confirmed,
        timestamp: 1,
        block_hash: Some("test-hash".into()),
        block_height: Some(1),
    };

    storage.store_slot(&slot).await.expect("store_slot should succeed");
    let read = storage
        .get_slot(slot.slot)
        .await
        .expect("get_slot should succeed")
        .expect("slot should exist in postgres");
    assert_eq!(read.slot, slot.slot);
    assert_eq!(read.block_hash.as_deref(), Some("test-hash"));
}
