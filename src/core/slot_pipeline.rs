use crate::context::AppContext;
use crate::core::channels;
use crate::core::slot_tracker::SlotTracker;
use crate::core::types::{Slot, TransactionInfo};
use crate::data_sources::YellowstoneSource;
use crate::data_sources::yellowstone_grpc::YellowstoneGrpc;
use crate::utils::config::RpcConfig;
use crate::utils::errors::Result;
use crate::utils::shutdown;
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

pub fn yellowstone_client(config: &RpcConfig) -> Option<Arc<dyn YellowstoneSource>> {
    config.yellowstone_grpc_url.as_ref().map(|url| {
        tracing::info!("Using Yellowstone gRPC");
        Arc::new(YellowstoneGrpc::new(
            url,
            config.yellowstone_grpc_token.clone(),
        )) as Arc<dyn YellowstoneSource>
    })
}

/// User-facing description of the slot pipeline data source mode.
pub fn streaming_mode_label(yellowstone: &Option<Arc<dyn YellowstoneSource>>) -> &'static str {
    if yellowstone.is_some() {
        "Yellowstone gRPC primary (RPC fallback if unavailable)"
    } else {
        "RPC polling (set YELLOWSTONE_GRPC_URL for gRPC streaming)"
    }
}

/// Spawns SlotTracker and the display consumer. Caller owns shutdown via `shutdown` sender.
pub fn spawn(
    ctx: AppContext,
    yellowstone: Option<Arc<dyn YellowstoneSource>>,
    options: SlotPipelineOptions,
    on_slot: Arc<dyn Fn(Slot, Option<String>) + Send + Sync>,
    on_tx: Arc<dyn Fn(TransactionInfo) + Send + Sync>,
    shutdown: broadcast::Sender<()>,
) -> (JoinHandle<()>, JoinHandle<()>) {
    let (slot_tx, slot_rx) = channels::slot_channel();
    let (tx_tx, tx_rx) = channels::transaction_channel();

    let tracker = SlotTracker::new(
        yellowstone,
        ctx.slot_source(),
        ctx.cache.clone(),
        slot_tx,
        tx_tx,
    );

    let tracker_handle = tokio::spawn(async move {
        if let Err(e) = tracker.start().await {
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

        loop {
            tokio::select! {
                biased;
                _ = shutdown_rx.recv() => break,
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

    (tracker_handle, display_handle)
}

/// Standalone pipeline run (track slots) with its own Ctrl+C handler.
pub async fn run(
    ctx: AppContext,
    yellowstone: Option<Arc<dyn YellowstoneSource>>,
    options: SlotPipelineOptions,
    on_slot: Arc<dyn Fn(Slot, Option<String>) + Send + Sync>,
    on_tx: Arc<dyn Fn(TransactionInfo) + Send + Sync>,
) -> Result<()> {
    let shutdown_tx = shutdown::channel();
    let (mut tracker_handle, mut display_handle) =
        spawn(ctx, yellowstone, options, on_slot, on_tx, shutdown_tx.clone());

    shutdown::wait_ctrl_c_or_2(
        shutdown_tx,
        "Shutdown signal received, stopping pipeline...",
        &mut tracker_handle,
        "Slot tracker",
        &mut display_handle,
        "Display",
    )
    .await;

    shutdown::abort_join_handles([tracker_handle, display_handle]).await;

    Ok(())
}
