//! Live network smoke tests (ignored by default).
//!
//! Run with env vars set:
//!   cargo test --test integration -- --ignored --test-threads=1
//!
//! Requires:
//!   SOLANA_RPC_URL          — for RPC tests
//!   YELLOWSTONE_GRPC_URL    — for Yellowstone test
//!   YELLOWSTONE_GRPC_TOKEN  — optional

use solana_indexer::data_sources::solana_rpc::SolanaRpc;
use solana_indexer::data_sources::yellowstone_grpc::YellowstoneGrpc;
use solana_indexer::data_sources::{AccountSource, SlotSource, YellowstoneSource};
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
    let rpc = Arc::new(SolanaRpc::new(&url)) as Arc<dyn SlotSource>;

    let leader = rpc
        .get_slot_leader()
        .await
        .expect("get_slot_leader should succeed");
    assert!(!leader.is_empty(), "leader pubkey should be non-empty");
}

#[tokio::test]
#[ignore = "requires SOLANA_RPC_URL"]
async fn rpc_fetches_wrapped_sol_account() {
    load_dotenv();
    let url = std::env::var("SOLANA_RPC_URL").expect("set SOLANA_RPC_URL for this test");
    let rpc = Arc::new(SolanaRpc::new(&url)) as Arc<dyn AccountSource>;

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
