use crate::core::types::{AccountState, Slot, TransactionInfo};
use crate::data_sources::{AccountSource, SlotSource};
use crate::utils::errors::Result;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Mutex};
use tokio::sync::mpsc;

/// Mock account fetcher for unit tests.
pub struct MockAccountSource {
    accounts: Mutex<HashMap<String, AccountState>>,
}

impl MockAccountSource {
    pub fn new() -> Self {
        Self {
            accounts: Mutex::new(HashMap::new()),
        }
    }

    pub fn insert(&self, account: AccountState) {
        self.accounts
            .lock()
            .unwrap()
            .insert(account.address.clone(), account);
    }

    pub fn set_lamports(&self, address: &str, lamports: u64) {
        if let Some(acc) = self.accounts.lock().unwrap().get_mut(address) {
            acc.lamports = lamports;
        }
    }
}

#[async_trait]
impl AccountSource for MockAccountSource {
    async fn get_account(&self, address: &str) -> Result<AccountState> {
        self.accounts
            .lock()
            .unwrap()
            .get(address)
            .cloned()
            .ok_or_else(|| {
                crate::utils::errors::IndexerError::RpcError(format!(
                    "mock account not found: {address}"
                ))
            })
    }
}

/// Mock slot source for unit tests.
pub struct MockSlotSource {
    leader: String,
    slots: Vec<Slot>,
}

impl MockSlotSource {
    pub fn new(leader: impl Into<String>) -> Self {
        Self {
            leader: leader.into(),
            slots: vec![],
        }
    }

    pub fn with_slots(leader: impl Into<String>, slots: Vec<Slot>) -> Self {
        Self {
            leader: leader.into(),
            slots,
        }
    }
}

#[async_trait]
impl SlotSource for MockSlotSource {
    async fn subscribe_slots(&self) -> Result<mpsc::Receiver<Slot>> {
        let (tx, rx) = mpsc::channel(self.slots.len().max(1));
        let slots = self.slots.clone();
        tokio::spawn(async move {
            for slot in slots {
                if tx.send(slot).await.is_err() {
                    break;
                }
            }
        });
        Ok(rx)
    }

    async fn get_block_with_transactions(&self, _slot: u64) -> Result<Vec<TransactionInfo>> {
        Ok(vec![])
    }

    async fn get_leader_at_slot(&self, _slot: u64) -> Result<String> {
        Ok(self.leader.clone())
    }

    async fn get_slot_leader(&self) -> Result<String> {
        Ok(self.leader.clone())
    }

    async fn enrich_slot_block_metadata(&self, slot: &mut Slot) -> Result<()> {
        if slot.block_hash.is_none() {
            slot.block_hash = Some(format!("mock-hash-{}", slot.slot));
        }
        if slot.block_height.is_none() {
            slot.block_height = Some(slot.slot);
        }
        Ok(())
    }
}

pub fn sample_account(address: &str, lamports: u64) -> AccountState {
    AccountState {
        address: address.to_string(),
        slot: 1,
        lamports,
        owner: "11111111111111111111111111111111".into(),
        executable: false,
        data: vec![],
        rent_epoch: 0,
    }
}
