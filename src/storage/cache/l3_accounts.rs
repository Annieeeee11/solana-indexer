use crate::core::types::AccountState;
use crate::storage::database::DatabaseStorage;
use crate::utils::errors::Result;
use moka::future::Cache;
use std::sync::Arc;

/// L3: in-memory account cache with database persistence and DB fallback on miss.
pub struct L3Accounts {
    memory: Arc<Cache<String, AccountState>>,
    db: Arc<dyn DatabaseStorage>,
}

impl L3Accounts {
    pub fn new(db: Arc<dyn DatabaseStorage>, max_size: usize) -> Self {
        let memory = Cache::builder()
            .max_capacity(max_size as u64)
            .build();

        Self {
            memory: Arc::new(memory),
            db,
        }
    }

    pub async fn get(&self, address: &str) -> Result<Option<AccountState>> {
        if let Some(account) = self.memory.get(address).await {
            return Ok(Some(account));
        }
        if let Some(account) = self.db.get_account(address).await? {
            self.memory
                .insert(account.address.clone(), account.clone())
                .await;
            return Ok(Some(account));
        }
        Ok(None)
    }

    pub async fn store(&self, account: AccountState) -> Result<()> {
        self.memory
            .insert(account.address.clone(), account.clone())
            .await;
        self.db.store_account(account).await
    }
}
