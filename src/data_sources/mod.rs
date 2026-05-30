use crate::core::types::AccountState;
use crate::utils::errors::Result;
use async_trait::async_trait;

/// Fetches on-chain account state (implemented by `SolanaRpc`)
#[async_trait]
pub trait AccountSource: Send + Sync {
    async fn get_account(&self, address: &str) -> Result<AccountState>;
}

pub mod solana_rpc;
pub mod yellowstone_grpc;
