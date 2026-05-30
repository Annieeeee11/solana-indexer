use crate::core::types::AccountState;
use crate::storage::database::DatabaseStorage;
use crate::utils::errors::Result;
use std::sync::Arc;

/// L3: database-backed account storage (diagram layer).
pub struct AccountStore {
    db: Arc<dyn DatabaseStorage>,
}

impl AccountStore {
    pub fn new(db: Arc<dyn DatabaseStorage>) -> Self {
        Self { db }
    }

    pub fn db(&self) -> &Arc<dyn DatabaseStorage> {
        &self.db
    }

    pub async fn get(&self, address: &str) -> Result<Option<AccountState>> {
        self.db.get_account(address).await
    }

    pub async fn store(&self, account: AccountState) -> Result<()> {
        self.db.store_account(account).await
    }
}
