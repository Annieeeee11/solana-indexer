use clap::{Parser, Subcommand};
use solana_stream_indexer::core::commands;
use solana_stream_indexer::utils::errors::Result;
use solana_stream_indexer::utils::logger;

#[derive(Parser)]
#[command(name = "solana-stream-indexer", about = "Solana blockchain stream indexer")]
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
    /// Start HTTP query API (indexer layer)
    Serve {
        #[arg(short, long)]
        port: Option<u16>,
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
        /// Also watch active wallets + WATCH_ACCOUNTS (same as `indexer start`)
        #[arg(short, long)]
        watch_accounts: bool,
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
async fn main() -> Result<()> {
    logger::init_logger();

    match Args::parse().cmd {
        Cmd::Start => commands::start().await,
        Cmd::Track { what } => match what {
            Track::Slots {
                leaders,
                transactions,
                watch_accounts,
            } => commands::track_slots(leaders, transactions, watch_accounts).await,
            Track::Wallets { action } => match action {
                Wallet::Add { address, name } => commands::wallet_add(address, name).await,
                Wallet::Remove { address } => commands::wallet_remove(address).await,
                Wallet::Watch => commands::wallet_watch().await,
                Wallet::List { detailed } => commands::wallet_list(detailed).await,
            },
        },
        Cmd::Watch { address } => commands::watch_account(address).await,
        Cmd::Query { what } => match what {
            Query::Latest => commands::query_latest().await,
            Query::Slot { number } => commands::query_slot(number).await,
            Query::Tx { signature } => commands::query_tx(signature).await,
            Query::Account { address } => commands::query_account(address).await,
        },
        Cmd::Serve { port } => commands::serve(port).await,
    }
}
