use crate::core::types::{AccountState, Slot, SlotStatus, TransactionInfo};
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
}

impl MockSlotSource {
    pub fn new(leader: impl Into<String>) -> Self {
        Self {
            leader: leader.into(),
        }
    }
}

#[async_trait]
impl SlotSource for MockSlotSource {
    async fn subscribe_slots(&self) -> Result<mpsc::Receiver<Slot>> {
        let (tx, rx) = mpsc::channel(1);
        drop(tx);
        Ok(rx)
    }

    async fn get_block_with_transactions(&self, _slot: u64) -> Result<Vec<TransactionInfo>> {
        Ok(vec![])
    }

    async fn get_slot_leader(&self) -> Result<String> {
        Ok(self.leader.clone())
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

pub fn sample_slot(n: u64) -> Slot {
    Slot {
        slot: n,
        parent: Some(n.saturating_sub(1)),
        status: SlotStatus::Confirmed,
        timestamp: 1,
        block_hash: None,
        block_height: None,
    }
}
