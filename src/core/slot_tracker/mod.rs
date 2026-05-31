use crate::core::enrichment_limiter::EnrichmentLimiter;
use crate::core::types::{Slot, TransactionInfo};
use crate::data_sources::{SlotSource, YellowstoneSource};
use crate::storage::cache::multi_cache::MultiCache;
use crate::utils::errors::{IndexerError, Result};
use crate::utils::metrics::IndexerMetrics;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};

/// Delay before background metadata backfill (lets blocks become available on RPC).
const METADATA_BACKFILL_DELAY: Duration = Duration::from_secs(2);

pub struct SlotTracker {
    yellowstone: Option<Arc<dyn YellowstoneSource>>,
    rpc: Arc<dyn SlotSource>,
    enrich_rpc: Arc<dyn SlotSource>,
    cache: Arc<MultiCache>,
    metrics: Arc<IndexerMetrics>,
    enrich_limiter: EnrichmentLimiter,
    slot_tx: mpsc::Sender<Slot>,
    tx_tx: mpsc::Sender<TransactionInfo>,
}

impl SlotTracker {
    pub fn new(
        yellowstone: Option<Arc<dyn YellowstoneSource>>,
        rpc: Arc<dyn SlotSource>,
        enrich_rpc: Arc<dyn SlotSource>,
        cache: Arc<MultiCache>,
        metrics: Arc<IndexerMetrics>,
        enrich_min_interval: Duration,
        slot_tx: mpsc::Sender<Slot>,
        tx_tx: mpsc::Sender<TransactionInfo>,
    ) -> Self {
        Self {
            yellowstone,
            rpc,
            enrich_rpc,
            cache,
            metrics,
            enrich_limiter: EnrichmentLimiter::new(enrich_min_interval),
            slot_tx,
            tx_tx,
        }
    }

