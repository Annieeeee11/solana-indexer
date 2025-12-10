use crate::core::types::Slot;
use std::collections::BTreeMap;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct L1HotSlots {
    cache: Arc<RwLock<BTreeMap<u64, Slot>>>,
    max_size: usize,
}

impl L1HotSlots {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: Arc::new(RwLock::new(BTreeMap::new())),
            max_size,
        }
    }

    pub async fn get(&self, slot: u64) -> Option<Slot> {
        self.cache.read().await.get(&slot).cloned()
    }

    pub async fn insert(&self, slot: Slot) {
        let mut cache = self.cache.write().await;

        if cache.len() >= self.max_size {
            if let Some(entry) = cache.first_entry() {
                entry.remove();
            }
        }

        cache.insert(slot.slot, slot);
    }

    pub async fn get_latest_slot(&self) -> Option<Slot> {
        let cache = self.cache.read().await;
        cache.values()
            .max_by_key(|slot| slot.slot)
            .cloned()
    }

    pub async fn get_all_slots(&self) -> Vec<Slot> {
        self.cache.read().await
            .values()
            .cloned()
            .collect()
    }
}