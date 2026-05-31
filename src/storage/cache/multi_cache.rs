use crate::core::types::{AccountState, Slot, Transaction};
use crate::storage::cache::l1_hot_slots::L1HotSlots;
use crate::storage::cache::l2_transactions::L2Transactions;
use crate::storage::cache::l3_accounts::L3Accounts;
use crate::storage::database::DatabaseStorage;
use crate::utils::errors::Result;
use crate::utils::metrics::IndexerMetrics;
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub struct MultiCache {
    l1: Arc<L1HotSlots>,
    l2: Arc<L2Transactions>,
    l3: Arc<L3Accounts>,
    db: Arc<dyn DatabaseStorage>,
    metrics: Arc<IndexerMetrics>,
}

impl MultiCache {
    pub fn new(
        l1_size: usize,
        l2_size: usize,
        l3_size: usize,
        db: Arc<dyn DatabaseStorage>,
        metrics: Arc<IndexerMetrics>,
    ) -> Self {
        Self {
            l1: Arc::new(L1HotSlots::new(l1_size)),
            l2: Arc::new(L2Transactions::new(l2_size)),
            l3: Arc::new(L3Accounts::new(db.clone(), l3_size)),
            db,
            metrics,
        }
    }

    pub async fn store_slot(&self, slot: Slot) -> Result<()> {
        self.l1.insert(slot.clone()).await;
        self.metrics.slots_ingested.fetch_add(1, Ordering::Relaxed);
        self.db.store_slot(&slot).await
    }

    pub async fn store_transaction(&self, tx: Transaction) -> Result<()> {
        self.l2.insert(tx.clone()).await;
        self.metrics.txs_ingested.fetch_add(1, Ordering::Relaxed);
        self.db.store_transaction(tx).await
    }

    pub async fn get_account(&self, address: &str) -> Result<Option<AccountState>> {
        self.l3.get(address).await
    }

    pub async fn store_account(&self, account: AccountState) -> Result<()> {
        self.l3.store(account).await
    }

    pub async fn get_slot(&self, slot: u64) -> Result<Option<Slot>> {
        if let Some(cached) = self.l1.get(slot).await {
            self.metrics.l1_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(Some(cached));
        }
        self.metrics.l1_misses.fetch_add(1, Ordering::Relaxed);
        if let Some(slot) = self.db.get_slot(slot).await? {
            self.l1.insert(slot.clone()).await;
            return Ok(Some(slot));
        }
        Ok(None)
    }

    pub async fn get_latest_slot(&self) -> Result<Option<Slot>> {
        if let Some(cached) = self.l1.get_latest_slot().await {
            self.metrics.l1_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(Some(cached));
        }
        self.metrics.l1_misses.fetch_add(1, Ordering::Relaxed);
        if let Some(slot) = self.db.get_latest_slot().await? {
            self.l1.insert(slot.clone()).await;
            return Ok(Some(slot));
        }
        Ok(None)
    }

    pub async fn get_transaction(&self, signature: &str) -> Result<Option<Transaction>> {
        if let Some(cached) = self.l2.get(signature).await {
            self.metrics.l2_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(Some(cached));
        }
        self.metrics.l2_misses.fetch_add(1, Ordering::Relaxed);
        if let Some(tx) = self.db.get_transaction(signature).await? {
            self.l2.insert(tx.clone()).await;
            return Ok(Some(tx));
        }
        Ok(None)
    }

    pub async fn add_wallet(&self, address: String, name: Option<String>) -> Result<()> {
        self.db.add_wallet(address, name).await
    }

    pub async fn remove_wallet(&self, address: &str) -> Result<()> {
        self.db.remove_wallet(address).await
    }

    pub async fn list_wallets(
        &self,
        active_only: bool,
    ) -> Result<Vec<(String, Option<String>, i64)>> {
        self.db.list_wallets(active_only).await
    }

    pub async fn get_active_wallets(&self) -> Result<Vec<String>> {
        self.db.get_active_wallets().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::types::Transaction;
    use crate::testing::fixtures::sample_slot;
    use crate::testing::mock_db::MockDatabase;

    fn test_cache(db: Arc<MockDatabase>) -> MultiCache {
        MultiCache::new(10, 10, 10, db, IndexerMetrics::new())
    }

    #[tokio::test]
    async fn get_slot_backfills_l1_from_db() {
        let db = Arc::new(MockDatabase::new());
        db.store_slot(&sample_slot(42)).await.unwrap();

        let cache = test_cache(db);
        let slot = cache.get_slot(42).await.unwrap().expect("slot in db");
        assert_eq!(slot.slot, 42);

        let cached = cache.get_slot(42).await.unwrap().expect("l1 hit");
        assert_eq!(cached.slot, 42);
    }

    #[tokio::test]
    async fn get_transaction_backfills_l2_from_db() {
        let db = Arc::new(MockDatabase::new());
        let tx = Transaction {
            signature: "sig1".into(),
            slot: 1,
            block_time: None,
            fee: 5000,
            success: true,
            accounts: vec![],
        };
        db.store_transaction(tx.clone()).await.unwrap();

        let cache = test_cache(db);
        let found = cache
            .get_transaction("sig1")
            .await
            .unwrap()
            .expect("tx in db");
        assert_eq!(found.fee, 5000);

        let cached = cache
            .get_transaction("sig1")
            .await
            .unwrap()
            .expect("l2 hit");
        assert_eq!(cached.fee, 5000);
    }
}
