use crate::context::AppContext;
use crate::core::commands::display::{account_change_handler, slot_and_tx_handlers};
use crate::core::runtime::{self, IndexerOptions};
use crate::core::slot_pipeline;
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
    use crate::core::slot_pipeline::SlotPipelineOptions;

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
