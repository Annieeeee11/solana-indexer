use colored::*;
use crate::context::AppContext;
use crate::core::account_watcher::AccountWatcher;
use crate::core::commands::display::{account_change_handler, slot_and_tx_handlers};
use crate::core::runtime::{self, IndexerOptions};
use crate::core::slot_pipeline::{self, SlotPipelineOptions};
use crate::utils::cli_animations::Cli;
use crate::utils::errors::Result;

pub async fn start() -> Result<()> {
    Cli::banner();

    let ctx = AppContext::new().await?;
    let yellowstone = slot_pipeline::yellowstone_client(&ctx.config.rpc);
    let (on_slot, on_tx) = slot_and_tx_handlers();
    let on_account_change = account_change_handler();

    let watch_count = runtime::collect_watch_accounts(&ctx).await?.len();
    Cli::success("Indexer running");
    if watch_count > 0 {
        Cli::info(&format!(
            "Slot pipeline + {} account(s) watching in parallel",
            watch_count
        ));
    } else {
        Cli::info("Slot pipeline running (add wallets or WATCH_ACCOUNTS to enable watcher)");
    }
    Cli::info("Ctrl+C to stop");

    runtime::run(
        ctx,
        yellowstone,
        IndexerOptions::default(),
        on_slot,
        on_tx,
        on_account_change,
    )
    .await?;

    Cli::info("Indexer stopped");
    Ok(())
}

pub async fn track_slots(leaders: bool, transactions: bool) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;

    let mut info = vec!["slots"];
    if leaders {
        info.push("leaders");
    }
    if transactions {
        info.push("txs");
    }
    Cli::success(&format!("Tracking: {}", info.join(", ")));
    Cli::info("Ctrl+C to stop");

    let (on_slot, on_tx) = slot_and_tx_handlers();

    slot_pipeline::run(
        ctx,
        None,
        SlotPipelineOptions {
            show_leaders: leaders,
            show_transactions: transactions,
        },
        on_slot,
        on_tx,
    )
    .await?;

    Ok(())
}

pub async fn watch_account(address: String) -> Result<()> {
    Cli::banner();

    let ctx = AppContext::new().await?;
    Cli::connecting(&ctx.config.rpc.solana_rpc_url);

    let mut watcher = AccountWatcher::new(ctx.rpc, ctx.cache);
    watcher.add_account(address.clone());

    match watcher.fetch_account(&address).await {
        Ok(acc) => Cli::account(&acc),
        Err(e) => {
            Cli::error("Fetch", &e.to_string());
            return Ok(());
        }
    }

    Cli::info("Watching for changes... (Ctrl+C to stop)");
    watcher
        .run(|addr, prev, curr| {
            Cli::account_change(addr, prev.lamports, curr.lamports, curr.slot);
        })
        .await?;
    Ok(())
}

pub async fn wallet_watch() -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    let wallets = ctx.cache.get_active_wallets().await?;

    if wallets.is_empty() {
        Cli::warning("No wallets. Add with: indexer track wallets add -a <address>");
        return Ok(());
    }

    let watcher = AccountWatcher::with_accounts(ctx.rpc, ctx.cache, wallets.clone());

    Cli::success(&format!("Watching {} wallet(s)", wallets.len()));
    Cli::info("Ctrl+C to stop");
    watcher
        .run(|addr, prev, curr| {
            Cli::account_change(addr, prev.lamports, curr.lamports, curr.slot);
        })
        .await?;
    Ok(())
}

pub async fn wallet_add(address: String, name: Option<String>) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    ctx.cache.add_wallet(address.clone(), name).await?;
    Cli::success(&format!("Added: {}", address));
    Ok(())
}

pub async fn wallet_remove(address: String) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    ctx.cache.remove_wallet(&address).await?;
    Cli::success(&format!("Removed: {}", address));
    Ok(())
}

pub async fn wallet_list(detailed: bool) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    let wallets = ctx.cache.list_wallets(true).await?;

    if wallets.is_empty() {
        Cli::warning("No wallets.");
        return Ok(());
    }

    println!();
    for (addr, name, _) in wallets {
        let n = name.as_deref().unwrap_or("unnamed");
        if detailed {
            println!(
                "    {} {}",
                addr.bright_white(),
                format!("({})", n).bright_black()
            );
        } else {
            Cli::wallet(&addr, n);
        }
    }
    println!();

    Ok(())
}

pub async fn query_latest() -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    match ctx.cache.get_latest_slot().await? {
        Some(slot) => {
            Cli::success("Latest slot (L1 → DB fallback)");
            Cli::slot(&slot, None);
        }
        None => Cli::warning("No slots indexed yet. Run `indexer start` first."),
    }
    Ok(())
}

pub async fn query_slot(number: u64) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    match ctx.cache.get_slot(number).await? {
        Some(slot) => {
            Cli::success(&format!("Slot {} (L1 → DB fallback)", number));
            Cli::slot(&slot, None);
        }
        None => Cli::warning(&format!("Slot {} not found in cache or DB", number)),
    }
    Ok(())
}

pub async fn query_tx(signature: String) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    match ctx.cache.get_transaction(&signature).await? {
        Some(tx) => {
            Cli::success("Transaction (L2 → DB fallback)");
            println!();
            println!(
                "    {} {}",
                "Sig:".bright_white(),
                tx.signature.bright_cyan()
            );
            println!(
                "    {} {}  {} {}",
                "Slot:".bright_white(),
                tx.slot.to_string().bright_yellow(),
                "Fee:".bright_white(),
                tx.fee
            );
            println!(
                "    {} {}",
                "Success:".bright_white(),
                if tx.success {
                    "yes".bright_green()
                } else {
                    "no".bright_red()
                }
            );
            println!(
                "    {} {}",
                "Accounts:".bright_white(),
                tx.accounts.len()
            );
            println!();
        }
        None => Cli::warning("Transaction not found in cache or DB"),
    }
    Ok(())
}

pub async fn query_account(address: String) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    match ctx.cache.get_account(&address).await? {
        Some(acc) => {
            Cli::success("Account (L3 → DB fallback)");
            Cli::account(&acc);
        }
        None => Cli::warning("Account not found in cache or DB"),
    }
    Ok(())
}
