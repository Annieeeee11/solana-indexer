use crate::core::types::{AccountState, Slot, TransactionInfo};
use crate::utils::errors::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Fetches on-chain account state (implemented by `SolanaRpc`).
#[async_trait]
pub trait AccountSource: Send + Sync {
    async fn get_account(&self, address: &str) -> Result<AccountState>;
}

/// RPC slot streaming and block reads (implemented by `SolanaRpc`).
#[async_trait]
pub trait SlotSource: Send + Sync {
    async fn subscribe_slots(&self) -> Result<mpsc::Receiver<Slot>>;

    async fn get_block_with_transactions(&self, slot: u64) -> Result<Vec<TransactionInfo>>;

    async fn get_slot_leader(&self) -> Result<String>;
}

/// Real-time slot + transaction streaming via Yellowstone gRPC.
#[async_trait]
pub trait YellowstoneSource: Send + Sync {
    async fn subscribe_with_transactions(
        &self,
    ) -> Result<(mpsc::Receiver<Slot>, mpsc::Receiver<TransactionInfo>)>;
}

pub mod solana_rpc;
pub mod yellowstone_grpc;