    pub async fn start_until(&self, mut shutdown: broadcast::Receiver<()>) -> Result<()> {
        if let Some(yellowstone) = &self.yellowstone {
            match yellowstone.subscribe_with_transactions().await {
                Ok((slot_stream, tx_stream)) => {
                    tracing::info!("Using Yellowstone gRPC (real-time streaming)");

                    match self
                        .stream_from_yellowstone(slot_stream, tx_stream, &mut shutdown)
                        .await
                    {
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
        self.poll_from_rpc(&mut shutdown).await
    }

    async fn maybe_enrich_slot(&self, slot: &mut Slot) {
        if !self.enrich_limiter.should_enrich(slot) {
            if slot.block_hash.is_none() || slot.block_height.is_none() {
                self.metrics
                    .enrich_rate_limited
                    .fetch_add(1, Ordering::Relaxed);
            }
            return;
        }

        if self.enrich_rpc.enrich_slot_block_metadata(slot).await.is_ok() {
            if slot.block_hash.is_some() || slot.block_height.is_some() {
                self.metrics.enrich_success.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn spawn_metadata_backfill(&self, slot: Slot) {
        if slot.block_hash.is_some() && slot.block_height.is_some() {
            return;
        }

        let cache = self.cache.clone();
        let enrich_rpc = self.enrich_rpc.clone();
        let metrics = self.metrics.clone();

        tokio::spawn(async move {
            tokio::time::sleep(METADATA_BACKFILL_DELAY).await;
            let mut updated = slot;
            if enrich_rpc
                .enrich_slot_block_metadata(&mut updated)
                .await
                .is_ok()
                && (updated.block_hash.is_some() || updated.block_height.is_some())
            {
                metrics.enrich_success.fetch_add(1, Ordering::Relaxed);
                if let Err(e) = cache.store_slot(updated).await {
                    tracing::debug!("Metadata backfill store failed: {e}");
                }
            }
        });
    }

    async fn forward_slot(&self, mut slot: Slot) -> bool {
        self.maybe_enrich_slot(&mut slot).await;

        if self.slot_tx.send(slot.clone()).await.is_err() {
            tracing::debug!("Pipeline channel closed, stopping slot tracker");
            return true;
        }

        if let Err(e) = self.cache.store_slot(slot.clone()).await {
            tracing::error!("Failed to cache slot: {}", e);
        } else {
            self.metrics.maybe_log_periodic(100);
            self.spawn_metadata_backfill(slot);
        }

        false
    }

    async fn forward_tx(&self, tx: TransactionInfo) -> bool {
        if self.tx_tx.send(tx.clone()).await.is_err() {
            tracing::debug!("Pipeline channel closed, stopping slot tracker");
            return true;
        }

        if let Err(e) = self.cache.store_transaction(tx.into()).await {
            tracing::error!("Failed to cache transaction: {}", e);
        }

        false
    }

    async fn stream_from_yellowstone(
        &self,
        mut slot_stream: mpsc::Receiver<Slot>,
        mut tx_stream: mpsc::Receiver<TransactionInfo>,
        shutdown: &mut broadcast::Receiver<()>,
    ) -> Result<()> {
        loop {
            tokio::select! {
                biased;
                _ = shutdown.recv() => {
                    tracing::debug!("Slot tracker stopping (shutdown signal)");
                    return Ok(());
                }
                slot = slot_stream.recv() => {
                    let Some(slot) = slot else {
                        return Err(IndexerError::ChannelError(
                            "Yellowstone slot stream closed".into(),
                        ));
                    };

                    tracing::debug!(slot = slot.slot, source = "yellowstone", "ingest_slot");

                    if self.forward_slot(slot).await {
                        return Ok(());
                    }
                }
                tx = tx_stream.recv() => {
                    let Some(tx) = tx else {
                        return Err(IndexerError::ChannelError(
                            "Yellowstone tx stream closed".into(),
                        ));
                    };

                    tracing::debug!(signature = %tx.signature, source = "yellowstone", "ingest_tx");

                    if self.forward_tx(tx).await {
                        return Ok(());
                    }
                }
            }
        }
    }

    async fn poll_from_rpc(&self, shutdown: &mut broadcast::Receiver<()>) -> Result<()> {
        let mut slot_stream = self.rpc.subscribe_slots().await?;

        loop {
            tokio::select! {
                biased;
                _ = shutdown.recv() => {
                    tracing::debug!("Slot tracker stopping (shutdown signal)");
                    return Ok(());
                }
                slot = slot_stream.recv() => {
                    let Some(slot) = slot else {
                        return Ok(());
                    };

                    tracing::debug!(slot = slot.slot, source = "rpc", "ingest_slot");

                    if let Ok(transactions) = self.rpc.get_block_with_transactions(slot.slot).await {
                        for tx in transactions {
                            if self.forward_tx(tx).await {
                                return Ok(());
                            }
                        }
                    }

                    if self.forward_slot(slot).await {
                        return Ok(());
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::channels;
    use crate::storage::cache::multi_cache::MultiCache;
    use crate::testing::fixtures::sample_slot;
    use crate::testing::mock_db::MockDatabase;
    use crate::testing::mock_sources::MockSlotSource;
    use crate::utils::metrics::IndexerMetrics;
    use tokio::sync::broadcast;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn rpc_poll_forwards_slot_to_channel_and_cache() {
        let slot = sample_slot(42);
        let metrics = IndexerMetrics::new();
        let rpc: Arc<dyn SlotSource> =
            Arc::new(MockSlotSource::with_slots("mock-leader", vec![slot.clone()]));
        let db = Arc::new(MockDatabase::new());
        let cache = Arc::new(MultiCache::new(10, 10, 10, db, metrics.clone()));
        let (slot_tx, mut slot_rx) = channels::slot_channel();
        let (tx_tx, _tx_rx) = channels::transaction_channel();

        let tracker = SlotTracker::new(
            None,
            rpc.clone(),
            rpc,
            cache.clone(),
            metrics,
            Duration::from_millis(0),
            slot_tx,
            tx_tx,
        );
        let (_shutdown_tx, shutdown_rx) = broadcast::channel(1);
        let tracker_task = tokio::spawn(async move {
            tracker.start_until(shutdown_rx).await
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

    #[tokio::test]
    async fn yellowstone_path_enriches_block_metadata_via_rpc() {
        let mut slot = sample_slot(99);
        assert!(slot.block_hash.is_none());

        let rpc: Arc<dyn SlotSource> = Arc::new(MockSlotSource::new("leader"));
        rpc.enrich_slot_block_metadata(&mut slot)
            .await
            .expect("enrichment should succeed");
        assert_eq!(slot.block_hash.as_deref(), Some("mock-hash-99"));
        assert_eq!(slot.block_height, Some(99));
    }
}
