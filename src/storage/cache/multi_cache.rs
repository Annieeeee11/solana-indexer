use crate::core::types::{AccountState, Slot, SlotStatus, Transaction};
use crate::storage::cache::account_store::AccountStore;
use crate::storage::cache::l1_hot_slots::L1HotSlots;
use crate::storage::cache::l2_transactions::L2Transactions;
use crate::storage::database::DatabaseStorage;
use crate::utils::errors::Result;
use std::sync::Arc;

pub struct MultiCache {
    l1: Arc<L1HotSlots>,
    l2: Arc<L2Transactions>,
    accounts: Arc<AccountStore>,
}

impl MultiCache {
    pub fn new(l1_size: usize, l2_size: usize, db: Arc<dyn DatabaseStorage>) -> Self {
        Self {
            l1: Arc::new(L1HotSlots::new(l1_size)),
            l2: Arc::new(L2Transactions::new(l2_size)),
            accounts: Arc::new(AccountStore::new(db)),
        }
    }

    pub async fn store_slot(&self, slot: Slot) -> Result<()> {
        self.l1.insert(slot.clone()).await;
        let status = match slot.status {
            SlotStatus::Processed => "Processed",
            SlotStatus::Confirmed => "Confirmed",
            SlotStatus::Finalized => "Finalized",
        };
        self.accounts
            .db()
            .store_slot(slot.slot, slot.timestamp, slot.parent, status)
            .await
    }

    pub async fn store_transaction(&self, tx: Transaction) -> Result<()> {
        self.l2.insert(tx.clone()).await;
        self.accounts.db().store_transaction(tx).await
    }

    pub async fn get_account(&self, address: &str) -> Result<Option<AccountState>> {
        self.accounts.get(address).await
    }

    pub async fn store_account(&self, account: AccountState) -> Result<()> {
        self.accounts.store(account).await
    }

    /// L1 → DB fallback (populates L1 on DB hit).
    pub async fn get_slot(&self, slot: u64) -> Result<Option<Slot>> {
        if let Some(cached) = self.l1.get(slot).await {
            return Ok(Some(cached));
        }
        if let Some(slot) = self.accounts.db().get_slot(slot).await? {
            self.l1.insert(slot.clone()).await;
            return Ok(Some(slot));
        }
        Ok(None)
    }

    /// L1 hot cache → DB fallback (populates L1 on DB hit).
    pub async fn get_latest_slot(&self) -> Result<Option<Slot>> {
        if let Some(cached) = self.l1.get_latest_slot().await {
            return Ok(Some(cached));
        }
        if let Some(slot) = self.accounts.db().get_latest_slot().await? {
            self.l1.insert(slot.clone()).await;
            return Ok(Some(slot));
        }
        Ok(None)
    }

    /// L2 → DB fallback (populates L2 on DB hit).
    pub async fn get_transaction(&self, signature: &str) -> Result<Option<Transaction>> {
        if let Some(cached) = self.l2.get(signature).await {
            return Ok(Some(cached));
        }
        if let Some(tx) = self.accounts.db().get_transaction(signature).await? {
            self.l2.insert(tx.clone()).await;
            return Ok(Some(tx));
        }
        Ok(None)
    }
}
