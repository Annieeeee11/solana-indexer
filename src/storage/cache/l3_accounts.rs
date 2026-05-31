use crate::core::types::AccountState;
use crate::storage::database::DatabaseStorage;
use crate::utils::errors::Result;
use crate::utils::metrics::IndexerMetrics;
use moka::future::Cache;
use std::sync::atomic::Ordering;
use std::sync::Arc;

/// L3: in-memory account cache with database persistence and DB fallback on miss.
pub struct L3Accounts {
    memory: Arc<Cache<String, AccountState>>,
    db: Arc<dyn DatabaseStorage>,
    metrics: Arc<IndexerMetrics>,
}

impl L3Accounts {
    pub fn new(db: Arc<dyn DatabaseStorage>, max_size: usize, metrics: Arc<IndexerMetrics>) -> Self {
        let memory = Cache::builder()
            .max_capacity(max_size as u64)
            .build();

        Self {
            memory: Arc::new(memory),
            db,
            metrics,
        }
    }

    pub async fn get(&self, address: &str) -> Result<Option<AccountState>> {
        if let Some(account) = self.memory.get(address).await {
            self.metrics.l3_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(Some(account));
        }
        if let Some(account) = self.db.get_account(address).await? {
            self.metrics.l3_hits.fetch_add(1, Ordering::Relaxed);
            self.memory
                .insert(account.address.clone(), account.clone())
                .await;
            return Ok(Some(account));
        }
        self.metrics.l3_misses.fetch_add(1, Ordering::Relaxed);
        Ok(None)
    }

    pub async fn store(&self, account: AccountState) -> Result<()> {
        self.memory
            .insert(account.address.clone(), account.clone())
            .await;
        self.db.store_account(account).await
    }
}
