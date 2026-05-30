use clap::{Parser, Subcommand};
use colored::*;
use solana_indexer::context::AppContext;
use solana_indexer::core::account_watcher::AccountWatcher;
use solana_indexer::core::slot_pipeline::{self, SlotPipelineOptions};
use solana_indexer::core::types::{Slot, TransactionInfo};
use solana_indexer::utils::cli_animations::Cli;
use solana_indexer::utils::logger;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "indexer", about = "Solana blockchain indexer")]
struct Args {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Start the indexer
    Start,
    /// Track blockchain data
    Track {
        #[command(subcommand)]
        what: Track,
    },
    /// Watch a specific account
    Watch { address: String },
}

#[derive(Subcommand)]
enum Track {
    /// Track slots (persisted by SlotTracker → MultiCache)
    Slots {
        #[arg(short, long)]
        leaders: bool,
        #[arg(short, long)]
        transactions: bool,
    },
    /// Manage wallets
    Wallets {
        #[command(subcommand)]
        action: Wallet,
    },
}

#[derive(Subcommand)]
enum Wallet {
    Add {
        #[arg(short, long)]
        address: String,
        #[arg(short, long)]
        name: Option<String>,
    },
    Remove {
        #[arg(short, long)]
        address: String,
    },
    Watch,
    List {
        #[arg(short, long)]
        detailed: bool,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logger::init_logger();

    match Args::parse().cmd {
        Cmd::Start => start().await,
        Cmd::Track { what } => match what {
            Track::Slots { leaders, transactions } => track_slots(leaders, transactions).await,
            Track::Wallets { action } => match action {
                Wallet::Add { address, name } => wallet_add(address, name).await,
                Wallet::Remove { address } => wallet_remove(address).await,
                Wallet::Watch => wallet_watch().await,
                Wallet::List { detailed } => wallet_list(detailed).await,
            },
        },
        Cmd::Watch { address } => watch_account(address).await,
    }
}

fn display_handlers() -> (
    Arc<dyn Fn(Slot, Option<String>) + Send + Sync>,
    Arc<dyn Fn(TransactionInfo) + Send + Sync>,
) {
    let on_slot = Arc::new(|slot: Slot, leader: Option<String>| {
        Cli::slot(&slot, leader.as_deref())
    });
    let on_tx = Arc::new(|tx: TransactionInfo| {
        Cli::transaction(
            &tx.signature,
            tx.slot,
            tx.success,
            tx.fee,
            &tx.program,
            tx.instructions,
            tx.compute_units,
        )
    });
    (on_slot, on_tx)
}

async fn start() -> anyhow::Result<()> {
    Cli::banner();

    let ctx = AppContext::new().await?;
    let yellowstone = slot_pipeline::yellowstone_client(&ctx.config.rpc);
    let (on_slot, on_tx) = display_handlers();

    Cli::success("Indexer running");
    Cli::info("Ctrl+C to stop");

    slot_pipeline::run(
        ctx,
        yellowstone,
        SlotPipelineOptions::default(),
        true,
        on_slot,
        on_tx,
    )
    .await?;

    Ok(())
}

async fn track_slots(leaders: bool, transactions: bool) -> anyhow::Result<()> {
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

    let (on_slot, on_tx) = display_handlers();

    slot_pipeline::run(
        ctx,
        None,
        SlotPipelineOptions {
            show_leaders: leaders,
            show_transactions: transactions,
        },
        false,
        on_slot,
        on_tx,
    )
    .await?;

    Ok(())
}

async fn wallet_add(address: String, name: Option<String>) -> anyhow::Result<()> {
    Cli::banner();
    let (_, db) = AppContext::db_only().await?;
    db.add_wallet(address.clone(), name).await?;
    Cli::success(&format!("Added: {}", address));
    Ok(())
}

async fn wallet_remove(address: String) -> anyhow::Result<()> {
    Cli::banner();
    let (_, db) = AppContext::db_only().await?;
    db.remove_wallet(&address).await?;
    Cli::success(&format!("Removed: {}", address));
    Ok(())
}

async fn wallet_watch() -> anyhow::Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    let wallets = ctx.db.get_active_wallets().await?;
    
    if wallets.is_empty() {
        Cli::warning("No wallets. Add with: indexer track wallets add -a <address>");
        return Ok(());
    }
    
    let mut watcher = AccountWatcher::new(ctx.rpc, ctx.cache);
    for w in &wallets {
        watcher.add_account(w.clone());
    }
    
    Cli::success(&format!("Watching {} wallet(s)", wallets.len()));
    watcher.start().await?;
    Ok(())
}

async fn wallet_list(detailed: bool) -> anyhow::Result<()> {
    Cli::banner();
    let (_, db) = AppContext::db_only().await?;
    let wallets = db.list_wallets(true).await?;
    
    if wallets.is_empty() {
        Cli::warning("No wallets.");
        return Ok(());
    }
    
    println!();
    for (addr, name, _) in wallets {
        let n = name.as_deref().unwrap_or("unnamed");
        if detailed {
            println!("    {} {}", addr.bright_white(), format!("({})", n).bright_black());
        } else {
            Cli::wallet(&addr, n);
        }
    }
    println!();
    
    Ok(())
}

async fn watch_account(address: String) -> anyhow::Result<()> {
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

    Cli::info("Watching for changes...");
    watcher
        .run(|addr, prev, curr| {
            Cli::account_change(addr, prev.lamports, curr.lamports, curr.slot);
        })
        .await?;
    Ok(())
}
