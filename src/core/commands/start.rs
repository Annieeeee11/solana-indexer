use crate::context::AppContext;
use crate::core::commands::display::{account_change_handler, slot_and_tx_handlers};
use crate::core::runtime::{self, IndexerOptions};
use crate::core::slot_pipeline::SlotPipelineOptions;
use crate::utils::cli_animations::Cli;
use crate::utils::errors::Result;

pub async fn start() -> Result<()> {
    Cli::banner();

    let ctx = AppContext::new().await?;
    let (on_slot, on_tx) = slot_and_tx_handlers();
    let on_account_change = account_change_handler();

    let watch_count = runtime::collect_watch_accounts(&ctx).await?.len();
    let api_port = ctx.config.api_port;

    Cli::success("Indexer running");
    Cli::info(ctx.streaming_mode_label());
    if watch_count > 0 {
        Cli::info(&format!(
            "Slot pipeline + {} account(s) watching in parallel",
            watch_count
        ));
    } else {
        Cli::info("Slot pipeline running (add wallets or WATCH_ACCOUNTS to enable watcher)");
    }
    if let Some(port) = api_port {
        Cli::info(&format!(
            "HTTP query API on port {port} (parallel — set API_PORT in .env)"
        ));
    }
    Cli::info("Ctrl+C to stop");

    runtime::run(
        ctx,
        IndexerOptions {
            api_port,
            ..IndexerOptions::default()
        },
        on_slot,
        on_tx,
        on_account_change,
    )
    .await?;

    Cli::info("Indexer stopped");
    Ok(())
}

pub async fn track_slots(leaders: bool, transactions: bool, watch_accounts: bool) -> Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;

    let mut info = vec!["slots"];
    if leaders {
        info.push("leaders");
    }
    if transactions {
        info.push("txs");
    }
    if watch_accounts {
        info.push("accounts");
    }
    Cli::success(&format!("Tracking: {}", info.join(", ")));

    Cli::info(ctx.streaming_mode_label());
    if watch_accounts {
        let watch_count = runtime::collect_watch_accounts(&ctx).await?.len();
        if watch_count > 0 {
            Cli::info(&format!(
                "Slot pipeline + {} account(s) watching in parallel",
                watch_count
            ));
        } else {
            Cli::info(
                "Account watching enabled (add wallets or WATCH_ACCOUNTS to watch addresses)",
            );
        }
    }
    Cli::info("Ctrl+C to stop");

    let (on_slot, on_tx) = slot_and_tx_handlers();
    let pipeline = SlotPipelineOptions {
        show_leaders: leaders,
        show_transactions: transactions,
    };

    if watch_accounts {
        runtime::run(
            ctx,
            IndexerOptions {
                pipeline,
                watch_accounts: true,
                api_port: None,
            },
            on_slot,
            on_tx,
            account_change_handler(),
        )
        .await?;
    } else {
        crate::core::slot_pipeline::run(ctx, pipeline, on_slot, on_tx).await?;
    }

    Ok(())
}
