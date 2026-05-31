use crate::core::types::{Slot, SlotStatus};

/// Shared test slot fixture (parent defaults to `n - 1`).
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
