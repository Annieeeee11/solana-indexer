use crate::core::types::AccountState;
use crate::storage::database::DatabaseStorage;
use crate::utils::errors::Result;
use std::sync::Arc;

/// L3 Cache: Persistent database for account states
pub struct L3Accounts {
    db: Arc<dyn DatabaseStorage>,
}

impl L3Accounts {
    pub fn new(db: Arc<dyn DatabaseStorage>) -> Self {
        Self { db }
    }

    pub async fn get(&self, address: &str) -> Result<Option<AccountState>> {
        self.db.get_account(address).await
    }

    pub async fn insert(&self, account: AccountState) -> Result<()> {
        self.db.store_account(account).await
    }
}