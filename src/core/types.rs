use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Slot {
    pub slot: u64,
    pub parent: Option<u64>,
    pub status: SlotStatus,
    pub timestamp: i64,
    pub block_hash: Option<String>,
    pub block_height: Option<u64>,
}

// Slot confirmation status.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SlotStatus {
    Processed,
    Confirmed,
    Finalized,
}

impl FromStr for SlotStatus {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(match s {
            "Finalized" => SlotStatus::Finalized,
            "Confirmed" => SlotStatus::Confirmed,
            _ => SlotStatus::Processed,
        })
    }
}

impl SlotStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            SlotStatus::Processed => "Processed",
            SlotStatus::Confirmed => "Confirmed",
            SlotStatus::Finalized => "Finalized",
        }
    }
}

// Basic transaction data for storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub signature: String,
    pub slot: u64,
    pub block_time: Option<i64>,
    pub fee: u64,
    pub success: bool,
    pub accounts: Vec<String>,
}

// Rich transaction info for display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionInfo {
    pub signature: String,
    pub slot: u64,
    pub success: bool,
    pub fee: u64,
    pub program: String,
    pub instructions: usize,
    pub compute_units: u64,
    pub accounts: Vec<String>,
    pub timestamp: i64,
}

impl From<TransactionInfo> for Transaction {
    fn from(info: TransactionInfo) -> Self {
        Self {
            signature: info.signature,
            slot: info.slot,
            block_time: Some(info.timestamp),
            fee: info.fee,
            success: info.success,
            accounts: info.accounts,
        }
    }
}

// Current state of a Solana account.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountState {
    pub address: String,
    pub slot: u64,
    pub lamports: u64,
    pub owner: String,
    pub executable: bool,
    pub data: Vec<u8>,
    pub rent_epoch: u64,
}