use crate::context::AppContext;
use crate::core::account_watcher::AccountWatcher;
use crate::core::slot_pipeline::{self, SlotPipelineOptions};
use crate::core::types::{AccountState, Slot, TransactionInfo};
use crate::data_sources::yellowstone_grpc::YellowstoneGrpc;
use crate::utils::errors::Result;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

#[derive(Clone, Copy)]
pub struct IndexerOptions {
    pub pipeline: SlotPipelineOptions,
    pub watch_accounts: bool,
}

impl Default for IndexerOptions {
    fn default() -> Self {
        Self {
            pipeline: SlotPipelineOptions::default(),
            watch_accounts: true,
        }
    }
}

/// Collects active DB wallets plus `WATCH_ACCOUNTS` env addresses (deduped).
pub async fn collect_watch_accounts(ctx: &AppContext) -> Result<Vec<String>> {
    let mut addresses = ctx.cache.get_active_wallets().await?;
    for addr in &ctx.config.watch_accounts {
        if !addresses.contains(addr) {
            addresses.push(addr.clone());
        }
    }
    Ok(addresses)
}

/// Runs slot pipeline and account watcher in parallel with a single Ctrl+C shutdown.
pub async fn run(
    ctx: AppContext,
    yellowstone: Option<Arc<YellowstoneGrpc>>,
    options: IndexerOptions,
    on_slot: Arc<dyn Fn(Slot, Option<String>) + Send + Sync>,
    on_tx: Arc<dyn Fn(TransactionInfo) + Send + Sync>,
    on_account_change: Arc<dyn Fn(&str, &AccountState, &AccountState) + Send + Sync>,
) -> Result<()> {
    let (shutdown_tx, _) = broadcast::channel(1);

    let (mut tracker_handle, mut display_handle) = slot_pipeline::spawn(
        ctx.clone(),
        yellowstone,
        options.pipeline,
        on_slot,
        on_tx,
        shutdown_tx.clone(),
    );

    let mut watcher_handle: Option<JoinHandle<()>> = if options.watch_accounts {
        let accounts = collect_watch_accounts(&ctx).await?;
        if accounts.is_empty() {
            tracing::info!("No wallets or WATCH_ACCOUNTS configured; account watcher idle");
            None
        } else {
            tracing::info!("Watching {} account(s) in parallel", accounts.len());
            let watcher =
                AccountWatcher::with_accounts(ctx.rpc.clone(), ctx.cache.clone(), accounts);
            watcher.seed_accounts().await?;

            let shutdown_rx = shutdown_tx.subscribe();
            let callback = on_account_change;
            Some(tokio::spawn(async move {
                if let Err(e) = watcher
                    .run_until(
                        move |addr, prev, curr| {
                            callback(addr, prev, curr);
                        },
                        shutdown_rx,
                    )
                    .await
                {
                    tracing::error!("Account watcher error: {}", e);
                }
            }))
        }
    } else {
        None
    };

    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("Shutdown signal received, stopping indexer...");
        let _ = shutdown_tx.send(());
    };

    match &mut watcher_handle {
        Some(wh) => {
            tokio::select! {
                () = shutdown => {}
                result = &mut tracker_handle => {
                    if let Err(e) = result {
                        tracing::error!("Slot tracker task failed: {}", e);
                    }
                }
                result = &mut display_handle => {
                    if let Err(e) = result {
                        tracing::error!("Display task failed: {}", e);
                    }
                }
                result = wh => {
                    if let Err(e) = result {
                        tracing::error!("Account watcher task failed: {}", e);
                    }
                }
            }
        }
        None => {
            tokio::select! {
                () = shutdown => {}
                result = &mut tracker_handle => {
                    if let Err(e) = result {
                        tracing::error!("Slot tracker task failed: {}", e);
                    }
                }
                result = &mut display_handle => {
                    if let Err(e) = result {
                        tracing::error!("Display task failed: {}", e);
                    }
                }
            }
        }
    }

    tracker_handle.abort();
    display_handle.abort();
    if let Some(h) = watcher_handle {
        h.abort();
        let _ = h.await;
    }
    let _ = tracker_handle.await;
    let _ = display_handle.await;

    Ok(())
}
