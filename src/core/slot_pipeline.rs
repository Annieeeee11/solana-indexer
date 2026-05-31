use crate::context::AppContext;
use crate::core::channels;
use crate::core::leader_cache::LeaderCache;
use crate::core::slot_tracker::SlotTracker;
use crate::core::types::{Slot, TransactionInfo};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

#[derive(Clone, Copy)]
pub struct SlotPipelineOptions {
    pub show_leaders: bool,
    pub show_transactions: bool,
}

impl Default for SlotPipelineOptions {
    fn default() -> Self {
        Self {
            show_leaders: true,
            show_transactions: true,
        }
    }
}

/// Spawns SlotTracker and the display consumer. Caller owns shutdown via `shutdown` sender.
pub fn spawn(
    ctx: AppContext,
    options: SlotPipelineOptions,
    on_slot: Arc<dyn Fn(Slot, Option<String>) + Send + Sync>,
    on_tx: Arc<dyn Fn(TransactionInfo) + Send + Sync>,
    shutdown: broadcast::Sender<()>,
) -> (JoinHandle<()>, JoinHandle<()>) {
    let (slot_tx, slot_rx) = channels::slot_channel();
    let (tx_tx, tx_rx) = channels::transaction_channel();

    let tracker = SlotTracker::new(
        ctx.yellowstone_source(),
        ctx.slot_source(),
        ctx.cache.clone(),
        ctx.metrics.clone(),
        ctx.slot_enrich_interval(),
        slot_tx,
        tx_tx,
    );

    let shutdown_rx_tracker = shutdown.subscribe();

    let tracker_handle = tokio::spawn(async move {
        if let Err(e) = tracker.start_until(shutdown_rx_tracker).await {
            tracing::error!("Tracker error: {}", e);
        }
    });

    let rpc = ctx.slot_source();
    let show_leaders = options.show_leaders;
    let show_transactions = options.show_transactions;
    let mut shutdown_rx = shutdown.subscribe();

    let display_handle = tokio::spawn(async move {
        let mut slot_rx = slot_rx;
        let mut tx_rx = tx_rx;
        let mut leader_cache = LeaderCache::new();

        loop {
            tokio::select! {
                biased;
                _ = shutdown_rx.recv() => break,
                Some(slot) = slot_rx.recv() => {
                    let leader = if show_leaders {
                        leader_cache.leader_for_slot(slot.slot, &rpc).await
                    } else {
                        None
                    };
                    on_slot(slot, leader);
                }
                Some(tx) = tx_rx.recv(), if show_transactions => {
                    on_tx(tx);
                }
                else => break,
            }
        }
    });

    (tracker_handle, display_handle)
}

