use clap::{Parser, Subcommand};
use colored::*;
use solana_indexer::context::AppContext;
use solana_indexer::core::account_watcher::AccountWatcher;
use solana_indexer::core::slot_tracker::SlotTracker;
use solana_indexer::data_sources::yellowstone_grpc::YellowstoneGrpc;
use solana_indexer::utils::cli_animations::Cli;
use solana_indexer::utils::config::Config;
use solana_indexer::utils::logger;
use std::sync::Arc;
use tokio::sync::mpsc;

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
    /// Track slots
    Slots {
        #[arg(short, long)]
        leaders: bool,
        #[arg(short, long)]
        transactions: bool,
        #[arg(short, long)]
        save: bool,
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
            Track::Slots { leaders, transactions, save } => track_slots(leaders, transactions, save).await,
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

async fn start() -> anyhow::Result<()> {
    Cli::banner();
    
    let ctx = AppContext::new().await?;
    
    let yellowstone = ctx.config.rpc.yellowstone_grpc_url.as_ref().map(|url| {
        tracing::info!("Using Yellowstone gRPC");
        Arc::new(YellowstoneGrpc::new(url))
    });
    
    let (slot_tx, mut slot_rx) = mpsc::channel(1000);
    let (tx_tx, mut tx_rx) = mpsc::channel(10000);

    let tracker = SlotTracker::new(yellowstone, ctx.rpc.clone(), ctx.cache.clone(), slot_tx, tx_tx);
    let tracker_handle = tokio::spawn(async move {
        if let Err(e) = tracker.start().await {
            tracing::error!("Tracker error: {}", e);
        }
    });
    
    let rpc = ctx.rpc.clone();
    let cache = ctx.cache.clone();
    
    let proc_handle = tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(slot) = slot_rx.recv() => {
                    let leader = rpc.get_slot_leader().await.ok();
                    Cli::slot(&slot, leader.as_deref());
                    let _ = cache.store_slot(slot).await;
                }
                Some(tx) = tx_rx.recv() => {
                    Cli::transaction(&tx.signature, tx.slot, tx.success, tx.fee, &tx.program, tx.instructions, tx.compute_units);
                }
                else => break,
            }
        }
    });

    Cli::success("Indexer running");
    Cli::info("Ctrl+C to stop");
    
    tokio::select! {
        _ = tracker_handle => {}
        _ = proc_handle => {}
    }

    Ok(())
}

async fn track_slots(leaders: bool, transactions: bool, save: bool) -> 
anyhow::Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    
    let (slot_tx, mut slot_rx) = mpsc::channel(1000);
    let (tx_tx, mut tx_rx) = mpsc::channel(10000);
    
    let tracker = SlotTracker::new(None, ctx.rpc.clone(), ctx.cache.clone(), slot_tx, tx_tx);
    let handle = tokio::spawn(async move {
        let _ = tracker.start().await;
    });
    
    let mut info = vec!["slots"];
    if leaders { info.push("leaders"); }
    if transactions { info.push("txs"); }
    if save { info.push("saving"); }
    
    Cli::success(&format!("Tracking: {}", info.join(", ")));
    
    let rpc = ctx.rpc.clone();
    let cache = ctx.cache.clone();
    
    loop {
        tokio::select! {
            Some(slot) = slot_rx.recv() => {
                let leader = if leaders { rpc.get_slot_leader().await.ok() } else { None };
                Cli::slot(&slot, leader.as_deref());
                if save { let _ = cache.store_slot(slot).await; }
            }
            Some(tx) = tx_rx.recv(), if transactions => {
                Cli::transaction(&tx.signature, tx.slot, tx.success, tx.fee, &tx.program, tx.instructions, tx.compute_units);
            }
            else => break,
        }
    }
    
    handle.abort();
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
    const POLL_INTERVAL_SECS: u64 = 5;
    
    let config = Config::load()?;
    Cli::connecting(&config.rpc.solana_rpc_url);
    
    let ctx = AppContext::new().await?;
    
    match ctx.rpc.get_account(&address).await {
        Ok(acc) => {
            Cli::account(&acc);
            ctx.cache.store_account(acc).await?;
        }
        Err(e) => {
            Cli::error("Fetch", &e.to_string());
            return Ok(());
        }
    }
    
    Cli::info("Watching for changes...");
    
    let mut tick = tokio::time::interval(tokio::time::Duration::from_secs(POLL_INTERVAL_SECS));
    
    loop {
        tick.tick().await;
        
        if let Ok(curr) = ctx.rpc.get_account(&address).await {
            if let Ok(Some(prev)) = ctx.cache.get_account(&address).await {
                if prev.lamports != curr.lamports || prev.data != curr.data {
                    Cli::account_change(&address, prev.lamports, curr.lamports, curr.slot);
                }
            }
            let _ = ctx.cache.store_account(curr).await;
        }
    }
}
