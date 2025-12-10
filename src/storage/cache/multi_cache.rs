use crate::core::types::{AccountState, Slot, Transaction};
use crate::storage::cache::l1_hot_slots::L1HotSlots;
use crate::storage::cache::l2_transactions::L2Transactions;
use crate::storage::cache::l3_accounts::L3Accounts;
use crate::storage::database::DatabaseStorage;
use crate::utils::errors::Result;
use std::sync::Arc;

pub struct MultiCache {
    l1: Arc<L1HotSlots>,
    l2: Arc<L2Transactions>,
    l3: Arc<L3Accounts>,
    db: Arc<dyn DatabaseStorage>,
}

impl MultiCache {
    pub fn new(l1_size: usize, l2_size: usize, db: Arc<dyn DatabaseStorage>) -> Self {
        Self {
            l1: Arc::new(L1HotSlots::new(l1_size)),
            l2: Arc::new(L2Transactions::new(l2_size)),
            l3: Arc::new(L3Accounts::new(db.clone())),
            db,
        }
    }

    pub async fn store_slot(&self, slot: Slot) -> Result<()> {
        self.l1.insert(slot.clone()).await;
        let status_str = match slot.status {
            crate::core::types::SlotStatus::Processed => "Processed",
            crate::core::types::SlotStatus::Confirmed => "Confirmed",
            crate::core::types::SlotStatus::Finalized => "Finalized",
        };
        self.db
            .store_slot(slot.slot, slot.timestamp, slot.parent, status_str)
            .await?;

        Ok(())
    }

    pub async fn store_transaction(&self, tx: Transaction) -> Result<()> {
        self.l2.insert(tx.clone()).await;
        self.db.store_transaction(tx).await?;

        Ok(())
    }

    pub async fn get_account(&self, address: &str) -> Result<Option<AccountState>> {
        self.l3.get(address).await
    }

    pub async fn store_account(&self, account: AccountState) -> Result<()> {
        self.l3.insert(account).await
    }
}