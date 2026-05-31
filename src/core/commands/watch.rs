use crate::context::AppContext;
use crate::core::account_watcher::AccountWatcher;
use crate::core::commands::display::run_watcher_with_cli;
use crate::utils::cli_animations::Cli;
use crate::utils::errors::Result;

pub async fn watch_account(address: String) -> Result<()> {
    Cli::banner();

    let ctx = AppContext::new().await?;
    Cli::connecting(&ctx.config.rpc.solana_rpc_url);

    let watcher = AccountWatcher::with_accounts(
        ctx.account_source(),
        ctx.cache,
        vec![address.clone()],
    );

    match watcher.fetch_account(&address).await {
        Ok(acc) => Cli::account(&acc),
        Err(e) => {
            Cli::error("Fetch", &e.to_string());
            return Ok(());
        }
    }

    Cli::info("Watching for changes... (Ctrl+C to stop)");
    run_watcher_with_cli(&watcher).await
}

pub async fn wallet_watch() -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    let wallets = ctx.cache.get_active_wallets().await?;

    if wallets.is_empty() {
        Cli::warning("No wallets. Add with: indexer track wallets add -a <address>");
        return Ok(());
    }

    let watcher = AccountWatcher::with_accounts(ctx.account_source(), ctx.cache, wallets.clone());

    Cli::success(&format!("Watching {} wallet(s)", wallets.len()));
    Cli::info("Ctrl+C to stop");
    run_watcher_with_cli(&watcher).await
}
