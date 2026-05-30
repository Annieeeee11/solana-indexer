use clap::{Parser, Subcommand};
use colored::*;
use solana_indexer::context::AppContext;
use solana_indexer::core::account_watcher::AccountWatcher;
use solana_indexer::core::runtime::{self, IndexerOptions};
use solana_indexer::core::slot_pipeline::{self, SlotPipelineOptions};
use solana_indexer::core::types::{AccountState, Slot, TransactionInfo};
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
    /// Start the indexer (slot pipeline + account watcher in parallel)
    Start,
    /// Track blockchain data
    Track {
        #[command(subcommand)]
        what: Track,
    },
    /// Watch a specific account
    Watch { address: String },
    /// Query indexed data via MultiCache (L1/L2/L3 → DB fallback)
    Query {
        #[command(subcommand)]
        what: Query,
    },
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
enum Query {
    /// Latest indexed slot
    Latest,
    /// Slot by number
    Slot { number: u64 },
    /// Transaction by signature
    Tx { signature: String },
    /// Account state by address
    Account { address: String },
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
        Cmd::Query { what } => match what {
            Query::Latest => query_latest().await,
            Query::Slot { number } => query_slot(number).await,
            Query::Tx { signature } => query_tx(signature).await,
            Query::Account { address } => query_account(address).await,
        },
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

fn account_change_handler() -> Arc<dyn Fn(&str, &AccountState, &AccountState) + Send + Sync> {
    Arc::new(|addr, prev, curr| {
        Cli::account_change(addr, prev.lamports, curr.lamports, curr.slot);
    })
}

async fn start() -> anyhow::Result<()> {
    Cli::banner();

    let ctx = AppContext::new().await?;
    let yellowstone = slot_pipeline::yellowstone_client(&ctx.config.rpc);
    let (on_slot, on_tx) = display_handlers();
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
    Cli::info("Ctrl+C to stop");

    let (on_slot, on_tx) = display_handlers();

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

async fn wallet_add(address: String, name: Option<String>) -> anyhow::Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    ctx.cache.add_wallet(address.clone(), name).await?;
    Cli::success(&format!("Added: {}", address));
    Ok(())
}

async fn wallet_remove(address: String) -> anyhow::Result<()> {
    Cli::banner();
    let ctx = AppContext::new().await?;
    ctx.cache.remove_wallet(&address).await?;
    Cli::success(&format!("Removed: {}", address));
    Ok(())
}

async fn wallet_watch() -> anyhow::Result<()> {
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

async fn wallet_list(detailed: bool) -> anyhow::Result<()> {
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

    Cli::info("Watching for changes... (Ctrl+C to stop)");
    watcher
        .run(|addr, prev, curr| {
            Cli::account_change(addr, prev.lamports, curr.lamports, curr.slot);
        })
        .await?;
    Ok(())
}

async fn query_latest() -> anyhow::Result<()> {
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

async fn query_slot(number: u64) -> anyhow::Result<()> {
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

async fn query_tx(signature: String) -> anyhow::Result<()> {
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

async fn query_account(address: String) -> anyhow::Result<()> {
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
