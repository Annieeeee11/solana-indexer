use crate::context::AppContext;
use crate::core::channels;
use crate::core::slot_tracker::SlotTracker;
use crate::core::types::{Slot, TransactionInfo};
use crate::data_sources::yellowstone_grpc::YellowstoneGrpc;
use crate::utils::config::RpcConfig;
use crate::utils::errors::Result;
use std::sync::Arc;

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

pub fn yellowstone_client(config: &RpcConfig) -> Option<Arc<YellowstoneGrpc>> {
    config.yellowstone_grpc_url.as_ref().map(|url| {
        tracing::info!("Using Yellowstone gRPC");
        Arc::new(YellowstoneGrpc::new(
            url,
            config.yellowstone_grpc_token.clone(),
        ))
    })
}

/// Runs SlotTracker → channels → handlers. Persistence stays in SlotTracker.
pub async fn run(
    ctx: AppContext,
    yellowstone: Option<Arc<YellowstoneGrpc>>,
    options: SlotPipelineOptions,
    wait_for_tracker: bool,
    on_slot: Arc<dyn Fn(Slot, Option<String>) + Send + Sync>,
    on_tx: Arc<dyn Fn(TransactionInfo) + Send + Sync>,
) -> Result<()> {
    let (slot_tx, slot_rx) = channels::slot_channel();
    let (tx_tx, tx_rx) = channels::transaction_channel();

    let tracker = SlotTracker::new(yellowstone, ctx.rpc.clone(), ctx.cache.clone(), slot_tx, tx_tx);
    let tracker_handle = tokio::spawn(async move {
        if let Err(e) = tracker.start().await {
            tracing::error!("Tracker error: {}", e);
        }
    });

    let rpc = ctx.rpc;
    let show_leaders = options.show_leaders;
    let show_transactions = options.show_transactions;

    let display_handle = tokio::spawn(async move {
        let mut slot_rx = slot_rx;
        let mut tx_rx = tx_rx;

        loop {
            tokio::select! {
                Some(slot) = slot_rx.recv() => {
                    let leader = if show_leaders {
                        rpc.get_slot_leader().await.ok()
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

    if wait_for_tracker {
        tokio::select! {
            _ = tracker_handle => {}
            _ = display_handle => {}
        }
    } else {
        let _ = display_handle.await;
        tracker_handle.abort();
    }

    Ok(())
}
