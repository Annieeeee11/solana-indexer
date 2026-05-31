use crate::core::types::{Slot, TransactionInfo};
use crate::data_sources::{SlotSource, YellowstoneSource};
use crate::storage::cache::multi_cache::MultiCache;
use crate::utils::errors::{IndexerError, Result};
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct SlotTracker {
    yellowstone: Option<Arc<dyn YellowstoneSource>>,
    rpc: Arc<dyn SlotSource>,
    cache: Arc<MultiCache>,
    slot_tx: mpsc::Sender<Slot>,
    tx_tx: mpsc::Sender<TransactionInfo>,
}

impl SlotTracker {
    pub fn new(
        yellowstone: Option<Arc<dyn YellowstoneSource>>,
        rpc: Arc<dyn SlotSource>,
        cache: Arc<MultiCache>,
        slot_tx: mpsc::Sender<Slot>,
        tx_tx: mpsc::Sender<TransactionInfo>,
    ) -> Self {
        Self {
            yellowstone,
            rpc,
            cache,
            slot_tx,
            tx_tx,
        }
    }

    pub async fn start(&self) -> Result<()> {
        if let Some(yellowstone) = &self.yellowstone {
            match yellowstone.subscribe_with_transactions().await {
                Ok((slot_stream, tx_stream)) => {
                    tracing::info!("Using Yellowstone gRPC (real-time streaming)");

                    match self.stream_from_yellowstone(slot_stream, tx_stream).await {
                        Ok(_) => return Ok(()),
                        Err(e) => {
                            tracing::warn!("Yellowstone stream failed: {}, falling back to RPC", e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Yellowstone connection failed: {}, using RPC polling", e);
                }
            }
        } else {
            tracing::info!("No Yellowstone configured, using RPC polling");
        }
        tracing::info!("Using RPC polling (fallback mode)");
        self.poll_from_rpc().await
    }

    async fn stream_from_yellowstone(
        &self,
        mut slot_stream: mpsc::Receiver<Slot>,
        mut tx_stream: mpsc::Receiver<TransactionInfo>,
    ) -> Result<()> {
        loop {
            tokio::select! {
                slot = slot_stream.recv() => {
                    let Some(slot) = slot else {
                        return Err(IndexerError::ChannelError(
                            "Yellowstone slot stream closed".into(),
                        ));
                    };

                    tracing::debug!("Received slot from Yellowstone: {}", slot.slot);

                    if self.slot_tx.send(slot.clone()).await.is_err() {
                        tracing::error!("Failed to send slot to pipeline");
                        continue;
                    }

                    if let Err(e) = self.cache.store_slot(slot).await {
                        tracing::error!("Failed to cache slot: {}", e);
                    }
                }
                tx = tx_stream.recv() => {
                    let Some(tx) = tx else {
                        return Err(IndexerError::ChannelError(
                            "Yellowstone tx stream closed".into(),
                        ));
                    };

                    tracing::debug!("Received tx from Yellowstone: {}", tx.signature);

                    if self.tx_tx.send(tx.clone()).await.is_err() {
                        tracing::error!("Failed to send tx to pipeline");
                        continue;
                    }

                    if let Err(e) = self.cache.store_transaction(tx.into()).await {
                        tracing::error!("Failed to cache transaction: {}", e);
                    }
                }
            }
        }
    }

    async fn poll_from_rpc(&self) -> Result<()> {
        let mut slot_stream = self.rpc.subscribe_slots().await?;

        while let Some(slot) = slot_stream.recv().await {
            tracing::debug!("Received slot from RPC: {}", slot.slot);

            if let Ok(transactions) = self.rpc.get_block_with_transactions(slot.slot).await {
                for tx in transactions {
                    if self.tx_tx.send(tx.clone()).await.is_err() {
                        tracing::error!("Failed to send tx to pipeline");
                        continue;
                    }

                    if let Err(e) = self.cache.store_transaction(tx.into()).await {
                        tracing::error!("Failed to cache transaction: {}", e);
                    }
                }
            }

            if self.slot_tx.send(slot.clone()).await.is_err() {
                tracing::error!("Failed to send slot to pipeline");
                continue;
            }

            if let Err(e) = self.cache.store_slot(slot).await {
                tracing::error!("Failed to cache slot: {}", e);
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::channels;
    use crate::core::types::{Slot, SlotStatus};
    use crate::storage::cache::multi_cache::MultiCache;
    use crate::testing::mock_db::MockDatabase;
    use crate::testing::mock_sources::MockSlotSource;
    use std::sync::Arc;
    use tokio::time::{timeout, Duration};

    fn sample_slot(n: u64) -> Slot {
        Slot {
            slot: n,
            parent: Some(n.saturating_sub(1)),
            status: SlotStatus::Confirmed,
            timestamp: 1,
            block_hash: None,
            block_height: None,
        }
    }

    #[tokio::test]
    async fn rpc_poll_forwards_slot_to_channel_and_cache() {
        let slot = sample_slot(42);
        let rpc: Arc<dyn SlotSource> =
            Arc::new(MockSlotSource::with_slots("mock-leader", vec![slot.clone()]));
        let db = Arc::new(MockDatabase::new());
        let cache = Arc::new(MultiCache::new(10, 10, 10, db));
        let (slot_tx, mut slot_rx) = channels::slot_channel();
        let (tx_tx, _tx_rx) = channels::transaction_channel();

        let tracker = SlotTracker::new(None, rpc, cache.clone(), slot_tx, tx_tx);
        let tracker_task = tokio::spawn(async move {
            tracker.start().await
        });

        let received = timeout(Duration::from_secs(2), slot_rx.recv())
            .await
            .expect("should receive slot within 2s")
            .expect("slot channel should stay open until slot is sent");
        assert_eq!(received.slot, 42);

        let cached = cache
            .get_slot(42)
            .await
            .expect("cache read should succeed")
            .expect("slot should be stored in cache");
        assert_eq!(cached.slot, 42);

        tracker_task.abort();
    }
}
