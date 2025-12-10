use crate::core::types::{AccountState, Slot, Transaction};
use crate::utils::errors::Result;

#[async_trait::async_trait]
pub trait DatabaseStorage: Send + Sync {
    async fn store_slot(
        &self,
        slot: u64,
        timestamp: i64,
        parent: Option<u64>,
        status: &str,
    ) -> Result<()>;

    async fn store_account(&self, account: AccountState) -> Result<()>;

    async fn get_account(&self, address: &str) -> Result<Option<AccountState>>;

    async fn get_slot(&self, slot: u64) -> Result<Option<Slot>>;

    async fn store_transaction(&self, tx: Transaction) -> Result<()>;

    async fn get_transaction(&self, signature: &str) -> Result<Option<Transaction>>;

    async fn get_latest_slot(&self) -> Result<Option<Slot>>;

    async fn add_wallet(&self, address: String, name: Option<String>) -> Result<()>;

    async fn remove_wallet(&self, address: &str) -> Result<()>;

    async fn list_wallets(&self, active_only: bool) -> Result<Vec<(String, Option<String>, i64)>>;

    async fn get_active_wallets(&self) -> Result<Vec<String>>;
}