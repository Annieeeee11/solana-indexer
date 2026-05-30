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

pub async fn collect_watch_accounts(ctx: &AppContext) -> Result<Vec<String>> {
    let mut addresses = ctx.cache.get_active_wallets().await?;
    for addr in &ctx.config.watch_accounts {
        if !addresses.contains(addr) {
            addresses.push(addr.clone());
        }
    }
    Ok(addresses)
}

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

    let mut watcher_handle = spawn_account_watcher(
        &ctx,
        options.watch_accounts,
        &shutdown_tx,
        on_account_change,
    )
    .await?;

    wait_for_shutdown(
        shutdown_tx,
        &mut tracker_handle,
        &mut display_handle,
        &mut watcher_handle,
    )
    .await;

    abort_handles(tracker_handle, display_handle, watcher_handle).await;

    Ok(())
}

async fn spawn_account_watcher(
    ctx: &AppContext,
    watch_accounts: bool,
    shutdown_tx: &broadcast::Sender<()>,
    on_account_change: Arc<dyn Fn(&str, &AccountState, &AccountState) + Send + Sync>,
) -> Result<Option<JoinHandle<()>>> {
    if !watch_accounts {
        return Ok(None);
    }

    let accounts = collect_watch_accounts(ctx).await?;
    if accounts.is_empty() {
        tracing::info!("No wallets or WATCH_ACCOUNTS configured; account watcher idle");
        return Ok(None);
    }

    tracing::info!("Watching {} account(s) in parallel", accounts.len());
    let watcher = AccountWatcher::with_accounts(
        ctx.account_source(),
        ctx.cache.clone(),
        accounts,
    );
    watcher.seed_accounts().await?;

    let shutdown_rx = shutdown_tx.subscribe();
    let callback = on_account_change;
    Ok(Some(tokio::spawn(async move {
        if let Err(e) = watcher
            .run_until(
                move |addr, prev, curr| callback(addr, prev, curr),
                shutdown_rx,
            )
            .await
        {
            tracing::error!("Account watcher error: {}", e);
        }
    })))
}

async fn wait_for_shutdown(
    shutdown_tx: broadcast::Sender<()>,
    tracker_handle: &mut JoinHandle<()>,
    display_handle: &mut JoinHandle<()>,
    watcher_handle: &mut Option<JoinHandle<()>>,
) {
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("Shutdown signal received, stopping indexer...");
        let _ = shutdown_tx.send(());
    };

    if let Some(wh) = watcher_handle.as_mut() {
        tokio::select! {
            () = shutdown => {}
            result = tracker_handle => log_join_error("Slot tracker", result),
            result = display_handle => log_join_error("Display", result),
            result = wh => log_join_error("Account watcher", result),
        }
    } else {
        tokio::select! {
            () = shutdown => {}
            result = tracker_handle => log_join_error("Slot tracker", result),
            result = display_handle => log_join_error("Display", result),
        }
    }
}

fn log_join_error(label: &str, result: std::result::Result<(), tokio::task::JoinError>) {
    if let Err(e) = result {
        tracing::error!("{label} task failed: {e}");
    }
}

async fn abort_handles(
    mut tracker_handle: JoinHandle<()>,
    mut display_handle: JoinHandle<()>,
    watcher_handle: Option<JoinHandle<()>>,
) {
    tracker_handle.abort();
    display_handle.abort();
    if let Some(h) = watcher_handle {
        h.abort();
        let _ = h.await;
    }
    let _ = tracker_handle.await;
    let _ = display_handle.await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::cache::multi_cache::MultiCache;
    use crate::testing::mock_db::MockDatabase;
    use crate::utils::config::{CacheConfig, Config, RpcConfig, StorageConfig};
    use std::path::PathBuf;

    fn test_context(wallets: Vec<String>, watch_accounts: Vec<String>) -> AppContext {
        let db = Arc::new(MockDatabase::with_wallets(wallets));
        AppContext {
            config: Config {
                rpc: RpcConfig {
                    solana_rpc_url: "http://localhost".into(),
                    yellowstone_grpc_url: None,
                    yellowstone_grpc_token: None,
                },
                storage: StorageConfig {
                    sqlite_path: PathBuf::from("test.db"),
                    postgres_url: None,
                },
                cache: CacheConfig {
                    l1_size: 10,
                    l2_size: 10,
                    l3_size: 10,
                },
                watch_accounts,
                api_port: None,
            },
            cache: Arc::new(MultiCache::new(10, 10, 10, db)),
            rpc: Arc::new(crate::data_sources::solana_rpc::SolanaRpc::new("http://localhost")),
        }
    }

    #[tokio::test]
    async fn collect_watch_accounts_dedupes_env_and_db() {
        let ctx = test_context(
            vec!["wallet1".into()],
            vec!["wallet2".into(), "wallet1".into()],
        );
        let addrs = collect_watch_accounts(&ctx).await.unwrap();
        assert_eq!(addrs.len(), 2);
        assert!(addrs.contains(&"wallet1".to_string()));
        assert!(addrs.contains(&"wallet2".to_string()));
    }
}
