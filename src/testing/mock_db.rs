use crate::core::types::{AccountState, Slot, Transaction};
use crate::storage::database::DatabaseStorage;
use crate::utils::errors::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

/// In-memory `DatabaseStorage` for unit tests.
pub struct MockDatabase {
    slots: Mutex<HashMap<u64, Slot>>,
    txs: Mutex<HashMap<String, Transaction>>,
    wallets: Mutex<Vec<String>>,
}

impl MockDatabase {
    pub fn new() -> Self {
        Self {
            slots: Mutex::new(HashMap::new()),
            txs: Mutex::new(HashMap::new()),
            wallets: Mutex::new(vec![]),
        }
    }

    pub fn with_wallets(wallets: Vec<String>) -> Self {
        Self {
            slots: Mutex::new(HashMap::new()),
            txs: Mutex::new(HashMap::new()),
            wallets: Mutex::new(wallets),
        }
    }
}

#[async_trait]
impl DatabaseStorage for MockDatabase {
    async fn store_slot(&self, slot: &Slot) -> Result<()> {
        self.slots.lock().unwrap().insert(slot.slot, slot.clone());
        Ok(())
    }

    async fn store_account(&self, _account: AccountState) -> Result<()> {
        Ok(())
    }

    async fn get_account(&self, _address: &str) -> Result<Option<AccountState>> {
        Ok(None)
    }

    async fn get_slot(&self, slot: u64) -> Result<Option<Slot>> {
        Ok(self.slots.lock().unwrap().get(&slot).cloned())
    }

    async fn store_transaction(&self, tx: Transaction) -> Result<()> {
        self.txs
            .lock()
            .unwrap()
            .insert(tx.signature.clone(), tx);
        Ok(())
    }

    async fn get_transaction(&self, signature: &str) -> Result<Option<Transaction>> {
        Ok(self.txs.lock().unwrap().get(signature).cloned())
    }

    async fn get_latest_slot(&self) -> Result<Option<Slot>> {
        Ok(self
            .slots
            .lock()
            .unwrap()
            .values()
            .max_by_key(|s| s.slot)
            .cloned())
    }

    async fn add_wallet(&self, _address: String, _name: Option<String>) -> Result<()> {
        Ok(())
    }

    async fn remove_wallet(&self, _address: &str) -> Result<()> {
        Ok(())
    }

    async fn list_wallets(
        &self,
        _active_only: bool,
    ) -> Result<Vec<(String, Option<String>, i64)>> {
        Ok(vec![])
    }

    async fn get_active_wallets(&self) -> Result<Vec<String>> {
        Ok(self.wallets.lock().unwrap().clone())
    }
}
