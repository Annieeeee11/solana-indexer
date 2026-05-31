use crate::api;
use crate::context::AppContext;
use crate::core::account_watcher::AccountWatcher;
use crate::core::slot_pipeline::{self, SlotPipelineOptions};
use crate::core::types::{AccountState, Slot, TransactionInfo};
use crate::utils::errors::Result;
use crate::utils::shutdown;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

#[derive(Clone, Copy)]
pub struct IndexerOptions {
    pub pipeline: SlotPipelineOptions,
    pub watch_accounts: bool,
    /// When `Some`, spawns HTTP query API in parallel (also reads `API_PORT` from config on start).
    pub api_port: Option<u16>,
}

impl Default for IndexerOptions {
    fn default() -> Self {
        Self {
            pipeline: SlotPipelineOptions::default(),
            watch_accounts: true,
            api_port: None,
        }
    }
}

pub async fn collect_watch_accounts(ctx: &AppContext) -> Result<Vec<String>> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for addr in ctx.cache.get_active_wallets().await? {
        if seen.insert(addr.clone()) {
            out.push(addr);
        }
    }
    for addr in &ctx.config.watch_accounts {
        if seen.insert(addr.clone()) {
            out.push(addr.clone());
        }
    }
    Ok(out)
}

pub async fn run(
    ctx: AppContext,
    options: IndexerOptions,
    on_slot: Arc<dyn Fn(Slot, Option<String>) + Send + Sync>,
    on_tx: Arc<dyn Fn(TransactionInfo) + Send + Sync>,
    on_account_change: Arc<dyn Fn(&str, &AccountState, &AccountState) + Send + Sync>,
) -> Result<()> {
    let (shutdown_tx, _) = broadcast::channel(1);

    let (mut tracker_handle, mut display_handle) = slot_pipeline::spawn(
        ctx.clone(),
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

    let api_port = options.api_port.or(ctx.config.api_port);
    let mut api_handle = spawn_api_server(&ctx, api_port, &shutdown_tx);

    wait_for_shutdown(
        shutdown_tx,
        &mut tracker_handle,
        &mut display_handle,
        &mut watcher_handle,
        &mut api_handle,
    )
    .await;

    let mut handles = vec![tracker_handle, display_handle];
    if let Some(h) = watcher_handle {
        handles.push(h);
    }
    if let Some(h) = api_handle {
        handles.push(h);
    }
    shutdown::shutdown_handles(handles).await;

    Ok(())
}

fn spawn_api_server(
    ctx: &AppContext,
    port: Option<u16>,
    shutdown_tx: &broadcast::Sender<()>,
) -> Option<JoinHandle<()>> {
    let port = port?;
    let cache = ctx.cache.clone();
    let shutdown_rx = shutdown_tx.subscribe();
    tracing::info!("Starting HTTP query API on port {port} (parallel with indexer)");
    Some(tokio::spawn(async move {
        if let Err(e) = api::serve_until_shutdown(cache, port, shutdown_rx).await {
            tracing::error!("HTTP API error: {e}");
        }
    }))
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
            tracing::error!("Account watcher error: {e}");
        }
    })))
}

async fn wait_for_shutdown(
    shutdown_tx: broadcast::Sender<()>,
    tracker_handle: &mut JoinHandle<()>,
    display_handle: &mut JoinHandle<()>,
    watcher_handle: &mut Option<JoinHandle<()>>,
    api_handle: &mut Option<JoinHandle<()>>,
) {
    const MSG: &str = "Shutdown signal received, stopping indexer...";

    let mut tasks: Vec<(&mut JoinHandle<()>, &str)> = vec![
        (tracker_handle, "Slot tracker"),
        (display_handle, "Display"),
    ];
    if let Some(w) = watcher_handle.as_mut() {
        tasks.push((w, "Account watcher"));
    }
    if let Some(a) = api_handle.as_mut() {
        tasks.push((a, "HTTP API"));
    }

    shutdown::wait_ctrl_c_or_any(shutdown_tx, MSG, &mut tasks).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::context::test_context;

    #[tokio::test]
    async fn collect_watch_accounts_dedupes_env_and_db() {
        let ctx = test_context(
            vec!["wallet1".into()],
            vec!["wallet2".into(), "wallet1".into()],
            None,
        );
        let addrs = collect_watch_accounts(&ctx).await.unwrap();
        assert_eq!(addrs.len(), 2);
        assert!(addrs.contains(&"wallet1".to_string()));
        assert!(addrs.contains(&"wallet2".to_string()));
    }
}
