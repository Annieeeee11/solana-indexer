use crate::core::types::AccountState;
use crate::data_sources::solana_rpc::SolanaRpc;
use crate::storage::cache::multi_cache::MultiCache;
use crate::utils::errors::Result;
use std::sync::Arc;
use tokio::time::{interval, Duration};

const POLL_INTERVAL_SECS: u64 = 5;

pub struct AccountWatcher {
    rpc: Arc<SolanaRpc>,
    cache: Arc<MultiCache>,
    accounts_to_watch: Vec<String>,
}

impl AccountWatcher {
    pub fn new(rpc: Arc<SolanaRpc>, cache: Arc<MultiCache>) -> Self {
        Self {
            rpc,
            cache,
            accounts_to_watch: vec![],
        }
    }

    pub fn add_account(&mut self, address: String) {
        self.accounts_to_watch.push(address);
    }

    /// Fetches one account and seeds the cache (used before `run`).
    pub async fn fetch_account(&self, address: &str) -> Result<AccountState> {
        let account = self.rpc.get_account(address).await?;
        self.cache.store_account(account.clone()).await?;
        Ok(account)
    }

    /// Polls all registered accounts; calls `on_change` when balance or data differs from cache.
    pub async fn run<F>(&self, mut on_change: F) -> Result<()>
    where
        F: FnMut(&str, &AccountState, &AccountState),
    {
        let mut ticker = interval(Duration::from_secs(POLL_INTERVAL_SECS));
        let accounts = self.accounts_to_watch.clone();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    for address in &accounts {
                        match self.rpc.get_account(address).await {
                            Ok(current) => {
                                if let Some(previous) = self.cache.get_account(address).await? {
                                    if previous.lamports != current.lamports
                                        || previous.data != current.data
                                    {
                                        on_change(address, &previous, &current);
                                    }
                                }
                                self.cache.store_account(current).await?;
                            }
                            Err(e) => {
                                tracing::warn!("Failed to fetch account {}: {}", address, e);
                            }
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Shutdown signal received, stopping account watcher...");
                    return Ok(());
                }
            }
        }
    }

    pub async fn start(&self) -> Result<()> {
        self.run(|address, _, _| {
            tracing::info!("Account {} changed", address);
        })
        .await
    }
}
