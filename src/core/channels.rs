use crate::core::types::{Slot, TransactionInfo};
use tokio::sync::mpsc;

/// Slot stream capacity (architecture: 1000).
pub const SLOT_CHANNEL_CAPACITY: usize = 1000;

/// Transaction stream capacity (architecture: 10000).
pub const TX_CHANNEL_CAPACITY: usize = 10000;

pub fn slot_channel() -> (mpsc::Sender<Slot>, mpsc::Receiver<Slot>) {
    mpsc::channel(SLOT_CHANNEL_CAPACITY)
}

pub fn transaction_channel() -> (mpsc::Sender<TransactionInfo>, mpsc::Receiver<TransactionInfo>) {
    mpsc::channel(TX_CHANNEL_CAPACITY)
}
