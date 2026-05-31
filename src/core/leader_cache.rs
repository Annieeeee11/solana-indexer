use crate::data_sources::SlotSource;
use std::collections::HashMap;
use std::sync::Arc;

const MAX_ENTRIES: usize = 128;

/// Caches slot → leader lookups to avoid one RPC call per streamed slot.
pub struct LeaderCache {
    entries: HashMap<u64, String>,
    order: Vec<u64>,
}

impl LeaderCache {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: Vec::new(),
        }
    }

    pub async fn leader_for_slot(
        &mut self,
        slot: u64,
        rpc: &Arc<dyn SlotSource>,
    ) -> Option<String> {
        if let Some(leader) = self.entries.get(&slot) {
            return Some(leader.clone());
        }

        let leader = rpc.get_leader_at_slot(slot).await.ok()?;
        self.insert(slot, leader.clone());
        Some(leader)
    }

    fn insert(&mut self, slot: u64, leader: String) {
        if self.entries.contains_key(&slot) {
            return;
        }
        if self.entries.len() >= MAX_ENTRIES {
            if let Some(oldest) = self.order.first().copied() {
                self.order.remove(0);
                self.entries.remove(&oldest);
            }
        }
        self.order.push(slot);
        self.entries.insert(slot, leader);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::mock_sources::MockSlotSource;

    #[tokio::test]
    async fn caches_leader_per_slot() {
        let rpc: Arc<dyn SlotSource> = Arc::new(MockSlotSource::new("leader-a"));
        let mut cache = LeaderCache::new();

        let first = cache.leader_for_slot(100, &rpc).await.unwrap();
        let second = cache.leader_for_slot(100, &rpc).await.unwrap();
        assert_eq!(first, "leader-a");
        assert_eq!(second, "leader-a");
    }
}
